//! Persist / reset editor edits.
//!
//! Widget saves stay separate from profile transactions: `layout.toml` gets
//! the `[scene]` plus a NON-authoritative snapshot of the last committed
//! lane profile (compatibility only — startup ignores it whenever
//! `lane-profiles.toml` exists). Unsaved lane profile drafts are never
//! written here and their dirty state is untouched.

use bevy::prelude::*;
use dtx_layout::{LaneArrangement, LayoutFile, LATEST_VERSION};

use crate::widget_layout::WidgetLayouts;

use super::profile_state::CustomizeSession;

/// Build a `LayoutFile` from the widget scene and the committed active lane
/// arrangement (compatibility snapshot).
pub fn layout_file_from(layouts: &WidgetLayouts, active_lanes: &LaneArrangement) -> LayoutFile {
    LayoutFile {
        version: LATEST_VERSION,
        lanes: dtx_layout::LanesSection::from_arrangement(active_lanes),
        scene: dtx_layout::SceneSection::from_map(&layouts.0),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            save_hotkey
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
            save_layout_on_close,
        ),
    );
}

/// Layout auto-saves when the surface closes (EditorOpen true→false while still
/// in Performance — the Esc route), matching the config auto-save contract.
/// The song-ended route is covered by `close_editor_on_exit`. Snapshots the
/// last committed lane profile, never the unsaved preview draft.
fn save_layout_on_close(
    open: Res<super::EditorOpen>,
    layouts: Res<WidgetLayouts>,
    session: Res<CustomizeSession>,
    state: Res<State<game_shell::AppState>>,
    mut was_open: Local<bool>,
) {
    if !super::should_persist_close(
        open.0,
        *state.get() == game_shell::AppState::Performance,
        &mut was_open,
    ) {
        return;
    }
    let file = layout_file_from(&layouts, &session.0.lanes.saved.arrangement);
    if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
        warn!("layout auto-save failed: {e}");
    }
}

/// Ctrl+S writes layout.toml. Never commits profile drafts.
fn save_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    layouts: Res<WidgetLayouts>,
    session: Res<CustomizeSession>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        let file = layout_file_from(&layouts, &session.0.lanes.saved.arrangement);
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

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::WidgetKind;

    #[test]
    fn save_file_round_trips_through_resolve() {
        let mut layouts = WidgetLayouts::default();
        layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (12.0, 34.0);
        let file = layout_file_from(&layouts, &dtx_layout::classic());
        assert_eq!(
            file.scene.resolve()[&WidgetKind::Combo].offset,
            (12.0, 34.0)
        );
    }

    #[test]
    fn reset_widget_restores_default() {
        let mut layouts = WidgetLayouts::default();
        layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (9.0, 9.0);
        reset_widget(&mut layouts, WidgetKind::Combo);
        assert_eq!(layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
    }
}
