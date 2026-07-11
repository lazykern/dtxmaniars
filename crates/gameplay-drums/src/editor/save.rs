//! Persist / reset editor edits, and cycle lane presets.

use bevy::prelude::*;
use dtx_layout::{LATEST_VERSION, LayoutFile};

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
        (
            save_hotkey
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
            save_layout_on_close,
        ),
    );
}

/// Layout auto-saves when the surface closes (EditorOpen true→false while still
/// in Performance — the Esc route), matching the config/bindings auto-save
/// contract. The song-ended route is covered by `close_editor_on_exit`.
fn save_layout_on_close(
    open: Res<super::EditorOpen>,
    layouts: Res<WidgetLayouts>,
    lanes: Res<Lanes>,
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
    let file = layout_file_from(&layouts, &lanes);
    if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
        warn!("layout auto-save failed: {e}");
    }
}

/// Ctrl+S writes layout.toml.
fn save_hotkey(keys: Res<ButtonInput<KeyCode>>, layouts: Res<WidgetLayouts>, lanes: Res<Lanes>) {
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
