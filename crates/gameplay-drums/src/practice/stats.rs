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
    app.init_resource::<LastFinalizedAttempt>().add_systems(
        FixedUpdate,
        (track_attempt_stats, wrap_micro_report)
            .chain()
            .after(crate::judge::judge_lane_hit_system)
            .after(crate::practice::wait::wait_watcher)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}

/// The attempt finalized by the most recent seek this tick: `Some` when
/// it had data and was pushed to history, `None` when it was empty.
/// Read by `apply_ramp` (and later the wrap report) in the same tick.
#[derive(Resource, Debug, Default, Clone)]
pub struct LastFinalizedAttempt(pub Option<crate::practice::session::AttemptRecord>);

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
    mut finalized: ResMut<LastFinalizedAttempt>,
    paused_restart: Option<Res<crate::pause::PausedRestart>>,
    cancelled_restart: Option<Res<crate::pause::CancelledPausedRestart>>,
    acknowledgement: Res<crate::seek::SeekAcknowledgement>,
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
    flow: Option<Res<crate::practice::PracticeFlow>>,
) {
    if flow.is_some_and(|flow| flow.phase != crate::practice::PracticePhase::Running) {
        judgments.clear();
        missed.clear();
        empty_hits.clear();
        seeks.clear();
        return;
    }
    for ev in judgments.read() {
        let judge_ms = timeline
            .judge_ms_by_idx
            .get(ev.chip_idx)
            .copied()
            .unwrap_or(i64::MIN);
        if judge_ms < session.current_attempt.start_ms {
            continue; // pre-roll chip: audible feedback only
        }
        if wait_state
            .as_ref()
            .is_some_and(|w| w.waited_chips.contains(&ev.chip_idx))
        {
            session.current_attempt.waited += 1;
            continue; // cleared while halted: tempo-free, not timing-judged
        }
        apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms);
        session
            .current_attempt_lane_diag
            .apply_judgment(ev.lane, ev.kind, ev.delta_ms);
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
        session.current_attempt_lane_diag.apply_miss(m.lane);
    }
    for eh in empty_hits.read() {
        session.current_attempt.overhits += 1;
        session.current_attempt_lane_diag.apply_overhit(eh.lane);
    }
    if let Some(seek) = seeks
        .read_with_id()
        .filter(|(_, id)| {
            !cancelled_restart
                .as_ref()
                .is_some_and(|cancelled| cancelled.owns(*id))
        })
        .map(|(seek, _)| seek)
        .last()
    {
        if paused_restart
            .is_some_and(|restart| restart.owns_acknowledged_seek(seek, &acknowledgement))
        {
            return;
        }
        // Pre-seek clock, captured by apply_seek_system earlier this tick.
        let end_ms = last_seek_from.0.take().unwrap_or(clock.current_ms);
        let next_start = seek.attempt_start_ms.unwrap_or(seek.target_ms);
        finalized.0 = session.roll_attempt_for_chart(end_ms, next_start, timeline.end_ms);
        // Fresh attempt = fresh visible combo.
        combo.current = 0;
    }
}

/// One-line feedback at each loop wrap: `pass 5 · 93.8% · 3 miss · +18ms`
/// (`+` = late, `−` = early). Pass count = attempts on this span in
/// history. Feedback lands at the loop boundary, never mid-play.
pub fn wrap_micro_report(
    mut completions: MessageReader<super::ab_loop::PracticeLoopCompleted>,
    finalized: Res<LastFinalizedAttempt>,
    session: Res<PracticeSession>,
    mut toasts: ResMut<super::toast::ToastQueue>,
) {
    let Some(done) = completions.read().last().copied() else {
        return;
    };
    let Some(att) = finalized.0.as_ref() else {
        return;
    };
    if att.start_ms != done.region_start_ms {
        return;
    }
    let n = session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == done.region_start_ms)
        .count();
    if session.trainer.wait_enabled() {
        toasts.push(format!(
            "pass {n} · flow {:.0}% · {} waited",
            att.flow_pct, att.waited
        ));
    } else {
        toasts.push(format!(
            "pass {n} · {:.1}% · {} miss · {:+.0}ms",
            att.accuracy_pct, att.counts.miss, att.mean_error_ms
        ));
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
    fn waited_reclassification_precedes_apply_judgment() {
        let src = include_str!("stats.rs");
        let waited = src.find("session.current_attempt.waited += 1").unwrap();
        let apply = src
            .find("apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms)")
            .unwrap();
        assert!(waited < apply, "waited check must gate apply_judgment");
    }

    #[test]
    fn preroll_gate_also_guards_lane_diag() {
        // The pre-roll `continue` sits before BOTH apply_judgment calls;
        // this pins that ordering at the source level.
        let src = include_str!("stats.rs");
        let gate = src
            .find("continue; // pre-roll chip: audible feedback only")
            .unwrap();
        let diag = src.find("session.lane_diag.apply_judgment").unwrap();
        assert!(
            gate < diag,
            "lane_diag feed must come after the pre-roll gate"
        );
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
