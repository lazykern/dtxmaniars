//! In-Performance layout editor overlay. Inert unless opened (Ctrl+Shift+E).
//!
//! Opening force-enables autoplay (notes flow hands-free), gates drum input +
//! pause, and spawns the sidebar. Closing restores the prior autoplay flag and
//! despawns the UI. All mutation targets `WidgetLayouts` / `Lanes`, which the
//! HUD already reacts to (plan 1 + 2).

use bevy::prelude::*;
use game_shell::AppState;

pub mod drag;
pub mod hotkeys;
pub mod panel;
pub mod picking;
pub mod save;
pub mod selection_box;
pub mod session;
pub mod settings_data;
pub mod snap;
pub mod stage;
pub mod tabs;
pub mod ui;
pub mod undo;

/// True while the editor overlay is open. Default false — normal play/practice.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorOpen(pub bool);

/// Remembers the autoplay flag from before the editor forced it on.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PrevAutoplay(pub bool);

/// Ordering: picking (AABBs/hover) → gestures (drag) → overlay sync.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorPickSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorGestureSet;

pub fn plugin(app: &mut App) {
    app.init_resource::<EditorOpen>()
        .init_resource::<PrevAutoplay>()
        .init_resource::<drag::Selection>()
        .init_resource::<undo::UndoStack>()
        .add_systems(
            Update,
            toggle_editor
                .run_if(in_state(AppState::Performance))
                .run_if(|s: Res<game_shell::EditorSession>| !s.0),
        )
        .add_systems(OnExit(AppState::Performance), close_editor_on_exit)
        .configure_sets(Update, (EditorPickSet, EditorGestureSet).chain())
        .add_plugins((
            drag::plugin,
            hotkeys::plugin,
            undo::plugin,
            save::plugin,
            ui::plugin,
            picking::plugin,
            selection_box::plugin,
            panel::plugin,
            snap::plugin,
            stage::plugin,
            session::plugin,
            tabs::plugin,
        ));
}

/// Leaving Performance with the editor still open (e.g. the song ended mid-edit)
/// must restore autoplay and clear `EditorOpen`, else the next song starts with
/// drum input + pause dead and no sidebar (the sidebar despawn is in ui.rs).
fn close_editor_on_exit(
    mut open: ResMut<EditorOpen>,
    prev: Res<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
    mut selection: ResMut<drag::Selection>,
    mut session: ResMut<game_shell::EditorSession>,
) {
    if open.0 {
        autoplay.0 = prev.0;
        open.0 = false;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
    selection.0 = None;
    // Covers non-Esc exits (song ended, forced transition): a stale session
    // flag would make the next Performance force-open the editor.
    session.0 = false;
}

/// Ctrl+Shift+E toggles the editor while in Performance.
fn toggle_editor(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<EditorOpen>,
    mut prev: ResMut<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut selection: ResMut<drag::Selection>,
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
            selection.0 = None;
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
