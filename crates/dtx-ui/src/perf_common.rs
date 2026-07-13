//! Common performance sub-acts — base types shared between Drums/Guitar.
//!
//! Strict-port-first (ADR-0010). These are the 9 CActPerfCommon*.cs files
//! from `references/DTXmaniaNX/DTXMania/Stage/06.Performance/`.
//!
//! ## Coverage
//!
//! | Sub-act | Ref | Status | Notes |
//! |---------|----:|--------|-------|
//! | `CActPerfCommonScore`           | 142 | partial | gameplay-drums/src/score.rs |
//! | `CActPerfCommonCombo`           | 794 | partial | gameplay-drums/src/hud.rs (combo display) |
//! | `CActPerfCommonGauge`           | 296 | partial | gameplay-drums/src/hud.rs (gauge) |
//! | `CActPerfCommonStatusPanel`     | 531 | partial | gameplay-drums/src/hud.rs (status panel) |
//! | `CActPerfCommonJudgementString` | 301 | partial | gameplay-drums/src/hud.rs (judgement string) |
//! | `CActPerfCommonLaneFlushGB`     |  70 | **here** | 10-lane flush state, see `LaneFlushGB` |
//! | `CActPerfCommonRGB`             |  59 | **here** | 10-pressed-state array, see `RgbState` |
//! | `CActPerfCommonDanger`          |  57 | **here** | 3-instrument danger flags, see `DangerState` |
//! | `CActPerfCommonWailingBonus`    |  43 | **here** | wailing bonus trigger, see `WailingBonusState` |

#![allow(dead_code)] // Sub-acts consumed by gameplay-drums / gameplay-guitar.

use std::time::Duration;

/// 10-lane flush state (BocuD CActPerfCommonLaneFlushGB.cs:13-14, 32-37).
///
/// `ctUpdate` per lane tracks the flash duration (0..70 frames at 60fps).
/// Reference: `CActPerfCommonLaneFlushGB.cs:34-35` — `new CCounter(0, 70, 1, ...)`.
#[derive(Debug, Clone, Default)]
pub struct LaneFlushGB {
    /// Per-lane flash counters (`Option<Duration>` because lanes that haven't
    /// been hit are inactive). 0..10 indices map to 5 RGB lanes + 5 reverses.
    pub ct_update: [Option<Duration>; 10],
}

impl LaneFlushGB {
    /// Frame count when flush fully decays (BocuD CActPerfCommonLaneFlushGB.cs:34).
    pub const MAX_FRAMES: u32 = 70;
    /// Lane count (BocuD allocates 10).
    pub const LANE_COUNT: usize = 10;

    /// Construct a fresh, all-inactive flush state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Trigger a lane flush (BocuD CActPerfCommonLaneFlushGB.cs:31-37).
    /// Panics if `n_lane` is out of range, matching C# behavior.
    pub fn start(&mut self, n_lane: usize) {
        if n_lane >= Self::LANE_COUNT {
            panic!("有効範囲は 0～{} です。", Self::LANE_COUNT - 1);
        }
        self.ct_update[n_lane] = Some(Duration::ZERO);
    }

    /// Advance the per-lane flash counters by `dt`.
    pub fn tick(&mut self, dt: Duration) {
        let max = Duration::from_secs_f32(Self::MAX_FRAMES as f32 / 60.0);
        for ct in self.ct_update.iter_mut().flatten() {
            *ct = (*ct + dt).min(max);
        }
    }

    /// True if any lane is currently flashing.
    pub fn any_active(&self) -> bool {
        self.ct_update.iter().any(|c| c.is_some())
    }
}

/// RGB pressed state (BocuD CActPerfCommonRGB.cs:14).
///
/// `bPressedState` is a 10-element bool array. `Push(nLane)` sets
/// `bPressedState[nLane] = true`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RgbState {
    /// Per-lane pressed flag (10 lanes).
    pub pressed: [bool; 10],
}

impl RgbState {
    /// Lane count (BocuD CActPerfCommonRGB.cs:14).
    pub const LANE_COUNT: usize = 10;

    /// All lanes unpressed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark `n_lane` as pressed (BocuD CActPerfCommonRGB.cs:31-34).
    pub fn push(&mut self, n_lane: usize) {
        if n_lane < Self::LANE_COUNT {
            self.pressed[n_lane] = true;
        }
    }

    /// Clear all pressed flags (called on note release / OnActivate).
    pub fn clear(&mut self) {
        self.pressed = [false; 10];
    }

    /// Count of currently-pressed lanes.
    pub fn count_pressed(&self) -> usize {
        self.pressed.iter().filter(|p| **p).count()
    }
}

/// Danger state (BocuD CActPerfCommonDanger.cs:36).
///
/// `bDanger中` is a 3-element bool array indexed by instrument:
/// 0=Drums, 1=Guitar, 2=Bass. The C# class is abstract; the drums/guitar/bass
/// variants override `tUpdateAndDraw` to render the actual overlay. In Rust
/// we model this as a struct + trait.
#[derive(Debug, Clone, Copy, Default)]
pub struct DangerState {
    /// 0=Drums, 1=Guitar, 2=Bass (BocuD CActPerfCommonDanger.cs:36).
    pub danger: [bool; 3],
}

/// Instrument index for `DangerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerInstrument {
    Drums = 0,
    Guitar = 1,
    Bass = 2,
}

impl DangerState {
    /// Instrument count (BocuD CActPerfCommonDanger.cs:34-36).
    pub const INSTRUMENT_COUNT: usize = 3;

    /// All instruments safe.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set danger flag for one instrument.
    pub fn set(&mut self, inst: DangerInstrument, is_danger: bool) {
        self.danger[inst as usize] = is_danger;
    }

    /// True if the given instrument is in danger (gauge low).
    pub fn is_danger(&self, inst: DangerInstrument) -> bool {
        self.danger[inst as usize]
    }

    /// True if any instrument is in danger.
    pub fn any_danger(&self) -> bool {
        self.danger.iter().any(|d| *d)
    }
}

/// Wailing bonus trigger state (BocuD CActPerfCommonWailingBonus.cs).
///
/// Abstract base class in C#; the drums/guitar/bass variants override `Start`
/// to trigger the bonus effect. In Rust we model this as a state struct
/// with a `start` method.
#[derive(Debug, Clone, Default)]
pub struct WailingBonusState {
    /// True between `start()` and the next frame's tick (transient flag).
    pub triggered: bool,
    /// Instrument that triggered the bonus (0=Drums, 1=Guitar, 2=Bass).
    pub instrument: Option<u8>,
    /// Time of trigger (audio clock ms).
    pub time_ms: i64,
}

impl WailingBonusState {
    /// Construct a fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Trigger the wailing bonus (BocuD CActPerfCommonWailingBonus.cs:21).
    /// `part` is the instrument (0=Drums, 1=Guitar, 2=Bass).
    pub fn start(&mut self, part: u8, time_ms: i64) {
        self.triggered = true;
        self.instrument = Some(part);
        self.time_ms = time_ms;
    }

    /// Acknowledge the trigger (called after the bonus has been applied).
    pub fn acknowledge(&mut self) {
        self.triggered = false;
        self.instrument = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === LaneFlushGB ===

    #[test]
    fn lane_flush_gb_default_inactive() {
        let f = LaneFlushGB::new();
        assert!(!f.any_active());
        assert_eq!(f.ct_update.len(), LaneFlushGB::LANE_COUNT);
    }

    #[test]
    fn lane_flush_gb_start_activates_lane() {
        let mut f = LaneFlushGB::new();
        f.start(3);
        assert!(f.ct_update[3].is_some());
        assert!(f.any_active());
    }

    #[test]
    fn lane_flush_gb_start_out_of_range_panics() {
        let mut f = LaneFlushGB::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.start(LaneFlushGB::LANE_COUNT);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn lane_flush_gb_tick_advances() {
        let mut f = LaneFlushGB::new();
        f.start(0);
        f.tick(Duration::from_millis(16));
        assert!(f.ct_update[0].unwrap() > Duration::ZERO);
    }

    #[test]
    fn lane_flush_gb_tick_caps_at_max_frames() {
        let mut f = LaneFlushGB::new();
        f.start(0);
        f.tick(Duration::from_secs(10));
        let max = Duration::from_secs_f32(LaneFlushGB::MAX_FRAMES as f32 / 60.0);
        assert_eq!(f.ct_update[0].unwrap(), max);
    }

    // === RgbState ===

    #[test]
    fn rgb_state_default_unpressed() {
        let s = RgbState::new();
        assert_eq!(s.count_pressed(), 0);
    }

    #[test]
    fn rgb_state_push_marks_lane() {
        let mut s = RgbState::new();
        s.push(2);
        assert!(s.pressed[2]);
        assert_eq!(s.count_pressed(), 1);
    }

    #[test]
    fn rgb_state_push_out_of_range_ignored() {
        let mut s = RgbState::new();
        s.push(20);
        assert_eq!(s.count_pressed(), 0);
    }

    #[test]
    fn rgb_state_clear_resets_all() {
        let mut s = RgbState::new();
        s.push(0);
        s.push(5);
        assert_eq!(s.count_pressed(), 2);
        s.clear();
        assert_eq!(s.count_pressed(), 0);
    }

    // === DangerState ===

    #[test]
    fn danger_state_default_no_danger() {
        let d = DangerState::new();
        assert!(!d.any_danger());
        for i in 0..DangerState::INSTRUMENT_COUNT {
            assert!(!d.danger[i]);
        }
    }

    #[test]
    fn danger_state_set_per_instrument() {
        let mut d = DangerState::new();
        d.set(DangerInstrument::Drums, true);
        assert!(d.is_danger(DangerInstrument::Drums));
        assert!(!d.is_danger(DangerInstrument::Guitar));
        assert!(d.any_danger());
    }

    #[test]
    fn danger_state_clear_per_instrument() {
        let mut d = DangerState::new();
        d.set(DangerInstrument::Guitar, true);
        d.set(DangerInstrument::Guitar, false);
        assert!(!d.any_danger());
    }

    // === WailingBonusState ===

    #[test]
    fn wailing_bonus_default_not_triggered() {
        let w = WailingBonusState::new();
        assert!(!w.triggered);
        assert!(w.instrument.is_none());
    }

    #[test]
    fn wailing_bonus_start_records_instrument_and_time() {
        let mut w = WailingBonusState::new();
        w.start(0, 12345);
        assert!(w.triggered);
        assert_eq!(w.instrument, Some(0));
        assert_eq!(w.time_ms, 12345);
    }

    #[test]
    fn wailing_bonus_acknowledge_clears_state() {
        let mut w = WailingBonusState::new();
        w.start(1, 1000);
        w.acknowledge();
        assert!(!w.triggered);
        assert!(w.instrument.is_none());
    }
}
