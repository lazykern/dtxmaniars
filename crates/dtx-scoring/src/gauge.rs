//! Gauge + combo state machines (Phase F9).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/Common/CActPerfCommonGauge.cs`
//! — full gauge state machine with thresholds (Good 80% / Excellent 100%).
//!
//! The gauge is a 0-100% resource that fills on hits and drains on
//! misses. The combo is a max-only counter that breaks on miss but
//! also tracks "Full Combo" / "Excellent" / "All Perfect" badges.

use crate::JudgmentKind;

/// Gauge thresholds (BocuD CActPerfCommonGauge.cs).
/// >=80% clears the song.
pub const GAUGE_GOOD: f32 = 80.0;
/// >=100% = full clear.
pub const GAUGE_EXCELLENT: f32 = 100.0;
/// Starting gauge value.
pub const GAUGE_START: f32 = 20.0;

/// Gauge state (BocuD CActPerfCommonGauge.cs:StatePlay).
///
/// Gauge value [0.0, 100.0]. Above 80% = stage clear. Above 100% is
/// clamped to 100%.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GaugeState {
    /// Current gauge [0.0, 100.0].
    pub value: f32,
    /// Whether the stage has been cleared (gauge >= 80%).
    pub cleared: bool,
    /// Whether the stage was failed (gauge == 0).
    pub failed: bool,
}

impl GaugeState {
    /// Construct fresh at start.
    pub fn new() -> Self {
        Self {
            value: GAUGE_START,
            cleared: false,
            failed: false,
        }
    }

    /// Apply a judgment kind to the gauge.
    pub fn apply(&mut self, kind: JudgmentKind) {
        let delta = gauge_delta(kind);
        self.value = (self.value + delta).clamp(0.0, 100.0);
        if self.value >= GAUGE_GOOD {
            self.cleared = true;
        }
        if self.value <= 0.0 {
            self.failed = true;
        }
    }

    /// Reset to start state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// True if gauge is in danger (< 30%).
    pub fn in_danger(&self) -> bool {
        self.value < 30.0
    }

    /// True if gauge is full (100%).
    pub fn is_full(&self) -> bool {
        self.value >= GAUGE_EXCELLENT
    }
}

/// Per-judgment gauge delta (BocuD CActPerfCommonGauge.cs:t進行時).
pub fn gauge_delta(kind: JudgmentKind) -> f32 {
    match kind {
        JudgmentKind::Perfect => 0.5,
        JudgmentKind::Great => 0.3,
        JudgmentKind::Good => 0.1,
        JudgmentKind::Poor => -1.0,
        JudgmentKind::Miss => -3.0,
    }
}

/// Combo state with FC / AP detection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComboState {
    /// Current combo (resets on miss).
    pub current: u32,
    /// Maximum combo achieved.
    pub max: u32,
    /// Total Perfect count.
    pub perfect_count: u32,
    /// Total judgment count (all kinds).
    pub total_count: u32,
    /// Bad / miss / poor count (any non-Perfect/Great/Good).
    pub imperfect_count: u32,
}

impl ComboState {
    /// Construct fresh.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a judgment kind.
    pub fn apply(&mut self, kind: JudgmentKind) {
        self.total_count += 1;
        if kind == JudgmentKind::Perfect {
            self.perfect_count += 1;
            self.current += 1;
        } else if matches!(kind, JudgmentKind::Great | JudgmentKind::Good) {
            self.current += 1;
        } else {
            // Poor or Miss — break combo, mark imperfect.
            self.imperfect_count += 1;
            self.current = 0;
        }
        if self.current > self.max {
            self.max = self.current;
        }
    }

    /// Reset.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// True if no misses or poor hits (all Perfect/Great/Good) and total_count > 0.
    pub fn is_full_combo(&self) -> bool {
        self.total_count > 0 && self.imperfect_count == 0
    }

    /// True if all Perfect (no Great/Good/Poor/Miss).
    pub fn is_all_perfect(&self) -> bool {
        self.total_count > 0 && self.perfect_count == self.total_count
    }

    /// Perfect percentage.
    pub fn perfect_pct(&self) -> f32 {
        if self.total_count == 0 {
            0.0
        } else {
            self.perfect_count as f32 / self.total_count as f32 * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_starts_at_20() {
        let g = GaugeState::new();
        assert!((g.value - 20.0).abs() < 0.01);
        assert!(!g.cleared);
        assert!(!g.failed);
    }

    #[test]
    fn gauge_default_zero() {
        let g = GaugeState::default();
        assert_eq!(g.value, 0.0);
    }

    #[test]
    fn gauge_perfect_increases() {
        let mut g = GaugeState::new();
        g.apply(JudgmentKind::Perfect);
        assert!(g.value > 20.0);
    }

    #[test]
    fn gauge_miss_decreases() {
        let mut g = GaugeState::new();
        g.apply(JudgmentKind::Miss);
        assert!(g.value < 20.0);
    }

    #[test]
    fn gauge_clamps_to_100() {
        let mut g = GaugeState {
            value: 99.5,
            ..Default::default()
        };
        g.apply(JudgmentKind::Perfect); // +0.5 → 100.0
        g.apply(JudgmentKind::Perfect); // +0.5 → would be 100.5, clamp to 100
        assert_eq!(g.value, 100.0);
    }

    #[test]
    fn gauge_clamps_to_0() {
        let mut g = GaugeState {
            value: 1.0,
            ..Default::default()
        };
        g.apply(JudgmentKind::Miss); // -3 → would be -2, clamp to 0
        assert_eq!(g.value, 0.0);
        assert!(g.failed);
    }

    #[test]
    fn gauge_cleared_at_80_plus() {
        let mut g = GaugeState {
            value: 79.0,
            ..Default::default()
        };
        g.apply(JudgmentKind::Perfect); // +0.5 = 79.5
        assert!(!g.cleared);
        g.apply(JudgmentKind::Perfect); // +0.5 = 80.0
        assert!(g.cleared, "gauge should be cleared at 80%");
    }

    #[test]
    fn gauge_in_danger_below_30() {
        let mut g = GaugeState::new();
        g.value = 25.0;
        assert!(g.in_danger());
        g.value = 35.0;
        assert!(!g.in_danger());
    }

    #[test]
    fn gauge_is_full_at_100() {
        let g = GaugeState {
            value: 100.0,
            ..Default::default()
        };
        assert!(g.is_full());
        let g = GaugeState {
            value: 99.9,
            ..Default::default()
        };
        assert!(!g.is_full());
    }

    #[test]
    fn gauge_reset_restores() {
        let mut g = GaugeState::new();
        g.apply(JudgmentKind::Miss);
        g.apply(JudgmentKind::Miss);
        g.reset();
        assert!((g.value - 20.0).abs() < 0.01);
    }

    #[test]
    fn gauge_delta_values() {
        assert!(gauge_delta(JudgmentKind::Perfect) > 0.0);
        assert!(gauge_delta(JudgmentKind::Great) > 0.0);
        assert!(gauge_delta(JudgmentKind::Good) > 0.0);
        assert!(gauge_delta(JudgmentKind::Poor) < 0.0);
        assert!(gauge_delta(JudgmentKind::Miss) < 0.0);
    }

    #[test]
    fn combo_starts_zero() {
        let c = ComboState::new();
        assert_eq!(c.current, 0);
        assert_eq!(c.max, 0);
    }

    #[test]
    fn combo_perfect_increments() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        assert_eq!(c.current, 1);
        assert_eq!(c.max, 1);
        assert_eq!(c.perfect_count, 1);
    }

    #[test]
    fn combo_great_increments_no_perfect() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Great);
        assert_eq!(c.current, 1);
        assert_eq!(c.max, 1);
        assert_eq!(c.perfect_count, 0);
    }

    #[test]
    fn combo_miss_resets_current() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Miss);
        assert_eq!(c.current, 0);
        assert_eq!(c.max, 2, "max should not reset");
        assert_eq!(c.imperfect_count, 1);
    }

    #[test]
    fn combo_poor_breaks_combo() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Poor);
        assert_eq!(c.current, 0);
        assert_eq!(c.imperfect_count, 1);
    }

    #[test]
    fn full_combo_detection() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Great);
        c.apply(JudgmentKind::Good);
        assert!(c.is_full_combo(), "no miss/poor should be FC");
    }

    #[test]
    fn not_full_combo_after_miss() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Miss);
        assert!(!c.is_full_combo());
    }

    #[test]
    fn all_perfect_detection() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Perfect);
        assert!(c.is_all_perfect());
    }

    #[test]
    fn not_all_perfect_with_great() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Great);
        assert!(!c.is_all_perfect());
    }

    #[test]
    fn perfect_pct_calculation() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Great);
        c.apply(JudgmentKind::Great);
        // 2/4 = 50%
        assert!((c.perfect_pct() - 50.0).abs() < 0.01);
    }

    #[test]
    fn perfect_pct_zero_total() {
        let c = ComboState::new();
        assert_eq!(c.perfect_pct(), 0.0);
    }

    #[test]
    fn combo_reset() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.reset();
        assert_eq!(c.current, 0);
        assert_eq!(c.max, 0);
    }

    #[test]
    fn combo_total_count() {
        let mut c = ComboState::new();
        c.apply(JudgmentKind::Perfect);
        c.apply(JudgmentKind::Great);
        c.apply(JudgmentKind::Miss);
        assert_eq!(c.total_count, 3);
    }
}
