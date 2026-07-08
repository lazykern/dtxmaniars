//! Keyboard input → `LaneHit` events.
//!
//! Keys resolve to lanes via [`crate::bindings::BindResolver`], built from the
//! persisted `dtx-config` `InputBindings`. MIDI is handled in the lib.rs
//! `midi_consumer` module.
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
use crate::events::LaneHit;
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
                .run_if(in_state(game_shell::PauseState::Running)),
        )
        .add_systems(
            FixedUpdate,
            emit_pending_lane_hits
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running)),
        );
}

#[derive(Resource, Default, Debug)]
struct PendingLaneInputs {
    events: Vec<CapturedLaneInput>,
}

#[derive(Debug, Clone, Copy)]
struct CapturedLaneInput {
    lane: u8,
    captured_at: Instant,
}

fn capture_key_to_lane_input(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<BindResolver>,
    mode: Res<EGameMode>,
    mut pending: ResMut<PendingLaneInputs>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    for key in keys.get_just_pressed() {
        if let Some(lane) = resolver.lane_for_key(*key) {
            pending.events.push(CapturedLaneInput {
                lane,
                captured_at: Instant::now(),
            });
        }
    }
}

fn emit_pending_lane_hits(
    clock: Res<GameplayClock>,
    mut pending: ResMut<PendingLaneInputs>,
    mut events: MessageWriter<LaneHit>,
) {
    if !clock.is_ready() {
        return;
    }
    if pending.events.is_empty() {
        return;
    }
    let now = Instant::now();
    for captured in std::mem::take(&mut pending.events) {
        events.write(LaneHit {
            lane: captured.lane,
            audio_ms: compensated_audio_ms(
                clock.current_ms,
                now.saturating_duration_since(captured.captured_at),
            ),
        });
    }
}

fn compensated_audio_ms(current_audio_ms: i64, capture_delay: Duration) -> i64 {
    current_audio_ms.saturating_sub(capture_delay.as_millis() as i64)
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
}
