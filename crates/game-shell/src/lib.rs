//! game-shell — AppState machine + Performance state wiring + transitions.
//!
//! ADR-0014: 300ms OutQuint screen fades via `TransitionRequest`.

use bevy::prelude::*;

pub mod nav;
pub mod score_store;
pub mod states;

mod performance;
mod transition;

pub use nav::{MidiConnected, NavAction, NavSource, NavVerb};
pub use score_store::ScoreStoreResource;
pub use states::{
    AppState, CompletedRunContext, CustomizeTab, EGameMode, EditorSession, PauseState,
    PendingCustomizeTab, PracticeIntent, RunKind, SelectedDifficulty, StageEntity, despawn_stage,
};
pub use transition::{TransitionRequest, request_transition};

/// Root plugin. Registers AppState + transitions + Performance wiring.
pub struct GameShellPlugin;

impl Plugin for GameShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_state::<PauseState>()
            .init_resource::<states::PracticeIntent>()
            .init_resource::<states::CompletedRunContext>()
            .init_resource::<states::SelectedDifficulty>()
            .init_resource::<states::EditorSession>()
            .init_resource::<states::PendingCustomizeTab>()
            .add_plugins((
                dtx_ui::plugin,
                nav::plugin,
                transition::plugin,
                performance::plugin,
            ));
    }
}
