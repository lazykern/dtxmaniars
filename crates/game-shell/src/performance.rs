//! CStagePerfDrumsScreen / CStagePerfGuitarScreen — gameplay stage.
//!
//! Both gameplay-drums and gameplay-guitar register their own plugins
//! independently. This stage plugin just owns the OnEnter fade and the
//! ESC → Result transition.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (194KB)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`

use bevy::prelude::*;

use crate::fade::start_fade;
use crate::states::{AppState, EGameMode};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), (start_fade, log_mode))
        .add_systems(
            Update,
            performance_input.run_if(in_state(AppState::Performance)),
        );
}

/// Log the active EGameMode on entering Performance. Verifies the user-selected
/// mode actually drives the stage (per M6b contract).
fn log_mode(mode: Res<EGameMode>) {
    info!("Performance: EGameMode = {:?}", *mode);
}

fn performance_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    // ESC → result screen (M5 ships game-results crate).
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Result);
    }
}
