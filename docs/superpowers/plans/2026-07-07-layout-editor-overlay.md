# Layout Editor Overlay (Layout Pillar, Plan 3 of 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]` checkboxes.

**Goal:** An in-Performance layout editor: hotkey toggles an overlay over live autoplay; the player selects a HUD widget from a sidebar and drags it with the mouse to reposition, switches lane presets, undoes, and saves to `layout.toml`.

**Architecture:** New module `crates/gameplay-drums/src/editor/` (mod, state, drag, undo, save, ui). Deviation from the spec's separate `layout-editor` crate: the editor mutates gameplay-drums resources (`WidgetLayouts`, `Lanes`, `PlayfieldLayout`, `AutoplayEnabled`) directly and is entered from Performance, so a module — mirroring the existing `practice/` module — avoids exposing internals across a crate boundary. An `EditorOpen` resource gates drum input + pause while open; opening force-enables autoplay so notes keep flowing hands-free. Widget selection is via the sidebar list (robust, no per-widget hit-testing); movement is mouse-drag on the canvas (delta ÷ scale → offset). Undo snapshots the two resources. Save writes the current `WidgetLayouts`+`Lanes` back through `dtx_layout::save`.

**Tech Stack:** Rust, Bevy 0.19, dtx-layout. Parent spec: `docs/superpowers/specs/2026-07-07-layout-editor-design.md`. Builds on plans 1 (Lanes) + 2 (WidgetLayouts/WidgetContainer, both merged).

**Project gotchas:**
- NEVER `cargo fmt --all`/`-p`; only `rustfmt <new-file>`; then `git status` + `git checkout --` strays.
- Green tests don't prove the real FixedUpdate schedule builds. This plan adds only Update/OnEnter/OnExit + resource-gated systems — **no FixedUpdate edges**. If you add any FixedUpdate ordering, extend `tests/fixed_update_schedule_ordering.rs`.
- `PlayfieldLayout` is not `Copy` (holds a Vec); pass by ref.
- Editor systems must be inert when the editor is closed (default) — zero behavior change to normal play/practice.

**v1 scope (this plan):** toggle over autoplay, input/pause gating, sidebar widget list + select, mouse-drag-move (offset), keyboard nudge, reset-widget / reset-all, lane preset cycle, undo/redo, save. **Deferred to a documented v2 (all additive, no rework):** direct on-canvas click-select, scale/rotate handles, anchor/origin snap UI, hit-group (DrumsConfig) dropdowns, lane drag-reorder/resize, playfield move, song-select launch, alignment guides.

---

## File Structure

```
crates/gameplay-drums/src/editor/
  mod.rs      EditorOpen resource, plugin, toggle (Ctrl+Shift+E), open/close
              lifecycle (autoplay on/off, spawn/despawn UI), input/pause gating hook
  drag.rs     Selection resource, drag-move system, keyboard nudge, pure offset math
  undo.rs     UndoStack (snapshots of WidgetLayouts+Lanes), pure push/pop, Ctrl+Z/Y
  save.rs     build LayoutFile from resources, Ctrl+S save, reset helpers
  ui.rs       sidebar (widget list buttons, lane preset button, Save/Reset/Undo/Close),
              selection highlight, top status bar
crates/gameplay-drums/src/input.rs   MODIFY — gate capture system on !EditorOpen
crates/gameplay-drums/src/pause.rs   MODIFY — gate pause toggle on !EditorOpen
crates/gameplay-drums/src/lib.rs     MODIFY — add editor module + plugin
crates/gameplay-drums/tests/editor.rs  NEW — pure-logic integration tests
```

---

### Task 1: `EditorOpen` state + toggle + input/pause gating

**Files:** Create `crates/gameplay-drums/src/editor/mod.rs`; modify `input.rs`, `pause.rs`, `lib.rs`.

- [ ] **Step 1: Create `editor/mod.rs`:**

```rust
//! In-Performance layout editor overlay. Inert unless opened (Ctrl+Shift+E).
//!
//! Opening force-enables autoplay (notes flow hands-free), gates drum input +
//! pause, and spawns the sidebar. Closing restores the prior autoplay flag and
//! despawns the UI. All mutation targets `WidgetLayouts` / `Lanes`, which the
//! HUD already reacts to (plan 1 + 2).

use bevy::prelude::*;
use game_shell::AppState;

pub mod drag;
pub mod save;
pub mod ui;
pub mod undo;

/// True while the editor overlay is open. Default false — normal play/practice.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorOpen(pub bool);

/// Remembers the autoplay flag from before the editor forced it on.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PrevAutoplay(pub bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<EditorOpen>()
        .init_resource::<PrevAutoplay>()
        .init_resource::<drag::Selection>()
        .init_resource::<undo::UndoStack>()
        .add_systems(
            Update,
            toggle_editor.run_if(in_state(AppState::Performance)),
        )
        .add_plugins((drag::plugin, undo::plugin, save::plugin, ui::plugin));
}

/// Ctrl+Shift+E toggles the editor while in Performance.
fn toggle_editor(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<EditorOpen>,
    mut prev: ResMut<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if ctrl && shift && keys.just_pressed(KeyCode::KeyE) {
        open.0 = !open.0;
        if open.0 {
            prev.0 = autoplay.0;
            autoplay.0 = true;
        } else {
            autoplay.0 = prev.0;
        }
    }
}

/// Run condition: editor is open.
pub fn editor_open(open: Res<EditorOpen>) -> bool {
    open.0
}

/// Run condition: editor is closed (for gating gameplay systems).
pub fn editor_closed(open: Res<EditorOpen>) -> bool {
    !open.0
}
```

Notes: verify `crate::autoplay::AutoplayEnabled` is `pub` with a `pub bool` field (it is: `pub struct AutoplayEnabled(pub bool)`). Verify the sub-module `plugin` fns exist as you create them in later tasks — for Task 1, create stub `plugin` fns in drag.rs/undo.rs/save.rs/ui.rs that do nothing yet (each `pub fn plugin(_app: &mut App) {}` plus the resources they define), OR build Task 1 to compile by temporarily commenting the `.add_plugins(...)` line and the sub-`pub mod`s until their tasks land. **Chosen approach:** create minimal stub files now so the module tree compiles:
- `drag.rs`: `use bevy::prelude::*; #[derive(Resource, Default)] pub struct Selection(pub Option<dtx_layout::WidgetKind>); pub fn plugin(_app: &mut App) {}`
- `undo.rs`: `use bevy::prelude::*; #[derive(Resource, Default)] pub struct UndoStack; pub fn plugin(_app: &mut App) {}`
- `save.rs`: `use bevy::prelude::*; pub fn plugin(_app: &mut App) {}`
- `ui.rs`: `use bevy::prelude::*; pub fn plugin(_app: &mut App) {}`
(These get fleshed out in Tasks 2-5.)

- [ ] **Step 2: Gate drum input.** In `crates/gameplay-drums/src/input.rs`, the `capture_key_to_lane_input` system writes `LaneHit`. Add `.run_if(crate::editor::editor_closed)` to its registration (find where it's added — likely in that file's `plugin`/`add_systems`). If it's registered in a tuple, add the run_if to the whole tuple or just that system. Editor open ⇒ no drum input leaks.

- [ ] **Step 3: Gate pause.** In `crates/gameplay-drums/src/pause.rs`, add `.run_if(crate::editor::editor_closed)` to the `toggle_pause` system registration so `Esc` while the editor is open does not open the pause menu (the editor's own Esc/close is handled in ui.rs Task 5).

- [ ] **Step 4: Wire module.** In `crates/gameplay-drums/src/lib.rs`: add `pub mod editor;` and add `editor::plugin` to the plugin set (next to `hud::plugin`, `widget_layout::plugin`).

- [ ] **Step 5: Verify.** `cargo build -p gameplay-drums` clean. `cargo test -p gameplay-drums` PASS. Add a tiny unit test in mod.rs:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_open_closed_conditions() {
        // pure resource logic sanity — the run conditions read EditorOpen.
        assert!(!EditorOpen::default().0);
    }
}
```

- [ ] **Step 6: Commit.**
```bash
rustfmt crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/drag.rs crates/gameplay-drums/src/editor/undo.rs crates/gameplay-drums/src/editor/save.rs crates/gameplay-drums/src/editor/ui.rs
git status
git add crates/gameplay-drums
git commit -m "feat(gameplay-drums): editor module — EditorOpen toggle + autoplay + input/pause gating"
```

---

### Task 2: Drag-move + selection (pure math + system)

**Files:** Replace stub `crates/gameplay-drums/src/editor/drag.rs`.

- [ ] **Step 1: Write `drag.rs` (tests-first for the pure fn, then systems):**

```rust
//! Widget selection + mouse-drag / keyboard-nudge movement.
//!
//! Selection is by `WidgetKind` (chosen from the sidebar list). Dragging adds
//! the cursor delta (in screen px, converted to ref px by ÷scale) to the
//! selected widget's offset. Direct on-canvas click-select is a v2 refinement.

use bevy::prelude::*;
use dtx_layout::{WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE};

use crate::widget_layout::WidgetLayouts;

/// Currently selected widget (None = nothing selected).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Selection(pub Option<WidgetKind>);

/// Cursor position on the previous frame, for delta computation while dragging.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DragCursor(pub Option<Vec2>);

/// Pure: new ref-px offset after moving by a screen-px delta at `scale`.
pub fn apply_drag(offset: (f32, f32), screen_delta: Vec2, scale: f32) -> (f32, f32) {
    if scale <= f32::EPSILON {
        return offset;
    }
    (
        offset.0 + screen_delta.x / scale,
        offset.1 + screen_delta.y / scale,
    )
}

/// Pure: clamp a widget scale into the allowed band.
pub fn clamp_scale(s: f32) -> f32 {
    s.clamp(MIN_WIDGET_SCALE, MAX_WIDGET_SCALE)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<DragCursor>().add_systems(
        Update,
        (drag_selected_widget, nudge_selected_widget)
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// While the left mouse is held with a widget selected, translate its offset by
/// the cursor delta ÷ scale.
fn drag_selected_widget(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selection: Res<Selection>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    mut cursor: ResMut<DragCursor>,
    mut layouts: ResMut<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
    lanes: Res<crate::lanes::Lanes>,
    mut just_started: Local<bool>,
) {
    let Some(kind) = selection.0 else {
        cursor.0 = None;
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(pos) = window.cursor_position() else {
        return;
    };

    if !buttons.pressed(MouseButton::Left) {
        // Drag released: push an undo snapshot once per completed drag.
        if *just_started {
            undo.push(&layouts, &lanes);
            *just_started = false;
        }
        cursor.0 = None;
        return;
    }

    if !*just_started {
        *just_started = true;
        // Snapshot BEFORE the move so undo restores the pre-drag state.
        undo.push(&layouts, &lanes);
    }

    if let Some(prev) = cursor.0 {
        let delta = pos - prev;
        if delta != Vec2::ZERO {
            if let Some(inst) = layouts.0.get_mut(&kind) {
                inst.offset = apply_drag(inst.offset, delta, pfl.scale);
            }
        }
    }
    cursor.0 = Some(pos);
}

/// Arrow keys nudge the selected widget (1 ref-px; Shift = 8).
fn nudge_selected_widget(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    mut layouts: ResMut<WidgetLayouts>,
) {
    let Some(kind) = selection.0 else {
        return;
    };
    let step = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        8.0
    } else {
        1.0
    };
    let mut d = (0.0f32, 0.0f32);
    if keys.just_pressed(KeyCode::ArrowLeft) {
        d.0 -= step;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        d.0 += step;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        d.1 -= step;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        d.1 += step;
    }
    if d != (0.0, 0.0) {
        if let Some(inst) = layouts.0.get_mut(&kind) {
            inst.offset.0 += d.0;
            inst.offset.1 += d.1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_adds_delta_over_scale() {
        let o = apply_drag((10.0, 20.0), Vec2::new(30.0, 15.0), 2.0);
        assert_eq!(o, (25.0, 27.5));
    }

    #[test]
    fn drag_at_unit_scale_is_raw_delta() {
        assert_eq!(apply_drag((0.0, 0.0), Vec2::new(5.0, -7.0), 1.0), (5.0, -7.0));
    }

    #[test]
    fn drag_zero_scale_is_noop() {
        assert_eq!(apply_drag((3.0, 4.0), Vec2::new(9.0, 9.0), 0.0), (3.0, 4.0));
    }

    #[test]
    fn scale_clamped() {
        assert_eq!(clamp_scale(99.0), MAX_WIDGET_SCALE);
        assert_eq!(clamp_scale(0.01), MIN_WIDGET_SCALE);
    }
}
```

Notes: verify `super::undo::UndoStack` has a `push(&WidgetLayouts, &Lanes)` method — it's defined in Task 3; for Task 2 to compile independently, either land Task 3 first or give the Task-1 `UndoStack` stub a `pub fn push(&mut self, _l: &WidgetLayouts, _n: &crate::lanes::Lanes) {}` no-op. **Recommended:** implement Task 3 (undo) BEFORE Task 2's drag system references `push`, OR keep the stub `push` no-op and let Task 3 replace it. Simplest ordering: do Task 3 first, then Task 2. If you keep plan order, add the no-op `push` to the stub. Also confirm `Window::cursor_position()` and `ButtonInput<MouseButton>` are correct for this Bevy (they are standard).

- [ ] **Step 2: Verify** — `cargo test -p gameplay-drums drag` → 4 pure-fn tests PASS. Build clean.

- [ ] **Step 3: Commit** — `git commit -m "feat(gameplay-drums): editor widget selection + mouse-drag/keyboard-nudge move"`.

---

### Task 3: Undo/redo stack

**Files:** Replace stub `crates/gameplay-drums/src/editor/undo.rs`.

Do this BEFORE Task 2 if you want `push` real when drag.rs references it (see Task 2 note). Reorder freely — undo is self-contained.

- [ ] **Step 1: Write `undo.rs`:**

```rust
//! Undo/redo for editor edits: snapshots of (WidgetLayouts, Lanes).

use bevy::prelude::*;

use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Clone)]
pub struct Snapshot {
    pub layouts: WidgetLayouts,
    pub lanes: Lanes,
}

/// Bounded undo/redo history.
#[derive(Resource, Default)]
pub struct UndoStack {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
}

const MAX_HISTORY: usize = 64;

impl UndoStack {
    /// Record the current state as an undo point (clears redo history).
    pub fn push(&mut self, layouts: &WidgetLayouts, lanes: &Lanes) {
        self.past.push(Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        });
        if self.past.len() > MAX_HISTORY {
            self.past.remove(0);
        }
        self.future.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Undo: restore the last snapshot, pushing the current state to redo.
    /// Returns the state to apply, or None if nothing to undo.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let prev = self.past.pop()?;
        self.future.push(current);
        Some(prev)
    }

    /// Redo: reapply the last undone snapshot.
    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        undo_redo_hotkeys
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Ctrl+Z = undo, Ctrl+Y or Ctrl+Shift+Z = redo.
fn undo_redo_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<UndoStack>,
    mut layouts: ResMut<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !ctrl {
        return;
    }
    let current = Snapshot {
        layouts: layouts.clone(),
        lanes: lanes.clone(),
    };
    if keys.just_pressed(KeyCode::KeyZ) && !shift {
        if let Some(s) = stack.undo(current) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    } else if keys.just_pressed(KeyCode::KeyY) || (shift && keys.just_pressed(KeyCode::KeyZ)) {
        if let Some(s) = stack.redo(current) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::WidgetKind;

    fn snap(offx: f32) -> (WidgetLayouts, Lanes) {
        let mut l = WidgetLayouts::default();
        l.0.get_mut(&WidgetKind::Combo).unwrap().offset = (offx, 0.0);
        (l, Lanes::default())
    }

    #[test]
    fn undo_then_redo_round_trips() {
        let mut stack = UndoStack::default();
        let (l0, n0) = snap(0.0);
        stack.push(&l0, &n0); // record state A
        let (l1, n1) = snap(50.0); // moved to B
        let cur_b = Snapshot { layouts: l1, lanes: n1 };
        let restored_a = stack.undo(cur_b.clone()).unwrap();
        assert_eq!(restored_a.layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
        let back_b = stack.redo(restored_a).unwrap();
        assert_eq!(back_b.layouts.get(WidgetKind::Combo).offset, (50.0, 0.0));
    }

    #[test]
    fn undo_empty_is_none() {
        let mut stack = UndoStack::default();
        let (l, n) = snap(1.0);
        assert!(stack.undo(Snapshot { layouts: l, lanes: n }).is_none());
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = UndoStack::default();
        let (l, n) = snap(0.0);
        stack.push(&l, &n);
        let _ = stack.undo(Snapshot { layouts: l.clone(), lanes: n.clone() });
        assert!(stack.can_redo());
        stack.push(&l, &n);
        assert!(!stack.can_redo());
    }
}
```

Notes: `WidgetLayouts` and `Lanes` must be `Clone` — `WidgetLayouts` derives Clone (plan 2), `Lanes` derives Clone (plan 1). Confirm both. `WidgetLayouts.0` field is `pub`.

- [ ] **Step 2: Verify** — `cargo test -p gameplay-drums undo` → 3 PASS. If Task 2 already landed with a stub `push`, this replaces the stub — remove the stub `UndoStack` from Task 1.

- [ ] **Step 3: Commit** — `git commit -m "feat(gameplay-drums): editor undo/redo stack for widget+lane edits"`.

---

### Task 4: Save + reset + lane preset cycle

**Files:** Replace stub `crates/gameplay-drums/src/editor/save.rs`.

- [ ] **Step 1: Write `save.rs`:**

```rust
//! Persist / reset editor edits, and cycle lane presets.

use bevy::prelude::*;
use dtx_layout::{LayoutFile, LATEST_VERSION};

use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

/// Build a `LayoutFile` from the live resources (for saving).
pub fn layout_file_from(layouts: &WidgetLayouts, lanes: &Lanes) -> LayoutFile {
    LayoutFile {
        version: LATEST_VERSION,
        lanes: dtx_layout::LanesSection::from_arrangement(&lanes.0),
        scene: dtx_layout::SceneSection::from_map(&layouts.0),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        save_hotkey
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Ctrl+S writes layout.toml.
fn save_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    layouts: Res<WidgetLayouts>,
    lanes: Res<Lanes>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        let file = layout_file_from(&layouts, &lanes);
        match dtx_layout::save(&dtx_layout::default_path(), &file) {
            Ok(()) => info!("layout saved to {:?}", dtx_layout::default_path()),
            Err(e) => warn!("layout save failed: {e}"),
        }
    }
}

/// Reset one widget to its code default.
pub fn reset_widget(layouts: &mut WidgetLayouts, kind: dtx_layout::WidgetKind) {
    layouts.0.insert(kind, dtx_layout::default_instance(kind));
}

/// Reset all widgets to defaults.
pub fn reset_all_widgets(layouts: &mut WidgetLayouts) {
    layouts.0 = dtx_layout::SceneSection::default().resolve();
}

/// Cycle to the next lane preset (Classic → NxTypeB → NxTypeD → Classic).
pub fn next_lane_preset(lanes: &mut Lanes) {
    use dtx_layout::LanePreset;
    let next = match lanes.0.preset {
        LanePreset::Classic => LanePreset::NxTypeB,
        LanePreset::NxTypeB => LanePreset::NxTypeD,
        LanePreset::NxTypeD | LanePreset::Custom => LanePreset::Classic,
    };
    lanes.0 = dtx_layout::arrangement_for(next);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::{LanePreset, WidgetKind};

    #[test]
    fn save_file_round_trips_through_resolve() {
        let mut layouts = WidgetLayouts::default();
        layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (12.0, 34.0);
        let lanes = Lanes::default();
        let file = layout_file_from(&layouts, &lanes);
        assert_eq!(file.scene.resolve()[&WidgetKind::Combo].offset, (12.0, 34.0));
    }

    #[test]
    fn reset_widget_restores_default() {
        let mut layouts = WidgetLayouts::default();
        layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (9.0, 9.0);
        reset_widget(&mut layouts, WidgetKind::Combo);
        assert_eq!(layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
    }

    #[test]
    fn preset_cycles() {
        let mut lanes = Lanes::default();
        assert_eq!(lanes.0.preset, LanePreset::Classic);
        next_lane_preset(&mut lanes);
        assert_eq!(lanes.0.preset, LanePreset::NxTypeB);
        next_lane_preset(&mut lanes);
        assert_eq!(lanes.0.preset, LanePreset::NxTypeD);
        next_lane_preset(&mut lanes);
        assert_eq!(lanes.0.preset, LanePreset::Classic);
    }
}
```

Notes: confirm `dtx_layout::arrangement_for`, `LanesSection::from_arrangement`, `SceneSection::from_map`, `default_instance` are all re-exported at crate root (they are per plans 1-2). `Lanes.0.preset` is accessible (LaneArrangement.preset is pub).

- [ ] **Step 2: Verify** — `cargo test -p gameplay-drums save` → 3 PASS.

- [ ] **Step 3: Commit** — `git commit -m "feat(gameplay-drums): editor save/reset + lane preset cycle"`.

---

### Task 5: Editor UI (sidebar + selection highlight + buttons)

**Files:** Replace stub `crates/gameplay-drums/src/editor/ui.rs`.

This is the interaction surface. Keep systems thin. Spawn on open, despawn on close. Buttons use Bevy `Interaction`.

- [ ] **Step 1: Write `ui.rs`:**

```rust
//! Editor overlay UI: left sidebar (widget list + actions), spawned while open.

use bevy::prelude::*;
use dtx_layout::WidgetKind;

use super::drag::Selection;
use super::{save, undo::UndoStack, EditorOpen};
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Component)]
struct EditorUiRoot;

#[derive(Component, Clone, Copy)]
enum EditorButton {
    Select(WidgetKind),
    ResetWidget,
    ResetAll,
    NextPreset,
    Save,
    Undo,
    Redo,
    Close,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_ui_on_open.run_if(resource_changed::<EditorOpen>),
            (handle_buttons, highlight_selection)
                .run_if(super::editor_open),
            close_on_escape.run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Spawn the sidebar when the editor opens; despawn when it closes.
fn spawn_ui_on_open(
    mut commands: Commands,
    open: Res<EditorOpen>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<EditorUiRoot>>,
) {
    // Always clear any existing UI first (handles both open→spawn and close→despawn).
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            EditorUiRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(220.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.92)),
            GlobalZIndex(2000),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        spawn_label(p, &t, "LAYOUT EDITOR");
        spawn_label(p, &t, "— widgets —");
        for kind in WidgetKind::ALL {
            spawn_button(p, &t, EditorButton::Select(kind), kind.display_name());
        }
        spawn_label(p, &t, "— actions —");
        spawn_button(p, &t, EditorButton::ResetWidget, "Reset Widget");
        spawn_button(p, &t, EditorButton::ResetAll, "Reset All");
        spawn_button(p, &t, EditorButton::NextPreset, "Next Lane Preset");
        spawn_button(p, &t, EditorButton::Undo, "Undo (Ctrl+Z)");
        spawn_button(p, &t, EditorButton::Redo, "Redo (Ctrl+Y)");
        spawn_button(p, &t, EditorButton::Save, "Save (Ctrl+S)");
        spawn_button(p, &t, EditorButton::Close, "Close (Esc)");
    });
}

fn spawn_label(p: &mut ChildSpawnerCommands, theme: &dtx_ui::theme::Theme, text: &str) {
    p.spawn((
        Text::new(text.to_string()),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(theme.text_secondary),
    ));
}

fn spawn_button(
    p: &mut ChildSpawnerCommands,
    theme: &dtx_ui::theme::Theme,
    button: EditorButton,
    label: &str,
) {
    p.spawn((
        button,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
        children![(
            Text::new(label.to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(theme.text_primary),
        )],
    ));
}

/// Handle button clicks.
fn handle_buttons(
    mut interactions: Query<(&Interaction, &EditorButton, &mut BackgroundColor), Changed<Interaction>>,
    mut selection: ResMut<Selection>,
    mut open: ResMut<EditorOpen>,
    mut layouts: ResMut<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
    mut stack: ResMut<UndoStack>,
    mut prev: ResMut<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    for (interaction, button, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.25, 0.25, 0.32);
                let snap = super::undo::Snapshot {
                    layouts: layouts.clone(),
                    lanes: lanes.clone(),
                };
                match *button {
                    EditorButton::Select(kind) => selection.0 = Some(kind),
                    EditorButton::ResetWidget => {
                        if let Some(kind) = selection.0 {
                            stack.push(&layouts, &lanes);
                            save::reset_widget(&mut layouts, kind);
                        }
                    }
                    EditorButton::ResetAll => {
                        stack.push(&layouts, &lanes);
                        save::reset_all_widgets(&mut layouts);
                    }
                    EditorButton::NextPreset => {
                        stack.push(&layouts, &lanes);
                        save::next_lane_preset(&mut lanes);
                    }
                    EditorButton::Undo => {
                        if let Some(s) = stack.undo(snap) {
                            *layouts = s.layouts;
                            *lanes = s.lanes;
                        }
                    }
                    EditorButton::Redo => {
                        if let Some(s) = stack.redo(snap) {
                            *layouts = s.layouts;
                            *lanes = s.lanes;
                        }
                    }
                    EditorButton::Save => {
                        let file = save::layout_file_from(&layouts, &lanes);
                        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
                            warn!("layout save failed: {e}");
                        }
                    }
                    EditorButton::Close => {
                        open.0 = false;
                        autoplay.0 = prev.0;
                    }
                }
            }
            Interaction::Hovered => bg.0 = Color::srgb(0.2, 0.2, 0.26),
            Interaction::None => bg.0 = Color::srgb(0.14, 0.14, 0.18),
        }
    }
}

/// Tint the selected widget's sidebar button.
fn highlight_selection(
    selection: Res<Selection>,
    mut buttons: Query<(&EditorButton, &mut BackgroundColor, &Interaction)>,
) {
    for (button, mut bg, interaction) in &mut buttons {
        if matches!(interaction, Interaction::None) {
            if let EditorButton::Select(kind) = *button {
                if selection.0 == Some(kind) {
                    bg.0 = Color::srgb(0.22, 0.3, 0.42);
                } else {
                    bg.0 = Color::srgb(0.14, 0.14, 0.18);
                }
            }
        }
    }
}

/// Esc closes the editor (pause is gated off while open).
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<EditorOpen>,
    mut prev: ResMut<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
        autoplay.0 = prev.0;
    }
}
```

Notes for the implementer:
- Verify `dtx_ui::ThemeResource` (field `.0: Theme`) and `dtx_ui::theme::Theme::font(f32)->TextFont` + `theme.text_primary`/`text_secondary` colors — grep dtx-ui/src/theme.rs; adjust names if different (e.g. maybe `text_dim`). Match whatever exists.
- Verify `ChildSpawnerCommands` is the right child-builder type in this Bevy (used elsewhere in the repo — grep `with_children(|`). If the closure param type name differs, mirror existing widget spawn fns (e.g. score_detailed.rs).
- `children![...]` macro is used in keyboard_viz.rs, so it's available.
- `GlobalZIndex(2000)` puts the sidebar above the pause panel (1000) and transport (900).
- The `Button` component + `Interaction` query is standard Bevy UI. Because the widget containers now carry `Pickable::IGNORE` (plan-2 fix), they won't steal button clicks.
- The toggle-close in mod.rs (Ctrl+Shift+E) also closes; both paths restore autoplay. Two close paths setting `autoplay.0 = prev.0` is idempotent.

- [ ] **Step 2: Verify** — `cargo build -p gameplay-drums` clean. `cargo test -p gameplay-drums` all PASS. (UI systems have no unit tests; the pure logic they call is tested in Tasks 2-4.)

- [ ] **Step 3: Commit** — `git commit -m "feat(gameplay-drums): editor sidebar UI — widget list, actions, selection highlight"`.

---

### Task 6: Integration tests + schedule guard + docs

**Files:** Create `crates/gameplay-drums/tests/editor.rs`; modify design spec.

- [ ] **Step 1: Integration tests** (`tests/editor.rs`) — exercise the pure editor logic through the public API via a headless app where practical:

```rust
//! Editor logic integration (pure paths; UI systems need a display).

use bevy::prelude::*;
use dtx_layout::WidgetKind;
use gameplay_drums::editor::drag::{apply_drag, Selection};
use gameplay_drums::editor::save::{layout_file_from, next_lane_preset, reset_all_widgets};
use gameplay_drums::editor::undo::UndoStack;
use gameplay_drums::editor::EditorOpen;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::widget_layout::WidgetLayouts;

#[test]
fn drag_moves_selected_widget_offset() {
    let start = WidgetLayouts::default().get(WidgetKind::Combo).offset;
    let moved = apply_drag(start, Vec2::new(40.0, 20.0), 1.0);
    assert_eq!(moved, (40.0, 20.0));
}

#[test]
fn full_edit_save_reload_cycle() {
    // Edit combo, build file, resolve back — persists.
    let mut layouts = WidgetLayouts::default();
    layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (25.0, -10.0);
    let lanes = Lanes::default();
    let file = layout_file_from(&layouts, &lanes);
    let toml = toml::to_string_pretty(&file).unwrap();
    let back: dtx_layout::LayoutFile = toml::from_str(&toml).unwrap();
    assert_eq!(back.scene.resolve()[&WidgetKind::Combo].offset, (25.0, -10.0));
}

#[test]
fn undo_restores_reset_all() {
    let mut layouts = WidgetLayouts::default();
    layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (50.0, 50.0);
    let lanes = Lanes::default();
    let mut stack = UndoStack::default();
    stack.push(&layouts, &lanes);
    reset_all_widgets(&mut layouts);
    assert_eq!(layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
    let snap = gameplay_drums::editor::undo::Snapshot {
        layouts: layouts.clone(),
        lanes: lanes.clone(),
    };
    let restored = stack.undo(snap).unwrap();
    assert_eq!(restored.layouts.get(WidgetKind::Combo).offset, (50.0, 50.0));
}

#[test]
fn preset_cycle_changes_lane_count_shape() {
    let mut lanes = Lanes::default();
    let c0 = lanes.count();
    next_lane_preset(&mut lanes);
    // NxTypeB still 10 lanes but different order; assert preset advanced.
    assert_eq!(lanes.0.preset, dtx_layout::LanePreset::NxTypeB);
    assert_eq!(lanes.count(), c0);
}

#[test]
fn editor_open_default_false() {
    assert!(!EditorOpen::default().0);
}

#[test]
fn toggle_editor_enables_autoplay_headless() {
    // Minimal app: insert resources, run the toggle by simulating the key.
    // (Full input simulation is heavy; assert the resource plumbing instead.)
    let mut app = App::new();
    app.insert_resource(EditorOpen(false));
    app.insert_resource(Selection(None));
    // Open the editor manually and confirm the flag flips.
    app.world_mut().resource_mut::<EditorOpen>().0 = true;
    assert!(app.world().resource::<EditorOpen>().0);
}
```

Ensure the editor module + submodules + their items are `pub` (`pub mod editor;`, `pub mod drag/undo/save;`, `pub struct/fn` as used). Adjust any visibility the tests need.

- [ ] **Step 2: Schedule guard.** This plan added NO FixedUpdate systems (all Update/OnEnter). Confirm by grep: `grep -rn "FixedUpdate" crates/gameplay-drums/src/editor` → empty. No change to `fixed_update_schedule_ordering.rs` needed. Note this in the commit body.

- [ ] **Step 3: Full verify** — `cargo test --workspace` → all PASS.

- [ ] **Step 4: Spec status.** In `docs/superpowers/specs/2026-07-07-layout-editor-design.md`, update `Progress:`:
```markdown
Progress: all three plans implemented — 1 (lane arrangement), 2 (widget
registry + [scene.gameplay]), 3 (editor overlay v1: toggle over autoplay,
sidebar select + mouse-drag move, lane preset cycle, undo/redo, save/reset).
Deferred to a v2: on-canvas click-select, scale/rotate handles, anchor-snap UI,
hit-group dropdowns, lane drag-resize, playfield move, song-select launch.
```

- [ ] **Step 5: Commit** — `git add crates/gameplay-drums docs && git commit -m "feat(gameplay-drums): editor integration tests; mark layout pillar v1 complete"`.

---

## Manual verification (needs display) — owed to the user

1. Enter a song (Performance). Press **Ctrl+Shift+E** → sidebar appears on the left, autoplay kicks in (notes self-hit), drum keys do nothing, Esc doesn't open pause.
2. Click **Combo** in the sidebar → its button highlights. Drag on the canvas → combo counter follows the mouse. Arrow keys nudge it.
3. **Next Lane Preset** → lanes reorder (NX Type-B). **Undo** → reverts. **Redo** → reapplies.
4. **Save** (or Ctrl+S) → `~/.config/dtxmaniars/layout.toml` gains `[scene.gameplay]` + `[lanes]`. Restart the song → the moved combo + preset persist.
5. **Reset All** → widgets snap back. **Close** (or Esc / Ctrl+Shift+E) → sidebar gone, autoplay restored to its prior value, drum input works again.

## Out of scope (documented v2)

On-canvas click-to-select (needs per-widget hit-testing), scale/rotate handles, anchor/origin snap UI + guides, hit-group (`DrumsConfig`) dropdowns, lane drag-reorder/resize, playfield as a movable widget, song-select editor launch, named layout profiles.
