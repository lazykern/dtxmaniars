//! DrumsScreen CActPerf sub-acts — mechanics-only port.
//!
//! ADR-0010 relaxed: position constants + texture rects stripped (UI layer
//! is osu-style redesigned). State machines (DrumsLane, DrumsPadState,
//! DrumsDangerState, DrumsFillingEffect) kept.
//!
//! ## Sub-acts ported
//!
//! | Sub-act | Reference | Status |
//! |---------|-----------|--------|
//! | `CActPerfDrumsPad`            | CActPerfDrumsPad.cs            | mechanics (pressed[] + last_hit_ms[]) |
//! | `CActPerfDrumsDanger`         | CActPerfDrumsDanger.cs         | full state machine (ct_move, ct_opacity) |
//! | `CActPerfDrumsFillingEffect`  | CActPerfDrumsFillingEffect.cs  | mechanics (active flag) |
//!
//! Other sub-acts (Score, ComboDGB, Gauge, StatusPanel, JudgementString,
//! LaneFlushD) ported in their respective crates.

#![allow(dead_code)] // Sub-acts consumed by gameplay systems.

use std::time::Duration;

/// Lane index enum matching the 10 drums lanes (BocuD CActPerfDrumsPad.cs:11-200).
///
/// LC=0, HH=1, SD=2, BD=3, HT=4, LT=5, FT=6, CY=7, RD=8, LP=9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumsLane {
    /// Left Cymbal
    LC = 0,
    /// Hi-Hat (closed)
    HH = 1,
    /// Snare Drum
    SD = 2,
    /// Bass Drum
    BD = 3,
    /// High Tom
    HT = 4,
    /// Low Tom
    LT = 5,
    /// Floor Tom
    FT = 6,
    /// Cymbal
    CY = 7,
    /// Ride Cymbal
    RD = 8,
    /// Left Pedal
    LP = 9,
}

impl DrumsLane {
    /// All 10 lanes in reference order.
    pub fn all() -> [Self; 10] {
        [
            Self::LC,
            Self::HH,
            Self::SD,
            Self::BD,
            Self::HT,
            Self::LT,
            Self::FT,
            Self::CY,
            Self::RD,
            Self::LP,
        ]
    }
}

/// Pad pressed state (BocuD CActPerfDrumsPad.cs:201-220 — pad state array).
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct DrumsPadState {
    /// Per-lane pressed flag (true while the lane is being hit).
    pub pressed: [bool; 10],
    /// Last hit time per lane (for decay animation).
    pub last_hit_ms: [i64; 10],
}

impl DrumsPadState {
    /// Construct empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark `lane` as pressed at `time_ms`.
    pub fn press(&mut self, lane: DrumsLane, time_ms: i64) {
        self.pressed[lane as usize] = true;
        self.last_hit_ms[lane as usize] = time_ms;
    }

    /// Mark `lane` as released.
    pub fn release(&mut self, lane: DrumsLane) {
        self.pressed[lane as usize] = false;
    }

    /// True if `lane` is currently held.
    pub fn is_pressed(&self, lane: DrumsLane) -> bool {
        self.pressed[lane as usize]
    }
}

/// Danger overlay state for drums (BocuD CActPerfDrumsDanger.cs:25-30).
///
/// `ct移動用` is `CCounter(0, 0x7f, 7, ...)` — 0..127 stepping 7.
/// `ct透明度用` is `CCounter(0, 250, 4, ...)` — 0..250 stepping 4.
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct DrumsDangerState {
    /// Whether the drums gauge is currently in danger.
    pub is_danger: bool,
    /// Position counter (CCounter 0..0x7f step 7).
    pub ct_move: i32,
    /// Opacity counter (CCounter 0..250 step 4).
    pub ct_opacity: i32,
}

impl DrumsDangerState {
    /// Counter max for `ct移動用` (BocuD CActPerfDrumsDanger.cs:27).
    pub const CT_MOVE_MAX: i32 = 0x7f;
    /// Counter step for `ct移動用`.
    pub const CT_MOVE_STEP: i32 = 7;
    /// Counter max for `ct透明度用` (BocuD CActPerfDrumsDanger.cs:28).
    pub const CT_OPACITY_MAX: i32 = 250;
    /// Counter step for `ct透明度用`.
    pub const CT_OPACITY_STEP: i32 = 4;

    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Trigger danger (called when `bIsDangerDrums` becomes true).
    /// Resets both counters per BocuD CActPerfDrumsDanger.cs:27-28.
    pub fn trigger(&mut self) {
        if !self.is_danger {
            self.ct_move = 0;
            self.ct_opacity = 0;
            self.is_danger = true;
        }
    }

    /// Tick counters (BocuD CActPerfDrumsDanger.cs:33-34).
    /// `dt` is the elapsed real time; counters loop independently.
    pub fn tick(&mut self, dt: Duration) {
        let frames = (dt.as_secs_f32() * 60.0) as i32;
        if frames > 0 {
            self.ct_move = (self.ct_move + frames * Self::CT_MOVE_STEP) % Self::CT_MOVE_MAX;
            self.ct_opacity =
                (self.ct_opacity + frames * Self::CT_OPACITY_STEP) % Self::CT_OPACITY_MAX;
        }
    }

    /// Current opacity in [0.0, 1.0] (BocuD CActPerfDrumsDanger.cs:37 — `num / 255.0f`).
    pub fn opacity_alpha(&self) -> f32 {
        self.ct_opacity as f32 / 255.0
    }
}

/// Fillin effect active flag (BocuD CActPerfDrumsFillingEffect.cs).
///
/// The C# class is essentially empty — no textures, no draw. It's a marker
/// sub-act. We model the same as a boolean.
#[derive(Debug, Clone, Copy, Default, bevy::prelude::Resource)]
pub struct DrumsFillingEffect {
    /// True while the fillin visual is active.
    pub active: bool,
}

impl DrumsFillingEffect {
    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start the fillin effect.
    pub fn start(&mut self) {
        self.active = true;
    }

    /// End the fillin effect.
    pub fn end(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === DrumsLane ===

    #[test]
    fn drums_lane_all_has_10() {
        assert_eq!(DrumsLane::all().len(), 10);
    }

    #[test]
    fn drums_lane_discriminants_match_index() {
        for (i, lane) in DrumsLane::all().iter().enumerate() {
            assert_eq!(*lane as usize, i, "lane {lane:?} should have index {i}");
        }
    }

    // === DrumsPadState ===

    #[test]
    fn pad_state_default_unpressed() {
        let s = DrumsPadState::new();
        for lane in DrumsLane::all() {
            assert!(!s.is_pressed(lane));
        }
    }

    #[test]
    fn pad_state_press_and_release() {
        let mut s = DrumsPadState::new();
        s.press(DrumsLane::SD, 1000);
        assert!(s.is_pressed(DrumsLane::SD));
        assert_eq!(s.last_hit_ms[DrumsLane::SD as usize], 1000);
        s.release(DrumsLane::SD);
        assert!(!s.is_pressed(DrumsLane::SD));
    }

    #[test]
    fn pad_state_independent_lanes() {
        let mut s = DrumsPadState::new();
        s.press(DrumsLane::HH, 100);
        s.press(DrumsLane::BD, 200);
        assert!(s.is_pressed(DrumsLane::HH));
        assert!(s.is_pressed(DrumsLane::BD));
        assert!(!s.is_pressed(DrumsLane::SD));
    }

    // === DrumsDangerState ===

    #[test]
    fn danger_state_default_not_danger() {
        let d = DrumsDangerState::new();
        assert!(!d.is_danger);
        assert_eq!(d.ct_move, 0);
        assert_eq!(d.ct_opacity, 0);
    }

    #[test]
    fn danger_state_trigger_resets_counters() {
        let mut d = DrumsDangerState {
            is_danger: false,
            ct_move: 50,
            ct_opacity: 100,
        };
        d.trigger();
        assert!(d.is_danger);
        assert_eq!(d.ct_move, 0);
        assert_eq!(d.ct_opacity, 0);
    }

    #[test]
    fn danger_state_trigger_idempotent_when_already_danger() {
        let mut d = DrumsDangerState {
            is_danger: true,
            ct_move: 30,
            ct_opacity: 60,
        };
        d.trigger();
        // Already in danger — counters NOT reset.
        assert_eq!(d.ct_move, 30);
        assert_eq!(d.ct_opacity, 60);
    }

    #[test]
    fn danger_state_tick_advances_within_max() {
        let mut d = DrumsDangerState {
            is_danger: true,
            ct_move: 0,
            ct_opacity: 0,
        };
        d.tick(Duration::from_secs_f32(1.0 / 60.0)); // 1 frame
        assert_eq!(d.ct_move, DrumsDangerState::CT_MOVE_STEP);
        assert_eq!(d.ct_opacity, DrumsDangerState::CT_OPACITY_STEP);
    }

    #[test]
    fn danger_state_opacity_alpha_in_range() {
        let d = DrumsDangerState {
            is_danger: true,
            ct_move: 50,
            ct_opacity: 127,
        };
        let a = d.opacity_alpha();
        assert!((0.0..=1.0).contains(&a));
    }

    // === DrumsFillingEffect ===

    #[test]
    fn filling_effect_default_inactive() {
        let f = DrumsFillingEffect::new();
        assert!(!f.active);
    }

    #[test]
    fn filling_effect_start_and_end() {
        let mut f = DrumsFillingEffect::new();
        f.start();
        assert!(f.active);
        f.end();
        assert!(!f.active);
    }
}
