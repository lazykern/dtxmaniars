//! A/B loop: when the clock passes B, seek back to A (with pre-roll).
//! With no explicit region armed, practice loops the whole song
//! implicitly — the stage never "ends" in practice.

use bevy::prelude::*;
use game_shell::{AppState, PauseState};

use super::session::{preroll_target, LoopRegion, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// One loop pass finished: the wrap seek was just emitted. The ONLY
/// trigger for ramp decisions and wrap reports — manual seeks and
/// restarts never produce this.
#[derive(Message, Debug, Clone, Copy)]
pub struct PracticeLoopCompleted {
    pub region_start_ms: i64,
    pub region_end_ms: i64,
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<PracticeLoopCompleted>().add_systems(
        FixedUpdate,
        loop_watcher
            .before(crate::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}

/// The region practice is looping right now: the armed A/B region, or
/// the whole song when none is set (A-only regions count as unset).
pub fn active_region(session: &PracticeSession, timeline: &ChipTimeline) -> LoopRegion {
    session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .unwrap_or(LoopRegion {
            start_ms: 0,
            end_ms: timeline.end_ms,
        })
}

pub fn loop_watcher(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut completed: MessageWriter<PracticeLoopCompleted>,
) {
    if !clock.is_ready() || timeline.end_ms <= 0 {
        return;
    }
    let region = active_region(&session, &timeline);
    if clock.current_ms >= region.end_ms {
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.transport.preroll, region.start_ms),
            snap: None,
            attempt_start_ms: Some(region.start_ms),
        });
        completed.write(PracticeLoopCompleted {
            region_start_ms: region.start_ms,
            region_end_ms: region.end_ms,
        });
    }
}
