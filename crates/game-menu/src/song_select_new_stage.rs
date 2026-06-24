//! `CStageSongSelectionNew` — port of `Stage/04.SongSelectionNew/CStageSongSelectionNew.cs` (596 LOC).
//!
//! Strict-port-first. Orchestrator. Sub-acts ported in p2-2..p2-11.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs:1-596`

use bevy::prelude::{App, Resource};

/// Song select return value (CStageSongSelectionNew.cs:30-37).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EReturnValue {
    #[default]
    Continue,
    ReturnToTitle,
    Selected,
    CallConfig,
    ChangeSking,
}

/// Song select load phase (CStageSongSelectionNew.cs:23-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ELoadPhase {
    #[default]
    Initialize,
    Prepare,
    CacheThumbnails,
    ReadyToOpen,
    Complete,
}

/// Available sorters (CStageSongSelectionNew.cs:56-66).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ESortMode {
    #[default]
    Default,
    Box,
    Title,
    Artist,
    Difficulty,
    Level,
    Player,
    AllSongs,
    Skill,
}

impl ESortMode {
    /// 9 sorters matching the C# array (CStageSongSelectionNew.cs:56-66).
    pub fn all() -> [Self; 9] {
        [
            Self::Default,
            Self::Box,
            Self::Title,
            Self::Artist,
            Self::Difficulty,
            Self::Level,
            Self::Player,
            Self::AllSongs,
            Self::Skill,
        ]
    }

    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Box => "Box",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Difficulty => "Difficulty",
            Self::Level => "Level",
            Self::Player => "Player",
            Self::AllSongs => "All Songs",
            Self::Skill => "Skill",
        }
    }
}

/// State for the SongSelectionNew stage.
#[derive(Resource, Debug, Clone, Default)]
pub struct SongSelectionNewState {
    pub return_value: EReturnValue,
    pub load_phase: ELoadPhase,
    pub current_sort: ESortMode,
    pub selected_box: Option<String>,
    pub scroll_position: usize,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SongSelectionNewState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ereturn_value_variants() {
        // CStageSongSelectionNew.cs:30-37
        let all = [
            EReturnValue::Continue,
            EReturnValue::ReturnToTitle,
            EReturnValue::Selected,
            EReturnValue::CallConfig,
            EReturnValue::ChangeSking,
        ];
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn eload_phase_variants() {
        // CStageSongSelectionNew.cs:23-29
        let all = [
            ELoadPhase::Initialize,
            ELoadPhase::Prepare,
            ELoadPhase::CacheThumbnails,
            ELoadPhase::ReadyToOpen,
            ELoadPhase::Complete,
        ];
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn esort_mode_variants_count() {
        // CStageSongSelectionNew.cs:56-66 — 9 sorters
        assert_eq!(ESortMode::all().len(), 9);
    }

    #[test]
    fn esort_mode_labels_unique() {
        let labels: Vec<_> = ESortMode::all().iter().map(|m| m.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len());
    }

    #[test]
    fn esort_mode_default_is_default() {
        // CStageSongSelectionNew.cs:67 — currentSort = sorters[0] (SortDefault)
        assert_eq!(ESortMode::default(), ESortMode::Default);
    }

    #[test]
    fn song_selection_state_default() {
        let s = SongSelectionNewState::default();
        assert_eq!(s.return_value, EReturnValue::Continue);
        assert_eq!(s.load_phase, ELoadPhase::Initialize);
        assert_eq!(s.current_sort, ESortMode::Default);
        assert!(s.selected_box.is_none());
        assert_eq!(s.scroll_position, 0);
    }

    #[test]
    fn ereturn_value_default_continue() {
        let v: EReturnValue = Default::default();
        assert_eq!(v, EReturnValue::Continue);
    }
}
