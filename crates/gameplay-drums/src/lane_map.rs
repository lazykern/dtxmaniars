//! Keyboard → lane mapping.
//!
//! Default drums layout: 9 lanes mapped to number keys 1–9.
//! Users re-bind later via `dtx-config`. Per ADR-0001: drums-first MVP.
//!
//! Lane visual order matches DTXmania BocuD (CActPerfDrumsLaneFlushD.cs),
//! excluding optional LC + LP lanes (those land in M5+ as extras).

use std::collections::HashMap;

use bevy::prelude::KeyCode;
use bevy::prelude::Resource as _;
use bevy::prelude::*;
use dtx_core::EChannel;

/// Lane index in the visual order (0..9).
pub type LaneId = u8;

/// Channel → visual lane index. Index 0 is leftmost.
pub const LANE_ORDER: [EChannel; 9] = [
    EChannel::HiHatClose,
    EChannel::Snare,
    EChannel::BassDrum,
    EChannel::HighTom,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::HiHatOpen,
    EChannel::RideCymbal,
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

/// User-rebindable keyboard → lane mapping. Persisted via dtx-config (M3+).
#[derive(Resource, Debug, Clone)]
pub struct LaneMap {
    /// LaneId → display name (for HUD + config UI).
    pub labels: [&'static str; 9],
    /// KeyCode → LaneId. May have duplicates for two-key assignments.
    pub keys: HashMap<KeyCode, LaneId>,
}

impl Default for LaneMap {
    fn default() -> Self {
        Self::default_drums()
    }
}

impl dtx_input::keyboard::KeyLaneMap for LaneMap {
    fn lane_for_key(&self, key: KeyCode) -> Option<dtx_input::LaneId> {
        self.lane_for_key(key)
    }
}

impl LaneMap {
    /// Default drums layout: number row 1–9 mapped left to right.
    pub fn default_drums() -> Self {
        let keys = [
            (KeyCode::Digit1, 0u8), // HH
            (KeyCode::Digit2, 1),   // SD
            (KeyCode::Digit3, 2),   // BD
            (KeyCode::Digit4, 3),   // HT
            (KeyCode::Digit5, 4),   // LT
            (KeyCode::Digit6, 5),   // FT
            (KeyCode::Digit7, 6),   // CY
            (KeyCode::Digit8, 7),   // HHO
            (KeyCode::Digit9, 8),   // RD
        ]
        .into_iter()
        .map(|(k, v)| (k, v as LaneId))
        .collect();

        Self {
            labels: ["HH", "SD", "BD", "HT", "LT", "FT", "CY", "HHO", "RD"],
            keys,
        }
    }

    /// Look up the lane id for a key press. Returns None if unmapped.
    pub fn lane_for_key(&self, key: KeyCode) -> Option<LaneId> {
        self.keys.get(&key).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_maps_digits_1_through_9() {
        let m = LaneMap::default_drums();
        assert_eq!(m.lane_for_key(KeyCode::Digit1), Some(0));
        assert_eq!(m.lane_for_key(KeyCode::Digit9), Some(8));
        assert_eq!(m.lane_for_key(KeyCode::Digit0), None);
        assert_eq!(m.lane_for_key(KeyCode::KeyA), None);
    }

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
    fn default_labels_match_lane_order() {
        let m = LaneMap::default_drums();
        assert_eq!(m.labels[0], "HH");
        assert_eq!(m.labels[2], "BD");
        assert_eq!(m.labels[8], "RD");
    }

    #[test]
    fn key_lane_map_trait_impl() {
        let m = LaneMap::default_drums();
        // Use the trait to look up.
        use dtx_input::keyboard::KeyLaneMap;
        assert_eq!(KeyLaneMap::lane_for_key(&m, KeyCode::Digit3), Some(2));
    }

    #[test]
    fn lane_count_is_nine() {
        assert_eq!(LANE_ORDER.len(), 9);
    }
}
