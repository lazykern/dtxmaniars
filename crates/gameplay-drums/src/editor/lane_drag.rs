//! Mouse drag on the Lanes tab preview pads (Task 9): drag a pad horizontally
//! to reorder its lane; drag its left/right edge to resize the lane's width.
//! Row list clicks (`lanes_panel::handle_lane_row_click`) stay select-only;
//! keyboard reorder/resize is the deferred `lanes_panel::reduce_lanes_nav`.
//!
//! Pad geometry is the key-cap row (`keyboard_viz.rs`), read straight from
//! `PlayfieldLayout` (`col_left`/`col_width`/`key_viz_top`/`key_cap_height`) —
//! SCENE space (full-window, pre stage-transform), matching how
//! `bindings_spatial` positions the selected-lane outline. The cursor
//! converts once at the input boundary via `stage_rect::window_to_scene`
//! (inverse of the stage transform), per the customize-visual-punchlist rule:
//! all widget math happens in scene space.
//!
//! Lane width is a REFERENCE-pixel value (`dtx_layout::MIN_LANE_WIDTH` ..=
//! `MAX_LANE_WIDTH`), not a scene-px or multiplier value. A scene-space
//! cursor delta is divided by `PlayfieldLayout::scale` (the ref→scene scale)
//! before it's added to a lane's pixel width — the same unit conversion the
//! Widgets-tab body drag uses (`drag::apply_drag`, `÷ pfl.scale`).

use bevy::prelude::*;
use dtx_layout::{LaneArrangement, MAX_LANE_WIDTH, MIN_LANE_WIDTH};

use super::lanes_panel::SelectedLane;
use super::picking::{node_rect, EditorChrome};
use super::undo::UndoStack;
use crate::lanes::Lanes;
use crate::layout::PlayfieldLayout;
use crate::stage_rect::{window_to_scene, StageRect};
use crate::widget_layout::WidgetLayouts;

/// Scene-px zone around a pad's edge that arms a resize instead of a reorder.
const EDGE_GRAB: f32 = 6.0;
/// Scene-px cursor travel below which a press-release counts as a click
/// (already selected the lane on press) rather than a reorder drop.
const CLICK_SLOP: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Left,
    Right,
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
enum LaneDrag {
    #[default]
    None,
    Reorder {
        from: usize,
        start_cursor_x: f32,
    },
    Resize {
        index: usize,
        edge: Edge,
        start_px: f32,
        start_cursor_x: f32,
        /// True once this drag pushed its undo snapshot — one per drag, not
        /// one per frame (mirrors `apply_lane_width_sliders`'s
        /// `snapped_this_hold`).
        pushed: bool,
    },
}

/// Target lane index for a dropped pad at scene-x: nearest lane center.
pub fn drop_index(centers: &[f32], x: f32) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (**a - x).abs().total_cmp(&(**b - x).abs()))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// New lane width (reference px) from an edge drag: current width plus a
/// signed delta already converted to reference-px units, clamped to the
/// shared [MIN_LANE_WIDTH, MAX_LANE_WIDTH] band.
pub fn edge_width(current_px: f32, dx: f32) -> f32 {
    (current_px + dx).clamp(MIN_LANE_WIDTH, MAX_LANE_WIDTH)
}

/// Repeatedly swap `from` toward `target` one step at a time — `reorder_lane`
/// only swaps adjacent lanes, so an arbitrary drop index walks there.
fn move_lane_to(arr: &mut LaneArrangement, mut from: usize, target: usize) {
    while from < target {
        dtx_layout::reorder_lane(arr, from, 1);
        from += 1;
    }
    while from > target {
        dtx_layout::reorder_lane(arr, from, -1);
        from -= 1;
    }
}

/// Scene-space left/right/top/bottom of pad `col`'s key-cap box.
fn pad_bounds(pfl: &PlayfieldLayout, col: usize) -> (f32, f32, f32, f32) {
    let left = pfl.col_left(col);
    let right = left + pfl.col_width(col);
    let top = pfl.key_viz_top();
    let bottom = top + pfl.key_cap_height();
    (left, right, top, bottom)
}

fn cursor_over_chrome(
    window_pos: Vec2,
    chrome: &Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<EditorChrome>>,
) -> bool {
    chrome
        .iter()
        .any(|(cn, gt)| node_rect(cn, gt).contains(window_pos))
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LaneDrag>().add_systems(
        Update,
        (begin_lane_drag, update_lane_drag)
            .chain()
            .before(super::lanes_panel::mirror_lane_edits_to_draft)
            .run_if(super::editor_open)
            .run_if(super::lanes_tab_active)
            .run_if(super::profile_dialog::profile_dialog_closed)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Left-press on a pad: within `EDGE_GRAB` of an edge arms a Resize,
/// otherwise arms a Reorder. Either way the pad's lane is selected
/// immediately (mirrors the Widgets-tab body drag selecting on press).
/// No undo snapshot here — a plain select-only click must not wipe the
/// redo stack; the snapshot is pushed lazily by `update_lane_drag` only
/// once the drag actually mutates the arrangement.
fn begin_lane_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    rect: Res<StageRect>,
    pfl: Res<PlayfieldLayout>,
    lanes: Res<Lanes>,
    chrome: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<EditorChrome>>,
    mut drag: ResMut<LaneDrag>,
    mut selected: ResMut<SelectedLane>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(win_pos) = window.cursor_position() else {
        return;
    };
    if cursor_over_chrome(win_pos, &chrome) {
        return;
    }
    let win_size = Vec2::new(window.width(), window.height());
    let pos = window_to_scene(win_pos, *rect, win_size);

    let count = lanes.0.lanes.len();
    let Some(col) = (0..count).find(|&i| {
        let (left, right, top, bottom) = pad_bounds(&pfl, i);
        pos.x >= left && pos.x <= right && pos.y >= top && pos.y <= bottom
    }) else {
        return;
    };

    let (left, right, ..) = pad_bounds(&pfl, col);
    selected.0 = Some(col);

    *drag = if (pos.x - left).abs() <= EDGE_GRAB {
        LaneDrag::Resize {
            index: col,
            edge: Edge::Left,
            start_px: lanes.0.lanes[col].width,
            start_cursor_x: pos.x,
            pushed: false,
        }
    } else if (right - pos.x).abs() <= EDGE_GRAB {
        LaneDrag::Resize {
            index: col,
            edge: Edge::Right,
            start_px: lanes.0.lanes[col].width,
            start_cursor_x: pos.x,
            pushed: false,
        }
    } else {
        LaneDrag::Reorder {
            from: col,
            start_cursor_x: pos.x,
        }
    };
}

/// Advance the active drag each frame; release finalizes a Reorder (a
/// negligible-movement release is just the press-time selection — no swap).
fn update_lane_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    rect: Res<StageRect>,
    pfl: Res<PlayfieldLayout>,
    layouts: Res<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
    mut selected: ResMut<SelectedLane>,
    mut drag: ResMut<LaneDrag>,
    mut undo: ResMut<UndoStack>,
) {
    let Ok(window) = windows.single() else { return };
    let win_size = Vec2::new(window.width(), window.height());

    if !buttons.pressed(MouseButton::Left) {
        if let LaneDrag::Reorder {
            from,
            start_cursor_x,
        } = *drag
        {
            if let Some(win_pos) = window.cursor_position() {
                let pos = window_to_scene(win_pos, *rect, win_size);
                // Sub-slop release = plain click (already selected on press):
                // no reorder, and crucially no undo push (would clobber redo).
                if (pos.x - start_cursor_x).abs() >= CLICK_SLOP {
                    let centers: Vec<f32> = (0..lanes.0.lanes.len())
                        .map(|i| {
                            let (left, right, ..) = pad_bounds(&pfl, i);
                            (left + right) / 2.0
                        })
                        .collect();
                    let target = drop_index(&centers, pos.x);
                    if target != from {
                        undo.push(&layouts, &lanes);
                        move_lane_to(&mut lanes.0, from, target);
                        selected.0 = Some(target);
                    }
                }
            }
        }
        *drag = LaneDrag::None;
        return;
    }

    let Some(win_pos) = window.cursor_position() else {
        return;
    };
    let pos = window_to_scene(win_pos, *rect, win_size);

    if let LaneDrag::Resize {
        index,
        edge,
        start_px,
        start_cursor_x,
        pushed,
    } = *drag
    {
        let raw_dx = pos.x - start_cursor_x;
        // Right edge: dragging right grows. Left edge: dragging LEFT grows
        // (the edge moves away from the pad body), so the sign flips.
        let signed_dx = match edge {
            Edge::Right => raw_dx,
            Edge::Left => -raw_dx,
        };
        let dx_ref = signed_dx / pfl.scale.max(f32::EPSILON);
        let new_width = edge_width(start_px, dx_ref);
        // One snapshot per drag, taken just before the first real mutation.
        if !pushed && (new_width - lanes.0.lanes[index].width).abs() > f32::EPSILON {
            undo.push(&layouts, &lanes);
            *drag = LaneDrag::Resize {
                index,
                edge,
                start_px,
                start_cursor_x,
                pushed: true,
            };
        }
        dtx_layout::set_lane_width(&mut lanes.0, index, new_width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_x_maps_to_nearest_center() {
        let centers = [100.0, 200.0, 300.0, 400.0];
        assert_eq!(drop_index(&centers, 260.0), 2);
        assert_eq!(drop_index(&centers, 50.0), 0);
        assert_eq!(drop_index(&centers, 500.0), 3);
    }

    #[test]
    fn edge_drag_resizes_pixels_with_clamp() {
        assert_eq!(edge_width(72.0, 20.0), 92.0);
        assert_eq!(edge_width(72.0, -1000.0), MIN_LANE_WIDTH);
        assert_eq!(edge_width(72.0, 1000.0), MAX_LANE_WIDTH);
    }

    #[test]
    fn move_lane_to_walks_adjacent_swaps() {
        let mut arr = dtx_layout::classic();
        let id0 = arr.lanes[0].id.clone();
        move_lane_to(&mut arr, 0, 3);
        assert_eq!(arr.lanes[3].id, id0, "lane 0 walked to index 3");
    }

    #[test]
    fn move_lane_to_walks_backward_and_shifts_others_right() {
        let mut arr = dtx_layout::classic();
        let id3 = arr.lanes[3].id.clone();
        let id0 = arr.lanes[0].id.clone();
        let id1 = arr.lanes[1].id.clone();
        let id2 = arr.lanes[2].id.clone();
        move_lane_to(&mut arr, 3, 0);
        assert_eq!(arr.lanes[0].id, id3, "lane 3 walked to index 0");
        // The three it passed each shifted one slot right.
        assert_eq!(arr.lanes[1].id, id0);
        assert_eq!(arr.lanes[2].id, id1);
        assert_eq!(arr.lanes[3].id, id2);
    }

    #[test]
    fn move_lane_to_same_index_is_noop() {
        let mut arr = dtx_layout::classic();
        let before = arr.clone();
        move_lane_to(&mut arr, 2, 2);
        assert_eq!(arr, before);
    }
}
