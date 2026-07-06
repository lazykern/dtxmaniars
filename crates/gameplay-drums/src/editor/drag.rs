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
/// the cursor delta ÷ scale. Pushes one undo snapshot per completed drag.
fn drag_selected_widget(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selection: Res<Selection>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    mut cursor: ResMut<DragCursor>,
    mut layouts: ResMut<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
    lanes: Res<crate::lanes::Lanes>,
    mut dragging: Local<bool>,
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
        *dragging = false;
        cursor.0 = None;
        return;
    }

    if !*dragging {
        // Drag just started: snapshot BEFORE the move so undo restores pre-drag.
        *dragging = true;
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
