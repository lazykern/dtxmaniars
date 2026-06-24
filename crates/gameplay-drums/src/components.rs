//! Gameplay entity components.

use crate::lane_map::LaneId;
use bevy::prelude::*;

/// A single chip currently on the highway (spawned by `scroll::spawn_notes_system`).
///
/// `target_ms` is the absolute audio-clock ms when this note should be hit.
/// `chip_id` is the index into `ActiveChart.chips` for reverse-lookup.
#[derive(Component, Debug, Clone, Copy)]
pub struct Note {
    pub chip_id: usize,
    pub lane: LaneId,
    pub target_ms: i64,
}

/// Marker for the visual entity (sprite/text/etc.) that renders a `Note`.
///
/// Decoupled so M3+ can swap visual implementations (Sprite → 3D mesh, etc.).
#[derive(Component, Debug, Clone, Copy)]
pub struct NoteVisual;

/// Last judgment shown in HUD. Updated by score system, consumed by HUD.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastJudgment(pub Option<crate::events::JudgmentEvent>);
