#![allow(missing_docs)]
//! DrumsScreen sub-acts — batched port (p3-23..p3-33).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/`

/// CStagePerfDrumsScreen (3671 LOC) — Drums performance screen orchestrator.
pub mod stage_perf_drums {
    /// Default bIsCalledFromOutsidePerformance (constructor arg).
    pub const DEFAULT_FROM_OUTSIDE: bool = false;
}

/// CActPerfDrumsScore (76 LOC).
pub mod drums_score {
    /// Position (CActPerfDrumsScore.cs:12-13).
    pub const SCORE_X: f32 = 40.0;
    pub const SCORE_Y: f32 = 13.0;
    pub const SCORE_DIGITS: usize = 7;
}

/// CActPerfDrumsComboDGB (112 LOC).
pub mod drums_combo_dgb {
    /// Position (CActPerfDrumsComboDGB.cs:44-45).
    pub const COMBO_X: f32 = 1245.0;
    pub const COMBO_Y: f32 = 60.0;
    pub const BOMB_X: f32 = 845.0;
    pub const BOMB_Y: f32 = -130.0;
}

/// CActPerfDrumsGauge (88 LOC).
pub mod drums_gauge {
    /// Position (CActPerfDrumsGauge.cs:31).
    pub const GAUGE_X: f32 = 294.0;
    pub const GAUGE_Y_NORMAL: f32 = 626.0;
    pub const GAUGE_Y_REVERSE: f32 = 28.0;
}

/// CActPerfDrumsStatusPanel (211 LOC).
pub mod drums_status_panel {
    /// Position (CActPerfDrumsStatusPanel.cs:22).
    pub const X: f32 = 22.0;
    pub const Y: f32 = 250.0;
    /// 6 row positions.
    pub const ROW_DY: f32 = 30.0;
    pub const ROW0_Y: f32 = 72.0;
    /// Special row offsets.
    pub const ACHIEVE_Y: f32 = 277.0;
    pub const SKILL_Y: f32 = 363.0;
}

/// CActPerfDrumsJudgementString (102 LOC).
pub mod drums_judgement {
    /// Y position (CActPerfDrumsJudgementString.cs:73-79).
    pub const Y_FORWARD: f32 = 348.0;
    pub const Y_REVERSE: f32 = 583.0;
    pub const Y_TOP_REVERSE: f32 = 80.0;
    pub const VERT_DX: f32 = 32.0;
}

/// CActPerfDrumsLaneFlushD (456 LOC).
pub mod drums_lane_flush {
    /// 10 drums lanes (LC, HH, SD, BD, HT, LT, FT, CY, LP, RD).
    pub const LANE_COUNT: usize = 10;
    /// Lane X positions (CActPerfDrumsLaneFlushD.cs:17-72).
    pub const LANE_X: [f32; 10] = [
        298.0, 370.0, 470.0, 582.0, 528.0, 645.0, 694.0, 748.0, 419.0, 815.0,
    ];
    /// Lane widths.
    pub const LANE_W: [f32; 10] = [64.0, 46.0, 54.0, 60.0, 46.0, 46.0, 46.0, 64.0, 48.0, 38.0];
}

/// CActPerfDrumsPad (498 LOC).
pub mod drums_pad {
    /// Pad X positions (CActPerfDrumsPad.cs:16-103).
    pub const PAD_X: [f32; 10] = [
        263.0, 336.0, 446.0, 565.0, 510.0, 622.0, 672.0, 735.0, 791.0, 396.0,
    ];
    pub const PAD_Y: [f32; 10] = [10.0; 10];
    pub const PAD_SIZE: f32 = 96.0;
}

/// CActPerfDrumsDanger (77 LOC).
pub mod drums_danger {
    /// Full-screen red overlay (CActPerfDrumsDanger.cs:43-44).
    pub const OVERLAY_X: f32 = 0.0;
    pub const OVERLAY_Y: f32 = 0.0;
    pub const OVERLAY_W: f32 = 1280.0;
    pub const OVERLAY_H: f32 = 720.0;
    /// Counter max.
    pub const COUNTER_MAX: u32 = 0x7f;
}

/// CActPerfDrumsFillingEffect (41 LOC).
pub mod drums_filling {
    /// Fillin effect active flag.
    pub const FILLIN_EFFECT_DEFAULT: bool = false;
}

/// CActPerfPerfChipFireD (1080 LOC) — drum chip-strike particles.
pub mod drums_chip_fire {
    /// Particles per strike.
    pub const PARTICLES_PER_STRIKE: usize = 8;
    /// Particle lifetime frames.
    pub const LIFETIME_FRAMES: u32 = 60;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_perf_drums_default() {
        assert!(!stage_perf_drums::DEFAULT_FROM_OUTSIDE);
    }

    #[test]
    fn drums_score_position() {
        assert_eq!(drums_score::SCORE_X, 40.0);
        assert_eq!(drums_score::SCORE_Y, 13.0);
    }

    #[test]
    fn drums_combo_dgb_position() {
        assert_eq!(drums_combo_dgb::COMBO_X, 1245.0);
        assert_eq!(drums_combo_dgb::BOMB_X, 845.0);
    }

    #[test]
    fn drums_gauge_position() {
        assert_eq!(drums_gauge::GAUGE_X, 294.0);
        assert_eq!(drums_gauge::GAUGE_Y_NORMAL, 626.0);
    }

    #[test]
    fn drums_status_panel_position() {
        assert_eq!(drums_status_panel::X, 22.0);
        assert_eq!(drums_status_panel::Y, 250.0);
    }

    #[test]
    fn drums_lane_flush_count_and_positions() {
        assert_eq!(drums_lane_flush::LANE_COUNT, 10);
        assert_eq!(drums_lane_flush::LANE_X[0], 298.0);
        assert_eq!(drums_lane_flush::LANE_X[7], 748.0);
    }

    #[test]
    fn drums_pad_count() {
        assert_eq!(drums_pad::PAD_X.len(), 10);
    }

    #[test]
    fn drums_danger_fullscreen() {
        assert_eq!(drums_danger::OVERLAY_W, 1280.0);
        assert_eq!(drums_danger::OVERLAY_H, 720.0);
    }
}
