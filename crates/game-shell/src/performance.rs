//! CStagePerfDrumsScreen / CStagePerfGuitarScreen — gameplay stage.
//!
//! Both gameplay-drums and gameplay-guitar register their own plugins
//! independently. This stage plugin just logs the active mode and the
//! ESC → Result transition.
//!
//! ADR-0010 relaxed: fade UI removed (osu-style no fades).
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (194KB)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`

use bevy::prelude::*;

use crate::states::{AppState, EGameMode};
use crate::transition::{TransitionRequest, request_transition};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), log_mode)
        .add_systems(
            Update,
            performance_input.run_if(in_state(AppState::Performance)),
        );
}

/// Log the active EGameMode on entering Performance.
fn log_mode(mode: Res<EGameMode>) {
    info!("Performance: EGameMode = {:?}", *mode);
}

fn performance_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Result);
    }
}
