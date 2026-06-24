//! Guitar mode events. Mirror of gameplay-drums::events so M6.1 can
//! extract shared message types into a common crate.

use bevy::prelude::*;
use bevy::prelude::{Component as _, Message as _, Resource as _};
use serde::{Deserialize, Serialize};

pub use crate::lane_map::LaneId;

/// A single key press or release on a guitar lane.
///
/// `LaneHitKind::Press` is emitted on key-down. `Release` on key-up.
/// M6b: only Press is consumed by judge; Release hooks land in M6.1 for
/// hold-note support.
#[derive(Debug, Clone, Copy, Message, Serialize, Deserialize)]
pub struct LaneHit {
    /// Lane index (0..5 for guitar).
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

/// Press or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LaneHitKind {
    /// Key down.
    Press,
    /// Key up.
    Release,
}

/// Judgment result emitted by the judge system per chip.
#[derive(Debug, Clone, Copy, Message)]
pub struct JudgmentEvent {
    /// Lane index (for HUD).
    pub lane: LaneId,
    /// Judgment kind.
    pub kind: dtx_scoring::JudgmentKind,
    /// Signed delta from target (ms). 0 = perfect.
    pub delta_ms: i32,
}

/// Emitted by the miss detector when a chip is past the judgment window.
#[derive(Debug, Clone, Copy, Message)]
pub struct NoteMissed {
    /// Lane index.
    pub lane: LaneId,
    /// AudioClock ms when miss was detected.
    pub audio_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_hit_press_sets_kind() {
        let h = LaneHit::press(2, 1000);
        assert_eq!(h.lane, 2);
        assert_eq!(h.audio_ms, 1000);
        assert_eq!(h.kind, LaneHitKind::Press);
    }

    #[test]
    fn lane_hit_kind_press_and_release_distinct() {
        assert_ne!(LaneHitKind::Press, LaneHitKind::Release);
    }
}
