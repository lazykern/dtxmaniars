//! game-shell — AppState machine + DTXManiaNX fade transition (ADR-0010).
//!
//! Minimal: just the state machine + fade + performance-state wiring.
//! All menu stages (Title, SongSelect, Config, Result, ChangeSkin, End,
//! SongLoading, Startup) live in the `game-menu` crate and are registered
//! separately by the binary entry point.
//!
//! ## Reference
//! - `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs` (full, 699 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs` (EStage enum)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (194KB)

use bevy::prelude::*;

pub mod fade;
pub mod states;

mod performance;

pub use fade::{FADE_DURATION_MS, FadePlugin, FadeState};
pub use states::{AppState, EGameMode, StageEntity, despawn_stage};

/// Root plugin. Registers AppState + fade + Performance state wiring.
/// Game-menu registers the other states' plugins separately.
pub struct GameShellPlugin;

impl Plugin for GameShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_plugins(FadePlugin)
            .add_plugins(performance::plugin);
    }
}
