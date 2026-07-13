//! Gameplay events (beve 0.19: `Event` was renamed to `Message`).

use crate::lane_map::LaneId;
use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use std::time::Instant;

/// Keyboard or pad hit detected.
///
/// `audio_ms` is the value of `AudioClock.current_ms` at the moment of input.
/// The judge system uses this against the chart's chip `target_ms`.
#[derive(Message, Debug, Clone, Copy)]
pub struct LaneHit {
    pub lane: LaneId,
    pub audio_ms: i64,
}

/// One physical input with its primary lane followed by accepted alternates.
///
/// Multi-target bindings are resolved as one atomic action by the judge; they
/// never fan out into several independently judged lane hits.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct InputHit {
    pub lanes: Vec<LaneId>,
    pub audio_ms: i64,
    /// Monotonic wall-clock timestamp captured at the physical input.
    /// Unlike `audio_ms`, this keeps advancing while practice wait freezes
    /// the gameplay clock.
    pub captured_at: Instant,
}

/// Judge result, dispatched by the judge system, consumed by score system + HUD.
#[derive(Message, Debug, Clone, Copy)]
pub struct JudgmentEvent {
    pub lane: LaneId,
    pub kind: JudgmentKind,
    pub delta_ms: i64,
    /// Index into `ActiveChart.chart.chips` for the chip that was judged.
    pub chip_idx: usize,
}

/// A chip that scrolled past the judgment line without being hit.
#[derive(Message, Debug, Clone, Copy)]
pub struct NoteMissed {
    pub lane: LaneId,
    pub audio_ms: i64,
    /// Index into `ActiveChart.chart.chips` for the missed chip.
    pub chip_idx: usize,
}

/// Pad press with no chip in the judgment window (empty hit / whiff).
#[derive(Message, Debug, Clone, Copy)]
pub struct EmptyHit {
    pub lane: LaneId,
    pub audio_ms: i64,
}

/// A bound system verb fired by a key or a pad. Emitted from `DrumsSets::Input`
/// before the gameplay-ready gate, so it works during live play; consumers gate
/// themselves (`pause.rs`).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemVerbHit {
    pub verb: dtx_input::SystemVerb,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_hit_construct() {
        let h = LaneHit {
            lane: 3,
            audio_ms: 12345,
        };
        assert_eq!(h.lane, 3);
        assert_eq!(h.audio_ms, 12345);
    }

    #[test]
    fn judgment_event_construct() {
        let j = JudgmentEvent {
            lane: 2,
            kind: JudgmentKind::Perfect,
            delta_ms: 5,
            chip_idx: 0,
        };
        assert_eq!(j.kind, JudgmentKind::Perfect);
    }

    #[test]
    fn note_missed_construct() {
        let m = NoteMissed {
            lane: 1,
            audio_ms: 99999,
            chip_idx: 7,
        };
        assert_eq!(m.lane, 1);
        assert_eq!(m.chip_idx, 7);
    }
}
