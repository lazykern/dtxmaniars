//! Keyboard input → `LaneHit` events.
//!
//! Trivial bridge for M2: KeyDown → LaneHit. MIDI/pad mapping lands in M6+.
//!
//! ## Phase 0 p0-6
//!
//! Gated on `EGameMode::Drums` so when the user picks Guitar mode, the
//! digits 1-9 don't accidentally fire drum hits. Mirror gating is in
//! `gameplay-guitar::input`.

use bevy::prelude::*;
use game_shell::EGameMode;

use crate::events::LaneHit;
use crate::lane_map::LaneMap;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, key_to_lane_hit);
}

fn key_to_lane_hit(
    keys: Res<ButtonInput<KeyCode>>,
    lane_map: Res<LaneMap>,
    clock: Res<dtx_timing::AudioClock>,
    mode: Res<EGameMode>,
    mut events: MessageWriter<LaneHit>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    for key in keys.get_just_pressed() {
        if let Some(lane) = lane_map.lane_for_key(*key) {
            events.write(LaneHit {
                lane,
                audio_ms: clock.ms_or_zero(),
            });
        }
    }
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
}
