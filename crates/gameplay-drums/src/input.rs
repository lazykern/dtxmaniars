//! Keyboard input → `LaneHit` events.
//!
//! Keys resolve to lanes via [`crate::bindings::BindResolver`], built from the
//! persisted `dtx-config` `InputBindings`. MIDI is handled in the lib.rs
//! `midi_gate` module (pump lives in `dtx_input::pump`).
//!
//! ## Phase 0 p0-6
//!
//! Gated on `EGameMode::Drums` so when the user picks Guitar mode, the
//! digits 1-9 don't accidentally fire drum hits. Mirror gating is in
//! `gameplay-guitar::input`.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_shell::EGameMode;

use crate::bindings::BindResolver;
use crate::events::InputHit;
use crate::resources::GameplayClock;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PendingLaneInputs>()
        .add_systems(
            PreUpdate,
            // Not gated on `editor_closed`: while the Customize surface is open we
            // still want captured hits to reach `LaneHit` (flash + feedback sound).
            // Scoring is gated instead — see `judge::judge_lane_hit_system`.
            capture_key_to_lane_input
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(crate::practice::gameplay_input_active),
        )
        .add_systems(
            PreUpdate,
            // NO state gate: bound system-verb keys translate everywhere and
            // the game-shell router decides delivery per context. In
            // particular no PauseState gate — the key that paused the song has
            // to un-pause it. `RawInputOwned` still silences the translator
            // while a capture flow owns the keyboard.
            dtx_input::keyboard::keyboard_system_verbs.after(bevy::input::InputSystems),
        )
        .add_systems(
            FixedUpdate,
            emit_pending_lane_hits
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(crate::practice::gameplay_input_active),
        );
}

#[derive(Resource, Default, Debug)]
pub(crate) struct PendingLaneInputs {
    events: Vec<CapturedLaneInput>,
}

#[derive(Debug, Clone)]
struct CapturedLaneInput {
    lanes: Vec<u8>,
    captured_at: Instant,
}

pub(crate) fn clear_pending_lane_inputs(commands: &mut Commands) {
    commands.insert_resource(PendingLaneInputs::default());
}

pub(crate) fn clear_pending_lane_inputs_now(pending: &mut PendingLaneInputs) {
    pending.events.clear();
}

fn capture_key_to_lane_input(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<BindResolver>,
    mode: Res<EGameMode>,
    capture: Res<crate::editor::bindings_capture::CaptureState>,
    mut pending: ResMut<PendingLaneInputs>,
) {
    if *mode != EGameMode::Drums
        || !matches!(
            *capture,
            crate::editor::bindings_capture::CaptureState::Idle
        )
    {
        return;
    }
    for key in keys.get_just_pressed() {
        let captured_at = Instant::now();
        let lanes: Vec<_> = resolver.lanes_for_key(*key).collect();
        if !lanes.is_empty() {
            pending
                .events
                .push(CapturedLaneInput { lanes, captured_at });
        }
    }
}

fn emit_pending_lane_hits(
    clock: Res<GameplayClock>,
    mut pending: ResMut<PendingLaneInputs>,
    mut events: MessageWriter<InputHit>,
) {
    if !clock.is_ready() {
        return;
    }
    if pending.events.is_empty() {
        return;
    }
    let now = Instant::now();
    for captured in std::mem::take(&mut pending.events) {
        events.write(InputHit {
            lanes: captured.lanes,
            audio_ms: compensated_audio_ms(
                clock.current_ms,
                now.saturating_duration_since(captured.captured_at),
            ),
            captured_at: captured.captured_at,
        });
    }
}

fn compensated_audio_ms(current_audio_ms: i64, capture_delay: Duration) -> i64 {
    current_audio_ms.saturating_sub(capture_delay.as_millis() as i64)
}

#[cfg(test)]
fn capture_targets_for_keys(
    keys: impl IntoIterator<Item = KeyCode>,
    resolver: &BindResolver,
) -> Vec<Vec<u8>> {
    keys.into_iter()
        .map(|key| resolver.lanes_for_key(key).collect())
        .filter(|lanes: &Vec<u8>| !lanes.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_drums_default_matches_bocud() {
        // BocuD default: X = HH, Space = BD.
        let r = crate::bindings::BindResolver::default();
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0));
        assert_eq!(r.lane_for_key(KeyCode::Space), Some(2));
    }

    #[test]
    fn input_audio_ms_is_compensated_for_capture_delay() {
        assert_eq!(
            compensated_audio_ms(1000, std::time::Duration::from_millis(12)),
            988
        );
    }

    #[test]
    fn one_shared_key_captures_one_ordered_input() {
        let mut bindings = dtx_input::InputBindings::default();
        bindings.bind_shared(
            dtx_core::EChannel::LeftBassDrum,
            dtx_input::BindSource::Key(KeyCode::Space),
        );
        let resolver = crate::bindings::BindResolver::from_bindings(&bindings);

        let inputs = capture_targets_for_keys([KeyCode::Space], &resolver);

        assert_eq!(inputs, vec![vec![2, 11]]);
    }
}
