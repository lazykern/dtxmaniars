//! Guitar gameplay components.

use bevy::prelude::*;

use crate::events::LaneId;

/// A note entity currently on screen.
#[derive(Component, Debug, Clone, Copy)]
pub struct Note {
    /// Index into ActiveChart.chips.
    pub chip_id: usize,
    /// Lane this note is heading toward.
    pub lane: LaneId,
    /// AudioClock ms at which the note should be hit.
    pub target_ms: i64,
}

/// Marker for note visuals. Separated from `Note` so the lane-of logic can
/// be tested without spawning a full entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct NoteVisual;
