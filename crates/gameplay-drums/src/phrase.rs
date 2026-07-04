//! Phrase detection — BocuD `CActPerfProgressBar` port.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfProgressBar.cs`.
//!
//! BocuD partitions the chart into `nSectionIntervalCount = 64` time slices
//! and counts drum chips per slice. The visual phrase meter is drawn as a
//! vertical column of varying-width blocks proportional to each slice's
//! chip count (`nChipCount / base / nSectionIntervalCount`, capped at
//! `nブロック最大数 = 10`).
//!
//! Ponytail: `chip_time_ms_with_bpm_changes` is already exposed by `dtx-timing`
//! — we just walk drum chips and bucket them. No new math.

use bevy::prelude::Resource;
use dtx_core::Chart;
use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};

/// BocuD constant (CActPerfProgressBar.cs:514). Number of vertical sections.
pub const PHRASE_SECTION_COUNT: usize = 64;

/// BocuD constant (CActPerfProgressBar.cs:33-36). Chips/Drums baseline density.
const DRUMS_CHIP_BASELINE: f64 = 1600.0;

/// BocuD constant (CActPerfProgressBar.cs:32). Max block width units.
const BLOCKS_MAX: u32 = 10;

/// Phrase meter state — per-section chip count + total chart duration.
///
/// Rebuilt once on `OnEnter(AppState::Performance)` from the active chart.
#[derive(Debug, Clone, Resource)]
pub struct PhraseMeter {
    /// Chip count per section (length = PHRASE_SECTION_COUNT).
    pub sections: [u32; PHRASE_SECTION_COUNT],
    /// Time (ms) of the last drum chip (BocuD `nLastChipTime`).
    pub last_chip_ms: i64,
    /// Total drum chips counted.
    pub total_drum_chips: u32,
}

impl Default for PhraseMeter {
    fn default() -> Self {
        Self {
            sections: [0u32; PHRASE_SECTION_COUNT],
            last_chip_ms: 0,
            total_drum_chips: 0,
        }
    }
}

impl PhraseMeter {
    /// Empty meter (no chart loaded).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build from a chart using its drum chips.
    pub fn from_chart(chart: &Chart, base_bpm: f32, bpm_changes: &[BpmChange]) -> Self {
        let mut sections = [0u32; PHRASE_SECTION_COUNT];
        let mut last_ms: i64 = 0;
        let mut total: u32 = 0;

        for chip in chart.drum_chips() {
            let t = chip_time_ms_with_bpm_changes(
                chip.measure,
                chip.value,
                base_bpm,
                bpm_changes,
            );
            if t < 0 {
                continue;
            }
            if t > last_ms {
                last_ms = t;
            }
            total += 1;
        }

        if last_ms > 0 {
            for chip in chart.drum_chips() {
                let t = chip_time_ms_with_bpm_changes(
                    chip.measure,
                    chip.value,
                    base_bpm,
                    bpm_changes,
                );
                if t < 0 {
                    continue;
                }
                let mut idx = (t as u128 * PHRASE_SECTION_COUNT as u128 / last_ms as u128)
                    as usize;
                if idx >= PHRASE_SECTION_COUNT {
                    idx = PHRASE_SECTION_COUNT - 1;
                }
                sections[idx] += 1;
            }
        }

        Self {
            sections,
            last_chip_ms: last_ms,
            total_drum_chips: total,
        }
    }

    /// Block width units (0..=BLOCKS_MAX) for section `i`.
    pub fn block_units(&self, i: usize) -> u32 {
        if i >= PHRASE_SECTION_COUNT {
            return 0;
        }
        let n = self.sections[i] as f64;
        let denom = DRUMS_CHIP_BASELINE / BLOCKS_MAX as f64 / PHRASE_SECTION_COUNT as f64;
        let units = (n / denom) as u32 + 1;
        units.min(BLOCKS_MAX)
    }

    /// Vertical slice top [0, 1] from top of the meter for section `i`.
    pub fn slice_top(&self, i: usize) -> f32 {
        if i >= PHRASE_SECTION_COUNT {
            return 0.0;
        }
        let n = PHRASE_SECTION_COUNT as f32;
        1.0 - ((i + 1) as f32) / n
    }

    /// Height of one slice as a fraction of total bar height.
    pub fn slice_height(&self) -> f32 {
        1.0 / PHRASE_SECTION_COUNT as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::{Chart, Chip, EChannel};

    #[test]
    fn empty_chart_zero_sections() {
        let p = PhraseMeter::from_chart(&Chart::default(), 120.0, &[]);
        assert_eq!(p.last_chip_ms, 0);
        assert_eq!(p.total_drum_chips, 0);
        assert_eq!(p.sections.iter().sum::<u32>(), 0);
    }

    #[test]
    fn single_chip_buckets_into_first_section() {
        let chart = Chart {
            chips: vec![Chip::new(0, EChannel::BassDrum, 0.5)],
            ..Default::default()
        };
        let p = PhraseMeter::from_chart(&chart, 120.0, &[]);
        assert_eq!(p.total_drum_chips, 1);
        assert_eq!(p.sections[0], 1);
        assert_eq!(p.last_chip_ms, 1000);
    }

    #[test]
    fn two_chips_at_same_time_share_section() {
        let chart = Chart {
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(0, EChannel::Snare, 0.0),
            ],
            ..Default::default()
        };
        let p = PhraseMeter::from_chart(&chart, 120.0, &[]);
        assert_eq!(p.total_drum_chips, 2);
        assert_eq!(p.sections[0], 2);
    }

    #[test]
    fn bpm_change_shifts_time_buckets() {
        let chart = Chart {
            chips: vec![
                Chip::new(1, EChannel::BassDrum, 0.0),
                Chip::new(2, EChannel::Snare, 0.0),
            ],
            ..Default::default()
        };
        let changes = vec![BpmChange { measure: 1, bpm: 240.0 }];
        let p = PhraseMeter::from_chart(&chart, 120.0, &changes);
        assert_eq!(p.total_drum_chips, 2);
        assert_eq!(p.sections.iter().sum::<u32>(), 2);
    }

    #[test]
    fn block_units_capped_at_max() {
        let chips: Vec<Chip> = (0..1600).map(|_| Chip::new(0, EChannel::BassDrum, 0.0)).collect();
        let chart = Chart { chips, ..Default::default() };
        let p = PhraseMeter::from_chart(&chart, 120.0, &[]);
        assert_eq!(p.block_units(0), BLOCKS_MAX);
    }

    #[test]
    fn slice_top_descends() {
        let p = PhraseMeter::empty();
        assert!(p.slice_top(0) > p.slice_top(1));
    }

    #[test]
    fn out_of_range_block_units_returns_zero() {
        let p = PhraseMeter::empty();
        assert_eq!(p.block_units(100), 0);
    }
}
