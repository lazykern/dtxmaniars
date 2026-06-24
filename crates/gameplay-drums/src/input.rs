//! Keyboard input → `LaneHit` events.
//!
//! Trivial bridge for M2: KeyDown → LaneHit. MIDI/pad mapping lands in M6+.

use bevy::prelude::MessageWriter as _;
use bevy::prelude::*;

use crate::events::LaneHit;
use crate::lane_map::LaneMap;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, key_to_lane_hit);
}

fn key_to_lane_hit(
    keys: Res<ButtonInput<KeyCode>>,
    lane_map: Res<LaneMap>,
    clock: Res<dtx_timing::AudioClock>,
    mut events: MessageWriter<LaneHit>,
) {
    for key in keys.get_just_pressed() {
        if let Some(lane) = lane_map.lane_for_key(*key) {
            events.write(LaneHit {
                lane,
                audio_ms: clock.ms_or_zero(),
            });
        }
    }
}
