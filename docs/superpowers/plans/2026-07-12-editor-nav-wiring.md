# Editor Nav Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing-but-unused nav reducers (`reduce_controls_nav`, `reduce_lanes_nav`) into production so the Controls and Lanes tabs are fully keyboard-operable, add a Tab/Shift+Tab widget-selection cycle to the Widgets tab, and give all four mouse-only button dialogs (close guard, Dirty, ConfirmDelete, CorruptReset) ←/→/Enter/Esc traversal. Fixes audit findings F18 and F14. Spec: `docs/superpowers/specs/2026-07-12-editor-nav-wiring-design.md` (authority). Research: `docs/notes/2026-07-12-streams-research.md` §Stream 3.

**Architecture:** New per-tab `NavAction` consumer systems (keyboard-source only, parallel `MessageReader`s — the generic `settings_nav_consumer` keeps its own reader) drive the existing pure reducers; the consumers own only what the reducers don't model (row cursor, capture arming, binding delete, undo pushes, width mutation). Dialogs get a focused-index resource per file plus a keyboard system that dispatches through the exact same code path as mouse clicks. Visual focus reuses the existing `FOCUS_RING` + `Outline` pattern from `keyboard_nav.rs`.

**Tech Stack:** Rust, Bevy 0.19 (`Message`/`MessageReader`/`MessageWriter`, `run_if` conditions combined with `.or_else` as the codebase already does at `panel.rs:124`), crate `gameplay-drums`. Repo rules: NO `unwrap()` anywhere under `crates/*` (use `expect` in tests), conventional commits like `feat(customize): ...`, NO co-author trailers ever.

---

## Findings from source reading (deviations from spec assumptions — the plan below already accounts for them)

1. **Spec Part C's assumption is FALSE:** `apply_practice_actions` / `emit_practice_actions` are gated `PauseState::Running` + `resource_exists::<PracticeSession>` only (`crates/gameplay-drums/src/practice/mod.rs:30-40`) — there is **no `editor_closed` gate**. In practice the two states shouldn't coexist (PracticeSession requires `PracticeIntent`, the editor session doesn't set it), but the spec's instruction applies: add the gate and a regression test (Task 7).
2. **Tab is already taken inside the editor:** `update_preview_state` (`editor/mod.rs:81-97`) maps *held* Tab to the play-view peek (footer legend: "Tab peek"). A naive `just_pressed(Tab)` cycle would advance the selection on every peek. Resolution: tap-vs-hold — a Tab *released* within 0.25 s cycles; a longer hold stays a pure peek (Task 7).
3. **The generic consumer is NOT a full no-op on Controls/Lanes:** at `NavLevel::Rail`, `settings_nav_consumer` switches tabs on Dec/Inc **unconditionally** (`keyboard_nav.rs:204-205`). Once Controls focus is at SegmentSelector (where Dec/Inc must toggle the segment) or Lanes focus is at Detail (where Dec/Inc must adjust width), the same keypress would ALSO switch the active tab. Task 1 adds a `subtab_focus_captured` guard.
4. **Close dialog does not suppress panel nav:** the `keyboard_nav` chain is gated `profile_dialog_closed` only — while `PendingCloseState::Pending` (close dialog up), arrows still drive the panel underneath and Enter is double-handled. Task 1 adds a `pending_close_none` run condition (the gap the spec told us to verify in Part D).
5. **Esc is not `NavVerb::Back`:** `keyboard_emit_nav` never emits `Back`; Esc goes to `close_on_escape`. The spec's Lanes Detail footer hint says "Esc back", so Task 3 reads Esc directly in the lanes consumer and suppresses `close_on_escape` while Detail is focused.
6. **Enter is not a reserved capture key** (`bindings_capture::is_reserved`): the Enter that arms a capture would be captured as the binding itself if `capture_binding` runs after the consumer in the same frame. Task 2 orders the consumer `.after(capture_binding)` and skips frames where `CaptureState` just changed (also prevents the Enter that commits an Arrived state from instantly re-arming).
7. The Widgets sidebar list renders `WidgetKind::ALL` (`panel.rs:372`) — that IS the cycle order (11 kinds, includes Playfield, which is selectable-but-uninspectable, same as clicking it in the list).
8. `move_lane_to` (`lane_drag.rs:82`) and `bindable_channels_in_order` (`bindings_panel.rs:168`) are private; both become `pub(super)` (sibling modules under `editor/` can see them).

**Untouchable, must stay green:** all existing reducer tests (`controls_panel.rs:241-284`, `lanes_panel.rs:744-820`), `pad_exclusion_matches_controls_contract` (`controls_panel.rs:278`), the `apply_keyboard` mirror tests in `keyboard_nav.rs`, and every other existing test. No reducer API changes.

---

## File structure

```
crates/gameplay-drums/src/
├── editor/
│   ├── mod.rs                 # Task 2: register controls_panel::plugin
│   ├── keyboard_nav.rs        # Task 1: pending_close_none gate on chain; subtab_focus_captured guard
│   ├── profile_state.rs       # Task 1: pub fn pending_close_none (run condition)
│   ├── controls_panel.rs      # Task 2: RowStep + step_channel + controls_nav_consumer + plugin
│   ├── bindings_panel.rs      # Task 2: pub(super) bindable_channels_in_order + last_segment_source_index
│   │                          # Task 4: Outline on BindChannelRow spawn
│   ├── bindings_capture.rs    # Task 4: highlight_selected_row drives the FOCUS_RING outline
│   ├── lanes_panel.rs         # Task 1: init LanesFocus; Task 3: WIDTH_STEP + lanes_nav_consumer
│   │                          #   + lanes_detail_focus; Task 5: focus param + outlines
│   ├── lane_drag.rs           # Task 3: move_lane_to → pub(super)
│   ├── ui.rs                  # Task 3: close_on_escape pub(super) + not(lanes_detail_focus) gate
│   ├── panel.rs               # Task 5: LanesFocus in LeftPanelSig + run condition + pass-through
│   ├── footer.rs              # Task 6: nav_hint_text + update_footer_desc priority slot
│   ├── drag.rs                # Task 7: cycle_widget + cycle_widget_selection
│   ├── close_dialog.rs        # Task 8: CloseDialogFocus + step_focus + keys + focus ring
│   └── profile_dialog_ui.rs   # Task 9: ProfileDialogFocus + dialog_buttons + keys + focus ring
└── practice/
    └── mod.rs                 # Task 7: editor_closed gate + regression test
```

---

## Task 1 — Shared nav gating groundwork

**Files:**
- `crates/gameplay-drums/src/editor/profile_state.rs` (new run condition)
- `crates/gameplay-drums/src/editor/keyboard_nav.rs` (gate + rail guard + tests)
- `crates/gameplay-drums/src/editor/lanes_panel.rs` (init `LanesFocus`)

### Step 1.1 — failing test: rail tab-switch guard

- [ ] Append to the `tests` module in `crates/gameplay-drums/src/editor/keyboard_nav.rs`:

```rust
    #[test]
    fn rail_tab_switch_yields_while_subtab_focus_is_below_tabbar() {
        use crate::editor::controls_panel::ControlsFocus;
        use crate::editor::lanes_panel::LanesFocus;
        // Controls: only a focus below TabBar captures ←/→.
        assert!(!subtab_focus_captured(
            CustomizeTab::Controls,
            ControlsFocus::TabBar,
            LanesFocus::TabBar
        ));
        assert!(subtab_focus_captured(
            CustomizeTab::Controls,
            ControlsFocus::SegmentSelector,
            LanesFocus::TabBar
        ));
        assert!(subtab_focus_captured(
            CustomizeTab::Controls,
            ControlsFocus::Rows,
            LanesFocus::TabBar
        ));
        // Lanes: Rows/Detail capture; TabBar does not.
        assert!(!subtab_focus_captured(
            CustomizeTab::Lanes,
            ControlsFocus::TabBar,
            LanesFocus::TabBar
        ));
        assert!(subtab_focus_captured(
            CustomizeTab::Lanes,
            ControlsFocus::TabBar,
            LanesFocus::Detail
        ));
        // Settings tabs never capture, regardless of stale kit focus.
        assert!(!subtab_focus_captured(
            CustomizeTab::Gameplay,
            ControlsFocus::Rows,
            LanesFocus::Detail
        ));
    }
```

- [ ] Run `cargo test -p gameplay-drums rail_tab_switch_yields` — expect **compile failure** (`subtab_focus_captured` does not exist).

### Step 1.2 — implementation

- [ ] In `crates/gameplay-drums/src/editor/profile_state.rs`, next to `PendingCloseState`, add:

```rust
/// Run condition: the dirty-close guard dialog is NOT up. Keyboard nav
/// (and the new per-tab consumers) must yield to the dialog exactly as they
/// already yield to profile dialogs via `profile_dialog_closed`.
pub fn pending_close_none(pending: bevy::prelude::Res<PendingCloseState>) -> bool {
    matches!(*pending, PendingCloseState::None)
}
```

(If `profile_state.rs` does not already import bevy's prelude for `Res`, use the fully qualified form above — it compiles either way.)

- [ ] In `crates/gameplay-drums/src/editor/keyboard_nav.rs`, add the pure guard (near `pad_excluded`):

```rust
/// True when a kit tab's OWN focus machine sits below the tab bar — the
/// generic Rail-level ←/→ tab switch must yield there (Controls uses ←/→ to
/// toggle the segment, Lanes Detail uses them to adjust width).
pub fn subtab_focus_captured(
    tab: CustomizeTab,
    controls: super::controls_panel::ControlsFocus,
    lanes: super::lanes_panel::LanesFocus,
) -> bool {
    match tab {
        CustomizeTab::Controls => controls != super::controls_panel::ControlsFocus::TabBar,
        CustomizeTab::Lanes => lanes != super::lanes_panel::LanesFocus::TabBar,
        _ => false,
    }
}
```

- [ ] In the same file, wire it into `settings_nav_consumer`: add two params after `mut actions`:

```rust
    controls_focus: Res<super::controls_panel::ControlsFocus>,
    lanes_focus: Res<super::lanes_panel::LanesFocus>,
```

and insert this as the FIRST statement inside the `for action in actions.read()` loop (before `let items = ...`), leaving the existing arms and the `apply_keyboard` test mirror untouched:

```rust
        // A kit tab whose own focus machine is below the tab bar owns
        // Dec/Inc (segment toggle / width adjust) — don't also switch tabs.
        if action.source == NavSource::Keyboard
            && matches!(action.verb, NavVerb::Dec | NavVerb::Inc)
            && matches!(*level, NavLevel::Rail)
            && subtab_focus_captured(active.0, *controls_focus, *lanes_focus)
        {
            continue;
        }
```

- [ ] Still in `keyboard_nav.rs`, extend the plugin chain's run conditions (`plugin`, ~line 56) with the close-guard gate — after `.run_if(super::profile_dialog::profile_dialog_closed)` add:

```rust
                .run_if(super::profile_state::pending_close_none),
```

- [ ] In `crates/gameplay-drums/src/editor/lanes_panel.rs` `plugin`, register the dead resource (first line of the fn body):

```rust
    app.init_resource::<LanesFocus>();
```

(keep the existing `init_resource::<SelectedLane>()` chain as-is; a separate statement is fine).

### Step 1.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums rail_tab_switch_yields` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums editor::keyboard_nav` — all existing keyboard_nav tests still pass.
- [ ] Commit: `feat(customize): gate editor nav under close dialog and kit-tab focus`

---

## Task 2 — Controls tab keyboard nav consumer

**Files:**
- `crates/gameplay-drums/src/editor/bindings_panel.rs` (expose display order; new pure helper + tests)
- `crates/gameplay-drums/src/editor/controls_panel.rs` (`RowStep`, `step_channel`, consumer, plugin, tests)
- `crates/gameplay-drums/src/editor/mod.rs` (register plugin)

### Step 2.1 — failing tests: pure parts

- [ ] In `crates/gameplay-drums/src/editor/bindings_panel.rs`, change `fn bindable_channels_in_order` to `pub(super) fn bindable_channels_in_order` (line ~168; its existing tests keep passing).
- [ ] Append to `bindings_panel.rs` tests:

```rust
    #[test]
    fn last_segment_source_index_picks_last_matching_segment() {
        use dtx_core::EChannel;
        use dtx_input::{BindSource, InputBindings};

        let mut b = InputBindings::default();
        b.map.insert(
            EChannel::Snare,
            vec![
                BindSource::Key(KeyCode::KeyA),
                BindSource::Midi { note: 60 },
                BindSource::Key(KeyCode::KeyB),
                BindSource::Midi { note: 61 },
            ],
        );
        assert_eq!(
            last_segment_source_index(&b, EChannel::Snare, ControlsSegment::Keyboard),
            Some(2),
            "last KEY source, full-list index"
        );
        assert_eq!(
            last_segment_source_index(&b, EChannel::Snare, ControlsSegment::Midi),
            Some(3)
        );
        // No source in the segment → None (Backspace no-ops).
        b.map.insert(EChannel::HighTom, vec![BindSource::Midi { note: 48 }]);
        assert_eq!(
            last_segment_source_index(&b, EChannel::HighTom, ControlsSegment::Keyboard),
            None
        );
        // Unknown channel → None.
        b.map.remove(&EChannel::LowTom);
        assert_eq!(
            last_segment_source_index(&b, EChannel::LowTom, ControlsSegment::Keyboard),
            None
        );
    }
```

- [ ] Append to `crates/gameplay-drums/src/editor/controls_panel.rs` tests:

```rust
    #[test]
    fn row_step_walks_display_order_and_hands_off_at_top() {
        use dtx_core::EChannel;
        let chs = [EChannel::LeftCymbal, EChannel::HiHatClose, EChannel::Snare];
        // Stale / missing selection clamps to the first channel.
        assert_eq!(step_channel(&chs, None, 1), RowStep::Select(EChannel::LeftCymbal));
        assert_eq!(
            step_channel(&chs, Some(EChannel::Cymbal), -1),
            RowStep::Select(EChannel::LeftCymbal),
            "channel not in display order clamps to first"
        );
        // Down walks and clamps at the bottom.
        assert_eq!(
            step_channel(&chs, Some(EChannel::LeftCymbal), 1),
            RowStep::Select(EChannel::HiHatClose)
        );
        assert_eq!(
            step_channel(&chs, Some(EChannel::Snare), 1),
            RowStep::Select(EChannel::Snare)
        );
        // Up from the first row returns focus to the segment selector.
        assert_eq!(
            step_channel(&chs, Some(EChannel::HiHatClose), -1),
            RowStep::Select(EChannel::LeftCymbal)
        );
        assert_eq!(step_channel(&chs, Some(EChannel::LeftCymbal), -1), RowStep::ToSegmentSelector);
        // Empty list: Up hands off, Down does nothing.
        assert_eq!(step_channel(&[], None, -1), RowStep::ToSegmentSelector);
        assert_eq!(step_channel(&[], None, 1), RowStep::None);
    }
```

- [ ] Run `cargo test -p gameplay-drums last_segment_source_index_picks row_step_walks` — expect **compile failure** (helpers don't exist).

### Step 2.2 — implement pure helpers

- [ ] In `bindings_panel.rs`, below `segment_matches`:

```rust
/// Index (into the channel's FULL, unfiltered source list) of the LAST
/// source belonging to `segment` — the target of a keyboard Backspace on the
/// Controls rows. `None` = nothing to delete (Backspace no-ops).
pub(super) fn last_segment_source_index(
    b: &InputBindings,
    channel: dtx_core::EChannel,
    segment: ControlsSegment,
) -> Option<usize> {
    b.map
        .get(&channel)?
        .iter()
        .rposition(|source| segment_matches(segment, source))
}
```

- [ ] In `controls_panel.rs`, below `reduce_controls_nav`:

```rust
/// Outcome of one Up/Down step through the Controls rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStep {
    /// Up from the first row: focus returns to the segment selector.
    ToSegmentSelector,
    /// The row cursor lands on this channel.
    Select(EChannel),
    /// Nothing to do (Down on an empty row list).
    None,
}

/// Step the row cursor through `channels` (the panel's display order). A
/// missing or stale `current` clamps to the first channel; Up from the first
/// row hands focus back to the segment selector (the reducer's Rows+Up arm).
pub fn step_channel(channels: &[EChannel], current: Option<EChannel>, dir: i32) -> RowStep {
    if channels.is_empty() {
        return if dir < 0 { RowStep::ToSegmentSelector } else { RowStep::None };
    }
    let Some(index) = current.and_then(|ch| channels.iter().position(|c| *c == ch)) else {
        return RowStep::Select(channels[0]);
    };
    if dir < 0 {
        if index == 0 {
            RowStep::ToSegmentSelector
        } else {
            RowStep::Select(channels[index - 1])
        }
    } else {
        RowStep::Select(channels[(index + 1).min(channels.len() - 1)])
    }
}
```

- [ ] Run `cargo test -p gameplay-drums row_step_walks last_segment_source_index_picks` — expect **pass**.

### Step 2.3 — failing test: the consumer system

- [ ] Append to `controls_panel.rs` tests:

```rust
    #[test]
    fn keyboard_descends_arms_capture_and_deletes_last_binding() {
        use crate::editor::bindings_capture::{CaptureState, SelectedChannel};
        use crate::editor::bindings_panel::BindingsRev;
        use bevy::prelude::*;
        use dtx_core::EChannel;
        use game_shell::{NavAction, NavSource};

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsFocus>()
            .init_resource::<ControlsSegment>()
            .init_resource::<CaptureState>()
            .init_resource::<SelectedChannel>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::bindings::LiveBindings>()
            .init_resource::<crate::lanes::Lanes>()
            .insert_resource(crate::editor::tabs::ActiveTab(
                game_shell::CustomizeTab::Controls,
            ))
            .add_message::<NavAction>()
            .add_systems(Update, controls_nav_consumer);
        // First update flushes the ActiveTab insertion's change tick.
        app.update();

        fn nav(app: &mut App, verb: NavVerb) {
            app.world_mut()
                .resource_mut::<Messages<NavAction>>()
                .write(NavAction {
                    verb,
                    source: NavSource::Keyboard,
                    coarse: false,
                });
            app.update();
        }

        nav(&mut app, NavVerb::Down);
        assert_eq!(
            *app.world().resource::<ControlsFocus>(),
            ControlsFocus::SegmentSelector
        );
        nav(&mut app, NavVerb::Down);
        assert_eq!(*app.world().resource::<ControlsFocus>(), ControlsFocus::Rows);
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::LeftCymbal),
            "entering Rows seeds the cursor with the first display channel"
        );
        nav(&mut app, NavVerb::Down);
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::HiHatClose)
        );

        // Backspace deletes the LAST keyboard source of the selected channel.
        let before = app
            .world()
            .resource::<crate::bindings::LiveBindings>()
            .0
            .map
            .get(&EChannel::HiHatClose)
            .map(Vec::len)
            .expect("HH has default bindings");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Backspace);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        let after = app
            .world()
            .resource::<crate::bindings::LiveBindings>()
            .0
            .map
            .get(&EChannel::HiHatClose)
            .map(Vec::len)
            .expect("HH row still present");
        assert_eq!(after, before - 1, "one keyboard source removed");
        assert_eq!(app.world().resource::<BindingsRev>().0, 1, "repaint requested");

        // Enter arms keyboard capture for the selected channel.
        nav(&mut app, NavVerb::Confirm);
        assert!(matches!(
            *app.world().resource::<CaptureState>(),
            CaptureState::Keyboard(EChannel::HiHatClose)
        ));

        // While the capture is armed the consumer is inert.
        nav(&mut app, NavVerb::Down);
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::HiHatClose),
            "capture owns input; row cursor frozen"
        );
    }
```

- [ ] Run `cargo test -p gameplay-drums keyboard_descends_arms_capture` — expect **compile failure** (`controls_nav_consumer` does not exist).

### Step 2.4 — implement the consumer

- [ ] In `controls_panel.rs`, extend the imports at the top:

```rust
use game_shell::{NavAction, NavSource, NavVerb};

use super::bindings_capture::{CaptureState, SelectedChannel};
use super::bindings_panel::BindingsRev;
use crate::bindings::LiveBindings;
use crate::lanes::Lanes;
```

(replacing the existing `use game_shell::NavVerb;` line; `bevy::prelude::*` is already imported.)

- [ ] Add the consumer + plugin below `step_channel`:

```rust
/// Keyboard-only `NavAction` consumer for the Controls tab. Level moves go
/// through the pure `reduce_controls_nav`; at `Rows` the consumer owns what
/// the reducer doesn't model: the row cursor (`SelectedChannel` through the
/// panel's display order), Enter → capture arming, Backspace → delete the
/// last binding of the selected channel in the active segment.
///
/// Ordered `.after(capture_binding)`: Enter is NOT a reserved capture key,
/// so the press that arms a capture must already be stale when the capture
/// machine next reads the keyboard — and the Enter that commits an Arrived
/// state must not re-enter here and instantly re-arm (covered by the
/// `capture.is_changed()` skip).
#[allow(clippy::too_many_arguments)]
pub(super) fn controls_nav_consumer(
    mut actions: MessageReader<NavAction>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<super::tabs::ActiveTab>,
    lanes: Res<Lanes>,
    mut capture: ResMut<CaptureState>,
    mut focus: ResMut<ControlsFocus>,
    mut segment: ResMut<ControlsSegment>,
    mut selected: ResMut<SelectedChannel>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    if active.0 != game_shell::CustomizeTab::Controls {
        return; // own reader: unread messages just expire
    }
    if active.is_changed() && *focus != ControlsFocus::TabBar {
        // Fresh visit (tab switched here): keyboard focus restarts at the bar.
        *focus = ControlsFocus::TabBar;
    }
    if !matches!(*capture, CaptureState::Idle) || capture.is_changed() {
        actions.clear();
        return;
    }
    let channels = super::bindings_panel::bindable_channels_in_order(&lanes.0);
    // Backspace is not a NavVerb — read it directly, Rows level only.
    if *focus == ControlsFocus::Rows && keys.just_pressed(KeyCode::Backspace) {
        if let Some(channel) = selected.0 {
            if let Some(index) =
                super::bindings_panel::last_segment_source_index(&live.0, channel, *segment)
            {
                if let Some(sources) = live.0.map.get_mut(&channel) {
                    sources.remove(index);
                    rev.0 = rev.0.wrapping_add(1);
                }
            }
        }
    }
    for action in actions.read() {
        if action.source != NavSource::Keyboard {
            continue;
        }
        match (*focus, action.verb) {
            (ControlsFocus::Rows, NavVerb::Up) | (ControlsFocus::Rows, NavVerb::Down) => {
                let dir = if action.verb == NavVerb::Up { -1 } else { 1 };
                match step_channel(&channels, selected.0, dir) {
                    RowStep::ToSegmentSelector => {
                        let (next_focus, next_segment) =
                            reduce_controls_nav(*focus, *segment, NavVerb::Up);
                        if *focus != next_focus {
                            *focus = next_focus;
                        }
                        if *segment != next_segment {
                            *segment = next_segment;
                        }
                    }
                    RowStep::Select(channel) => {
                        if selected.0 != Some(channel) {
                            selected.0 = Some(channel);
                        }
                    }
                    RowStep::None => {}
                }
            }
            (ControlsFocus::Rows, NavVerb::Confirm) => {
                if let Some(channel) = selected.0.filter(|ch| channels.contains(ch)) {
                    *capture = match *segment {
                        ControlsSegment::Keyboard => CaptureState::Keyboard(channel),
                        ControlsSegment::Midi => CaptureState::Midi(channel),
                    };
                    actions.clear();
                    return; // the capture flow owns input from here
                }
            }
            _ => {
                let (next_focus, next_segment) =
                    reduce_controls_nav(*focus, *segment, action.verb);
                let entered_rows =
                    *focus != ControlsFocus::Rows && next_focus == ControlsFocus::Rows;
                if *focus != next_focus {
                    *focus = next_focus;
                }
                if *segment != next_segment {
                    *segment = next_segment;
                }
                if entered_rows && !selected.0.is_some_and(|ch| channels.contains(&ch)) {
                    // Seed the row cursor so Enter/Backspace always target a row.
                    if let Some(first) = channels.first() {
                        selected.0 = Some(*first);
                    }
                }
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        controls_nav_consumer
            .after(super::bindings_capture::capture_binding)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open)
            .run_if(super::profile_dialog::profile_dialog_closed)
            .run_if(super::profile_state::pending_close_none),
    );
}
```

- [ ] In `crates/gameplay-drums/src/editor/mod.rs`, add `controls_panel::plugin,` to the inner plugin tuple (the group that already holds `bindings_panel::plugin`, `lanes_panel::plugin`, …).

### Step 2.5 — verify & commit

- [ ] Run `cargo test -p gameplay-drums keyboard_descends_arms_capture` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums editor::controls_panel editor::bindings_panel` — all existing tests (incl. `pad_exclusion_matches_controls_contract`, `controls_up_returns_one_level`, display-order tests) still pass.
- [ ] Commit: `feat(customize): keyboard nav consumer for the Controls tab`

---

## Task 3 — Lanes tab keyboard nav consumer

**Files:**
- `crates/gameplay-drums/src/editor/lane_drag.rs` (`move_lane_to` → `pub(super)`)
- `crates/gameplay-drums/src/editor/lanes_panel.rs` (`WIDTH_STEP`, consumer, `lanes_detail_focus`, registration, tests)
- `crates/gameplay-drums/src/editor/ui.rs` (`close_on_escape` visibility + gate)

### Step 3.1 — failing test

- [ ] Append to `lanes_panel.rs` tests:

```rust
    #[test]
    fn lanes_consumer_reorders_adjusts_width_and_batches_undo_per_visit() {
        use bevy::prelude::*;
        use game_shell::{NavAction, NavSource};

        use crate::editor::undo::{Snapshot, UndoStack};
        use crate::widget_layout::WidgetLayouts;

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<LanesFocus>()
            .init_resource::<SelectedLane>()
            .init_resource::<Lanes>()
            .init_resource::<WidgetLayouts>()
            .init_resource::<UndoStack>()
            .insert_resource(crate::editor::tabs::ActiveTab(
                game_shell::CustomizeTab::Lanes,
            ))
            .add_message::<NavAction>()
            .add_systems(Update, lanes_nav_consumer);
        app.update(); // flush insertion change ticks

        let mut nav = |app: &mut App, verb: NavVerb, coarse: bool| {
            app.world_mut()
                .resource_mut::<Messages<NavAction>>()
                .write(NavAction {
                    verb,
                    source: NavSource::Keyboard,
                    coarse,
                });
            app.update();
        };

        // TabBar → Rows; None selection bridges to 0.
        nav(&mut app, NavVerb::Down, false);
        assert_eq!(*app.world().resource::<LanesFocus>(), LanesFocus::Rows);
        assert_eq!(app.world().resource::<SelectedLane>().0, Some(0));

        // Shift+Down twice: two reorders, one undo snapshot EACH.
        let id0 = app.world().resource::<Lanes>().0.lanes[0].id.clone();
        nav(&mut app, NavVerb::Down, true);
        nav(&mut app, NavVerb::Down, true);
        assert_eq!(app.world().resource::<Lanes>().0.lanes[2].id, id0);
        assert_eq!(app.world().resource::<SelectedLane>().0, Some(2));

        // Enter → Detail; ←/→ adjust width with ONE snapshot for the visit.
        nav(&mut app, NavVerb::Confirm, false);
        assert_eq!(*app.world().resource::<LanesFocus>(), LanesFocus::Detail);
        let w0 = app.world().resource::<Lanes>().0.lanes[2].width;
        nav(&mut app, NavVerb::Inc, false); // +4
        nav(&mut app, NavVerb::Inc, true); // +16 (coarse ×4)
        nav(&mut app, NavVerb::Dec, false); // −4
        let w1 = app.world().resource::<Lanes>().0.lanes[2].width;
        assert!((w1 - (w0 + 16.0)).abs() < 0.01, "4 + 16 - 4 = +16, got {}", w1 - w0);

        // Esc backs out to Rows (does not close the surface — gated in ui.rs).
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert_eq!(*app.world().resource::<LanesFocus>(), LanesFocus::Rows);

        // Second Detail visit: width undo re-arms → one MORE snapshot.
        nav(&mut app, NavVerb::Confirm, false);
        nav(&mut app, NavVerb::Inc, false);

        // Snapshot ledger: 2 (reorders) + 1 (visit one) + 1 (visit two) = 4.
        let world = app.world_mut();
        let current = Snapshot {
            layouts: world.resource::<WidgetLayouts>().clone(),
            lanes: world.resource::<Lanes>().clone(),
        };
        let mut stack_pops = 0;
        {
            let mut stack = world.resource_mut::<UndoStack>();
            let mut cursor = current;
            while let Some(prev) = stack.undo(cursor.clone()) {
                cursor = prev;
                stack_pops += 1;
            }
        }
        assert_eq!(stack_pops, 4, "per-press reorder undo + once-per-visit width undo");
    }

    #[test]
    fn lanes_width_adjust_clamps_at_both_bounds() {
        // Clamp is applied by the consumer via the shared band; verify the
        // arithmetic contract the consumer uses.
        let min = dtx_layout::MIN_LANE_WIDTH;
        let max = dtx_layout::MAX_LANE_WIDTH;
        assert_eq!((min + -1.0 * WIDTH_STEP).clamp(min, max), min);
        assert_eq!((max + WIDTH_STEP * 4.0).clamp(min, max), max);
    }
```

- [ ] Run `cargo test -p gameplay-drums lanes_consumer_reorders` — expect **compile failure** (`lanes_nav_consumer`, `WIDTH_STEP` don't exist).

### Step 3.2 — implementation

- [ ] In `crates/gameplay-drums/src/editor/lane_drag.rs`, change `fn move_lane_to` to `pub(super) fn move_lane_to` (line ~82).
- [ ] In `lanes_panel.rs`, below the `LanesNavEffect` enum, add:

```rust
/// Ref-px width nudge per Detail ←/→ press (Shift/coarse: ×4). Same unit as
/// `dtx_layout::MIN_LANE_WIDTH`/`MAX_LANE_WIDTH`.
pub const WIDTH_STEP: f32 = 4.0;
```

- [ ] Add the consumer + run condition below `reduce_lanes_nav`:

```rust
/// Keyboard-only `NavAction` consumer for the Lanes tab. All focus/selection
/// transitions go through the pure `reduce_lanes_nav`; this driver applies
/// the returned effects: `Reorder` = one undo snapshot PER keypress + the
/// same adjacent-swap walk mouse drag uses; `AdjustWidth` = one undo
/// snapshot per Detail VISIT (drag's `pushed`-flag pattern) + clamped
/// `set_lane_width`. Esc maps to `NavVerb::Back` while Detail is focused
/// (`close_on_escape` is suppressed for that case and ordered before this).
#[allow(clippy::too_many_arguments)]
pub(super) fn lanes_nav_consumer(
    mut actions: MessageReader<game_shell::NavAction>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<super::tabs::ActiveTab>,
    layouts: Res<WidgetLayouts>,
    mut focus: ResMut<LanesFocus>,
    mut selected: ResMut<SelectedLane>,
    mut lanes: ResMut<Lanes>,
    mut undo: ResMut<UndoStack>,
    mut width_undo_pushed: Local<bool>,
) {
    use game_shell::{NavSource, NavVerb};

    if active.is_changed() && *focus != LanesFocus::TabBar {
        *focus = LanesFocus::TabBar;
        *width_undo_pushed = false;
    }
    let mut pending: Vec<(NavVerb, bool)> = actions
        .read()
        .filter(|action| action.source == NavSource::Keyboard)
        .map(|action| (action.verb, action.coarse))
        .collect();
    if *focus == LanesFocus::Detail && keys.just_pressed(KeyCode::Escape) {
        pending.push((NavVerb::Back, false));
    }
    for (verb, coarse) in pending {
        let lane_count = lanes.0.lanes.len();
        let cursor = selected.0.unwrap_or(0).min(lane_count.saturating_sub(1));
        let (next_focus, next_selected, effect) =
            reduce_lanes_nav(*focus, cursor, lane_count, verb, coarse);
        match effect {
            LanesNavEffect::Reorder { index, dir } => {
                // One snapshot per reorder keypress: each press IS a gesture
                // (unlike drag's one-per-hold).
                undo.push(&layouts, &lanes);
                let target = index.saturating_add_signed(dir as isize);
                super::lane_drag::move_lane_to(&mut lanes.0, index, target);
            }
            LanesNavEffect::AdjustWidth { index, dir } => {
                if let Some(lane) = lanes.0.lanes.get(index) {
                    let step = WIDTH_STEP * if coarse { 4.0 } else { 1.0 };
                    let next = (lane.width + dir as f32 * step)
                        .clamp(dtx_layout::MIN_LANE_WIDTH, dtx_layout::MAX_LANE_WIDTH);
                    if (next - lane.width).abs() > f32::EPSILON {
                        // One snapshot per Detail visit, armed just before
                        // the first real mutation (lane_drag's `pushed`).
                        if !*width_undo_pushed {
                            undo.push(&layouts, &lanes);
                            *width_undo_pushed = true;
                        }
                        dtx_layout::set_lane_width(&mut lanes.0, index, next);
                    }
                }
            }
            LanesNavEffect::None => {}
        }
        if next_focus != LanesFocus::Detail {
            *width_undo_pushed = false; // re-arm for the next Detail visit
        }
        if *focus != next_focus {
            *focus = next_focus;
        }
        if next_focus != LanesFocus::TabBar && selected.0 != Some(next_selected) {
            selected.0 = Some(next_selected);
        }
    }
}

/// Run condition (negated on `ui::close_on_escape`): the Lanes detail card
/// holds keyboard focus, so Esc means "back to rows", not "close Customize".
pub(super) fn lanes_detail_focus(
    active: Res<super::tabs::ActiveTab>,
    focus: Res<LanesFocus>,
) -> bool {
    active.0 == game_shell::CustomizeTab::Lanes && *focus == LanesFocus::Detail
}
```

- [ ] Register it in `lanes_panel::plugin` (separate `add_systems` call, appended to the existing chain):

```rust
    app.add_systems(
        Update,
        lanes_nav_consumer
            .after(super::ui::close_on_escape)
            .before(mirror_lane_edits_to_draft)
            .run_if(super::editor_open)
            .run_if(super::lanes_tab_active)
            .run_if(super::profile_dialog::profile_dialog_closed)
            .run_if(super::profile_state::pending_close_none)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
```

- [ ] In `crates/gameplay-drums/src/editor/ui.rs`: change `fn close_on_escape(` to `pub(super) fn close_on_escape(`, and in `plugin` add the gate so one Esc can't both back-out and close:

```rust
            close_on_escape
                .run_if(super::editor_open)
                .run_if(not(super::bindings_capture::capture_active))
                // Esc while the Lanes detail card is focused backs out one
                // level (lanes_nav_consumer, ordered after this) instead of
                // closing the surface.
                .run_if(not(super::lanes_panel::lanes_detail_focus))
                .before(super::calibration::confirm_or_cancel),
```

### Step 3.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums lanes_consumer_reorders lanes_width_adjust_clamps` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums editor::lanes_panel editor::lane_drag` — all existing reducer + drag tests still pass.
- [ ] Commit: `feat(customize): keyboard nav consumer for the Lanes tab`

---

## Task 4 — Controls visuals: FOCUS_RING on the selected row while at Rows

**Files:**
- `crates/gameplay-drums/src/editor/bindings_panel.rs` (spawn `Outline` on rows)
- `crates/gameplay-drums/src/editor/bindings_capture.rs` (`highlight_selected_row` drives it; test)

### Step 4.1 — failing test

- [ ] Append to `bindings_capture.rs` tests:

```rust
    #[test]
    fn rows_focus_draws_focus_ring_on_selected_row_only() {
        use crate::editor::bindings_panel::BindChannelRow;
        use crate::editor::controls_panel::ControlsFocus;
        use dtx_core::EChannel;

        let mut app = App::new();
        app.insert_resource(SelectedChannel(Some(EChannel::Snare)))
            .insert_resource(ControlsFocus::Rows)
            .add_systems(Update, highlight_selected_row);
        let sd = app
            .world_mut()
            .spawn((
                BindChannelRow(EChannel::Snare),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
            ))
            .id();
        let hh = app
            .world_mut()
            .spawn((
                BindChannelRow(EChannel::HiHatClose),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
            ))
            .id();
        app.update();
        let width = |app: &App, e| app.world().entity(e).get::<Outline>().map(|o| o.width);
        assert_eq!(width(&app, sd), Some(Val::Px(2.0)), "selected row ringed at Rows");
        assert_eq!(width(&app, hh), Some(Val::Px(0.0)));

        // Outside Rows the ring disappears (selection tint remains).
        app.insert_resource(ControlsFocus::SegmentSelector);
        app.update();
        assert_eq!(width(&app, sd), Some(Val::Px(0.0)));
    }
```

- [ ] Run `cargo test -p gameplay-drums rows_focus_draws_focus_ring` — expect **failure** (`highlight_selected_row` has no `Outline`/focus params; rows spawn without `Outline`).

### Step 4.2 — implementation

- [ ] In `bindings_panel.rs` `spawn_pads_card`, add an `Outline` to the row bundle (after `BorderColor::all(...)` in the `card.spawn((BindChannelRow(ch), ...))` tuple):

```rust
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
```

- [ ] In `bindings_capture.rs`, replace `highlight_selected_row` with:

```rust
/// Tint the selected channel row (ROW_SELECTED_BG + accent left border) so
/// the pick is visible in the list; an unselected, unbound row keeps its
/// WARN_TINT baseline instead of going transparent. While keyboard focus is
/// at the Rows level, the selected row additionally carries the FOCUS_RING
/// outline — keyboard focus reads as distinct from mere selection. Runs
/// every frame, so no repaint plumbing is needed (the spec's
/// `SelectedChannel`-into-`LeftPanelSig` fallback stays unnecessary).
fn highlight_selected_row(
    selected: Res<SelectedChannel>,
    focus: Res<super::controls_panel::ControlsFocus>,
    mut rows: Query<(
        &BindChannelRow,
        Has<super::bindings_panel::UnboundRow>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Outline,
    )>,
) {
    let rows_focused = *focus == super::controls_panel::ControlsFocus::Rows;
    for (row, unbound, mut bg, mut border, mut outline) in &mut rows {
        let on = selected.0 == Some(row.0);
        *bg = BackgroundColor(if on {
            super::chrome::ROW_SELECTED_BG
        } else if unbound {
            super::chrome::WARN_TINT
        } else {
            Color::NONE
        });
        *border = BorderColor::all(if on { super::chrome::ACCENT } else { Color::NONE });
        if on && rows_focused {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}
```

### Step 4.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums rows_focus_draws_focus_ring` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums editor::bindings_capture` — existing capture tests still pass.
- [ ] Commit: `feat(customize): focus ring on the selected Controls row at Rows level`

---

## Task 5 — Lanes visuals: LanesFocus repaint + row/detail focus rings

**Files:**
- `crates/gameplay-drums/src/editor/panel.rs` (`LeftPanelSig`, `LanesInputs`, run condition, call site)
- `crates/gameplay-drums/src/editor/lanes_panel.rs` (focus-aware spawn; pure ring predicate + test)

### Step 5.1 — failing test

- [ ] Append to `lanes_panel.rs` tests:

```rust
    #[test]
    fn lane_focus_rings_follow_focus_level() {
        // Row ring only while Rows is focused AND the row is the selection.
        assert!(lane_row_ring(LanesFocus::Rows, true));
        assert!(!lane_row_ring(LanesFocus::Rows, false));
        assert!(!lane_row_ring(LanesFocus::TabBar, true));
        assert!(!lane_row_ring(LanesFocus::Detail, true));
        // Detail-card ring (and accent width value) only at Detail.
        assert!(lane_detail_ring(LanesFocus::Detail));
        assert!(!lane_detail_ring(LanesFocus::Rows));
        assert!(!lane_detail_ring(LanesFocus::TabBar));
    }
```

- [ ] Run `cargo test -p gameplay-drums lane_focus_rings_follow` — expect **compile failure**.

### Step 5.2 — implementation

- [ ] In `lanes_panel.rs`, add above `spawn_lane_block`:

```rust
/// Row FOCUS_RING predicate: keyboard focus at Rows on the selected row.
pub(super) fn lane_row_ring(focus: LanesFocus, is_selected: bool) -> bool {
    focus == LanesFocus::Rows && is_selected
}

/// Detail-card FOCUS_RING (and accent width value) predicate.
pub(super) fn lane_detail_ring(focus: LanesFocus) -> bool {
    focus == LanesFocus::Detail
}
```

- [ ] Change `spawn_lane_block`'s signature to take the focus level and thread it through:

```rust
pub fn spawn_lane_block(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    lanes: &Lanes,
    selected: Option<usize>,
    add_popup_open: bool,
    focus: LanesFocus,
) {
```

Inside the per-row spawn, extend the row bundle (after `BorderColor::all(...)`) with:

```rust
                Outline::new(
                    if lane_row_ring(focus, is_selected) {
                        Val::Px(2.0)
                    } else {
                        Val::Px(0.0)
                    },
                    Val::Px(1.0),
                    if lane_row_ring(focus, is_selected) {
                        super::panel::FOCUS_RING
                    } else {
                        Color::NONE
                    },
                ),
```

and change the detail-card call to `spawn_lane_detail_card(p, t, lanes, i, add_popup_open, focus);`.

- [ ] Change `spawn_lane_detail_card` to accept `focus: LanesFocus` (last param). After `let body = panel_kit::spawn_card(p, &title);` add:

```rust
    if lane_detail_ring(focus) {
        p.commands_mut().entity(body).insert(Outline::new(
            Val::Px(2.0),
            Val::Px(2.0),
            super::panel::FOCUS_RING,
        ));
    }
    let width_color = if lane_detail_ring(focus) {
        chrome::ACCENT
    } else {
        t.text_primary
    };
```

and in the width readout change `TextColor(t.text_primary)` on the `LaneWidthValueText` spawn to `TextColor(width_color)`.

- [ ] In `panel.rs`:
  - `LeftPanelSig`: add field `lanes_focus: super::lanes_panel::LanesFocus,` (it derives `PartialEq + Clone`; `LanesFocus` is `Copy + PartialEq`).
  - `LanesInputs`: add `focus: Res<'w, super::lanes_panel::LanesFocus>,`.
  - In `rebuild_left_content`, populate the sig with `lanes_focus: *lanes_ui.focus,` and change the Lanes call to:

```rust
        game_shell::CustomizeTab::Lanes => super::lanes_panel::spawn_lane_block(
            p,
            &t,
            &lanes,
            lanes_ui.selected.0,
            lanes_ui.add_popup.0,
            *lanes_ui.focus,
        ),
```

  - In the `rebuild_left_content` run condition chain (line ~127), add after the `SelectedLane` line:

```rust
                        .or_else(resource_changed::<super::lanes_panel::LanesFocus>)
```

### Step 5.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums lane_focus_rings_follow` — expect **pass**.
- [ ] Run `cargo check -p gameplay-drums` — the signature change has exactly one caller (`panel.rs`); confirm no others slipped in.
- [ ] Commit: `feat(customize): LanesFocus repaint signal and lane focus rings`

---

## Task 6 — Footer hints for Controls Rows and Lanes levels

**Files:**
- `crates/gameplay-drums/src/editor/footer.rs` (`nav_hint_text` + priority slot + params; test)

### Step 6.1 — failing test

- [ ] Append to a new `tests` module in `footer.rs` (the file has none today; `capture_footer_text`'s test lives in `controls_panel.rs` and stays there):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::controls_panel::ControlsFocus;
    use crate::editor::lanes_panel::LanesFocus;
    use game_shell::CustomizeTab;

    #[test]
    fn nav_hints_cover_controls_rows_and_lanes_levels() {
        // Controls: hint only at Rows.
        assert_eq!(
            nav_hint_text(CustomizeTab::Controls, ControlsFocus::Rows, LanesFocus::TabBar),
            Some("Enter capture · Bksp remove")
        );
        assert_eq!(
            nav_hint_text(CustomizeTab::Controls, ControlsFocus::SegmentSelector, LanesFocus::TabBar),
            None
        );
        // Lanes: per-level hints.
        assert_eq!(
            nav_hint_text(CustomizeTab::Lanes, ControlsFocus::TabBar, LanesFocus::Rows),
            Some("↑↓ select · Shift+↑↓ reorder · Enter detail")
        );
        assert_eq!(
            nav_hint_text(CustomizeTab::Lanes, ControlsFocus::TabBar, LanesFocus::Detail),
            Some("←→ width · Shift ×4 · Esc back")
        );
        assert_eq!(
            nav_hint_text(CustomizeTab::Lanes, ControlsFocus::TabBar, LanesFocus::TabBar),
            None
        );
        // Other tabs: never.
        assert_eq!(
            nav_hint_text(CustomizeTab::Gameplay, ControlsFocus::Rows, LanesFocus::Detail),
            None
        );
    }
}
```

- [ ] Run `cargo test -p gameplay-drums nav_hints_cover` — expect **compile failure**.

### Step 6.2 — implementation

- [ ] In `footer.rs`, below `capture_footer_text`:

```rust
/// Key-hint line for the focused kit-tab level; None = fall through to the
/// hover description. Sits below capture text and the save-error banner in
/// the footer's priority chain.
pub fn nav_hint_text(
    tab: game_shell::CustomizeTab,
    controls: super::controls_panel::ControlsFocus,
    lanes: super::lanes_panel::LanesFocus,
) -> Option<&'static str> {
    use super::controls_panel::ControlsFocus;
    use super::lanes_panel::LanesFocus;
    match tab {
        game_shell::CustomizeTab::Controls if controls == ControlsFocus::Rows => {
            Some("Enter capture · Bksp remove")
        }
        game_shell::CustomizeTab::Lanes => match lanes {
            LanesFocus::Rows => Some("↑↓ select · Shift+↑↓ reorder · Enter detail"),
            LanesFocus::Detail => Some("←→ width · Shift ×4 · Esc back"),
            LanesFocus::TabBar => None,
        },
        _ => None,
    }
}
```

- [ ] Extend `update_footer_desc` — add three params:

```rust
    active: Res<super::tabs::ActiveTab>,
    controls_focus: Res<super::controls_panel::ControlsFocus>,
    lanes_focus: Res<super::lanes_panel::LanesFocus>,
```

change the early-out to:

```rust
    if !desc.is_changed()
        && !capture.is_changed()
        && !active.is_changed()
        && !controls_focus.is_changed()
        && !lanes_focus.is_changed()
        && err.message.is_none()
    {
        return;
    }
```

and slot the hint into the priority chain (capture > save-error > nav hint > hover desc):

```rust
    let (line, color) = if let Some(cap) = capture_footer_text(&capture) {
        (cap, theme.0.text_primary)
    } else if let Some(msg) = &err.message {
        (msg.clone(), super::chrome::ERR)
    } else if let Some(hint) = nav_hint_text(active.0, *controls_focus, *lanes_focus) {
        (hint.to_string(), theme.0.text_primary)
    } else {
        (desc_text(&desc), theme.0.text_primary)
    };
```

### Step 6.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums nav_hints_cover footer_describes_keyboard_capture` — both **pass** (the existing capture-footer test is untouched).
- [ ] Commit: `feat(customize): footer key hints for Controls rows and Lanes levels`

---

## Task 7 — Widgets Tab/Shift+Tab selection cycle + practice-Tab gate

**Files:**
- `crates/gameplay-drums/src/editor/drag.rs` (`cycle_widget`, tap-vs-hold system, registration, tests)
- `crates/gameplay-drums/src/practice/mod.rs` (`editor_closed` gate + regression test)

### Step 7.1 — failing tests

- [ ] Append to `drag.rs` tests:

```rust
    #[test]
    fn tab_cycle_walks_sidebar_order_wraps_and_reverses() {
        let all = WidgetKind::ALL;
        // None starts at the first list entry (both directions).
        assert_eq!(cycle_widget(None, false), all[0]);
        assert_eq!(cycle_widget(None, true), all[0]);
        // Forward walk + wrap.
        assert_eq!(cycle_widget(Some(all[0]), false), all[1]);
        assert_eq!(cycle_widget(Some(all[all.len() - 1]), false), all[0]);
        // Reverse walk + wrap.
        assert_eq!(cycle_widget(Some(all[1]), true), all[0]);
        assert_eq!(cycle_widget(Some(all[0]), true), all[all.len() - 1]);
    }
```

- [ ] Append to `crates/gameplay-drums/src/practice/mod.rs` tests:

```rust
    #[test]
    fn practice_actions_are_dead_while_editor_is_open() {
        use bevy::prelude::*;
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<actions::PracticeBindings>()
            .insert_resource(crate::editor::EditorOpen(true))
            .add_message::<actions::PracticeAction>()
            .add_systems(
                Update,
                actions::emit_practice_actions.run_if(crate::editor::editor_closed),
            );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<actions::PracticeAction>>()
                .is_empty(),
            "editor open must gate practice actions (Tab = OpenFullHud)"
        );

        // Editor closed: the same press emits again.
        app.world_mut().resource_mut::<crate::editor::EditorOpen>().0 = false;
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        assert!(!app
            .world()
            .resource::<Messages<actions::PracticeAction>>()
            .is_empty());
    }
```

- [ ] Run `cargo test -p gameplay-drums tab_cycle_walks practice_actions_are_dead` — expect **compile failure** (`cycle_widget` missing) and the practice test to be the gate's living documentation.

### Step 7.2 — implementation

- [ ] In `drag.rs`, below `nudge_selected_widget`:

```rust
/// A Tab press released within this window is a "tap" (cycle selection);
/// anything longer was the existing hold-to-peek (`update_preview_state`)
/// and must not move the selection.
const TAB_TAP_MAX_SECS: f32 = 0.25;

/// Next/previous widget in the sidebar list order (`WidgetKind::ALL` — the
/// exact order `panel::spawn_widget_list` renders). Wraps; `None` starts at
/// the first entry.
pub fn cycle_widget(current: Option<WidgetKind>, reverse: bool) -> WidgetKind {
    let all = WidgetKind::ALL;
    match current.and_then(|k| all.iter().position(|x| *x == k)) {
        None => all[0],
        Some(i) if reverse => all[(i + all.len() - 1) % all.len()],
        Some(i) => all[(i + 1) % all.len()],
    }
}

/// Tab-tap cycles the widget selection (Shift+Tab reverses); a held Tab
/// stays the play-view peek. Shift is sampled at press time so releasing it
/// mid-tap still reverses.
fn cycle_widget_selection(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut pressed: Local<Option<(f32, bool)>>,
    mut selection: ResMut<Selection>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        *pressed = Some((time.elapsed_secs(), shift));
    }
    if keys.just_released(KeyCode::Tab) {
        if let Some((at, shift)) = pressed.take() {
            if time.elapsed_secs() - at <= TAB_TAP_MAX_SECS {
                selection.0 = Some(cycle_widget(selection.0, shift));
            }
        }
    }
}
```

- [ ] In `drag.rs` `plugin`, add a second `add_systems` call (the existing gesture chain is untouched — nudge stays exactly as is):

```rust
    app.add_systems(
        Update,
        cycle_widget_selection
            .run_if(super::editor_open)
            .run_if(super::widgets_tab_active)
            .run_if(super::profile_dialog::profile_dialog_closed)
            .run_if(super::profile_state::pending_close_none)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
```

- [ ] In `crates/gameplay-drums/src/practice/mod.rs`, add the missing gate to the actions chain (after `.run_if(resource_exists::<PracticeSession>)`):

```rust
                .run_if(crate::editor::editor_closed),
```

### Step 7.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums tab_cycle_walks practice_actions_are_dead` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums --test practice_mode` — the integration harness (which registers the systems itself, ungated) still passes.
- [ ] Commit: `feat(customize): Tab cycles widget selection; gate practice actions while editor open`

---

## Task 8 — Close dialog keyboard traversal

**Files:**
- `crates/gameplay-drums/src/editor/close_dialog.rs` (focus resource, `step_focus`, key system, focus ring, tests)

### Step 8.1 — failing tests

- [ ] Append to `close_dialog.rs` tests:

```rust
    #[test]
    fn step_focus_clamps_at_both_ends() {
        assert_eq!(step_focus(2, 3, true, false), 1);
        assert_eq!(step_focus(0, 3, true, false), 0, "clamps left");
        assert_eq!(step_focus(2, 3, false, true), 2, "clamps right");
        assert_eq!(step_focus(9, 3, false, false), 2, "stale index clamps into range");
        assert_eq!(step_focus(0, 0, true, true), 0, "empty row is inert");
    }

    #[test]
    fn arrows_move_focus_and_enter_dispatches_focused_decision() {
        use crate::editor::profile_state::{CloseIntent, PendingClose, ProfileKind};

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CloseDialogFocus>()
            .insert_resource(PendingCloseState::Pending(PendingClose {
                intent: CloseIntent::Customize,
                dirty: vec![ProfileKind::Midi],
            }))
            .init_resource::<CustomizeSession>()
            .init_resource::<dtx_ui::ThemeResource>()
            .add_message::<CloseDecisionRequest>()
            .add_systems(Update, (sync_dialog, close_dialog_keys).chain());

        app.update(); // armed frame: sync sets focus, keys system skips it
        assert_eq!(
            app.world().resource::<CloseDialogFocus>().0,
            2,
            "initial focus = layout default (Save all), never the destructive button"
        );

        // ← moves to Discard; Enter dispatches THAT decision.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowLeft);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        assert_eq!(app.world().resource::<CloseDialogFocus>().0, 1);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        let requests: Vec<_> = app
            .world()
            .resource::<Messages<CloseDecisionRequest>>()
            .iter_current_update_messages()
            .map(|request| request.0)
            .collect();
        assert_eq!(requests, vec![CloseDecision::DiscardAll]);
    }
```

- [ ] Run `cargo test -p gameplay-drums step_focus_clamps arrows_move_focus_and_enter` — expect **compile failure**.

### Step 8.2 — implementation

- [ ] In `close_dialog.rs`, add after the existing component definitions:

```rust
/// Keyboard-focused button index in the close dialog's row
/// (Cancel | Discard all | Save all). Reset to the layout's default focus
/// whenever the guard is (re)armed. Esc stays with `resolve_pending_close`'s
/// existing `close_decision_for_key` fallback (Cancel) — never double-handled
/// here.
#[derive(Resource, Default)]
pub struct CloseDialogFocus(pub usize);

#[derive(Component, Clone, Copy)]
struct CloseDialogBtnIndex(usize);

const CLOSE_DECISIONS: [CloseDecision; 3] = [
    CloseDecision::Cancel,
    CloseDecision::DiscardAll,
    CloseDecision::SaveAll,
];

/// Clamp-move a dialog-row focus index (shared with profile_dialog_ui).
pub(super) fn step_focus(focus: usize, len: usize, left: bool, right: bool) -> usize {
    if len == 0 {
        return 0;
    }
    let mut next = focus.min(len - 1);
    if left {
        next = next.saturating_sub(1);
    }
    if right {
        next = (next + 1).min(len - 1);
    }
    next
}
```

- [ ] `sync_dialog`: add param `mut focus: ResMut<CloseDialogFocus>,` and right after `let layout = dirty_dialog_layout(...)`:

```rust
    focus.0 = layout.default_focus;
```

Replace the local `decisions` array with `CLOSE_DECISIONS` (delete the shadowing `let decisions = [...]` and use `.zip(CLOSE_DECISIONS)`), and extend the per-button bundle with:

```rust
                                    CloseDialogBtnIndex(index),
                                    Outline::new(Val::Px(0.0), Val::Px(2.0), Color::NONE),
```

- [ ] Add the two new systems:

```rust
/// ←/→ move the focused button (clamped); Enter activates it — the same
/// `CloseDecisionRequest` a click sends, resolved by `resolve_pending_close`
/// (whose Enter→SaveAll fallback only fires when no request arrived, so the
/// focused decision wins).
fn close_dialog_keys(
    keys: Res<ButtonInput<KeyCode>>,
    pending: Res<PendingCloseState>,
    mut focus: ResMut<CloseDialogFocus>,
    mut requests: MessageWriter<CloseDecisionRequest>,
) {
    if !matches!(*pending, PendingCloseState::Pending(_)) || pending.is_changed() {
        // Closed, or armed this frame (the arming Esc/Enter must not resolve it).
        return;
    }
    let next = step_focus(
        focus.0,
        CLOSE_DECISIONS.len(),
        keys.just_pressed(KeyCode::ArrowLeft),
        keys.just_pressed(KeyCode::ArrowRight),
    );
    if next != focus.0 {
        focus.0 = next;
    }
    if keys.just_pressed(KeyCode::Enter) {
        requests.write(CloseDecisionRequest(CLOSE_DECISIONS[focus.0]));
    }
}

/// FOCUS_RING outline on the focused button, in addition to the existing
/// default/destructive coloring. Hover never moves keyboard focus.
fn update_close_dialog_focus_ring(
    focus: Res<CloseDialogFocus>,
    mut buttons: Query<(&CloseDialogBtnIndex, &mut Outline)>,
) {
    for (index, mut outline) in &mut buttons {
        if index.0 == focus.0 {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}
```

- [ ] Register in `plugin` — add `init_resource` and the systems:

```rust
    app.add_message::<CloseDecisionRequest>()
        .init_resource::<CloseDialogFocus>()
        .add_systems(
            Update,
            (
                sync_dialog.run_if(resource_changed::<PendingCloseState>),
                close_dialog_keys.before(super::resolve_pending_close),
                handle_buttons.before(super::resolve_pending_close),
                update_close_dialog_focus_ring,
            )
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(OnExit(game_shell::AppState::Performance), despawn_dialog);
```

### Step 8.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums step_focus_clamps arrows_move_focus_and_enter pending_midi_close_spawns` — all **pass** (existing spawn test included).
- [ ] Commit: `feat(customize): keyboard traversal for the dirty-close dialog`

---

## Task 9 — Profile dialog keyboard traversal (Dirty / ConfirmDelete / CorruptReset)

**Files:**
- `crates/gameplay-drums/src/editor/profile_dialog_ui.rs` (focus resource, button maps, dispatch extraction, key system, focus ring, tests)

### Step 9.1 — failing tests

- [ ] Append a `tests` module to `profile_dialog_ui.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::profile_state::PendingProfileAction;

    fn dirty_state() -> ProfileDialogState {
        ProfileDialogState::Dirty {
            kind: ProfileKind::Keyboard,
            pending: PendingProfileAction::Select("Desk".to_owned()),
            builtin_selected: false,
        }
    }

    #[test]
    fn dialog_buttons_put_safe_dismiss_first_and_focus_never_destructive() {
        // ConfirmDelete: [Cancel, Delete] — initial focus on Cancel.
        let confirm = ProfileDialogState::ConfirmDelete { name: "Desk".to_owned() };
        assert!(matches!(dialog_buttons(&confirm)[0], DialogButton::CancelDelete));
        assert_eq!(dialog_buttons(&confirm).len(), 2);
        assert_eq!(initial_focus(&confirm), 0);

        // Dirty: [Cancel, Discard, Save] — initial focus = layout default
        // (Save), which is asserted never-destructive by profile_state tests.
        let dirty = dirty_state();
        let buttons = dialog_buttons(&dirty);
        assert_eq!(buttons.len(), 3);
        assert!(matches!(buttons[0], DialogButton::Dirty(CloseDecision::Cancel)));
        let layout = profile_state::dirty_dialog_layout(&[ProfileKind::Keyboard], false);
        assert_eq!(initial_focus(&dirty), layout.default_focus);
        assert_ne!(initial_focus(&dirty), layout.destructive);

        // CorruptReset: [Cancel, Back up & reset] — initial focus on Cancel.
        let corrupt = ProfileDialogState::CorruptReset {
            kind: ProfileKind::Midi,
            message: "corrupt".to_owned(),
        };
        assert!(matches!(dialog_buttons(&corrupt)[0], DialogButton::CorruptCancel));
        assert_eq!(initial_focus(&corrupt), 0);

        // Name / Closed have no traversable row (Name owns its own keys).
        assert!(dialog_buttons(&ProfileDialogState::Closed).is_empty());
    }

    #[test]
    fn escape_dismisses_confirm_delete_without_deleting() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<profile_bar_ui::DialogKind>()
            .init_resource::<ProfileDialogFocus>()
            .init_resource::<CustomizeSession>()
            .init_resource::<LaneProfileDraft>()
            .init_resource::<LiveBindings>()
            .init_resource::<super::super::bindings_panel::BindingsRev>()
            .init_resource::<ProfileUiErrorState>()
            .insert_resource(ProfileDialogState::ConfirmDelete { name: "Desk".to_owned() })
            .add_systems(Update, handle_dialog_keys);
        app.update(); // opening frame: is_changed skip
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(
            *app.world().resource::<ProfileDialogState>(),
            ProfileDialogState::Closed,
            "Esc = safe dismiss (CancelDelete path — no registry write)"
        );
    }
}
```

- [ ] Run `cargo test -p gameplay-drums dialog_buttons_put_safe escape_dismisses_confirm_delete` — expect **compile failure**.

### Step 9.2 — implementation

- [ ] In `profile_dialog_ui.rs`, add after the `DialogButton` enum:

```rust
/// Keyboard-focused button index for the current button-row dialog
/// (ConfirmDelete / Dirty / CorruptReset). The Name dialog keeps its own
/// text-entry key handling and ignores this.
#[derive(Resource, Default)]
pub struct ProfileDialogFocus(pub usize);

#[derive(Component, Clone, Copy)]
struct DialogBtnIndex(usize);

/// Button row (left→right, matching spawn order) for the current dialog.
/// Index 0 is ALWAYS the safe dismiss. Empty for Closed/Name.
fn dialog_buttons(state: &ProfileDialogState) -> Vec<DialogButton> {
    match state {
        ProfileDialogState::ConfirmDelete { .. } => {
            vec![DialogButton::CancelDelete, DialogButton::ConfirmDelete]
        }
        ProfileDialogState::Dirty { .. } => vec![
            DialogButton::Dirty(CloseDecision::Cancel),
            DialogButton::Dirty(CloseDecision::DiscardAll),
            DialogButton::Dirty(CloseDecision::SaveAll),
        ],
        ProfileDialogState::CorruptReset { .. } => {
            vec![DialogButton::CorruptCancel, DialogButton::CorruptConfirm]
        }
        ProfileDialogState::Closed | ProfileDialogState::Name { .. } => Vec::new(),
    }
}

/// Initial keyboard focus per dialog: the dirty guard uses its layout's
/// default (Save — the layout guarantees it is never the destructive
/// button); everything else starts on the safe dismiss.
fn initial_focus(state: &ProfileDialogState) -> usize {
    match state {
        ProfileDialogState::Dirty {
            kind,
            builtin_selected,
            ..
        } => profile_state::dirty_dialog_layout(&[*kind], *builtin_selected).default_focus,
        _ => 0,
    }
}
```

- [ ] `spawn_dialog_btn`: add an `index: usize` parameter (after `button`) and extend the bundle with `DialogBtnIndex(index),` and `Outline::new(Val::Px(0.0), Val::Px(2.0), Color::NONE),`. Update every call site with its left-to-right ordinal, matching `dialog_buttons` exactly:
  - Name dialog: `NameCancel` → 0, `NameOk` → 1 (row untraversed; indices only feed the ring system, which ignores Name).
  - ConfirmDelete: `CancelDelete` → 0, `ConfirmDelete` → 1.
  - Dirty loop: pass the existing `index` from `enumerate()`.
  - CorruptReset: `CorruptCancel` → 0, `CorruptConfirm` → 1.
- [ ] `sync_dialog`: add param `mut focus: ResMut<ProfileDialogFocus>,` and as the first statement after the despawn loop:

```rust
    focus.0 = initial_focus(&dialog);
```

- [ ] Extract the body of `handle_dialog_buttons`'s `match` into a shared dispatcher (mouse and keyboard MUST go through the same path):

```rust
/// Apply one activated dialog button — shared by mouse clicks and the
/// keyboard's Enter/Esc so both dispatch identically.
#[allow(clippy::too_many_arguments)]
fn dispatch_dialog_button(
    pressed: DialogButton,
    dialog_kind: Option<ProfileKind>,
    dialog: &mut ProfileDialogState,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
    live: &mut LiveBindings,
    rev: &mut super::bindings_panel::BindingsRev,
    error: &mut ProfileUiErrorState,
) {
    let snapshot = dialog.clone();
    match (&snapshot, pressed) {
        (ProfileDialogState::ConfirmDelete { .. }, DialogButton::ConfirmDelete) => {
            let Some(kind) = dialog_kind else {
                *dialog = ProfileDialogState::Closed;
                return;
            };
            match profile_bar_ui::delete_kind(kind, session, lane_draft) {
                Ok(()) => {
                    error.0 = None;
                    profile_bar_ui::refresh_live_bindings(kind, session, live, rev);
                }
                Err(message) => error.0 = Some(profile_bar_ui::ui_error(kind, message)),
            }
            *dialog = ProfileDialogState::Closed;
        }
        (ProfileDialogState::ConfirmDelete { .. }, DialogButton::CancelDelete) => {
            *dialog = ProfileDialogState::Closed;
        }
        (
            ProfileDialogState::Dirty {
                kind,
                pending,
                builtin_selected,
            },
            DialogButton::Dirty(decision),
        ) => {
            let (kind, builtin_selected) = (*kind, *builtin_selected);
            match profile_bar_ui::resolve_dirty(
                kind,
                pending,
                builtin_selected,
                decision,
                session,
                lane_draft,
            ) {
                Ok(needs_refresh) => {
                    error.0 = None;
                    if needs_refresh {
                        profile_bar_ui::refresh_live_bindings(kind, session, live, rev);
                    }
                }
                Err(message) => error.0 = Some(profile_bar_ui::ui_error(kind, message)),
            }
            *dialog = ProfileDialogState::Closed;
        }
        (ProfileDialogState::CorruptReset { kind, .. }, DialogButton::CorruptConfirm) => {
            let result = backup_and_reset(*kind);
            *dialog = profile_dialog::apply_reset_outcome(*kind, result);
        }
        (ProfileDialogState::CorruptReset { .. }, DialogButton::CorruptCancel) => {
            *dialog = ProfileDialogState::Closed;
        }
        _ => {}
    }
}
```

and shrink `handle_dialog_buttons` to:

```rust
#[allow(clippy::too_many_arguments)]
fn handle_dialog_buttons(
    buttons: Query<(&Interaction, &DialogButton), Changed<Interaction>>,
    dialog_kind: Res<DialogKind>,
    mut dialog: ResMut<ProfileDialogState>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut error: ResMut<ProfileUiErrorState>,
) {
    let Some(pressed) = buttons
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, button)| *button)
    else {
        return;
    };
    dispatch_dialog_button(
        pressed,
        dialog_kind.0,
        &mut dialog,
        &mut session,
        &mut lane_draft,
        &mut live,
        &mut rev,
        &mut error,
    );
}
```

- [ ] Add the keyboard system + focus ring:

```rust
/// ←/→/Enter/Esc for the button-row dialogs. Esc always activates the safe
/// dismiss (row index 0); Enter activates the focused button through the
/// exact same dispatcher as a click. Skips its opening frame (the keypress
/// that raised the dialog must not immediately act on it) and the Name
/// dialog (own key handling).
#[allow(clippy::too_many_arguments)]
fn handle_dialog_keys(
    keys: Res<ButtonInput<KeyCode>>,
    dialog_kind: Res<DialogKind>,
    mut focus: ResMut<ProfileDialogFocus>,
    mut dialog: ResMut<ProfileDialogState>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut error: ResMut<ProfileUiErrorState>,
) {
    let buttons = dialog_buttons(&dialog);
    if buttons.is_empty() || dialog.is_changed() {
        return;
    }
    let next = super::close_dialog::step_focus(
        focus.0,
        buttons.len(),
        keys.just_pressed(KeyCode::ArrowLeft),
        keys.just_pressed(KeyCode::ArrowRight),
    );
    if next != focus.0 {
        focus.0 = next;
    }
    let pressed = if keys.just_pressed(KeyCode::Escape) {
        Some(buttons[0])
    } else if keys.just_pressed(KeyCode::Enter) {
        buttons.get(focus.0.min(buttons.len() - 1)).copied()
    } else {
        None
    };
    if let Some(pressed) = pressed {
        dispatch_dialog_button(
            pressed,
            dialog_kind.0,
            &mut dialog,
            &mut session,
            &mut lane_draft,
            &mut live,
            &mut rev,
            &mut error,
        );
    }
}

/// FOCUS_RING on the focused button; cleared entirely for Name/Closed.
fn update_profile_dialog_focus_ring(
    dialog: Res<ProfileDialogState>,
    focus: Res<ProfileDialogFocus>,
    mut buttons: Query<(&DialogBtnIndex, &mut Outline)>,
) {
    let focusable = !dialog_buttons(&dialog).is_empty();
    for (index, mut outline) in &mut buttons {
        if focusable && index.0 == focus.0 {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}
```

- [ ] Register in `plugin`:

```rust
    app.init_resource::<ProfileDialogFocus>().add_systems(
        Update,
        (
            sync_dialog.run_if(resource_changed::<ProfileDialogState>),
            handle_name_dialog_input,
            handle_dialog_keys,
            handle_dialog_buttons,
            update_profile_dialog_focus_ring,
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_dialog);
```

### Step 9.3 — verify & commit

- [ ] Run `cargo test -p gameplay-drums dialog_buttons_put_safe escape_dismisses_confirm_delete` — expect **pass**.
- [ ] Run `cargo test -p gameplay-drums editor::profile` — all existing profile_state / profile_dialog tests (incl. `discard_never_has_default_focus`) still pass.
- [ ] Commit: `feat(customize): keyboard traversal for profile dialogs`

---

## Task 10 — Full gates and contract sweep

**Files:** none (verification only)

- [ ] `cargo check --workspace` — clean.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean (watch for `too_many_arguments` on the new consumers — the `#[allow]`s above cover the known ones).
- [ ] `cargo test -p gameplay-drums` — full crate green. Explicitly confirm in the output:
  - `pad_exclusion_matches_controls_contract` — **unchanged and green** (pads still excluded from Controls; nothing in this plan touches `pad_excluded` or any pad path).
  - All `reduce_controls_nav` tests (`controls_segment_left_right_switches`, `controls_down_enters_segment_then_rows`, `controls_up_returns_one_level`) — untouched, green.
  - All `reduce_lanes_nav` tests (`lanes_tabbar_confirm_enters_rows` … `lanes_detail_back_returns_to_rows_keeping_selection`) — untouched, green.
- [ ] `cargo test --workspace` — no cross-crate fallout (game-shell nav types untouched).
- [ ] Commit only if gate runs produced changes (they shouldn't): otherwise nothing to commit.

---

## Acceptance criteria → task map

| # | Criterion (spec) | Tasks |
|---|---|---|
| 1 | Controls tab fully keyboard-operable: segment toggle, row selection, capture start, binding delete | 1 (rail guard), 2 (consumer), 4 (visible focus), 6 (hints) |
| 2 | Lanes tab fully keyboard-operable: select, reorder (Shift+↑↓), width (Detail ←/→), correct undo granularity | 1 (init resource), 3 (consumer + undo tests), 5 (visuals), 6 (hints) |
| 3 | Pads behave exactly as before on every tab (contract test green) | 2/10 (no pad-path change; `NavSource::Keyboard` filter in both consumers; contract test verified in Task 10) |
| 4 | Widgets: Tab/Shift+Tab cycle selection; nudge unchanged | 7 (cycle system added beside an untouched `nudge_selected_widget`) |
| 5 | All four button dialogs: ←/→/Enter/Esc; initial focus safe; destructive never default | 8 (close dialog), 9 (Dirty/ConfirmDelete/CorruptReset), plus existing `discard_never_has_default_focus` |
| 6 | Full gates: workspace check + clippy `-D warnings` + `cargo test -p gameplay-drums` green | 10 |

Spec error-handling items: empty lane list (reducer no-op, already tested), channel with no bindings → Backspace no-op (`last_segment_source_index` → `None`, Task 2), stale `SelectedChannel` → clamp to first on next Up/Down (`step_channel`, Task 2).

---

## Verification

**Gates (Task 10, run by the worker):**

```
cargo check --workspace \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test -p gameplay-drums
```

**Runtime smoke (by the controller, via BRP — see memory note `brp-smoke-driving.md`; `move_mouse` before `click_mouse` is mandatory):**

1. Launch, open Customize → Controls. ↓ into segment selector (ring on selector), ←/→ toggles Keyboard/MIDI **without switching tabs**, ↓ into rows → red ring + selection tint on first row, footer shows `Enter capture · Bksp remove`. Enter → capture modal appears; Esc cancels (surface stays open). Backspace on a bound row removes one chip (verify chip count drops).
2. Lanes tab: ↓ enters rows (ring on selected row), Shift+↓ reorders (preview lane moves, selection follows), Enter → detail card ringed + width value accented, footer shows `←→ width · Shift ×4 · Esc back`, ←/→ change the width readout, Esc returns to rows (surface stays open), second Esc from tab bar closes. Ctrl+Z after a multi-press width session undoes it in ONE step; each Shift+↓ press undoes individually.
3. Widgets tab: Tab tap cycles the selection ring through the sidebar order, Shift+Tab reverses, holding Tab still peeks without moving the selection; arrow nudge unchanged.
4. Dirty a profile, press Esc → close dialog: ring starts on Save, ←/→ move it, Enter on Discard discards, Esc cancels. Repeat for a profile-bar delete (ConfirmDelete: focus starts on Cancel; Esc dismisses without deleting).
5. Regression: with a real/virtual pad, hit pads on the Controls tab — focus must not move (pad exclusion contract).
