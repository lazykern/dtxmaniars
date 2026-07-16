# Unified semantic input, application-owned routing, and Settings program

Date: 2026-07-16
Status: Approved (brainstormed with user)
Scope: the remaining UI/input program after Home, Song Select, Song Ready, and
Practice Setup landed. Serialized multi-PR program (PR1–PR8); PR0 baseline is
recorded at `dd65e6c`.

## Goals

1. Keyboard and mouse are first-class: the whole application works with no
   MIDI device.
2. One canonical persisted non-lane action vocabulary: `dtx_input::SystemVerb`.
3. General UI context routing owned by `game-shell`, not `gameplay-drums`.
4. Dynamic prompts follow the last intentional input source and real bindings.
5. Dedicated `AppState::Settings` with YARG-style IA, draft Apply/Discard, and
   search; no chart or Performance dependency.
6. Throne navigation configurable via profiles; readiness reported as
   capabilities, never as a gate on Play.

Non-goals: redesigning Home/Song Ready/Practice UX, second lane renderer,
mechanics changes, requiring MIDI anywhere outside drum input.

## Decisions (user-approved)

- **D1 — Centralized router.** One game-shell router converts
  `SystemVerbHit` → `NavAction`. Per-screen keyboard emitters are deleted;
  keyboard menu input flows through profile bindings + resolver, so it is
  configurable and prompt-visible. (Alternative rejected: keep per-screen
  producers — fails configurability and prompt truthfulness.)
- **D2 — Readiness struct, not enum.** Independent capabilities:
  `keyboard_ready`, `midi_gameplay: MidiGameplay {Ready|NoProfileLanes|
  DeviceUnavailable|NoDevice}`, `throne: ThroneStatus {Ready|Incomplete{missing}|
  NoMidi}`. Pure `assess(keyboard_profile, midi_profile, connected)`.
- **D3 — SettingsDraft with per-domain snapshots.** `Domain<T> {entry, draft}`
  for `Config`, keyboard registry, MIDI registry, prompt preference. Dirty =
  `entry != draft`. Apply: validate all → ordered writes → per-domain failure
  keeps that domain dirty and reports the path; successes clear. Discard:
  draft ← entry. (Command-log rejected: YAGNI.)
- **D4 — New Game-layer crate `game-settings`** + Pure descriptor module
  `dtx-config::settings` shared by the old layout editor and new Settings.
  Input-profile rows stay in game-settings (dtx-input types must not enter a
  Pure crate).

## Architecture

### Input pipeline (end state)

```
dtx-input (Engine)                       game-shell (Game)                    screens
┌──────────────────────────┐   ┌────────────────────────────────┐   ┌─────────────────────┐
│ profiles (kb+MIDI)       │   │ navigation::context            │   │ publish NavContext  │
│  system: SystemVerb→src  │   │   NavContextStack (push/pop)   │◄──│ on enter/exit       │
│ resolver                 │   │ navigation::router             │   │                     │
│  key→verbs note→verbs    │   │   SystemVerbHit ─┬─ scope gate │   │ consume NavAction   │
│  menu verbs lane-share   │   │   debounce/grace ┤             │──►│ (verbs only,        │
│  live verbs lane-excl    │   │   Left/Right→Dec/Inc in Edit   │   │  no raw keys)       │
│ pump: SystemVerbHit ─────┼──►│ navigation::source             │   │                     │
│  (kb translator + MIDI)  │   │   LastIntentionalInputSource   │   │ mouse: report       │
│ MidiConnected            │   │   PromptSourcePreference       │   │ MouseIntent msg     │
└──────────────────────────┘   └────────────────────────────────┘   └─────────────────────┘
```

- `SystemVerb` (already landed): 16 variants; stable kebab-case keys with
  `pause`/`restart` frozen. `VerbScope::Menu` verbs may lane-share;
  `LiveSystem` (`OpenSystemMenu`, `Pause`, `Restart`) stay lane-exclusive
  (lane-wins skip in the resolver). Menu verbs left fully unbound fall back to
  built-in keyboard defaults in the resolver — navigation never bricks for
  migrated v1 profiles. Live verbs are never defaulted or injected.
- Built-in defaults: Arrows/Enter/Escape/Tab/PageUp/PageDown; MIDI drum
  convention HH↑ CY/RD↓ HT← LT→ BD-confirm SD-back FT-next-tab, expressed as
  profile bindings. No guessed `OpenSystemMenu` note.
- `keyboard_system_verbs` runs globally; `RawInputOwned` suppresses it during
  capture/text entry. Router drops menu-scope hits unless a UI context owns
  input; live-system hits always pass to their consumers.
- Router translates `NavigateLeft/Right` → `Decrease/Increase` while the top
  context is an Edit-type context. Shift stamps `coarse`; Shift+Tab →
  `PreviousTab`.
- 80 ms debounce + 500 ms entry grace live in the router, applied to
  MIDI-sourced actions only (as today). Context change resets grace
  (loading-cancel protection preserved).
- Deleted by PR8: `PadNavHit`, `verb_for_lane()`, per-screen keyboard
  emitters, `NavSource`. `gameplay-drums/menu_nav.rs` shrinks to a publisher
  of gameplay facts (paused/practice-setup/loading/editor) that the shell
  composes into the stack.

### Envelope

```rust
pub struct NavAction { pub verb: SystemVerb, pub source: InputSource,
                       pub coarse: bool, pub repeated: bool }
pub enum InputSource { Keyboard, Mouse, MidiKit, Gamepad }
```

Transitional (current PR1 state): `NavAction { verb: SystemVerb,
source: NavSource {Keyboard|Pad}, coarse }`; `InputSource`/`repeated` arrive
with the PR2 router. Practice is not a verb: Song Select's Shift+Enter rides
`Confirm` + `coarse=true` as a screen-local accelerator; Song Ready's internal
mode toggle and Results' verb row are the visible Practice choices.

### Contexts and source tracking

```rust
pub enum NavContext { Home, SongSelectSongs, SongSelectDifficulty,
    SongReadyBrowse, SongReadyEdit, SongLoading, PracticeSetupSettings,
    PracticeSetupPreview, PauseMenu, Results, SettingsTabs, SettingsRows,
    SettingsEdit, ModalDialog, BindingCapture, LayoutEditor, LiveGameplay }
pub struct NavContextStack(Vec<NavContext>); // top owns input
```

`ModalDialog`/`BindingCapture` swallow all menu routing (capture also sets
`RawInputOwned`). `LiveGameplay` on top = menu verbs dropped, lanes judge.
Screens push/pop in `OnEnter`/`OnExit` and overlay toggles; screens filter
`NavAction` on their own focus state (existing pattern), so exactly one
surface consumes each action.

```rust
pub struct LastIntentionalInputSource(pub InputSource); // default Keyboard, never StateScoped
pub struct MouseIntent; // message written by click/wheel/drag reporters
pub enum PromptSourcePreference { Automatic, Always(InputSource) } // persisted under accessibility
```

Accepted keyboard verb → Keyboard; post-gate MIDI verb → MidiKit;
`MouseIntent` → Mouse; pointer motion never writes. Hover may style, must not
steal keyboard/MIDI focus; click focuses+activates per the screen's model.

### Prompt bar (dtx-ui)

`nav_legend.rs` → reusable prompt bar. Screens declare
`PromptSpec { actions: Vec<PromptAction { verb, text, enabled }> }`; renderer
resolves display from effective source (preference else last-intentional) ×
resolver bindings × spec. MIDI labels: lane-shared note → lane short name;
spare note → compact note number or profile label; unbound → "Unbound" or
omitted with a Settings shortcut; never panic. Keyboard/mouse prompts never
hidden by MIDI absence.

### Settings

- `AppState::Settings`; Home → `TransitionRequest(Settings)`; Back → Home
  (300 ms OutQuint). Works with empty SongDb and no MIDI. Never touches
  `EditorSession`/autoplay/Performance.
- Crate layout:

```
crates/game-settings/src/{lib,state,draft,navigation,search,ui,rows}.rs
crates/game-settings/src/pages/{setup,controls,kit,audio,gameplay,visuals,library,accessibility}.rs
```

- IA: `SETUP | CONTROLS | KIT | AUDIO | GAMEPLAY | VISUALS | LIBRARY |
  ACCESSIBILITY` — one line at 1280×720 and 1920×1080, horizontal scroll
  below that. Focus model Tabs→Rows→Edit mirrors the shared browse/edit
  contract; focus never indicated by color alone.
- Rows: Toggle, Enum, Stepper, Slider, Action, Link, BindingCapture,
  ReadOnlyStatus — config-backed rows driven by `dtx_config::settings::
  SettingDescriptor` (id, label, description, keywords, kind, get/adjust/
  reset, clamps; Pure, Bevy-free). Old editor `settings_data.rs` consumes the
  same descriptors during migration — one source of clamps/defaults.
- Draft/apply/discard per D3; Back-with-dirty modal: Keep Editing / Discard
  Changes / Apply Changes. Search: typing opens search unless capture/text/
  modal owns input; results show category path + current value; selection
  deep-links to the real row.
- Setup page renders `InputReadiness` + latency/library status with actions
  (Test Controls, Complete Throne Controls, Calibrate Latency, Manage
  Library). Controls = keyboard/mouse-facing; Kit = MIDI-facing (device,
  profile, threshold, live note monitor via `LastMidiHit`, pad/zone map, menu
  binds, spare `OpenSystemMenu`, guided throne test, readiness). A struck
  note shows note/velocity/gameplay-owner/menu-owner.
- Visuals → "Edit Full Layout": adapter picks last-valid else first chart,
  sets existing editor session, routes through SongLoading/Performance,
  returns to Settings on exit; disabled with reason when no chart exists.
- Audio/Gameplay/Visuals/Library/Accessibility pages expose only currently
  persisted options; unavailable actions disabled with explicit reason; scan
  diagnostics move to Library. Accessibility updates live policy on preview;
  persistence still via Apply.

### Throne readiness (PR4)

Required throne actions: NavigateUp/Down/Left/Right, Confirm, Back,
OpenSystemMenu. `OpenSystemMenu` opens the normal/practice pause overlay
during live play; keyboard Escape always works regardless. Guided-test API
(UI-agnostic): list required actions, current sources, missing/conflicts,
test-hit→expected-action, clear/rebind, active device/profile.

### Home readiness panel (PR8)

Compact secondary panel under the three primary choices, rendered from
`InputReadiness` + latency/library status; rows may deep-link into Settings.
Retire `request_gameplay_settings` as the ordinary Settings route; adapter
survives only as the Full Layout Editor launcher.

## Error handling

- Missing MIDI: status only; keyboard/mouse routing unaffected by design
  (router never consults `MidiConnected`).
- Apply failure: failed domain stays dirty; dirty bar names domain + path.
- Registry load failure: existing `RegistryStartup` fallback, read-only
  banner on Controls/Kit.
- Binding conflict: live-system collision names the owning lane and is
  refused; menu lane-sharing allowed and labeled.
- Unbound advertised verb: prompt shows Unbound/omits + Settings shortcut.
- All transitions via `TransitionRequest`; feature crates never write
  `NextState<AppState>`.

## Testing strategy

TDD on pure units: router reducer, `NavGuard`, `assess()`, draft
apply/discard/partial-failure (temp dirs), descriptor clamps, search ranking,
verb key stability + v1 profile fixtures. Bevy App tests: stack push/pop on
transitions, confirm-cannot-cancel-loading, capture suppression, source
persistence across states, Settings reachable with empty SongDb +
`MidiConnected(false)`. Per-PR gates per AGENTS.md package-test policy; full
matrix before final handoff.

## PR sequence

1. **PR1** — canonical vocabulary (landed, `c171eb7`) + NavAction envelope
   sweep, Practice-verb removal (in flight).
2. **PR2** — context stack, centralized router, source tracking; screens
   consume `NavAction` only.
3. **PR3** — prompt bar + screen migration off hard-coded footers.
4. **PR4** — readiness assess + guided-test API + OpenSystemMenu live wiring.
5. **PR5** — `AppState::Settings`, game-settings shell, category bar,
   descriptor extraction.
6. **PR6** — draft/apply/discard, search, Setup/Controls/Kit.
7. **PR7** — Audio/Gameplay/Visuals/Library/Accessibility + layout-editor
   adapter.
8. **PR8** — Home readiness panel, retire old route, delete transitional
   nav types, docs + ADR for the unified vocabulary and Settings boundary.
