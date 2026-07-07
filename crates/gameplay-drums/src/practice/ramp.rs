//! Accuracy-gated rate ramp (Rocksmith riff-repeater model). The
//! protocol is a pure function; systems only apply its decisions.

use bevy::prelude::*;

use super::session::{preroll_target, PracticeSession, RampConfig, RampState};
use super::toast::ToastQueue;
use crate::seek::SeekToChartTime;

/// Outcome of one finished loop pass while the ramp is armed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampDecision {
    StepUp { new_rate: f32 },
    StepDown { new_rate: f32 },
    /// First fail at a step: keep the rate, remember the fail.
    Hold,
    /// Target reached: rate pinned to target, ramp disarms.
    Complete { new_rate: f32 },
}

/// Pure ramp protocol. Pass (accuracy ≥ threshold) → step up, completing
/// at the target. Two consecutive fails → step down once, floored at the
/// start rate.
pub fn ramp_step(cfg: &RampConfig, state: &mut RampState, accuracy_pct: f32) -> RampDecision {
    if accuracy_pct >= cfg.threshold_pct {
        state.consecutive_fails = 0;
        let next = (state.current_rate + cfg.step).min(cfg.target_rate);
        state.current_rate = next;
        if next >= cfg.target_rate - 1e-6 {
            state.armed = false;
            RampDecision::Complete {
                new_rate: cfg.target_rate,
            }
        } else {
            RampDecision::StepUp { new_rate: next }
        }
    } else {
        state.consecutive_fails += 1;
        if state.consecutive_fails >= 2 {
            state.consecutive_fails = 0;
            let next = (state.current_rate - cfg.step).max(cfg.start_rate);
            state.current_rate = next;
            RampDecision::StepDown { new_rate: next }
        } else {
            RampDecision::Hold
        }
    }
}

/// `(current, total)` step indices for display ("RAMP 3/6").
pub fn ramp_step_index(cfg: &RampConfig, rate: f32) -> (u32, u32) {
    if cfg.step <= 0.0 {
        return (0, 0);
    }
    let total = ((cfg.target_rate - cfg.start_rate) / cfg.step).round().max(0.0) as u32;
    let cur = (((rate - cfg.start_rate) / cfg.step).round() as i64).clamp(0, total as i64) as u32;
    (cur, total)
}

use crate::timeline::ChipTimeline;

/// Arm/disarm from `PracticeAction::ToggleRamp` (own reader; the quick
/// applier deliberately ignores this variant). Arming without an armed
/// A/B loop is an error toast + no-op. Arming resets the rate to the
/// configured start and restarts the loop so the first pass is clean.
pub fn handle_toggle_ramp(
    mut actions: MessageReader<super::actions::PracticeAction>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut toasts: ResMut<ToastQueue>,
) {
    for action in actions.read() {
        if *action != super::actions::PracticeAction::ToggleRamp {
            continue;
        }
        if session.ramp.armed {
            session.ramp.armed = false;
            toasts.push("ramp off");
            continue;
        }
        if !session.loop_armed() {
            toasts.push("ramp needs an A/B loop");
            continue;
        }
        let cfg = session.ramp_config;
        session.ramp = RampState {
            armed: true,
            current_rate: cfg.start_rate,
            consecutive_fails: 0,
            skip_next_roll: true,
        };
        session.rate = cfg.start_rate;
        let a_ms = session.loop_region.expect("loop_armed checked").start_ms;
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.preroll, a_ms),
            snap: None,
            attempt_start_ms: Some(a_ms),
        });
        toasts.push(format!("ramp armed @ {:.2}×", cfg.start_rate));
    }
}

/// Apply one ramp decision per finished loop pass. Runs after
/// `track_attempt_stats` (same tick as the loop's seek) so the finished
/// attempt is already in history. Re-adopts `session.rate` as the
/// current step first — a manual nudge simply moves the ramp.
pub fn apply_ramp(
    mut seeks: MessageReader<SeekToChartTime>,
    mut session: ResMut<PracticeSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    if seeks.read().last().is_none() {
        return;
    }
    if !session.ramp.armed {
        return;
    }
    if session.ramp.skip_next_roll {
        session.ramp.skip_next_roll = false;
        return;
    }
    let Some(region) = session.loop_region.filter(|r| r.end_ms != i64::MAX) else {
        return;
    };
    let Some(last) = session.attempt_history.last() else {
        return;
    };
    if last.start_ms != region.start_ms {
        return; // manual seek elsewhere, not a loop pass
    }
    let accuracy = last.accuracy_pct;
    session.ramp.current_rate = session.rate;
    let cfg = session.ramp_config;
    match ramp_step(&cfg, &mut session.ramp, accuracy) {
        RampDecision::StepUp { new_rate } => {
            session.rate = new_rate;
            toasts.push(format!("ramp: {new_rate:.2}×"));
        }
        RampDecision::StepDown { new_rate } => {
            session.rate = new_rate;
            toasts.push(format!("ramp: back to {new_rate:.2}×"));
        }
        RampDecision::Hold => toasts.push("ramp: one more fail steps down"),
        RampDecision::Complete { new_rate } => {
            session.rate = new_rate;
            toasts.push("ramp complete");
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    use game_shell::AppState;
    app.add_systems(
        Update,
        // Not Running-gated: the rail's ramp row (Task 12) toggles while
        // paused via the same message.
        handle_toggle_ramp
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(
        FixedUpdate,
        apply_ramp
            .after(crate::practice::stats::track_attempt_stats)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RampConfig {
        RampConfig::default() // 0.70 → 1.00, step 0.05, threshold 90%
    }

    fn state(rate: f32, fails: u8) -> RampState {
        RampState {
            armed: true,
            current_rate: rate,
            consecutive_fails: fails,
            skip_next_roll: false,
        }
    }

    #[test]
    fn clean_pass_steps_up() {
        let mut s = state(0.70, 0);
        let d = ramp_step(&cfg(), &mut s, 95.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.75 });
        assert!((s.current_rate - 0.75).abs() < 1e-6);
        assert_eq!(s.consecutive_fails, 0);
    }

    #[test]
    fn first_fail_holds() {
        let mut s = state(0.80, 0);
        let d = ramp_step(&cfg(), &mut s, 60.0);
        assert_eq!(d, RampDecision::Hold);
        assert_eq!(s.consecutive_fails, 1);
        assert!((s.current_rate - 0.80).abs() < 1e-6);
    }

    #[test]
    fn second_consecutive_fail_steps_down() {
        let mut s = state(0.80, 1);
        let d = ramp_step(&cfg(), &mut s, 60.0);
        assert_eq!(d, RampDecision::StepDown { new_rate: 0.75 });
        assert_eq!(s.consecutive_fails, 0, "fail counter resets after demotion");
    }

    #[test]
    fn step_down_floors_at_start_rate() {
        let mut s = state(0.70, 1);
        let d = ramp_step(&cfg(), &mut s, 0.0);
        assert_eq!(d, RampDecision::StepDown { new_rate: 0.70 });
    }

    #[test]
    fn pass_reaching_target_completes_and_disarms() {
        let mut s = state(0.95, 0);
        let d = ramp_step(&cfg(), &mut s, 92.0);
        assert_eq!(d, RampDecision::Complete { new_rate: 1.00 });
        assert!(!s.armed);
    }

    #[test]
    fn pass_resets_fail_counter() {
        let mut s = state(0.80, 1);
        let d = ramp_step(&cfg(), &mut s, 91.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.85 });
        assert_eq!(s.consecutive_fails, 0);
    }

    #[test]
    fn manual_nudge_adoption_steps_from_the_nudged_rate() {
        // A manual nudge to 0.90 mid-ramp becomes the current step.
        let mut s = state(0.75, 0);
        s.current_rate = 0.90; // applier does this from session.rate
        let d = ramp_step(&cfg(), &mut s, 95.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.95 });
    }

    #[test]
    fn step_index_display() {
        let c = cfg();
        assert_eq!(ramp_step_index(&c, 0.70), (0, 6));
        assert_eq!(ramp_step_index(&c, 0.85), (3, 6));
        assert_eq!(ramp_step_index(&c, 1.00), (6, 6));
    }
}
