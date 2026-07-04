//! AppState + EGameMode — mirrors DTXManiaNX's CStage.EStage + EGameMode.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs` — EStage enum (8 values)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Game/EInstrument.cs` — drums/guitar/bass

use bevy::prelude::*;

/// Top-level game state. One variant per DTXManiaNX stage.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// CStageStartup — initial boot + splash. Default.
    #[default]
    Startup,
    /// CStageTitle — title screen, "Press ENTER to start".
    Title,
    /// CStageConfig — settings/config screen.
    Config,
    /// CStageSongSelectionNew — BocuD's new song select.
    SongSelect,
    /// CStageSongLoading — chart + BGM preview load.
    SongLoading,
    /// CStagePerfDrumsScreen / CStagePerfGuitarScreen — gameplay.
    Performance,
    /// Stage-clear banner shown briefly after a survived performance.
    /// Ref `CStagePerfDrumsScreen.cs:270-279` (clear path).
    StageClear,
    /// Stage-failed banner shown when the life gauge drains out.
    /// Ref `CActPerfStageFailure.cs`.
    StageFailed,
    /// CStageResult — post-play results screen.
    Result,
    /// CStageChangeSkin — skin selection.
    ChangeSkin,
    /// CStageEnd — exit screen.
    End,
}

/// Which game mode the user picked. Resource, not state (sub-state within
/// Performance). Defaults to Drums per ADR-0001.
///
/// Reference: `references/DTXmaniaNX-BocuD/DTXMania/Game/EInstrument.cs`
/// has Drums / Guitar / Bass. M6b ships Drums + Guitar. Bass is M6.2.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum EGameMode {
    /// Drum kit (9 lanes, digits 1-9).
    #[default]
    Drums,
    /// Guitar (5 lanes R/G/B/Y/P).
    Guitar,
}

impl EGameMode {
    /// Short label for HUD.
    pub fn label(&self) -> &'static str {
        match self {
            EGameMode::Drums => "Drums",
            EGameMode::Guitar => "Guitar",
        }
    }

    /// Cycle to the next mode. Used by F2 in SongSelect.
    pub fn next(&self) -> Self {
        match self {
            EGameMode::Drums => EGameMode::Guitar,
            EGameMode::Guitar => EGameMode::Drums,
        }
    }
}

/// Pause state, orthogonal to [`AppState`]. Only meaningful during
/// `AppState::Performance`. Mirrors dtxpt's `PauseState`.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PauseState {
    /// Gameplay is running normally.
    #[default]
    Running,
    /// Gameplay is paused (BGM + clock frozen, overlay shown).
    Paused,
}

impl PauseState {
    /// True when paused.
    pub fn is_paused(self) -> bool {
        matches!(self, PauseState::Paused)
    }
}

/// Marker component: entity belongs to the named stage. OnExit despawns.
#[derive(Component)]
pub struct StageEntity(pub AppState);

/// Generic despawn-by-component helper. Used by each stage's OnExit.
pub fn despawn_stage<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
