//! A/B loop: when the clock passes B, seek back to A (with pre-roll).

use bevy::prelude::*;
use game_shell::{AppState, PauseState};

use super::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        loop_watcher
            .before(crate::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}

pub fn loop_watcher(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    if !clock.is_ready() {
        return;
    }
    let Some(region) = session.transport.loop_region else {
        return;
    };
    if region.end_ms == i64::MAX {
        return; // only A set — not armed yet
    }
    if clock.current_ms >= region.end_ms {
        let target = preroll_target(&timeline, session.transport.preroll, region.start_ms);
        seeks.write(SeekToChartTime {
            target_ms: target,
            snap: None,
            attempt_start_ms: Some(region.start_ms),
        });
    }
}
