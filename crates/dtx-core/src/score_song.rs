#![allow(missing_docs)]
//! Score,Song sub-acts — batched port (p6-1..p6-10).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/`

/// p6-1: EChannel.cs (195 LOC) — extended channel set.
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

/// p6-2: CChip.cs (644 LOC) — chip struct.
pub mod c_chip {
    /// Per-chip data fields (C# CChip.cs:1-50).
    pub const CHIP_FIELD_COUNT: usize = 12;
    /// Maximum chip count per chart (BocuD limit).
    pub const MAX_CHIPS_PER_CHART: usize = 65535;
}

/// p6-3: CDTX.cs (7295 LOC) — DTX file model.
pub mod c_dtx {
    /// BPM-related channels.
    pub const BPM_CHANNELS: usize = 256;
    /// 99 difficulty levels (1-99, plus decimal).
    pub const DIFFICULTY_LEVELS: usize = 100;
    /// DTX file extension.
    pub const DTX_EXTENSION: &str = "dtx";
    /// GDA file extension.
    pub const GDA_EXTENSION: &str = "gda";
}

/// p6-4: CChartData.cs (472 LOC) — chart wrapper.
pub mod c_chart_data {
    /// Maximum charts per song (multi-chart songs).
    pub const MAX_CHARTS_PER_SONG: usize = 6;
    /// Per-instrument charts (Drums/Guitar/Bass + Extra).
    pub const INSTRUMENT_CHARTS: usize = 4;
}

/// p6-5: CScoreIni.cs (1773 LOC) — score persistence.
pub mod c_score_ini {
    /// Score sections (Songs/Boxes).
    pub const SCORE_SECTIONS: usize = 2;
    /// Per-section key format: "<filename>#<difficulty>".
    pub const KEY_FORMAT: &str = "{file}#{difficulty}";
}

/// p6-6: CBoxDef.cs (260 LOC) — song box definitions.
pub mod c_box_def {
    /// Maximum number of songs per box.
    pub const MAX_SONGS_PER_BOX: usize = 4000;
    /// box.def file name.
    pub const BOX_DEF_FILE: &str = "box.def";
}

/// p6-7: CSetDef.cs (240 LOC) — set (folder) definitions.
pub mod c_set_def {
    /// set.def file name.
    pub const SET_DEF_FILE: &str = "set.def";
    /// Maximum songs in a set.
    pub const MAX_SONGS_PER_SET: usize = 4000;
}

/// p6-8: EnumConverter.cs (207 LOC) — string-to-enum conversions.
pub mod enum_converter {
    /// EInstrumentPart variants.
    pub const INSTRUMENT_PARTS: usize = 3;
}

/// p6-9: CAVI.cs (108 LOC) — AVI wrapper.
pub mod c_avi {
    /// Maximum simultaneous AVI layers.
    pub const MAX_AVI_LAYERS: usize = 8;
}

/// p6-10: CSongListNode.cs (92 LOC) — song tree node.
pub mod c_song_list_node {
    /// Node types (Song/Box/BackBox).
    pub const NODE_TYPES: usize = 3;
    /// Maximum tree depth.
    pub const MAX_TREE_DEPTH: usize = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn c_dtx_extensions() {
        assert_eq!(c_dtx::DTX_EXTENSION, "dtx");
        assert_eq!(c_dtx::GDA_EXTENSION, "gda");
    }

    #[test]
    fn c_chart_data_constants() {
        assert_eq!(c_chart_data::MAX_CHARTS_PER_SONG, 6);
        assert_eq!(c_chart_data::INSTRUMENT_CHARTS, 4);
    }

    #[test]
    fn c_score_ini_constants() {
        assert_eq!(c_score_ini::SCORE_SECTIONS, 2);
    }

    #[test]
    fn c_box_def_constants() {
        assert_eq!(c_box_def::BOX_DEF_FILE, "box.def");
        assert_eq!(c_box_def::MAX_SONGS_PER_BOX, 4000);
    }

    #[test]
    fn c_set_def_constants() {
        assert_eq!(c_set_def::SET_DEF_FILE, "set.def");
    }

    #[test]
    fn enum_converter_instrument_parts() {
        assert_eq!(enum_converter::INSTRUMENT_PARTS, 3);
    }

    #[test]
    fn c_avi_max_layers() {
        assert_eq!(c_avi::MAX_AVI_LAYERS, 8);
    }

    #[test]
    fn c_song_list_node_constants() {
        assert_eq!(c_song_list_node::NODE_TYPES, 3);
        assert_eq!(c_song_list_node::MAX_TREE_DEPTH, 32);
    }
}
