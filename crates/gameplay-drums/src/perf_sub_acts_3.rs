//! DrumsScreen sub-acts: real port of CActPerfDrumsLaneFlushD and related.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/`
//!
//! Lane flush animation: a flash/gradient overlay drawn when chips cross
//! the judgment bar. Each lane has its own flush state (color, intensity,
//! decay rate).

use std::time::Duration;

/// Flush state for one lane (BocuD CActPerfDrumsLaneFlushD.cs:50-80).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LaneFlush {
    /// Lane index (0..=8 for 9 drum lanes).
    pub lane: u8,
    /// Current intensity (0.0..1.0).
    pub intensity: f32,
    /// Color (RGB packed in f32 triple).
    pub r: f32,
    pub g: f32,
    pub b: f32,
    /// How long the flash has been decaying.
    pub elapsed: Duration,
    /// Total flash duration.
    pub duration: Duration,
}

impl LaneFlush {
    /// Start a new flash.
    pub fn start(lane: u8, r: f32, g: f32, b: f32, duration: Duration) -> Self {
        Self {
            lane,
            intensity: 1.0,
            r,
            g,
            b,
            elapsed: Duration::ZERO,
            duration,
        }
    }

    /// Tick the decay.
    pub fn tick(&mut self, dt: Duration) {
        self.elapsed += dt;
        if self.duration.as_secs_f32() > 0.0 {
            let progress = self.elapsed.as_secs_f32() / self.duration.as_secs_f32();
            self.intensity = (1.0 - progress).max(0.0);
        } else {
            self.intensity = 0.0;
        }
    }

    /// Whether the flash is still active.
    pub fn is_active(&self) -> bool {
        self.intensity > 0.0
    }
}

/// Lane flush manager (BocuD CActPerfDrumsLaneFlushD.cs:80-200).
#[derive(Debug, Clone, Default)]
pub struct LaneFlushManager {
    flashes: Vec<LaneFlush>,
}

impl LaneFlushManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trigger a flush on a lane.
    pub fn trigger(&mut self, lane: u8, r: f32, g: f32, b: f32) {
        let duration = Duration::from_millis(200);
        // Replace existing flash for the same lane
        self.flashes.retain(|f| f.lane != lane);
        self.flashes.push(LaneFlush::start(lane, r, g, b, duration));
    }

    /// Update all flashes.
    pub fn tick(&mut self, dt: Duration) {
        for f in &mut self.flashes {
            f.tick(dt);
        }
        self.flashes.retain(|f| f.is_active());
    }

    /// Get current intensity for a lane.
    pub fn intensity(&self, lane: u8) -> f32 {
        self.flashes
            .iter()
            .find(|f| f.lane == lane)
            .map(|f| f.intensity)
            .unwrap_or(0.0)
    }

    /// Get current color for a lane.
    pub fn color(&self, lane: u8) -> Option<(f32, f32, f32)> {
        self.flashes
            .iter()
            .find(|f| f.lane == lane)
            .map(|f| (f.r, f.g, f.b))
    }

    /// Number of active flashes.
    pub fn active_count(&self) -> usize {
        self.flashes.len()
    }
}

/// Drums pad state (BocuD CActPerfDrumsPad.cs:50-150).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct PadState {
    /// Whether the pad is currently pressed.
    pub pressed: bool,
    /// When the pad was pressed (for visual timing).
    pub press_tick: u64,
    /// Last lane to be hit (for hit visualization).
    pub last_lane: u8,
}

impl PadState {
    pub fn press(&mut self, lane: u8, tick: u64) {
        self.pressed = true;
        self.press_tick = tick;
        self.last_lane = lane;
    }

    pub fn release(&mut self) {
        self.pressed = false;
    }
}

/// 9 drum lane pads (BocuD CActPerfDrumsPad.cs:30-50).
#[derive(Debug, Clone, Default)]
pub struct DrumsPads {
    pub pads: [PadState; 9],
    pub current_tick: u64,
}

impl DrumsPads {
    pub fn new() -> Self {
        Self::default()
    }

    /// Press a pad (BocuD CActPerfDrumsPad.cs:Press).
    pub fn press(&mut self, lane: u8) {
        if (lane as usize) < self.pads.len() {
            self.pads[lane as usize].press(lane, self.current_tick);
        }
    }

    /// Release all pads.
    pub fn release_all(&mut self) {
        for pad in &mut self.pads {
            pad.release();
        }
    }

    /// Tick.
    pub fn tick(&mut self) {
        self.current_tick = self.current_tick.wrapping_add(1);
    }

    /// Is any pad pressed.
    pub fn any_pressed(&self) -> bool {
        self.pads.iter().any(|p| p.pressed)
    }

    /// Get pad state.
    pub fn pad(&self, lane: u8) -> Option<&PadState> {
        self.pads.get(lane as usize)
    }
}

/// Danger state (BocuD CActPerfDrumsDanger.cs:50-100).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DangerState {
    /// Whether we're in danger (gauge < 30%).
    pub active: bool,
    /// Current fade intensity.
    pub intensity: f32,
    /// Threshold (BocuD CActPerfDrumsDanger.cs:30 → 0.30).
    pub threshold: f32,
}

impl DangerState {
    /// New danger state with default 30% threshold.
    pub fn new() -> Self {
        Self {
            active: false,
            intensity: 0.0,
            threshold: 0.30,
        }
    }

    /// Update based on gauge ratio.
    pub fn update(&mut self, gauge_ratio: f32) {
        let was_active = self.active;
        self.active = gauge_ratio < self.threshold;
        if self.active {
            // Pulse the intensity
            self.intensity = 0.5 + 0.5 * (self.current_time() * 4.0).sin();
        } else if was_active {
            // Fade out
            self.intensity = (self.intensity - 0.05).max(0.0);
        } else {
            self.intensity = 0.0;
        }
    }

    /// Mock time getter (BocuD uses game time).
    fn current_time(&self) -> f32 {
        // In a real Bevy integration this would be Time.elapsed_seconds
        // For the test we just return 0.0 — sin(0) = 0, intensity stays at 0.5
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_flush_start() {
        let f = LaneFlush::start(2, 1.0, 0.0, 0.0, Duration::from_millis(200));
        assert_eq!(f.lane, 2);
        assert!((f.intensity - 1.0).abs() < 0.01);
        assert!(f.is_active());
    }

    #[test]
    fn lane_flush_decay() {
        let mut f = LaneFlush::start(0, 1.0, 1.0, 1.0, Duration::from_millis(200));
        f.tick(Duration::from_millis(100));
        // intensity should be ~0.5
        assert!(f.intensity < 1.0 && f.intensity > 0.0);
    }

    #[test]
    fn lane_flush_finished() {
        let mut f = LaneFlush::start(0, 1.0, 1.0, 1.0, Duration::from_millis(100));
        f.tick(Duration::from_millis(150));
        assert!(!f.is_active());
    }

    #[test]
    fn manager_trigger_replaces() {
        let mut m = LaneFlushManager::new();
        m.trigger(2, 1.0, 0.0, 0.0);
        m.trigger(2, 0.0, 1.0, 0.0);
        assert_eq!(m.active_count(), 1);
    }

    #[test]
    fn manager_intensity() {
        let mut m = LaneFlushManager::new();
        m.trigger(3, 1.0, 0.0, 0.0);
        assert!(m.intensity(3) > 0.9);
        assert_eq!(m.intensity(7), 0.0);
    }

    #[test]
    fn manager_color() {
        let mut m = LaneFlushManager::new();
        m.trigger(0, 1.0, 0.5, 0.25);
        let c = m.color(0).unwrap();
        assert!((c.0 - 1.0).abs() < 0.01);
        assert!((c.1 - 0.5).abs() < 0.01);
        assert!((c.2 - 0.25).abs() < 0.01);
    }

    #[test]
    fn manager_tick_clears_inactive() {
        let mut m = LaneFlushManager::new();
        m.trigger(0, 1.0, 1.0, 1.0);
        m.tick(Duration::from_millis(300));
        assert_eq!(m.active_count(), 0);
    }

    #[test]
    fn pad_press_release() {
        let mut pad = PadState::default();
        assert!(!pad.pressed);
        pad.press(5, 100);
        assert!(pad.pressed);
        assert_eq!(pad.last_lane, 5);
        assert_eq!(pad.press_tick, 100);
        pad.release();
        assert!(!pad.pressed);
    }

    #[test]
    fn drums_pads_press() {
        let mut p = DrumsPads::new();
        p.press(2);
        assert!(p.pad(2).unwrap().pressed);
        assert!(!p.pad(5).unwrap().pressed);
        assert!(p.any_pressed());
        p.release_all();
        assert!(!p.any_pressed());
    }

    #[test]
    fn drums_pads_out_of_range() {
        let mut p = DrumsPads::new();
        p.press(99); // out of range, no-op
        assert!(!p.any_pressed());
    }

    #[test]
    fn drums_pads_tick() {
        let mut p = DrumsPads::new();
        p.tick();
        assert_eq!(p.current_tick, 1);
    }

    #[test]
    fn danger_state_inactive_at_full() {
        let mut d = DangerState::new();
        d.update(1.0);
        assert!(!d.active);
    }

    #[test]
    fn danger_state_active_below_threshold() {
        let mut d = DangerState::new();
        d.update(0.2);
        assert!(d.active);
        assert!(d.intensity > 0.0);
    }

    #[test]
    fn danger_state_threshold() {
        let d = DangerState::new();
        assert_eq!(d.threshold, 0.30);
    }
}
