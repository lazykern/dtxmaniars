//! Keyboard → lane mapping.
//!
//! Default drums layout: 9 lanes mapped to number keys 1–9.
//! Users re-bind later via `dtx-config`. Per ADR-0001: drums-first MVP.
//!
//! Lane visual order matches DTXmania BocuD (CActPerfDrumsLaneFlushD.cs),
//! excluding optional LC + LP lanes (those land in M5+ as extras).

use std::collections::HashMap;

use bevy::prelude::KeyCode;
use bevy::prelude::*;
use dtx_core::EChannel;

/// Lane index in the visual order (0..9).
pub type LaneId = u8;

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

/// User-rebindable keyboard → lane mapping. Persisted via dtx-config (M3+).
#[derive(Resource, Debug, Clone)]
pub struct LaneMap {
    /// LaneId → display name (for HUD + config UI).
    pub labels: [&'static str; LANE_COUNT],
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
    /// Default drums layout: DTXManiaIX / BocuD defaults.
    /// Ported from `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:3783`
    /// (`tSetDefaultKeyAssignments`). SlimDX `Key` codes decoded via
    /// `references/DTXmaniaNX-BocuD/FDK/Input/SlimDX.DirectInput.Key.cs`.
    pub fn default_drums() -> Self {
        let keys = [
            (KeyCode::KeyX, 0u8),      // HH     (SlimDX X = 33)
            (KeyCode::KeyC, 1),        // SD     (C)
            (KeyCode::KeyD, 1),        // SD     (D)
            (KeyCode::Space, 2),       // BD     (Space)
            (KeyCode::Convert, 2),     // BD     (Convert)
            (KeyCode::KeyV, 3),        // HT     (V)
            (KeyCode::KeyF, 3),        // HT     (F)
            (KeyCode::KeyB, 4),        // LT     (B)
            (KeyCode::KeyG, 4),        // LT     (G)
            (KeyCode::KeyN, 5),        // FT     (N)
            (KeyCode::KeyH, 5),        // FT     (H)
            (KeyCode::KeyM, 6),        // CY     (M)
            (KeyCode::KeyJ, 6),        // CY     (J)
            (KeyCode::KeyS, 7),        // HHO    (S)
            (KeyCode::Comma, 8),       // RD     (Comma)
            (KeyCode::KeyK, 8),        // RD     (K)
            (KeyCode::KeyZ, 9),        // LC     (Z)
            (KeyCode::KeyA, 9),        // LC     (A)
            (KeyCode::NonConvert, 10), // LP     (NoConvert)
            (KeyCode::AltLeft, 11),    // LBD    (LeftAlt)
        ]
        .into_iter()
        .map(|(k, v)| (k, v as LaneId))
        .collect();

        Self {
            labels: [
                "HH", "SD", "BD", "HT", "LT", "FT", "CY", "HHO", "RD", "LC", "LP", "LBD",
            ],
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
    fn default_maps_bocud_drums() {
        // references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:3783
        let m = LaneMap::default_drums();
        assert_eq!(m.lane_for_key(KeyCode::KeyX), Some(0)); // HH
        assert_eq!(m.lane_for_key(KeyCode::KeyC), Some(1)); // SD
        assert_eq!(m.lane_for_key(KeyCode::KeyD), Some(1)); // SD alt
        assert_eq!(m.lane_for_key(KeyCode::Space), Some(2)); // BD
        assert_eq!(m.lane_for_key(KeyCode::Convert), Some(2)); // BD alt
        assert_eq!(m.lane_for_key(KeyCode::KeyV), Some(3)); // HT
        assert_eq!(m.lane_for_key(KeyCode::KeyF), Some(3)); // HT alt
        assert_eq!(m.lane_for_key(KeyCode::KeyB), Some(4)); // LT
        assert_eq!(m.lane_for_key(KeyCode::KeyG), Some(4)); // LT alt
        assert_eq!(m.lane_for_key(KeyCode::KeyN), Some(5)); // FT
        assert_eq!(m.lane_for_key(KeyCode::KeyH), Some(5)); // FT alt
        assert_eq!(m.lane_for_key(KeyCode::KeyM), Some(6)); // CY
        assert_eq!(m.lane_for_key(KeyCode::KeyJ), Some(6)); // CY alt
        assert_eq!(m.lane_for_key(KeyCode::KeyS), Some(7)); // HHO
        assert_eq!(m.lane_for_key(KeyCode::Comma), Some(8)); // RD
        assert_eq!(m.lane_for_key(KeyCode::KeyK), Some(8)); // RD alt
        assert_eq!(m.lane_for_key(KeyCode::KeyZ), Some(9)); // LC
        assert_eq!(m.lane_for_key(KeyCode::KeyA), Some(9)); // LC alt
        assert_eq!(m.lane_for_key(KeyCode::NonConvert), Some(10)); // LP
        assert_eq!(m.lane_for_key(KeyCode::AltLeft), Some(11)); // LBD
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
        // Space is the primary BD binding (BocuD default).
        use dtx_input::keyboard::KeyLaneMap;
        assert_eq!(KeyLaneMap::lane_for_key(&m, KeyCode::Space), Some(2));
    }

    #[test]
    fn lane_count_is_twelve() {
        assert_eq!(LANE_ORDER.len(), 12);
    }
}
