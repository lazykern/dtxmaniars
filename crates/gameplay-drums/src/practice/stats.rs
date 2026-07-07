//! Per-attempt section stats: accumulate judgements between seeks.
//!
//! An attempt spans seek-to-seek. On each `SeekToChartTime` the running
//! attempt is finalized into history and a fresh one starts at the
//! seek's `attempt_start_ms`. Pre-roll chips (judged before the attempt
//! start in chart time) are excluded.
//!
//! Runs after `apply_seek_system` (via `.after(judge_lane_hit_system)`,
//! which is itself transitively after `apply_seek_system`), so by the
//! time this runs the clock already holds the post-seek position. The
//! finished attempt's end point is instead read from
//! [`crate::seek::LastSeekFrom`], which `apply_seek_system` sets to the
//! pre-seek clock value before jumping.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use game_shell::AppState;

use super::session::PracticeSession;
use crate::events::{EmptyHit, JudgmentEvent, NoteMissed};
use crate::resources::{Combo, GameplayClock};
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        track_attempt_stats
            .after(crate::judge::judge_lane_hit_system)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}

/// Fold one judgement into the attempt (pure; unit-tested).
pub fn apply_judgment(
    attempt: &mut super::session::AttemptStats,
    kind: JudgmentKind,
    delta_ms: i64,
) {
    match kind {
        JudgmentKind::Perfect => attempt.counts.perfect += 1,
        JudgmentKind::Great => attempt.counts.great += 1,
        JudgmentKind::Good => attempt.counts.good += 1,
        JudgmentKind::Poor => attempt.counts.ok += 1,
        JudgmentKind::Miss => attempt.counts.miss += 1,
    }
    if kind == JudgmentKind::Miss {
        attempt.combo = 0;
    } else {
        attempt.combo += 1;
        attempt.max_combo = attempt.max_combo.max(attempt.combo);
        attempt.error_sum_ms += delta_ms;
        attempt.error_count += 1;
    }
}

pub fn track_attempt_stats(
    mut judgments: MessageReader<JudgmentEvent>,
    mut missed: MessageReader<NoteMissed>,
    mut empty_hits: MessageReader<EmptyHit>,
    mut seeks: MessageReader<SeekToChartTime>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut last_seek_from: ResMut<crate::seek::LastSeekFrom>,
    mut session: ResMut<PracticeSession>,
    mut combo: ResMut<Combo>,
) {
    for ev in judgments.read() {
        let judge_ms = timeline
            .judge_ms_by_idx
            .get(ev.chip_idx)
            .copied()
            .unwrap_or(i64::MIN);
        if judge_ms < session.current_attempt.start_ms {
            continue; // pre-roll chip: audible feedback only
        }
        apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms);
    }
    for m in missed.read() {
        let judge_ms = timeline
            .judge_ms_by_idx
            .get(m.chip_idx)
            .copied()
            .unwrap_or(i64::MIN);
        if judge_ms < session.current_attempt.start_ms {
            continue; // pre-roll chip: audible feedback only
        }
        session.current_attempt.counts.miss += 1;
        session.current_attempt.combo = 0;
    }
    for _ in empty_hits.read() {
        session.current_attempt.overhits += 1;
    }
    if let Some(seek) = seeks.read().last() {
        // Pre-seek clock, captured by apply_seek_system earlier this tick.
        let end_ms = last_seek_from.0.take().unwrap_or(clock.current_ms);
        let next_start = seek.attempt_start_ms.unwrap_or(seek.target_ms);
        session.roll_attempt(end_ms, next_start);
        // Fresh attempt = fresh visible combo.
        combo.current = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::AttemptStats;

    #[test]
    fn hits_accumulate_counts_combo_and_error() {
        let mut a = AttemptStats::default();
        apply_judgment(&mut a, JudgmentKind::Perfect, -5);
        apply_judgment(&mut a, JudgmentKind::Great, 20);
        apply_judgment(&mut a, JudgmentKind::Perfect, -15);
        assert_eq!(a.counts.perfect, 2);
        assert_eq!(a.counts.great, 1);
        assert_eq!(a.max_combo, 3);
        assert_eq!(a.error_count, 3);
        assert_eq!(a.error_sum_ms, 0);
    }

    #[test]
    fn miss_resets_combo_and_skips_error() {
        let mut a = AttemptStats::default();
        apply_judgment(&mut a, JudgmentKind::Perfect, 0);
        apply_judgment(&mut a, JudgmentKind::Miss, 400);
        apply_judgment(&mut a, JudgmentKind::Perfect, 0);
        assert_eq!(a.counts.miss, 1);
        assert_eq!(a.combo, 1);
        assert_eq!(a.max_combo, 1);
        assert_eq!(a.error_count, 2, "miss delta must not pollute mean error");
    }
}
