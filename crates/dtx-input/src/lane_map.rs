//! Fixed BocuD lane order + channel↔lane helpers.
//!
//! Moved from gameplay-drums (menu-nav extraction, 2026-07-15 spec). Owned by
//! dtx-input so bind resolution can map channels to lanes without a gameplay
//! dependency. Lane visual order matches DTXmania BocuD
//! (CActPerfDrumsLaneFlushD.cs).

use dtx_core::EChannel;

/// Lane index in the visual order (0..9).
pub type LaneId = u8;

/// Number of drum lanes in the fixed order.
pub const LANE_COUNT: usize = 12;

/// Channel → visual lane index. Index 0 is leftmost (HH … RD, LC, LP, LBD).
pub const LANE_ORDER: [EChannel; LANE_COUNT] = [
    EChannel::HiHatClose,
    EChannel::Snare,
    EChannel::BassDrum,
    EChannel::HighTom,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::HiHatOpen,
    EChannel::RideCymbal,
    EChannel::LeftCymbal,
    EChannel::LeftPedal,
    EChannel::LeftBassDrum,
];

/// Map a channel to its lane id (None if not a drum lane).
pub fn lane_of(channel: EChannel) -> Option<LaneId> {
    LANE_ORDER
        .iter()
        .position(|&c| c == channel)
        .map(|i| i as LaneId)
}

/// Map a lane id back to its channel. Panics if lane id out of range.
pub fn lane_channel(lane: LaneId) -> Option<EChannel> {
    LANE_ORDER.get(lane as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_order_matches_bocud() {
        assert_eq!(LANE_ORDER[0], EChannel::HiHatClose);
        assert_eq!(LANE_ORDER[2], EChannel::BassDrum);
        assert_eq!(LANE_ORDER[8], EChannel::RideCymbal);
    }

    #[test]
    fn lane_of_non_drum_is_none() {
        assert_eq!(lane_of(EChannel::BGM), None);
        assert_eq!(lane_of(EChannel::BarLine), None);
    }

    #[test]
    fn lane_channel_round_trip() {
        for (i, &c) in LANE_ORDER.iter().enumerate() {
            assert_eq!(lane_channel(i as u8), Some(c));
            assert_eq!(lane_of(c), Some(i as u8));
        }
    }

    #[test]
    fn lane_channel_out_of_range() {
        assert_eq!(lane_channel(99), None);
    }

    #[test]
    fn lane_count_is_twelve() {
        assert_eq!(LANE_ORDER.len(), 12);
    }
}
