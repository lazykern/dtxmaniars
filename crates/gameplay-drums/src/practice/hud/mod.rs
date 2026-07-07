//! Two-tier practice HUD: quick tier (mini strip, chip, toasts) during
//! play, full HUD (timeline + right rail) while paused. Fixed overlay —
//! deliberately NOT a dtx-layout widget (no editor-pillar dependency).

pub mod full_hud;
pub mod timeline_ui;

use bevy::prelude::*;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

pub(super) fn plugin(app: &mut App) {
    use game_shell::{AppState, PauseState};
    app.init_resource::<full_hud::RailSelection>()
        .init_resource::<full_hud::ExitArmed>()
        .add_systems(
            OnEnter(PauseState::Paused),
            full_hud::spawn_full_hud.run_if(resource_exists::<crate::practice::PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), full_hud::despawn_full_hud)
        .add_systems(
            Update,
            (full_hud::full_hud_input, full_hud::update_full_hud_markers)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<crate::practice::PracticeSession>),
        );
}
