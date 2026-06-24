//! Guitar keyboard → lane mapping.
//!
//! Default guitar layout: 5 lanes (R/G/B/Y/P) mapped to A/S/D/F/G.
//! Per BocuD `CStagePerfGuitarScreen.cs:99-105` the visual layout is left-to-right
//! R/G/B/Y/P at screen x = [107, 146, 185, 224, 264].
//!
//! ## M6b scope
//!
//! Only single-note channels map to a single lane. Open (0x20) and chord
//! channels (0x23, 0x25-0x27, 0x94-0x97, etc.) are NOT mapped — chord
//! judgment lands in M6.1.

use std::collections::HashMap;

use bevy::prelude::KeyCode;
use bevy::prelude::*;

/// Lane index in the visual order (0..4).
pub type LaneId = u8;

/// Visual lane order, left to right, matching BocuD
/// `CStagePerfGuitarScreen.cs:99-105`.
pub const GUITAR_LANES: [&str; 5] = ["R", "G", "B", "Y", "P"];

/// Number of guitar lanes.
pub const GUITAR_LANE_COUNT: u8 = 5;

/// Map a chip's EChannel to a guitar lane. Returns None if the channel is
/// not a guitar single-note.
///
/// ## M6b coverage
///
/// - 0x24 (Guitar_Rxxxx) → R lane (0)
/// - 0x22 (GuitarRxGxx) → G lane (1)
/// - 0x21 (GuitarRxxBxx) → B lane (2)
/// - 0x93 (GuitarYxxYx) → Y lane (3)
/// - 0xA3 (GuitarPxx) → P lane (4)
///
/// Open (0x20) and chords return None — M6.1.
pub fn lane_of(channel: dtx_core::EChannel) -> Option<LaneId> {
    use dtx_core::EChannel as C;
    let lane = match channel {
        C::GuitarRxxxx => 0,  // R  (0x24)
        C::GuitarRGxxx => 0,  // R+G chord → R lane (M6b: lowest lane wins)
        C::GuitarRxBxx => 0,  // R+B → R
        C::GuitarRGBxx => 0,  // R+G+B → R
        C::GuitarRxGxx => 1,  // G  (0x22)
        C::GuitarRxGBxx => 1, // G+B → G
        C::GuitarRxxBxx => 2, // B  (0x21)
        C::GuitarYxxYx => 3,  // Y  (0x93)
        C::GuitarPxx => 4,    // P  (0xA3)
        _ => return None,
    };
    Some(lane)
}

/// Map a lane id back to its label. Used by HUD.
pub fn lane_label(lane: LaneId) -> &'static str {
    GUITAR_LANES.get(lane as usize).copied().unwrap_or("?")
}

/// Inverse: lane id → representative channel (used for tests + documentation).
pub fn lane_channel(lane: LaneId) -> Option<dtx_core::EChannel> {
    use dtx_core::EChannel as C;
    match lane {
        0 => Some(C::GuitarRxxxx),
        1 => Some(C::GuitarRxGxx),
        2 => Some(C::GuitarRxxBxx),
        3 => Some(C::GuitarYxxYx),
        4 => Some(C::GuitarPxx),
        _ => None,
    }
}

/// User-rebindable keyboard → lane mapping.
#[derive(Resource, Debug, Clone)]
pub struct LaneMap {
    /// LaneId → display name.
    pub labels: [&'static str; 5],
    /// KeyCode → LaneId.
    pub keys: HashMap<KeyCode, LaneId>,
}

impl Default for LaneMap {
    fn default() -> Self {
        Self::default_guitar()
    }
}

impl LaneMap {
    /// Default guitar layout: A/S/D/F/G mapped left to right.
    pub fn default_guitar() -> Self {
        let keys = [
            (KeyCode::KeyA, 0u8), // R
            (KeyCode::KeyS, 1),   // G
            (KeyCode::KeyD, 2),   // B
            (KeyCode::KeyF, 3),   // Y
            (KeyCode::KeyG, 4),   // P
        ]
        .into_iter()
        .map(|(k, v)| (k, v as LaneId))
        .collect();

        Self {
            labels: ["R", "G", "B", "Y", "P"],
            keys,
        }
    }

    /// Look up the lane id for a key press.
    pub fn lane_for_key(&self, key: KeyCode) -> Option<LaneId> {
        self.keys.get(&key).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn default_maps_5_letters() {
        let m = LaneMap::default_guitar();
        assert_eq!(m.lane_for_key(KeyCode::KeyA), Some(0));
        assert_eq!(m.lane_for_key(KeyCode::KeyS), Some(1));
        assert_eq!(m.lane_for_key(KeyCode::KeyD), Some(2));
        assert_eq!(m.lane_for_key(KeyCode::KeyF), Some(3));
        assert_eq!(m.lane_for_key(KeyCode::KeyG), Some(4));
    }

    #[test]
    fn unknown_key_returns_none() {
        let m = LaneMap::default_guitar();
        assert_eq!(m.lane_for_key(KeyCode::Digit1), None);
        assert_eq!(m.lane_for_key(KeyCode::KeyZ), None);
    }

    #[test]
    fn lane_order_matches_bocud() {
        assert_eq!(GUITAR_LANES[0], "R");
        assert_eq!(GUITAR_LANES[1], "G");
        assert_eq!(GUITAR_LANES[2], "B");
        assert_eq!(GUITAR_LANES[3], "Y");
        assert_eq!(GUITAR_LANES[4], "P");
    }

    #[test]
    fn lane_of_single_notes() {
        assert_eq!(lane_of(EChannel::GuitarRxxxx), Some(0));
        assert_eq!(lane_of(EChannel::GuitarRxGxx), Some(1));
        assert_eq!(lane_of(EChannel::GuitarRxxBxx), Some(2));
        assert_eq!(lane_of(EChannel::GuitarYxxYx), Some(3));
        assert_eq!(lane_of(EChannel::GuitarPxx), Some(4));
    }

    #[test]
    fn lane_of_chords_map_to_lowest_lane_m6b() {
        // M6b: chord channels return the lowest lane (R for RxBxx, etc.)
        // M6.1 will replace with multi-lane judgment.
        assert_eq!(lane_of(EChannel::GuitarRGxxx), Some(0));
        assert_eq!(lane_of(EChannel::GuitarRGBxx), Some(0));
        assert_eq!(lane_of(EChannel::GuitarRxGBxx), Some(1));
    }

    #[test]
    fn lane_of_non_guitar_returns_none() {
        assert_eq!(lane_of(EChannel::BGM), None);
        assert_eq!(lane_of(EChannel::BarLine), None);
        assert_eq!(lane_of(EChannel::BassDrum), None);
    }

    #[test]
    fn lane_label_roundtrip() {
        for (i, expected) in GUITAR_LANES.iter().enumerate() {
            let l = lane_label(i as u8);
            assert_eq!(&l, expected);
        }
    }

    #[test]
    fn lane_channel_roundtrip() {
        for i in 0..5u8 {
            assert_eq!(lane_of(lane_channel(i).unwrap()), Some(i));
        }
    }
}
