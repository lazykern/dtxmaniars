//! Per-chart derived data computed once on Performance enter.
//!
//! Holds phrase meter + skill computation inputs so the `on_enter_performance`
//! system stays under Bevy's 16-arg system limit.

use bevy::prelude::Resource;
use dtx_core::Chart;

use crate::judge::BpmChangeList;
use crate::phrase::PhraseMeter;

/// All chart-derived data needed by the Performance HUD.
///
/// Resource initialized in `on_enter_performance` from `ActiveChart`.
#[derive(Resource, Debug, Clone, Default)]
pub struct ChartDerived {
    pub phrase: PhraseMeter,
    /// Chart difficulty level (e.g., 8.20 for MASTER 82).
    pub chart_level: f64,
    /// Maximum theoretical skill at full combo + all P.
    pub max_skill: f64,
    /// Total drum chip count (cached for % math).
    pub total_drum_chips: u32,
}

/// Populate `ChartDerived` from a parsed chart + BPM list.
///
/// Called from `on_enter_performance` after the BPM list is built.
pub fn compute_from_chart(
    derived: &mut ChartDerived,
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    drum_chip_count: u32,
) {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    derived.phrase = PhraseMeter::from_chart(chart, base_bpm, &bpm_changes.changes);
    derived.chart_level = chart
        .metadata
        .dlevel
        .map(|v| v as f64 / 10.0)
        .unwrap_or(0.0);
    derived.total_drum_chips = drum_chip_count;
    derived.max_skill = crate::skill::game_skill(100.0, derived.chart_level, false);
}
