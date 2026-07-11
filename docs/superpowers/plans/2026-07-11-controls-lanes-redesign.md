# Controls & Lanes Tab Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Customize surface's Controls and Lanes tabs per `docs/superpowers/specs/2026-07-11-controls-lanes-redesign-design.md` — profile bar UI, Keyboard|MIDI segments, capture modal, slim lanes rows + detail card, preview manipulation, shared-source MIDI fan-out, quiet-instrument styling.

**Architecture:** The profile registries, profile-bar model (`profile_bar.rs`), dialogs model (`profile_dialog.rs`), dirty/close guard (`profile_state.rs`), and keyboard fan-out ALREADY EXIST and are tested — this plan renders and wires them. Net-new logic: MIDI note fan-out, capture-modal `Arrived` stage, lane hide/restore ops, preview drag ops. UI work follows the repo's split: pure reducers unit-tested, Bevy spawn/systems patterned on existing code, BRP smoke at the end.

**Tech Stack:** Rust, Bevy (UI nodes, no egui), workspace crates `gameplay-drums`, `dtx-input`, `dtx-layout`, `dtx-ui`.

**Key existing anchors (read these before each task):**

| Thing | Where |
|---|---|
| Controls tab render | `crates/gameplay-drums/src/editor/bindings_panel.rs:145` `spawn_bindings_block` |
| Lanes tab render | `crates/gameplay-drums/src/editor/panel.rs:535` `spawn_lane_block` |
| Tab dispatch | `crates/gameplay-drums/src/editor/panel.rs:274-297` |
| Profile bar model (unrendered) | `crates/gameplay-drums/src/editor/profile_bar.rs` |
| Profile dialog model (unrendered) | `crates/gameplay-drums/src/editor/profile_dialog.rs` |
| Drafts/session/dirty | `crates/gameplay-drums/src/editor/profile_state.rs` (`CustomizeSession:99`, `LaneProfileDraft:120`) |
| Capture machine | `crates/gameplay-drums/src/editor/bindings_capture.rs` (`CaptureState:34`) |
| Segment + focus reducers | `crates/gameplay-drums/src/editor/controls_panel.rs` (`ControlsSegment:78`, `reduce_controls_nav:115`) |
| Resolver (fan-out site) | `crates/gameplay-drums/src/bindings.rs:49` `BindResolver` |
| MIDI consumer | `crates/gameplay-drums/src/lib.rs:469` `consume_midi_events` |
| Lane edit ops | `crates/dtx-layout/src/lane_edit.rs` |
| Lane registry | `crates/dtx-layout/src/profiles.rs` |
| Kb/MIDI registries | `crates/dtx-input/src/profiles.rs` |
| Form widgets (slider/stepper/toggle) | `crates/dtx-ui/src/widget/controls.rs` |
| Chrome constants | `crates/gameplay-drums/src/editor/chrome.rs` |
| Close dialog render pattern (copy for modals) | `crates/gameplay-drums/src/editor/close_dialog.rs` |

Run tests with `cargo test -p <crate>`; full sweep `cargo test --workspace`.

---

### Task 1: MIDI note fan-out (one note → many lanes)

Keyboard already fans out (`key_to_lanes: HashMap<KeyCode, Vec<LaneId>>`). MIDI is single (`note_to_lane: HashMap<u8, LaneId>`). Make MIDI symmetric.

**Files:**
- Modify: `crates/gameplay-drums/src/bindings.rs:49-135` (BindResolver)
- Modify: `crates/gameplay-drums/src/lib.rs:496-506` (consume_midi_events)
- Tests: inline `#[cfg(test)]` in both files (existing modules)

- [ ] **Step 1: Find every `lane_for_note` / `note_to_lane` caller**

Run: `grep -rn 'lane_for_note\|note_to_lane' crates --include=*.rs`
Expected callers: `bindings.rs` itself, `lib.rs:496`. If more appear, list them and convert them in Step 3 the same way.

- [ ] **Step 2: Write the failing tests**

In `crates/gameplay-drums/src/bindings.rs` tests module:

```rust
#[test]
fn note_shared_by_two_channels_resolves_both_lanes() {
    use dtx_input::{BindSource, InputBindings};
    let mut b = InputBindings::default();
    // Share note 36 between BD and LBD.
    b.bind_shared(dtx_core::EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
    let r = BindResolver::from_bindings(&b);
    let lanes: Vec<_> = r.lanes_for_note(36).collect();
    assert_eq!(lanes.len(), 2, "36 owned by BD and LBD: {lanes:?}");
    // Single-owner note unaffected.
    assert_eq!(r.lanes_for_note(42).count(), 1);
    // Unbound note yields nothing.
    assert_eq!(r.lanes_for_note(99).count(), 0);
}
```

In `crates/gameplay-drums/src/lib.rs` midi_consumer tests module (mirror the existing consume test setup there):

```rust
#[test]
fn shared_note_emits_one_hit_per_owning_lane() {
    use dtx_input::{BindSource, InputBindings};
    let mut b = InputBindings::default();
    b.bind_shared(dtx_core::EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
    let resolver = crate::bindings::BindResolver::from_bindings(&b);
    let mut last = LastMidiHit::default();
    let out = consume_midi_events(
        [dtx_input::midi::MidiEvent::NoteOn { note: 36, velocity: 100, audio_ms: Some(10) }],
        &resolver,
        true,
        0,
        &mut last,
    );
    assert_eq!(out.hits.len(), 2, "BD and LBD both hit");
    assert_eq!(out.nav_lanes.len(), 2);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums note_shared -- --nocapture` and `cargo test -p gameplay-drums shared_note_emits`
Expected: FAIL (`lanes_for_note` not found).

- [ ] **Step 4: Implement**

In `crates/gameplay-drums/src/bindings.rs`:

```rust
// Field rename + type change (both builders):
note_to_lanes: HashMap<u8, Vec<LaneId>>,

// In from_profiles and from_bindings, replace
//   note_to_lane.insert(*note, lane);
// with:
note_to_lanes.entry(*note).or_insert_with(Vec::new).push(lane);

/// Lanes for a MIDI note (a note may be shared by several channels).
pub fn lanes_for_note(&self, note: u8) -> impl Iterator<Item = LaneId> + '_ {
    self.note_to_lanes
        .get(&note)
        .into_iter()
        .flat_map(|lanes| lanes.iter().copied())
}

/// First lane for a MIDI note, if bound (nav/back-compat).
pub fn lane_for_note(&self, note: u8) -> Option<LaneId> {
    self.lanes_for_note(note).next()
}
```

In `crates/gameplay-drums/src/lib.rs` `consume_midi_events`, replace the single-lane block (`:496-506`) with a loop matching the keyboard path:

```rust
let mut any = false;
for lane in resolver.lanes_for_note(note) {
    any = true;
    nav_lanes.push(lane);
    if gameplay_ready {
        hits.push(LaneHit {
            lane,
            audio_ms: stamp_audio_ms(Some(clock_ms), audio_ms),
        });
    }
}
if !any {
    continue;
}
```

Also update the doc comment on `InputBindings.map` in `crates/dtx-input/src/bindings.rs` ("MIDI sources appear under at most one channel" → "keyboard and MIDI sources may appear under multiple channels").

- [ ] **Step 5: Run tests, then the crate suite**

Run: `cargo test -p gameplay-drums && cargo test -p dtx-input`
Expected: PASS (fix any other `lane_for_note` callers found in Step 1 — nav paths keep `lane_for_note`, judgment paths use the loop).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/bindings.rs crates/gameplay-drums/src/lib.rs crates/dtx-input/src/bindings.rs
git commit -m "feat(input): fan out one MIDI note to every owning lane"
```

---

### Task 1.5: Let MIDI notes persist under multiple channels (dtx-input)

Discovered during Task 1: shared MIDI (spec decision "B") cannot survive today. Four collapse points in `dtx-input`, all must lift for a shared note to round-trip: `split_bindings` mirrors via steal-semantics `bind_note`; `MidiProfile::bind_note` retain-removes the note from other channels; `MidiProfile::deserialize` hard-errors on a duplicate note; legacy `BindingsFile::resolve` dedups. Keyboard already supports sharing end-to-end (`KeyboardProfile.add_key` appends, no uniqueness check) — mirror that.

**Files:**
- Modify: `crates/dtx-input/src/profiles.rs` (`MidiProfile`, `split_bindings`, deserialize)
- Modify: `crates/dtx-input/src/bindings.rs` (`BindingsFile::resolve` dedup)

- [ ] **Step 1: Failing tests**

In `profiles.rs` tests:

```rust
#[test]
fn midi_profile_allows_shared_note_and_round_trips() {
    let mut p = MidiProfile::default();
    p.bind_note_shared(EChannel::BassDrum, 36);
    p.bind_note_shared(EChannel::LeftBassDrum, 36);
    assert!(p.map[&EChannel::BassDrum].contains(&36));
    assert!(p.map[&EChannel::LeftBassDrum].contains(&36));
    let toml = toml::to_string(&p).unwrap();
    let back: MidiProfile = toml::from_str(&toml).unwrap();
    assert_eq!(back.map[&EChannel::BassDrum], p.map[&EChannel::BassDrum]);
    assert_eq!(back.map[&EChannel::LeftBassDrum], p.map[&EChannel::LeftBassDrum]);
}

#[test]
fn bind_note_still_steals_for_move_semantics() {
    let mut p = MidiProfile::default();
    p.bind_note_shared(EChannel::BassDrum, 36);
    p.bind_note(EChannel::LeftBassDrum, 36); // steal
    assert!(!p.map[&EChannel::BassDrum].contains(&36));
    assert!(p.map[&EChannel::LeftBassDrum].contains(&36));
}

#[test]
fn split_bindings_preserves_shared_midi_note() {
    use crate::{BindSource, InputBindings};
    let mut b = InputBindings::default();
    b.bind_shared(EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
    let (_kb, midi) = split_bindings(&b);
    assert!(midi.map[&EChannel::BassDrum].contains(&36));
    assert!(midi.map[&EChannel::LeftBassDrum].contains(&36));
}
```

Run: `cargo test -p dtx-input shared_note midi_profile_allows split_bindings_preserves` → FAIL.

- [ ] **Step 2: Implement**

```rust
// MidiProfile impl: keep bind_note (steal) as-is; add the shared variant.
/// Append `note` to `channel` without removing it from other channels.
pub fn bind_note_shared(&mut self, channel: EChannel, note: u8) {
    let notes = self.map.entry(channel).or_default();
    if !notes.contains(&note) {
        notes.push(note);
    }
}
```

- In `split_bindings`, change the MIDI arm from `midi.bind_note(*channel, *note)` to `midi.bind_note_shared(*channel, *note)` (InputBindings is the source of truth for sharing; mirroring must not steal).
- In `MidiProfile::deserialize`, DELETE the duplicate-owner rejection block (the `owners.insert(*note, name)` loop that returns `serde::de::Error::custom("MIDI note ... bound to both ...")`). The runtime resolver now fans out, so shared notes are valid on disk. Keep the `EChannel::from_short_name` filtering.
- In `crates/dtx-input/src/bindings.rs`, `BindingsFile::resolve`: the MIDI dedup ("duplicate MIDI source; kept first") drops shared notes on legacy load — change it to keep every (channel, note) pair (append instead of skip). Preserve the resolve function's other behavior; only stop discarding duplicate MIDI sources.

- [ ] **Step 3: Run full dtx-input suite**

Run: `cargo test -p dtx-input`
Expected: PASS. Some existing tests may assert the old rejection/dedup — update them to the new shared-allowed contract (a test named around "duplicate note rejected" becomes "duplicate note allowed"). If a test encodes a real invariant unrelated to sharing, keep it.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-input/src/profiles.rs crates/dtx-input/src/bindings.rs
git commit -m "feat(input): allow a MIDI note to bind multiple channels"
```

---

### Task 2: Capture machine — `Arrived` stage with shared/move choice

Extend the pure state machine so BOTH sources stop at an "arrived" preview before commit, and a conflicting source offers Add-shared / Move-here. Replaces `ConfirmMidiSteal`.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_capture.rs`

- [ ] **Step 1: Write the failing reducer tests**

Add to the tests module in `bindings_capture.rs`:

```rust
#[test]
fn keyboard_capture_stops_at_arrived_with_owners() {
    // Reducer surfaces the key + its other owners; nothing binds yet.
    let step = keyboard_capture_step(false, false, Some(KeyCode::KeyX));
    assert_eq!(step, KeyboardCaptureStep::Bind(KeyCode::KeyX)); // unchanged reducer
    // New: arrived-stage decision reducer.
    let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Shared, true);
    assert_eq!(d, ArrivedStep::CommitShared);
    let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Move, true);
    assert_eq!(d, ArrivedStep::CommitMove);
    // No conflict: choice irrelevant, plain commit.
    let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Shared, false);
    assert_eq!(d, ArrivedStep::CommitShared);
    let d = arrived_step(ArrivedInput::Cancel, ArrivedChoice::Shared, true);
    assert_eq!(d, ArrivedStep::Cancelled);
    let d = arrived_step(ArrivedInput::Toggle, ArrivedChoice::Shared, true);
    assert_eq!(d, ArrivedStep::Choice(ArrivedChoice::Move));
    // Toggle with no conflict is inert.
    let d = arrived_step(ArrivedInput::Toggle, ArrivedChoice::Shared, false);
    assert_eq!(d, ArrivedStep::Choice(ArrivedChoice::Shared));
}

#[test]
fn same_note_again_confirms_midi_arrived() {
    assert_eq!(rearm_confirms(38, Some(38)), true);
    assert_eq!(rearm_confirms(38, Some(40)), false); // different note re-arms instead
    assert_eq!(rearm_confirms(38, None), false);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums arrived -- --nocapture`
Expected: FAIL (types not defined).

- [ ] **Step 3: Implement states + reducers**

In `bindings_capture.rs`, replace `ConfirmMidiSteal` in `CaptureState` (`:34`) with:

```rust
/// A captured key awaits confirm; `owners` = other channels holding it.
KeyArrived { channel: dtx_core::EChannel, key: KeyCode, owners: Vec<dtx_core::EChannel>, choice: ArrivedChoice },
/// A learned note awaits confirm; velocity shown in the modal.
MidiArrived { channel: dtx_core::EChannel, note: u8, velocity: u8, owners: Vec<dtx_core::EChannel>, choice: ArrivedChoice },
```

```rust
/// What the user picks when the source already has other owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrivedChoice {
    /// Keep every owner (chip marked shared).
    #[default]
    Shared,
    /// Remove the source from other owners first.
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivedInput { Confirm, Cancel, Toggle, None }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivedStep {
    Pending,
    Cancelled,
    CommitShared,
    CommitMove,
    Choice(ArrivedChoice),
}

/// Pure arrived-stage decision. Toggle flips the shared/move choice only
/// while a conflict exists; Confirm commits the current choice (plain add
/// when there is no other owner is CommitShared — bind_shared is a no-op
/// steal-wise with zero other owners).
pub fn arrived_step(input: ArrivedInput, choice: ArrivedChoice, has_conflict: bool) -> ArrivedStep {
    match input {
        ArrivedInput::Cancel => ArrivedStep::Cancelled,
        ArrivedInput::Confirm => match choice {
            ArrivedChoice::Move if has_conflict => ArrivedStep::CommitMove,
            _ => ArrivedStep::CommitShared,
        },
        ArrivedInput::Toggle if has_conflict => ArrivedStep::Choice(match choice {
            ArrivedChoice::Shared => ArrivedChoice::Move,
            ArrivedChoice::Move => ArrivedChoice::Shared,
        }),
        _ => ArrivedStep::Pending,
    }
}

/// Hitting the SAME note again while MidiArrived confirms it.
pub fn rearm_confirms(arrived_note: u8, new_note: Option<u8>) -> bool {
    new_note == Some(arrived_note)
}
```

- [ ] **Step 4: Rewire the driver `capture_binding` (`:193`)**

- `Keyboard(ch)` + `KeyboardCaptureStep::Bind(key)` → transition to `KeyArrived` (owners = `live.0.channels_for_key(key)` minus `ch`), do NOT bind.
- `Midi(ch)` + `MidiCaptureStep::Bind(note)` or `ConfirmSteal{..}` → transition to `MidiArrived` (owners from `live.0` note lookup minus `ch`; velocity from `LastMidiHit`). Delete `MidiCaptureStep::ConfirmSteal` handling downstream (the variant may stay in the reducer or collapse to `Bind` — collapse it and delete the variant + its tests).
- `KeyArrived`/`MidiArrived`: map inputs — Esc→`Cancel`, Enter→`Confirm`, Left/Right→`Toggle`, same-note NoteOn (`rearm_confirms`)→`Confirm`. On `CommitShared` call `live.0.bind_shared(ch, src)`; on `CommitMove` call `live.0.bind(ch, src)` (the existing steal-on-bind path). Bump `BindingsRev` as today.
- A NoteOn with a DIFFERENT note while `MidiArrived` re-arms: replace the arrived note/velocity/owners in place (fast retry).
- Below-threshold hits never reach `MidiArrived` (existing `strictly_new_note` + threshold gate in the driver stays; the modal UI in Task 6 shows them live from `LastMidiHit`).

Update `capture_footer_text` in `footer.rs` for the new states (arrived: "Enter confirm · ←→ shared/move · Esc cancel"). Fix the `footer_describes_keyboard_capture` test in `controls_panel.rs:228`.

- [ ] **Step 5: Run suite**

Run: `cargo test -p gameplay-drums`
Expected: PASS (update any test constructing `ConfirmMidiSteal`).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/footer.rs crates/gameplay-drums/src/editor/controls_panel.rs
git commit -m "feat(customize): capture arrives before commit with shared/move choice"
```

---

### Task 3: Quiet-instrument tokens + panel kit

One token set + shared spawn helpers so Tasks 4-8 stop inlining `Color::srgb` literals.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/chrome.rs` (stays the layout-constants home)
- Create: `crates/gameplay-drums/src/editor/panel_kit.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (declare module)

- [ ] **Step 1: Add tokens to `chrome.rs`**

```rust
// Quiet-instrument palette (spec 2026-07-11-controls-lanes-redesign).
pub const PANEL_BG: Color = Color::srgb(0.070, 0.078, 0.102);      // #12141a
pub const CARD_BG: Color = Color::srgb(0.090, 0.102, 0.133);       // #171a22
pub const CARD_BORDER: Color = Color::srgb(0.149, 0.165, 0.208);   // #262a35
pub const ACCENT: Color = Color::srgb(0.357, 0.549, 1.0);          // #5b8cff
pub const ROW_SELECTED_BG: Color = Color::srgb(0.114, 0.149, 0.208);
pub const CHIP_BG: Color = Color::srgb(0.122, 0.141, 0.188);
pub const CHIP_BORDER: Color = Color::srgb(0.20, 0.227, 0.29);
pub const TEXT_MUTED: Color = Color::srgb(0.365, 0.396, 0.47);
pub const DIRTY: Color = Color::srgb(0.847, 0.627, 0.184);         // amber
pub const OK: Color = Color::srgb(0.306, 0.788, 0.541);            // green
pub const ERR: Color = Color::srgb(0.86, 0.34, 0.34);              // red
pub const WARN_TINT: Color = Color::srgb(0.19, 0.12, 0.10);        // unbound row bg
```

(`use bevy::prelude::Color;` at top.)

- [ ] **Step 2: Create `panel_kit.rs` with card/chip/dot helpers**

Mirror the spawn style of `bindings_panel.rs:238` (device section) — plain functions taking `&mut ChildSpawnerCommands`:

```rust
//! Shared spawn helpers for Customize panel content: cards with uppercase
//! micro-labels, source chips, channel color dots. Visual tokens live in
//! `chrome.rs`; this module owns only structure.

use bevy::prelude::*;
use dtx_ui::theme::Theme;

use super::chrome;

/// Card container with an uppercase micro-label title. Returns the body
/// entity; callers spawn rows into it.
pub fn spawn_card(parent: &mut ChildSpawnerCommands, title: &str) -> Entity {
    let mut body = Entity::PLACEHOLDER;
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(chrome::CARD_BG),
            BorderColor::all(chrome::CARD_BORDER),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(title.to_uppercase()),
                Theme::font(9.0),
                TextColor(chrome::TEXT_MUTED),
                Node { margin: UiRect::bottom(Val::Px(6.0)), ..default() },
            ));
            body = card
                .spawn(Node { flex_direction: FlexDirection::Column, ..default() })
                .id();
        });
    body
}

/// Source chip: dark fill, hairline border; `shared` appends the ⧉ marker.
/// Extra components (e.g. remove-button marker) attach via `bundle`.
pub fn spawn_chip(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    shared: bool,
    bundle: impl Bundle,
) -> Entity {
    let text = if shared { format!("{label} ⧉") } else { label.to_owned() };
    parent
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                margin: UiRect::right(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(chrome::CHIP_BG),
            BorderColor::all(chrome::CHIP_BORDER),
            BorderRadius::all(Val::Px(4.0)),
            bundle,
        ))
        .with_children(|chip| {
            chip.spawn((Text::new(text), Theme::font(11.0)));
        })
        .id()
}

/// 9px rounded channel color dot.
pub fn spawn_channel_dot(parent: &mut ChildSpawnerCommands, color: Color) {
    parent.spawn((
        Node {
            width: Val::Px(9.0),
            height: Val::Px(9.0),
            margin: UiRect::right(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(color),
        BorderRadius::all(Val::Px(3.0)),
    ));
}
```

Adjust `Theme::font` / `BorderColor` calls to match the exact signatures used in `bindings_panel.rs` (copy a compiling call site — the repo's Bevy version is authoritative, not this plan).

- [ ] **Step 3: Declare + compile**

Add `mod panel_kit;` in `editor/mod.rs`. Run: `cargo check -p gameplay-drums`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/editor/chrome.rs crates/gameplay-drums/src/editor/panel_kit.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(customize): quiet-instrument tokens and shared panel kit"
```

---

### Task 4: Render the profile bar + dialogs

The models exist (`profile_bar.rs`, `profile_dialog.rs`) with zero rendering. Render one reusable bar pinned at the top of the panel for Controls (per active segment kind) and Lanes; render the Save-As/Rename name dialog, Delete confirm, and error surface.

**Files:**
- Create: `crates/gameplay-drums/src/editor/profile_bar_ui.rs`
- Create: `crates/gameplay-drums/src/editor/profile_dialog_ui.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs:204` (`rebuild_left_content` spawns the bar before tab content)
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (modules + systems)

- [ ] **Step 1: Write failing tests for the glue logic**

The bar itself is model-driven; the new pure glue is "which ProfileKind does the current tab+segment show" and "what does one bar row contain". In `profile_bar_ui.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use game_shell::CustomizeTab;
    use crate::editor::controls_panel::ControlsSegment;
    use crate::editor::profile_state::ProfileKind;

    #[test]
    fn bar_kind_follows_tab_and_segment() {
        assert_eq!(bar_kind(CustomizeTab::Controls, ControlsSegment::Keyboard), Some(ProfileKind::Keyboard));
        assert_eq!(bar_kind(CustomizeTab::Controls, ControlsSegment::Midi), Some(ProfileKind::Midi));
        assert_eq!(bar_kind(CustomizeTab::Lanes, ControlsSegment::Keyboard), Some(ProfileKind::Lanes));
        assert_eq!(bar_kind(CustomizeTab::Widgets, ControlsSegment::Keyboard), None);
    }
}
```

Run: `cargo test -p gameplay-drums bar_kind` → FAIL.

- [ ] **Step 2: Implement `profile_bar_ui.rs`**

```rust
/// Which profile registry the bar edits for the current tab (+segment).
pub fn bar_kind(tab: CustomizeTab, segment: ControlsSegment) -> Option<ProfileKind> {
    match tab {
        CustomizeTab::Controls => Some(match segment {
            ControlsSegment::Keyboard => ProfileKind::Keyboard,
            ControlsSegment::Midi => ProfileKind::Midi,
        }),
        CustomizeTab::Lanes => Some(ProfileKind::Lanes),
        _ => None,
    }
}
```

Spawn (called from `rebuild_left_content` before the tab content when `bar_kind` is Some): one row —

- profile name button + `▾` (marker `ProfileSelectorBtn`), amber dirty dot (`Visibility` driven by the kind's `ProfileDraft::is_dirty` via `CustomizeSession`),
- `Save` button (`ProfileBarBtn(ProfileBarAction::Save)`), disabled styling from `save_enabled(builtin_selected, dirty)` (`profile_bar.rs:91`),
- `Save As` button (`ProfileBarBtn(ProfileBarAction::SaveAs)`),
- `…` overflow button → popup listing `overflow_actions(builtin_selected)` (`profile_bar.rs:78`).

Selector click toggles a dropdown popup: absolute-positioned card under the bar listing `profile_bar_items(builtins, users, selected)` (`profile_bar.rs:55`); built-ins get a muted "built-in" suffix. Item click → `ProfileBarAction::Select(name)`.

Interaction systems (pattern: `handle_lane_buttons` `panel.rs:1238` — `Query<(&Interaction, &ProfileBarBtn), Changed<Interaction>>`):

- `Select(name)`: if current draft dirty → open `ProfileDialogState::Dirty` (routes through existing close-guard reducers in `profile_state.rs:309`); else run the select transaction.
- `Save`: `run_transaction` (`profile_bar.rs:108`) with `build_next` = save current draft under its name; write via the kind's registry save fn (`save_keyboard_registry` / `save_midi_registry` `dtx-input/profiles.rs:637,644`; `save_lane_registry` `dtx-layout/profiles.rs:280`).
- `SaveAs` / `Rename`: open `ProfileDialogState::Name{..}` with `suggest_copy_name` (`dtx-persistence`) as the seed.
- `Delete`: open `ProfileDialogState::ConfirmDelete`.
- On `TransactionResult::Failed`, store `ProfileUiError` in a `Resource` and render its message in the bar (small `ERR`-colored text, path + cause).

- [ ] **Step 3: Implement `profile_dialog_ui.rs`**

Copy the modal skeleton from `close_dialog.rs:119` (centered card over scrim). Render per `ProfileDialogState` (`profile_dialog.rs:22`):

- `Name{..}`: title ("Save as…" / "Rename"), editable name line, OK/Cancel. Text entry: a system reading `EventReader<KeyboardInput>` for pressed keys with `Key::Character(c)` → append, `Backspace` → pop, `Enter` → `submit_name` (`profile_dialog.rs` reducer — it validates via `validate_profile_name` and reserved built-in names), `Escape` → close. While the dialog is open, capture and panel hotkeys are suppressed (same gate the close dialog uses in `ui.rs:267`).
- `ConfirmDelete`: name + Delete/Cancel buttons.
- `Dirty`: reuse the existing close-dialog three-way layout (`dirty_dialog_layout` `profile_state.rs:376`) with the pending action run after save/discard.
- `CorruptReset`: message + "Backup & reset" button → `backup_and_reset_*` fns.

- [ ] **Step 4: Wire in `mod.rs` and `panel.rs`**

- `mod profile_bar_ui; mod profile_dialog_ui;` + their systems in the editor plugin's Update set (run_if `editor_open`).
- In `rebuild_left_content` (`panel.rs:204`): spawn bar above tab content for Controls/Lanes.
- Rebuild triggers: bar respawns when `ActiveTab`, `ControlsSegment`, or the kind's draft/session changes (same change-detection pattern the panel already uses).

- [ ] **Step 5: Test + run**

Run: `cargo test -p gameplay-drums && cargo check --workspace`
Manual BRP smoke (see Task 10 checklist) can wait; compile + unit green here.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/profile_bar_ui.rs crates/gameplay-drums/src/editor/profile_dialog_ui.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(customize): render profile bar and profile dialogs"
```

---

### Task 5: Controls tab rebuild — visible segments, per-segment content

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs` (spawn_bindings_block + row spawn)
- Modify: `crates/gameplay-drums/src/editor/controls_panel.rs` (no reducer change expected; verify)

- [ ] **Step 1: Failing test for per-segment chip filtering + shared detection**

In `bindings_panel.rs` tests:

```rust
#[test]
fn segment_filters_sources_and_flags_shared() {
    use dtx_input::{BindSource, InputBindings};
    let mut b = InputBindings::default();
    b.bind_shared(dtx_core::EChannel::LeftBassDrum, BindSource::Key(KeyCode::Space));
    let rows = segment_rows(&b, ControlsSegment::Keyboard, &dtx_layout::classic());
    let bd = rows.iter().find(|r| r.channel == dtx_core::EChannel::BassDrum).unwrap();
    assert!(bd.chips.iter().all(|c| matches!(c.source, BindSource::Key(_))), "keyboard segment shows keys only");
    assert!(bd.chips.iter().any(|c| c.shared), "Space owned by BD+LBD is shared");
    let hh = rows.iter().find(|r| r.channel == dtx_core::EChannel::HiHatClose).unwrap();
    assert!(hh.chips.iter().all(|c| !c.shared));
    // A channel with zero sources in this segment reports unbound.
    let mut empty = b.clone();
    empty.map.get_mut(&dtx_core::EChannel::Cymbal).unwrap().retain(|s| !matches!(s, BindSource::Key(_)));
    let rows = segment_rows(&empty, ControlsSegment::Keyboard, &dtx_layout::classic());
    assert!(rows.iter().find(|r| r.channel == dtx_core::EChannel::Cymbal).unwrap().unbound);
}
```

Run: `cargo test -p gameplay-drums segment_filters` → FAIL.

- [ ] **Step 2: Implement the pure row model**

```rust
pub struct SegmentChip { pub source: BindSource, pub label: String, pub shared: bool }
pub struct SegmentRow { pub channel: EChannel, pub chips: Vec<SegmentChip>, pub unbound: bool }

/// Rows for the active segment: only that segment's sources, shared flag when
/// another channel holds the same source, unbound when the channel has no
/// source in this segment.
pub fn segment_rows(b: &InputBindings, segment: ControlsSegment, lanes: &LaneArrangement) -> Vec<SegmentRow> {
    channels_in_display_order(lanes)
        .into_iter()
        .map(|channel| {
            let chips: Vec<SegmentChip> = b
                .map
                .get(&channel)
                .into_iter()
                .flatten()
                .filter(|src| match segment {
                    ControlsSegment::Keyboard => matches!(src, BindSource::Key(_)),
                    ControlsSegment::Midi => matches!(src, BindSource::Midi { .. }),
                })
                .map(|src| SegmentChip {
                    source: *src,
                    label: source_label(src), // existing chip-label helper in this file
                    shared: b.map.iter().any(|(ch, srcs)| *ch != channel && srcs.contains(src)),
                })
                .collect();
            SegmentRow { unbound: chips.is_empty(), channel, chips }
        })
        .collect()
}
```

- [ ] **Step 3: Rebuild `spawn_bindings_block` rendering**

- Delete the `"Controls"` header + `MODIFIED` badge (`:167-180`) — the Task 4 profile bar supersedes both. Keep `RESET TAB` inside the card header row.
- Spawn a **segment selector** under the profile bar: two joined buttons `Keyboard | MIDI` (marker `SegmentBtn(ControlsSegment)`), active one `ACCENT` background; click sets `ControlsSegment` (which already re-triggers rebuild + capture kind). Focus ring when `ControlsFocus::SegmentSelector`.
- MIDI segment only: `panel_kit::spawn_card("Device")` containing the existing port cycler → replace with a dropdown (reuse the Task 4 dropdown popup pattern) listing enumerated ports + "(first available)", status dot `OK`/`ERR` from the existing `PortMatch` state, velocity threshold as `controls::spawn_slider` with numeric value, Rescan button. The existing handlers (`:249-425`) keep their markers — only the shells restyle.
- `panel_kit::spawn_card("Pads")`: rows from `segment_rows` — `spawn_channel_dot`, name, chips via `spawn_chip(label, shared, (BindChipRemove(..),))` with the existing `×` remove behavior, `+` capture chip (existing `BindCaptureStart`). Unbound row: `WARN_TINT` background + muted "no binding" text. Selected row: `ROW_SELECTED_BG` + 2px `ACCENT` left border (replaces `highlight_selected_row` styling).
- Hover/focus on a shared chip lights every owning lane in the preview: extend the existing selection→`bindings_spatial` path to accept a set of channels (currently one).

- [ ] **Step 4: Run + fix**

Run: `cargo test -p gameplay-drums`
Expected: PASS (update tests referencing the deleted header/badge).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/bindings_panel.rs crates/gameplay-drums/src/editor/controls_panel.rs crates/gameplay-drums/src/editor/bindings_spatial.rs
git commit -m "feat(customize): segmented controls tab with per-segment cards"
```

---

### Task 6: Capture modal UI

**Files:**
- Create: `crates/gameplay-drums/src/editor/capture_modal.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

- [ ] **Step 1: Implement the modal render**

Copy the `close_dialog.rs` modal skeleton. Spawn/despawn driven by `CaptureState` changes (system `sync_capture_modal`):

- `Keyboard(ch)` / `Midi(ch)`: title "Press a key for {ch}" / "Hit a pad for {ch}", subtitle "Esc cancel". MIDI: live line from `LastMidiHit` — below-threshold hits render `TEXT_MUTED` "note 38 · velocity 4 — below threshold".
- `KeyArrived`/`MidiArrived`: arrived line ("KeyX" / "note 38 · velocity 92"), then if `owners` non-empty two choice buttons **Add shared** / **Move here** (active = `choice`, `ACCENT` border; Left/Right toggles via `arrived_step`) plus "also bound to LC" caption; if empty a single **Confirm** hint. Footer verbs already updated in Task 2.
- Target lane stays lit in the preview: reuse the selection lighting with `SelectedChannel = ch` while capturing (already happens for MIDI autoselect; set it on capture start).

The panel behind dims: one scrim node inside the left panel (the playfield preview must stay visible — scrim covers the panel only, NOT the stage).

- [ ] **Step 2: Mouse path**

Choice buttons clickable (`Interaction` → feed `ArrivedInput::Toggle`/`Confirm` into the same driver-state transition — route through a small event or shared fn so keyboard and mouse hit one code path).

- [ ] **Step 3: Verify pad exclusion still holds**

`pad_excluded(CustomizeTab::Controls)` (`keyboard_nav.rs`) must stay true so pads can't navigate while a capture is armed; the capture driver consumes NoteOns. Run: `cargo test -p gameplay-drums pad_exclusion`.

- [ ] **Step 4: Run + commit**

Run: `cargo test -p gameplay-drums && cargo check --workspace`

```bash
git add crates/gameplay-drums/src/editor/capture_modal.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(customize): capture modal with arrived preview and shared/move"
```

---

### Task 7: Lane hide/restore ops (pure, dtx-layout)

Spec: hide a lane → its channels collect in a "Hidden" strip, restorable. Model as remove-lane/add-lane in `lane_edit.rs` — hidden channels are exactly the channels present in no lane.

**Files:**
- Modify: `crates/dtx-layout/src/lane_edit.rs`

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn hide_lane_unassigns_its_channels_and_restore_reinserts() {
    let mut a = crate::classic();
    let n = a.lanes.len();
    let hh_index = a.lanes.iter().position(|l| l.primary == EChannel::HiHatClose).unwrap();
    let hidden = hide_lane(&mut a, hh_index);
    assert_eq!(a.lanes.len(), n - 1);
    assert!(hidden.contains(&EChannel::HiHatClose));
    assert!(unassigned_channels(&a).contains(&EChannel::HiHatClose));
    // Restore as a new lane at the end, default width.
    restore_lane(&mut a, EChannel::HiHatClose);
    assert_eq!(a.lanes.len(), n);
    assert!(unassigned_channels(&a).is_empty() || !unassigned_channels(&a).contains(&EChannel::HiHatClose));
}

#[test]
fn hide_last_lane_is_refused() {
    let mut a = crate::classic();
    while a.lanes.len() > 1 { hide_lane(&mut a, 0); }
    assert!(hide_lane(&mut a, 0).is_empty(), "cannot hide the only lane");
    assert_eq!(a.lanes.len(), 1);
}

#[test]
fn width_clamps_to_minimum() {
    let mut a = crate::classic();
    set_lane_width(&mut a, 0, 0.0);
    assert!(a.lanes[0].width >= MIN_LANE_WIDTH);
}
```

Run: `cargo test -p dtx-layout hide_lane` → FAIL.

- [ ] **Step 2: Implement**

```rust
/// Smallest permitted lane width multiplier (spec: zero-width impossible).
pub const MIN_LANE_WIDTH: f32 = 0.25;

/// Remove lane `index`, returning its channels (now unassigned). Refuses to
/// remove the last lane (returns empty, arrangement untouched).
pub fn hide_lane(a: &mut LaneArrangement, index: usize) -> Vec<EChannel> { ... }

/// Channels of DRUM_CHANNELS present in no lane, canonical order.
pub fn unassigned_channels(a: &LaneArrangement) -> Vec<EChannel> { ... }

/// Append a new default-width lane with `primary`; no-op if already assigned.
pub fn restore_lane(a: &mut LaneArrangement, primary: EChannel) { ... }
```

Clamp inside the existing `set_lane_width` (`lane_edit.rs:28`) with `MIN_LANE_WIDTH`. Check `LaneArrangement` serde round-trips an arrangement with unassigned channels (registry TOML already stores lanes as a list — confirm and add a serde test if the loader validates completeness anywhere).

**Important:** unassigned channels must still JUDGE (lane profiles never change judgment routing — spec + `lane_of` contract). Verify: gameplay `LaneHit` lanes come from fixed `lane_of`, display lookup for a channel with no display lane must not panic — grep display-column lookups (`dtx-layout/src/lanes.rs`) and make them `Option`-safe where needed. Add a test in gameplay-drums that a chart with a hidden lane's channel still scores (mirror an existing lanes.rs test).

- [ ] **Step 3: Run + commit**

Run: `cargo test -p dtx-layout && cargo test -p gameplay-drums`

```bash
git add crates/dtx-layout/src/lane_edit.rs crates/dtx-layout/src/lanes.rs
git commit -m "feat(layout): lane hide/restore ops with width floor"
```

---

### Task 8: Lanes tab rebuild — slim rows + detail card + hidden strip

**Files:**
- Create: `crates/gameplay-drums/src/editor/lanes_panel.rs` (move `spawn_lane_block` + lane handlers out of `panel.rs`)
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (dispatch to new module; delete moved code)
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

- [ ] **Step 1: Move, then reshape**

Mechanical move first (compile green, commit `refactor(customize): extract lanes_panel from panel.rs`), then reshape:

- **Selection resource:** `#[derive(Resource, Default)] pub struct SelectedLane(pub Option<usize>);` — row click selects; preview pad click (Task 9) selects; selection lights the lane via the existing `bindings_spatial` outline path.
- **Slim rows:** drag-handle glyph `≡` (drag in Step 3), `spawn_channel_dot`, lane name, muted `+HHO`-style secondary summary (`TEXT_MUTED`). Selected styling as Controls rows. The old inline `^ v` buttons, width slider, split/merge chips (`panel.rs:565-687`) move into the detail card or die: reorder = drag or keyboard verb; `^ v` buttons die.
- **Detail card** (below the list, only when `SelectedLane` is Some): title "{ID} LANE"; width `controls::spawn_slider` + numeric multiplier text (existing `LaneWidthSlider(i)` handler `apply_lane_width_sliders` `panel.rs:1311` moves along); channel chips — primary flat, secondaries `spawn_chip` with `×` = existing split (`ChipSplitBtn`); `+ add` opens a small popup of `unassigned_channels` + channels currently secondary elsewhere (existing merge semantics — reuse `LaneMergeBtn` handler logic re-keyed to explicit channel choice); `hide lane` button → `hide_lane`, clear selection.
- **Hidden strip:** when `unassigned_channels` non-empty, a muted card "HIDDEN" listing chips per channel; click → `restore_lane`.
- **Row drag reorder (mouse):** on drag over the list, compute target index from cursor Y (row height is fixed) and call the existing `reorder_lane`; simplest correct version = on-release drop (no live ghost).
- All edits keep the existing flow: mutate `Lanes` → `mirror_lane_edits_to_draft` (`panel.rs:1283`) → dirty dot appears via the Task 4 bar. Delete the static `PresetLabel` (`panel.rs:551`) — the profile bar's selector replaces it.

- [ ] **Step 2: Keyboard/pad verbs**

Extend the Lanes focus reducer (new, in `lanes_panel.rs`, same shape as `reduce_controls_nav`):

```rust
pub enum LanesFocus { TabBar, Rows, Detail }
pub fn reduce_lanes_nav(focus: LanesFocus, verb: NavVerb, move_held: bool) -> LanesFocusOutcome
```

- `Rows`: Up/Down moves selection; Confirm/Down→`Detail`; with move-verb held (reuse the surface's existing modifier convention) Up/Down calls `reorder_lane`.
- `Detail`: Left/Right adjusts width (`set_lane_width` ± step); Up/Back→`Rows`; Confirm cycles detail controls.
Unit-test the reducer transitions like `controls_down_enters_segment_then_rows` (`controls_panel.rs:256`).

- [ ] **Step 3: Run + commit**

Run: `cargo test -p gameplay-drums`

```bash
git add crates/gameplay-drums/src/editor/lanes_panel.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(customize): lanes tab slim rows, detail card, hidden strip"
```

---

### Task 9: Preview manipulation (drag pads to reorder/resize)

**Files:**
- Modify: `crates/gameplay-drums/src/editor/picking.rs` (pads pickable on Lanes tab)
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs` or `lanes_panel.rs` (drag systems)

- [ ] **Step 1: Failing tests for the pure drag math**

```rust
#[test]
fn drag_x_maps_to_target_index() {
    // 4 lanes with centers at scene-x 100,200,300,400: dropping at 260 → index 2 (before HT).
    let centers = [100.0, 200.0, 300.0, 400.0];
    assert_eq!(drop_index(&centers, 260.0), 2);
    assert_eq!(drop_index(&centers, 50.0), 0);
    assert_eq!(drop_index(&centers, 500.0), 3);
}

#[test]
fn edge_drag_scales_width_with_floor() {
    // Lane 60px wide at width 1.0; dragging its right edge +30px → 1.5; -70px clamps to floor.
    assert!((edge_width(1.0, 60.0, 30.0) - 1.5).abs() < 1e-4);
    assert_eq!(edge_width(1.0, 60.0, -70.0), dtx_layout::lane_edit::MIN_LANE_WIDTH);
}
```

Run: `cargo test -p gameplay-drums drop_index` → FAIL.

- [ ] **Step 2: Implement**

```rust
/// Target lane index for a dropped pad at scene-x: nearest center.
pub fn drop_index(centers: &[f32], x: f32) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - x).abs().partial_cmp(&(*b - x).abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// New width multiplier from an edge drag: current px + delta, floored.
pub fn edge_width(current_mult: f32, current_px: f32, dx: f32) -> f32 {
    let px_per_mult = current_px / current_mult;
    ((current_px + dx) / px_per_mult).max(dtx_layout::lane_edit::MIN_LANE_WIDTH)
}
```

Wire drag systems (active only on Lanes tab, using existing conversions — cursor→scene via `stage_rect::window_to_scene`, pad geometry from the existing lane outline/pick data in `picking.rs`/`bindings_spatial.rs`):

- Press on pad body → arm reorder drag, remember lane index; on release call `reorder_lane(lanes, from, drop_index(..))`.
- Press within ~6 scene-px of a pad's left/right edge → arm resize; per-frame `set_lane_width(lanes, i, edge_width(..))` (live feedback); edge zone wins over body when both hit.
- Click without drag → `SelectedLane` select only.
- Every mutation goes through the same `Lanes` → mirror-to-draft path as panel edits.

- [ ] **Step 3: Run + commit**

Run: `cargo test -p gameplay-drums`

```bash
git add crates/gameplay-drums/src/editor/picking.rs crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/editor/lanes_panel.rs
git commit -m "feat(customize): drag lanes preview to reorder and resize"
```

---

### Task 10: Cleanup, startup verify, full sweep, BRP smoke

**Files:**
- Modify: touched editor files (color sweep), `crates/gameplay-drums/src/lanes.rs` (startup load check)

- [ ] **Step 1: Verify lane registry loads at startup**

Explore flagged this: registries load at save-commit, startup path unconfirmed. Grep `load_lane_registry` / `load_layout_with_lane_authority` callers; confirm the arrangement in a fresh run comes from `lane-profiles.toml`'s active profile (add the runtime load beside the keyboard/midi `reload_profiles` in `bindings.rs:216` if missing). Test: unit-test the load path; manual: set active profile to NX Type-B, relaunch, lanes match.

- [ ] **Step 2: Style sweep**

In every file this plan touched, replace remaining inline `Color::srgb` literals with `chrome::` tokens (only in touched files — no repo-wide restyle). Delete dead code: `PresetLabel`, `preset_name` if unreferenced, `^ v` reorder buttons, `MODIFIED` badge, `ConfirmMidiSteal` leftovers.

- [ ] **Step 3: Full test sweep**

Run: `cargo test --workspace`
Expected: PASS. Fix stragglers.

- [ ] **Step 4: BRP smoke checklist (manual, from a worktree launch)**

Per the repo's established BRP loop (see `docs/superpowers/customize-visual-punchlist.md`):

1. Controls/Keyboard: segment toggle visible; rows show key chips only; profile bar shows keyboard profile; edit → dirty dot; Save As → name dialog → new profile in dropdown.
2. Controls/MIDI: device card with port dropdown + status dot; `+` → modal "Hit a pad for SD"; hit pad → note+velocity shown → Enter commits; hit a note owned elsewhere → Add shared / Move here; shared chip shows ⧉ and hover lights both lanes.
3. One key → BD+LBD: bind Space shared to LBD, in gameplay one press hits both lanes (Task 1 fan-out, MIDI: same with a shared note).
4. Lanes: slim rows; select → detail card; width slider live in preview; drag pad in preview → reorder; drag edge → resize; hide HH → hidden strip; restore. Preset dropdown switches instantly; dirty → switching prompts.
5. Close with dirty draft → existing dirty dialog.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore(customize): style sweep, startup lane registry load, cleanup"
```

---

## Self-review notes (already applied)

- Spec coverage: shell/profile bar (T4), segments (T5), capture modal + shared/steal (T2/T6), MIDI fan-out (T1), slim rows/detail/hidden (T7/T8), preview manipulation (T9), focus model (T5 selector focus, T8 reducer), quiet tokens (T3), edge cases (unbound tint T5, width floor T7, disconnected port = existing PortMatch untouched), startup load (T10).
- Bevy API details (exact `BorderColor`/`Theme::font` signatures) intentionally deferred to compiling call sites in the named anchor files — the repo's Bevy version is authoritative.
- Keyboard fan-out needs no work (already `lanes_for_key`); only MIDI changes.
