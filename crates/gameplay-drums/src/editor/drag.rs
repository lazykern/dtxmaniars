//! Widget selection + mouse-drag / keyboard-nudge movement.
//!
//! Selection is on-canvas (click a widget) or from the sidebar list. A single
//! `ActiveGesture` state machine arbitrates body move-drags vs corner scale-handle
//! drags. Dragging adds the cursor delta (in screen px, converted to ref px by
//! ÷scale) to the selected widget's offset.

use bevy::prelude::*;
use dtx_layout::{MAX_WIDGET_SCALE, MIN_WIDGET_SCALE, WidgetKind};

use crate::widget_layout::WidgetLayouts;

/// Currently selected widget (None = nothing selected).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Selection(pub Option<WidgetKind>);

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
        /// AABB center captured at press. Fixed for the whole gesture so the
        /// reference distance can't drift as scaling moves the live center.
        start_center: Vec2,
    },
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ActiveGesture(pub Gesture);

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

/// First edit converts a Natural widget to Anchored, capturing its current
/// visual position so nothing jumps. Keeps existing anchor/origin values.
pub fn ensure_anchored(
    inst: &mut dtx_layout::WidgetInstance,
    visual_top_left: Vec2,
    unscaled_size: Vec2,
    parent: (f32, f32, f32, f32),
    pfl_scale: f32,
) {
    if inst.placement == dtx_layout::Placement::Anchored {
        return;
    }
    inst.placement = dtx_layout::Placement::Anchored;
    inst.scale = 1.0;
    let off_px = dtx_layout::offset_for_top_left(
        inst.anchor,
        inst.origin,
        (unscaled_size.x, unscaled_size.y),
        1.0,
        (visual_top_left.x, visual_top_left.y),
        parent,
    );
    inst.offset = (
        off_px.0 / pfl_scale.max(f32::EPSILON),
        off_px.1 / pfl_scale.max(f32::EPSILON),
    );
}

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

/// Left-press routing (canvas only; chrome masked): scale handle → Scale
/// gesture; widget under cursor → select + Move gesture (Alt cycles stacked
/// candidates); empty canvas → deselect. Playfield selects but never moves.
fn begin_gesture(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    over_chrome: Res<super::picking::CursorOverChrome>,
    aabbs: Res<super::picking::WidgetAabbs>,
    hidden: Res<super::picking::CanvasHidden>,
    handles: Query<(
        &super::selection_box::ScaleHandle,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mut selection: ResMut<Selection>,
    mut gesture: ResMut<ActiveGesture>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<crate::lanes::Lanes>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    rect: Res<crate::stage_rect::StageRect>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    if !buttons.just_pressed(MouseButton::Left) || over_chrome.0 {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        return;
    };

    // 1. Scale handles first (they can overhang neighboring widgets).
    if let Some(kind) = selection.0 {
        if kind != dtx_layout::WidgetKind::Playfield {
            for (_, cn, gt) in &handles {
                let r = super::picking::node_rect(cn, gt);
                // Inflate for easier grabbing.
                let r = Rect::from_center_size(r.center(), r.size() + Vec2::splat(6.0));
                if r.contains(pos) {
                    if let Some(aabb) = aabbs.0.get(&kind) {
                        let start_center = aabb.center();
                        let start_dist = (pos - start_center).length().max(1.0);
                        let start_scale = layouts.get(kind).scale;
                        undo.push(&layouts, &lanes);
                        convert_to_anchored(&mut layouts, &geoms, &pfl, *rect, kind);
                        gesture.0 = Gesture::Scale {
                            start_dist,
                            start_scale,
                            start_center,
                        };
                        return;
                    }
                }
            }
        }
    }

    // 2. Canvas widgets (hidden-in-mode widgets are unpickable on canvas —
    // they're only selectable from the sidebar list).
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let mut cands = super::picking::candidates_at(&aabbs.0, |k| layouts.get(k).z, pos);
    cands.retain(|k| !hidden.0.contains(k));
    let picked = if alt {
        super::picking::cycle_pick(&cands, selection.0)
    } else {
        cands.first().copied()
    };
    selection.0 = picked;
    if let Some(kind) = picked {
        if kind != dtx_layout::WidgetKind::Playfield {
            undo.push(&layouts, &lanes);
            convert_to_anchored(&mut layouts, &geoms, &pfl, *rect, kind);
            gesture.0 = Gesture::Move { last_cursor: pos };
        }
    }
}

/// Convert a widget Natural→Anchored at gesture start, capturing its current
/// visual top-left (from the geom pushed through its applied transform — NOT
/// `WidgetAabbs`, whose rects are inflated to MIN_GRAB for tiny widgets) so the
/// widget doesn't jump.
fn convert_to_anchored(
    layouts: &mut WidgetLayouts,
    geoms: &crate::widget_layout::WidgetGeoms,
    pfl: &crate::layout::PlayfieldLayout,
    rect: crate::stage_rect::StageRect,
    kind: WidgetKind,
) {
    if let Some(g) = geoms.0.get(&kind).copied() {
        if let Some(inst) = layouts.0.get_mut(&kind) {
            let sc = rect.center();
            let visual_min = crate::widget_layout::transform_point(
                g.unscaled.min,
                sc,
                g.applied_translation,
                g.applied_scale,
            );
            let parent = crate::widget_layout::parent_rect_px(inst.space, rect, pfl);
            ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
        }
    }
}

/// Advance the active gesture each frame; release ends it.
fn update_gesture(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selection: Res<Selection>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    rect: Res<crate::stage_rect::StageRect>,
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
    let Some(pos) = window.cursor_position() else {
        return;
    };
    // The scene is drawn at `pfl.scale` (ref→window) AND the HudRoot stage
    // transform `s` (window→miniature). A screen-px cursor delta therefore maps
    // to `delta / (pfl.scale * s)` ref-px, or the widget out-runs the cursor on
    // the shrunk Widgets tab.
    let stage_s =
        crate::stage_rect::stage_xform(*rect, Vec2::new(window.width(), window.height())).0;
    let drag_scale = pfl.scale * stage_s;
    match gesture.0 {
        Gesture::None => {}
        Gesture::Move { last_cursor } => {
            let delta = pos - last_cursor;
            if delta != Vec2::ZERO {
                if let Some(inst) = layouts.0.get_mut(&kind) {
                    inst.offset = apply_drag(inst.offset, delta, drag_scale);
                }
            }
            gesture.0 = Gesture::Move { last_cursor: pos };
        }
        Gesture::Scale {
            start_dist,
            start_scale,
            start_center,
        } => {
            // Fixed reference center (captured at press) — never the live AABB,
            // which scaling would move and feed back into the distance.
            let dist = (pos - start_center).length().max(1.0);
            let next = clamp_scale(start_scale * dist / start_dist);
            if let Some(inst) = layouts.0.get_mut(&kind) {
                if (inst.scale - next).abs() > f32::EPSILON {
                    inst.scale = next;
                }
            }
        }
    }
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
        assert_eq!(
            apply_drag((0.0, 0.0), Vec2::new(5.0, -7.0), 1.0),
            (5.0, -7.0)
        );
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

    #[test]
    fn ensure_anchored_preserves_position() {
        let mut inst = dtx_layout::default_instance(dtx_layout::WidgetKind::Combo);
        let parent = (0.0, 0.0, 1280.0, 720.0);
        let visual = Vec2::new(831.0, 72.0);
        let size = Vec2::new(140.0, 60.0);
        ensure_anchored(&mut inst, visual, size, parent, 1.0);
        assert_eq!(inst.placement, dtx_layout::Placement::Anchored);
        let tl = dtx_layout::resolve_top_left(
            inst.anchor,
            inst.origin,
            (size.x, size.y),
            inst.scale,
            (inst.offset.0, inst.offset.1),
            parent,
        );
        assert!((tl.0 - visual.x).abs() < 0.001 && (tl.1 - visual.y).abs() < 0.001);
    }
}
