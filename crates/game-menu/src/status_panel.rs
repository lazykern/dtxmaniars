#![allow(missing_docs)]
//! `StatusPanel` — port of `Stage/04.SongSelectionNew/StatusPanel.cs` (143 LOC).
//!
//! Strict-port-first. Status panel showing drum/guitar/bass difficulty per row.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/StatusPanel.cs:1-143`

use bevy::prelude::Resource;

/// Instrument part (matches EInstrumentPart in BocuD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EInstrumentPart {
    Drums,
    Guitar,
    Bass,
}

impl EInstrumentPart {
    pub fn all() -> [Self; 3] {
        [Self::Drums, Self::Guitar, Self::Bass]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Drums => "Drums",
            Self::Guitar => "Guitar",
            Self::Bass => "Bass",
        }
    }
}

/// Per-pane state: the 5 skill/rank slots for one instrument.
#[derive(Debug, Clone, Default)]
pub struct StatusPane {
    pub instrument: Option<EInstrumentPart>,
    /// 5 skill-rating slots (Skill_0..Skill_4).
    pub skills: [bool; 5],
    /// 5 rank slots (Rank_0..Rank_4).
    pub ranks: [bool; 5],
    /// Is the pane currently visible.
    pub visible: bool,
}

impl StatusPane {
    pub fn new() -> Self {
        Self::default()
    }
}

/// State for the full StatusPanel (3 panes).
#[derive(Resource, Debug, Default, Clone)]
pub struct StatusPanelState {
    pub drums: StatusPane,
    pub guitar: StatusPane,
    pub bass: StatusPane,
}

impl StatusPanelState {
    /// StatusPanel.cs:5-23 — drums at (430, 720), guitar at (200, 720), bass at (430, 720).
    pub fn default_layout() -> Self {
        Self {
            drums: StatusPane {
                instrument: Some(EInstrumentPart::Drums),
                visible: true,
                ..Default::default()
            },
            guitar: StatusPane {
                instrument: Some(EInstrumentPart::Guitar),
                visible: false,
                ..Default::default()
            },
            bass: StatusPane {
                instrument: Some(EInstrumentPart::Bass),
                visible: false,
                ..Default::default()
            },
        }
    }

    /// Update visibility based on which mode is active.
    pub fn update_visibility(&mut self, drums_enabled: bool) {
        self.drums.visible = drums_enabled;
        self.guitar.visible = !drums_enabled;
        self.bass.visible = !drums_enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn einstrument_part_variants() {
        // 3 instruments (StatusPanel.cs:5-15).
        assert_eq!(EInstrumentPart::all().len(), 3);
    }

    #[test]
    fn einstrument_part_labels_unique() {
        let labels: Vec<_> = EInstrumentPart::all().iter().map(|p| p.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len());
    }

    #[test]
    fn status_pane_default() {
        let p = StatusPane::new();
        assert_eq!(p.skills, [false; 5]);
        assert_eq!(p.ranks, [false; 5]);
        assert!(!p.visible);
    }

    #[test]
    fn status_panel_state_default_layout() {
        // StatusPanel.cs:6-15
        let s = StatusPanelState::default_layout();
        assert_eq!(s.drums.instrument, Some(EInstrumentPart::Drums));
        assert!(s.drums.visible);
        assert!(!s.guitar.visible);
        assert!(!s.bass.visible);
    }

    #[test]
    fn status_panel_visibility_drums_mode() {
        // StatusPanel.cs:34-37
        let mut s = StatusPanelState::default_layout();
        s.update_visibility(true);
        assert!(s.drums.visible);
        assert!(!s.guitar.visible);
    }

    #[test]
    fn status_panel_visibility_guitar_mode() {
        let mut s = StatusPanelState::default_layout();
        s.update_visibility(false);
        assert!(!s.drums.visible);
        assert!(s.guitar.visible);
    }
}
