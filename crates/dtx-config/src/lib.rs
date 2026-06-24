//! Pure configuration schema. RON load/save. Lands in M1+.

#![allow(dead_code)] // scaffold stub; populated in M1

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Master volume 0.0..1.0
    pub volume: f32,
    /// Scroll speed multiplier
    pub scroll_speed: f32,
    /// Show FPS overlay
    pub show_fps: bool,
}
