//! Per-lane diagnosis: accuracy + signed timing bias per drum lane,
//! accumulated over all attempts on the current loop region.
//! Spec: docs/superpowers/specs/2026-07-11-practice-lane-diagnosis-design.md

use std::collections::HashMap;

use dtx_scoring::JudgmentKind;

use crate::lane_map::{lane_channel, LaneId};

/// Bias smaller than this (ms, absolute) reads as "on time".
pub const BIAS_THRESHOLD_MS: f32 = 10.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LaneAgg {
    /// Judgments counted (pre-roll chips excluded by the caller).
    pub judged: u32,
    pub misses: u32,
    /// EmptyHit (whiff) on this lane.
    pub overhits: u32,
    /// Signed delta sum, hits only (miss delta excluded).
    pub delta_sum_ms: i64,
    pub delta_count: u32,
}

impl LaneAgg {
    pub fn hit_pct(&self) -> f32 {
        if self.judged == 0 {
            0.0
        } else {
            (self.judged - self.misses) as f32 / self.judged as f32 * 100.0
        }
    }

    pub fn mean_delta_ms(&self) -> f32 {
        if self.delta_count == 0 {
            0.0
        } else {
            self.delta_sum_ms as f32 / self.delta_count as f32
        }
    }

    /// "−18ms rushing" / "+14ms dragging" / "on time".
    pub fn bias_label(&self) -> String {
        let mean = self.mean_delta_ms();
        if self.delta_count == 0 {
            "—".into()
        } else if mean < -BIAS_THRESHOLD_MS {
            format!("{mean:+.0}ms rushing")
        } else if mean > BIAS_THRESHOLD_MS {
            format!("{mean:+.0}ms dragging")
        } else {
            "on time".into()
        }
    }
}

/// Per-lane aggregates for the current loop region.
#[derive(Debug, Clone, Default)]
pub struct LaneDiagnosis {
    pub lanes: HashMap<LaneId, LaneAgg>,
}

impl LaneDiagnosis {
    pub fn clear(&mut self) {
        self.lanes.clear();
    }

    pub fn apply_judgment(&mut self, lane: LaneId, kind: JudgmentKind, delta_ms: i64) {
        let agg = self.lanes.entry(lane).or_default();
        agg.judged += 1;
        if kind == JudgmentKind::Miss {
            agg.misses += 1;
        } else {
            agg.delta_sum_ms += delta_ms;
            agg.delta_count += 1;
        }
    }

    pub fn apply_miss(&mut self, lane: LaneId) {
        let agg = self.lanes.entry(lane).or_default();
        agg.judged += 1;
        agg.misses += 1;
    }

    pub fn apply_overhit(&mut self, lane: LaneId) {
        self.lanes.entry(lane).or_default().overhits += 1;
    }

    /// Rows sorted worst-first: lowest hit%, ties by larger |mean delta|.
    pub fn sorted_rows(&self) -> Vec<(LaneId, LaneAgg)> {
        let mut rows: Vec<(LaneId, LaneAgg)> =
            self.lanes.iter().map(|(&l, &a)| (l, a)).collect();
        rows.sort_by(|a, b| {
            a.1.hit_pct()
                .partial_cmp(&b.1.hit_pct())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    b.1.mean_delta_ms()
                        .abs()
                        .partial_cmp(&a.1.mean_delta_ms().abs())
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        rows
    }
}

/// Panel text: one row per lane with data, worst-first.
/// `HH   82%  −18ms rushing   3 miss  1 over`
pub fn diagnosis_text(diag: &LaneDiagnosis) -> String {
    let rows = diag.sorted_rows();
    if rows.is_empty() {
        return "LANES\n(no data yet)".into();
    }
    let mut out = String::from("LANES");
    for (lane, agg) in rows {
        let name = lane_channel(lane)
            .and_then(dtx_layout::channel_short_name)
            .unwrap_or("?");
        out.push_str(&format!(
            "\n{name:<4} {:>3.0}%  {}  {} miss  {} over",
            agg.hit_pct(),
            agg.bias_label(),
            agg.misses,
            agg.overhits
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgments_accumulate_per_lane() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(0, JudgmentKind::Perfect, -20);
        d.apply_judgment(0, JudgmentKind::Great, -16);
        d.apply_judgment(1, JudgmentKind::Perfect, 2);
        let hh = d.lanes[&0];
        assert_eq!(hh.judged, 2);
        assert_eq!(hh.mean_delta_ms(), -18.0);
        assert_eq!(hh.bias_label(), "-18ms rushing");
        assert_eq!(d.lanes[&1].bias_label(), "on time");
    }

    #[test]
    fn miss_counts_without_polluting_bias() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(2, JudgmentKind::Perfect, 4);
        d.apply_miss(2);
        let bd = d.lanes[&2];
        assert_eq!(bd.judged, 2);
        assert_eq!(bd.misses, 1);
        assert_eq!(bd.delta_count, 1, "miss must not enter mean delta");
        assert_eq!(bd.hit_pct(), 50.0);
    }

    #[test]
    fn overhit_tracked_without_touching_judged() {
        let mut d = LaneDiagnosis::default();
        d.apply_overhit(1);
        assert_eq!(d.lanes[&1].overhits, 1);
        assert_eq!(d.lanes[&1].judged, 0);
    }

    #[test]
    fn dragging_label_positive() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(3, JudgmentKind::Good, 14);
        assert_eq!(d.lanes[&3].bias_label(), "+14ms dragging");
    }

    #[test]
    fn sorted_rows_worst_first() {
        let mut d = LaneDiagnosis::default();
        // lane 0: 100%, lane 1: 50%, lane 2: 0%.
        d.apply_judgment(0, JudgmentKind::Perfect, 0);
        d.apply_judgment(1, JudgmentKind::Perfect, 0);
        d.apply_miss(1);
        d.apply_miss(2);
        let order: Vec<u8> = d.sorted_rows().iter().map(|(l, _)| *l).collect();
        assert_eq!(order, vec![2, 1, 0]);
    }

    #[test]
    fn clear_empties_everything() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(0, JudgmentKind::Perfect, 0);
        d.clear();
        assert!(d.lanes.is_empty());
        assert!(diagnosis_text(&d).contains("no data"));
    }
}
