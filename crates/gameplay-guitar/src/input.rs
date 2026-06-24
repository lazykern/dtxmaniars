//! Guitar keyboard → LaneHit input system.
//!
//! Reads `ButtonInput<KeyCode>` and the `LaneMap` resource, emits `LaneHit`
//! messages on press + release.
//!
//! Reference: gameplay-drums/src/input.rs (same pattern, guitar keys).

use bevy::prelude::*;
use dtx_timing::AudioClock;

use crate::events::{LaneHit, LaneHitKind};
use crate::lane_map::LaneMap;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, keyboard_to_lane_hits);
}

fn keyboard_to_lane_hits(
    keys: Res<ButtonInput<KeyCode>>,
    map: Res<LaneMap>,
    clock: Res<AudioClock>,
    mut hits: MessageWriter<LaneHit>,
) {
    let now = clock.current_ms.unwrap_or(0);
    for key in keys.get_just_pressed() {
        if let Some(lane) = map.lane_for_key(*key) {
            hits.write(LaneHit::press(lane, now));
        }
    }
    for key in keys.get_just_released() {
        if let Some(lane) = map.lane_for_key(*key) {
            hits.write(LaneHit {
                lane,
                audio_ms: now,
                kind: LaneHitKind::Release,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_no_lane() {
        let m = LaneMap::default_guitar();
        assert!(m.lane_for_key(KeyCode::KeyZ).is_none());
    }
}
