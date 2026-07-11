//! Runtime lane arrangement resource (display axis).
//!
//! Wraps `dtx_layout::LaneArrangement`. Replaces the old compile-time
//! `lane_geometry::COLUMNS`. Judgment-side grouping stays in `drum_groups.rs`.

use bevy::prelude::*;
use dtx_core::EChannel;

/// `$XDG_CONFIG_HOME/dtxmaniars/lane-profiles.toml` (next to layout.toml).
pub fn lane_registry_path() -> std::path::PathBuf {
    let mut p = dtx_layout::default_path();
    p.set_file_name("lane-profiles.toml");
    p
}

/// Display lane arrangement. Default = classic (legacy NX Type-A geometry).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Lanes(pub dtx_layout::LaneArrangement);

impl Default for Lanes {
    fn default() -> Self {
        Self(dtx_layout::classic())
    }
}

impl Lanes {
    pub fn count(&self) -> usize {
        self.0.lanes.len()
    }

    pub fn label(&self, i: usize) -> &str {
        &self.0.lanes[i].label
    }

    /// Ref-px offset of column `i` from the strip's left edge.
    pub fn ref_offset(&self, i: usize) -> f32 {
        self.0.lane_ref_offset(i)
    }

    pub fn ref_width(&self, i: usize) -> f32 {
        self.0.lanes[i].width
    }

    pub fn strip_ref_width(&self) -> f32 {
        self.0.strip_ref_width()
    }

    /// Visual column for a channel (None for non-drum chips).
    pub fn col_of(&self, channel: EChannel) -> Option<usize> {
        self.0.lane_index_of(channel)
    }

    fn lane_base_color(&self, i: usize) -> (f32, f32, f32) {
        self.0.lanes[i]
            .color
            .unwrap_or_else(|| classic_channel_color(self.0.lanes[i].primary))
    }

    /// Column base color as a Bevy `Color`.
    pub fn column_color(&self, i: usize) -> Color {
        let (r, g, b) = if i < self.count() {
            self.lane_base_color(i)
        } else {
            (1.0, 1.0, 1.0)
        };
        Color::srgb(r, g, b)
    }

    /// Chip color: lane base, with the legacy secondary variants (HHO reads
    /// brighter, LBD darker) applied when the chip is a secondary on its lane.
    pub fn chip_color(&self, channel: EChannel) -> Color {
        let Some(col) = self.col_of(channel) else {
            return Color::WHITE;
        };
        let (r, g, b) = self.lane_base_color(col);
        if !self.0.is_secondary(channel) {
            return Color::srgb(r, g, b);
        }
        match channel {
            EChannel::HiHatOpen => Color::srgb((r + 0.25).min(1.0), (g + 0.15).min(1.0), 1.0),
            EChannel::LeftBassDrum => Color::srgb(r * 0.6, g * 0.6, b * 0.6),
            _ => Color::srgb(r, g, b),
        }
    }

    /// Secondary chips render hollow (outline) to stay distinct from the
    /// filled primary sharing their lane.
    pub fn is_hollow(&self, channel: EChannel) -> bool {
        self.0.is_secondary(channel)
    }
}

/// Classic per-channel base color (matches `dtx_layout::classic()` lane colors),
/// used as the fallback when a custom lane has no explicit color. Const lookup,
/// no allocation (called per-chip in the note-spawn hot path).
fn classic_channel_color(ch: EChannel) -> (f32, f32, f32) {
    match ch {
        EChannel::LeftCymbal => (0.945, 0.247, 0.725),
        EChannel::HiHatClose | EChannel::HiHatOpen => (0.000, 0.541, 1.000),
        EChannel::LeftPedal | EChannel::LeftBassDrum => (1.000, 0.353, 0.627),
        EChannel::Snare => (0.941, 0.824, 0.000),
        EChannel::HighTom => (0.157, 0.765, 0.157),
        EChannel::BassDrum => (0.588, 0.353, 0.941),
        EChannel::LowTom => (0.882, 0.176, 0.176),
        EChannel::FloorTom => (1.000, 0.659, 0.000),
        EChannel::Cymbal => (1.000, 0.471, 0.000),
        EChannel::RideCymbal => (0.000, 0.541, 1.000),
        _ => (1.0, 1.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn classic_matches_legacy_geometry() {
        let lanes = Lanes::default();
        let legacy: [(&str, f32, f32); 10] = [
            ("LC", 0.0, 72.0),
            ("HH", 72.0, 49.0),
            ("LP", 121.0, 51.0),
            ("SD", 172.0, 57.0),
            ("HT", 229.0, 49.0),
            ("BD", 278.0, 69.0),
            ("LT", 347.0, 49.0),
            ("FT", 396.0, 54.0),
            ("CY", 450.0, 70.0),
            ("RD", 520.0, 38.0),
        ];
        assert_eq!(lanes.count(), 10);
        for (i, (label, off, w)) in legacy.iter().enumerate() {
            assert_eq!(lanes.label(i), *label);
            assert!(
                (lanes.ref_offset(i) - off).abs() < 0.01,
                "lane {label} offset"
            );
            assert!((lanes.ref_width(i) - w).abs() < 0.01, "lane {label} width");
        }
        assert!((lanes.strip_ref_width() - 558.0).abs() < 0.01);
    }

    #[test]
    fn col_of_matches_legacy_mapping() {
        let lanes = Lanes::default();
        assert_eq!(lanes.col_of(EChannel::LeftCymbal), Some(0));
        assert_eq!(
            lanes.col_of(EChannel::HiHatOpen),
            lanes.col_of(EChannel::HiHatClose)
        );
        assert_eq!(
            lanes.col_of(EChannel::LeftBassDrum),
            lanes.col_of(EChannel::BassDrum)
        );
        assert_eq!(lanes.col_of(EChannel::BGM), None);
    }

    #[test]
    fn secondary_chips_hollow_and_tinted() {
        let lanes = Lanes::default();
        assert!(lanes.is_hollow(EChannel::HiHatOpen));
        assert!(lanes.is_hollow(EChannel::LeftBassDrum));
        assert!(!lanes.is_hollow(EChannel::HiHatClose));
        assert_ne!(
            lanes.chip_color(EChannel::HiHatOpen),
            lanes.chip_color(EChannel::HiHatClose)
        );
        assert_ne!(
            lanes.chip_color(EChannel::LeftBassDrum),
            lanes.chip_color(EChannel::BassDrum)
        );
    }

    #[test]
    fn split_arrangement_gives_hho_its_own_column() {
        let section = dtx_layout::LanesSection {
            preset: dtx_layout::LanePreset::Custom,
            order: Some(
                [
                    "LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ),
            map: Some([("HHO".to_string(), "HHO".to_string())].into()),
            ..Default::default()
        };
        let lanes = Lanes(section.resolve());
        assert_eq!(lanes.count(), 11);
        assert_ne!(
            lanes.col_of(EChannel::HiHatOpen),
            lanes.col_of(EChannel::HiHatClose)
        );
        assert!(!lanes.is_hollow(EChannel::HiHatOpen));
    }
}
