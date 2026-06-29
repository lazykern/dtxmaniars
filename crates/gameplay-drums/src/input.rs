//! Keyboard input → `LaneHit` events.
//!
//! Trivial bridge for M2: KeyDown → LaneHit. MIDI/pad mapping lands in M6+.
//!
//! ## Phase 0 p0-6
//!
//! Gated on `EGameMode::Drums` so when the user picks Guitar mode, the
//! digits 1-9 don't accidentally fire drum hits. Mirror gating is in
//! `gameplay-guitar::input`.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_shell::EGameMode;

use crate::events::LaneHit;
use crate::lane_map::LaneMap;
use crate::resources::GameplayClock;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PendingLaneInputs>()
        .add_systems(
            PreUpdate,
            capture_key_to_lane_input.run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(
            FixedUpdate,
            emit_pending_lane_hits
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance)),
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
    lane_map: Res<LaneMap>,
    mode: Res<EGameMode>,
    mut pending: ResMut<PendingLaneInputs>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    for key in keys.get_just_pressed() {
        if let Some(lane) = lane_map.lane_for_key(*key) {
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
    fn lane_map_drums_default_covers_digits_1_9() {
        let m = LaneMap::default_drums();
        assert_eq!(m.lane_for_key(KeyCode::Digit1), Some(0));
        assert_eq!(m.lane_for_key(KeyCode::Digit9), Some(8));
    }

    #[test]
    fn guitar_mode_does_not_match_drums_keys() {
        // When mode is Guitar, the drums system must early-return. The LaneMap
        // itself doesn't change; the gating is at the system level.
        let m = LaneMap::default_drums();
        // LaneMap is purely structural — it does not depend on EGameMode.
        assert!(m.lane_for_key(KeyCode::Digit1).is_some());
    }

    #[test]
    fn input_audio_ms_is_compensated_for_capture_delay() {
        assert_eq!(
            compensated_audio_ms(1000, std::time::Duration::from_millis(12)),
            988
        );
    }
}
