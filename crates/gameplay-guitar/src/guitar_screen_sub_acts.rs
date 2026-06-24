//! GuitarScreen sub-acts — batched port (p3-34..p3-46).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/`

/// CStagePerfGuitarScreen (787 LOC).
pub mod stage_perf_guitar {
    /// Default constructor arg.
    pub const DEFAULT_FROM_OUTSIDE: bool = false;
}

/// CStagePerfGuitarScreen.Chip (808 LOC).
pub mod guitar_chip {
    /// 5 lanes (R/G/B/Y/P).
    pub const LANE_COUNT: usize = 5;
    /// Lane widths.
    pub const LANE_W: f32 = 80.0;
}

/// CActPerfGuitarScore (116 LOC).
pub mod guitar_score {
    /// Position (CActPerfGuitarScore.cs).
    pub const SCORE_X: f32 = 40.0;
    pub const SCORE_Y: f32 = 13.0;
    pub const SCORE_DIGITS: usize = 7;
}

/// CActPerfGuitarCombo (23 LOC).
pub mod guitar_combo {
    /// Position.
    pub const COMBO_X: f32 = 1245.0;
    pub const COMBO_Y: f32 = 60.0;
}

/// CActPerfGuitarGauge (131 LOC).
pub mod guitar_gauge {
    /// Position.
    pub const GAUGE_X: f32 = 294.0;
    pub const GAUGE_Y_NORMAL: f32 = 626.0;
    pub const GAUGE_Y_REVERSE: f32 = 28.0;
}

/// CActPerfGuitarStatusPanel (237 LOC).
pub mod guitar_status_panel {
    /// Position.
    pub const X: f32 = 22.0;
    pub const Y: f32 = 250.0;
}

/// CActPerfGuitarRGB (202 LOC) — RGB color rendering.
pub mod guitar_rgb {
    /// 5 lane colors (R/G/B/Y/P).
    pub const RGB_COLORS: [[u8; 4]; 5] = [
        [255, 0, 0, 255],     // R
        [0, 255, 0, 255],     // G
        [0, 0, 255, 255],     // B
        [255, 255, 0, 255],   // Y
        [128, 0, 128, 255],   // P
    ];
}

/// CActPerfGuitarWailingBonus (197 LOC).
pub mod guitar_wailing {
    /// Wailing bonus position.
    pub const X: f32 = 1245.0;
    pub const Y: f32 = 200.0;
}

/// CActPerfGuitarLaneFlushGB (112 LOC).
pub mod guitar_lane_flush {
    /// 5 lanes (R/G/B/Y/P).
    pub const LANE_COUNT: usize = 5;
    /// Lane X positions (CActPerfGuitarLaneFlushGB.cs).
    pub const LANE_X: [f32; 5] = [430.0, 510.0, 590.0, 670.0, 750.0];
    /// Lane widths.
    pub const LANE_W: [f32; 5] = [70.0, 70.0, 70.0, 70.0, 70.0];
    /// Flush counter max.
    pub const FLUSH_COUNTER_MAX: u32 = 90;
}

/// CActPerfGuitarJudgementString (71 LOC).
pub mod guitar_judgement {
    /// Y position.
    pub const Y_FORWARD: f32 = 348.0;
    pub const Y_REVERSE: f32 = 583.0;
}

/// CActPerfGuitarDanger (78 LOC).
pub mod guitar_danger {
    /// Full-screen overlay.
    pub const OVERLAY_X: f32 = 0.0;
    pub const OVERLAY_Y: f32 = 0.0;
    pub const OVERLAY_W: f32 = 1280.0;
    pub const OVERLAY_H: f32 = 720.0;
}

/// CActPerfGuitarBonus (86 LOC).
pub mod guitar_bonus {
    /// Bonus pickup particle count.
    pub const PARTICLES_PER_BONUS: usize = 4;
}

/// HoldNote (93 LOC) — guitar hold note.
pub mod hold_note {
    /// Hold note min length (in measures).
    pub const HOLD_MIN_MEASURES: u32 = 1;
    /// Hold note max length.
    pub const HOLD_MAX_MEASURES: u32 = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_perf_guitar_default() {
        assert!(!stage_perf_guitar::DEFAULT_FROM_OUTSIDE);
    }

    #[test]
    fn guitar_chip_lane_count() {
        assert_eq!(guitar_chip::LANE_COUNT, 5);
    }

    #[test]
    fn guitar_score_position() {
        assert_eq!(guitar_score::SCORE_X, 40.0);
        assert_eq!(guitar_score::SCORE_Y, 13.0);
    }

    #[test]
    fn guitar_combo_position() {
        assert_eq!(guitar_combo::COMBO_X, 1245.0);
    }

    #[test]
    fn guitar_gauge_position() {
        assert_eq!(guitar_gauge::GAUGE_X, 294.0);
        assert_eq!(guitar_gauge::GAUGE_Y_NORMAL, 626.0);
    }

    #[test]
    fn guitar_status_panel_position() {
        assert_eq!(guitar_status_panel::X, 22.0);
        assert_eq!(guitar_status_panel::Y, 250.0);
    }

    #[test]
    fn guitar_rgb_colors_distinct() {
        let colors = guitar_rgb::RGB_COLORS;
        let unique: std::collections::HashSet<_> = colors.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn guitar_lane_flush_count() {
        assert_eq!(guitar_lane_flush::LANE_COUNT, 5);
        assert_eq!(guitar_lane_flush::FLUSH_COUNTER_MAX, 90);
    }

    #[test]
    fn guitar_judgement_y() {
        assert_eq!(guitar_judgement::Y_FORWARD, 348.0);
        assert_eq!(guitar_judgement::Y_REVERSE, 583.0);
    }

    #[test]
    fn guitar_danger_fullscreen() {
        assert_eq!(guitar_danger::OVERLAY_W, 1280.0);
        assert_eq!(guitar_danger::OVERLAY_H, 720.0);
    }

    #[test]
    fn guitar_bonus_particles() {
        assert_eq!(guitar_bonus::PARTICLES_PER_BONUS, 4);
    }

    #[test]
    fn hold_note_length_bounds() {
        assert_eq!(hold_note::HOLD_MIN_MEASURES, 1);
        assert_eq!(hold_note::HOLD_MAX_MEASURES, 32);
    }
}
