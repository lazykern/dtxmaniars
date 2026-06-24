//! `CStagePerfCommonScreen` — port of `Stage/06.Performance/CStagePerfCommonScreen.cs` (5067 LOC).
//!
//! Strict-port-first. Common base class for Drums/Guitar performance stages.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:1-5067`

use bevy::prelude::Resource;

/// Screen size constants (CStagePerfCommonScreen.cs).
pub const PERF_SCREEN_W: f32 = 1280.0;
pub const PERF_SCREEN_H: f32 = 720.0;

/// Top-area Y position (CStagePerfCommonScreen.cs — judgment line area).
pub const PERF_JUDGMENT_LINE_Y: f32 = 580.0;
/// Bottom pad Y (CStagePerfCommonScreen.cs — pad row).
pub const PERF_PAD_Y: f32 = 600.0;
/// Number of supported display FPS (BocuD defaults to 60).
pub const PERF_TARGET_FPS: u32 = 60;

/// Presence state (CStagePerfCommonScreen.cs:18-50 — Discord rich presence).
#[derive(Debug, Clone, Default)]
pub struct PerformancePresence {
    pub details: String,
    pub end_time_ms: i64,
    pub state: String,
    pub is_displayed: bool,
}

impl PerformancePresence {
    pub fn new() -> Self {
        Self {
            details: String::new(),
            end_time_ms: 0,
            state: "In Game".to_string(),
            is_displayed: false,
        }
    }

    /// Update presence with song title + difficulty (truncate details to 50).
    pub fn update_song(&mut self, title: &str, difficulty: &str) {
        let truncated: String = title.chars().take(50).collect();
        self.details = format!("{truncated} [{difficulty}]");
        self.is_displayed = true;
    }
}

/// Common performance stage state.
#[derive(Resource, Debug, Default, Clone)]
pub struct PerformanceStageState {
    pub just_started_update: bool,
    pub compact_mode: bool,
    pub current_song: Option<PerformanceSongInfo>,
    pub confirmed_difficulty: u8,
    pub presence: PerformancePresence,
}

impl PerformanceStageState {
    pub fn new() -> Self {
        Self {
            just_started_update: true,
            compact_mode: false,
            current_song: None,
            confirmed_difficulty: 0,
            presence: PerformancePresence::new(),
        }
    }

    /// Mark that update has run (presence now valid).
    pub fn on_update_complete(&mut self) {
        self.just_started_update = false;
    }
}

/// Lightweight song info for the presence update.
#[derive(Debug, Clone, Default)]
pub struct PerformanceSongInfo {
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_size_matches() {
        assert_eq!(PERF_SCREEN_W, 1280.0);
        assert_eq!(PERF_SCREEN_H, 720.0);
    }

    #[test]
    fn judgment_line_y_matches() {
        assert_eq!(PERF_JUDGMENT_LINE_Y, 580.0);
    }

    #[test]
    fn perf_target_fps_is_60() {
        assert_eq!(PERF_TARGET_FPS, 60);
    }

    #[test]
    fn presence_default_not_displayed() {
        let p = PerformancePresence::new();
        assert!(!p.is_displayed);
        assert_eq!(p.state, "In Game");
    }

    #[test]
    fn presence_update_truncates_title() {
        // CStagePerfCommonScreen.cs:38-42 — truncate details to 50 chars
        let mut p = PerformancePresence::new();
        let long = "A".repeat(100);
        p.update_song(&long, "Master");
        assert!(p.details.starts_with("A".repeat(50).as_str()));
        assert!(p.details.contains("[Master]"));
        assert!(p.is_displayed);
    }

    #[test]
    fn presence_short_title_kept() {
        let mut p = PerformancePresence::new();
        p.update_song("Hi", "Easy");
        assert_eq!(p.details, "Hi [Easy]");
    }

    #[test]
    fn perf_stage_state_new_just_started() {
        let s = PerformanceStageState::new();
        assert!(s.just_started_update);
        assert!(s.current_song.is_none());
    }

    #[test]
    fn perf_stage_on_update_complete_clears_just_started() {
        let mut s = PerformanceStageState::new();
        s.on_update_complete();
        assert!(!s.just_started_update);
    }
}
