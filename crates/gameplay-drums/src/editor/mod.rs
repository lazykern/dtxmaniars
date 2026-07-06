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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_open_default_false() {
        assert!(!EditorOpen::default().0);
    }
}
