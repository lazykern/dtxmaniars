//! Practice mode: seek/scrub, A/B loop, playback rate, attempt stats.
//!
//! `PracticeSession` present = practice; absent = normal play with zero
//! behavior change. Inserted on Performance enter when
//! `game_shell::PracticeIntent` is set, removed on returning to song
//! select (it must survive StageClear/Result so the save gate sees it).

pub mod ab_loop;
pub mod actions;
pub mod diagnosis;
pub mod hud;
pub mod metronome;
pub mod ramp;
pub mod rate;
pub mod session;
pub mod stats;
pub mod toast;
pub mod wait;

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
        metronome::plugin,
        ramp::plugin,
        rate::plugin,
        stats::plugin,
        wait::plugin,
    ));
}

fn enter_practice_session(
    intent: Res<PracticeIntent>,
    mut commands: Commands,
    mut wait_state: ResMut<wait::WaitState>,
    mut chord_hits: ResMut<wait::ChordHitTimes>,
    mut deferred: ResMut<wait::DeferredWaitJudgments>,
) {
    wait_state.phase = wait::WaitPhase::Flowing;
    wait_state.waited_chips.clear();
    wait_state.enabled_from_ms = None;
    chord_hits.0.clear();
    deferred.0.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::wait::{WaitSet, WaitState};

    #[test]
    fn entering_normal_play_clears_wait_halt() {
        let mut app = App::new();
        app.insert_resource(PracticeIntent(false));
        app.insert_resource(WaitState {
            phase: wait::WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![7],
            }),
            waited_chips: [7].into(),
            ..default()
        });
        app.init_resource::<wait::ChordHitTimes>();
        app.init_resource::<wait::DeferredWaitJudgments>();
        app.add_systems(Update, enter_practice_session);
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(!state.halted());
        assert!(state.waited_chips.is_empty());
        assert!(!app.world().contains_resource::<PracticeSession>());
    }
}
