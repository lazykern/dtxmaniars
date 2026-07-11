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
            e.judge_ms == target_ms && lane_of(e.channel).is_some() && !judged.contains(&e.chip_idx)
        })
        .map(|e| e.chip_idx)
        .collect();
    Some(WaitSet { target_ms, chips })
}

/// True once every chip in the set has been judged (hit).
pub fn is_cleared(set: &WaitSet, judged: &HashSet<usize>) -> bool {
    set.chips.iter().all(|c| judged.contains(c))
}

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use game_shell::{AppState, PauseState};

use super::session::PracticeSession;
use crate::judge::JudgedChips;
use crate::resources::{ActiveDrumSounds, GameplayClock};
use crate::seek::SeekToChartTime;

/// Runtime wait state. `waited_chips` accumulates the chips of every
/// halt this attempt; stats reclassify their judgments as "waited".
#[derive(Resource, Debug, Default)]
pub struct WaitState {
    pub phase: WaitPhase,
    pub waited_chips: HashSet<usize>,
}

impl WaitState {
    pub fn halted(&self) -> bool {
        matches!(self.phase, WaitPhase::Halted(_))
    }
}

/// Run condition for the clock-sync chain: tick only while not halted.
pub fn wait_flowing(state: Option<Res<WaitState>>) -> bool {
    state.is_none_or(|s| !s.halted())
}

/// Drive halt/resume. Runs after Judge so this tick's hits are already
/// in `JudgedChips` (no one-tick spurious halts on exact hits).
#[allow(clippy::too_many_arguments)]
pub fn wait_watcher(
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    judged: Res<JudgedChips>,
    clock: Res<GameplayClock>,
    mut state: ResMut<WaitState>,
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if !clock.is_ready() {
        return;
    }
    if !session.trainer.wait_enabled {
        if state.halted() {
            crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
            state.phase = WaitPhase::Flowing;
        }
        return;
    }
    // Clone the phase before matching: matching on `&state.phase` would
    // hold a borrow of `state` across the arm bodies (E0502).
    match state.phase.clone() {
        WaitPhase::Flowing => {
            let span_start = session.current_attempt.start_ms;
            if let Some(set) = check_halt(&timeline, &judged.0, clock.current_ms, span_start) {
                state.waited_chips.extend(set.chips.iter().copied());
                crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Halted(set);
            }
        }
        WaitPhase::Halted(set) => {
            if is_cleared(&set, &judged.0) {
                crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Flowing;
            }
        }
    }
}

/// Any seek resets to Flowing (audio ownership passes back to the seek
/// engine, which stops/restarts instances itself) and starts a fresh
/// waited-chip set for the new attempt.
pub fn reset_wait_on_seek(mut seeks: MessageReader<SeekToChartTime>, mut state: ResMut<WaitState>) {
    if seeks.read().last().is_some() {
        state.phase = WaitPhase::Flowing;
        state.waited_chips.clear();
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<WaitState>().add_systems(
        FixedUpdate,
        (
            reset_wait_on_seek.after(crate::seek::apply_seek_system),
            wait_watcher.after(crate::judge::judge_lane_hit_system),
        )
            .chain()
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
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
