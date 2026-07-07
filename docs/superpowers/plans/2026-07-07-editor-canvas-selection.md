# Editor On-Canvas Selection Implementation Plan (v2 plan 1 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the layout editor direct-manipulation: hover/click widgets on the canvas to select them, show an osu-style selection box (border, name tag, anchor line, corner scale handles), and drag widgets/handles directly.

**Architecture:** Manual AABB hit-testing (no bevy_picking on gameplay nodes — v1 containers carry `Pickable::IGNORE`). A `WidgetAabbs` resource is rebuilt each frame from the union of each `WidgetContainer`'s descendant `ComputedNode` rects; `Hovered`/`Selection` are driven by cursor tests against it, masked by editor chrome (sidebar). A single `ActiveGesture` state machine arbitrates move-drags vs scale-handle-drags. The selection box is an editor-UI overlay that tracks the selected widget's AABB every frame.

**Tech Stack:** Rust, Bevy 0.19 (`ComputedNode.size` is physical px; multiply by `ComputedNode.inverse_scale_factor` for logical px; `Window::cursor_position()` is logical; UI node `GlobalTransform` translation is the node's center in physical px; `UiTransform { rotation: Rot2 }` rotates UI nodes clockwise).

**Spec:** `docs/superpowers/specs/2026-07-07-layout-editor-v2-design.md` (sections 1, 2 + Input Precedence). Reference (behavior only, C#): `references/osu-lazer/osu.Game/Overlays/SkinEditor/SkinBlueprint.cs`, `SkinSelectionHandler.cs`, `references/osu-lazer/osu.Game/Screens/Edit/Compose/Components/SelectionBox.cs`.

**Branch:** `feat/editor-canvas-selection` off `main`.

**Existing context (v1, already merged):**
- `crates/gameplay-drums/src/editor/` — `mod.rs` (`EditorOpen`, `editor_open`/`editor_closed` run conditions, Ctrl+Shift+E toggle), `drag.rs` (`Selection(Option<WidgetKind>)`, `DragCursor`, drag-anywhere-while-selected + arrow nudge), `undo.rs` (`UndoStack::push(&WidgetLayouts, &Lanes)`), `save.rs`, `ui.rs` (left sidebar, `EditorUiRoot`, buttons).
- `crates/gameplay-drums/src/widget_layout.rs` — `WidgetContainer(pub WidgetKind)` full-screen container nodes at (0,0); `WidgetLayouts(HashMap<WidgetKind, WidgetInstance>)`; `widget_visible(inst, practice)`.
- `crates/gameplay-drums/src/layout.rs` — `PlayfieldLayout` (logical px; `backboard_left/top/width/height()` = playfield rect incl. pad).
- `crates/dtx-layout/src/widgets.rs` — `WidgetKind` (11 kinds incl. `Playfield`), `WidgetInstance { anchor, origin, offset, scale, z, .. }`, `Anchor9::frac()`.
- Editor systems all run `.run_if(super::editor_open)` + `.run_if(in_state(game_shell::AppState::Performance))`; gameplay hotkeys are gated `editor_closed` — do not remove those gates.
- rustfmt gotcha: NEVER run bare `cargo fmt --all`; format only files you touched via `cargo fmt -p gameplay-drums` or leave formatting as-written.

---

## File Structure

- Create: `crates/gameplay-drums/src/editor/picking.rs` — AABB collection, hover, chrome masking, pure pick/cycle fns.
- Create: `crates/gameplay-drums/src/editor/selection_box.rs` — selection overlay (border, name tag, anchor line + dots, scale handles) + hover outline.
- Modify: `crates/gameplay-drums/src/editor/drag.rs` — gesture state machine (press→select→move / scale-handle), keep nudge.
- Modify: `crates/gameplay-drums/src/editor/mod.rs` — register new plugins.
- Modify: `crates/gameplay-drums/src/editor/ui.rs` — tag sidebar root as chrome.
- Test: unit tests inline in each module + `crates/gameplay-drums/tests/editor_canvas.rs` integration.

### Task 0: Branch

- [ ] **Step 0.1:**

```bash
cd /home/lazykern/lab/dtxmaniars && git checkout -b feat/editor-canvas-selection main
```

### Task 1: picking.rs — pure hit-test math

**Files:**
- Create: `crates/gameplay-drums/src/editor/picking.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (add `pub mod picking;`)

- [ ] **Step 1.1: Create `picking.rs` with resources + pure fns + failing-by-absence tests**

```rust
//! On-canvas widget hit-testing for the layout editor.
//!
//! No bevy_picking on gameplay nodes (containers carry `Pickable::IGNORE`);
//! instead a per-frame AABB per widget is built from the union of the widget
//! container's descendant `ComputedNode` rects (logical px). Hover/click test
//! the cursor against those AABBs, masked by editor chrome (sidebar/panel).

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_layout::WidgetKind;

use crate::layout::PlayfieldLayout;
use crate::widget_layout::{widget_visible, WidgetContainer, WidgetLayouts};

/// Logical-px AABB per widget, rebuilt each frame while the editor is open.
/// Entries persist across frames (last non-empty wins) so widgets that render
/// nothing this instant (e.g. judgment popup between hits) stay grabbable.
#[derive(Resource, Debug, Default)]
pub struct WidgetAabbs(pub HashMap<WidgetKind, Rect>);

/// Widget currently under the cursor (topmost by z, then smallest area).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct Hovered(pub Option<WidgetKind>);

/// True while the cursor is over editor chrome (sidebar/panel) — canvas
/// hover/click are masked.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct CursorOverChrome(pub bool);

/// Marker for editor UI surfaces that mask the canvas (sidebar root; the
/// settings panel adds itself in plan 2).
#[derive(Component)]
pub struct EditorChrome;

/// Minimum grab box (logical px) for widgets whose content AABB is empty.
pub const MIN_GRAB: f32 = 24.0;

/// Candidates under `cursor`, best-first: higher z wins, ties → smaller area.
pub fn candidates_at(
    aabbs: &HashMap<WidgetKind, Rect>,
    z_of: impl Fn(WidgetKind) -> i32,
    cursor: Vec2,
) -> Vec<WidgetKind> {
    let mut hits: Vec<(WidgetKind, i32, f32)> = aabbs
        .iter()
        .filter(|(_, r)| r.contains(cursor))
        .map(|(k, r)| (*k, z_of(*k), r.width() * r.height()))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.total_cmp(&b.2)));
    hits.into_iter().map(|(k, _, _)| k).collect()
}

/// Topmost candidate, or None on empty canvas.
pub fn pick_topmost(
    aabbs: &HashMap<WidgetKind, Rect>,
    z_of: impl Fn(WidgetKind) -> i32,
    cursor: Vec2,
) -> Option<WidgetKind> {
    candidates_at(aabbs, z_of, cursor).into_iter().next()
}

/// Alt+click cycling: next candidate after `current` in the priority order
/// (wraps). If `current` isn't among the candidates, behaves like topmost.
pub fn cycle_pick(candidates: &[WidgetKind], current: Option<WidgetKind>) -> Option<WidgetKind> {
    if candidates.is_empty() {
        return None;
    }
    match current.and_then(|c| candidates.iter().position(|&k| k == c)) {
        Some(i) => Some(candidates[(i + 1) % candidates.len()]),
        None => Some(candidates[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    #[test]
    fn topmost_prefers_higher_z() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Combo, r(0.0, 0.0, 100.0, 100.0));
        m.insert(WidgetKind::ScorePanel, r(0.0, 0.0, 100.0, 100.0));
        let z = |k: WidgetKind| if k == WidgetKind::Combo { 5 } else { 0 };
        assert_eq!(
            pick_topmost(&m, z, Vec2::new(50.0, 50.0)),
            Some(WidgetKind::Combo)
        );
    }

    #[test]
    fn equal_z_prefers_smaller_area() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Playfield, r(0.0, 0.0, 500.0, 500.0));
        m.insert(WidgetKind::Combo, r(10.0, 10.0, 50.0, 50.0));
        assert_eq!(
            pick_topmost(&m, |_| 0, Vec2::new(20.0, 20.0)),
            Some(WidgetKind::Combo)
        );
    }

    #[test]
    fn miss_returns_none() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Combo, r(0.0, 0.0, 10.0, 10.0));
        assert_eq!(pick_topmost(&m, |_| 0, Vec2::new(99.0, 99.0)), None);
    }

    #[test]
    fn cycle_wraps_through_candidates() {
        let c = [WidgetKind::Combo, WidgetKind::ScorePanel, WidgetKind::Playfield];
        assert_eq!(cycle_pick(&c, None), Some(WidgetKind::Combo));
        assert_eq!(cycle_pick(&c, Some(WidgetKind::Combo)), Some(WidgetKind::ScorePanel));
        assert_eq!(cycle_pick(&c, Some(WidgetKind::Playfield)), Some(WidgetKind::Combo));
        assert_eq!(cycle_pick(&[], Some(WidgetKind::Combo)), None);
    }
}
```

- [ ] **Step 1.2: Add `pub mod picking;` to `crates/gameplay-drums/src/editor/mod.rs`** (alphabetical with the existing `pub mod drag; pub mod save; pub mod ui; pub mod undo;`).

- [ ] **Step 1.3: Run tests**

Run: `cargo test -p gameplay-drums editor::picking -- --nocapture`
Expected: 4 PASS.

- [ ] **Step 1.4: Commit**

```bash
git add crates/gameplay-drums/src/editor/picking.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(editor): pure AABB hit-test + alt-cycle picking math"
```

### Task 2: picking.rs — AABB collection + hover systems

**Files:**
- Modify: `crates/gameplay-drums/src/editor/picking.rs`
- Modify: `crates/gameplay-drums/src/editor/ui.rs` (tag sidebar with `EditorChrome`)
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (register plugin)

- [ ] **Step 2.1: Append collection/hover systems + plugin to `picking.rs`**

```rust
/// Logical-px rect of a laid-out UI node (GlobalTransform = center, physical).
pub(crate) fn node_rect(node: &ComputedNode, gt: &GlobalTransform) -> Rect {
    let inv = node.inverse_scale_factor();
    let center = gt.translation().truncate() * inv;
    let size = node.size() * inv;
    Rect::from_center_size(center, size)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<WidgetAabbs>()
        .init_resource::<Hovered>()
        .init_resource::<CursorOverChrome>()
        .add_systems(
            Update,
            (collect_widget_aabbs, update_cursor_over_chrome, update_hover)
                .chain()
                .in_set(super::EditorPickSet)
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// Union of each widget container's descendant node rects (skipping the
/// full-screen container itself). Hidden-in-mode widgets are removed so they
/// aren't hit-testable; empty unions keep their previous entry (falling back
/// to a MIN_GRAB box at the container's offset if never seen).
fn collect_widget_aabbs(
    mut aabbs: ResMut<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    containers: Query<(Entity, &WidgetContainer, &Node)>,
    children_q: Query<&Children>,
    nodes: Query<(&ComputedNode, &GlobalTransform)>,
) {
    let is_practice = practice.is_some();
    for (entity, container, cnode) in &containers {
        let kind = container.0;
        if kind == WidgetKind::Playfield {
            continue;
        }
        if !widget_visible(layouts.get(kind), is_practice) {
            aabbs.0.remove(&kind);
            continue;
        }
        let mut union: Option<Rect> = None;
        let mut stack: Vec<Entity> = children_q
            .get(entity)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        while let Some(e) = stack.pop() {
            if let Ok((cn, gt)) = nodes.get(e) {
                if cn.size().x > 0.0 && cn.size().y > 0.0 {
                    let r = node_rect(cn, gt);
                    union = Some(union.map_or(r, |u| u.union(r)));
                }
            }
            if let Ok(c) = children_q.get(e) {
                stack.extend(c.iter());
            }
        }
        match union {
            Some(r) if r.width() >= 1.0 && r.height() >= 1.0 => {
                let min_sized = Rect::from_center_size(
                    r.center(),
                    r.size().max(Vec2::splat(MIN_GRAB)),
                );
                aabbs.0.insert(kind, min_sized);
            }
            _ => {
                aabbs.0.entry(kind).or_insert_with(|| {
                    let (l, t) = match (&cnode.left, &cnode.top) {
                        (Val::Px(l), Val::Px(t)) => (*l, *t),
                        _ => (0.0, 0.0),
                    };
                    Rect::new(l, t, l + MIN_GRAB, t + MIN_GRAB)
                });
            }
        }
    }
    // Playfield AABB straight from layout geometry (backboard incl. pad).
    aabbs.0.insert(
        WidgetKind::Playfield,
        Rect::new(
            pfl.backboard_left(),
            pfl.backboard_top(),
            pfl.backboard_left() + pfl.backboard_width(),
            pfl.backboard_top() + pfl.backboard_height(),
        ),
    );
}

fn update_cursor_over_chrome(
    mut over: ResMut<CursorOverChrome>,
    windows: Query<&Window>,
    chrome: Query<(&ComputedNode, &GlobalTransform), With<EditorChrome>>,
) {
    over.0 = false;
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else { return };
    for (cn, gt) in &chrome {
        if node_rect(cn, gt).contains(pos) {
            over.0 = true;
            return;
        }
    }
}

fn update_hover(
    mut hovered: ResMut<Hovered>,
    over_chrome: Res<CursorOverChrome>,
    gesture: Res<super::drag::ActiveGesture>,
    aabbs: Res<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    windows: Query<&Window>,
) {
    if over_chrome.0 || !matches!(gesture.0, super::drag::Gesture::None) {
        hovered.0 = None;
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    hovered.0 = pick_topmost(&aabbs.0, |k| layouts.get(k).z, pos);
}
```

Note: `super::EditorPickSet` and `super::drag::ActiveGesture`/`Gesture` are defined in Steps 2.2/3.1 — this task compiles only after Step 3.1; write Tasks 2 and 3 code before running the build (the checklist order below runs the build at 3.4).

- [ ] **Step 2.2: In `editor/mod.rs`, define the system set and register the plugin**

Add near the top (after the resource definitions):

```rust
/// Ordering: picking (AABBs/hover) → gestures (drag) → overlay sync.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorPickSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorGestureSet;
```

In `plugin(...)`, extend the `.add_plugins((...))` tuple with `picking::plugin` and configure set order:

```rust
        .configure_sets(
            Update,
            (EditorPickSet, EditorGestureSet).chain(),
        )
        .add_plugins((drag::plugin, undo::plugin, save::plugin, ui::plugin, picking::plugin));
```

- [ ] **Step 2.3: Tag the sidebar as chrome in `ui.rs`** — in `spawn_ui_on_open`, add `super::picking::EditorChrome,` to the root spawn bundle (the one with `EditorUiRoot`).

- [ ] **Step 2.4: Proceed to Task 3 before building** (Gesture types referenced above land there).

### Task 3: drag.rs — gesture state machine (press→select→move)

**Files:**
- Modify: `crates/gameplay-drums/src/editor/drag.rs`

- [ ] **Step 3.1: Replace `DragCursor`/`drag_selected_widget` with a gesture model**

Delete the `DragCursor` resource and `drag_selected_widget` system. Add:

```rust
/// Active mouse gesture. Scale carries drag-start reference data.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Gesture {
    #[default]
    None,
    Move {
        last_cursor: Vec2,
    },
    Scale {
        start_dist: f32,
        start_scale: f32,
    },
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ActiveGesture(pub Gesture);
```

New systems (registered in Step 3.2):

```rust
/// Left-press routing (canvas only; chrome masked): scale handle → Scale
/// gesture; widget under cursor → select + Move gesture (Alt cycles stacked
/// candidates); empty canvas → deselect. Playfield selects but never moves.
fn begin_gesture(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    over_chrome: Res<super::picking::CursorOverChrome>,
    aabbs: Res<super::picking::WidgetAabbs>,
    handles: Query<(&super::selection_box::ScaleHandle, &ComputedNode, &GlobalTransform)>,
    mut selection: ResMut<Selection>,
    mut gesture: ResMut<ActiveGesture>,
    layouts: Res<WidgetLayouts>,
    lanes: Res<crate::lanes::Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    if !buttons.just_pressed(MouseButton::Left) || over_chrome.0 {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else { return };

    // 1. Scale handles first (they can overhang neighboring widgets).
    if let Some(kind) = selection.0 {
        if kind != dtx_layout::WidgetKind::Playfield {
            for (_, cn, gt) in &handles {
                let r = super::picking::node_rect(cn, gt);
                // Inflate for easier grabbing.
                let r = Rect::from_center_size(r.center(), r.size() + Vec2::splat(6.0));
                if r.contains(pos) {
                    if let Some(aabb) = aabbs.0.get(&kind) {
                        let start_dist = (pos - aabb.center()).length().max(1.0);
                        let start_scale = layouts.get(kind).scale;
                        undo.push(&layouts, &lanes);
                        gesture.0 = Gesture::Scale { start_dist, start_scale };
                        return;
                    }
                }
            }
        }
    }

    // 2. Canvas widgets.
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let cands = super::picking::candidates_at(&aabbs.0, |k| layouts.get(k).z, pos);
    let picked = if alt {
        super::picking::cycle_pick(&cands, selection.0)
    } else {
        cands.first().copied()
    };
    selection.0 = picked;
    if let Some(kind) = picked {
        if kind != dtx_layout::WidgetKind::Playfield {
            undo.push(&layouts, &lanes);
            gesture.0 = Gesture::Move { last_cursor: pos };
        }
    }
}

/// Advance the active gesture each frame; release ends it.
fn update_gesture(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selection: Res<Selection>,
    aabbs: Res<super::picking::WidgetAabbs>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    mut gesture: ResMut<ActiveGesture>,
    mut layouts: ResMut<WidgetLayouts>,
) {
    if !buttons.pressed(MouseButton::Left) {
        gesture.0 = Gesture::None;
        return;
    }
    let Some(kind) = selection.0 else {
        gesture.0 = Gesture::None;
        return;
    };
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else { return };
    match gesture.0 {
        Gesture::None => {}
        Gesture::Move { last_cursor } => {
            let delta = pos - last_cursor;
            if delta != Vec2::ZERO {
                if let Some(inst) = layouts.0.get_mut(&kind) {
                    inst.offset = apply_drag(inst.offset, delta, pfl.scale);
                }
            }
            gesture.0 = Gesture::Move { last_cursor: pos };
        }
        Gesture::Scale { start_dist, start_scale } => {
            if let Some(aabb) = aabbs.0.get(&kind) {
                let dist = (pos - aabb.center()).length().max(1.0);
                if let Some(inst) = layouts.0.get_mut(&kind) {
                    inst.scale = clamp_scale(start_scale * dist / start_dist);
                }
            }
        }
    }
}
```

Note on the undo-on-select behavior: `begin_gesture` pushes a snapshot at Move/Scale gesture start (same policy as v1's drag). Selecting without moving pushes one snapshot too — acceptable; `UndoStack` is capped at 64.

- [ ] **Step 3.2: Rewire `drag::plugin`**

```rust
pub fn plugin(app: &mut App) {
    app.init_resource::<ActiveGesture>().add_systems(
        Update,
        (begin_gesture, update_gesture, nudge_selected_widget)
            .chain()
            .in_set(super::EditorGestureSet)
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}
```

`apply_drag`, `clamp_scale`, `Selection`, `nudge_selected_widget`, and the existing unit tests stay unchanged.

- [ ] **Step 3.3: Reset gesture + hover on editor close/exit** — in `editor/mod.rs`, extend `close_editor_on_exit` and the toggle-off branch of `toggle_editor` is NOT needed (resources are inert while closed), but add to `close_editor_on_exit`:

```rust
fn close_editor_on_exit(
    mut open: ResMut<EditorOpen>,
    prev: Res<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
) {
    if open.0 {
        autoplay.0 = prev.0;
        open.0 = false;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
}
```

- [ ] **Step 3.4: Build once selection_box.rs stub exists** — Task 4 Step 4.1 creates `ScaleHandle`; to compile now, create the stub file first (see Task 4 Step 4.1), then:

Run: `cargo test -p gameplay-drums editor:: -- --nocapture`
Expected: all existing editor tests + picking tests PASS.

- [ ] **Step 3.5: Commit**

```bash
git add crates/gameplay-drums/src/editor/
git commit -m "feat(editor): press-to-select gesture state machine on canvas"
```

### Task 4: selection_box.rs — overlay (border, name tag, hover outline)

**Files:**
- Create: `crates/gameplay-drums/src/editor/selection_box.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (add `pub mod selection_box;`, register plugin)

- [ ] **Step 4.1: Create module with markers + spawn/despawn/sync systems**

```rust
//! Selection overlay: border + name tag + anchor line + corner scale handles
//! around the selected widget's AABB, and a lighter hover outline. Editor-only
//! UI, kept above the sidebar via GlobalZIndex.

use bevy::prelude::*;
use bevy::ui::UiTransform;
use dtx_layout::WidgetKind;

use super::drag::Selection;
use super::picking::{Hovered, WidgetAabbs};
use crate::widget_layout::{widget_visible, WidgetLayouts};

/// Every editor-overlay entity (cleanup marker).
#[derive(Component)]
pub struct EditorOverlay;

/// The selection border box (one, reused; hidden when no selection).
#[derive(Component)]
pub struct SelectionBoxRoot;

/// One of the four corner scale handles; index 0..4 = TL, TR, BL, BR.
#[derive(Component, Clone, Copy)]
pub struct ScaleHandle(pub usize);

#[derive(Component)]
pub struct SelectionNameTag;

#[derive(Component)]
pub struct AnchorLine;

#[derive(Component)]
pub struct AnchorDot;

#[derive(Component)]
pub struct OriginDot;

/// Root of the hover outline (separate from selection so both can show).
#[derive(Component)]
pub struct HoverOutlineRoot;

const ACCENT: Color = Color::srgb(1.0, 0.75, 0.1);
const HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.5);
const HANDLE_SIZE: f32 = 10.0;
const DOT_SIZE: f32 = 6.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_overlay_on_open,
            sync_selection_border,
            sync_anchor_viz,
            sync_hover_outline,
        )
            .chain()
            .after(super::EditorGestureSet)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        OnExit(game_shell::AppState::Performance),
        despawn_overlay,
    );
}

fn despawn_overlay(mut commands: Commands, roots: Query<Entity, With<EditorOverlay>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Spawn the overlay entities when the editor opens; despawn when it closes.
fn spawn_overlay_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    existing: Query<Entity, With<EditorOverlay>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    commands.spawn((
        EditorOverlay,
        HoverOutlineRoot,
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        BorderColor::all(HOVER),
        Visibility::Hidden,
        GlobalZIndex(2100),
        Pickable::IGNORE,
    ));
    commands
        .spawn((
            EditorOverlay,
            SelectionBoxRoot,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(ACCENT),
            Visibility::Hidden,
            GlobalZIndex(2200),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            for i in 0..4usize {
                let (l, t) = match i {
                    0 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Px(-HANDLE_SIZE / 2.0)),
                    1 => (Val::Auto, Val::Px(-HANDLE_SIZE / 2.0)),
                    2 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Auto),
                    _ => (Val::Auto, Val::Auto),
                };
                let (r, b) = match i {
                    1 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Auto),
                    3 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Px(-HANDLE_SIZE / 2.0)),
                    2 => (Val::Auto, Val::Px(-HANDLE_SIZE / 2.0)),
                    _ => (Val::Auto, Val::Auto),
                };
                p.spawn((
                    ScaleHandle(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: l,
                        top: t,
                        right: r,
                        bottom: b,
                        width: Val::Px(HANDLE_SIZE),
                        height: Val::Px(HANDLE_SIZE),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                    Pickable::IGNORE,
                ));
            }
            p.spawn((
                SelectionNameTag,
                Text::new(""),
                dtx_ui::theme::Theme::font(11.0),
                TextColor(ACCENT),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-18.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
    // Anchor viz nodes live outside the box (positions are unrelated rects).
    commands.spawn((
        EditorOverlay,
        AnchorLine,
        Node {
            position_type: PositionType::Absolute,
            height: Val::Px(2.0),
            ..default()
        },
        UiTransform::default(),
        BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.9)),
        Visibility::Hidden,
        GlobalZIndex(2150),
        Pickable::IGNORE,
    ));
    commands.spawn((
        EditorOverlay,
        AnchorDot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(DOT_SIZE),
            height: Val::Px(DOT_SIZE),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.3, 0.3)),
        BorderRadius::all(Val::Px(DOT_SIZE / 2.0)),
        Visibility::Hidden,
        GlobalZIndex(2150),
        Pickable::IGNORE,
    ));
    commands.spawn((
        EditorOverlay,
        OriginDot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(DOT_SIZE),
            height: Val::Px(DOT_SIZE),
            ..default()
        },
        BackgroundColor(ACCENT),
        BorderRadius::all(Val::Px(DOT_SIZE / 2.0)),
        Visibility::Hidden,
        GlobalZIndex(2150),
        Pickable::IGNORE,
    ));
}
```

(`EditorOverlay` is the sole cleanup marker; `SelectionBoxRoot` marks ONLY the border box so `single_mut` queries stay unambiguous.)

- [ ] **Step 4.2: Add the per-frame sync systems (same file)**

IMPORTANT Bevy gotcha: two queries in ONE system that both access `&mut Node` panic at startup (B0001) unless provably disjoint via explicit `Without<...>` filters. Avoid the problem entirely: split into three systems (cross-system access never conflicts) and use `ParamSet` where one system must touch several `&mut Node` groups. All three are already registered in the plugin `chain()` shown in Step 4.1.

Selection state shared by the three systems — a small helper:

```rust
/// The selected widget's kind + AABB, or None (nothing selected / no AABB).
fn selected_aabb(
    open: &super::EditorOpen,
    selection: &Selection,
    aabbs: &WidgetAabbs,
) -> Option<(WidgetKind, Rect)> {
    if !open.0 {
        return None;
    }
    let kind = selection.0?;
    Some((kind, *aabbs.0.get(&kind)?))
}

/// Parent-space rect for a widget's anchor (logical px): screen or playfield.
fn parent_rect(
    space: dtx_layout::AnchorSpace,
    window: &Window,
    pfl: &crate::layout::PlayfieldLayout,
) -> Rect {
    match space {
        dtx_layout::AnchorSpace::Screen => {
            Rect::new(0.0, 0.0, window.width(), window.height())
        }
        dtx_layout::AnchorSpace::Playfield => Rect::new(
            pfl.strip_left(),
            pfl.lane_top(),
            pfl.strip_left() + pfl.strip_width(),
            pfl.lane_top() + pfl.lane_height(),
        ),
    }
}
```

System 1 — border box + name tag + handle visibility (`&mut Node` only on the box root; `Text` and handle `Visibility` don't overlap it):

```rust
fn sync_selection_border(
    open: Res<super::EditorOpen>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    mut box_q: Query<
        (&mut Node, &mut Visibility, &mut BorderColor),
        With<SelectionBoxRoot>,
    >,
    mut tag_q: Query<&mut Text, With<SelectionNameTag>>,
    mut handles: Query<&mut Visibility, (With<ScaleHandle>, Without<SelectionBoxRoot>)>,
) {
    let Ok((mut node, mut vis, mut border)) = box_q.single_mut() else { return };
    let Some((kind, aabb)) = selected_aabb(&open, &selection, &aabbs) else {
        *vis = Visibility::Hidden;
        return;
    };
    let inst = layouts.get(kind);
    let is_practice = practice.is_some();

    node.left = Val::Px(aabb.min.x);
    node.top = Val::Px(aabb.min.y);
    node.width = Val::Px(aabb.width());
    node.height = Val::Px(aabb.height());
    *vis = Visibility::Visible;
    // Hidden-in-mode widget: dim the border (selected from the sidebar list).
    let alpha = if widget_visible(inst, is_practice) { 1.0 } else { 0.35 };
    *border = BorderColor::all(ACCENT.with_alpha(alpha));

    if let Ok(mut text) = tag_q.single_mut() {
        text.0 = kind.display_name().to_string();
    }
    let show_handles = kind != WidgetKind::Playfield;
    for mut hv in handles.iter_mut() {
        *hv = if show_handles { Visibility::Inherited } else { Visibility::Hidden };
    }
}
```

System 2 — anchor line + dots (three `&mut Node` groups → `ParamSet`):

```rust
fn sync_anchor_viz(
    open: Res<super::EditorOpen>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window>,
    mut viz: ParamSet<(
        Query<(&mut Node, &mut Visibility, &mut UiTransform), With<AnchorLine>>,
        Query<(&mut Node, &mut Visibility), With<AnchorDot>>,
        Query<(&mut Node, &mut Visibility), With<OriginDot>>,
    )>,
) {
    let sel = selected_aabb(&open, &selection, &aabbs);
    let Some((kind, aabb)) = sel else {
        if let Ok((_, mut v, _)) = viz.p0().single_mut() { *v = Visibility::Hidden; }
        if let Ok((_, mut v)) = viz.p1().single_mut() { *v = Visibility::Hidden; }
        if let Ok((_, mut v)) = viz.p2().single_mut() { *v = Visibility::Hidden; }
        return;
    };
    let Ok(window) = windows.single() else { return };
    let inst = layouts.get(kind);
    let parent = parent_rect(inst.space, window, &pfl);
    let (af_x, af_y) = inst.anchor.frac();
    let anchor_pt = Vec2::new(
        parent.min.x + af_x * parent.width(),
        parent.min.y + af_y * parent.height(),
    );
    let (of_x, of_y) = inst.origin.frac();
    let origin_pt = Vec2::new(
        aabb.min.x + of_x * aabb.width(),
        aabb.min.y + of_y * aabb.height(),
    );

    if let Ok((mut ln, mut lv, mut lt)) = viz.p0().single_mut() {
        let seg = anchor_pt - origin_pt;
        let len = seg.length();
        let mid = (anchor_pt + origin_pt) / 2.0;
        ln.left = Val::Px(mid.x - len / 2.0);
        ln.top = Val::Px(mid.y - 1.0);
        ln.width = Val::Px(len);
        lt.rotation = Rot2::radians(seg.y.atan2(seg.x));
        *lv = if len > 4.0 { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok((mut dn, mut dv)) = viz.p1().single_mut() {
        dn.left = Val::Px(anchor_pt.x - DOT_SIZE / 2.0);
        dn.top = Val::Px(anchor_pt.y - DOT_SIZE / 2.0);
        *dv = Visibility::Visible;
    }
    if let Ok((mut dn, mut dv)) = viz.p2().single_mut() {
        dn.left = Val::Px(origin_pt.x - DOT_SIZE / 2.0);
        dn.top = Val::Px(origin_pt.y - DOT_SIZE / 2.0);
        *dv = Visibility::Visible;
    }
}
```

System 3 — hover outline:

```rust
fn sync_hover_outline(
    open: Res<super::EditorOpen>,
    hovered: Res<Hovered>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    mut q: Query<(&mut Node, &mut Visibility), With<HoverOutlineRoot>>,
) {
    let Ok((mut node, mut vis)) = q.single_mut() else { return };
    let show = open.0
        && hovered.0.is_some()
        && hovered.0 != selection.0;
    let Some(aabb) = hovered.0.and_then(|k| aabbs.0.get(&k).copied()).filter(|_| show) else {
        *vis = Visibility::Hidden;
        return;
    };
    node.left = Val::Px(aabb.min.x);
    node.top = Val::Px(aabb.min.y);
    node.width = Val::Px(aabb.width());
    node.height = Val::Px(aabb.height());
    node.border = UiRect::all(Val::Px(1.0));
    *vis = Visibility::Visible;
}
```

- [ ] **Step 4.3: Register in `mod.rs`** — add `pub mod selection_box;` and `selection_box::plugin` to the second `.add_plugins` tuple (16-plugin tuple limit gotcha: put it in whichever `add_plugins` call has room).

- [ ] **Step 4.4: Build + fix API drift**

Run: `cargo build -p gameplay-drums 2>&1 | tail -20`
Expected: clean. Known risk spots (fix per compiler): `BorderColor::all` vs `BorderColor(...)` constructor shape, `Rot2::radians`, `UiTransform` import path (`bevy::ui::UiTransform`), `BorderRadius::all`. Consult the bevy 0.19 source under `~/.cargo/registry/src/*/bevy_ui-0.19.0/src/` if a signature differs — do NOT downgrade the approach to `Transform`.

- [ ] **Step 4.5: Run all editor tests**

Run: `cargo test -p gameplay-drums editor -- --nocapture`
Expected: PASS.

- [ ] **Step 4.6: Commit**

```bash
git add crates/gameplay-drums/src/editor/
git commit -m "feat(editor): selection box overlay with handles, name tag, anchor line"
```

### Task 5: Esc deselects first

**Files:**
- Modify: `crates/gameplay-drums/src/editor/ui.rs` (`close_on_escape`)

- [ ] **Step 5.1: Change `close_on_escape` to deselect before closing**

```rust
/// Esc: first press deselects; with nothing selected it closes the editor
/// (pause is gated off while open).
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut open: ResMut<EditorOpen>,
    prev: Res<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if selection.0.is_some() {
            selection.0 = None;
        } else {
            open.0 = false;
            autoplay.0 = prev.0;
        }
    }
}
```

(`Selection` is already imported in ui.rs.)

- [ ] **Step 5.2: Run tests**

Run: `cargo test -p gameplay-drums editor -- --nocapture`
Expected: PASS.

- [ ] **Step 5.3: Commit**

```bash
git add crates/gameplay-drums/src/editor/ui.rs
git commit -m "feat(editor): Esc deselects before closing"
```

### Task 6: Integration tests

**Files:**
- Create: `crates/gameplay-drums/tests/editor_canvas.rs`

- [ ] **Step 6.1: Write integration tests**

```rust
//! Editor v2 canvas selection: AABB collection + hover/selection resources.

use bevy::prelude::*;
use dtx_layout::WidgetKind;
use gameplay_drums::editor::picking::{
    candidates_at, cycle_pick, pick_topmost, WidgetAabbs,
};

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

#[test]
fn stacked_widgets_pick_by_z_then_area() {
    let mut aabbs = WidgetAabbs::default();
    aabbs.0.insert(WidgetKind::Playfield, r(100.0, 0.0, 600.0, 700.0));
    aabbs.0.insert(WidgetKind::Combo, r(300.0, 50.0, 120.0, 60.0));
    aabbs.0.insert(WidgetKind::JudgmentPopup, r(320.0, 60.0, 80.0, 30.0));
    let z = |_k: WidgetKind| 0;
    // Point inside all three → smallest wins on equal z.
    assert_eq!(
        pick_topmost(&aabbs.0, z, Vec2::new(350.0, 70.0)),
        Some(WidgetKind::JudgmentPopup)
    );
    // Raise combo's z → combo wins.
    let z2 = |k: WidgetKind| if k == WidgetKind::Combo { 3 } else { 0 };
    assert_eq!(
        pick_topmost(&aabbs.0, z2, Vec2::new(350.0, 70.0)),
        Some(WidgetKind::Combo)
    );
}

#[test]
fn alt_cycle_walks_the_stack() {
    let mut aabbs = WidgetAabbs::default();
    aabbs.0.insert(WidgetKind::Playfield, r(0.0, 0.0, 500.0, 500.0));
    aabbs.0.insert(WidgetKind::Combo, r(10.0, 10.0, 100.0, 50.0));
    let cands = candidates_at(&aabbs.0, |_| 0, Vec2::new(20.0, 20.0));
    assert_eq!(cands.len(), 2);
    let first = cycle_pick(&cands, None);
    let second = cycle_pick(&cands, first);
    let third = cycle_pick(&cands, second);
    assert_ne!(first, second);
    assert_eq!(first, third, "cycle wraps");
}

#[test]
fn gesture_types_default_to_none() {
    use gameplay_drums::editor::drag::{ActiveGesture, Gesture};
    assert_eq!(ActiveGesture::default().0, Gesture::None);
}
```

Note: if `editor` module items aren't visible from integration tests, check `crates/gameplay-drums/src/lib.rs` — the `editor` module must be `pub mod editor;` (it already is; `picking`/`drag` submodules are `pub`).

- [ ] **Step 6.2: Run**

Run: `cargo test -p gameplay-drums --test editor_canvas -- --nocapture`
Expected: 3 PASS.

- [ ] **Step 6.3: Full crate test run**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all PASS, no failures.

- [ ] **Step 6.4: Commit**

```bash
git add crates/gameplay-drums/tests/editor_canvas.rs
git commit -m "test(editor): canvas selection integration tests"
```

### Task 7: Real-binary verification + workspace tests

- [ ] **Step 7.1: Workspace tests**

Run: `cargo test --workspace 2>&1 | tail -8`
Expected: all PASS.

- [ ] **Step 7.2: Real binary launch (schedule-cycle gotcha — hand-wired test apps don't prove the real plugin schedule builds)**

Run: `timeout 40 cargo run 2>&1 | tail -20; echo "exit=$?"`
Expected: `exit=124` (clean timeout), log reaches Title state, NO panic, NO "schedule build" / cycle error.

- [ ] **Step 7.3: Commit any fixes, then report DONE with the manual checklist below for the human**

Manual checklist (needs display, not blocking):
- Ctrl+Shift+E in a song → hover widgets shows white outline + selection shows amber box.
- Click combo → box + 4 corner handles + name tag + red anchor line/dot appear.
- Drag widget body → moves; drag corner handle → uniform scale.
- Alt+click over stacked widgets cycles selection.
- Click empty canvas → deselect. Esc → deselect, Esc again → close.
- Sidebar clicks still work; hovering sidebar never selects canvas widgets.
