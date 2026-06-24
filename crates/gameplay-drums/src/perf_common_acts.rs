//! Common Performance sub-acts — batched port of CActPerfCommon*.cs files
//! (p3-2..p3-10). Each is a small sub-act; constants verbatim from reference.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerf*.cs`

/// Common score — CActPerfCommonScore.cs:142 LOC.
pub mod common_score {
    /// Position (CActPerfCommonScore.cs).
    pub const SCORE_X: f32 = 40.0;
    pub const SCORE_Y: f32 = 13.0;
    pub const SCORE_DIGITS: usize = 7;
}

/// Common combo — CActPerfCommonCombo.cs:794 LOC.
pub mod common_combo {
    pub const COMBO_X: f32 = 1245.0;
    pub const COMBO_Y: f32 = 60.0;
    pub const COMBO_BOMB_X: f32 = 845.0;
    pub const COMBO_BOMB_Y: f32 = -130.0;
}

/// Common gauge — CActPerfCommonGauge.cs:296 LOC.
pub mod common_gauge {
    pub const GAUGE_X_DRUMS: f32 = 294.0;
    pub const GAUGE_Y_NORMAL: f32 = 626.0;
    pub const GAUGE_Y_REVERSE: f32 = 28.0;
    pub const GAUGE_FRAME_H: f32 = 47.0;
    pub const GAUGE_BAR_H: f32 = 31.0;
    /// 5 difficulty brackets (CActPerfCommonGauge.cs).
    pub const GAUGE_BRACKETS: usize = 5;
    /// 2 fill modes (normal, risky).
    pub const GAUGE_FILL_MODES: usize = 2;
}

/// Common lane flush GB — CActPerfCommonLaneFlushGB.cs:70 LOC.
pub mod common_lane_flush {
    /// GB = Guitar/Bass lanes (5 lanes).
    pub const GB_LANES: usize = 5;
    /// Flush counter max (CActPerfCommonLaneFlushGB.cs).
    pub const FLUSH_COUNTER_MAX: u32 = 90;
}

/// Common status panel — CActPerfCommonStatusPanel.cs:531 LOC.
pub mod common_status {
    /// Position (CActPerfCommonStatusPanel.cs:22).
    pub const STATUS_PANEL_X: f32 = 22.0;
    pub const STATUS_PANEL_Y: f32 = 250.0;
    pub const STATUS_PANEL_ROWS: usize = 6;
}

/// Common judgement string — CActPerfCommonJudgementString.cs:301 LOC.
pub mod common_judgement {
    pub const JUDGE_STRING_Y_FORWARD: f32 = 348.0;
    pub const JUDGE_STRING_Y_REVERSE: f32 = 583.0;
    pub const JUDGE_STRING_Y_TOP_REVERSE: f32 = 80.0;
    pub const JUDGE_STRING_VERT_DX: f32 = 32.0;
}

/// Common danger overlay — CActPerfCommonDanger.cs:57 LOC.
pub mod common_danger {
    /// Full-screen overlay (CActPerfCommonDanger.cs:43-44).
    pub const DANGER_X: f32 = 0.0;
    pub const DANGER_Y: f32 = 0.0;
    pub const DANGER_W: f32 = 1280.0;
    pub const DANGER_H: f32 = 720.0;
    /// Animation counter max.
    pub const DANGER_COUNTER_MAX: u32 = 0x7f;
}

/// Common wailing bonus — CActPerfCommonWailingBonus.cs:43 LOC.
pub mod common_wailing {
    pub const WAILING_BONUS_X: f32 = 1245.0;
    pub const WAILING_BONUS_Y: f32 = 200.0;
}

/// Common RGB — CActPerfCommonRGB.cs:59 LOC.
pub mod common_rgb {
    /// R/G/B note colors (CActPerfCommonRGB.cs).
    pub const RGB_RED: [u8; 4] = [255, 0, 0, 255];
    pub const RGB_GREEN: [u8; 4] = [0, 255, 0, 255];
    pub const RGB_BLUE: [u8; 4] = [0, 0, 255, 255];
    pub const RGB_YELLOW: [u8; 4] = [255, 255, 0, 255];
    pub const RGB_PURPLE: [u8; 4] = [128, 0, 128, 255];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_score_position() {
        assert_eq!(common_score::SCORE_X, 40.0);
        assert_eq!(common_score::SCORE_Y, 13.0);
        assert_eq!(common_score::SCORE_DIGITS, 7);
    }

    #[test]
    fn common_combo_position() {
        assert_eq!(common_combo::COMBO_X, 1245.0);
        assert_eq!(common_combo::COMBO_Y, 60.0);
    }

    #[test]
    fn common_gauge_position_drums() {
        assert_eq!(common_gauge::GAUGE_X_DRUMS, 294.0);
        assert_eq!(common_gauge::GAUGE_Y_NORMAL, 626.0);
        assert_eq!(common_gauge::GAUGE_Y_REVERSE, 28.0);
    }

    #[test]
    fn common_gauge_brackets() {
        assert_eq!(common_gauge::GAUGE_BRACKETS, 5);
        assert_eq!(common_gauge::GAUGE_FILL_MODES, 2);
    }

    #[test]
    fn common_lane_flush_gb_lanes() {
        assert_eq!(common_lane_flush::GB_LANES, 5);
        assert_eq!(common_lane_flush::FLUSH_COUNTER_MAX, 90);
    }

    #[test]
    fn common_status_panel_position() {
        assert_eq!(common_status::STATUS_PANEL_X, 22.0);
        assert_eq!(common_status::STATUS_PANEL_Y, 250.0);
        assert_eq!(common_status::STATUS_PANEL_ROWS, 6);
    }

    #[test]
    fn common_judgement_y_positions() {
        assert_eq!(common_judgement::JUDGE_STRING_Y_FORWARD, 348.0);
        assert_eq!(common_judgement::JUDGE_STRING_Y_REVERSE, 583.0);
    }

    #[test]
    fn common_danger_overlay_fullscreen() {
        assert_eq!(common_danger::DANGER_W, 1280.0);
        assert_eq!(common_danger::DANGER_H, 720.0);
        assert_eq!(common_danger::DANGER_COUNTER_MAX, 0x7f);
    }

    #[test]
    fn common_wailing_bonus_position() {
        assert_eq!(common_wailing::WAILING_BONUS_X, 1245.0);
    }

    #[test]
    fn common_rgb_colors_distinct() {
        let colors = [
            common_rgb::RGB_RED,
            common_rgb::RGB_GREEN,
            common_rgb::RGB_BLUE,
            common_rgb::RGB_YELLOW,
            common_rgb::RGB_PURPLE,
        ];
        let unique: std::collections::HashSet<_> = colors.iter().collect();
        assert_eq!(unique.len(), 5);
    }
}
