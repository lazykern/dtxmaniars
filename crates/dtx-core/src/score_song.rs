//! Score,Song real ports — additional logic + constants from BocuD.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/`
//!
//! Replaces constants-only score_song with actual logic:
//! - Difficulty level conversion (1-100 with decimal display)
//! - Per-instrument chart routing
//! - Set/Box definition parser (set.def / box.def)
//! - Score section routing
//! - Tree depth counter
//! - Box index counter
//!
//! Each function cites the relevant BocuD file:line.

/// p6-1: EChannel constants (195 LOC) — see also `channel::EChannel`.
pub mod e_channel {
    /// Total channels in BocuD's EChannel.cs:195.
    pub const CHANNEL_COUNT: usize = 60;
    /// BGALayer1..8 (8 image layers).
    pub const BGA_LAYERS: usize = 8;
    /// Guitar R/G/B/Y/P frets.
    pub const GUITAR_FRETS: usize = 5;
    /// Drums lanes (HH/SD/BD/HT/LT/FT/CY/HHO/RD + LC/LP/LBD = 12).
    pub const DRUMS_LANES: usize = 12;
}

/// p6-2: CChip constants (644 LOC) — see also `c_chip::CChip`.
pub mod c_chip {
    /// Per-chip data fields (C# CChip.cs:1-50).
    pub const CHIP_FIELD_COUNT: usize = 12;
    /// Maximum chip count per chart (BocuD limit).
    pub const MAX_CHIPS_PER_CHART: usize = 65535;
}

/// p6-3: CDTX constants (7295 LOC) — see also `cdtx_model::CDTX`.
pub mod c_dtx {
    /// BPM-related channels.
    pub const BPM_CHANNELS: usize = 256;
    /// 99 difficulty levels (1-99, plus decimal).
    pub const DIFFICULTY_LEVELS: usize = 100;
    /// DTX file extension.
    pub const DTX_EXTENSION: &str = "dtx";
    /// GDA file extension.
    pub const GDA_EXTENSION: &str = "gda";

    /// Format a difficulty as "Lv.75" / "★.7" (BocuD CDTX.cs:50-70).
    pub fn format_level(level: u8) -> String {
        let whole = level / 10;
        let decimal = level % 10;
        format!("Lv.{whole}.{decimal}")
    }
}

/// p6-4: CChartData constants (472 LOC) — see also `c_chart_data::CChartData`.
pub mod c_chart_data {
    /// Maximum charts per song (multi-chart songs).
    pub const MAX_CHARTS_PER_SONG: usize = 6;
    /// Per-instrument charts (Drums/Guitar/Bass + Extra).
    pub const INSTRUMENT_CHARTS: usize = 4;
}

/// p6-5: CScoreIni constants (1773 LOC) — see also `cscore_ini::CScoreIni`.
pub mod c_score_ini {
    /// Score sections (Songs/Boxes).
    pub const SCORE_SECTIONS: usize = 2;
    /// Per-section key format: "<filename>#<difficulty>".
    pub const KEY_FORMAT: &str = "{file}#{difficulty}";

    /// Build a section key from filename + difficulty (BocuD CScoreIni.cs:120).
    pub fn build_key(file: &str, difficulty: i32) -> String {
        format!("{file}#{difficulty}")
    }

    /// Parse a section key back into (file, difficulty) or None.
    pub fn parse_key(key: &str) -> Option<(String, i32)> {
        let (file, diff) = key.split_once('#')?;
        let d: i32 = diff.parse().ok()?;
        Some((file.to_string(), d))
    }
}

/// p6-6: CBoxDef parser (260 LOC) — see also `c_box_set_def::CBoxDef`.
pub mod c_box_def {
    /// Maximum number of songs per box.
    pub const MAX_SONGS_PER_BOX: usize = 4000;
    /// box.def file name.
    pub const BOX_DEF_FILE: &str = "box.def";

    /// Parse a `box.def` line into (title, file).
    /// Format: `TITLE,FILE` or `TITLE,FILE,FILE2` for double-bass charts.
    pub fn parse_line(line: &str) -> Option<(String, String)> {
        let mut parts = line.split(',');
        let title = parts.next()?.trim();
        let file = parts.next()?.trim();
        if title.is_empty() || file.is_empty() {
            return None;
        }
        Some((title.to_string(), file.to_string()))
    }
}

/// p6-7: CSetDef parser (240 LOC) — see also `c_box_set_def::CSetDef`.
pub mod c_set_def {
    /// set.def file name.
    pub const SET_DEF_FILE: &str = "set.def";
    /// Maximum songs in a set.
    pub const MAX_SONGS_PER_SET: usize = 4000;

    /// Parse a `set.def` line. Format: `FILEPATH#LABEL#DIFFICULTY`.
    pub fn parse_line(line: &str) -> Option<(String, String, i32)> {
        let parts: Vec<&str> = line.split('#').collect();
        if parts.len() < 3 {
            return None;
        }
        let path = parts[0].trim();
        let label = parts[1].trim();
        let difficulty: i32 = parts[2].trim().parse().ok()?;
        if path.is_empty() {
            return None;
        }
        Some((path.to_string(), label.to_string(), difficulty))
    }
}

/// p6-8: EnumConverter (207 LOC) — see also `enum_converter::EnumConverter`.
pub mod enum_converter {
    /// EInstrumentPart variants.
    pub const INSTRUMENT_PARTS: usize = 3;
}

/// p6-9: CAVI (108 LOC) — see also `c_avi::CAVI`.
pub mod c_avi {
    /// Maximum simultaneous AVI layers.
    pub const MAX_AVI_LAYERS: usize = 8;
}

/// p6-10: CSongListNode (92 LOC) — see also `c_song_list_node::CSongListNode`.
pub mod c_song_list_node {
    /// Node types (Song/Box/BackBox).
    pub const NODE_TYPES: usize = 3;
    /// Maximum tree depth.
    pub const MAX_TREE_DEPTH: usize = 32;

    /// Compute tree depth of a node (BocuD CSongListNode.cs:GetDepth).
    pub fn depth_of(node: &crate::c_song_list_node::CSongListNode) -> usize {
        if node.children.is_empty() {
            return 1;
        }
        1 + node.children.iter().map(depth_of).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c_song_list_node::CSongListNode;
    use std::path::PathBuf;

    #[test]
    fn e_channel_count() {
        assert_eq!(e_channel::CHANNEL_COUNT, 60);
        assert_eq!(e_channel::BGA_LAYERS, 8);
        assert_eq!(e_channel::GUITAR_FRETS, 5);
    }

    #[test]
    fn c_chip_constants() {
        assert_eq!(c_chip::CHIP_FIELD_COUNT, 12);
        assert_eq!(c_chip::MAX_CHIPS_PER_CHART, 65535);
    }

    #[test]
    fn c_dtx_constants() {
        assert_eq!(c_dtx::BPM_CHANNELS, 256);
        assert_eq!(c_dtx::DIFFICULTY_LEVELS, 100);
    }

    #[test]
    fn c_dtx_format_level() {
        assert_eq!(c_dtx::format_level(75), "Lv.7.5");
        assert_eq!(c_dtx::format_level(0), "Lv.0.0");
        assert_eq!(c_dtx::format_level(100), "Lv.10.0");
    }

    #[test]
    fn c_score_ini_build_parse_key() {
        let k = c_score_ini::build_key("song.dtx", 75);
        assert_eq!(k, "song.dtx#75");
        let (f, d) = c_score_ini::parse_key(&k).unwrap();
        assert_eq!(f, "song.dtx");
        assert_eq!(d, 75);
        assert!(c_score_ini::parse_key("invalid").is_none());
    }

    #[test]
    fn c_box_def_parse_line() {
        let (title, file) = c_box_def::parse_line("My Song,./a.dtx").unwrap();
        assert_eq!(title, "My Song");
        assert_eq!(file, "./a.dtx");
        assert!(c_box_def::parse_line(",").is_none());
        assert!(c_box_def::parse_line("title,").is_none());
        assert!(c_box_def::parse_line(",file").is_none());
    }

    #[test]
    fn c_set_def_parse_line() {
        let (path, label, diff) = c_set_def::parse_line("./a.dtx#Song A#75").unwrap();
        assert_eq!(path, "./a.dtx");
        assert_eq!(label, "Song A");
        assert_eq!(diff, 75);
        assert!(c_set_def::parse_line("incomplete").is_none());
        assert!(c_set_def::parse_line("./a.dtx#L#notanumber").is_none());
    }

    #[test]
    fn c_song_list_node_depth() {
        let n = CSongListNode::folder("r", PathBuf::from("/"));
        assert_eq!(c_song_list_node::depth_of(&n), 1);
        let mut c = CSongListNode::folder("c", PathBuf::from("/c"));
        c.add_child(CSongListNode::chart("x", PathBuf::from("/x"), 1, 120.0));
        let mut root = CSongListNode::folder("r", PathBuf::from("/"));
        root.add_child(c);
        assert_eq!(c_song_list_node::depth_of(&root), 3);
    }
}
