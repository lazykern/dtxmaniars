//! Display-lane model: order, widths, channel→lane mapping.
//!
//! DISPLAY axis only. Judgment-side pad grouping lives in
//! `dtx_config::DrumsConfig` + `gameplay-drums/src/drum_groups.rs` (NX port)
//! and is deliberately untouched by this crate.

use std::collections::HashMap;

use dtx_core::EChannel;

pub const MIN_LANE_WIDTH: f32 = 24.0;
pub const MAX_LANE_WIDTH: f32 = 160.0;

/// The 12 drum channels, canonical order (matches `lane_map::LANE_ORDER` labels).
pub const DRUM_CHANNELS: [EChannel; 12] = [
    EChannel::LeftCymbal,
    EChannel::HiHatClose,
    EChannel::HiHatOpen,
    EChannel::LeftPedal,
    EChannel::LeftBassDrum,
    EChannel::Snare,
    EChannel::HighTom,
    EChannel::BassDrum,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::RideCymbal,
];

/// Canonical short name for a drum channel (used as lane ids + TOML keys).
pub fn channel_short_name(ch: EChannel) -> Option<&'static str> {
    Some(match ch {
        EChannel::LeftCymbal => "LC",
        EChannel::HiHatClose => "HH",
        EChannel::HiHatOpen => "HHO",
        EChannel::LeftPedal => "LP",
        EChannel::LeftBassDrum => "LBD",
        EChannel::Snare => "SD",
        EChannel::HighTom => "HT",
        EChannel::BassDrum => "BD",
        EChannel::LowTom => "LT",
        EChannel::FloorTom => "FT",
        EChannel::Cymbal => "CY",
        EChannel::RideCymbal => "RD",
        _ => return None,
    })
}

pub fn channel_from_short(name: &str) -> Option<EChannel> {
    DRUM_CHANNELS
        .into_iter()
        .find(|&ch| channel_short_name(ch) == Some(name))
}

/// Default ref-px width when a channel gets its own lane (ported from the
/// old `lane_geometry::COLUMNS` widths; split-out channels inherit their
/// merged sibling's width).
pub fn default_lane_width(ch: EChannel) -> f32 {
    match ch {
        EChannel::LeftCymbal => 72.0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 49.0,
        EChannel::LeftPedal | EChannel::LeftBassDrum => 51.0,
        EChannel::Snare => 57.0,
        EChannel::HighTom => 49.0,
        EChannel::BassDrum => 69.0,
        EChannel::LowTom => 49.0,
        EChannel::FloorTom => 54.0,
        EChannel::Cymbal => 70.0,
        EChannel::RideCymbal => 38.0,
        _ => 49.0,
    }
}

/// One on-screen lane column.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLane {
    /// Stable id — always a channel short name ("HH", "BD", …). v1 rule:
    /// lane ids are limited to channel short names so the primary channel
    /// is always derivable.
    pub id: String,
    pub label: String,
    /// Ref-px width, clamped to [MIN_LANE_WIDTH, MAX_LANE_WIDTH].
    pub width: f32,
    /// Base chip color (sRGB). `None` = derive from the primary channel's
    /// classic color at consumption time.
    pub color: Option<(f32, f32, f32)>,
    /// The channel this lane primarily represents (chips of other channels
    /// mapped here render as hollow "secondary" chips).
    pub primary: EChannel,
}

/// Runtime lane arrangement (display axis).
#[derive(Debug, Clone, PartialEq)]
pub struct LaneArrangement {
    pub preset: crate::presets::LanePreset,
    /// Display order left→right. Variable count (10 classic, 11 with HHO split…).
    pub lanes: Vec<DisplayLane>,
    /// Every drum channel maps to a lane id present in `lanes`.
    pub map: HashMap<EChannel, String>,
}

impl LaneArrangement {
    /// Index into `lanes` for a channel. None for non-drum channels.
    pub fn lane_index_of(&self, ch: EChannel) -> Option<usize> {
        let id = self.map.get(&ch)?;
        self.lanes.iter().position(|l| &l.id == id)
    }

    pub fn strip_ref_width(&self) -> f32 {
        self.lanes.iter().map(|l| l.width).sum()
    }

    /// Ref-px left offset of lane `i` measured from the strip's left edge.
    pub fn lane_ref_offset(&self, i: usize) -> f32 {
        self.lanes[..i].iter().map(|l| l.width).sum()
    }

    /// True when `ch` is a secondary chip on its lane (renders hollow).
    pub fn is_secondary(&self, ch: EChannel) -> bool {
        let Some(i) = self.lane_index_of(ch) else {
            return false;
        };
        self.lanes[i].primary != ch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn short_name_round_trip_for_all_drum_channels() {
        for ch in DRUM_CHANNELS {
            let name = channel_short_name(ch).expect("drum channel has a name");
            assert_eq!(channel_from_short(name), Some(ch), "round trip {name}");
        }
    }

    #[test]
    fn non_drum_channels_have_no_short_name() {
        assert_eq!(channel_short_name(EChannel::BGM), None);
        assert_eq!(channel_short_name(EChannel::BarLine), None);
    }

    #[test]
    fn default_width_defined_for_every_drum_channel() {
        for ch in DRUM_CHANNELS {
            assert!(default_lane_width(ch) >= MIN_LANE_WIDTH);
            assert!(default_lane_width(ch) <= MAX_LANE_WIDTH);
        }
    }

    #[test]
    fn arrangement_lane_index_lookup() {
        let arr = crate::presets::classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        assert_eq!(hh, hho, "classic merges HHO into HH lane");
        assert_eq!(arr.lanes[hh].id, "HH");
    }

    #[test]
    fn strip_width_is_sum_of_lane_widths() {
        let arr = crate::presets::classic();
        let sum: f32 = arr.lanes.iter().map(|l| l.width).sum();
        assert!((arr.strip_ref_width() - sum).abs() < f32::EPSILON);
    }
}
