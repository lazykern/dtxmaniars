//! Judge `LaneHit` events against chart chips with pad grouping.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::drum_groups::{resolve_explicit_lanes, resolve_judgments, DrumPad};
use crate::events::{EmptyHit, InputHit, JudgmentEvent, LaneHit};
use crate::resources::{ActiveChart, DrumGameplaySettings, GameplayClock};
use dtx_scoring::classify;
use dtx_timing::math::{
    chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, BpmChange, ChartTiming,
};

#[derive(Resource, Default, Debug)]
pub struct JudgedChips(pub HashSet<usize>);

/// Sorted list of BPM changes parsed from `#BPM` / `#BPMxx` chips.
#[derive(Resource, Default, Debug, Clone)]
pub struct BpmChangeList {
    pub changes: Vec<BpmChange>,
}

impl BpmChangeList {
    pub fn from_chart(chart: &dtx_core::Chart) -> Self {
        Self {
            changes: dtx_core::timing::bpm_changes_from_chart(chart),
        }
    }
}

/// Sorted list of bar-length (meter change) events parsed from `#02` chips.
#[derive(Resource, Default, Debug, Clone)]
pub struct BarLengthChangeList {
    pub changes: Vec<BarLengthChange>,
}

impl BarLengthChangeList {
    pub fn from_chart(chart: &dtx_core::Chart) -> Self {
        Self {
            changes: dtx_core::timing::bar_changes_from_chart(chart),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<JudgedChips>()
        .init_resource::<BpmChangeList>()
        .init_resource::<BarLengthChangeList>()
        .add_systems(
            FixedUpdate,
            judge_lane_hit_system
                .in_set(super::DrumsSets::Judge)
                .run_if(in_state(game_shell::AppState::Performance))
                // No judgment/scoring while the Customize surface is open; hits
                // still flash + play a voice via `hit_feedback` (feedback only).
                .run_if(crate::editor::editor_closed),
        );
}

pub(crate) fn judge_lane_hit_system(
    mut lane_hits: MessageReader<LaneHit>,
    mut input_hits: MessageReader<InputHit>,
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    drum_settings: Res<DrumGameplaySettings>,
    input_offset: Res<crate::resources::InputOffsetMs>,
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
    mut chord_hits: Option<ResMut<crate::practice::wait::ChordHitTimes>>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
    mut empty_hits: MessageWriter<EmptyHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    // While practice wait mode is halted, only the halted chord's chips
    // are judgeable — otherwise a repeated hit on an already-satisfied
    // lane "borrows" credit from a future note on that same lane.
    let halted_chips: Option<&[usize]> = wait_state.as_deref().and_then(|s| match &s.phase {
        crate::practice::wait::WaitPhase::Halted(set) => Some(set.chips.as_slice()),
        crate::practice::wait::WaitPhase::Flowing => None,
    });

    for hit in lane_hits.read() {
        let Some(pad) = DrumPad::from_lane(hit.lane) else {
            continue;
        };

        // Shift the measured hit time by the configured input offset before
        // matching chips (DTXManiaNX nInputAdjustTimeMs).
        let adjusted_hit_ms = hit.audio_ms - input_offset.0 as i64;
        let results = resolve_judgments(
            pad,
            adjusted_hit_ms,
            &chart.chart,
            &judged.0,
            base_bpm,
            timing,
            &drum_settings.groups,
        );
        let results = filter_to_halted_set(results, halted_chips);
        if let Some(chord_hits) = chord_hits.as_deref_mut() {
            record_chord_hit_times(chord_hits, &results, halted_chips, adjusted_hit_ms);
        }

        if results.is_empty() {
            empty_hits.write(EmptyHit {
                lane: hit.lane,
                audio_ms: hit.audio_ms,
            });
            continue;
        }

        for (idx, delta) in results {
            judged.0.insert(idx);
            events.write(JudgmentEvent {
                lane: hit.lane,
                kind: classify(delta as i32),
                delta_ms: delta,
                chip_idx: idx,
            });
        }
    }

    for hit in input_hits.read() {
        let Some(&primary_lane) = hit.lanes.first() else {
            continue;
        };
        let adjusted_hit_ms = hit.audio_ms - input_offset.0 as i64;
        let result = resolve_explicit_lanes(
            &hit.lanes,
            adjusted_hit_ms,
            &chart.chart,
            &judged.0,
            base_bpm,
            timing,
            halted_chips,
        );
        let Some((idx, delta)) = result else {
            empty_hits.write(EmptyHit {
                lane: primary_lane,
                audio_ms: hit.audio_ms,
            });
            continue;
        };
        if let Some(chord_hits) = chord_hits.as_deref_mut() {
            record_chord_hit_times(chord_hits, &[(idx, delta)], halted_chips, adjusted_hit_ms);
        }
        judged.0.insert(idx);
        events.write(JudgmentEvent {
            lane: crate::lane_map::lane_of(chart.chart.chips[idx].channel).unwrap_or(primary_lane),
            kind: classify(delta as i32),
            delta_ms: delta,
            chip_idx: idx,
        });
    }
}

/// While practice wait mode is halted on a chord, drop any judged chip
/// that isn't part of that chord — a repeated hit on an already-satisfied
/// lane must not "borrow" credit from a future note on the same lane.
fn filter_to_halted_set(
    results: Vec<(usize, i64)>,
    halted_chips: Option<&[usize]>,
) -> Vec<(usize, i64)> {
    match halted_chips {
        Some(allowed) => results
            .into_iter()
            .filter(|(idx, _)| allowed.contains(idx))
            .collect(),
        None => results,
    }
}

/// Record `hit_ms` for every `(chip_idx, _delta)` in `results` into
/// `chord_hits`, but only while wait mode is halted (`halted_chips` is
/// `Some`) — outside a halt there is no chord being evaluated, so there
/// is nothing to record against.
fn record_chord_hit_times(
    chord_hits: &mut crate::practice::wait::ChordHitTimes,
    results: &[(usize, i64)],
    halted_chips: Option<&[usize]>,
    hit_ms: i64,
) {
    if halted_chips.is_none() {
        return;
    }
    for (idx, _delta) in results {
        chord_hits.0.insert(*idx, hit_ms);
    }
}

pub fn chip_target_ms(chip: &dtx_core::Chip, base_bpm: f32, timing: ChartTiming<'_>) -> i64 {
    chip_time_ms_with_bpm_and_bar_changes(chip.measure, chip.value, base_bpm, timing)
}

/// Chip target with optional play-speed scaling (`nPlaySpeed / 20.0`).
/// Speed = 1.0 is a no-op; >1.0 makes the chart finish earlier.
pub fn chip_target_ms_with_speed(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    timing: ChartTiming<'_>,
    play_speed: f32,
) -> i64 {
    if play_speed <= 0.0 || (play_speed - 1.0).abs() < f32::EPSILON {
        return chip_target_ms(chip, base_bpm, timing);
    }
    ((chip_target_ms(chip, base_bpm, timing) as f64) / (play_speed as f64)) as i64
}

/// Chart time for auto-play chips (BGM/SE) including BGM adjust offset.
/// `play_speed` is applied to the chip time before adding the BGM offset,
/// matching BocuD semantics (chip time scales, BGMAdjust stays absolute).
pub fn auto_chip_target_ms(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    timing: ChartTiming<'_>,
    bgm_adjust_ms: i32,
) -> i64 {
    chip_target_ms(chip, base_bpm, timing) + i64::from(bgm_adjust_ms)
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::drum_groups::{ChartChipPresence, DrumPad, EffectiveGroups, MAX_JUDGE_WINDOW_MS};
    use crate::lane_map::{lane_of, LANE_ORDER};
    use dtx_config::{CyGroup, DrumsConfig};

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
        let judged = JudgedChips::default();
        let groups =
            EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default());
        let results = resolve_judgments(
            DrumPad::Bd,
            hit.audio_ms,
            &chart,
            &judged.0,
            120.0,
            ChartTiming::default(),
            &groups,
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn judge_prefers_smallest_delta_over_earlier_chip() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.50));
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.56));

        let groups =
            EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default());
        let results = resolve_judgments(
            DrumPad::Bd,
            1100,
            &chart,
            &HashSet::new(),
            120.0,
            ChartTiming::default(),
            &groups,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn judge_rejects_hits_outside_nx_poor_window() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.50));

        let groups =
            EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default());
        let results = resolve_judgments(
            DrumPad::Bd,
            1118,
            &chart,
            &HashSet::new(),
            120.0,
            ChartTiming::default(),
            &groups,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn empty_chart_produces_no_judgment() {
        let chart = dtx_core::Chart::default();
        let groups =
            EffectiveGroups::from_config(&DrumsConfig::default(), &ChartChipPresence::default());
        let results = resolve_judgments(
            DrumPad::Bd,
            1000,
            &chart,
            &HashSet::new(),
            120.0,
            ChartTiming::default(),
            &groups,
        );
        assert!(results.is_empty());
    }

    #[test]
    fn bpm_change_list_extracts_bpm_chips() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(4, dtx_core::EChannel::BPM, 180.0));
        chart
            .chips
            .push(dtx_core::Chip::new(8, dtx_core::EChannel::BPM, 90.0));
        chart
            .chips
            .push(dtx_core::Chip::new(0, dtx_core::EChannel::BassDrum, 0.0));

        let list = BpmChangeList::from_chart(&chart);
        assert_eq!(list.changes.len(), 2);
        assert_eq!(list.changes[0].measure, 4);
    }

    #[test]
    fn judge_with_bpm_change_uses_new_bpm() {
        let mut chart = dtx_core::Chart::default();
        chart.metadata.bpm = Some(120.0);
        chart
            .chips
            .push(dtx_core::Chip::new(8, dtx_core::EChannel::BassDrum, 0.0));
        chart
            .chips
            .push(dtx_core::Chip::new(4, dtx_core::EChannel::BPM, 240.0));

        let bpm_changes = BpmChangeList::from_chart(&chart);
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let timing = ChartTiming {
            bpm_changes: &bpm_changes.changes,
            bar_changes: &[],
        };
        let target_ms = chip_target_ms(&chart.chips[0], base_bpm, timing);
        assert_eq!(12000 - target_ms, 0);
    }

    #[test]
    fn lane_of_integration_with_chart() {
        assert_eq!(lane_of(dtx_core::EChannel::BassDrum), Some(2));
        assert_eq!(lane_of(dtx_core::EChannel::LeftCymbal), Some(9));
        assert_eq!(LANE_ORDER.len(), 12);
    }

    #[test]
    fn max_judge_window_matches_nx_poor_window() {
        assert_eq!(MAX_JUDGE_WINDOW_MS, 117);
    }

    #[test]
    fn halted_filter_passes_through_when_not_halted() {
        let results = vec![(5, 10)];
        assert_eq!(filter_to_halted_set(results.clone(), None), results);
    }

    #[test]
    fn halted_filter_keeps_only_chord_chips() {
        let results = vec![(2, 0), (5, 3)];
        // chip 2 is part of the halted chord, chip 5 (a future note on the
        // same lane) must not be credited while still halted.
        assert_eq!(filter_to_halted_set(results, Some(&[2, 3])), vec![(2, 0)]);
    }

    #[test]
    fn halted_filter_drops_future_note_entirely() {
        let results = vec![(5, 3)];
        assert_eq!(filter_to_halted_set(results, Some(&[2, 3])), Vec::new());
    }

    #[test]
    fn records_hit_time_only_for_halted_chips() {
        use crate::practice::wait::ChordHitTimes;
        let mut chord_hits = ChordHitTimes::default();
        record_chord_hit_times(
            &mut chord_hits,
            &[(2, 0_i64), (5, 3)],
            Some(&[2, 3]),
            12_345,
        );
        assert_eq!(chord_hits.0.get(&2), Some(&12_345));
        assert_eq!(chord_hits.0.get(&5), Some(&12_345));

        let mut chord_hits2 = ChordHitTimes::default();
        record_chord_hit_times(&mut chord_hits2, &[(2, 0_i64)], None, 12_345);
        assert!(chord_hits2.0.is_empty());
    }

    #[test]
    fn records_explicit_lane_hit_time_while_halted() {
        use crate::practice::wait::ChordHitTimes;

        let mut chord_hits = ChordHitTimes::default();
        record_chord_hit_times(&mut chord_hits, &[(3, 0_i64)], Some(&[2, 3]), 12_345);

        assert_eq!(chord_hits.0.get(&3), Some(&12_345));
    }
}
