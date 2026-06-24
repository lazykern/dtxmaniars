//! LaneHit message — a single key/note press or release on a lane.

use bevy::prelude::Message as _;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Opaque lane id. Each gameplay crate's LaneMap maps it to game semantics.
pub type LaneId = u8;

/// Press or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LaneHitKind {
    /// Key/note down.
    Press,
    /// Key/note up.
    Release,
}

/// A single key/note event from any input source.
///
/// `LaneHit::lane` is a u8 — the gameplay crate's `LaneMap` decides whether
/// lane 0 means HH (drums) or R (guitar).
#[derive(Debug, Clone, Copy, Message, Serialize, Deserialize)]
pub struct LaneHit {
    /// Lane index (gameplay-crate-specific).
    pub lane: LaneId,
    /// AudioClock ms when the event occurred.
    pub audio_ms: i64,
    /// Press or release.
    pub kind: LaneHitKind,
}

impl LaneHit {
    /// Construct a press hit at `audio_ms` for `lane`.
    pub fn press(lane: LaneId, audio_ms: i64) -> Self {
        Self {
            lane,
            audio_ms,
            kind: LaneHitKind::Press,
        }
    }
}

impl PartialEq for LaneHit {
    fn eq(&self, other: &Self) -> bool {
        self.lane == other.lane && self.audio_ms == other.audio_ms && self.kind == other.kind
    }
}

impl Eq for LaneHit {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_constructs_correctly() {
        let h = LaneHit::press(3, 500);
        assert_eq!(h.lane, 3);
        assert_eq!(h.audio_ms, 500);
        assert_eq!(h.kind, LaneHitKind::Press);
    }

    #[test]
    fn lane_hit_kind_press_release_distinct() {
        assert_ne!(LaneHitKind::Press, LaneHitKind::Release);
    }
}
