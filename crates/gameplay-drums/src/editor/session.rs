//! Editor-session runtime: force-open on Performance enter, seamless
//! chart-end loop (seek back to 0 instead of Results), exit-to-title.

use bevy::prelude::*;
use game_shell::{AppState, EditorSession, PauseState};

use crate::orchestrator::DrumsStageCompletion;
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), force_open_for_session)
        .add_systems(
            FixedUpdate,
            session_loop_watcher
                .before(crate::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(session_active),
        );
}

pub fn session_active(session: Res<EditorSession>) -> bool {
    session.0
}

/// Entering Performance in a session: editor opens immediately, autoplay on.
fn force_open_for_session(
    session: Res<EditorSession>,
    mut open: ResMut<super::EditorOpen>,
    mut prev: ResMut<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if !session.0 {
        return;
    }
    prev.0 = autoplay.0;
    autoplay.0 = true;
    open.0 = true;
}

/// Past chart end → seek to 0 (same mechanism as the practice A/B loop);
/// the orchestrator's StageClear transition is gated off during a session.
fn session_loop_watcher(
    clock: Res<GameplayClock>,
    completion: Res<DrumsStageCompletion>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    if !clock.is_ready() || completion.chart_end_ms <= 0 {
        return;
    }
    if clock.current_ms >= completion.chart_end_ms {
        seeks.write(SeekToChartTime {
            target_ms: 0,
            snap: None,
            attempt_start_ms: Some(0),
        });
    }
}
