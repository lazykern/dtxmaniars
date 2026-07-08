//! Editor action hotkeys: Undo (Ctrl+Z) and Redo (Ctrl+Y).
//!
//! These bodies were ported verbatim from the former rail action buttons
//! (`editor/ui.rs` `handle_buttons`) when the rail became tabs-only. Save
//! (Ctrl+S) is intentionally NOT re-implemented here — it already lives in
//! `editor/save.rs` `save_hotkey`; duplicating it would double-write the file.

use bevy::prelude::*;

use super::undo::UndoStack;
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        editor_action_hotkeys
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open),
    );
}

/// Ctrl+Z undoes, Ctrl+Y redoes. Bodies copied verbatim from the old
/// `handle_buttons` Undo/Redo arms (Ctrl+arrows are the perf hotkeys, no clash).
fn editor_action_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut layouts: ResMut<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
    mut stack: ResMut<UndoStack>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        let snap = super::undo::Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        };
        if let Some(s) = stack.undo(snap) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    }
    if keys.just_pressed(KeyCode::KeyY) {
        let snap = super::undo::Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        };
        if let Some(s) = stack.redo(snap) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    }
}
