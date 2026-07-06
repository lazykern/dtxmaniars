//! Precomputed chart timeline for seek/practice: per-chip times in both
//! timebases, timing-line times, snap points, BGM chip list, density.

use bevy::prelude::*;
use dtx_core::beat_lines::TimingLineKind;
use dtx_core::{Chart, EChannel};
use dtx_timing::math::ChartTiming;

use crate::judge::{auto_chip_target_ms, chip_target_ms, BarLengthChangeList, BpmChangeList};
use crate::lane_map::lane_of;

pub const DENSITY_BUCKETS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineEntry {
    pub chip_idx: usize,
    pub channel: EChannel,
    /// Judgement timebase (`chip_target_ms`, no BGM adjust).
    pub judge_ms: i64,
    /// Auto-scheduler timebase (`auto_chip_target_ms`, BGM adjust applied).
    pub auto_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapDivisor {
    #[default]
    Bar,
    Beat,
    Quarter,
}

impl SnapDivisor {
    pub fn label(self) -> &'static str {
        match self {
            SnapDivisor::Bar => "Bar",
            SnapDivisor::Beat => "Beat",
            SnapDivisor::Quarter => "1/2 beat",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SnapDivisor::Bar => SnapDivisor::Beat,
            SnapDivisor::Beat => SnapDivisor::Quarter,
            SnapDivisor::Quarter => SnapDivisor::Bar,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ChipTimeline {
    /// One entry per chart chip, sorted by `judge_ms`.
    pub entries: Vec<TimelineEntry>,
    /// `judge_ms` indexed by chip index (unsorted chart order).
    pub judge_ms_by_idx: Vec<i64>,
    /// Times of `dtx_core::expand_timing_lines` output, parallel to
    /// `TimingLineList.lines` (same expansion, same order).
    pub timing_line_ms: Vec<i64>,
    /// Bar-line times, sorted ascending.
    pub bar_ms: Vec<i64>,
    /// Bar + beat line times merged, sorted ascending.
    pub beat_ms: Vec<i64>,
    /// BGM chips with audio: `(chip_idx, auto_ms)`, sorted by `auto_ms`.
    pub bgm_chips: Vec<(usize, i64)>,
    /// Chart end incl. tail (mirrors `DrumsStageCompletion::chart_end_ms`).
    pub end_ms: i64,
    /// Normalized drum-chip density over `[0, end_ms]`.
    pub density: [f32; DENSITY_BUCKETS],
}

impl Default for ChipTimeline {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            judge_ms_by_idx: Vec::new(),
            timing_line_ms: Vec::new(),
            bar_ms: Vec::new(),
            beat_ms: Vec::new(),
            bgm_chips: Vec::new(),
            end_ms: 0,
            density: [0.0; DENSITY_BUCKETS],
        }
    }
}

impl ChipTimeline {
    pub fn from_chart(
        chart: &Chart,
        bpm_changes: &BpmChangeList,
        bar_changes: &BarLengthChangeList,
        bgm_adjust_ms: i32,
        end_ms: i64,
    ) -> Self {
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let timing = ChartTiming {
            bpm_changes: &bpm_changes.changes,
            bar_changes: &bar_changes.changes,
        };

        let mut entries: Vec<TimelineEntry> = chart
            .chips
            .iter()
            .enumerate()
            .map(|(idx, chip)| TimelineEntry {
                chip_idx: idx,
                channel: chip.channel,
                judge_ms: chip_target_ms(chip, base_bpm, timing),
                auto_ms: auto_chip_target_ms(chip, base_bpm, timing, bgm_adjust_ms),
            })
            .collect();
        let mut judge_ms_by_idx = vec![0i64; chart.chips.len()];
        for e in &entries {
            judge_ms_by_idx[e.chip_idx] = e.judge_ms;
        }
        entries.sort_by_key(|e| e.judge_ms);

        let lines = dtx_core::expand_timing_lines(chart);
        let mut timing_line_ms = Vec::with_capacity(lines.len());
        let mut bar_ms = Vec::new();
        let mut beat_only_ms = Vec::new();
        for line in &lines {
            let (measure, fraction) = dtx_core::beat_lines::tick_to_measure_fraction(line.tick);
            let ms = dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(
                measure, fraction, base_bpm, timing,
            );
            timing_line_ms.push(ms);
            match line.kind {
                TimingLineKind::Bar => bar_ms.push(ms),
                TimingLineKind::Beat => beat_only_ms.push(ms),
            }
        }
        bar_ms.sort_unstable();
        bar_ms.dedup();
        let mut beat_ms: Vec<i64> = bar_ms.iter().copied().chain(beat_only_ms).collect();
        beat_ms.sort_unstable();
        beat_ms.dedup();

        let mut bgm_chips: Vec<(usize, i64)> = entries
            .iter()
            .filter(|e| e.channel == EChannel::BGM && chart.chips[e.chip_idx].wav_slot != 0)
            .map(|e| (e.chip_idx, e.auto_ms))
            .collect();
        bgm_chips.sort_by_key(|&(_, ms)| ms);

        let mut density = [0.0_f32; DENSITY_BUCKETS];
        if end_ms > 0 {
            for e in &entries {
                if lane_of(e.channel).is_none() {
                    continue;
                }
                let slot =
                    ((e.judge_ms.max(0) as f64 / end_ms as f64) * DENSITY_BUCKETS as f64) as usize;
                density[slot.min(DENSITY_BUCKETS - 1)] += 1.0;
            }
            let max = density.iter().cloned().fold(0.0_f32, f32::max);
            if max > 0.0 {
                for d in &mut density {
                    *d /= max;
                }
            }
        }

        Self {
            entries,
            judge_ms_by_idx,
            timing_line_ms,
            bar_ms,
            beat_ms,
            bgm_chips,
            end_ms,
            density,
        }
    }

    fn snap_points(&self, snap: SnapDivisor) -> Vec<i64> {
        match snap {
            SnapDivisor::Bar => self.bar_ms.clone(),
            SnapDivisor::Beat => self.beat_ms.clone(),
            SnapDivisor::Quarter => {
                let mut pts = self.beat_ms.clone();
                for w in self.beat_ms.windows(2) {
                    pts.push(w[0] + (w[1] - w[0]) / 2);
                }
                pts.sort_unstable();
                pts.dedup();
                pts
            }
        }
    }

    /// Floor `target_ms` to the nearest snap point at or before it,
    /// clamped into `[first point, end_ms]`.
    pub fn resolve_snap(&self, target_ms: i64, snap: SnapDivisor) -> i64 {
        let pts = self.snap_points(snap);
        if pts.is_empty() {
            return target_ms.clamp(0, self.end_ms.max(0));
        }
        let clamped = target_ms.clamp(pts[0], self.end_ms.max(pts[0]));
        match pts.binary_search(&clamped) {
            Ok(i) => pts[i],
            Err(0) => pts[0],
            Err(i) => pts[i - 1],
        }
    }

    /// Next (`dir > 0`) or previous (`dir < 0`) snap point from `ms`.
    /// Saturates at the ends.
    pub fn snap_neighbor(&self, ms: i64, snap: SnapDivisor, dir: i8) -> i64 {
        let pts = self.snap_points(snap);
        if pts.is_empty() {
            return ms;
        }
        let cur = self.resolve_snap(ms, snap);
        let i = pts
            .binary_search(&cur)
            .unwrap_or_else(|e| e.min(pts.len() - 1));
        let j = if dir > 0 {
            (i + 1).min(pts.len() - 1)
        } else {
            i.saturating_sub(1)
        };
        pts[j]
    }

    /// Start of the bar at or before `ms` (pre-roll anchor).
    pub fn bar_start_before(&self, ms: i64) -> i64 {
        self.resolve_snap(ms, SnapDivisor::Bar)
    }

    /// The BGM chip whose stream should be playing at chart time
    /// `target_ms`: the last chip with `auto_ms <= target_ms`.
    pub fn governing_bgm_chip(&self, target_ms: i64) -> Option<(usize, i64)> {
        self.bgm_chips
            .iter()
            .take_while(|&&(_, ms)| ms <= target_ms)
            .last()
            .copied()
    }
}

/// Build the timeline on Performance enter. Ordered after the drums
/// enter chain so BPM/bar lists and `chart_end_ms` are already derived.
pub fn build_chip_timeline(
    chart: Res<crate::resources::ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<crate::resources::BgmAdjustState>,
    completion: Res<crate::orchestrator::DrumsStageCompletion>,
    mut timeline: ResMut<ChipTimeline>,
) {
    *timeline = ChipTimeline::from_chart(
        &chart.chart,
        &bpm_changes,
        &bar_changes,
        bgm_adjust.total_ms(),
        completion.chart_end_ms,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::assets::DtxAssets;
    use dtx_core::chart::{Chip, Metadata};

    // 120 BPM, 4/4: one measure = 2000ms, one beat = 500ms.
    fn test_chart() -> Chart {
        let mut assets = DtxAssets::default();
        assets.wav.insert(1, "bgm_a.ogg".into());
        assets.wav.insert(2, "bgm_b.ogg".into());
        assets.wav.insert(3, "snare.ogg".into());
        Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1), // slice A @ 0ms
                Chip::with_wav(4, EChannel::BGM, 0.0, 2), // slice B @ 8000ms
                Chip::new(0, EChannel::BassDrum, 0.0),    // 0ms
                Chip::new(1, EChannel::Snare, 0.5),       // 3000ms
                Chip::new(6, EChannel::BassDrum, 0.0),    // 12000ms
            ],
            assets,
            ..Default::default()
        }
    }

    fn build(chart: &Chart) -> ChipTimeline {
        let bpm = BpmChangeList::from_chart(chart);
        let bar = BarLengthChangeList::from_chart(chart);
        ChipTimeline::from_chart(chart, &bpm, &bar, 0, 14_000)
    }

    #[test]
    fn entries_sorted_and_indexable() {
        let chart = test_chart();
        let tl = build(&chart);
        assert_eq!(tl.entries.len(), chart.chips.len());
        assert!(tl
            .entries
            .windows(2)
            .all(|w| w[0].judge_ms <= w[1].judge_ms));
        // idx→time lookup covers every chip.
        assert_eq!(tl.judge_ms_by_idx.len(), chart.chips.len());
        assert_eq!(tl.judge_ms_by_idx[3], 3000);
    }

    #[test]
    fn bar_snap_floors_to_bar_start() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(3_100, SnapDivisor::Bar), 2_000);
        assert_eq!(tl.resolve_snap(1_999, SnapDivisor::Bar), 0);
    }

    #[test]
    fn beat_snap_floors_to_beat() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(3_100, SnapDivisor::Beat), 3_000);
        assert_eq!(tl.resolve_snap(2_600, SnapDivisor::Beat), 2_500);
    }

    #[test]
    fn snap_clamps_into_chart_range() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(-500, SnapDivisor::Bar), 0);
        assert!(tl.resolve_snap(99_999, SnapDivisor::Bar) <= tl.end_ms);
    }

    #[test]
    fn governing_bgm_chip_picks_last_at_or_before() {
        let tl = build(&test_chart());
        assert_eq!(tl.governing_bgm_chip(0), Some((0, 0)));
        assert_eq!(tl.governing_bgm_chip(7_999), Some((0, 0)));
        assert_eq!(tl.governing_bgm_chip(8_000), Some((1, 8_000)));
        assert_eq!(tl.governing_bgm_chip(12_000), Some((1, 8_000)));
    }

    #[test]
    fn governing_bgm_chip_none_before_first() {
        let mut chart = test_chart();
        // Move both BGM chips later than 0.
        chart.chips[0] = dtx_core::Chip::with_wav(2, EChannel::BGM, 0.0, 1);
        let tl = build(&chart);
        assert_eq!(tl.governing_bgm_chip(1_000), None);
    }

    #[test]
    fn snap_neighbor_steps_between_points() {
        let tl = build(&test_chart());
        assert_eq!(tl.snap_neighbor(2_000, SnapDivisor::Bar, 1), 4_000);
        assert_eq!(tl.snap_neighbor(2_000, SnapDivisor::Bar, -1), 0);
        assert_eq!(tl.snap_neighbor(0, SnapDivisor::Bar, -1), 0);
    }

    #[test]
    fn density_counts_only_drum_lanes() {
        let tl = build(&test_chart());
        let total: f32 = tl.density.iter().sum();
        assert!(total > 0.0);
        let max = tl.density.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            (max - 1.0).abs() < 1e-6,
            "density must be normalized to 1.0"
        );
    }
}
