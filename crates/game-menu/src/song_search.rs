#![allow(missing_docs)]
//! `SongSearchMenu` + `CommandHistory` — port of
//! `Stage/04.SongSelectionNew/SongSearchMenu.cs` (107 LOC) + `CommandHistory.cs` (99 LOC).
//!
//! Strict-port-first.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/SongSearchMenu.cs:1-107`
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CommandHistory.cs:1-99`

use super::status_panel::EInstrumentPart;

/// Song search menu size (SongSearchMenu.cs:36-37).
pub const SONG_SEARCH_W: f32 = 500.0;
pub const SONG_SEARCH_H: f32 = 300.0;
/// Header text font size (SongSearchMenu.cs:9).
pub const SONG_SEARCH_HEADER_PT: u32 = 28;
/// Text input font size (SongSearchMenu.cs:14).
pub const SONG_SEARCH_INPUT_PT: u32 = 25;
/// Status text font size (SongSearchMenu.cs:30).
pub const SONG_SEARCH_STATUS_PT: u32 = 18;
/// Header Y position (SongSearchMenu.cs:9 — name only, y=0).
/// Text input Y (SongSearchMenu.cs:14).
pub const SONG_SEARCH_INPUT_Y: f32 = 30.0;
/// Description Y (SongSearchMenu.cs:21).
pub const SONG_SEARCH_DESC_Y: f32 = 60.0;
/// Status Y (SongSearchMenu.cs:30).
pub const SONG_SEARCH_STATUS_Y: f32 = 250.0;

/// Song search menu state.
#[derive(Debug, Clone, Default)]
pub struct SongSearchMenu {
    pub query: String,
    pub status: String,
    pub is_active: bool,
}

impl SongSearchMenu {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            status: String::new(),
            is_active: false,
        }
    }

    /// Set search query (called by text input).
    pub fn set_query(&mut self, q: &str) {
        self.query = q.to_string();
    }

    /// Update status text (e.g. "12 matches found").
    pub fn set_status(&mut self, status: &str) {
        self.status = status.to_string();
    }
}

/// Pad flag bits (CommandHistory.cs EPadFlag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EPadFlag(pub u32);

impl EPadFlag {
    pub const NONE: EPadFlag = EPadFlag(0);
    pub const HH: EPadFlag = EPadFlag(1 << 0);
    pub const SD: EPadFlag = EPadFlag(1 << 1);
    pub const BD: EPadFlag = EPadFlag(1 << 2);
    pub const HT: EPadFlag = EPadFlag(1 << 3);
    pub const LT: EPadFlag = EPadFlag(1 << 4);
    pub const FT: EPadFlag = EPadFlag(1 << 5);
    pub const CY: EPadFlag = EPadFlag(1 << 6);
    pub const HHO: EPadFlag = EPadFlag(1 << 7);
    pub const RD: EPadFlag = EPadFlag(1 << 8);
    pub const LC: EPadFlag = EPadFlag(1 << 9);
    pub const LP: EPadFlag = EPadFlag(1 << 10);

    /// True if any of `other` bits are set in self.
    pub fn contains(&self, other: EPadFlag) -> bool {
        (self.0 & other.0) != 0
    }

    /// OR with another flag set.
    pub fn or(self, other: EPadFlag) -> EPadFlag {
        EPadFlag(self.0 | other.0)
    }
}

/// One command history entry (CommandHistory.cs:8-13).
#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub instrument: EInstrumentPart,
    pub pad: EPadFlag,
    pub time_ms: i64,
}

/// Command history (CommandHistory.cs:15 — buffer of 16).
#[derive(Debug, Clone)]
pub struct CommandHistory {
    /// Buffer size (CommandHistory.cs:16).
    pub buffer_size: usize,
    pub entries: Vec<CommandEntry>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self::with_size(16)
    }

    pub fn with_size(n: usize) -> Self {
        Self {
            buffer_size: n,
            entries: Vec::with_capacity(n),
        }
    }

    /// Add an entry, evicting oldest if at capacity (CommandHistory.cs:37-50).
    pub fn add(&mut self, instrument: EInstrumentPart, pad: EPadFlag, time_ms: i64) {
        if self.entries.len() >= self.buffer_size {
            self.entries.remove(0);
        }
        self.entries.push(CommandEntry {
            instrument,
            pad,
            time_ms,
        });
    }

    /// Remove entry at index (CommandHistory.cs:55-58).
    pub fn remove_at(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_search_size_matches_reference() {
        // SongSearchMenu.cs:36-37
        assert_eq!(SONG_SEARCH_W, 500.0);
        assert_eq!(SONG_SEARCH_H, 300.0);
    }

    #[test]
    fn song_search_font_sizes() {
        // SongSearchMenu.cs:9, 14, 30
        assert_eq!(SONG_SEARCH_HEADER_PT, 28);
        assert_eq!(SONG_SEARCH_INPUT_PT, 25);
        assert_eq!(SONG_SEARCH_STATUS_PT, 18);
    }

    #[test]
    fn song_search_positions() {
        // SongSearchMenu.cs:14, 21, 30
        assert_eq!(SONG_SEARCH_INPUT_Y, 30.0);
        assert_eq!(SONG_SEARCH_DESC_Y, 60.0);
        assert_eq!(SONG_SEARCH_STATUS_Y, 250.0);
    }

    #[test]
    fn song_search_default_empty() {
        let m = SongSearchMenu::new();
        assert!(m.query.is_empty());
        assert!(m.status.is_empty());
        assert!(!m.is_active);
    }

    #[test]
    fn song_search_set_query() {
        let mut m = SongSearchMenu::new();
        m.set_query("Beat");
        assert_eq!(m.query, "Beat");
    }

    #[test]
    fn song_search_set_status() {
        let mut m = SongSearchMenu::new();
        m.set_status("3 matches");
        assert_eq!(m.status, "3 matches");
    }

    #[test]
    fn command_history_default_buffer_16() {
        // CommandHistory.cs:16 — buffersize = 16
        let h = CommandHistory::new();
        assert_eq!(h.buffer_size, 16);
    }

    #[test]
    fn command_history_add_increments() {
        let mut h = CommandHistory::new();
        h.add(EInstrumentPart::Drums, EPadFlag::HH, 1000);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn command_history_evicts_oldest_at_capacity() {
        // CommandHistory.cs:42-45
        let mut h = CommandHistory::new();
        for i in 0..16 {
            h.add(EInstrumentPart::Drums, EPadFlag::HH, i as i64);
        }
        assert_eq!(h.len(), 16);
        h.add(EInstrumentPart::Drums, EPadFlag::SD, 16);
        assert_eq!(h.len(), 16);
        // Oldest (time_ms=0) should be gone, newest (16) is last.
        assert_eq!(h.entries[0].time_ms, 1);
        assert_eq!(h.entries[15].time_ms, 16);
    }

    #[test]
    fn command_history_remove_at() {
        let mut h = CommandHistory::new();
        h.add(EInstrumentPart::Drums, EPadFlag::HH, 1000);
        h.add(EInstrumentPart::Drums, EPadFlag::SD, 2000);
        h.remove_at(0);
        assert_eq!(h.len(), 1);
        assert_eq!(h.entries[0].pad, EPadFlag::SD);
    }

    #[test]
    fn epad_flag_or_combines() {
        let a = EPadFlag::HH;
        let b = EPadFlag::SD;
        let c = a.or(b);
        assert!(c.contains(EPadFlag::HH));
        assert!(c.contains(EPadFlag::SD));
    }

    #[test]
    fn epad_flag_contains_check() {
        let c = EPadFlag::HH.or(EPadFlag::SD);
        assert!(c.contains(EPadFlag::HH));
        assert!(!c.contains(EPadFlag::BD));
    }
}
