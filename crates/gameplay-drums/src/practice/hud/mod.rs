//! Two-tier practice HUD: quick tier (mini strip, chip, toasts) during
//! play, full HUD (timeline + right rail) on the **Tab** pause tier
//! (`PracticePauseSurface::Rail`); Esc pauses get the standard overlay.
//! Fixed overlay — deliberately NOT a dtx-layout widget (no
//! editor-pillar dependency).

pub mod chip;
pub mod full_hud;
pub mod mini_strip;
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

/// Run condition: the practice rail owns the current pause (Tab opener).
pub fn rail_surface_active(surface: Res<crate::pause::PracticePauseSurface>) -> bool {
    *surface == crate::pause::PracticePauseSurface::Rail
}

/// Exposed `pub` (not `pub(super)`) so integration tests can build the real
/// HUD plugin schedule headlessly; see `tests/practice_hud.rs`.
pub fn plugin(app: &mut App) {
    use game_shell::{AppState, PauseState};
    mini_strip::plugin(app);
    chip::plugin(app);
    wait_prompt::plugin(app);
    app.init_resource::<full_hud::RailSelection>()
        .init_resource::<crate::pause::PracticePauseSurface>()
        .init_resource::<timeline_ui::TimelineGesture>()
        .init_resource::<crate::practice::toast::ToastQueue>()
        .add_systems(
            OnEnter(PauseState::Paused),
            full_hud::spawn_full_hud
                .run_if(resource_exists::<crate::practice::PracticeSession>)
                .run_if(rail_surface_active),
        )
        .add_systems(OnExit(PauseState::Paused), full_hud::despawn_full_hud)
        .add_systems(
            Update,
            (
                timeline_ui::timeline_mouse,
                full_hud::full_hud_input,
                full_hud::rail_mouse,
                full_hud::transport_buttons,
                full_hud::refresh_rail,
                full_hud::update_full_hud_markers,
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<crate::practice::PracticeSession>)
                .run_if(rail_surface_active),
        );
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
