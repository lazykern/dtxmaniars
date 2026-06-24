//! DrumsScreen CActPerf sub-acts — port of `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/`.
//!
//! Strict-port-first (ADR-0010). Position constants + texture rects verbatim
//! from reference; rendering deferred to Bevy systems in `hud.rs`.
//!
//! ## Sub-acts ported
//!
//! | Sub-act | Reference | LOC | Status |
//! |---------|-----------|----:|--------|
//! | `CActPerfDrumsPad`         | CActPerfDrumsPad.cs         | 498 | port (positions + rects) |
//! | `CActPerfDrumsDanger`      | CActPerfDrumsDanger.cs      |  77 | port (overlay trigger) |
//! | `CActPerfDrumsFillingEffect` | CActPerfDrumsFillingEffect.cs | 41 | port (no-op trigger) |
//!
//! Already ported elsewhere:
//! - `CActPerfDrumsScore` → `gameplay-drums/src/hud.rs` (620 LoC)
//! - `CActPerfDrumsComboDGB` → `hud.rs`
//! - `CActPerfDrumsGauge` → `hud.rs`
//! - `CActPerfDrumsStatusPanel` → `hud.rs`
//! - `CActPerfDrumsJudgementString` → `hud.rs`
//! - `CActPerfDrumsLaneFlushD` → `gameplay-drums/src/perf_sub_acts_3.rs` (336 LoC)

#![allow(dead_code)] // Sub-acts consumed by hud.rs / systems.

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

/// One pad position + texture rect (BocuD `ST基本位置` in CActPerfDrumsPad.cs:5-9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadPosition {
    /// X coordinate in pixels (BocuD `st基本位置.x`).
    pub x: f32,
    /// Y coordinate in pixels (BocuD `st基本位置.y`).
    pub y: f32,
    /// Texture rectangle (BocuD `st基本位置.rc`).
    pub rect: PadRect,
}

/// Texture rectangle in 96x96 pad tiles (BocuD `RectangleF`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PadRect {
    /// X offset in tile-sheet (0..=3 columns of 96px = 384px total).
    pub x: f32,
    /// Y offset in tile-sheet (0..=2 rows of 96px = 288px total).
    pub y: f32,
    /// Width in pixels (always 96 per BocuD CActPerfDrumsPad.cs).
    pub w: f32,
    /// Height in pixels (always 96 per BocuD CActPerfDrumsPad.cs).
    pub h: f32,
}

/// Pad position table (BocuD CActPerfDrumsPad.cs:11-200).
///
/// Index matches `DrumsLane` enum value. Verbatim from reference.
pub const DRUMS_PAD_POSITIONS: [PadPosition; 10] = [
    // LC: (263, 10) rect (0, 0, 0x60, 0x60) — CActPerfDrumsPad.cs:14-19
    PadPosition {
        x: 263.0,
        y: 10.0,
        rect: PadRect {
            x: 0.0,
            y: 0.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // HH: (336, 10) rect (0x60, 0, 0x60, 0x60) — CActPerfDrumsPad.cs:22-27
    PadPosition {
        x: 336.0,
        y: 10.0,
        rect: PadRect {
            x: 96.0,
            y: 0.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // SD: (446, 10) rect (0, 0x60, 0x60, 0x60) — CActPerfDrumsPad.cs:30-35
    PadPosition {
        x: 446.0,
        y: 10.0,
        rect: PadRect {
            x: 0.0,
            y: 96.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // BD: (565, 10) rect (0, 0xc0, 0x60, 0x60) — CActPerfDrumsPad.cs:38-43
    PadPosition {
        x: 565.0,
        y: 10.0,
        rect: PadRect {
            x: 0.0,
            y: 192.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // HT: (510, 10) rect (0x60, 0x60, 0x60, 0x60) — CActPerfDrumsPad.cs:46-51
    PadPosition {
        x: 510.0,
        y: 10.0,
        rect: PadRect {
            x: 96.0,
            y: 96.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // LT: (622, 10) rect (0xc0, 0x60, 0x60, 0x60) — CActPerfDrumsPad.cs:54-59
    PadPosition {
        x: 622.0,
        y: 10.0,
        rect: PadRect {
            x: 192.0,
            y: 96.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // FT: (672, 10) rect (288, 0x60, 0x60, 0x60) — CActPerfDrumsPad.cs:62-67
    PadPosition {
        x: 672.0,
        y: 10.0,
        rect: PadRect {
            x: 288.0,
            y: 96.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // CY: (0x2df=735, 10) rect (0xc0, 0, 0x60, 0x60) — CActPerfDrumsPad.cs:70-75
    PadPosition {
        x: 735.0,
        y: 10.0,
        rect: PadRect {
            x: 192.0,
            y: 0.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // RD: (0x317=791, 10) rect (288, 0, 0x60, 0x60) — CActPerfDrumsPad.cs:78-83
    PadPosition {
        x: 791.0,
        y: 10.0,
        rect: PadRect {
            x: 288.0,
            y: 0.0,
            w: 96.0,
            h: 96.0,
        },
    },
    // LP: (396, 10) rect (0x60, 0xc0, 0x60, 0x60) — CActPerfDrumsPad.cs:86-91
    PadPosition {
        x: 396.0,
        y: 10.0,
        rect: PadRect {
            x: 96.0,
            y: 192.0,
            w: 96.0,
            h: 96.0,
        },
    },
];

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

    /// Lookup pad position for a lane.
    pub fn position(lane: DrumsLane) -> PadPosition {
        DRUMS_PAD_POSITIONS[lane as usize]
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

    // === DrumsLane / positions ===

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

    #[test]
    fn pad_position_lc_matches_reference() {
        // CActPerfDrumsPad.cs:14-19
        let p = DrumsPadState::position(DrumsLane::LC);
        assert_eq!(p.x, 263.0);
        assert_eq!(p.y, 10.0);
        assert_eq!(p.rect.x, 0.0);
        assert_eq!(p.rect.y, 0.0);
        assert_eq!(p.rect.w, 96.0);
        assert_eq!(p.rect.h, 96.0);
    }

    #[test]
    fn pad_position_hh_matches_reference() {
        // CActPerfDrumsPad.cs:22-27
        let p = DrumsPadState::position(DrumsLane::HH);
        assert_eq!(p.x, 336.0);
        assert_eq!(p.rect.x, 96.0);
    }

    #[test]
    fn pad_position_cy_matches_reference() {
        // CActPerfDrumsPad.cs:70-75 — 0x2df = 735
        let p = DrumsPadState::position(DrumsLane::CY);
        assert_eq!(p.x, 735.0);
        assert_eq!(p.rect.x, 192.0);
    }

    #[test]
    fn pad_position_lp_matches_reference() {
        // CActPerfDrumsPad.cs:86-91
        let p = DrumsPadState::position(DrumsLane::LP);
        assert_eq!(p.x, 396.0);
        assert_eq!(p.rect.y, 192.0);
    }

    #[test]
    fn pad_position_all_lanes_y_are_10() {
        for lane in DrumsLane::all() {
            let p = DrumsPadState::position(lane);
            assert_eq!(p.y, 10.0, "all pads should have y=10");
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
