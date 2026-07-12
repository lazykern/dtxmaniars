# Editor Nav Wiring (Controls / Lanes / Widgets / Dialogs) — Design

Stream 3 of the post-audit UX work. Wires the existing-but-unused nav reducers
(`reduce_controls_nav`, `reduce_lanes_nav`) into production, adds a keyboard
selection cycle to the Widgets tab, and gives the mouse-only dialogs keyboard
traversal. Fixes audit findings F18 (reducers unwired) and F14 (Discard dialog
mouse-only). Research base: `docs/notes/2026-07-12-streams-research.md`
§Stream 3.

## Goals

- Keyboard can fully operate the Controls tab (segment, rows, capture,
  delete) and Lanes tab (select, reorder, width).
- Keyboard can cycle widget selection on the Widgets tab.
- Button dialogs respond to arrows / Enter / Esc.

## Non-goals

- **Pads stay exactly as today**: excluded from Controls (stray hits while
  testing bindings must not move focus — the existing
  `pad_exclusion_matches_controls_contract` test stays valid), from Widgets,
  and from Lanes. Pad nav on settings tabs unchanged.
- Device-card steppers (MIDI port cycle, velocity threshold) stay mouse-only.
- Inspector (Widgets right panel) value editing stays mouse-only.
- No reducer API changes beyond what's listed; their existing tests keep
  passing.

## Part A — Controls tab keyboard nav

New system `controls_nav_consumer` in `controls_panel.rs`, registered in the
editor plugin with the same gates as `keyboard_nav`
(`AppState::Performance` + `editor_open` + `profile_dialog_closed`), plus:
acts only when `active.0 == EditorTab::Controls`, and only on
`NavAction { source: NavSource::Keyboard, .. }`. It owns its own
`MessageReader<NavAction>` (parallel readers each get their own copy; the
generic `settings_nav_consumer` already no-ops on Controls).

Level transitions go through the existing pure reducer:

```rust
let (focus2, segment2) = reduce_controls_nav(*focus, *segment, action.verb);
```

At `ControlsFocus::Rows`, the consumer additionally owns what the reducer
doesn't model:

- **Up/Down**: step `bindings_capture::SelectedChannel` through the panel's
  display-order channel list (the same ordering `spawn_pads_card` renders;
  exposed as a pure `display_channels() -> &'static [EChannel]` if not already
  one). Up from the first channel returns focus to `SegmentSelector` (reducer
  handles this via its Rows+Up transition — the driver only steps the channel
  when not at the top).
- **Confirm (Enter)**: start capture for the selected channel in the active
  segment — set `CaptureState::Keyboard(ch)` or `::Midi(ch)` exactly like
  `handle_capture_start`. The capture modal already has full keyboard support
  (Enter/←→/Esc via the shared `arrived_step` reducer) — nothing to add there.
- **Backspace**: delete the **last** source bound to the selected channel in
  the active segment (same mutation as `handle_bind_chip_remove` with
  `index = len - 1`), bump `BindingsRev`. No-op when the channel has no
  bindings in that segment.

While a capture is active (`CaptureState != Idle`), the consumer does nothing
(the existing capture flow owns input).

### Visuals

- `ControlsFocus` is already in `LeftPanelSig` — segment-selector focus
  repaints free.
- Rows level: the selected channel's row already highlights via
  `SelectedChannel` (`bindings_spatial` + row styling). Add a `FOCUS_RING`
  outline on the selected `BindChannelRow` only while
  `ControlsFocus::Rows` is active, so keyboard focus is visible as distinct
  from mere selection. (`SelectedChannel` changes already reach the panel via
  `BindingsRev`-independent row systems; if repaint proves stale, add
  `SelectedChannel` to `LeftPanelSig` — decide in implementation, both
  acceptable.)
- Footer hint line for the Controls tab gains
  `Enter capture · Bksp remove` when focus is at Rows.

## Part B — Lanes tab keyboard nav

- `init_resource::<LanesFocus>()` in the editor plugin (currently never
  registered).
- New system `lanes_nav_consumer` in `lanes_panel.rs`, same gates, acts only
  when `active.0 == EditorTab::Lanes`, keyboard source only.

Core loop:

```rust
let (focus2, selected2, effect) =
    reduce_lanes_nav(*focus, selected_idx, lane_count, action.verb, action.coarse);
```

- `selected_idx` bridges `SelectedLane(Option<usize>)`: `None` maps to 0 on
  entering Rows; the consumer writes back `Some(selected2)` while focus is
  Rows/Detail.
- **`LanesNavEffect::Reorder { index, dir }`**: push one undo snapshot
  (`UndoStack::push(&layouts, &lanes)`) **per reorder keypress**, then apply
  the same adjacent-swap walk `lane_drag` uses (`move_lane_to`
  index → index+dir). Selection follows the moved lane (reducer already
  returns the new index).
- **`LanesNavEffect::AdjustWidth { index, dir }`**: undo snapshot **once per
  Detail visit** (drag's `pushed`-flag pattern: a `Local<bool>` armed on
  entering Detail, snapshot taken before the first adjust, reset on leaving
  Detail), then `set_lane_width(index, width + dir * WIDTH_STEP)` clamped to
  `[MIN_LANE_WIDTH, MAX_LANE_WIDTH]`. `WIDTH_STEP = 4.0` ref-px (coarse: ×4).
- Existing live-mirror systems (`mirror_lane_edits_to_draft`,
  `apply_lane_draft_preview`, `refresh_lane_panel_values`) pick up the
  mutations unchanged.

### Visuals

- Add `lanes_focus: LanesFocus` to `LeftPanelSig` and a
  `resource_changed::<LanesFocus>` run condition on `rebuild_left_content`
  (mirroring the `ControlsFocus` wiring at `panel.rs:127`).
- Rows level: `FOCUS_RING` outline on the selected `LaneRow`.
- Detail level: `FOCUS_RING` outline on the detail card; width value gets
  accent color.
- Footer hint per level: Rows → `↑↓ select · Shift+↑↓ reorder · Enter detail`;
  Detail → `←→ width · Shift ×4 · Esc back`.

## Part C — Widgets tab selection cycle

Small direct-KeyCode system in `drag.rs` beside `nudge_selected_widget`, gated
`widgets_tab_active` (+ editor gates):

- **Tab**: `Selection` advances to the next widget in the visible widget list
  (same ordering the sidebar list renders — the visible `WidgetKind`s for the
  current layout); **Shift+Tab**: previous. Wraps. `Selection == None` starts
  at the first.
- Existing arrow nudge unchanged.
- Conflict check (verify during implementation): Tab is bound to
  `OpenFullHud` in practice, but that system is gated `PauseState::Running` +
  practice, while the editor implies the customize surface is open
  (`editor_open`) — `apply_practice_actions` is additionally gated
  `editor_closed`; confirm and add a regression test if the gate is missing.

## Part D — Dialog keyboard traversal

Applies to the button-row dialogs: `close_dialog.rs`
(Cancel / Discard all / Save all) and `profile_dialog_ui.rs` variants
`Dirty`, `ConfirmDelete`, `CorruptReset`. (Name dialog already
keyboard-complete; capture modal already keyboard-complete.)

Shared shape (small per-file systems; a shared helper only if it falls out
naturally — two files, resist premature abstraction):

- A focused-index resource/component per dialog (`DialogFocus(usize)`),
  initialized to the dialog's safe default (`layout.default_focus` in
  close_dialog; the non-destructive button in profile dialogs).
- **←/→** move focus across the button row (clamped).
- **Enter** activates the focused button (same effect as clicking it).
- **Esc** activates Cancel/close (the safe dismiss), everywhere.
- Focused button: `FOCUS_RING`-style outline (or accent border) in addition
  to the existing default/destructive coloring. Destructive buttons are never
  the initial focus.
- Mouse behavior unchanged; hover does not move keyboard focus.

While any of these dialogs is open, `keyboard_nav` is already suppressed via
`profile_dialog_closed` for profile dialogs; `close_dialog` must equally
suppress the underlying panel nav — verify the existing gate covers it (the
close dialog appears during editor exit flow; if a gap exists, add its
open-state to the same run-condition chain).

## Error handling

- Empty lane list: reducer already no-ops TabBar→Rows (tested).
- Channel with no bindings: Backspace no-ops.
- `SelectedChannel` pointing at a channel not in display order (stale):
  clamp to first on next Up/Down.

## Testing

Unit:

- Controls driver pure parts: channel stepping order (top → SegmentSelector
  handoff), Backspace target resolution (last index, none → no-op).
- Lanes driver: reorder undo pushed per keypress; width undo once per Detail
  visit (Local-flag logic extracted pure or tested via `run_system_once`
  sequences); WIDTH_STEP clamping at both bounds.
- Widgets: cycle order + wrap + Shift reverse (pure helper over the visible
  list).
- Dialogs: focus clamp, Enter dispatch mapping, Esc = safe dismiss, initial
  focus never destructive.
- All existing reducer tests and the pad-exclusion contract test unchanged
  and green.

Runtime (BRP smoke): open Customize → Controls: arrow into rows, Enter →
capture modal appears, Esc cancels, Backspace removes a binding (verify chip
count). Lanes: arrow into rows, Shift+Down reorders (verify preview), Enter →
detail, ←/→ width change, Ctrl+Z undoes once per visit. Widgets: Tab cycles
selection ring. Trigger close dialog with dirty state: ←/→ + Enter chooses,
Esc cancels.

## Acceptance criteria

1. Controls tab fully keyboard-operable: segment toggle, row selection,
   capture start, binding delete — without touching the mouse.
2. Lanes tab fully keyboard-operable: select, reorder (Shift+↑↓), width
   (Detail ←/→), with correct undo granularity (per reorder press; once per
   Detail visit for width).
3. Pads behave exactly as before on every tab (contract test green).
4. Widgets: Tab/Shift+Tab cycle selection; nudge unchanged.
5. All four button dialogs: ←/→/Enter/Esc work; initial focus safe;
   destructive never default.
6. Full gates: workspace check + clippy `-D warnings` + `cargo test -p
   gameplay-drums` green.
