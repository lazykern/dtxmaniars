//! Wait mode: halt the clock at any unhit note until the correct pads
//! clear it. Spec: docs/superpowers/specs/2026-07-11-practice-wait-mode-design.md

use std::collections::HashSet;

use crate::lane_map::lane_of;
use crate::timeline::ChipTimeline;

/// The chips the halt is waiting on (a chord shares one target time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitSet {
    pub target_ms: i64,
    pub chips: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WaitPhase {
    #[default]
    Flowing,
    Halted(WaitSet),
}

/// Earliest pending (unjudged) drum note at/before `clock_ms` inside the
/// attempt span, plus every unjudged drum chip sharing its target time.
pub fn check_halt(
    timeline: &ChipTimeline,
    judged: &HashSet<usize>,
    clock_ms: i64,
    span_start_ms: i64,
) -> Option<WaitSet> {
    let pending = timeline.entries.iter().find(|e| {
        lane_of(e.channel).is_some()
            && e.judge_ms >= span_start_ms
            && e.judge_ms <= clock_ms
            && !judged.contains(&e.chip_idx)
    })?;
    let target_ms = pending.judge_ms;
    let chips: Vec<usize> = timeline
        .entries
        .iter()
        .filter(|e| {
            e.judge_ms == target_ms
                && lane_of(e.channel).is_some()
                && !judged.contains(&e.chip_idx)
        })
        .map(|e| e.chip_idx)
        .collect();
    Some(WaitSet { target_ms, chips })
}

/// True once every chip in the set has been judged (hit).
pub fn is_cleared(set: &WaitSet, judged: &HashSet<usize>) -> bool {
    set.chips.iter().all(|c| judged.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms. Chips: BD@0, SD@2000, HH@2000 (chord),
    // BGM@0 (non-drum), BD@4000.
    fn timeline() -> ChipTimeline {
        let mut assets = dtx_core::assets::DtxAssets::default();
        assets.wav.insert(1, "bgm.ogg".into());
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1), // 0: not a drum lane
                Chip::new(0, EChannel::BassDrum, 0.0),    // 1: 0ms
                Chip::new(1, EChannel::Snare, 0.0),       // 2: 2000ms
                Chip::new(1, EChannel::HiHatClose, 0.0),  // 3: 2000ms (chord)
                Chip::new(2, EChannel::BassDrum, 0.0),    // 4: 4000ms
            ],
            assets,
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 6_000)
    }

    #[test]
    fn no_halt_while_notes_still_ahead() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        assert_eq!(check_halt(&tl, &judged, 1_500, 0), None);
    }

    #[test]
    fn halts_at_earliest_unhit_note() {
        let tl = timeline();
        let judged = HashSet::new();
        let set = check_halt(&tl, &judged, 5, 0).unwrap();
        assert_eq!(set.target_ms, 0);
        assert_eq!(set.chips, vec![1]);
    }

    #[test]
    fn chord_collects_all_unjudged_chips_at_target() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        let set = check_halt(&tl, &judged, 2_003, 0).unwrap();
        assert_eq!(set.target_ms, 2_000);
        assert_eq!(set.chips.len(), 2);
        assert!(set.chips.contains(&2) && set.chips.contains(&3));
    }

    #[test]
    fn partially_cleared_chord_keeps_only_pending_chips() {
        let tl = timeline();
        let judged: HashSet<usize> = [1, 2].into();
        let set = check_halt(&tl, &judged, 2_003, 0).unwrap();
        assert_eq!(set.chips, vec![3]);
    }

    #[test]
    fn preroll_notes_never_halt() {
        let tl = timeline();
        let judged = HashSet::new();
        // Span starts at 2000; BD@0 is pre-roll and must not halt.
        let set = check_halt(&tl, &judged, 2_001, 2_000).unwrap();
        assert_eq!(set.target_ms, 2_000, "halt on span note, not pre-roll note");
    }

    #[test]
    fn non_drum_channels_ignored() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        // Only BGM@0 is "pending" before 1500 — no halt.
        assert_eq!(check_halt(&tl, &judged, 1_500, 0), None);
    }

    #[test]
    fn is_cleared_requires_every_chip() {
        let set = WaitSet {
            target_ms: 2_000,
            chips: vec![2, 3],
        };
        assert!(!is_cleared(&set, &[2].into()));
        assert!(is_cleared(&set, &[2, 3].into()));
    }
}
