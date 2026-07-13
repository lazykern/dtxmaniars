#![allow(non_snake_case)]
//! `CChartData` (472 LOC) — chart-level data container.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CChartData.cs:1-472`
//!
//! v1 strict-port: holds per-chart data that spans the loading and
//! playback phases (BGA list, BPM list, chart-specific metadata).

use crate::channel::EChannel;
use crate::chart::Chart;

/// One BGA entry (BocuD CChartData.cs:30-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BgaEntry {
    /// Measure when this BGA frame should appear.
    pub measure: u32,
    /// BMP filename index (BocuD CChartData.cs:50).
    pub bmp_index: u16,
    /// BGA layer (1-8).
    pub layer: u8,
}

impl BgaEntry {
    /// Build a BGA entry.
    pub fn new(measure: u32, bmp_index: u16, layer: u8) -> Self {
        Self {
            measure,
            bmp_index,
            layer,
        }
    }
}

/// Per-chart data (BocuD CChartData.cs:60-100).
#[derive(Debug, Clone, Default)]
pub struct CChartData {
    /// Source path of the chart.
    pub file_path: Option<std::path::PathBuf>,
    /// Parsed chart.
    pub chart: Chart,
    /// BGA frames in playback order.
    pub bga_frames: Vec<BgaEntry>,
    /// BPM changes (position + bpm, no playback time).
    pub bpm_changes: Vec<crate::timing::BpmChange>,
    /// Pre-image (preview.jpg path).
    pub preimage: Option<std::path::PathBuf>,
    /// Preview audio (preview.ogg path).
    pub preview: Option<std::path::PathBuf>,
    /// Difficulty level (0-99).
    pub dlevel: i32,
    /// Guitar difficulty (0-99).
    pub glevel: i32,
    /// Bass difficulty (0-99).
    pub blevel: i32,
    /// Drum difficulty (0-99).
    pub drumlevel: i32,
}

impl CChartData {
    /// Build from a parsed Chart with no BGA/preview.
    pub fn from_chart(chart: Chart) -> Self {
        let mut data = Self {
            file_path: None,
            chart,
            bga_frames: Vec::new(),
            bpm_changes: Vec::new(),
            preimage: None,
            preview: None,
            dlevel: 0,
            glevel: 0,
            blevel: 0,
            drumlevel: 0,
        };
        data.collect_bga();
        data.collect_bpm();
        data
    }

    fn collect_bga(&mut self) {
        for chip in &self.chart.chips {
            if let Some(layer) = bga_layer(chip.channel) {
                self.bga_frames.push(BgaEntry {
                    measure: chip.measure,
                    bmp_index: chip.value as u16,
                    layer,
                });
            }
        }
        self.bga_frames.sort_by_key(|b| b.measure);
    }

    fn collect_bpm(&mut self) {
        self.bpm_changes = crate::timing::bpm_changes_from_chart(&self.chart);
    }

    /// Number of BGA frames.
    pub fn bga_count(&self) -> usize {
        self.bga_frames.len()
    }

    /// Number of BPM changes (excluding base BPM).
    pub fn bpm_change_count(&self) -> usize {
        self.bpm_changes.len()
    }

    /// Average difficulty across instruments.
    pub fn average_difficulty(&self) -> f32 {
        let (sum, count) = [self.dlevel, self.glevel, self.blevel, self.drumlevel]
            .iter()
            .fold(
                (0i32, 0i32),
                |(s, c), &v| {
                    if v > 0 {
                        (s + v, c + 1)
                    } else {
                        (s, c)
                    }
                },
            );
        if count == 0 {
            0.0
        } else {
            sum as f32 / count as f32
        }
    }
}

fn bga_layer(channel: EChannel) -> Option<u8> {
    match channel {
        EChannel::BGALayer1 => Some(1),
        EChannel::BGALayer2 => Some(2),
        EChannel::BGALayer3 => Some(3),
        EChannel::BGALayer4 => Some(4),
        EChannel::BGALayer5 => Some(5),
        EChannel::BGALayer6 => Some(6),
        EChannel::BGALayer7 => Some(7),
        EChannel::BGALayer8 => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chart() -> Chart {
        use crate::channel::EChannel;
        use crate::chart::{Chip, Metadata};
        Chart {
            metadata: Metadata {
                title: Some("Test".into()),
                artist: None,
                genre: None,
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::new(0, EChannel::BGALayer1, 1.0),
                Chip::new(1, EChannel::BGALayer2, 2.0),
                Chip::new(0, EChannel::BPM, 180.0),
                Chip::new(2, EChannel::BassDrum, 0.0),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn from_chart_collects_bga() {
        let d = CChartData::from_chart(make_chart());
        assert_eq!(d.bga_count(), 2);
        assert_eq!(d.bga_frames[0].layer, 1);
        assert_eq!(d.bga_frames[0].bmp_index, 1);
    }

    #[test]
    fn from_chart_collects_bpm() {
        let d = CChartData::from_chart(make_chart());
        assert_eq!(d.bpm_change_count(), 1);
        assert_eq!(d.bpm_changes[0].bpm, 180.0);
    }

    #[test]
    fn from_chart_ignores_drums_in_bga() {
        let d = CChartData::from_chart(make_chart());
        // BassDrum chip should not appear in BGA
        for bga in &d.bga_frames {
            assert!(bga.layer >= 1 && bga.layer <= 8);
        }
    }

    #[test]
    fn bga_frames_sorted_by_measure() {
        let d = CChartData::from_chart(make_chart());
        for w in d.bga_frames.windows(2) {
            assert!(w[0].measure <= w[1].measure);
        }
    }

    #[test]
    fn bpm_changes_sorted_by_measure() {
        let d = CChartData::from_chart(make_chart());
        for w in d.bpm_changes.windows(2) {
            assert!(w[0].measure <= w[1].measure);
        }
    }

    #[test]
    fn bga_entry_new() {
        let b = BgaEntry::new(0, 5, 3);
        assert_eq!(b.measure, 0);
        assert_eq!(b.bmp_index, 5);
        assert_eq!(b.layer, 3);
    }

    #[test]
    fn average_difficulty_no_levels() {
        let d = CChartData::from_chart(make_chart());
        assert_eq!(d.average_difficulty(), 0.0);
    }

    #[test]
    fn average_difficulty_with_levels() {
        let mut d = CChartData::from_chart(make_chart());
        d.dlevel = 50;
        d.glevel = 70;
        d.blevel = 30;
        d.drumlevel = 60;
        assert!((d.average_difficulty() - 52.5).abs() < 0.01);
    }

    #[test]
    fn bga_layer_mapping() {
        assert_eq!(bga_layer(EChannel::BGALayer1), Some(1));
        assert_eq!(bga_layer(EChannel::BGALayer8), Some(8));
        assert_eq!(bga_layer(EChannel::BassDrum), None);
        assert_eq!(bga_layer(EChannel::BGM), None);
    }
}
