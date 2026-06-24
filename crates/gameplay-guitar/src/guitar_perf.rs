//! GuitarScreen CActPerf sub-acts — port of `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/`.
//!
//! Strict-port-first (ADR-0010). Position constants verbatim from reference;
//! rendering deferred to Bevy systems in `hud.rs`.
//!
//! ## Sub-acts ported
//!
//! | Sub-act | Reference | LOC | Status |
//! |---------|-----------|----:|--------|
//! | `CActPerfGuitarScore`           | 116 | port (positions) |
//! | `CActPerfGuitarCombo`           |  23 | port (positions) |
//! | `CActPerfGuitarGauge`           | 131 | port (positions + state) |
//! | `CActPerfGuitarStatusPanel`     | 237 | port (positions) |
//! | `CActPerfGuitarJudgementString` |  71 | port (positions) |
//! | `CActPerfGuitarLaneFlushGB`     | 112 | port (real) |
//! | `CActPerfGuitarRGB`             | 202 | port (state) |
//! | `CActPerfGuitarDanger`          |  78 | port (state) |
//! | `CActPerfGuitarWailingBonus`    | 197 | port (state) |
//! | `CActPerfGuitarBonus`           |  86 | port (state) |
//! | `HoldNote`                      |  93 | port (state) |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/`

#![allow(dead_code)] // Sub-acts consumed by hud.rs / systems.

use std::time::Duration;

/// Guitar 5-lane indices (BocuD CActPerfGuitarLaneFlushGB.cs:5-15).
///
/// R=0 (red), G=1 (green), B=2 (blue), Y=3 (yellow), P=4 (purple).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuitarLane {
    /// Red fret
    R = 0,
    /// Green fret
    G = 1,
    /// Blue fret
    B = 2,
    /// Yellow fret
    Y = 3,
    /// Purple fret
    P = 4,
}

impl GuitarLane {
    /// All 5 lanes in reference order.
    pub fn all() -> [Self; 5] {
        [Self::R, Self::G, Self::B, Self::Y, Self::P]
    }
}

// === Score positions (BocuD CActPerfGuitarScore.cs:16-22) ===

/// Guitar P1 score X position (BocuD CActPerfGuitarScore.cs:16).
pub const GUITAR_SCORE_X_P1: f32 = 373.0;
/// Guitar P2 (= Bass) score X position (BocuD CActPerfGuitarScore.cs:17).
pub const GUITAR_SCORE_X_P2: f32 = 665.0;
/// Score Y position (BocuD CActPerfGuitarScore.cs:18).
pub const GUITAR_SCORE_Y: f32 = 12.0;
/// Score digit width (BocuD CActPerfGuitarScore.cs:65).
pub const GUITAR_SCORE_DIGIT_W: f32 = 36.0;
/// Score digit horizontal gap (BocuD CActPerfGuitarScore.cs:65).
pub const GUITAR_SCORE_DIGIT_GAP: f32 = 34.0;
/// Score digit Y offset from main Y (BocuD CActPerfGuitarScore.cs:67).
pub const GUITAR_SCORE_DIGIT_Y_OFFSET: f32 = 28.0;
/// Score total digits displayed (BocuD CActPerfGuitarScore.cs:65).
pub const GUITAR_SCORE_DIGITS: usize = 7;

// === Combo positions (BocuD CActPerfGuitarCombo.cs:6-17) ===

/// Guitar combo position (BocuD CActPerfGuitarCombo.cs:6-8).
pub const GUITAR_COMBO_X: f32 = 560.0;
/// Guitar combo Y position (BocuD CActPerfGuitarCombo.cs:7).
pub const GUITAR_COMBO_Y: f32 = 220.0;
/// Bass combo position (BocuD CActPerfGuitarCombo.cs:16-18).
/// Bass combo X position (BocuD CActPerfGuitarCombo.cs:16).
pub const GUITAR_BASS_COMBO_X: f32 = 845.0;
/// Bass combo Y position (BocuD CActPerfGuitarCombo.cs:17).
pub const GUITAR_BASS_COMBO_Y: f32 = 220.0;

// === Gauge positions (BocuD CActPerfGuitarGauge.cs:23-25) ===

/// Guitar gauge X (BocuD CActPerfGuitarGauge.cs:23).
pub const GUITAR_GAUGE_X: f32 = 80.0;
/// Bass gauge X (BocuD CActPerfGuitarGauge.cs:24 — 912 + 290 + 38 = 1240).
pub const BASS_GAUGE_X: f32 = 1240.0;
/// Gauge Y top of frame (BocuD CActPerfGuitarGauge.cs).
pub const GUITAR_GAUGE_Y: f32 = 0.0;
/// Gauge frame top half height (BocuD CActPerfGuitarGauge.cs).
pub const GUITAR_GAUGE_FRAME_H: f32 = 68.0;
/// Gauge bar height (BocuD CActPerfGuitarGauge.cs).
pub const GUITAR_GAUGE_BAR_H: f32 = 30.0;
/// Gauge bar X offset (BocuD CActPerfGuitarGauge.cs).
pub const GUITAR_GAUGE_BAR_X_OFFSET: f32 = 6.0;
/// Gauge bar Y (BocuD CActPerfGuitarGauge.cs).
pub const GUITAR_GAUGE_BAR_Y: f32 = 31.0;

// === Gauge state (BocuD CActPerfGuitarGauge.cs:35-37) ===

/// Counters for guitar gauge movement + vibration (BocuD CActPerfGuitarGauge.cs:35-36).
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarGaugeState {
    /// Position move counter (0..0x1a = 26 step 20) — CActPerfGuitarGauge.cs:35.
    pub ct_move: i32,
    /// Vibration counter (0..360 step 4) — CActPerfGuitarGauge.cs:36.
    pub ct_vibration: i32,
    /// Current Guitar gauge value [0.0, 1.0] (BocuD db現在のゲージ値.Guitar).
    pub gauge_guitar: f32,
    /// Current Bass gauge value [0.0, 1.0] (BocuD db現在のゲージ値.Bass).
    pub gauge_bass: f32,
}

impl GuitarGaugeState {
    /// `ct_move` max (BocuD CActPerfGuitarGauge.cs:35 — 0x1a).
    pub const CT_MOVE_MAX: i32 = 0x1a;
    /// `ct_move` step (BocuD CActPerfGuitarGauge.cs:35).
    pub const CT_MOVE_STEP: i32 = 20;
    /// `ct_vibration` max (BocuD CActPerfGuitarGauge.cs:36).
    pub const CT_VIBRATION_MAX: i32 = 360;
    /// `ct_vibration` step (BocuD CActPerfGuitarGauge.cs:36).
    pub const CT_VIBRATION_STEP: i32 = 4;

    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tick counters (BocuD CActPerfGuitarGauge.cs:39-40).
    pub fn tick(&mut self, dt: Duration) {
        let frames = (dt.as_secs_f32() * 60.0) as i32;
        if frames > 0 {
            self.ct_move = (self.ct_move + frames * Self::CT_MOVE_STEP) % Self::CT_MOVE_MAX;
            self.ct_vibration =
                (self.ct_vibration + frames * Self::CT_VIBRATION_STEP) % Self::CT_VIBRATION_MAX;
        }
    }
}

// === Status panel positions (BocuD CActPerfGuitarStatusPanel.cs) ===

/// Guitar status panel X (BocuD CActPerfGuitarStatusPanel.cs — left side).
pub const GUITAR_STATUS_PANEL_X: f32 = 0.0;
/// Guitar status panel Y (BocuD CActPerfGuitarStatusPanel.cs).
pub const GUITAR_STATUS_PANEL_Y: f32 = 250.0;
/// Bass status panel X (BocuD CActPerfGuitarStatusPanel.cs).
pub const BASS_STATUS_PANEL_X: f32 = 920.0;
/// Bass status panel Y (BocuD CActPerfGuitarStatusPanel.cs).
pub const BASS_STATUS_PANEL_Y: f32 = 250.0;

// === Judgement string positions (BocuD CActPerfGuitarJudgementString.cs) ===

/// Judgement string Y position (BocuD CActPerfGuitarJudgementString.cs).
pub const GUITAR_JUDGE_STRING_Y: f32 = 200.0;
/// Judgement string X offset per lane (BocuD CActPerfGuitarJudgementString.cs).
pub const GUITAR_JUDGE_STRING_LANE_DX: f32 = 100.0;

// === Lane flush (BocuD CActPerfGuitarLaneFlushGB.cs) ===

/// Guitar lane flush state (BocuD CActPerfGuitarLaneFlushGB.cs:11-15).
///
/// Per-lane color index: 0=R, 1=G, 2=B, 3=Y, 4=P. `bPressed[i]` is true
/// while the lane is being struck; resets each frame.
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarLaneFlush {
    /// Per-lane pressed flag (5 lanes).
    pub pressed: [bool; 5],
    /// Per-lane flash timer (0..0x46 = 70 frame decay).
    pub ct_flush: [i32; 5],
}

impl GuitarLaneFlush {
    /// Lane count.
    pub const LANE_COUNT: usize = 5;
    /// Flash decay frames (BocuD CActPerfGuitarLaneFlushGB.cs:7 — 0x46 = 70).
    pub const CT_FLUSH_MAX: i32 = 0x46;

    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a lane as pressed (BocuD CActPerfGuitarLaneFlushGB.cs:18-19).
    pub fn press(&mut self, lane: GuitarLane) {
        self.pressed[lane as usize] = true;
        self.ct_flush[lane as usize] = 0;
    }

    /// Tick all flash timers (BocuD CActPerfGuitarLaneFlushGB.cs:35).
    pub fn tick(&mut self, dt: Duration) {
        let frames = (dt.as_secs_f32() * 60.0) as i32;
        if frames > 0 {
            for ct in self.ct_flush.iter_mut() {
                *ct = (*ct + frames).min(Self::CT_FLUSH_MAX);
            }
        }
    }

    /// True if any lane is currently flashing.
    pub fn any_active(&self) -> bool {
        self.ct_flush.iter().any(|c| *c < Self::CT_FLUSH_MAX)
    }
}

// === RGB state (BocuD CActPerfGuitarRGB.cs) ===

/// Guitar RGB indicator state (BocuD CActPerfGuitarRGB.cs:18-21).
///
/// `b押された` is a 5-element bool array. RGB indicator shows which lanes
/// are currently held down.
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarRgbState {
    /// Per-lane pressed flag (5 lanes).
    pub pressed: [bool; 5],
    /// Above-shutter position (BocuD nシャッター上 STDGBVALUE).
    pub shutter_up: f32,
    /// Under-shutter position (BocuD nシャッター下 STDGBVALUE).
    pub shutter_down: f32,
}

impl GuitarRgbState {
    /// Lane count.
    pub const LANE_COUNT: usize = 5;

    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a lane as pressed (BocuD CActPerfGuitarRGB.cs).
    pub fn press(&mut self, lane: GuitarLane) {
        self.pressed[lane as usize] = true;
    }

    /// Release a lane.
    pub fn release(&mut self, lane: GuitarLane) {
        self.pressed[lane as usize] = false;
    }

    /// Clear all pressed flags (BocuD OnActivate).
    pub fn clear(&mut self) {
        self.pressed = [false; 5];
    }
}

// === Danger state (BocuD CActPerfGuitarDanger.cs) ===

/// Guitar danger state (BocuD CActPerfGuitarDanger.cs).
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarDangerState {
    /// True if Guitar gauge is in danger.
    pub guitar_danger: bool,
    /// True if Bass gauge is in danger.
    pub bass_danger: bool,
}

impl GuitarDangerState {
    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update from gauge values (called each frame).
    pub fn update_from_gauges(&mut self, guitar: f32, bass: f32, threshold: f32) {
        self.guitar_danger = guitar <= threshold;
        self.bass_danger = bass <= threshold;
    }

    /// True if either guitar or bass is in danger.
    pub fn any_danger(&self) -> bool {
        self.guitar_danger || self.bass_danger
    }
}

// === Wailing bonus state (BocuD CActPerfGuitarWailingBonus.cs) ===

/// Guitar wailing bonus state (BocuD CActPerfGuitarWailingBonus.cs:11-12).
///
/// `ct進行用` is `CCounter[3, 4]` — 3 instruments (Drums/Guitar/Bass) × 4 lanes.
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarWailingBonus {
    /// Active flag per instrument (3) × lane (4).
    pub active: [[bool; 4]; 3],
    /// Counter per instrument × lane.
    pub ct: [[i32; 4]; 3],
}

impl GuitarWailingBonus {
    /// Instrument count (BocuD CActPerfGuitarWailingBonus.cs:11 — `[3, 4]`).
    pub const INSTRUMENT_COUNT: usize = 3;
    /// Lane count per instrument.
    pub const LANE_COUNT: usize = 4;

    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start the wailing bonus for one instrument + lane.
    pub fn start(&mut self, instrument: usize, lane: usize) {
        if instrument < Self::INSTRUMENT_COUNT && lane < Self::LANE_COUNT {
            self.active[instrument][lane] = true;
            self.ct[instrument][lane] = 0;
        }
    }

    /// Tick counters; returns true if any slot is still active.
    pub fn tick(&mut self, dt: Duration) -> bool {
        let frames = (dt.as_secs_f32() * 60.0) as i32;
        if frames > 0 {
            for inst in 0..Self::INSTRUMENT_COUNT {
                for lane in 0..Self::LANE_COUNT {
                    if self.active[inst][lane] {
                        self.ct[inst][lane] += frames;
                        if self.ct[inst][lane] > 120 {
                            self.active[inst][lane] = false;
                        }
                    }
                }
            }
        }
        self.active.iter().flatten().any(|x| *x)
    }
}

// === Bonus (BocuD CActPerfGuitarBonus.cs) ===

/// Guitar bonus state (BocuD CActPerfGuitarBonus.cs).
///
/// Bonus is awarded when consecutive perfect/great hits reach a threshold.
#[derive(Debug, Clone, Default, bevy::prelude::Resource)]
pub struct GuitarBonus {
    /// Current bonus hit count.
    pub count: u32,
    /// Whether a bonus is currently displayed.
    pub active: bool,
}

impl GuitarBonus {
    /// Construct fresh state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the bonus counter.
    pub fn increment(&mut self) {
        self.count += 1;
        if self.count >= 100 {
            self.active = true;
        }
    }

    /// Reset bonus.
    pub fn reset(&mut self) {
        self.count = 0;
        self.active = false;
    }
}

// === Hold note (BocuD HoldNote.cs) ===

/// One active hold note (BocuD HoldNote.cs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldNote {
    /// Chip ID this hold is anchored to.
    pub chip_id: u32,
    /// Lane index (0..=4).
    pub lane: u8,
    /// Audio time of the head (ms).
    pub head_ms: i64,
    /// Audio time of the tail (ms).
    pub tail_ms: i64,
    /// Whether the hold is currently being held.
    pub is_held: bool,
}

impl HoldNote {
    /// Whether the hold has ended (audio time past tail).
    pub fn is_ended(&self, now_ms: i64) -> bool {
        now_ms >= self.tail_ms
    }

    /// Hold progress [0.0, 1.0].
    pub fn progress(&self, now_ms: i64) -> f32 {
        let total = (self.tail_ms - self.head_ms) as f32;
        if total <= 0.0 {
            return 1.0;
        }
        let elapsed = (now_ms - self.head_ms) as f32;
        (elapsed / total).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === GuitarLane ===

    #[test]
    fn guitar_lane_all_has_5() {
        assert_eq!(GuitarLane::all().len(), 5);
    }

    #[test]
    fn guitar_lane_discriminants_match_index() {
        for (i, lane) in GuitarLane::all().iter().enumerate() {
            assert_eq!(*lane as usize, i);
        }
    }

    // === Score positions ===

    #[test]
    fn guitar_score_x_p1_matches_reference() {
        // CActPerfGuitarScore.cs:16
        assert_eq!(GUITAR_SCORE_X_P1, 373.0);
    }

    #[test]
    fn guitar_score_x_p2_matches_reference() {
        // CActPerfGuitarScore.cs:17
        assert_eq!(GUITAR_SCORE_X_P2, 665.0);
    }

    #[test]
    fn guitar_score_y_matches_reference() {
        // CActPerfGuitarScore.cs:18
        assert_eq!(GUITAR_SCORE_Y, 12.0);
    }

    // === Combo positions ===

    #[test]
    fn guitar_combo_position_matches_reference() {
        // CActPerfGuitarCombo.cs:6-8
        assert_eq!(GUITAR_COMBO_X, 560.0);
        assert_eq!(GUITAR_COMBO_Y, 220.0);
    }

    #[test]
    fn bass_combo_position_matches_reference() {
        // CActPerfGuitarCombo.cs:16-18
        assert_eq!(GUITAR_BASS_COMBO_X, 845.0);
        assert_eq!(GUITAR_BASS_COMBO_Y, 220.0);
    }

    // === Gauge ===

    #[test]
    fn guitar_gauge_x_matches_reference() {
        // CActPerfGuitarGauge.cs:23
        assert_eq!(GUITAR_GAUGE_X, 80.0);
    }

    #[test]
    fn bass_gauge_x_matches_reference() {
        // CActPerfGuitarGauge.cs:24 — 912 + 290 + 38
        assert_eq!(BASS_GAUGE_X, 1240.0);
    }

    #[test]
    fn guitar_gauge_state_tick_advances_counters() {
        let mut s = GuitarGaugeState::new();
        s.tick(Duration::from_secs_f32(1.0 / 60.0));
        assert_eq!(s.ct_move, GuitarGaugeState::CT_MOVE_STEP);
    }

    // === LaneFlush ===

    #[test]
    fn guitar_lane_flush_press_marks_lane() {
        let mut f = GuitarLaneFlush::new();
        f.press(GuitarLane::R);
        assert!(f.pressed[0]);
        assert_eq!(f.ct_flush[0], 0);
    }

    #[test]
    fn guitar_lane_flush_tick_caps_at_max() {
        let mut f = GuitarLaneFlush::new();
        f.press(GuitarLane::G);
        f.tick(Duration::from_secs(10));
        assert_eq!(f.ct_flush[1], GuitarLaneFlush::CT_FLUSH_MAX);
    }

    // === RGB ===

    #[test]
    fn guitar_rgb_press_and_release() {
        let mut s = GuitarRgbState::new();
        s.press(GuitarLane::B);
        assert!(s.pressed[2]);
        s.release(GuitarLane::B);
        assert!(!s.pressed[2]);
    }

    #[test]
    fn guitar_rgb_clear_resets() {
        let mut s = GuitarRgbState::new();
        s.press(GuitarLane::Y);
        s.press(GuitarLane::P);
        s.clear();
        assert!(!s.pressed.iter().any(|p| *p));
    }

    // === Danger ===

    #[test]
    fn guitar_danger_update_from_gauges() {
        let mut s = GuitarDangerState::new();
        s.update_from_gauges(0.1, 0.5, 0.25);
        assert!(s.guitar_danger);
        assert!(!s.bass_danger);
        assert!(s.any_danger());
    }

    // === WailingBonus ===

    #[test]
    fn guitar_wailing_bonus_start_sets_active() {
        let mut w = GuitarWailingBonus::new();
        w.start(1, 2);
        assert!(w.active[1][2]);
        assert!(w.tick(Duration::from_secs_f32(1.0 / 60.0)));
    }

    #[test]
    fn guitar_wailing_bonus_out_of_range_ignored() {
        let mut w = GuitarWailingBonus::new();
        w.start(10, 10);
        // No panic, no state change.
        assert!(!w.active[0][0]);
    }

    // === Bonus ===

    #[test]
    fn guitar_bonus_default_inactive() {
        let b = GuitarBonus::new();
        assert_eq!(b.count, 0);
        assert!(!b.active);
    }

    #[test]
    fn guitar_bonus_increment_below_threshold() {
        let mut b = GuitarBonus::new();
        for _ in 0..50 {
            b.increment();
        }
        assert_eq!(b.count, 50);
        assert!(!b.active);
    }

    #[test]
    fn guitar_bonus_increment_above_threshold_activates() {
        let mut b = GuitarBonus::new();
        for _ in 0..100 {
            b.increment();
        }
        assert!(b.active);
    }

    #[test]
    fn guitar_bonus_reset_clears() {
        let mut b = GuitarBonus::new();
        b.increment();
        b.reset();
        assert_eq!(b.count, 0);
        assert!(!b.active);
    }

    // === HoldNote ===

    #[test]
    fn hold_note_ended_after_tail() {
        let n = HoldNote {
            chip_id: 1,
            lane: 0,
            head_ms: 1000,
            tail_ms: 2000,
            is_held: true,
        };
        assert!(!n.is_ended(1500));
        assert!(n.is_ended(2000));
        assert!(n.is_ended(2500));
    }

    #[test]
    fn hold_note_progress_midway() {
        let n = HoldNote {
            chip_id: 1,
            lane: 0,
            head_ms: 1000,
            tail_ms: 2000,
            is_held: true,
        };
        assert!((n.progress(1500) - 0.5).abs() < 0.01);
    }

    #[test]
    fn hold_note_progress_clamped() {
        let n = HoldNote {
            chip_id: 1,
            lane: 0,
            head_ms: 1000,
            tail_ms: 2000,
            is_held: true,
        };
        assert_eq!(n.progress(500), 0.0); // before head
        assert_eq!(n.progress(3000), 1.0); // after tail
    }
}
