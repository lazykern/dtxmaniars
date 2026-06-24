//! Guitar keyboard → LaneHit input system.
//!
//! Reads `ButtonInput<KeyCode>` and the `LaneMap` resource, emits `LaneHit`
//! messages on press + release.
//!
//! Reference: gameplay-drums/src/input.rs (same pattern, guitar keys).
//!
//! ## Phase 0 p0-6
//!
//! Gated on `EGameMode::Guitar` so when the user picks Drums mode, the
//! guitar letter keys (Q/W/E/R/T/Y) don't accidentally fire guitar hits.
//! Mirror gating is in `gameplay-drums::input`.

use bevy::prelude::*;
use dtx_timing::AudioClock;
use game_shell::EGameMode;

use crate::events::{LaneHit, LaneHitKind};
use crate::lane_map::LaneMap;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, keyboard_to_lane_hits);
}

fn keyboard_to_lane_hits(
    keys: Res<ButtonInput<KeyCode>>,
    map: Res<LaneMap>,
    clock: Res<AudioClock>,
    mode: Res<EGameMode>,
    mut hits: MessageWriter<LaneHit>,
) {
    if *mode != EGameMode::Guitar {
        return;
    }
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

    #[test]
    fn guitar_letters_resolve_to_different_lanes() {
        // CActPerfGuitarLaneFlushGB.cs: R/G/B/Y/P mapped to A/S/D/F/G
        // (per gameplay-guitar::lane_map::default_guitar).
        let m = LaneMap::default_guitar();
        let r = m.lane_for_key(KeyCode::KeyA);
        let g = m.lane_for_key(KeyCode::KeyS);
        let b = m.lane_for_key(KeyCode::KeyD);
        let y = m.lane_for_key(KeyCode::KeyF);
        let p = m.lane_for_key(KeyCode::KeyG);
        assert!(r.is_some() && g.is_some() && b.is_some() && y.is_some() && p.is_some());
        // All five must be distinct lanes.
        let lanes = [r, g, b, y, p];
        let unique: std::collections::HashSet<_> = lanes.iter().flatten().collect();
        assert_eq!(unique.len(), 5);
    }
}
