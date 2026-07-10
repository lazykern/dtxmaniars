# Input and Lane Profile Management Design

Date: 2026-07-11
Status: Approved

## Goal

Separate keyboard bindings from MIDI mappings and let players keep named keyboard, MIDI, and lane-arrangement profiles. Players select each profile type independently.

This design replaces the current mixed Bindings tab and its single `bindings.toml`. It also replaces the single unnamed custom lane arrangement stored in `layout.toml`.

## Current state

The current implementation has two useful boundaries that this work preserves:

```text
bindings.toml -> EChannel -> logical drum pad -> judgment
                       |
layout.toml  -> EChannel -> display column -> rendering
```

`dtx-config::InputBindings` stores keyboard keys, MIDI notes, MIDI port selection, and velocity threshold in one file. The Bindings tab shows keyboard and MIDI chips in each channel row, and its capture state listens for either source.

`dtx-layout::LaneArrangement` supports Classic, NX Type-B, NX Type-D, and one `Custom` arrangement. The Lanes tab cycles built-ins and edits the active arrangement. `layout.toml` stores that arrangement beside widget layout data. BocuD exposes the lane types in `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Drums.cs:L290-L307` and persists the selected type in `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:L1835` and `:L3025-L3027`.

Current gaps:

- Keyboard and MIDI editing share one surface and one capture state.
- Players cannot select keyboard and MIDI configurations independently.
- Each profile type has one global active value and no named user profiles.
- Editing a named lane preset changes the runtime marker to generic `Custom`.
- Current close behavior auto-saves instead of protecting a known-good profile.

## Decisions

| Area | Decision |
|---|---|
| Profile relationship | Keyboard, MIDI, and lane profiles remain independent. |
| Navigation | Replace Bindings with one Controls tab containing `Keyboard | MIDI`. |
| Storage | Use one versioned TOML registry per profile type. |
| Save model | Edit drafts, then use Save or Save As. |
| Built-ins | Keep built-ins immutable in code. |
| Binding target | Keep `EChannel`; lane profiles cannot change judgment routing. |
| Widget layout | Keep widget state in `layout.toml`; move lane selection and custom lanes to their registry. |
| Profile names | Use registry keys, compare case-insensitively, reserve built-in names. |

Rejected alternatives:

- Bundled setups couple physical inputs to presentation and prevent profile reuse.
- One file per profile adds path sanitization, slug collisions, and partial filesystem failures before import/export needs exist.
- Named snapshots around `bindings.toml` and `layout.toml` preserve the mixed schema and allow the active copy to drift from its named source.
- A dedicated profile-manager screen adds navigation for lists expected to stay small.

## Architecture

```text
Customize
  Controls
    Keyboard -> KeyboardProfileDraft -> keyboard-profiles.toml
    MIDI     -> MidiProfileDraft     -> midi-profiles.toml

  Lanes      -> LaneProfileDraft     -> lane-profiles.toml
  Widgets    -> WidgetLayouts        -> layout.toml

active KeyboardProfile + active MidiProfile
                    |
                    v
               BindResolver -> EChannel -> judgment
                                      |
active LaneProfile -------------------+-> display lane
```

### Crate ownership

`dtx-config` owns keyboard and MIDI profile schemas, built-in defaults, registry loading and saving, and migration from `bindings.toml`.

`dtx-layout` owns lane profile schemas, built-in lane arrangements, registry loading and saving, and migration from `layout.toml [lanes]`.

`gameplay-drums::bindings` composes active keyboard and MIDI profiles into the existing runtime resolver. Gameplay input and judgment continue to consume `EChannel` mappings.

`gameplay-drums::editor` owns draft resources, dirty state, Controls sub-tabs, shared profile controls, confirmation state, and live lane preview.

`game-shell::CustomizeTab::Bindings` becomes `CustomizeTab::Controls`. Controls remains one top-level tab, so the Customize bar keeps seven entries.

### Runtime types

Persistent profile data no longer uses mixed `BindSource` values.

```text
KeyboardProfile
  channel -> [KeyCode]

MidiProfile
  port filter
  velocity threshold
  channel -> [MIDI note]

LaneProfile
  lane order
  widths
  channel -> display-lane map
```

The runtime resolver may keep separate key and MIDI lookup maps. No profile or editor API binds directly to display-lane indices.

Keyboard keys may appear under multiple channels, preserving current shared-key behavior. A MIDI note may belong to one channel only.

## Storage

### Keyboard registry

```toml
version = 1
active = "DTXMania default"

[profiles."Desk"]
HH = ["KeyX", "KeyC"]
SD = ["KeyS", "KeyD"]
BD = ["Space"]
```

### MIDI registry

```toml
version = 1
active = "Roland TD-17"

[profiles."Roland TD-17"]
port = "TD-17"
velocity_threshold = 12

[profiles."Roland TD-17".map]
SD = [38]
HH = [42]
HHO = [46]
```

Each MIDI profile contains its preferred port filter, velocity threshold, and note map. The app does not auto-switch profiles when a device connects.

### Lane registry

```toml
version = 1
active = "Symmetric kit"

[profiles."Symmetric kit"]
order = ["LC", "HH", "SD", "LP", "BD", "HT", "LT", "FT", "CY", "RD"]

[profiles."Symmetric kit".widths]
BD = 69.0

[profiles."Symmetric kit".map]
HHO = "HH"
LBD = "BD"
```

### Built-ins

The app supplies these immutable profiles:

- Keyboard: `DTXMania default`
- MIDI: `General MIDI drums`
- Lanes: `Classic`, `NX Type-B`, `NX Type-D`

Registries store user profiles and the active profile name. Built-in profile names remain reserved and stable. Editing a built-in creates a dirty draft, but Save stays unavailable. Save As creates and activates a user profile.

### Name rules

Profile names:

- trim leading and trailing whitespace;
- must contain at least one non-whitespace character;
- must be unique within their registry under case-insensitive comparison;
- cannot equal a reserved built-in name;
- do not become filenames.

Save As preselects a suggested name. The suggestion removes a trailing space plus integer when present, then chooses the first unused integer starting at two:

```text
Studio kit       -> Studio kit 2
Studio kit 2     -> Studio kit 3
2 and 3 occupied -> Studio kit 4
```

The user can replace the selected suggestion before saving.

## Profile selection and editing

Each Keyboard, MIDI, and Lanes surface uses the same profile bar:

```text
[ Profile name v ] [ Save ] [ Save as... ] [ ... ]
```

The selector groups built-ins before user profiles and marks the current selection. Selecting a profile with a clean draft loads it and persists the active selection. If writing the active selection fails, the UI keeps the previous profile active and shows the error.

The overflow menu contains:

- Rename, Revert, and Delete for user profiles;
- Save As for built-ins.

Save As covers duplication, so the UI does not add a separate Duplicate action. Deleting the active user profile activates the profile type's built-in default after the registry write succeeds.

Changing top-level Customize tabs or switching Keyboard and MIDI keeps drafts in memory. These navigation actions do not prompt. The app can therefore hold dirty drafts for more than one profile type during a Customize session.

### Controls: Keyboard

The Keyboard segment shows only keys. Channel rows follow the active lane arrangement from left to right, with each channel appearing once. `Add key` arms keyboard-only capture for that channel.

Capture rejects Escape, Tab, function keys, and key presses with Ctrl, Alt, or Super held. Escape cancels capture. A captured key joins the channel's list without removing the same key from another channel.

### Controls: MIDI

The MIDI segment starts with port status, rescan, velocity threshold, and the live velocity meter. It then shows MIDI notes by channel in active lane order.

`Learn pad` arms MIDI-only capture for one channel. A note already owned by another channel opens the existing steal confirmation. No note moves silently.

A disconnected preferred port leaves the profile active and editable. The UI shows `Disconnected`; it does not select another port or profile. Physical MIDI hits provide meter and lane-test feedback and do not drive editor navigation.

### Lanes

The Lanes tab gains the shared profile bar above existing reorder, resize, split, and merge controls. Edits update a draft and live playfield preview. A user profile keeps its name while edited instead of becoming a generic `Custom` label.

Built-in selection continues to resolve the exact Classic, NX Type-B, or NX Type-D arrangement. Manual edits to a built-in require Save As.

## Unsaved-change protection

The app prompts only when an action would replace or destroy a dirty draft:

- selecting another profile of the same type;
- Revert;
- closing Customize;
- graceful app exit.

For one dirty user profile, the dialog uses this left-to-right order:

```text
Cancel | Discard changes | Save changes
```

Save changes is the primary action and receives default focus. Enter saves. Escape cancels. Discard uses destructive styling and never receives default focus.

For a dirty built-in draft, the primary action reads `Save as new profile`.

When several profile types are dirty at close or graceful exit, the dialog lists them and offers:

```text
Cancel | Discard all | Save all
```

Save All writes each dirty registry. Successfully written drafts become clean. Any failed draft remains dirty, the dialog stays open, and the UI lists the failed profile with its error. Discard All requires an explicit click or focus movement.

The app cannot prompt after a process crash or forced OS termination. Crash-recovery autosaves remain outside this version.

## Persistence safety

Registry writes follow this contract:

1. Validate the draft and profile name.
2. Serialize the complete registry in memory.
3. Write a temporary file in the target directory.
4. Flush it and replace the registry.
5. Mark the draft clean only after replacement succeeds.

A failed write leaves the prior registry intact. The UI keeps the draft and reports the path and error without exposing a false success state.

Missing registries load built-ins and create no error. A corrupt registry never falls back to defaults and then overwrites the corrupt file. The UI enters read-only built-in mode and offers `Back up and reset`. That confirmed action renames the corrupt file to a timestamped backup before creating a default registry.

## Migration

The app migrates each profile type only when its new registry does not exist.

```text
bindings.toml
  keyboard BindSource values -> keyboard registry
  MIDI BindSource values
  + port + threshold         -> MIDI registry

layout.toml [lanes]           -> lane registry
layout.toml [scene]           -> remains in layout.toml
```

For each type, migration compares old data with the built-in default:

- matching data activates the built-in;
- changed keyboard data becomes `Migrated keyboard`;
- changed MIDI data becomes `Migrated MIDI`;
- a custom lane arrangement becomes `Migrated lanes`;
- a named Classic, NX Type-B, or NX Type-D arrangement activates that built-in.

Each registry migrates independently through the safe-write contract. If one migration fails, the app keeps using its old source for that session and retries the missing registry next launch. Successful registry creation prevents repeated migration.

Migration leaves `bindings.toml` untouched. `layout.toml` moves to schema version 2 and writes widget scene data without an active lane section after lane migration. Its migration parser continues to accept version 1 files.

## Navigation and focus

Controls adds one focus level beneath the top-level tab:

```text
Controls tab
  Down / Enter -> Keyboard | MIDI segment
  Left / Right -> switch segment
  Down         -> profile and mapping rows
  Up           -> previous level
```

Keyboard capture consumes keys only while armed. MIDI capture consumes NoteOn messages only while armed. The footer always shows capture and cancellation controls. Existing Tab-to-peek behavior remains unchanged.

## Error handling

| Condition | Behavior |
|---|---|
| Duplicate, reserved, or blank name | Keep name field open and show inline error. |
| Registry save failure | Keep old file, active selection, and dirty draft. |
| Corrupt registry | Use built-ins read-only; offer confirmed backup and reset. |
| Missing MIDI device | Keep selected MIDI profile; show disconnected state. |
| MIDI note conflict | Ask to steal or cancel. |
| Delete active custom profile | Write fallback activation and deletion together. |
| Partial Save All failure | Clean successful drafts; retain failed drafts and dialog. |

## Verification

### Pure tests

- Keyboard, MIDI, and lane registry round trips.
- Missing and newer schema versions.
- Mixed v1 binding partition migration.
- Named and custom lane migration.
- Shared keyboard-key behavior.
- Exclusive MIDI-note behavior and conflict detection.
- Profile-name validation and case-insensitive uniqueness.
- Numeric Save As suggestions.
- Rename, revert, delete, and active-profile fallback.
- Dirty-decision transitions for Save, Save As, Discard, and Cancel.
- Corrupt registry cannot be overwritten without confirmed backup and reset.

### Headless integration

Run representative keyboard and MIDI events through the active profile composition:

```text
active keyboard + active MIDI -> BindResolver -> expected EChannel
active lane profile           -> expected display column
```

Repeat with Classic, NX Type-B, NX Type-D, and a custom lane profile. Changing lane profiles must not change the judged `EChannel`.

Exercise profile switching with clean and dirty drafts, multi-draft Save All, migration, failed writes, and active-selection rollback.

### Manual checks

- Keyboard and MIDI segment focus and pointer interaction.
- Save, Save As, rename, revert, and delete.
- Every dirty-exit confirmation path and button shortcut.
- Keyboard-only capture cannot consume MIDI.
- MIDI-only capture cannot consume keyboard input.
- Real MIDI disconnect, reconnect, rescan, velocity threshold, and note learning.
- Live lane preview and built-in Save As behavior.
- Profile lists and confirmation text remain readable at supported resolutions.

Final implementation gates:

```sh
cargo test -p dtx-config
cargo test -p dtx-layout
cargo test -p gameplay-drums
cargo check --workspace
```

## Out of scope

- Bundled setups spanning profile types.
- Automatic MIDI-profile switching by port name.
- Import, export, cloud sync, and profile-per-file storage.
- Dedicated profile manager, search, and sorting controls.
- Crash-recovery drafts.
- Per-song profiles.
- Guitar and bass profile UI; this version covers current drum channels.
