//! Accuracy-gated rate ramp (Rocksmith riff-repeater model). The
//! protocol is a pure function; systems only apply its decisions.

use bevy::prelude::*;

use super::session::{preroll_target, PracticeSession, RampConfig, RampState};
use super::toast::ToastQueue;
use crate::seek::SeekToChartTime;

/// Outcome of one completed loop pass while the ramp is armed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampDecision {
    StepUp {
        new_tempo: f32,
    },
    StepDown {
        new_tempo: f32,
    },
    /// Streak not yet met (first fail, or successes below the required
    /// streak): keep the tempo.
    Hold,
    /// Passed AT the target tempo: ramp disarms; caller graduates
    /// `user_tempo` to the target.
    Complete {
        new_tempo: f32,
    },
}

/// Pure ramp protocol. A pass (accuracy ≥ threshold) builds the success
/// streak; meeting it at the target completes, below it steps up. Two
/// consecutive fails step down once, floored at the start tempo.
pub fn ramp_step(cfg: &RampConfig, state: &mut RampState, accuracy_pct: f32) -> RampDecision {
    if accuracy_pct >= cfg.threshold_pct {
        state.fail_streak = 0;
        state.success_streak += 1;
        if state.success_streak < cfg.required_successes {
            return RampDecision::Hold;
        }
        state.success_streak = 0;
        if state.step_tempo >= cfg.target_tempo - 1e-6 {
            state.armed = false;
            RampDecision::Complete {
                new_tempo: cfg.target_tempo,
            }
        } else {
            let next = (state.step_tempo + cfg.step).min(cfg.target_tempo);
            state.step_tempo = next;
            RampDecision::StepUp { new_tempo: next }
        }
    } else {
        state.success_streak = 0;
        state.fail_streak += 1;
        if state.fail_streak >= 2 {
            state.fail_streak = 0;
            let next = (state.step_tempo - cfg.step).max(cfg.start_tempo);
            state.step_tempo = next;
            RampDecision::StepDown { new_tempo: next }
        } else {
            RampDecision::Hold
        }
    }
}

/// Clamp the live step into `[start, target]` after a config edit.
pub fn clamp_to_config(cfg: &RampConfig, state: &mut RampState) {
    state.step_tempo = state.step_tempo.max(cfg.start_tempo).min(cfg.target_tempo);
}

/// `(current, total)` step indices for display ("RAMP 3/6").
pub fn ramp_step_index(cfg: &RampConfig, tempo: f32) -> (u32, u32) {
    if cfg.step <= 0.0 {
        return (0, 0);
    }
    let total = ((cfg.target_tempo - cfg.start_tempo) / cfg.step)
        .round()
        .max(0.0) as u32;
    let cur = (((tempo - cfg.start_tempo) / cfg.step).round() as i64).clamp(0, total as i64) as u32;
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
        if session.trainer.ramp.armed {
            session.trainer.ramp.armed = false;
            toasts.push(format!(
                "ramp off — tempo {:.2}×",
                session.transport.user_tempo
            ));
            continue;
        }
        if !session.loop_armed() {
            toasts.push("ramp needs an A/B loop");
            continue;
        }
        let cfg = session.trainer.ramp_config;
        session.trainer.ramp = RampState {
            armed: true,
            step_tempo: cfg.start_tempo,
            success_streak: 0,
            fail_streak: 0,
        };
        let a_ms = session
            .transport
            .loop_region
            .expect("loop_armed checked")
            .start_ms;
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.transport.preroll, a_ms),
            snap: None,
            attempt_start_ms: Some(a_ms),
        });
        toasts.push(format!("ramp armed @ {:.2}×", cfg.start_tempo));
    }
}

/// Apply one ramp decision per finished loop pass. Runs after
/// `track_attempt_stats` (same tick as the loop's seek) so the finished
/// attempt is already in history. The ramp owns `step_tempo` while
/// armed; only `Complete` graduates it into `session.transport.user_tempo`.
pub fn apply_ramp(
    mut seeks: MessageReader<SeekToChartTime>,
    mut session: ResMut<PracticeSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    if seeks.read().last().is_none() {
        return;
    }
    if !session.trainer.ramp.armed {
        return;
    }
    let Some(region) = session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
    else {
        return;
    };
    let Some(last) = session.attempt_history.last() else {
        return;
    };
    if last.start_ms != region.start_ms {
        return; // manual seek elsewhere, not a loop pass
    }
    let accuracy = last.accuracy_pct;
    let cfg = session.trainer.ramp_config;
    match ramp_step(&cfg, &mut session.trainer.ramp, accuracy) {
        RampDecision::StepUp { new_tempo } => toasts.push(format!("ramp: {new_tempo:.2}×")),
        RampDecision::StepDown { new_tempo } => {
            toasts.push(format!("ramp: back to {new_tempo:.2}×"))
        }
        RampDecision::Hold => toasts.push("ramp: one more fail steps down"),
        RampDecision::Complete { new_tempo } => {
            session.transport.user_tempo = new_tempo;
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
        RampConfig::default() // 0.70 → 1.00, step 0.05, threshold 90%, streak 1
    }

    fn state(tempo: f32) -> RampState {
        RampState {
            armed: true,
            step_tempo: tempo,
            success_streak: 0,
            fail_streak: 0,
        }
    }

    #[test]
    fn clean_pass_steps_up() {
        let mut s = state(0.70);
        assert_eq!(
            ramp_step(&cfg(), &mut s, 95.0),
            RampDecision::StepUp { new_tempo: 0.75 }
        );
        assert!((s.step_tempo - 0.75).abs() < 1e-6);
    }

    #[test]
    fn first_fail_holds_second_steps_down() {
        let mut s = state(0.80);
        assert_eq!(ramp_step(&cfg(), &mut s, 60.0), RampDecision::Hold);
        assert_eq!(s.fail_streak, 1);
        assert_eq!(
            ramp_step(&cfg(), &mut s, 60.0),
            RampDecision::StepDown { new_tempo: 0.75 }
        );
        assert_eq!(s.fail_streak, 0, "fail counter resets after demotion");
    }

    #[test]
    fn step_down_floors_at_start_tempo() {
        let mut s = state(0.70);
        s.fail_streak = 1;
        assert_eq!(
            ramp_step(&cfg(), &mut s, 0.0),
            RampDecision::StepDown { new_tempo: 0.70 }
        );
    }

    #[test]
    fn pass_below_target_promotes_to_target_without_completing() {
        // v2 bug: pass at 0.95 completed instantly. v3: it promotes to
        // 1.00 and the NEXT pass (at target) completes.
        let mut s = state(0.95);
        assert_eq!(
            ramp_step(&cfg(), &mut s, 92.0),
            RampDecision::StepUp { new_tempo: 1.00 }
        );
        assert!(s.armed, "not complete until a pass AT target");
        assert_eq!(
            ramp_step(&cfg(), &mut s, 92.0),
            RampDecision::Complete { new_tempo: 1.00 }
        );
        assert!(!s.armed);
    }

    #[test]
    fn fail_at_target_steps_back_down() {
        let mut s = state(1.00);
        assert_eq!(ramp_step(&cfg(), &mut s, 50.0), RampDecision::Hold);
        assert_eq!(
            ramp_step(&cfg(), &mut s, 50.0),
            RampDecision::StepDown { new_tempo: 0.95 }
        );
    }

    #[test]
    fn required_successes_gate_promotion() {
        let mut c = cfg();
        c.required_successes = 2;
        let mut s = state(0.70);
        assert_eq!(ramp_step(&c, &mut s, 95.0), RampDecision::Hold);
        assert_eq!(s.success_streak, 1);
        assert_eq!(
            ramp_step(&c, &mut s, 95.0),
            RampDecision::StepUp { new_tempo: 0.75 }
        );
        assert_eq!(s.success_streak, 0);
    }

    #[test]
    fn fail_resets_success_streak_and_vice_versa() {
        let mut c = cfg();
        c.required_successes = 2;
        let mut s = state(0.80);
        ramp_step(&c, &mut s, 95.0); // success 1
        ramp_step(&c, &mut s, 50.0); // fail 1 — success streak dies
        assert_eq!(s.success_streak, 0);
        assert_eq!(s.fail_streak, 1);
        ramp_step(&c, &mut s, 95.0); // success 1 again — fail streak dies
        assert_eq!(s.fail_streak, 0);
    }

    #[test]
    fn clamp_to_config_pulls_step_into_range() {
        let mut s = state(0.70);
        let mut c = cfg();
        c.start_tempo = 0.80;
        clamp_to_config(&c, &mut s);
        assert!(
            (s.step_tempo - 0.80).abs() < 1e-6,
            "raised start pulls step up"
        );
        c.target_tempo = 0.75; // below current step
        clamp_to_config(&c, &mut s);
        assert!(
            (s.step_tempo - 0.75).abs() < 1e-6,
            "lowered target pulls step down"
        );
    }

    #[test]
    fn step_index_display() {
        let c = cfg();
        assert_eq!(ramp_step_index(&c, 0.70), (0, 6));
        assert_eq!(ramp_step_index(&c, 0.85), (3, 6));
        assert_eq!(ramp_step_index(&c, 1.00), (6, 6));
    }
}
