//! Gameplay resources.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_core::{Chart, Metadata};

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

/// Cumulative score. Updated by [`crate::score::update_score_system`].
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Score(pub u64);

/// Current and max combo. Miss resets current to 0.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Combo {
    pub current: u32,
    pub max: u32,
}

/// The AudioClock ms at which gameplay started. Used to compute absolute
/// chip ms from relative chart coordinates. Set on `OnEnter(Screen::Playing)`.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct GameStartMs(pub i64);

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
}

/// Scroll speed multiplier from `dtx-config` (`gameplay.scroll_speed`).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScrollSettings {
    pub pixels_per_ms: f32,
}

impl ScrollSettings {
    pub const BASE_PIXELS_PER_MS: f32 = 0.5;

    pub fn from_scroll_speed(speed: f32) -> Self {
        Self {
            pixels_per_ms: Self::BASE_PIXELS_PER_MS * speed.max(0.1),
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
}

impl ActiveDrumSounds {
    pub fn reset(&mut self) {
        self.hh_open_instances.clear();
        self.stick_se_instances.clear();
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

/// Gameplay clock. Uses explicit wall-clock mode only for charts without BGM;
/// BGM-backed charts require audio position and never silently fall back.
///
/// Mirrors dtxpt's `ChartClock` — `current_ms` is never `None` during
/// gameplay, so downstream systems (scroll, input, judge) can rely on it.
#[derive(Resource, Default, Debug)]
pub struct GameplayClock {
    pub current_ms: i64,
    started: bool,
    start_instant: Option<std::time::Instant>,
    mode: ClockMode,
    waiting_for_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ClockMode {
    #[default]
    WallClock,
    AudioRequired,
}

impl GameplayClock {
    pub fn start(&mut self) {
        self.start_wall_clock();
    }

    pub fn start_wall_clock(&mut self) {
        self.started = true;
        self.start_instant = Some(std::time::Instant::now());
        self.current_ms = 0;
        self.mode = ClockMode::WallClock;
        self.waiting_for_audio = false;
    }

    pub fn start_audio_required(&mut self) {
        self.started = true;
        self.start_instant = Some(std::time::Instant::now());
        self.current_ms = 0;
        self.mode = ClockMode::AudioRequired;
        self.waiting_for_audio = true;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn is_waiting_for_audio(&self) -> bool {
        self.waiting_for_audio
    }

    pub fn is_ready(&self) -> bool {
        self.started && !self.waiting_for_audio
    }

    /// Sync clock from audio position, or wall-clock only in explicit wall mode.
    pub fn sync(&mut self, audio_ms: Option<i64>) {
        if !self.started {
            return;
        }
        if let Some(ms) = audio_ms {
            self.current_ms = ms;
            self.waiting_for_audio = false;
        } else {
            match self.mode {
                ClockMode::WallClock => {
                    if let Some(start) = self.start_instant {
                        self.current_ms = start.elapsed().as_millis() as i64;
                    }
                    self.waiting_for_audio = false;
                }
                ClockMode::AudioRequired => {
                    self.waiting_for_audio = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GameplayClock;

    #[test]
    fn audio_required_clock_does_not_advance_without_audio_position() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();
        clock.sync(Some(250));
        clock.sync(None);

        assert_eq!(clock.current_ms, 250);
        assert!(clock.is_waiting_for_audio());
    }

    #[test]
    fn audio_required_clock_is_not_ready_until_audio_position_exists() {
        let mut clock = GameplayClock::default();
        clock.start_audio_required();

        assert!(clock.is_started());
        assert!(!clock.is_ready());

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
}
