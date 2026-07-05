//! Gameplay resources.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_core::{beat_lines::TimingLine, Chart, Metadata};

/// Expanded bar/beat timing lines for the active chart.
///
/// Built on Performance enter from [`dtx_core::expand_timing_lines`].
#[derive(Resource, Default, Debug, Clone)]
pub struct TimingLineList {
    pub lines: Vec<TimingLine>,
    pub base_bpm: f32,
}

impl TimingLineList {
    pub fn from_chart(chart: &Chart) -> Self {
        Self {
            lines: dtx_core::expand_timing_lines(chart),
            base_bpm: chart.metadata.bpm.unwrap_or(120.0),
        }
    }
}

/// The chart currently being played. Set by the loader before entering gameplay.
///
/// Default: empty chart (no chips). The scroll/judge systems no-op on empty.
#[derive(Resource, Default, Debug, Clone)]
pub struct ActiveChart {
    /// The parsed chart (chips + metadata).
    pub chart: Chart,
    /// Optional source path (used by game-results for SHA-256 hashing).
    pub source_path: Option<PathBuf>,
}

impl ActiveChart {
    /// Construct an `ActiveChart` with chart and optional source path.
    pub fn new(chart: Chart, source_path: Option<PathBuf>) -> Self {
        Self { chart, source_path }
    }

    /// Convenience: read metadata by delegating to the inner chart.
    pub fn metadata(&self) -> &Metadata {
        &self.chart.metadata
    }
}

/// Cumulative (rounded) score shown in the HUD / results.
/// Updated by [`crate::score::update_score_system`].
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Score(pub u64);

/// DTXManiaNX XG scoring state (fractional accumulator + chart constants).
///
/// The XG formula produces non-integer per-hit deltas, so we accumulate in f64
/// and round into [`Score`] for display. `total_notes` is `nComboMax`
/// (`CStagePerfCommonScreen.cs:1619`), `bonus_chips` is `nボーナスチップ数`
/// (unmodeled → 0).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct DrumScoring {
    pub accum: f64,
    pub total_notes: u32,
    pub bonus_chips: u32,
    /// Guards against applying the end-of-song FC/EXC bonus more than once.
    pub end_bonus_applied: bool,
}

impl DrumScoring {
    /// Reset for a fresh play of `total_notes` judgeable drum chips.
    pub fn reset(&mut self, total_notes: u32) {
        self.accum = 0.0;
        self.total_notes = total_notes;
        self.bonus_chips = 0;
        self.end_bonus_applied = false;
    }
}

/// Current and max combo. Miss resets current to 0.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Combo {
    pub current: u32,
    pub max: u32,
}

/// Chart time (ms) that corresponds to BGM playback position 0.
///
/// When the primary BGM chip starts mid-chart (common in DTX), kira reports
/// position 0 at that chip's [`chip_target_ms`], not at measure 0. The gameplay
/// clock is `GameStartMs + audio_position_ms`. Set on Performance enter from
/// the primary BGM chip (or 0 when BGM starts at chart time 0).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct GameStartMs(pub i64);

/// Global input timing offset (ms) from `dtx-config` (`gameplay.input_offset_ms`).
///
/// Applied to the judgement clock: the measured hit time is shifted by
/// `-offset` before comparison, so a positive offset judges inputs as if they
/// arrived later (compensating an early-hitting player / audio latency).
/// Mirrors DTXManiaNX `nInputAdjustTimeMs`.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct InputOffsetMs(pub i32);

/// BGM auto-chip timing offset: common config + per-song `.score.ini` `BGMAdjust`.
///
/// Applied only to auto-scheduled chips (BGM/SE layers), not drum judgment timing.
/// Reference: `CDTX.nBGMAdjust` + `CConfigIni.nCommonBGMAdjustMs`.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct BgmAdjustState {
    pub common_ms: i32,
    pub song_ms: i32,
}

impl BgmAdjustState {
    pub fn total_ms(self) -> i32 {
        self.common_ms + self.song_ms
    }
}

/// Runtime toggle for the in-song performance debug HUD (F11 / NX Help).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ShowPerfInfo(pub bool);

/// `bMetronome` — click when bar/beat lines cross the judge line.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MetronomeEnabled(pub bool);

/// `nLaneDisp` — whether bar/beat lines render (ALL ON / HALF only).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ShowTimingLines(pub bool);

/// Line ids that already fired metronome / cross detection this stage.
#[derive(Resource, Default, Debug)]
pub struct TimingLineCrossed(pub std::collections::HashSet<usize>);

/// Preloaded metronome click (`Sounds/Metronome.ogg` in BocuD).
#[derive(Resource, Default, Debug)]
pub struct MetronomeSound(pub Option<Handle<bevy_kira_audio::prelude::AudioSource>>);

/// Per-judgment counters accumulated during a song. Read by `game-results`
/// to display Perfect/Great/Good/Poor/Miss breakdown.
///
/// Updated by [`crate::score::update_score_system`] (each `JudgmentEvent`).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgmentCounts {
    pub perfect: u32,
    pub great: u32,
    pub good: u32,
    pub ok: u32,
    pub miss: u32,
}

impl JudgmentCounts {
    /// Total judgments (Perfect + Great + Good + Poor + Miss).
    pub fn total(&self) -> u32 {
        self.perfect + self.great + self.good + self.ok + self.miss
    }

    /// Perfect percentage (0..100). 0 if total == 0.
    pub fn perfect_pct(&self) -> f32 {
        let t = self.total();
        if t == 0 {
            0.0
        } else {
            self.perfect as f32 / t as f32 * 100.0
        }
    }

    /// Reset all counters to zero (used on re-entry to Performance).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// GITADORA-style achievement value in 0..100 (Perfect=100, Great=80,
    /// Good=60, Ok=40, Miss=0), weighted over total judged chips.
    pub fn achievement_pct(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        let weighted = self.perfect as f32 * 100.0
            + self.great as f32 * 80.0
            + self.good as f32 * 60.0
            + self.ok as f32 * 40.0;
        weighted / total as f32
    }
}

/// Running "Skills by Song" + max possible skill. Updated as judgments land.
///
/// Computed via [`crate::skill::calculate_skill_new`] × chart_level × 0.33.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct SkillValue {
    pub current: f64,
    /// Maximum theoretical skill (all P + full combo).
    pub max: f64,
}

/// Fast / Slow hit counter (BocuD `CActPerfDrumsJudgementString`).
///
/// `fast` = early hits, `slow` = late hits. Updated by `score` system when
/// judgment delta is non-zero.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct FastSlowCount {
    pub fast: u32,
    pub slow: u32,
}

/// Scroll speed from `dtx-config` (`gameplay.scroll_speed`, a display
/// multiplier where 1.0 == DTXManiaNX "x1.0").
///
/// [`Self::pixels_per_ms`] is the DTXManiaNX drum scroll velocity expressed in
/// pixels-per-ms at the 720px reference height; render systems multiply it by
/// the live `PlayfieldLayout::scale`.
///
/// Reference: `CChip.ComputeDistanceFromBar`
/// (`references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CChip.cs:568-578`):
/// ```text
/// ScrollSpeedDrums = (nScrollSpeedIndex + 1) * 0.5 * 37.5 * 286 / 60000
/// ```
/// The config value is the display multiplier `mult = (idx + 1) * 0.5`, so
/// `(idx + 1) = 2 * mult` and the velocity simplifies to `mult * 0.17875`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScrollSettings {
    /// Scroll velocity (px/ms) at the 720px reference height.
    pub pixels_per_ms: f32,
    /// Playback speed multiplier (`nPlaySpeed / 20.0`). 1.0 = native.
    /// Higher values make the chart finish earlier; only scroll + orchestrator
    /// honor it (audio playback rate does NOT rescale — caveat per M13.5).
    pub play_speed: f32,
}

impl ScrollSettings {
    /// DTXManiaNX drum scroll velocity at display multiplier x1.0 (px/ms @ ref
    /// height): `2 * 0.5 * 37.5 * 286 / 60000 = 0.17875`.
    pub const NX_BASE_PIXELS_PER_MS: f32 = 0.17875;

    pub fn from_scroll_speed(multiplier: f32) -> Self {
        Self {
            pixels_per_ms: Self::NX_BASE_PIXELS_PER_MS * multiplier.max(0.1),
            play_speed: 1.0,
        }
    }
}

impl Default for ScrollSettings {
    fn default() -> Self {
        Self::from_scroll_speed(1.0)
    }
}

/// Drum hit audio settings from `dtx-config`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DrumAudioSettings {
    pub enabled: bool,
    pub master_volume: f32,
    pub drum_volume: f32,
}

impl Default for DrumAudioSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            master_volume: 0.8,
            drum_volume: 0.8,
        }
    }
}

/// Latest NoChip template per lane (updated as chart scrolls past each template).
#[derive(Resource, Debug, Clone, Default)]
pub struct CurrentEmptyHitTemplates {
    pub by_lane: [Option<dtx_core::EmptyHitEvent>; crate::lane_map::LANE_COUNT],
}

impl CurrentEmptyHitTemplates {
    pub fn reset(&mut self) {
        self.by_lane = [None; crate::lane_map::LANE_COUNT];
    }

    pub fn set(&mut self, lane: u8, event: dtx_core::EmptyHitEvent) {
        if let Some(slot) = self.by_lane.get_mut(lane as usize) {
            *slot = Some(event);
        }
    }

    pub fn get(&self, lane: u8) -> Option<&dtx_core::EmptyHitEvent> {
        self.by_lane.get(lane as usize).and_then(|s| s.as_ref())
    }
}

/// Tracks active HH instances for close-HH muting (BocuD LP muting pattern).
#[derive(Resource, Default, Debug)]
pub struct ActiveDrumSounds {
    pub hh_open_instances: Vec<Handle<AudioInstance>>,
    pub stick_se_instances: HashMap<dtx_core::EChannel, Handle<AudioInstance>>,
    /// Non-primary BGM channel layers (backing drums, etc.).
    pub layer_bgm_instances: Vec<Handle<AudioInstance>>,
}

impl ActiveDrumSounds {
    pub fn reset(&mut self) {
        self.hh_open_instances.clear();
        self.stick_se_instances.clear();
        self.layer_bgm_instances.clear();
    }

    pub fn track_layer_bgm(&mut self, handle: Handle<AudioInstance>) {
        self.layer_bgm_instances.push(handle);
    }

    /// Pause all tracked non-primary chart audio (layers, HH, stick SE).
    pub fn pause_all(&self, instances: &mut Assets<AudioInstance>) {
        for handle in self
            .hh_open_instances
            .iter()
            .chain(self.stick_se_instances.values())
            .chain(self.layer_bgm_instances.iter())
        {
            dtx_audio::pause_audio_instance(instances, handle);
        }
    }

    /// Resume all tracked non-primary chart audio.
    pub fn resume_all(&self, instances: &mut Assets<AudioInstance>) {
        for handle in self
            .hh_open_instances
            .iter()
            .chain(self.stick_se_instances.values())
            .chain(self.layer_bgm_instances.iter())
        {
            dtx_audio::resume_audio_instance(instances, handle);
        }
    }

    /// Stop all tracked non-primary chart audio (layer BGM stems, HH, stick SE).
    pub fn stop_all(&self, instances: &mut Assets<AudioInstance>) {
        for handle in self
            .hh_open_instances
            .iter()
            .chain(self.stick_se_instances.values())
            .chain(self.layer_bgm_instances.iter())
        {
            if let Some(mut inst) = instances.get_mut(handle) {
                inst.stop(AudioTween::default());
            }
        }
    }
}

/// Drum grouping + chart presence for judgment and hit sounds.
#[derive(Resource, Debug, Clone)]
pub struct DrumGameplaySettings {
    pub config: dtx_config::DrumsConfig,
    pub presence: crate::drum_groups::ChartChipPresence,
    pub groups: crate::drum_groups::EffectiveGroups,
}

impl Default for DrumGameplaySettings {
    fn default() -> Self {
        let config = dtx_config::DrumsConfig::default();
        let presence = crate::drum_groups::ChartChipPresence::default();
        Self {
            groups: crate::drum_groups::EffectiveGroups::from_config(&config, &presence),
            config,
            presence,
        }
    }
}

impl DrumGameplaySettings {
    pub fn rebuild_from_chart(&mut self, chart: &Chart) {
        self.presence = crate::drum_groups::ChartChipPresence::from_chart(chart);
        self.groups =
            crate::drum_groups::EffectiveGroups::from_config(&self.config, &self.presence);
    }
}

/// Gameplay clock. Free-running like DTXManiaNX's `CSoundManager.rcPerformanceTimer`
/// (`references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs`)
/// and dtxpt's `ChartClock` (`gameplay/clock.rs`, `audio/playback/transport.rs`):
/// the clock advances on its own and BGM playback position is used **only** for
/// drift correction — it is never a gate. This is deliberate: gating note
/// spawn/scroll on a live BGM position stalls the whole stage if the tracked
/// instance is still decoding / never reports `Playing`.
///
/// - `current_ms` is the authoritative judgement/scheduling clock. It advances
///   by the FixedUpdate delta each tick and is drift-corrected toward the
///   measured kira BGM position, so it is smooth between the coarse audio
///   position callbacks rather than stepping.
/// - The first real BGM position observed after (re)start snaps the clock onto
///   the audio timeline (`audio_synced`), so audio becomes authoritative the
///   instant it is available without a slow catch-up ramp.
/// - `visual_ms` / `prev_visual_ms` are a display clock that catches up toward
///   `current_ms` and is sampled by [`crate::interp::RenderClock`] for
///   sub-frame note motion (`lerp(prev, current, overstep_fraction)`).
#[derive(Resource, Default, Debug)]
pub struct GameplayClock {
    /// Judgement/scheduling clock in ms (rounded [`Self::audio_ms`]).
    pub current_ms: i64,
    started: bool,
    mode: ClockMode,
    /// True once a real BGM position has been observed since the last (re)start.
    /// Before this the clock free-runs on `dt`; the first observation snaps it
    /// onto the audio timeline, and later observations drift-correct it.
    audio_synced: bool,
    /// Authoritative f64 audio position (ms). Advanced by dt + drift-corrected.
    audio_ms: f64,
    /// Smoothed display clock (ms). Catches up toward `audio_ms`.
    visual_ms: f64,
    /// Start-of-tick value of `visual_ms`, used by the render interpolator.
    prev_visual_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ClockMode {
    #[default]
    WallClock,
    AudioRequired,
}

impl GameplayClock {
    /// Per-second gain applied to drift when correcting toward measured audio.
    /// Ported from dtxpt `VISUAL_CORRECTION_GAIN` (constants.rs:19).
    const CORRECTION_GAIN: f64 = 10.0;
    /// Max per-tick clock correction (ms). dtxpt `MAX_VISUAL_CORRECTION_SECS`.
    const MAX_CORRECTION_MS: f64 = 20.0;
    /// Tolerated backward audio-position read (ms). dtxpt `MAX_AUDIO_BACKSTEP_SECS`.
    const MAX_BACKSTEP_MS: f64 = 8.0;
    /// Suspicious position jump guard (ms). dtxpt BASS glitch hotfix.
    const GLITCH_JUMP_MS: f64 = 500.0;

    pub fn start(&mut self) {
        self.start_wall_clock();
    }

    pub fn start_wall_clock(&mut self) {
        self.start_in_mode(ClockMode::WallClock);
    }

    pub fn start_audio_required(&mut self) {
        self.start_in_mode(ClockMode::AudioRequired);
    }

    fn start_in_mode(&mut self, mode: ClockMode) {
        self.started = true;
        self.mode = mode;
        self.audio_synced = false;
        self.current_ms = 0;
        self.audio_ms = 0.0;
        self.visual_ms = 0.0;
        self.prev_visual_ms = 0.0;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    /// True while a BGM-backed chart has started but no audio position has been
    /// observed yet. Informational only — the clock still free-runs and
    /// `is_ready()` is already true, so this never blocks rendering.
    pub fn is_waiting_for_audio(&self) -> bool {
        self.started && self.mode == ClockMode::AudioRequired && !self.audio_synced
    }

    /// Ready as soon as the clock has started. The clock free-runs and folds in
    /// BGM position when available, so there is no "wait for audio" stall.
    pub fn is_ready(&self) -> bool {
        self.started
    }

    /// Start-of-tick visual clock (ms) for render interpolation.
    pub fn prev_visual_ms(&self) -> f64 {
        self.prev_visual_ms
    }

    /// End-of-tick visual clock (ms) for render interpolation.
    pub fn visual_ms(&self) -> f64 {
        self.visual_ms
    }

    /// Advance the clock by `dt_secs`, drift-correcting toward the measured
    /// kira BGM position `measured_ms` when available.
    ///
    /// This is the per-FixedUpdate-tick path. Ported from dtxpt
    /// `sync_elapsed_from_audio` (transport.rs:82-160).
    pub fn tick(&mut self, dt_secs: f64, measured_ms: Option<i64>) {
        if !self.started {
            return;
        }

        // First real BGM position after (re)start: snap onto the audio timeline
        // so audio becomes authoritative immediately, with no catch-up ramp.
        // Ported intent from dtxpt `set_clock_to_time` + the first sync in
        // `sync_elapsed_from_audio` (transport.rs:99-124). Unlike the old
        // behavior, we do NOT hold the clock at zero while waiting — it
        // free-runs so notes spawn/scroll from t=0.
        if !self.audio_synced {
            if let Some(ms) = measured_ms {
                self.audio_synced = true;
                self.audio_ms = ms as f64;
                self.visual_ms = self.audio_ms;
                self.prev_visual_ms = self.audio_ms;
                self.current_ms = self.audio_ms.round() as i64;
                return;
            }
        }

        let dt_ms = dt_secs * 1000.0;
        let prev_audio = self.audio_ms;
        let mut next_audio = prev_audio + dt_ms;

        if let Some(ms) = measured_ms {
            let measured = ms as f64;
            let glitch = (measured - prev_audio).abs() > Self::GLITCH_JUMP_MS && dt_secs <= 0.5;
            if !glitch && measured >= prev_audio - Self::MAX_BACKSTEP_MS {
                let drift = measured - next_audio;
                let catchup = (Self::CORRECTION_GAIN * dt_secs).clamp(0.0, 1.0);
                next_audio +=
                    (drift * catchup).clamp(-Self::MAX_CORRECTION_MS, Self::MAX_CORRECTION_MS);
            }
        }

        self.audio_ms = next_audio;
        self.current_ms = self.audio_ms.round() as i64;

        // Smoothed display clock: advance by dt, then catch up toward audio_ms.
        self.prev_visual_ms = self.visual_ms;
        self.visual_ms += dt_ms;
        let drift = self.audio_ms - self.visual_ms;
        let catchup = (Self::CORRECTION_GAIN * dt_secs).clamp(0.0, 1.0);
        self.visual_ms +=
            (drift * catchup).clamp(-Self::MAX_CORRECTION_MS, Self::MAX_CORRECTION_MS);
    }

    /// Snap the clock directly to `audio_ms`. `None` is a no-op (the free-running
    /// clock keeps its current value). Used by tests and non-tick callers.
    pub fn sync(&mut self, audio_ms: Option<i64>) {
        if !self.started {
            return;
        }
        if let Some(ms) = audio_ms {
            self.current_ms = ms;
            self.audio_ms = ms as f64;
            self.visual_ms = ms as f64;
            self.prev_visual_ms = ms as f64;
            self.audio_synced = true;
        }
    }
}

/// Per-slot accuracy history for the live graph (128 song-position buckets).
#[derive(Resource, Debug, Clone)]
pub struct AccuracyHistory {
    pub samples: [Option<f32>; 128],
}

impl Default for AccuracyHistory {
    fn default() -> Self {
        Self { samples: [None; 128] }
    }
}

impl AccuracyHistory {
    pub fn record(&mut self, slot: usize, accuracy_pct: f32) {
        if let Some(s) = self.samples.get_mut(slot) {
            *s = Some(accuracy_pct);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccuracyHistory, GameplayClock, JudgmentCounts, ScrollSettings};

    #[test]
    fn achievement_pct_empty_is_full() {
        assert!((JudgmentCounts::default().achievement_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn achievement_pct_all_perfect_is_100() {
        let c = JudgmentCounts {
            perfect: 10,
            ..Default::default()
        };
        assert!((c.achievement_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn achievement_pct_all_good_is_60() {
        let c = JudgmentCounts {
            good: 4,
            ..Default::default()
        };
        assert!((c.achievement_pct() - 60.0).abs() < 0.01);
    }

    #[test]
    fn scroll_velocity_matches_nx_at_x1() {
        // CChip.cs:568-578 → mult * 0.17875 px/ms at ref height.
        let s = ScrollSettings::from_scroll_speed(1.0);
        assert!((s.pixels_per_ms - 0.17875).abs() < 1e-6);
    }

    #[test]
    fn scroll_velocity_scales_linearly_with_multiplier() {
        let s = ScrollSettings::from_scroll_speed(2.0);
        assert!((s.pixels_per_ms - 0.3575).abs() < 1e-6);
    }

    #[test]
    fn sync_none_is_a_noop_on_free_running_clock() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();
        clock.sync(Some(250));
        clock.sync(None);

        // None must not rewind the clock; it keeps the last synced value.
        assert_eq!(clock.current_ms, 250);
        // A real position was observed, so it is no longer waiting for audio.
        assert!(!clock.is_waiting_for_audio());
    }

    #[test]
    fn audio_required_clock_is_ready_immediately_and_free_runs() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();

        // Free-running: ready as soon as started, even before any audio.
        assert!(clock.is_started());
        assert!(clock.is_ready());
        // But it still reports that it has not yet locked onto audio.
        assert!(clock.is_waiting_for_audio());

        clock.sync(Some(250));

        assert!(clock.is_ready());
        assert!(!clock.is_waiting_for_audio());
    }

    #[test]
    fn wall_clock_mode_is_ready_immediately() {
        let mut clock = GameplayClock::default();
        clock.start_wall_clock();

        assert!(clock.is_ready());
        assert!(!clock.is_waiting_for_audio());
    }

    #[test]
    fn wall_clock_mode_can_advance_without_audio_position() {
        let mut clock = GameplayClock::default();
        clock.start_wall_clock();
        clock.sync(None);

        assert!(clock.is_started());
        assert!(!clock.is_waiting_for_audio());
    }

    #[test]
    fn tick_wall_clock_advances_by_dt() {
        let mut clock = GameplayClock::default();
        clock.start_wall_clock();
        // 3 ticks of 16.667ms ≈ 50ms.
        for _ in 0..3 {
            clock.tick(1.0 / 60.0, None);
        }
        assert!(
            (clock.current_ms - 50).abs() <= 1,
            "got {}",
            clock.current_ms
        );
    }

    #[test]
    fn tick_audio_required_free_runs_then_snaps_to_first_audio() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();
        // No audio yet: the clock free-runs by dt (does NOT hold at zero).
        clock.tick(1.0 / 60.0, None);
        assert!(clock.is_waiting_for_audio());
        assert!(
            clock.current_ms > 0,
            "free-run should advance, got {}",
            clock.current_ms
        );

        // First real position snaps the clock straight onto the audio timeline.
        clock.tick(1.0 / 60.0, Some(1000));
        assert!(!clock.is_waiting_for_audio());
        assert_eq!(clock.current_ms, 1000);
    }

    #[test]
    fn tick_drift_corrects_toward_measured_audio() {
        let mut clock = GameplayClock::default();
        clock.start_wall_clock();
        // Lock onto audio at 0 first (first measured always snaps).
        clock.tick(1.0 / 60.0, Some(0));
        // Free-run to ~500ms with no further positions.
        for _ in 0..30 {
            clock.tick(1.0 / 60.0, None);
        }
        let before = clock.current_ms;
        // Measured audio is a little ahead; clock should nudge toward it, bounded.
        clock.tick(1.0 / 60.0, Some(before + 100));
        assert!(clock.current_ms > before);
        // Correction is capped (< the full 100ms jump), not a snap.
        assert!(clock.current_ms < before + 100);
    }

    #[test]
    fn first_measured_position_snaps_to_chart_time() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();
        // BGM stream position 0 at chart time 46000 (mid-chart primary chip).
        clock.tick(1.0 / 60.0, Some(46000));
        assert_eq!(clock.current_ms, 46000);
        assert!(!clock.is_waiting_for_audio());
    }

    #[test]
    fn visual_clock_tracks_audio() {
        let mut clock = GameplayClock::default();
        clock.start_wall_clock();
        for _ in 0..60 {
            clock.tick(1.0 / 60.0, None);
        }
        // visual clock stays close to the judgement clock.
        assert!((clock.visual_ms() - clock.current_ms as f64).abs() < 25.0);
    }

    #[test]
    fn accuracy_history_defaults_empty() {
        let h = AccuracyHistory::default();
        assert_eq!(h.samples.len(), 128);
        assert!(h.samples.iter().all(|s| s.is_none()));
    }

    #[test]
    fn accuracy_history_records_slot() {
        let mut h = AccuracyHistory::default();
        h.record(3, 88.0);
        assert_eq!(h.samples[3], Some(88.0));
    }
}
