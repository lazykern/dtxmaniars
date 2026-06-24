//! Gameplay resources.

use std::path::PathBuf;

use bevy::prelude::*;
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
/// to display Perfect/Great/Good/Ok/Miss breakdown.
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
    /// Total judgments (Perfect + Great + Good + Ok + Miss).
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
