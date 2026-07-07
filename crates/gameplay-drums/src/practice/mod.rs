//! Practice mode: seek/scrub, A/B loop, playback rate, attempt stats.
//!
//! `PracticeSession` present = practice; absent = normal play with zero
//! behavior change. Inserted on Performance enter when
//! `game_shell::PracticeIntent` is set, removed on returning to song
//! select (it must survive StageClear/Result so the save gate sees it).

pub mod ab_loop;
pub mod actions;
pub mod hud;
pub mod ramp;
pub mod rate;
pub mod session;
pub mod stats;
pub mod toast;

use bevy::prelude::*;
use game_shell::{AppState, PracticeIntent};

pub use session::PracticeSession;

use crate::gauge::StageGauge;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<actions::PracticeBindings>()
        .add_message::<actions::PracticeAction>()
        .add_systems(
            Update,
            (
                actions::emit_practice_actions,
                actions::apply_practice_actions,
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        );
    app.init_resource::<toast::ToastQueue>().add_systems(
        Update,
        toast::toast_ui
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
    app.add_systems(
        OnEnter(AppState::Performance),
        enter_practice_session.before(crate::orchestrator::DrumsEnterSet),
    )
    .add_systems(OnEnter(AppState::SongSelect), remove_practice_session)
    .add_systems(
        FixedUpdate,
        freeze_gauge_in_practice
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_plugins((
        ab_loop::plugin,
        hud::plugin,
        ramp::plugin,
        rate::plugin,
        stats::plugin,
    ));
}

fn enter_practice_session(intent: Res<PracticeIntent>, mut commands: Commands) {
    if intent.0 {
        commands.insert_resource(PracticeSession::default());
    } else {
        commands.remove_resource::<PracticeSession>();
    }
}

fn remove_practice_session(mut commands: Commands) {
    commands.remove_resource::<PracticeSession>();
}

/// Gauge is meaningless in practice: pin it full so it can never fail
/// the stage and the HUD reads as neutral.
fn freeze_gauge_in_practice(mut gauge: ResMut<StageGauge>) {
    gauge.value = 1.0;
    gauge.failed = false;
}
