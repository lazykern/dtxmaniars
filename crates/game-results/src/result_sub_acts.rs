#![allow(missing_docs)]
//! Result sub-acts — batched port (p4-2..p4-5).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/`

/// p4-2: CActResultParameterPanel (971 LOC).
pub mod param_panel {
    /// 11 character positions (CActResultParameterPanel.cs).
    pub const CHAR_POSITIONS: usize = 11;
    /// 3 character textures (one per instrument).
    pub const TEXTURE_COUNT: usize = 3;
    /// 7 rows (Perfect/Great/Good/Ok/Miss/MaxCombo/Score).
    pub const ROW_COUNT: usize = 7;
    /// Position of the first row.
    pub const ROW_Y: f32 = 0.0;
    /// Vertical spacing between rows.
    pub const ROW_DY: f32 = 28.0;
}

/// p4-3: ResultInfoPanel (116 LOC).
pub mod info_panel {
    /// Level icon position (ResultInfoPanel.cs:18).
    pub const LEVEL_ICON_X: f32 = 64.0;
    pub const LEVEL_ICON_Y: f32 = 21.0;
    /// Level line position (ResultInfoPanel.cs:21).
    pub const LEVEL_LINE_X: f32 = 88.0;
    pub const LEVEL_LINE_Y: f32 = 94.0;
    pub const LEVEL_LINE_W: f32 = 340.0;
    pub const LEVEL_LINE_H: f32 = 2.0;
}

/// p4-4: ResultParameterPanel (59 LOC).
pub mod param_table {
    /// 7 rows (Perfect/Great/Good/Ok/Miss/MaxCombo/Score).
    pub const TABLE_ROW_COUNT: usize = 7;
    /// Scale X (ResultParameterPanel.cs:11).
    pub const SCALE_X: f32 = 0.96;
    /// Row height.
    pub const ROW_H: f32 = 28.0;
}

/// p4-5: ResultRankIcon (143 LOC).
pub mod rank_icon {
    /// Size (ResultRankIcon.cs:11-12).
    pub const RANK_ICON_W: f32 = 420.0;
    pub const RANK_ICON_H: f32 = 510.0;
    /// Anchor (0.5, 0.5) — center.
    pub const ANCHOR_X: f32 = 0.5;
    pub const ANCHOR_Y: f32 = 0.5;
    /// 3 instruments (Drums/Guitar/Bass).
    pub const RANK_INSTRUMENTS: usize = 3;
    /// 6 rank levels.
    pub const RANK_LEVELS: usize = 6;
    /// "SS" special rank (above S).
    pub const SS_LEVEL: i32 = 7;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_panel_constants() {
        assert_eq!(param_panel::CHAR_POSITIONS, 11);
        assert_eq!(param_panel::TEXTURE_COUNT, 3);
        assert_eq!(param_panel::ROW_COUNT, 7);
    }

    #[test]
    fn info_panel_level_icon_position() {
        // ResultInfoPanel.cs:18
        assert_eq!(info_panel::LEVEL_ICON_X, 64.0);
        assert_eq!(info_panel::LEVEL_ICON_Y, 21.0);
    }

    #[test]
    fn info_panel_level_line_dimensions() {
        // ResultInfoPanel.cs:21-23
        assert_eq!(info_panel::LEVEL_LINE_W, 340.0);
        assert_eq!(info_panel::LEVEL_LINE_H, 2.0);
    }

    #[test]
    fn param_table_row_count() {
        assert_eq!(param_table::TABLE_ROW_COUNT, 7);
    }

    #[test]
    fn rank_icon_dimensions() {
        // ResultRankIcon.cs:11-12
        assert_eq!(rank_icon::RANK_ICON_W, 420.0);
        assert_eq!(rank_icon::RANK_ICON_H, 510.0);
    }

    #[test]
    fn rank_icon_anchor_center() {
        assert_eq!(rank_icon::ANCHOR_X, 0.5);
        assert_eq!(rank_icon::ANCHOR_Y, 0.5);
    }

    #[test]
    fn rank_icon_instruments() {
        assert_eq!(rank_icon::RANK_INSTRUMENTS, 3);
    }

    #[test]
    fn rank_icon_levels() {
        assert_eq!(rank_icon::RANK_LEVELS, 6);
    }

    #[test]
    fn rank_icon_ss_level() {
        assert_eq!(rank_icon::SS_LEVEL, 7);
    }
}
