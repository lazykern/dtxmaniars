//! Shared Bevy wrapper around `dtx_scoring::ScoreStore`.
//!
//! Lives in game-shell so both game-results (writes after a play)
//! and game-menu (reads for song-select display) can use it without
//! depending on each other. Initialized and loaded at startup by the
//! desktop app.

use bevy::prelude::*;
use dtx_scoring::ScoreStore;

/// Bevy wrapper around `dtx_scoring::ScoreStore`.
#[derive(Resource, Deref, DerefMut, Default, Debug, Clone)]
pub struct ScoreStoreResource(pub ScoreStore);
