#![allow(missing_docs)]
//! Guitar mode resources. Mirror of gameplay-drums::resources shape.

use std::path::PathBuf;

use bevy::prelude::*;
use dtx_core::{Chart, Metadata};

/// The chart currently being played (guitar mode).
#[derive(Resource, Default, Debug, Clone)]
pub struct ActiveChart {
    /// The parsed chart.
    pub chart: Chart,
    /// Optional source path (used by game-results for SHA-256 hashing).
    pub source_path: Option<PathBuf>,
}

impl ActiveChart {
    /// Construct with chart + optional source path.
    pub fn new(chart: Chart, source_path: Option<PathBuf>) -> Self {
        Self { chart, source_path }
    }

    /// Convenience: read metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.chart.metadata
    }
}

/// Cumulative score.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Score(pub u64);

/// Current and max combo.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Combo {
    pub current: u32,
    pub max: u32,
}

/// AudioClock ms at which gameplay started.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct GameStartMs(pub i64);

/// BGM auto-chip offset for guitar/bass mode. Mirrors `gameplay_drums::BgmAdjustState`.
///
/// `total_ms()` = common (global config) + song (per-chart `.score.ini`).
/// Applied where guitar BGM/SE auto-chips fire, and baked into `GameStartMs`
/// so the playfield clock matches what the user hears.
#[derive(Resource, Debug, Clone, Copy)]
pub struct BgmAdjustState {
    /// Global BGM adjust from `dtx_config::GameplayConfig::bgm_adjust_ms`.
    pub common_ms: i32,
    /// Per-chart BGM adjust from `<chart>.score.ini` `[File] BGMAdjust=`.
    pub song_ms: i32,
}

impl Default for BgmAdjustState {
    fn default() -> Self {
        Self {
            common_ms: 0,
            song_ms: 0,
        }
    }
}

impl BgmAdjustState {
    /// Combined shift applied to BGM/SE auto-chip times.
    pub fn total_ms(self) -> i32 {
        self.common_ms + self.song_ms
    }
}

/// Per-judgment counters.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgmentCounts {
    pub perfect: u32,
    pub great: u32,
    pub good: u32,
    pub ok: u32,
    pub miss: u32,
}

impl JudgmentCounts {
    /// Total judgments.
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

    /// Reset to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
