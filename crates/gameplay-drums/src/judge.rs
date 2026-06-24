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

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::prelude::{MessageReader as _, MessageWriter as _, Resource as _};

use crate::events::{JudgmentEvent, LaneHit};
use crate::lane_map::{lane_channel, lane_of, LaneId, LANE_ORDER};
use crate::resources::ActiveChart;
use dtx_scoring::classify;

const MAX_JUDGE_WINDOW_MS: i64 = 200;

#[derive(Resource, Default, Debug)]
pub struct JudgedChips(pub HashSet<usize>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<JudgedChips>()
        .add_systems(Update, judge_lane_hit_system);
}

fn judge_lane_hit_system(
    mut lane_hits: MessageReader<LaneHit>,
    chart: Res<ActiveChart>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
) {
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
            let target_ms = chip_target_ms(chip, &chart.chart);
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

fn chip_target_ms(chip: &dtx_core::Chip, chart: &dtx_core::Chart) -> i64 {
    dtx_timing::math::chip_time_ms(
        chip.measure,
        chip.value,
        chart.metadata.bpm.unwrap_or(120.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut out: Vec<JudgmentEvent> = Vec::new();

        let lane_channel = lane_channel(hit.lane).unwrap();
        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart_r.chart.chips.iter().enumerate() {
            if chip.channel != lane_channel || judged.0.contains(&idx) {
                continue;
            }
            let target_ms = chip_target_ms(chip, &chart_r.chart);
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
        let lane_channel = lane_channel(2).unwrap();
        let mut judged = JudgedChips::default();
        let hit = LaneHit {
            lane: 2,
            audio_ms: 1000,
        };
        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart.chart.chips.iter().enumerate() {
            if chip.channel != lane_channel || judged.0.contains(&idx) {
                continue;
            }
            let target_ms = chip_target_ms(chip, &chart.chart);
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
    fn lane_of_integration_with_chart() {
        assert_eq!(lane_of(dtx_core::EChannel::BassDrum), Some(2));
        assert_eq!(lane_of(dtx_core::EChannel::HiHatOpen), Some(7));
        assert_eq!(LANE_ORDER[0], dtx_core::EChannel::HiHatClose);
    }
}
