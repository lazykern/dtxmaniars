//! Practice session state: loop region, rate, snap, pre-roll, attempts.

use bevy::prelude::*;

use crate::resources::JudgmentCounts;
use crate::timeline::{ChipTimeline, SnapDivisor};

use super::draft::PracticeTrainerMode;

pub const RATE_MIN: f32 = 0.5;
pub const RATE_MAX: f32 = 1.5;
pub const RATE_STEP: f32 = 0.05;
pub const MAX_ATTEMPT_HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopRegion {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrerollSetting {
    OneBar,
    Seconds(f32),
    Off,
}

impl PrerollSetting {
    pub fn label(self) -> String {
        match self {
            PrerollSetting::OneBar => "1 bar".into(),
            PrerollSetting::Seconds(s) => format!("{s:.0}s"),
            PrerollSetting::Off => "off".into(),
        }
    }

    pub fn next(self) -> Self {
        match self {
            PrerollSetting::OneBar => PrerollSetting::Seconds(2.0),
            PrerollSetting::Seconds(_) => PrerollSetting::Off,
            PrerollSetting::Off => PrerollSetting::OneBar,
        }
    }
}

/// Resolve the actual seek target for an intended attempt start:
/// back off by the configured pre-roll so the drummer gets ready-time.
pub fn preroll_target(timeline: &ChipTimeline, preroll: PrerollSetting, intent_ms: i64) -> i64 {
    match preroll {
        PrerollSetting::Off => intent_ms,
        PrerollSetting::Seconds(s) => (intent_ms - (s * 1000.0) as i64).max(0),
        PrerollSetting::OneBar => timeline.bar_start_before((intent_ms - 1).max(0)),
    }
}

pub const RAMP_START_DEFAULT: f32 = 0.70;
pub const RAMP_TARGET_DEFAULT: f32 = 1.00;
pub const RAMP_STEP_DEFAULT: f32 = 0.05;
pub const RAMP_THRESHOLD_DEFAULT: f32 = 90.0;
pub const RAMP_STREAK_DEFAULT: u8 = 1;

/// Accuracy-gated tempo-ramp configuration edited in Practice Settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampConfig {
    pub start_tempo: f32,
    pub target_tempo: f32,
    pub step: f32,
    pub threshold_pct: f32,
    /// Consecutive passes required per promotion (and for completion).
    pub required_successes: u8,
}

impl Default for RampConfig {
    fn default() -> Self {
        Self {
            start_tempo: RAMP_START_DEFAULT,
            target_tempo: RAMP_TARGET_DEFAULT,
            step: RAMP_STEP_DEFAULT,
            threshold_pct: RAMP_THRESHOLD_DEFAULT,
            required_successes: RAMP_STREAK_DEFAULT,
        }
    }
}

/// Live ramp state; meaningful only while `armed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampState {
    pub armed: bool,
    /// The ramp's current tempo step. Owns playback while armed
    /// (`PracticeSession::effective_tempo`, added in a later task).
    pub step_tempo: f32,
    pub success_streak: u8,
    pub fail_streak: u8,
}

impl Default for RampState {
    fn default() -> Self {
        Self {
            armed: false,
            step_tempo: RAMP_START_DEFAULT,
            success_streak: 0,
            fail_streak: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AttemptStats {
    /// Attempt span start (the intent, not the pre-roll point). Chips
    /// judged before this are pre-roll and excluded.
    pub start_ms: i64,
    pub counts: JudgmentCounts,
    pub combo: u32,
    pub max_combo: u32,
    pub overhits: u32,
    pub error_sum_ms: i64,
    pub error_count: u32,
    /// Notes cleared while halted in wait mode (tempo-free, not judged).
    pub waited: u32,
}

impl AttemptStats {
    pub fn accuracy_pct(&self) -> f32 {
        self.counts.achievement_pct()
    }

    /// Notes cleared without halting / all notes seen (%). The wait-mode
    /// analogue of achievement%.
    pub fn flow_pct(&self) -> f32 {
        let total = self.counts.total() + self.waited;
        if total == 0 {
            0.0
        } else {
            self.counts.total() as f32 / total as f32 * 100.0
        }
    }

    pub fn mean_error_ms(&self) -> f32 {
        if self.error_count == 0 {
            0.0
        } else {
            self.error_sum_ms as f32 / self.error_count as f32
        }
    }

    pub fn has_data(&self) -> bool {
        self.counts.total() > 0 || self.waited > 0
    }
}

#[derive(Debug, Clone)]
pub struct AttemptRecord {
    pub start_ms: i64,
    pub end_ms: i64,
    pub tempo: f32,
    pub counts: JudgmentCounts,
    pub max_combo: u32,
    pub overhits: u32,
    pub accuracy_pct: f32,
    pub mean_error_ms: f32,
    pub waited: u32,
    pub flow_pct: f32,
    pub trainer_mode: PracticeTrainerMode,
}

/// Transport state: what/where/how-fast the player chose. Only user
/// input mutates this.
#[derive(Debug, Clone)]
pub struct PracticeTransport {
    /// The player's chosen tempo. The ramp never writes this except on
    /// completion (graduation).
    pub user_tempo: f32,
    pub snap: SnapDivisor,
    pub preroll: PrerollSetting,
    /// Count-in click during pre-roll (spec: count-in metronome).
    pub metronome: bool,
    pub loop_region: Option<LoopRegion>,
    /// Timeline edit cursor in Setup/Settings (chart ms). None = playhead.
    pub scrub_cursor_ms: Option<i64>,
}

impl Default for PracticeTransport {
    fn default() -> Self {
        Self {
            user_tempo: 1.0,
            snap: SnapDivisor::Bar,
            preroll: PrerollSetting::OneBar,
            metronome: true,
            loop_region: None,
            scrub_cursor_ms: None,
        }
    }
}

/// Trainer state: the accuracy-gated ramp (future trainers live here).
#[derive(Debug, Clone)]
pub struct PracticeTrainer {
    pub mode: PracticeTrainerMode,
    pub ramp_config: RampConfig,
    pub ramp: RampState,
}

impl Default for PracticeTrainer {
    fn default() -> Self {
        Self {
            mode: PracticeTrainerMode::Off,
            ramp_config: RampConfig::default(),
            ramp: RampState::default(),
        }
    }
}

impl PracticeTrainer {
    pub fn wait_enabled(&self) -> bool {
        self.mode == PracticeTrainerMode::Wait
    }

    pub fn ramp_armed(&self) -> bool {
        self.mode == PracticeTrainerMode::Ramp && self.ramp.armed
    }

    pub fn disable(&mut self) {
        self.mode = PracticeTrainerMode::Off;
        self.ramp.armed = false;
    }

    pub fn enable_wait(&mut self, enabled: bool) {
        if enabled {
            self.mode = PracticeTrainerMode::Wait;
            self.ramp.armed = false;
        } else if self.mode == PracticeTrainerMode::Wait {
            self.disable();
        }
    }

    pub fn arm_ramp(&mut self) {
        self.mode = PracticeTrainerMode::Ramp;
        self.ramp = RampState {
            armed: true,
            step_tempo: self.ramp_config.start_tempo,
            success_streak: 0,
            fail_streak: 0,
        };
    }

    pub fn disarm_ramp(&mut self) {
        self.ramp.armed = false;
        if self.mode == PracticeTrainerMode::Ramp {
            self.mode = PracticeTrainerMode::Off;
        }
    }
}

/// Present only while the stage runs in practice mode. Absence = normal
/// play with zero behavior change.
#[derive(Resource, Debug, Clone)]
pub struct PracticeSession {
    pub transport: PracticeTransport,
    pub trainer: PracticeTrainer,
    pub current_attempt: AttemptStats,
    pub current_attempt_eligible: bool,
    pub attempt_history: Vec<AttemptRecord>,
    /// Per-lane data for the running attempt. It is promoted to `lane_diag`
    /// only when the attempt is eligible and reaches the current loop end.
    pub current_attempt_lane_diag: super::diagnosis::LaneDiagnosis,
    /// Per-lane diagnosis for the current loop region (Progress panel).
    pub lane_diag: super::diagnosis::LaneDiagnosis,
}

impl Default for PracticeSession {
    fn default() -> Self {
        Self {
            transport: PracticeTransport::default(),
            trainer: PracticeTrainer::default(),
            current_attempt: AttemptStats::default(),
            current_attempt_eligible: true,
            attempt_history: Vec::new(),
            current_attempt_lane_diag: super::diagnosis::LaneDiagnosis::default(),
            lane_diag: super::diagnosis::LaneDiagnosis::default(),
        }
    }
}

impl PracticeSession {
    /// The tempo playback actually runs at: the ramp's step while armed,
    /// the player's chosen tempo otherwise.
    pub fn effective_tempo(&self) -> f32 {
        if self.trainer.ramp_armed() {
            self.trainer.ramp.step_tempo
        } else {
            self.transport.user_tempo
        }
    }

    /// Clear the A/B loop (disarms the ramp — the ramp is a claim about
    /// one specific section).
    pub fn clear_loop(&mut self) {
        self.invalidate_current_attempt();
        self.current_attempt_lane_diag.clear();
        self.lane_diag.clear();
        self.transport.loop_region = None;
        self.trainer.disarm_ramp();
    }

    /// Step the user tempo by `dir` in RATE_STEP increments, clamped and
    /// quantized so repeated stepping never accumulates float error.
    pub fn step_user_tempo(&mut self, dir: i8) {
        self.invalidate_current_attempt();
        let steps = (self.transport.user_tempo / RATE_STEP).round() as i32 + dir as i32;
        self.transport.user_tempo = (steps as f32 * RATE_STEP).clamp(RATE_MIN, RATE_MAX);
    }

    pub fn invalidate_current_attempt(&mut self) {
        self.current_attempt_eligible = false;
    }

    /// Finalize the running attempt into history (skipped when it saw no
    /// judgements) and start a fresh one at `next_start_ms`. Returns the
    /// finalized record when it had data, `None` when the pass was empty.
    pub fn roll_attempt(&mut self, end_ms: i64, next_start_ms: i64) -> Option<AttemptRecord> {
        let completed_region = self.transport.loop_region.is_some_and(|region| {
            self.current_attempt.start_ms == region.start_ms && end_ms >= region.end_ms
        });
        let canonical_end_ms = self
            .transport
            .loop_region
            .filter(|_| completed_region)
            .map_or(end_ms, |region| region.end_ms);
        let record = if self.current_attempt_eligible && self.current_attempt.has_data() {
            let a = &self.current_attempt;
            let record = AttemptRecord {
                start_ms: a.start_ms,
                end_ms: canonical_end_ms,
                tempo: self.effective_tempo(),
                counts: a.counts,
                max_combo: a.max_combo,
                overhits: a.overhits,
                accuracy_pct: a.accuracy_pct(),
                mean_error_ms: a.mean_error_ms(),
                waited: a.waited,
                flow_pct: a.flow_pct(),
                trainer_mode: self.trainer.mode,
            };
            self.attempt_history.push(record.clone());
            if self.attempt_history.len() > MAX_ATTEMPT_HISTORY {
                self.attempt_history.remove(0);
            }
            if completed_region {
                self.lane_diag.merge(&self.current_attempt_lane_diag);
            }
            Some(record)
        } else {
            None
        };
        self.current_attempt = AttemptStats {
            start_ms: next_start_ms,
            ..Default::default()
        };
        self.current_attempt_lane_diag.clear();
        self.current_attempt_eligible = true;
        record
    }

    /// Set the A marker; keeps the region valid (swap, min length is
    /// enforced by the caller against bar data).
    pub fn set_loop_start(&mut self, ms: i64) {
        self.invalidate_current_attempt();
        self.current_attempt_lane_diag.clear();
        self.lane_diag.clear();
        self.trainer.disarm_ramp();
        let end = self.transport.loop_region.map(|r| r.end_ms);
        self.transport.loop_region = Some(match end {
            Some(e) if e > ms => LoopRegion {
                start_ms: ms,
                end_ms: e,
            },
            _ => LoopRegion {
                start_ms: ms,
                end_ms: i64::MAX,
            },
        });
    }

    pub fn set_loop_end(&mut self, ms: i64) {
        self.invalidate_current_attempt();
        self.current_attempt_lane_diag.clear();
        self.lane_diag.clear();
        self.trainer.disarm_ramp();
        let start = self.transport.loop_region.map(|r| r.start_ms).unwrap_or(0);
        self.transport.loop_region = Some(if ms > start {
            LoopRegion {
                start_ms: start,
                end_ms: ms,
            }
        } else {
            // B placed before A: swap.
            LoopRegion {
                start_ms: ms,
                end_ms: start.max(ms + 1),
            }
        });
    }

    /// True when a bounded loop region is armed.
    pub fn loop_armed(&self) -> bool {
        self.transport
            .loop_region
            .is_some_and(|r| r.end_ms != i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metronome_defaults_on() {
        let s = PracticeSession::default();
        assert!(s.transport.metronome);
    }

    #[test]
    fn wait_defaults_off_and_flow_pct_computes() {
        let s = PracticeSession::default();
        assert!(!s.trainer.wait_enabled());

        let mut a = AttemptStats::default();
        a.counts.perfect = 3;
        a.waited = 1;
        assert_eq!(a.flow_pct(), 75.0);

        let empty = AttemptStats::default();
        assert_eq!(empty.flow_pct(), 0.0);
    }

    #[test]
    fn roll_attempt_carries_waited_and_flow() {
        let mut s = PracticeSession::default();
        s.trainer.enable_wait(true);
        s.current_attempt.counts.perfect = 3;
        s.current_attempt.waited = 1;
        let rec = s.roll_attempt(4_000, 0).unwrap();
        assert_eq!(rec.waited, 1);
        assert_eq!(rec.flow_pct, 75.0);
        assert_eq!(rec.trainer_mode, PracticeTrainerMode::Wait);
        assert_eq!(s.current_attempt.waited, 0, "fresh attempt starts clean");
    }

    #[test]
    fn region_change_clears_lane_diag() {
        use dtx_scoring::JudgmentKind;
        let mut s = PracticeSession::default();
        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.set_loop_start(2_000);
        assert!(s.lane_diag.lanes.is_empty(), "Set A must clear diagnosis");

        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.set_loop_end(6_000);
        assert!(s.lane_diag.lanes.is_empty(), "Set B must clear diagnosis");

        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.clear_loop();
        assert!(
            s.lane_diag.lanes.is_empty(),
            "Clear loop must clear diagnosis"
        );
    }

    #[test]
    fn rate_step_quantized_and_clamped() {
        let mut s = PracticeSession::default();
        s.step_user_tempo(-1);
        assert!((s.transport.user_tempo - 0.95).abs() < 1e-6);
        for _ in 0..40 {
            s.step_user_tempo(-1);
        }
        assert!((s.transport.user_tempo - RATE_MIN).abs() < 1e-6);
        for _ in 0..40 {
            s.step_user_tempo(1);
        }
        assert!((s.transport.user_tempo - RATE_MAX).abs() < 1e-6);
    }

    #[test]
    fn roll_attempt_records_history_and_resets() {
        let mut s = PracticeSession::default();
        s.current_attempt.start_ms = 4_000;
        s.current_attempt.counts.perfect = 10;
        s.current_attempt.max_combo = 10;
        s.roll_attempt(8_000, 4_000);
        assert_eq!(s.attempt_history.len(), 1);
        assert_eq!(s.attempt_history[0].start_ms, 4_000);
        assert_eq!(s.attempt_history[0].end_ms, 8_000);
        assert!(!s.current_attempt.has_data());
        assert_eq!(s.current_attempt.start_ms, 4_000);
    }

    #[test]
    fn completed_loop_overshoot_is_canonicalized_to_loop_end() {
        let mut s = PracticeSession::default();
        s.transport.loop_region = Some(LoopRegion {
            start_ms: 4_000,
            end_ms: 8_000,
        });
        s.current_attempt.start_ms = 4_000;
        s.current_attempt.counts.perfect = 1;

        let record = s.roll_attempt(8_017, 4_000).expect("completed attempt");

        assert_eq!(record.end_ms, 8_000);
    }

    #[test]
    fn lane_diagnosis_merges_only_eligible_completed_attempts() {
        use dtx_scoring::JudgmentKind;

        let mut s = PracticeSession::default();
        s.transport.loop_region = Some(LoopRegion {
            start_ms: 4_000,
            end_ms: 8_000,
        });
        s.current_attempt.start_ms = 4_000;
        s.current_attempt.counts.perfect = 1;
        s.current_attempt_lane_diag
            .apply_judgment(0, JudgmentKind::Perfect, -12);
        s.invalidate_current_attempt();
        assert!(s.roll_attempt(6_000, 4_000).is_none());
        assert!(s.lane_diag.lanes.is_empty());
        assert!(s.current_attempt_lane_diag.lanes.is_empty());

        s.current_attempt.counts.perfect = 1;
        s.current_attempt_lane_diag
            .apply_judgment(0, JudgmentKind::Perfect, 4);
        assert!(s.roll_attempt(8_020, 4_000).is_some());
        assert_eq!(s.lane_diag.lanes[&0].judged, 1);
        assert_eq!(s.lane_diag.lanes[&0].delta_sum_ms, 4);
    }

    #[test]
    fn empty_attempt_not_recorded() {
        let mut s = PracticeSession::default();
        s.roll_attempt(1_000, 2_000);
        assert!(s.attempt_history.is_empty());
    }

    #[test]
    fn ineligible_attempt_is_not_recorded_and_next_attempt_is_eligible() {
        let mut s = PracticeSession::default();
        s.current_attempt.counts.perfect = 4;
        s.current_attempt_eligible = false;

        assert!(s.roll_attempt(4_000, 0).is_none());
        assert!(s.attempt_history.is_empty());
        assert!(s.current_attempt_eligible);
    }

    #[test]
    fn manual_loop_and_tempo_changes_invalidate_the_attempt() {
        let mut s = PracticeSession::default();
        s.set_loop_start(1_000);
        assert!(!s.current_attempt_eligible);

        s.current_attempt_eligible = true;
        s.step_user_tempo(1);
        assert!(!s.current_attempt_eligible);
    }

    #[test]
    fn history_capped() {
        let mut s = PracticeSession::default();
        for i in 0..(MAX_ATTEMPT_HISTORY + 5) {
            s.current_attempt.counts.perfect = 1;
            s.roll_attempt(i as i64, 0);
        }
        assert_eq!(s.attempt_history.len(), MAX_ATTEMPT_HISTORY);
    }

    #[test]
    fn loop_markers_swap_when_inverted() {
        let mut s = PracticeSession::default();
        s.set_loop_start(4_000);
        s.set_loop_end(2_000);
        let r = s.transport.loop_region.unwrap();
        assert!(r.start_ms < r.end_ms);
        assert_eq!(r.start_ms, 2_000);
    }

    #[test]
    fn loop_not_armed_until_both_markers() {
        let mut s = PracticeSession::default();
        assert!(!s.loop_armed());
        s.set_loop_start(2_000);
        assert!(!s.loop_armed());
        s.set_loop_end(4_000);
        assert!(s.loop_armed());
    }

    #[test]
    fn mean_error_signed() {
        let a = AttemptStats {
            error_sum_ms: -30,
            error_count: 10,
            ..Default::default()
        };
        assert!((a.mean_error_ms() + 3.0).abs() < 1e-6);
    }

    #[test]
    fn effective_tempo_layers_ramp_over_user() {
        let mut s = PracticeSession::default();
        s.transport.user_tempo = 1.0;
        assert!((s.effective_tempo() - 1.0).abs() < 1e-6);
        s.trainer.arm_ramp();
        assert!((s.effective_tempo() - 0.70).abs() < 1e-6);
        s.trainer.disarm_ramp();
        assert!(
            (s.effective_tempo() - 1.0).abs() < 1e-6,
            "disarm restores the user's tempo untouched"
        );
    }

    #[test]
    fn loop_mutation_disarms_ramp() {
        let mut s = PracticeSession::default();
        s.set_loop_start(2_000);
        s.set_loop_end(4_000);
        s.trainer.arm_ramp();
        s.set_loop_start(6_000);
        assert!(!s.trainer.ramp_armed(), "changing A disarms");
        s.trainer.arm_ramp();
        s.clear_loop();
        assert!(!s.trainer.ramp_armed(), "clearing the loop disarms");
    }
}
