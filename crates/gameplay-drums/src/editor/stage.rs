//! Customize stage-transform presets: map ActiveTab → target StageRect.

use crate::stage_rect::{StageRect, StageTarget};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shell::CustomizeTab;

/// Left sidebar width (editor/ui.rs) and right panel width (editor/panel.rs).
const LEFT_CHROME: f32 = 220.0;
const RIGHT_CHROME: f32 = 240.0;
const TOP_MARGIN: f32 = 24.0;

/// Preset rect for a tab given the window size.
pub fn preset_rect(tab: CustomizeTab, window: Vec2) -> StageRect {
    if tab.is_settings() {
        // Offset: true scale, playfield shifted into the gap right of the rail.
        StageRect {
            origin: Vec2::new(LEFT_CHROME, 0.0),
            size: window,
        }
    } else {
        // Fit: shrink whole screen into the gap between both chrome panels.
        StageRect {
            origin: Vec2::new(LEFT_CHROME, TOP_MARGIN),
            size: Vec2::new(
                (window.x - LEFT_CHROME - RIGHT_CHROME).max(1.0),
                (window.y - 2.0 * TOP_MARGIN).max(1.0),
            ),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, set_stage_target.run_if(super::editor_open));
}

/// While the surface is open, drive the target rect from the active tab.
fn set_stage_target(
    active: Res<super::tabs::ActiveTab>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut target: ResMut<StageTarget>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let want = preset_rect(active.0, Vec2::new(win.width(), win.height()));
    if target.0 != want {
        target.0 = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tab_is_offset_true_scale() {
        let r = preset_rect(CustomizeTab::Gameplay, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(220.0, 0.0));
        assert_eq!(r.size, Vec2::new(1600.0, 900.0)); // scale 1 preserved
    }

    #[test]
    fn kit_tab_is_fit_between_chrome() {
        let r = preset_rect(CustomizeTab::Widgets, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(220.0, 24.0));
        assert_eq!(r.size, Vec2::new(1600.0 - 220.0 - 240.0, 900.0 - 48.0));
    }
}
