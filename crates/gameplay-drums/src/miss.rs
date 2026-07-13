//! Empty compatibility plugin. Miss detection lives in `scroll::despawn_missed_notes_system`.
//! (since it has direct access to the note entities and AudioClock).
//!
//! Kept as a stub so the `plugin` fn pattern stays uniform across sub-modules.

use bevy::prelude::*;

pub(super) fn plugin(_app: &mut App) {}
