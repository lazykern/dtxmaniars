//! Keyboard → LaneHit helper.
//!
//! Per ADR-0009 the actual Bevy system lives in the gameplay crate (which
//! owns the concrete `LaneMap`). dtx-input provides:
//! - [`KeyLaneMap`] trait that gameplay crates' LaneMap impls.
//! - [`emit_pressed_lanes`] / [`emit_released_lanes`] helpers.
//!
//! Gameplay crate pseudocode:
//! ```ignore
//! fn keyboard_to_lane_hits(
//!     keys: Res<ButtonInput<KeyCode>>,
//!     map: Res<MyLaneMap>,
//!     clock: Res<AudioClock>,
//!     mut hits: MessageWriter<LaneHit>,
//! ) {
//!     let now = clock.current_ms.unwrap_or(0);
//!     emit_pressed_lanes(&keys, &*map, now, &mut hits);
//! }
//! ```

use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use crate::events::{LaneHit, LaneHitKind};

/// Map KeyCode → LaneId. Implemented by each gameplay crate's LaneMap.
pub trait KeyLaneMap {
    /// Return Some(lane_id) if `key` is mapped to a lane, None otherwise.
    fn lane_for_key(&self, key: KeyCode) -> Option<crate::events::LaneId>;
}

/// Emit LaneHit::Press for every just-pressed mapped key.
pub fn emit_pressed_lanes(
    keys: &ButtonInput<KeyCode>,
    map: &dyn KeyLaneMap,
    now_ms: i64,
    out: &mut Vec<LaneHit>,
) {
    for key in keys.get_just_pressed() {
        if let Some(lane) = map.lane_for_key(*key) {
            out.push(LaneHit::press(lane, now_ms));
        }
    }
}

/// Emit LaneHit::Release for every just-released mapped key.
pub fn emit_released_lanes(
    keys: &ButtonInput<KeyCode>,
    map: &dyn KeyLaneMap,
    now_ms: i64,
    out: &mut Vec<LaneHit>,
) {
    for key in keys.get_just_released() {
        if let Some(lane) = map.lane_for_key(*key) {
            out.push(LaneHit {
                lane,
                audio_ms: now_ms,
                kind: LaneHitKind::Release,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{LaneHit, LaneHitKind};
    use bevy::input::keyboard::KeyCode;

    struct TestMap;

    impl KeyLaneMap for TestMap {
        fn lane_for_key(&self, key: KeyCode) -> Option<crate::events::LaneId> {
            match key {
                KeyCode::KeyA => Some(0),
                KeyCode::KeyS => Some(1),
                _ => None,
            }
        }
    }

    #[test]
    fn test_map_a_returns_0() {
        let m = TestMap;
        assert_eq!(m.lane_for_key(KeyCode::KeyA), Some(0));
    }

    #[test]
    fn test_map_z_returns_none() {
        let m = TestMap;
        assert_eq!(m.lane_for_key(KeyCode::KeyZ), None);
    }

    #[test]
    fn lane_hit_press_equals_helper() {
        let a = LaneHit::press(1, 100);
        let b = LaneHit {
            lane: 1,
            audio_ms: 100,
            kind: LaneHitKind::Press,
        };
        assert_eq!(a, b);
    }
}
