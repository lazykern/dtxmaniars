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

/// Set by song select when the player chooses Practice instead of a
/// normal play; read on Performance enter to insert the practice
/// session. Lives in game-shell so game-menu doesn't need gameplay
/// internals to request practice.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PracticeIntent(pub bool);

/// True while the layout-editor session (title → F2) is active: Performance
/// runs on autoplay in a seamless loop with the editor open; Esc exits to
/// Title instead of Results.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorSession(pub bool);

/// Which Customize-surface tab is active. SETTINGS group edits `config.toml`;
/// KIT group edits the layout (lanes/widgets). Bindings tab lands in Phase 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomizeTab {
    Gameplay,
    Audio,
    Drums,
    System,
    Bindings,
    Lanes,
    Widgets,
}

impl CustomizeTab {
    /// All tabs in rail order.
    pub const ALL: [CustomizeTab; 7] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
        CustomizeTab::Bindings,
        CustomizeTab::Lanes,
        CustomizeTab::Widgets,
    ];
    /// Settings group (edits config.toml).
    pub const SETTINGS: [CustomizeTab; 4] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
    ];
    /// Kit group (edits layout.toml).
    pub const KIT: [CustomizeTab; 3] = [
        CustomizeTab::Bindings,
        CustomizeTab::Lanes,
        CustomizeTab::Widgets,
    ];

    /// Short rail label.
    pub fn label(self) -> &'static str {
        match self {
            CustomizeTab::Gameplay => "Gameplay",
            CustomizeTab::Audio => "Audio",
            CustomizeTab::Drums => "Drums",
            CustomizeTab::System => "System",
            CustomizeTab::Bindings => "Bindings",
            CustomizeTab::Lanes => "Lanes",
            CustomizeTab::Widgets => "Widgets",
        }
    }

    /// True if this tab edits `config.toml` (vs the layout).
    pub fn is_settings(self) -> bool {
        Self::SETTINGS.contains(&self)
    }

    /// Next tab in rail order, wrapping.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Previous tab in rail order, wrapping.
    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Initial Customize tab to open, set by the entry key (F1/F2) before the
/// SongLoading→Performance transition and consumed when the surface opens.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PendingCustomizeTab(pub Option<CustomizeTab>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customize_tab_groups_partition_all_variants() {
        let settings = CustomizeTab::SETTINGS;
        let kit = CustomizeTab::KIT;
        assert_eq!(settings.len() + kit.len(), CustomizeTab::ALL.len());
        for t in CustomizeTab::ALL {
            assert!(
                settings.contains(&t) ^ kit.contains(&t),
                "{t:?} must be in exactly one group"
            );
        }
    }

    #[test]
    fn bindings_is_a_kit_tab() {
        assert!(!CustomizeTab::Bindings.is_settings());
        assert!(CustomizeTab::KIT.contains(&CustomizeTab::Bindings));
    }

    #[test]
    fn pending_customize_tab_defaults_none() {
        assert_eq!(PendingCustomizeTab::default().0, None);
    }

    #[test]
    fn customize_tab_next_prev_cycle() {
        assert_eq!(CustomizeTab::Gameplay.next(), CustomizeTab::Audio);
        assert_eq!(CustomizeTab::Widgets.next(), CustomizeTab::Gameplay);
        assert_eq!(CustomizeTab::Gameplay.prev(), CustomizeTab::Widgets);
        assert_eq!(CustomizeTab::Audio.prev(), CustomizeTab::Gameplay);
    }
}
