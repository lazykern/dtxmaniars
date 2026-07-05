//! Visual lane columns (DTXManiaNX geometry).
//!
//! The input/judge/score model uses 12 `EChannel`/`LaneId` lanes (see `lane_map`).
//! The *render* layer collapses those into 10 on-screen columns matching NX:
//! order LC HH LP SD HT BD LT FT CY RD, with open hi-hat drawn on the HH column
//! and left bass on the BD column. Geometry derived from `CActPerfDrumsPad.cs`
//! pad bases + `CActPerfDrumsLaneFlushD.cs` flush rects at 1280x720 (EType.A/RCRD).

use bevy::prelude::Color;
use dtx_core::EChannel;

pub const COLUMN_COUNT: usize = 10;

pub struct Column {
    pub label: &'static str,
    pub ref_x: f32,
    pub ref_w: f32,
    /// Base chip color as an sRGB tuple.
    pub color: (f32, f32, f32),
}

/// Columns ordered left→right at 1280x720. Contiguous, strip = x 295..853 (w 558).
// GITADORA note-chip palette (sampled from real gameplay / DTXManiaNX pad sheet):
// LC magenta, HH blue, LP pink, SD yellow, HT green, BD purple, LT red,
// FT amber, CY orange, RD blue.
pub const COLUMNS: [Column; COLUMN_COUNT] = [
    Column { label: "LC", ref_x: 295.0, ref_w: 72.0, color: (0.945, 0.247, 0.725) }, // magenta
    Column { label: "HH", ref_x: 367.0, ref_w: 49.0, color: (0.000, 0.541, 1.000) }, // blue
    Column { label: "LP", ref_x: 416.0, ref_w: 51.0, color: (1.000, 0.353, 0.627) }, // pink
    Column { label: "SD", ref_x: 467.0, ref_w: 57.0, color: (0.941, 0.824, 0.000) }, // yellow
    Column { label: "HT", ref_x: 524.0, ref_w: 49.0, color: (0.157, 0.765, 0.157) }, // green
    Column { label: "BD", ref_x: 573.0, ref_w: 69.0, color: (0.588, 0.353, 0.941) }, // purple
    Column { label: "LT", ref_x: 642.0, ref_w: 49.0, color: (0.882, 0.176, 0.176) }, // red
    Column { label: "FT", ref_x: 691.0, ref_w: 54.0, color: (1.000, 0.659, 0.000) }, // amber
    Column { label: "CY", ref_x: 745.0, ref_w: 70.0, color: (1.000, 0.471, 0.000) }, // orange
    Column { label: "RD", ref_x: 815.0, ref_w: 38.0, color: (0.000, 0.541, 1.000) }, // blue
];

/// Strip left edge / total width at ref resolution.
pub const STRIP_REF_LEFT: f32 = 295.0;
pub const STRIP_REF_WIDTH: f32 = 558.0;

/// EChannel → visual column index. HHO→HH col, LBD→BD col. None if not a drum chip.
pub fn column_of(channel: EChannel) -> Option<usize> {
    Some(match channel {
        EChannel::LeftCymbal => 0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 1,
        EChannel::LeftPedal => 2,
        EChannel::Snare => 3,
        EChannel::HighTom => 4,
        EChannel::BassDrum | EChannel::LeftBassDrum => 5,
        EChannel::LowTom => 6,
        EChannel::FloorTom => 7,
        EChannel::Cymbal => 8,
        EChannel::RideCymbal => 9,
        _ => return None,
    })
}

/// Chip color for a channel: column base, with a distinct variant for the merged
/// secondary chips (HHO reads brighter than HH; LBD reads darker than BD).
pub fn chip_color(channel: EChannel) -> Color {
    let Some(col) = column_of(channel) else {
        return Color::WHITE;
    };
    let (r, g, b) = COLUMNS[col].color;
    match channel {
        EChannel::HiHatOpen => Color::srgb((r + 0.25).min(1.0), (g + 0.15).min(1.0), 1.0),
        EChannel::LeftBassDrum => Color::srgb(r * 0.6, g * 0.6, b * 0.6),
        _ => Color::srgb(r, g, b),
    }
}

/// Merged-secondary chips render as an outline (transparent fill) so they read
/// distinct from the filled primary sharing their column: HHO vs HH, LBD vs BD.
pub fn is_hollow(channel: EChannel) -> bool {
    matches!(channel, EChannel::HiHatOpen | EChannel::LeftBassDrum)
}

/// Column base color as a Bevy `Color`.
pub fn column_color(col: usize) -> Color {
    let (r, g, b) = COLUMNS.get(col).map(|c| c.color).unwrap_or((1.0, 1.0, 1.0));
    Color::srgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_columns_ordered_and_contiguous() {
        assert_eq!(COLUMNS.len(), COLUMN_COUNT);
        for w in COLUMNS.windows(2) {
            assert!(w[1].ref_x >= w[0].ref_x, "columns must be left→right");
            assert!(
                (w[0].ref_x + w[0].ref_w - w[1].ref_x).abs() < 1.0,
                "columns should be contiguous: {} end {} vs {} start {}",
                w[0].label,
                w[0].ref_x + w[0].ref_w,
                w[1].label,
                w[1].ref_x
            );
        }
    }

    #[test]
    fn strip_bounds_match_constants() {
        assert_eq!(COLUMNS[0].ref_x, STRIP_REF_LEFT);
        let last = &COLUMNS[COLUMN_COUNT - 1];
        assert!((last.ref_x + last.ref_w - (STRIP_REF_LEFT + STRIP_REF_WIDTH)).abs() < 1.0);
    }

    #[test]
    fn hho_maps_to_hh_column_bd_lbd_shared() {
        assert_eq!(column_of(EChannel::HiHatOpen), column_of(EChannel::HiHatClose));
        assert_eq!(column_of(EChannel::LeftBassDrum), column_of(EChannel::BassDrum));
        assert_eq!(COLUMNS[column_of(EChannel::HiHatClose).unwrap()].label, "HH");
        assert_eq!(COLUMNS[column_of(EChannel::BassDrum).unwrap()].label, "BD");
    }

    #[test]
    fn canonical_order_left_to_right() {
        let labels: Vec<_> = COLUMNS.iter().map(|c| c.label).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
        );
    }

    #[test]
    fn secondary_chips_distinct_from_primary() {
        assert_ne!(chip_color(EChannel::HiHatOpen), chip_color(EChannel::HiHatClose));
        assert_ne!(
            chip_color(EChannel::LeftBassDrum),
            chip_color(EChannel::BassDrum)
        );
    }

    #[test]
    fn non_drum_channel_has_no_column() {
        assert_eq!(column_of(EChannel::BGM), None);
        assert_eq!(column_of(EChannel::BarLine), None);
    }

    #[test]
    fn only_open_hh_and_left_bass_are_hollow() {
        assert!(is_hollow(EChannel::HiHatOpen));
        assert!(is_hollow(EChannel::LeftBassDrum));
        assert!(!is_hollow(EChannel::HiHatClose));
        assert!(!is_hollow(EChannel::BassDrum));
        assert!(!is_hollow(EChannel::Snare));
    }
}
