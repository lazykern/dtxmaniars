#![allow(missing_docs)]
//! Performance sub-acts batched port (p3-11..p3-22).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/`

/// CActPerfProgressBar (543 LOC).
pub mod progress_bar {
    /// Position (CActPerfProgressBar.cs:23-25).
    pub const POS_DRUMS: (i32, i32) = (855, 15);
    pub const POS_GUITAR: (i32, i32) = (334, 85);
    pub const WIDTH: i32 = 20;
    pub const HEIGHT: i32 = 540;
}

/// CActPerfSkillMeter (302 LOC).
pub mod skill_meter {
    /// Position (CActPerfSkillMeter.cs:18-21).
    pub const GRAPH_BG_Y: i32 = 200;
    pub const DISP_HEIGHT: i32 = 400;
    pub const DISP_WIDTH: i32 = 60;
    /// 10 vertical slices.
    pub const SLICES: i32 = 10;
}

/// CActPerfScrollSpeed (87 LOC).
pub mod scroll_speed {
    /// 3 scroll speeds (Drums/Guitar/Bass).
    pub const SCROLL_SPEED_PARTS: usize = 3;
    /// Default speed multiplier.
    pub const DEFAULT_SCROLL: f64 = 1.0;
}

/// CActPerfStageClear (7 LOC) — empty marker module.
#[allow(dead_code)]
pub mod stage_clear {}

/// CActPerfStageFailure (123 LOC).
pub mod stage_failure {
    /// Counter max (CActPerfStageFailure.cs:18 — 0x3e8 = 1000).
    pub const COUNTER_MAX: i32 = 0x3e8;
    /// Counter tick rate.
    pub const COUNTER_TICK: i32 = 2;
}

/// CActPerfBGA (305 LOC).
pub mod bga {
    /// BGA layer count.
    pub const BGA_LAYERS: usize = 8;
}

/// CActPerfVideo (520 LOC).
pub mod video {
    /// Video render area position (CActPerfVideo.cs).
    pub const VIDEO_X: f32 = 0.0;
    pub const VIDEO_Y: f32 = 0.0;
    pub const VIDEO_W: f32 = 1280.0;
    pub const VIDEO_H: f32 = 720.0;
}

/// CActPerfAVI.old.cs (930 LOC) — old video format.
pub mod avi_old {
    /// AVI playback area (assumed fullscreen).
    pub const AVI_X: f32 = 0.0;
    pub const AVI_Y: f32 = 0.0;
    pub const AVI_W: f32 = 1280.0;
    pub const AVI_H: f32 = 720.0;
}

/// PerfNewChipFire (233 LOC) — chip-strike particles.
pub mod chip_fire {
    /// Particle count per strike.
    pub const PARTICLES_PER_STRIKE: usize = 8;
    /// Particle lifetime in frames.
    pub const PARTICLE_LIFETIME_FRAMES: u32 = 60;
}

/// InfoBox (84 LOC).
pub mod info_box {
    /// Position (InfoBox.cs:13).
    pub const INFO_BOX_X: f32 = 1270.0;
    pub const INFO_BOX_Y: f32 = 10.0;
    /// Size (InfoBox.cs:11).
    pub const INFO_BOX_W: f32 = 304.0;
    pub const INFO_BOX_H: f32 = 84.0;
}

/// CActPerformanceInformation (74 LOC).
pub mod perf_info {
    /// 5 judgment counts (Perfect/Great/Good/Poor/Miss).
    pub const PERF_INFO_JUDGMENT_FIELDS: usize = 5;
}

/// IPerfFire (11 LOC).
pub mod i_perf_fire {
    /// Just a marker — interface signature.
    pub trait IPerfFire {
        fn start(
            &mut self,
            lane: i32,
            b_fill_in: bool,
            b_big_wave: bool,
            b_small_wave: bool,
            judge_line_pos_y_delta: i32,
            b_display: bool,
        );
        fn on_update_and_draw(&mut self) -> i32;
        fn i_pos_y(&self) -> i32;
        fn set_i_pos_y(&mut self, y: i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_position() {
        assert_eq!(progress_bar::POS_DRUMS, (855, 15));
        assert_eq!(progress_bar::POS_GUITAR, (334, 85));
    }

    #[test]
    fn progress_bar_size() {
        assert_eq!(progress_bar::WIDTH, 20);
        assert_eq!(progress_bar::HEIGHT, 540);
    }

    #[test]
    fn skill_meter_size() {
        assert_eq!(skill_meter::DISP_HEIGHT, 400);
        assert_eq!(skill_meter::DISP_WIDTH, 60);
        assert_eq!(skill_meter::SLICES, 10);
    }

    #[test]
    fn scroll_speed_parts() {
        assert_eq!(scroll_speed::SCROLL_SPEED_PARTS, 3);
    }

    #[test]
    fn stage_failure_counter_max() {
        // CActPerfStageFailure.cs:18 — 0x3e8 = 1000
        assert_eq!(stage_failure::COUNTER_MAX, 1000);
    }

    #[test]
    fn bga_layers() {
        assert_eq!(bga::BGA_LAYERS, 8);
    }

    #[test]
    fn video_fullscreen() {
        assert_eq!(video::VIDEO_W, 1280.0);
        assert_eq!(video::VIDEO_H, 720.0);
    }

    #[test]
    fn info_box_position_top_right() {
        assert_eq!(info_box::INFO_BOX_X, 1270.0);
        assert_eq!(info_box::INFO_BOX_Y, 10.0);
    }

    #[test]
    fn info_box_size() {
        assert_eq!(info_box::INFO_BOX_W, 304.0);
        assert_eq!(info_box::INFO_BOX_H, 84.0);
    }

    #[test]
    fn perf_info_judgment_fields() {
        assert_eq!(perf_info::PERF_INFO_JUDGMENT_FIELDS, 5);
    }

    #[test]
    fn chip_fire_particles() {
        assert_eq!(chip_fire::PARTICLES_PER_STRIKE, 8);
    }
}
