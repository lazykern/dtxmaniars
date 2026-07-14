//! Wait mode: halt the clock at any unhit note until the correct pads
//! clear it. Spec: docs/superpowers/specs/2026-07-11-practice-wait-mode-design.md

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

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
/// attempt span, plus every drum chip sharing its target time.
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
        .filter(|e| e.judge_ms == target_ms && lane_of(e.channel).is_some())
        .map(|e| e.chip_idx)
        .collect();
    Some(WaitSet { target_ms, chips })
}

/// True once every chip in the set has been judged (hit).
pub fn is_cleared(set: &WaitSet, judged: &HashSet<usize>) -> bool {
    set.chips.iter().all(|c| judged.contains(c))
}

/// Max acceptable spread between the earliest and latest hit in a chord
/// for it to count as "played together" (spec: 50ms, matches a Perfect
/// judge window).
pub const CHORD_WINDOW_MS: Duration = Duration::from_millis(50);

/// Spread (`max - min`) across every `chips` entry's recorded hit time.
/// `None` if any chip in `chips` has no recorded hit yet.
pub fn chord_spread(times: &HashMap<usize, Instant>, chips: &[usize]) -> Option<Duration> {
    let mut min = None;
    let mut max = None;
    for chip in chips {
        let t = *times.get(chip)?;
        min = Some(min.map_or(t, |earliest: Instant| earliest.min(t)));
        max = Some(max.map_or(t, |latest: Instant| latest.max(t)));
    }
    Some(max?.duration_since(min?))
}

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use game_shell::{AppState, PauseState};

use super::session::PracticeSession;
use crate::events::JudgmentEvent;
use crate::judge::JudgedChips;
use crate::resources::{ActiveDrumSounds, GameplayClock};
use crate::seek::SeekToChartTime;

/// Runtime wait state. `waited_chips` accumulates the chips of every
/// halt this attempt; stats reclassify their judgments as "waited".
#[derive(Resource, Debug, Default)]
pub struct WaitState {
    pub phase: WaitPhase,
    pub waited_chips: HashSet<usize>,
    /// Chart time at which wait mode was most recently enabled. Older notes
    /// belong to the preceding free-play segment and must not become a halt.
    pub enabled_from_ms: Option<i64>,
}

impl WaitState {
    pub fn halted(&self) -> bool {
        matches!(self.phase, WaitPhase::Halted(_))
    }

    /// Start a fresh wait-mode segment. This can run while the practice panel
    /// has paused the fixed-update schedule, so it also clears a prior halt.
    pub fn begin(&mut self, clock_ms: i64) {
        self.phase = WaitPhase::Flowing;
        self.enabled_from_ms = Some(clock_ms);
    }
}

/// Monotonic timestamp for every chip judged during the current attempt.
/// This deliberately does not use chart time: the gameplay clock stops while
/// wait mode is halted, but chord simultaneity must measure real elapsed time.
#[derive(Resource, Debug, Default)]
pub struct ChordHitTimes(pub HashMap<usize, Instant>);

/// Wait-mode judgements held until their chord has been accepted.
#[derive(Resource, Debug, Default)]
pub struct DeferredWaitJudgments(pub Vec<JudgmentEvent>);

fn take_deferred_judgments(
    deferred: &mut Vec<JudgmentEvent>,
    chips: &[usize],
) -> Vec<JudgmentEvent> {
    let mut released = Vec::new();
    deferred.retain(|event| {
        if chips.contains(&event.chip_idx) {
            released.push(*event);
            false
        } else {
            true
        }
    });
    released
}

fn discard_deferred_judgments(deferred: &mut Vec<JudgmentEvent>, chips: &[usize]) {
    deferred.retain(|event| !chips.contains(&event.chip_idx));
}

fn reset_wait_set_for_retry(
    set: &WaitSet,
    judged: &mut HashSet<usize>,
    chord_hits: &mut HashMap<usize, Instant>,
    deferred: &mut Vec<JudgmentEvent>,
) {
    for chip in &set.chips {
        judged.remove(chip);
        chord_hits.remove(chip);
    }
    discard_deferred_judgments(deferred, &set.chips);
}

fn write_deferred_judgments(
    deferred: &mut Vec<JudgmentEvent>,
    chips: &[usize],
    events: &mut MessageWriter<JudgmentEvent>,
) {
    for event in take_deferred_judgments(deferred, chips) {
        events.write(event);
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
    mut judged: ResMut<JudgedChips>,
    clock: Res<GameplayClock>,
    mut state: ResMut<WaitState>,
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut chord_hits: ResMut<ChordHitTimes>,
    mut deferred: ResMut<DeferredWaitJudgments>,
    mut events: MessageWriter<JudgmentEvent>,
    mut toasts: ResMut<crate::practice::toast::ToastQueue>,
) {
    if !clock.is_ready() {
        return;
    }
    if !session.trainer.wait_enabled() {
        if state.halted() {
            crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
            state.phase = WaitPhase::Flowing;
        }
        state.enabled_from_ms = None;
        chord_hits.0.clear();
        for event in deferred.0.drain(..) {
            events.write(event);
        }
        return;
    }
    // Clone the phase before matching: matching on `&state.phase` would
    // hold a borrow of `state` across the arm bodies (E0502).
    match state.phase.clone() {
        WaitPhase::Flowing => {
            let span_start = state
                .enabled_from_ms
                .unwrap_or(session.current_attempt.start_ms)
                .max(session.current_attempt.start_ms);
            if let Some(set) = check_halt(&timeline, &judged.0, clock.current_ms, span_start) {
                reset_wait_set_for_retry(&set, &mut judged.0, &mut chord_hits.0, &mut deferred.0);
                state.waited_chips.extend(set.chips.iter().copied());
                crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Halted(set);
            } else if let Some(set) = flush_resolved_deferred_judgments(
                &timeline,
                &judged.0,
                &mut chord_hits,
                &mut deferred.0,
                &mut events,
            ) {
                for chip in &set.chips {
                    judged.0.remove(chip);
                    chord_hits.0.remove(chip);
                }
                discard_deferred_judgments(&mut deferred.0, &set.chips);
                state.waited_chips.extend(set.chips.iter().copied());
                crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Halted(set);
                toasts.push("Hit together — retry the chord");
            }
        }
        WaitPhase::Halted(set) => {
            if !is_cleared(&set, &judged.0) {
                return;
            }
            match chord_spread(&chord_hits.0, &set.chips) {
                Some(spread) if spread <= CHORD_WINDOW_MS => {
                    write_deferred_judgments(&mut deferred.0, &set.chips, &mut events);
                    for chip in &set.chips {
                        chord_hits.0.remove(chip);
                    }
                    crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                    state.phase = WaitPhase::Flowing;
                }
                _ => {
                    for chip in &set.chips {
                        judged.0.remove(chip);
                        chord_hits.0.remove(chip);
                    }
                    discard_deferred_judgments(&mut deferred.0, &set.chips);
                    toasts.push("Hit together — retry the chord");
                }
            }
        }
    }
}

/// Any seek resets to Flowing (audio ownership passes back to the seek
/// engine, which stops/restarts instances itself) and starts a fresh
/// waited-chip set for the new attempt.
pub fn reset_wait_on_seek(
    mut seeks: MessageReader<SeekToChartTime>,
    mut state: ResMut<WaitState>,
    mut chord_hits: ResMut<ChordHitTimes>,
    mut deferred: ResMut<DeferredWaitJudgments>,
) {
    if seeks.read().last().is_some() {
        state.phase = WaitPhase::Flowing;
        state.waited_chips.clear();
        state.enabled_from_ms = None;
        chord_hits.0.clear();
        deferred.0.clear();
    }
}

fn wait_set_at(timeline: &ChipTimeline, target_ms: i64) -> WaitSet {
    WaitSet {
        target_ms,
        chips: timeline
            .entries
            .iter()
            .filter(|entry| entry.judge_ms == target_ms && lane_of(entry.channel).is_some())
            .map(|entry| entry.chip_idx)
            .collect(),
    }
}

fn flush_resolved_deferred_judgments(
    timeline: &ChipTimeline,
    judged: &HashSet<usize>,
    chord_hits: &mut ChordHitTimes,
    deferred: &mut Vec<JudgmentEvent>,
    events: &mut MessageWriter<JudgmentEvent>,
) -> Option<WaitSet> {
    let mut targets: Vec<_> = deferred
        .iter()
        .filter_map(|event| timeline.judge_ms_by_idx.get(event.chip_idx).copied())
        .collect();
    targets.sort_unstable();
    targets.dedup();

    for target_ms in targets {
        let set = wait_set_at(timeline, target_ms);
        if !is_cleared(&set, judged) {
            continue;
        }
        match chord_spread(&chord_hits.0, &set.chips) {
            Some(spread) if spread <= CHORD_WINDOW_MS => {
                write_deferred_judgments(deferred, &set.chips, events);
            }
            _ => return Some(set),
        }
    }
    None
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<WaitState>()
        .init_resource::<ChordHitTimes>()
        .init_resource::<DeferredWaitJudgments>()
        .add_systems(
            FixedUpdate,
            reset_wait_on_seek
                .after(crate::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            FixedUpdate,
            wait_watcher
                .after(reset_wait_on_seek)
                .after(crate::judge::judge_lane_hit_system)
                // A seek restarts BGM on a fresh instance. Observe that
                // handle before halting so wait mode pauses the restarted
                // song, not the stopped pre-seek instance.
                .after(crate::seek::start_pending_bgm)
                .before(crate::DrumsSets::Score)
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
    fn partially_cleared_chord_still_requires_the_whole_chord() {
        let tl = timeline();
        let judged: HashSet<usize> = [1, 2].into();
        let set = check_halt(&tl, &judged, 2_003, 0).unwrap();
        assert_eq!(set.chips, vec![2, 3]);
    }

    #[test]
    fn halting_partial_chord_resets_every_member_for_visible_retry() {
        let set = WaitSet {
            target_ms: 2_000,
            chips: vec![2, 3],
        };
        let mut judged = HashSet::from([1, 2]);
        let mut hit_times = HashMap::from([(2, Instant::now())]);
        let mut deferred = vec![JudgmentEvent {
            lane: 1,
            kind: dtx_scoring::JudgmentKind::Perfect,
            delta_ms: -10,
            chip_idx: 2,
        }];

        reset_wait_set_for_retry(&set, &mut judged, &mut hit_times, &mut deferred);

        assert_eq!(judged, HashSet::from([1]));
        assert!(hit_times.is_empty());
        assert!(deferred.is_empty());
    }

    #[test]
    fn reenabled_wait_starts_after_the_previous_free_play_segment() {
        let tl = timeline();
        let mut state = WaitState::default();
        state.begin(3_000);

        let span_start = state.enabled_from_ms.unwrap_or(0).max(0);
        assert_eq!(check_halt(&tl, &HashSet::new(), 3_000, span_start), None);

        let set = check_halt(&tl, &HashSet::new(), 4_001, span_start).unwrap();
        assert_eq!(set.target_ms, 4_000);
        assert_eq!(set.chips, vec![4]);
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

    #[test]
    fn chord_spread_accepts_within_window() {
        let start = std::time::Instant::now();
        let times = [
            (2, start),
            (3, start + std::time::Duration::from_millis(30)),
        ]
        .into();
        assert_eq!(
            chord_spread(&times, &[2, 3]),
            Some(std::time::Duration::from_millis(30))
        );
        assert!(chord_spread(&times, &[2, 3]).unwrap() <= CHORD_WINDOW_MS);
    }

    #[test]
    fn chord_spread_flags_outside_window() {
        let start = std::time::Instant::now();
        let times = [
            (2, start),
            (3, start + std::time::Duration::from_millis(80)),
        ]
        .into();
        assert_eq!(
            chord_spread(&times, &[2, 3]),
            Some(std::time::Duration::from_millis(80))
        );
        assert!(chord_spread(&times, &[2, 3]).unwrap() > CHORD_WINDOW_MS);
    }

    #[test]
    fn chord_spread_uses_full_range_for_three_notes() {
        let start = std::time::Instant::now();
        let times = [
            (2, start),
            (3, start + std::time::Duration::from_millis(45)),
            (4, start + std::time::Duration::from_millis(90)),
        ]
        .into();
        assert_eq!(
            chord_spread(&times, &[2, 3, 4]),
            Some(std::time::Duration::from_millis(90))
        );
    }

    #[test]
    fn chord_spread_none_when_a_chip_not_yet_hit() {
        let times = [(2, std::time::Instant::now())].into();
        assert_eq!(chord_spread(&times, &[2, 3]), None);
    }

    #[test]
    fn deferred_judgments_release_only_the_accepted_chord() {
        let mut deferred = vec![
            crate::events::JudgmentEvent {
                lane: 1,
                kind: dtx_scoring::JudgmentKind::Perfect,
                delta_ms: 0,
                chip_idx: 2,
            },
            crate::events::JudgmentEvent {
                lane: 2,
                kind: dtx_scoring::JudgmentKind::Perfect,
                delta_ms: 0,
                chip_idx: 3,
            },
            crate::events::JudgmentEvent {
                lane: 4,
                kind: dtx_scoring::JudgmentKind::Great,
                delta_ms: 30,
                chip_idx: 4,
            },
        ];

        let released = take_deferred_judgments(&mut deferred, &[2, 3]);

        assert_eq!(
            released
                .iter()
                .map(|event| event.chip_idx)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            deferred
                .iter()
                .map(|event| event.chip_idx)
                .collect::<Vec<_>>(),
            vec![4]
        );
    }

    fn wait_watcher_test_app(chord_hit_times: HashMap<usize, Instant>) -> App {
        let mut app = App::new();
        let mut session = PracticeSession::default();
        session.trainer.enable_wait(true);
        app.insert_resource(session);
        app.insert_resource(ChipTimeline::default());
        let mut judged = JudgedChips::default();
        judged.0.insert(2);
        judged.0.insert(3);
        app.insert_resource(judged);
        let mut clock = GameplayClock::default();
        clock.start();
        app.insert_resource(clock);
        app.insert_resource(BgmHandle::default());
        app.insert_resource(DrumPolyphony::default());
        app.insert_resource(ActiveDrumSounds::default());
        app.init_resource::<Assets<AudioInstance>>();
        app.insert_resource(ChordHitTimes(chord_hit_times));
        app.init_resource::<DeferredWaitJudgments>();
        app.add_message::<JudgmentEvent>();
        app.insert_resource(WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![2, 3],
            }),
            waited_chips: [2, 3].into(),
            ..default()
        });
        app.init_resource::<crate::practice::toast::ToastQueue>();
        app.add_systems(Update, wait_watcher);
        app
    }

    #[test]
    fn wait_watcher_clears_chord_hit_within_window() {
        let start = Instant::now();
        let mut app =
            wait_watcher_test_app([(2, start), (3, start + Duration::from_millis(30))].into());
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(!state.halted(), "spread within window must clear the halt");
        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "cleared chord's hit times must be dropped"
        );
    }

    #[test]
    fn wait_watcher_rejects_chord_hit_outside_window() {
        let start = Instant::now();
        let mut app =
            wait_watcher_test_app([(2, start), (3, start + Duration::from_millis(80))].into());
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(
            state.halted(),
            "spread outside window must reject, stay halted"
        );
        assert!(
            matches!(&state.phase, WaitPhase::Halted(set) if set.chips == vec![2, 3]),
            "same wait-set stays active for retry"
        );
        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&2) && !judged.0.contains(&3),
            "rejected chips must be un-judged so the player can retry"
        );
        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "rejected chord's hit times must be cleared for the retry"
        );
        assert_eq!(
            app.world()
                .resource::<crate::practice::toast::ToastQueue>()
                .len(),
            1,
            "reject should surface one feedback toast"
        );
        assert!(
            app.world().resource::<DeferredWaitJudgments>().0.is_empty(),
            "rejected chord messages must never reach score or stats"
        );
    }

    #[test]
    fn seek_clears_chord_hit_times() {
        let mut app = App::new();
        app.add_message::<SeekToChartTime>();
        app.insert_resource(WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![2],
            }),
            waited_chips: [2].into(),
            ..default()
        });
        app.insert_resource(ChordHitTimes([(2, Instant::now())].into()));
        app.init_resource::<DeferredWaitJudgments>();
        app.add_systems(Update, reset_wait_on_seek);
        app.world_mut().write_message(SeekToChartTime {
            target_ms: 0,
            snap: None,
            attempt_start_ms: None,
        });
        app.update();

        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "seek must drop any in-flight chord hit times"
        );
    }

    #[test]
    fn wait_watcher_runs_after_seek_bgm_restart() {
        let source = include_str!("wait.rs");
        let watcher = source
            .find("wait_watcher\n                .after(reset_wait_on_seek)")
            .expect("wait watcher registration");
        let restart_edge = source[watcher..]
            .find(".after(crate::seek::start_pending_bgm)")
            .expect("wait watcher must follow the deferred seek BGM restart");
        let plugin_end = source[watcher..]
            .find("#[cfg(test)]")
            .expect("plugin must precede tests");
        assert!(restart_edge < plugin_end);
    }

    #[test]
    fn seek_reset_is_not_gated_on_running_pause_state() {
        let source = include_str!("wait.rs");
        let reset = source
            .find("reset_wait_on_seek\n                .after(crate::seek::apply_seek_system)")
            .expect("seek reset registration");
        let watcher = source[reset..]
            .find("wait_watcher\n                .after(reset_wait_on_seek)")
            .expect("wait watcher registration");
        assert!(
            !source[reset..reset + watcher].contains("PauseState::Running"),
            "paused timeline seeks must still clear a stale wait halt"
        );
    }
}
