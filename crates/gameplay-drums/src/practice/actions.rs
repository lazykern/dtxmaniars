//! Input→action indirection for practice mode.
//!
//! Keyboard (v2) is translated into `PracticeAction` messages; MIDI pad
//! combos / foot control later bind here without touching any consumer.

use bevy::prelude::*;
use game_shell::PauseState;

use super::hud::timeline_ui::bar_number;
use super::session::{preroll_target, PracticeSession};
use super::toast::ToastQueue;
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::{ChipTimeline, SnapDivisor};

/// One quick-tier practice action. All hotkeys (and later MIDI combos)
/// route through this so consumers never read raw input.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeAction {
    SetLoopStart,
    SetLoopEnd,
    ClearLoop,
    RateDown,
    RateUp,
    RestartLoop,
    ToggleRamp,
    OpenFullHud,
}

/// Key→action table. A resource so a future bindings UI / MIDI layer can
/// replace it wholesale.
#[derive(Resource, Debug, Clone)]
pub struct PracticeBindings(pub Vec<(KeyCode, PracticeAction)>);

impl Default for PracticeBindings {
    fn default() -> Self {
        Self(vec![
            (KeyCode::BracketLeft, PracticeAction::SetLoopStart),
            (KeyCode::BracketRight, PracticeAction::SetLoopEnd),
            (KeyCode::Backspace, PracticeAction::ClearLoop),
            (KeyCode::Minus, PracticeAction::RateDown),
            (KeyCode::Equal, PracticeAction::RateUp),
            (KeyCode::KeyR, PracticeAction::RestartLoop),
            (KeyCode::KeyT, PracticeAction::ToggleRamp),
            (KeyCode::Tab, PracticeAction::OpenFullHud),
        ])
    }
}

/// Pure: the action bound to `key`, if any.
pub fn action_for(bindings: &PracticeBindings, key: KeyCode) -> Option<PracticeAction> {
    bindings.0.iter().find(|(k, _)| *k == key).map(|(_, a)| *a)
}

/// Quick tier only (Running): translate just-pressed keys into actions.
pub fn emit_practice_actions(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<PracticeBindings>,
    mut out: MessageWriter<PracticeAction>,
) {
    for key in keys.get_just_pressed() {
        if let Some(action) = action_for(&bindings, *key) {
            out.write(action);
        }
    }
}

/// Apply quick-tier actions. `ToggleRamp` is intentionally not handled
/// here: `ramp::handle_toggle_ramp` consumes the same message stream
/// with its own `MessageReader` (multiple readers are independent).
pub fn apply_practice_actions(
    mut actions: MessageReader<PracticeAction>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut toasts: ResMut<ToastQueue>,
) {
    for action in actions.read() {
        match action {
            PracticeAction::SetLoopStart => {
                let ms = timeline.bar_start_before(clock.current_ms);
                session.set_loop_start(ms);
                toasts.push(format!("A set @ bar {}", bar_number(&timeline.bar_ms, ms)));
            }
            PracticeAction::SetLoopEnd => {
                let mut ms = timeline.bar_start_before(clock.current_ms);
                // Min region: one bar. B on/before A pushes one bar past A.
                if let Some(r) = session.transport.loop_region {
                    if ms <= r.start_ms {
                        ms = timeline.snap_neighbor(r.start_ms, SnapDivisor::Bar, 1);
                    }
                }
                session.set_loop_end(ms);
                toasts.push(format!("B set @ bar {}", bar_number(&timeline.bar_ms, ms)));
            }
            PracticeAction::ClearLoop => {
                session.transport.loop_region = None;
                toasts.push("loop cleared");
            }
            PracticeAction::RateDown => {
                session.step_user_tempo(-1);
                toasts.push(format!("rate → {:.2}×", session.transport.user_tempo));
            }
            PracticeAction::RateUp => {
                session.step_user_tempo(1);
                toasts.push(format!("rate → {:.2}×", session.transport.user_tempo));
            }
            PracticeAction::RestartLoop => {
                let intent = session
                    .transport
                    .loop_region
                    .map(|r| r.start_ms)
                    .unwrap_or(session.current_attempt.start_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.transport.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                toasts.push("restart");
            }
            PracticeAction::OpenFullHud => next_pause.set(PauseState::Paused),
            PracticeAction::ToggleRamp => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::KeyCode;

    #[test]
    fn default_bindings_cover_spec_table() {
        let b = PracticeBindings::default();
        assert_eq!(
            action_for(&b, KeyCode::BracketLeft),
            Some(PracticeAction::SetLoopStart)
        );
        assert_eq!(
            action_for(&b, KeyCode::BracketRight),
            Some(PracticeAction::SetLoopEnd)
        );
        assert_eq!(
            action_for(&b, KeyCode::Backspace),
            Some(PracticeAction::ClearLoop)
        );
        assert_eq!(
            action_for(&b, KeyCode::Minus),
            Some(PracticeAction::RateDown)
        );
        assert_eq!(action_for(&b, KeyCode::Equal), Some(PracticeAction::RateUp));
        assert_eq!(
            action_for(&b, KeyCode::KeyR),
            Some(PracticeAction::RestartLoop)
        );
        assert_eq!(
            action_for(&b, KeyCode::KeyT),
            Some(PracticeAction::ToggleRamp)
        );
        assert_eq!(
            action_for(&b, KeyCode::Tab),
            Some(PracticeAction::OpenFullHud)
        );
        assert_eq!(action_for(&b, KeyCode::KeyQ), None);
    }
}
