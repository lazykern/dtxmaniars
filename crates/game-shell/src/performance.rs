//! CStagePerfDrumsScreen / CStagePerfGuitarScreen — gameplay stage.
//!
//! Both gameplay-drums and gameplay-guitar register their own plugins
//! independently. This stage plugin just logs the active mode on enter.
//! Pause/quit/retry ESC handling lives in `gameplay-drums::pause`.
//!
//! ADR-0010 relaxed: fade UI removed (osu-style no fades).
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (194KB)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`

use bevy::prelude::*;

use crate::states::{AppState, EGameMode};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), log_mode);
}

/// Log the active EGameMode on entering Performance.
fn log_mode(mode: Res<EGameMode>) {
    info!("Performance: EGameMode = {:?}", *mode);
}
