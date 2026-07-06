//! game-shell — AppState machine + Performance state wiring + transitions.
//!
//! ADR-0014: 300ms OutQuint screen fades via `TransitionRequest`.

use bevy::prelude::*;

pub mod states;

mod performance;
mod transition;

pub use states::{despawn_stage, AppState, EGameMode, PauseState, PracticeIntent, StageEntity};
pub use transition::{request_transition, TransitionRequest};

/// Root plugin. Registers AppState + transitions + Performance wiring.
pub struct GameShellPlugin;

impl Plugin for GameShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_state::<PauseState>()
            .init_resource::<states::PracticeIntent>()
            .add_plugins((dtx_ui::plugin, transition::plugin, performance::plugin));
    }
}
