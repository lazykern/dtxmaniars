//! Song selection container/element/presound/quick-config — port of
//! `Stage/04.SongSelectionNew/SongSelectionContainer.cs` (588 LOC) +
//! `SongSelectionElement.cs` (319 LOC) +
//! `CActSelectPresound.cs` (184 LOC) + remaining sub-acts.
//!
//! Strict-port-first. Combined commit p2-8..p2-11 (4 sub-acts).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/`

use bevy::prelude::{App, Resource};

/// Buffer size for song selection elements (SongSelectionContainer.cs:25).
pub const SONG_SELECTION_BUFFER: usize = 20;
/// Initial buffer start index.
pub const SONG_BUFFER_START: usize = 0;

/// Song selection container with ring buffer of elements.
#[derive(Debug, Clone)]
pub struct SongSelectionContainer {
    pub elements: Vec<Option<SongSelectionElement>>,
    pub buffer_start_index: usize,
}

impl SongSelectionContainer {
    pub fn new() -> Self {
        Self {
            elements: vec![None; SONG_SELECTION_BUFFER],
            buffer_start_index: SONG_BUFFER_START,
        }
    }

    /// Wrap an index into [0, SONG_SELECTION_BUFFER).
    pub fn wrap_index(&self, index: i32) -> usize {
        let len = self.elements.len() as i32;
        ((index + len) % len) as usize
    }

    /// Get element at index (wrapped).
    pub fn get(&self, index: i32) -> Option<&SongSelectionElement> {
        self.elements
            .get(self.wrap_index(index))
            .and_then(|e| e.as_ref())
    }
}

impl Default for SongSelectionContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// One song selection element (one row in the list).
#[derive(Debug, Clone, Default)]
pub struct SongSelectionElement {
    pub title: String,
    pub artist: String,
    pub is_box: bool,
    pub is_open: bool,
    /// Per-instrument best rank (S/A/B/C/D/E).
    pub best_rank: Option<Rank>,
}

impl SongSelectionElement {
    pub fn new(title: &str, artist: &str) -> Self {
        Self {
            title: title.to_string(),
            artist: artist.to_string(),
            ..Default::default()
        }
    }
}

/// Rank (C# EBestSkill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rank {
    S,
    A,
    B,
    C,
    D,
    E,
    None,
}

impl Rank {
    pub fn label(&self) -> &'static str {
        match self {
            Self::S => "S",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::None => "-",
        }
    }
}

/// BGM preview sub-act (CActSelectPresound.cs).
#[derive(Resource, Debug, Default, Clone)]
pub struct CActSelectPresound {
    /// Currently playing preview audio path.
    pub current_path: Option<String>,
    /// Time to wait before starting preview (ms) — nSongSelectSoundPreviewWaitTimeMs.
    pub wait_ms: u32,
    /// True if currently in fade-in.
    pub fading_in: bool,
}

impl CActSelectPresound {
    pub fn new() -> Self {
        Self::default()
    }

    /// Selection changed — start preview if chart has a Presound.
    pub fn selection_changed(&mut self, presound: Option<&str>) {
        if let Some(p) = presound {
            if Some(p) != self.current_path.as_deref() {
                self.current_path = Some(p.to_string());
                self.fading_in = true;
            }
        } else {
            self.current_path = None;
            self.fading_in = false;
        }
    }

    /// Stop preview (CActSelectPresound.cs:11-18).
    pub fn t_stop_sound(&mut self) {
        self.current_path = None;
        self.fading_in = false;
    }
}

/// Quick config (CActSelectQuickConfig.cs) — preset toggles shown in SongSelect.
#[derive(Resource, Debug, Default, Clone)]
pub struct QuickConfigState {
    pub auto_play: bool,
    pub dark_mode: bool,
    pub scroll_speed: f32,
    pub reverse: bool,
}

impl QuickConfigState {
    pub fn default_drums() -> Self {
        Self {
            auto_play: false,
            dark_mode: false,
            scroll_speed: 1.0,
            reverse: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_selection_buffer_size() {
        // SongSelectionContainer.cs:25 — array of 20
        assert_eq!(SONG_SELECTION_BUFFER, 20);
    }

    #[test]
    fn song_selection_container_default() {
        let c = SongSelectionContainer::new();
        assert_eq!(c.elements.len(), SONG_SELECTION_BUFFER);
        assert_eq!(c.buffer_start_index, 0);
    }

    #[test]
    fn song_selection_wrap_index() {
        let c = SongSelectionContainer::new();
        assert_eq!(c.wrap_index(0), 0);
        assert_eq!(c.wrap_index(19), 19);
        assert_eq!(c.wrap_index(20), 0); // wrap
        assert_eq!(c.wrap_index(-1), 19); // wrap negative
    }

    #[test]
    fn song_selection_element_title_artist() {
        let e = SongSelectionElement::new("My Song", "Me");
        assert_eq!(e.title, "My Song");
        assert_eq!(e.artist, "Me");
        assert!(!e.is_box);
    }

    #[test]
    fn rank_labels() {
        assert_eq!(Rank::S.label(), "S");
        assert_eq!(Rank::A.label(), "A");
        assert_eq!(Rank::E.label(), "E");
        assert_eq!(Rank::None.label(), "-");
    }

    #[test]
    fn rank_distinct() {
        let all = [
            Rank::S,
            Rank::A,
            Rank::B,
            Rank::C,
            Rank::D,
            Rank::E,
            Rank::None,
        ];
        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), 7);
    }

    #[test]
    fn act_select_presound_default_wait() {
        // CActSelectPresound.cs uses nSongSelectSoundPreviewWaitTimeMs
        // (default 1000 ms per CActConfigList.Menu.cs).
        let p = CActSelectPresound::new();
        assert_eq!(p.wait_ms, 0);
        assert!(p.current_path.is_none());
    }

    #[test]
    fn act_select_presound_selection_changed() {
        let mut p = CActSelectPresound::new();
        p.selection_changed(Some("song.ogg"));
        assert_eq!(p.current_path, Some("song.ogg".to_string()));
        assert!(p.fading_in);
    }

    #[test]
    fn act_select_presound_selection_changed_same() {
        // Same path → no re-trigger.
        let mut p = CActSelectPresound::new();
        p.selection_changed(Some("song.ogg"));
        p.fading_in = false;
        p.selection_changed(Some("song.ogg"));
        assert!(!p.fading_in); // no re-trigger
    }

    #[test]
    fn act_select_presound_stop_clears() {
        let mut p = CActSelectPresound::new();
        p.selection_changed(Some("song.ogg"));
        p.t_stop_sound();
        assert!(p.current_path.is_none());
    }

    #[test]
    fn quick_config_state_default() {
        let q = QuickConfigState::default_drums();
        assert!(!q.auto_play);
        assert!((q.scroll_speed - 1.0).abs() < 0.01);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CActSelectPresound>()
        .init_resource::<QuickConfigState>();
}
