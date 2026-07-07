//! Replay metadata skeleton.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::ChartIdentity;

/// Reference to a replay file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRef {
    /// Replay format version.
    pub format_version: u16,
    /// Relative or absolute replay path.
    pub path: PathBuf,
}

/// Header metadata for future replay files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayHeader {
    /// Replay file format version.
    pub format_version: u16,
    /// Scoring/judgment engine version.
    pub engine_version: u16,
    /// Chart identity.
    pub chart: ChartIdentity,
    /// Unix seconds.
    pub played_at: u64,
    /// Playback rate.
    pub rate: f32,
    /// Input offset in milliseconds.
    pub input_offset_ms: i32,
    /// BGM offset in milliseconds.
    pub bgm_offset_ms: i32,
    /// Visual offset in milliseconds.
    pub visual_offset_ms: i32,
}
