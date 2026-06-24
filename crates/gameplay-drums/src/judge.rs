//! Judge `LaneHit` events against chart chips.
//!
//! Algorithm:
//! 1. Find all un-judged chips in the hit lane whose `target_ms` is within
//!    ±200ms of `audio_ms` (max judgment window).
//! 2. Pick the closest to `audio_ms`.
//! 3. Classify the delta via `dtx_scoring::classify`.
//! 4. Emit `JudgmentEvent`, record chip as judged (so it won't be hit again).
//!
//! "Judged" state is tracked via [`JudgedChips`] resource.
//!
//! ## Phase 0 p0-5
//!
//! Uses `chip_time_ms_with_bpm_changes` so chips after a `#BPMxx` change
//! are timed against the new BPM, not the chart's base BPM. The
//! `BpmChangeList` resource is built from `EChannel::BPM` / `BPMEx` chips
//! when the chart is loaded.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::events::{JudgmentEvent, LaneHit};
use crate::lane_map::{lane_channel, lane_of, LANE_ORDER};
use crate::resources::ActiveChart;
use dtx_scoring::classify;
use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};

const MAX_JUDGE_WINDOW_MS: i64 = 200;

#[derive(Resource, Default, Debug)]
pub struct JudgedChips(pub HashSet<usize>);

/// Sorted list of BPM changes parsed from `#BPM` / `#BPMxx` chips.
#[derive(Resource, Default, Debug, Clone)]
pub struct BpmChangeList {
    pub changes: Vec<BpmChange>,
}

impl BpmChangeList {
    /// Build a BpmChangeList from a Chart by extracting all BPM/BPMEx chips.
    pub fn from_chart(chart: &dtx_core::Chart) -> Self {
        let mut changes: Vec<BpmChange> = chart
            .chips
            .iter()
            .filter(|c| {
                matches!(
                    c.channel,
                    dtx_core::EChannel::BPM | dtx_core::EChannel::BPMEx
                )
            })
            .map(|c| BpmChange {
                measure: c.measure,
                bpm: c.value,
            })
            .collect();
        changes.sort_by_key(|c| c.measure);
        Self { changes }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<JudgedChips>()
        .init_resource::<BpmChangeList>()
        .add_systems(Update, judge_lane_hit_system);
}

fn judge_lane_hit_system(
    mut lane_hits: MessageReader<LaneHit>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
) {
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);

    for hit in lane_hits.read() {
        let lane_channel = match lane_channel(hit.lane) {
            Some(c) => c,
            None => continue,
        };

        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart.chart.chips.iter().enumerate() {
            if chip.channel != lane_channel {
                continue;
            }
            if judged.0.contains(&idx) {
                continue;
            }
            let target_ms = chip_target_ms(chip, base_bpm, &bpm_changes.changes);
            let delta = hit.audio_ms - target_ms;
            if delta.abs() > MAX_JUDGE_WINDOW_MS {
                continue;
            }
            match best {
                Some((_, best_delta)) if best_delta.abs() <= delta.abs() => {}
                _ => best = Some((idx, delta)),
            }
        }

        if let Some((idx, delta)) = best {
            judged.0.insert(idx);
            events.write(JudgmentEvent {
                lane: hit.lane,
                kind: classify(delta as i32),
                delta_ms: delta,
            });
        }
    }
}

fn chip_target_ms(chip: &dtx_core::Chip, base_bpm: f32, bpm_changes: &[BpmChange]) -> i64 {
    chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, bpm_changes)
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::lane_map::lane_of;

    #[test]
    fn classifies_zero_delta_as_perfect() {
        assert_eq!(classify(0), dtx_scoring::JudgmentKind::Perfect);
        assert_eq!(classify(15), dtx_scoring::JudgmentKind::Perfect);
    }

    #[test]
    fn classifies_miss_outside_window() {
        assert_eq!(classify(500), dtx_scoring::JudgmentKind::Miss);
    }

    #[test]
    fn judge_selects_closest_chip_in_window() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.5));
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.75));

        let hit = LaneHit {
            lane: 2,
            audio_ms: 1000,
        };
        let mut judged = JudgedChips::default();
        let chart_r = ActiveChart::new(chart, None);
        let bpm_changes = BpmChangeList::from_chart(&chart_r.chart);
        let base_bpm = chart_r.chart.metadata.bpm.unwrap_or(120.0);
        let mut out: Vec<JudgmentEvent> = Vec::new();

        let lane_channel = lane_channel(hit.lane).unwrap();
        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart_r.chart.chips.iter().enumerate() {
            if chip.channel != lane_channel || judged.0.contains(&idx) {
                continue;
            }
            let target_ms = chip_target_ms(chip, base_bpm, &bpm_changes.changes);
            let delta = hit.audio_ms - target_ms;
            if delta.abs() > MAX_JUDGE_WINDOW_MS {
                continue;
            }
            match best {
                Some((_, d)) if d.abs() <= delta.abs() => {}
                _ => best = Some((idx, delta)),
            }
        }
        let (idx, delta) = best.unwrap();
        judged.0.insert(idx);
        out.push(JudgmentEvent {
            lane: hit.lane,
            kind: classify(delta as i32),
            delta_ms: delta,
        });

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, dtx_scoring::JudgmentKind::Perfect);
        assert_eq!(out[0].delta_ms, 0);
        assert!(judged.0.contains(&0));
        assert!(!judged.0.contains(&1));
    }

    #[test]
    fn empty_chart_produces_no_judgment() {
        let chart = ActiveChart::new(dtx_core::Chart::default(), None);
        let bpm_changes = BpmChangeList::from_chart(&chart.chart);
        let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
        let lane_channel = lane_channel(2).unwrap();
        let judged = JudgedChips::default();
        let hit = LaneHit {
            lane: 2,
            audio_ms: 1000,
        };
        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart.chart.chips.iter().enumerate() {
            if chip.channel != lane_channel || judged.0.contains(&idx) {
                continue;
            }
            let target_ms = chip_target_ms(chip, base_bpm, &bpm_changes.changes);
            let delta = hit.audio_ms - target_ms;
            if delta.abs() > MAX_JUDGE_WINDOW_MS {
                continue;
            }
            match best {
                Some((_, d)) if d.abs() <= delta.abs() => {}
                _ => best = Some((idx, delta)),
            }
        }
        assert!(best.is_none());
    }

    #[test]
    fn bpm_change_list_extracts_bpm_chips() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        // Insert some "chip" data on BPM channel.
        chart
            .chips
            .push(dtx_core::Chip::new(4, dtx_core::EChannel::BPM, 180.0));
        chart
            .chips
            .push(dtx_core::Chip::new(8, dtx_core::EChannel::BPM, 90.0));
        // And a drum chip that should be ignored.
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.0));

        let list = BpmChangeList::from_chart(&chart);
        assert_eq!(list.changes.len(), 2);
        assert_eq!(list.changes[0].measure, 4);
        assert!((list.changes[0].bpm - 180.0).abs() < 0.01);
        assert_eq!(list.changes[1].measure, 8);
    }

    #[test]
    fn bpm_change_list_handles_unsorted_input() {
        let mut chart = dtx_core::Chart::default();
        chart
            .chips
            .push(dtx_core::Chip::new(8, dtx_core::EChannel::BPM, 90.0));
        chart
            .chips
            .push(dtx_core::Chip::new(4, dtx_core::EChannel::BPM, 180.0));
        let list = BpmChangeList::from_chart(&chart);
        assert_eq!(list.changes[0].measure, 4);
        assert_eq!(list.changes[1].measure, 8);
    }

    #[test]
    fn bpm_change_list_empty_chart_is_empty() {
        let chart = dtx_core::Chart::default();
        let list = BpmChangeList::from_chart(&chart);
        assert!(list.changes.is_empty());
    }

    #[test]
    fn judge_with_bpm_change_uses_new_bpm() {
        // Chart: 120 BPM base, BPM change to 240 at measure 4.
        // Measure 8 at 240 BPM = 4 measures × 1000ms = 4000ms (after the change).
        // So target_ms for measure 8 = 4 measures × 2000ms (at 120) + 4 measures × 1000ms (at 240) = 12000ms.
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(8, dtx_core::EChannel::BassDrum, 0.0));
        chart
            .chips
            .push(dtx_core::Chip::new(4, dtx_core::EChannel::BPM, 240.0));

        let hit = LaneHit {
            lane: 2,
            audio_ms: 12000,
        };
        let bpm_changes = BpmChangeList::from_chart(&chart);
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let target_ms = chip_target_ms(&chart.chips[0], base_bpm, &bpm_changes.changes);
        let delta = hit.audio_ms - target_ms;
        assert_eq!(delta, 0); // perfect hit accounting for BPM change
    }

    #[test]
    fn lane_of_integration_with_chart() {
        assert_eq!(lane_of(dtx_core::EChannel::BassDrum), Some(2));
        assert_eq!(lane_of(dtx_core::EChannel::HiHatOpen), Some(7));
        assert_eq!(LANE_ORDER[0], dtx_core::EChannel::HiHatClose);
    }
}
