//! In-Performance layout editor overlay. Opens only via an editor session
//! (Customize entry from Title/SongSelect); never toggleable mid-gameplay.
//!
//! Opening force-enables autoplay (notes flow hands-free), gates drum input +
//! pause, and spawns the sidebar. Closing restores the prior autoplay flag and
//! despawns the UI. All mutation targets `WidgetLayouts` / `Lanes`, which the
//! HUD already reacts to (plan 1 + 2).

use bevy::prelude::*;
use game_shell::AppState;

pub mod bindings_capture;
pub mod bindings_panel;
pub mod bindings_spatial;
pub mod calibration;
pub mod chrome;
pub mod drag;
pub mod footer;
pub mod hotkeys;
pub mod keyboard_nav;
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

/// Request to close the overlay through the same save-on-close path as Esc.
#[derive(Debug, Clone, Copy, Message)]
pub struct EditorCloseRequest;

/// Remembers the autoplay flag from before the editor forced it on.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PrevAutoplay(pub bool);

/// Single source of truth for the Customize preview's frame state. Computed
/// once per frame (before the editor sets); systems read this instead of
/// re-deriving open/peek/tab/inspector themselves.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PreviewState {
    pub open: bool,
    /// Tab held: full play view peek (chrome + overlays hidden, identity rect).
    pub peeking: bool,
    pub tab: game_shell::CustomizeTab,
    /// Widgets tab with a live selection → right inspector reserves space.
    pub has_inspector: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            open: false,
            peeking: false,
            // Mirrors `tabs::ActiveTab::default()` (Widgets landing).
            tab: game_shell::CustomizeTab::Widgets,
            has_inspector: false,
        }
    }
}

fn update_preview_state(
    open: Res<EditorOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<tabs::ActiveTab>,
    selection: Res<drag::Selection>,
    mut state: ResMut<PreviewState>,
) {
    let next = PreviewState {
        open: open.0,
        peeking: open.0 && keys.pressed(KeyCode::Tab),
        tab: active.0,
        has_inspector: active.0 == game_shell::CustomizeTab::Widgets && selection.0.is_some(),
    };
    if *state != next {
        *state = next;
    }
}

/// Ordering: picking (AABBs/hover) → gestures (drag) → overlay sync.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorPickSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorGestureSet;

pub fn plugin(app: &mut App) {
    app.add_message::<EditorCloseRequest>()
        .init_resource::<EditorOpen>()
        .init_resource::<PrevAutoplay>()
        .init_resource::<PreviewState>()
        .init_resource::<drag::Selection>()
        .init_resource::<undo::UndoStack>()
        .add_systems(
            Update,
            (
                update_preview_state,
                clear_canvas_interaction_outside_widgets.run_if(editor_open),
            )
                .before(EditorPickSet)
                .run_if(in_state(AppState::Performance)),
        )
        .add_systems(OnExit(AppState::Performance), close_editor_on_exit)
        .configure_sets(Update, (EditorPickSet, EditorGestureSet).chain())
        .add_plugins((
            (
                bindings_panel::plugin,
                bindings_capture::plugin,
                bindings_spatial::plugin,
                drag::plugin,
            ),
            hotkeys::plugin,
            keyboard_nav::plugin,
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
            footer::plugin,
            calibration::plugin,
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
    layouts: Res<crate::widget_layout::WidgetLayouts>,
    lanes: Res<crate::lanes::Lanes>,
    draft: Res<tabs::ConfigDraft>,
    live_bindings: Res<crate::bindings::LiveBindings>,
    mut perf_draft: ResMut<crate::perf_hotkeys::PerfHotkeyDraft>,
    show_perf_info: Res<crate::resources::ShowPerfInfo>,
) {
    if open.0 {
        let file = save::layout_file_from(&layouts, &lanes);
        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
            warn!("layout save on exit failed: {e}");
        }
        if let Err(e) = dtx_config::save(&dtx_config::default_path(), &draft.0) {
            warn!("config save on exit failed: {e}");
        }
        if let Err(e) =
            dtx_config::save_bindings(&dtx_config::default_bindings_path(), &live_bindings.0)
        {
            warn!("bindings save on exit failed: {e}");
        }
        perf_draft.sync_from_editor(&draft.0, show_perf_info.0);
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

fn clear_canvas_interaction_outside_widgets(
    active: Res<tabs::ActiveTab>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
) {
    if active.0 == game_shell::CustomizeTab::Widgets {
        return;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
}

pub(super) fn just_closed(open: bool, was_open: &mut bool) -> bool {
    let closed = *was_open && !open;
    *was_open = open;
    closed
}

pub(super) fn should_persist_close(open: bool, in_performance: bool, was_open: &mut bool) -> bool {
    just_closed(open, was_open) && in_performance
}

/// Run condition: editor is open.
pub fn editor_open(open: Res<EditorOpen>) -> bool {
    open.0
}

/// Run condition: the Widgets layout tab is active.
pub fn widgets_tab_active(active: Res<tabs::ActiveTab>) -> bool {
    active.0 == game_shell::CustomizeTab::Widgets
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

    #[test]
    fn initial_closed_state_is_not_a_close_transition() {
        let mut was_open = false;
        assert!(!just_closed(false, &mut was_open));
    }

    #[test]
    fn open_to_closed_is_a_close_transition() {
        let mut was_open = false;
        assert!(!just_closed(true, &mut was_open));
        assert!(just_closed(false, &mut was_open));
        assert!(!just_closed(false, &mut was_open));
    }

    #[test]
    fn forced_exit_consumes_close_outside_performance() {
        let mut was_open = false;
        assert!(!should_persist_close(true, true, &mut was_open));
        assert!(!should_persist_close(false, false, &mut was_open));
        assert!(!should_persist_close(false, true, &mut was_open));
    }
}
