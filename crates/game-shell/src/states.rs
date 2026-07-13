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

/// Why a practice session was requested. Kept primitive so game-shell does
/// not depend on gameplay-drums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeReason {
    /// The player chose the ordinary practice action.
    Manual,
    /// Results identified a weak chart section, optionally tied to a lane.
    WeakSection {
        lane: Option<u8>,
        section_start_ms: i64,
    },
}

impl PracticeReason {
    pub fn lane(self) -> Option<u8> {
        match self {
            Self::Manual => None,
            Self::WeakSection { lane, .. } => lane,
        }
    }
}

/// Pre-roll expressed without depending on the gameplay practice transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PracticePreRoll {
    #[default]
    OneBar,
}

/// A preconfigured practice transport request originating outside gameplay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeRecommendation {
    pub loop_start_ms: i64,
    pub loop_end_ms: i64,
    pub pre_roll: PracticePreRoll,
    pub initial_tempo: f32,
    pub reason: PracticeReason,
}

impl PracticeRecommendation {
    pub fn weak_section(loop_start_ms: i64, loop_end_ms: i64, lane: Option<u8>) -> Self {
        Self {
            loop_start_ms,
            loop_end_ms,
            pre_roll: PracticePreRoll::OneBar,
            initial_tempo: 1.0,
            reason: PracticeReason::WeakSection {
                lane,
                section_start_ms: loop_start_ms,
            },
        }
    }

    pub fn has_valid_loop(self) -> bool {
        self.loop_start_ms >= 0 && self.loop_end_ms > self.loop_start_ms
    }
}

/// Set by song select or Results and read on Performance enter to insert a
/// practice session. Lives in game-shell so callers need no gameplay internals.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub enum PracticeIntent {
    /// Normal play.
    #[default]
    None,
    /// The player explicitly entered practice with default transport settings.
    Manual,
    /// Results supplied an immediately usable practice transport.
    Recommended(PracticeRecommendation),
}

impl PracticeIntent {
    pub fn is_requested(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn recommendation(self) -> Option<PracticeRecommendation> {
        match self {
            Self::Recommended(recommendation) => Some(recommendation),
            Self::None | Self::Manual => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunKind {
    #[default]
    Practice,
    Normal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunModifiers {
    pub no_fail: bool,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CompletedRunContext {
    pub kind: RunKind,
    pub playback_rate: f64,
    pub modifiers: RunModifiers,
}

impl CompletedRunContext {
    pub fn normal(playback_rate: f64, modifiers: RunModifiers) -> Self {
        Self {
            kind: RunKind::Normal,
            playback_rate,
            modifiers,
        }
    }
}

impl Default for CompletedRunContext {
    fn default() -> Self {
        Self {
            kind: RunKind::Practice,
            playback_rate: 1.0,
            modifiers: RunModifiers::default(),
        }
    }
}

/// Difficulty index (0 = BASIC) of the chart being played — the same value
/// the song wheel uses. Written by song loading on every SongLoading enter;
/// read by game-results to color the Lv chip. Lives in game-shell so
/// game-results doesn't need game-menu (same precedent as [`PracticeIntent`]).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedDifficulty(pub u8);

/// True while the layout-editor session (title → F2) is active: Performance
/// runs on autoplay in a seamless loop with the editor open; Esc exits to
/// Title instead of Results.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorSession(pub bool);

/// Which Customize-surface tab is active. SETTINGS group edits `config.toml`;
/// KIT group edits input profiles and the layout (lanes/widgets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomizeTab {
    Gameplay,
    Audio,
    Drums,
    System,
    Accessibility,
    Controls,
    Lanes,
    Widgets,
}

impl CustomizeTab {
    /// All tabs in rail order.
    pub const ALL: [CustomizeTab; 8] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
        CustomizeTab::Accessibility,
        CustomizeTab::Controls,
        CustomizeTab::Lanes,
        CustomizeTab::Widgets,
    ];
    /// Settings group (edits config.toml).
    pub const SETTINGS: [CustomizeTab; 5] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
        CustomizeTab::Accessibility,
    ];
    /// Kit group (edits layout.toml).
    pub const KIT: [CustomizeTab; 3] = [
        CustomizeTab::Controls,
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
            CustomizeTab::Accessibility => "Accessibility",
            CustomizeTab::Controls => "Controls",
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
    fn completed_run_defaults_to_safe_non_saving_practice() {
        let run = CompletedRunContext::default();
        assert_eq!(run.kind, RunKind::Practice);
        assert!((run.playback_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normal_run_records_its_rate() {
        let run = CompletedRunContext::normal(0.75, RunModifiers { no_fail: true });
        assert_eq!(run.kind, RunKind::Normal);
        assert!((run.playback_rate - 0.75).abs() < f64::EPSILON);
        assert!(run.modifiers.no_fail);
    }

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
    fn controls_is_a_kit_tab() {
        assert!(!CustomizeTab::Controls.is_settings());
        assert!(CustomizeTab::KIT.contains(&CustomizeTab::Controls));
    }

    #[test]
    fn accessibility_tab_is_a_settings_tab() {
        assert!(CustomizeTab::Accessibility.is_settings());
        assert_eq!(CustomizeTab::SETTINGS.len(), 5);
        assert_eq!(CustomizeTab::Accessibility.label(), "Accessibility");
    }

    #[test]
    fn customize_has_eight_tabs_after_accessibility_is_added() {
        assert_eq!(CustomizeTab::ALL.len(), 8);
        assert_eq!(CustomizeTab::Controls.label(), "Controls");
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

    #[test]
    fn selected_difficulty_defaults_to_basic() {
        assert_eq!(SelectedDifficulty::default().0, 0);
    }

    #[test]
    fn recommended_practice_intent_carries_its_loop() {
        let intent = PracticeIntent::Recommended(PracticeRecommendation::weak_section(
            1_000,
            5_000,
            Some(3),
        ));

        assert!(intent.is_requested());
        let recommendation = intent.recommendation().expect("recommendation retained");
        assert_eq!(recommendation.loop_start_ms, 1_000);
        assert_eq!(recommendation.loop_end_ms, 5_000);
        assert_eq!(recommendation.reason.lane(), Some(3));
    }
}
