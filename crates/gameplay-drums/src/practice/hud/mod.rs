//! Practice HUD: the setup/settings shell plus the compact running overlays.

pub mod chip;
pub mod mini_strip;
pub mod progress;
pub mod setup;
pub mod timeline_ui;
pub mod wait_prompt;

use bevy::prelude::*;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

/// Exposed `pub` (not `pub(super)`) so integration tests can build the real
/// HUD plugin schedule headlessly; see `tests/practice_hud.rs`.
pub fn plugin(app: &mut App) {
    use game_shell::AppState;
    mini_strip::plugin(app);
    chip::plugin(app);
    wait_prompt::plugin(app);
    app.init_resource::<setup::PracticeTab>()
        .init_resource::<timeline_ui::TimelineGesture>()
        .init_resource::<crate::practice::toast::ToastQueue>()
        .add_systems(
            Update,
            (
                setup::ensure_setup_shell,
                setup::update_tab_selection,
                timeline_ui::timeline_mouse.run_if(crate::practice::practice_surface_open),
                timeline_ui::preview_transport_buttons
                    .run_if(crate::practice::practice_surface_open),
                timeline_ui::update_timeline_markers.run_if(crate::practice::practice_surface_open),
                timeline_ui::update_transport_label.run_if(crate::practice::practice_surface_open),
                setup::refresh_setup_copy.run_if(crate::practice::practice_surface_open),
                progress::refresh_progress_copy.run_if(crate::practice::practice_surface_open),
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<crate::practice::PracticeFlow>),
        )
        .add_systems(OnExit(AppState::Performance), setup::despawn_setup_shell)
        .add_systems(
            Update,
            sync_compact_hud_visibility
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<crate::practice::PracticeFlow>),
        );
}

fn sync_compact_hud_visibility(
    flow: Res<crate::practice::PracticeFlow>,
    mut compact: Query<
        &mut Visibility,
        Or<(With<mini_strip::MiniStripRoot>, With<chip::StatusChip>)>,
    >,
) {
    let visibility = if flow.phase == crate::practice::PracticePhase::Running {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut current in &mut compact {
        *current = visibility;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_time_formats_minutes_seconds_tenths() {
        assert_eq!(format_chart_time(0), "0:00.0");
        assert_eq!(format_chart_time(83_450), "1:23.4");
        assert_eq!(format_chart_time(-50), "0:00.0");
    }
}
