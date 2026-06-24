//! game-shell — AppState machine + Performance state wiring.
//!
//! ADR-0010 relaxed: UI/skin files stripped. StageManager's C# form
//! boilerplate is replaced by Bevy State and StageEntity cleanup.
//!
//! ## Reference
//! - `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs` (full, 699 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs` (EStage enum)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (194KB)

use bevy::prelude::*;

pub mod states;

mod performance;

pub use states::{AppState, EGameMode, StageEntity, despawn_stage};

/// Root plugin. Registers AppState + Performance state wiring.
/// Game-menu registers the other states' plugins separately.
pub struct GameShellPlugin;

impl Plugin for GameShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_plugins(performance::plugin);
    }
}
