//! Damage level logic (Phase F6).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConstants.cs:44-48`
//! (EDamageLevel) + `Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`
//! (HP drain on miss).
//!
//! 4 levels (BocuD): None / Small / Normal / High (we use the names
//! `dtx_core::DamageLevel` enum + a per-level `Behavior` trait that maps
//! HP=0 + judgment to failure mode).

use bevy::prelude::Resource;
use dtx_core::constants::DamageLevel;
use dtx_scoring::JudgmentKind;

/// Result of a damage-level event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageEvent {
    /// HP unchanged (e.g. None level).
    NoChange,
    /// HP drained by N (e.g. miss penalty).
    HpDrain(u8),
    /// HP restored by N (e.g. perfect hit gain).
    HpGain(u8),
    /// Stage failed — HP reached 0.
    StageFailed,
    /// Combo broken but no HP impact.
    ComboBroken,
}

/// Per-level damage table: HP delta per judgment kind.
///
/// Each level has different gain/loss magnitudes. None=ignore; Small=±1;
/// Normal=±2; High=±3 (lose more on miss, gain less on hit).
pub fn hp_delta_for_judgment(level: DamageLevel, kind: JudgmentKind) -> i8 {
    match level {
        DamageLevel::Small => match kind {
            JudgmentKind::Perfect => 1,
            JudgmentKind::Great => 0,
            JudgmentKind::Good => 0,
            JudgmentKind::Ok => -1,
            JudgmentKind::Miss => -2,
        },
        DamageLevel::Normal => match kind {
            JudgmentKind::Perfect => 1,
            JudgmentKind::Great => 0,
            JudgmentKind::Good => -1,
            JudgmentKind::Ok => -2,
            JudgmentKind::Miss => -3,
        },
        DamageLevel::High => match kind {
            JudgmentKind::Perfect => 0,
            JudgmentKind::Great => 0,
            JudgmentKind::Good => -1,
            JudgmentKind::Ok => -3,
            JudgmentKind::Miss => -5,
        },
    }
}

/// HP state for one instrument.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HpState {
    /// Current HP [0, 1000]. 0 = stage failed.
    pub current: i32,
    /// Maximum HP (default 1000). Used for gauge-style displays.
    pub max: i32,
    /// Whether the stage has failed (HP=0 with damage level active).
    pub stage_failed: bool,
}

impl HpState {
    /// Construct fresh HP at max.
    pub fn new(max: i32) -> Self {
        Self {
            current: max,
            max,
            stage_failed: false,
        }
    }

    /// Apply a damage event.
    pub fn apply(&mut self, evt: DamageEvent) {
        match evt {
            DamageEvent::NoChange | DamageEvent::ComboBroken => {}
            DamageEvent::HpDrain(d) => {
                self.current = (self.current - d as i32).max(0);
                if self.current == 0 {
                    self.stage_failed = true;
                }
            }
            DamageEvent::HpGain(g) => {
                self.current = (self.current + g as i32).min(self.max);
            }
            DamageEvent::StageFailed => {
                self.current = 0;
                self.stage_failed = true;
            }
        }
    }

    /// Apply a judgment with the given damage level.
    pub fn apply_judgment(&mut self, level: DamageLevel, kind: JudgmentKind) {
        let d = hp_delta_for_judgment(level, kind);
        if d > 0 {
            self.apply(DamageEvent::HpGain(d as u8));
        } else if d < 0 {
            self.apply(DamageEvent::HpDrain((-d) as u8));
        }
    }

    /// HP percentage [0, 100].
    pub fn pct(&self) -> f32 {
        if self.max <= 0 {
            0.0
        } else {
            (self.current as f32 / self.max as f32) * 100.0
        }
    }

    /// Reset to max HP (used on stage entry).
    pub fn reset(&mut self) {
        self.current = self.max;
        self.stage_failed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hp_starts_at_max() {
        let hp = HpState::new(1000);
        assert_eq!(hp.current, 1000);
        assert!(!hp.stage_failed);
    }

    #[test]
    fn hp_default_zero() {
        let hp = HpState::default();
        assert_eq!(hp.current, 0);
        assert_eq!(hp.max, 0);
    }

    #[test]
    fn small_perfect_gains_1() {
        assert_eq!(
            hp_delta_for_judgment(DamageLevel::Small, JudgmentKind::Perfect),
            1
        );
    }

    #[test]
    fn small_miss_drains_2() {
        assert_eq!(
            hp_delta_for_judgment(DamageLevel::Small, JudgmentKind::Miss),
            -2
        );
    }

    #[test]
    fn normal_perfect_gains_1_miss_drains_3() {
        assert_eq!(
            hp_delta_for_judgment(DamageLevel::Normal, JudgmentKind::Perfect),
            1
        );
        assert_eq!(
            hp_delta_for_judgment(DamageLevel::Normal, JudgmentKind::Miss),
            -3
        );
    }

    #[test]
    fn high_miss_drains_5() {
        assert_eq!(
            hp_delta_for_judgment(DamageLevel::High, JudgmentKind::Miss),
            -5
        );
    }

    #[test]
    fn apply_hp_drain() {
        let mut hp = HpState::new(1000);
        hp.apply(DamageEvent::HpDrain(3));
        assert_eq!(hp.current, 997);
        assert!(!hp.stage_failed);
    }

    #[test]
    fn apply_hp_drain_clamps_to_zero() {
        let mut hp = HpState::new(10);
        hp.apply(DamageEvent::HpDrain(20));
        assert_eq!(hp.current, 0);
        assert!(hp.stage_failed);
    }

    #[test]
    fn apply_hp_gain_clamps_to_max() {
        let mut hp = HpState::new(100);
        hp.apply(DamageEvent::HpGain(50));
        assert_eq!(hp.current, 100);
    }

    #[test]
    fn apply_judgment_perfect_gains() {
        let mut hp = HpState::new(100);
        hp.apply_judgment(DamageLevel::Normal, JudgmentKind::Perfect);
        assert!(hp.current >= 100);
    }

    #[test]
    fn apply_judgment_miss_drains() {
        let mut hp = HpState::new(100);
        hp.apply_judgment(DamageLevel::Normal, JudgmentKind::Miss);
        assert_eq!(hp.current, 97);
    }

    #[test]
    fn stage_failed_when_hp_zero() {
        let mut hp = HpState::new(5);
        hp.apply(DamageEvent::HpDrain(5));
        assert!(hp.stage_failed);
    }

    #[test]
    fn pct_calculation() {
        let hp = HpState {
            current: 50,
            max: 100,
            stage_failed: false,
        };
        assert!((hp.pct() - 50.0).abs() < 0.01);
    }

    #[test]
    fn pct_zero_when_max_zero() {
        let hp = HpState::default();
        assert_eq!(hp.pct(), 0.0);
    }

    #[test]
    fn reset_restores_max() {
        let mut hp = HpState::new(100);
        hp.apply(DamageEvent::HpDrain(50));
        assert_eq!(hp.current, 50);
        hp.reset();
        assert_eq!(hp.current, 100);
        assert!(!hp.stage_failed);
    }

    #[test]
    fn no_change_event_does_nothing() {
        let mut hp = HpState::new(100);
        hp.apply(DamageEvent::NoChange);
        assert_eq!(hp.current, 100);
    }

    #[test]
    fn stage_failed_event_sets_zero() {
        let mut hp = HpState::new(100);
        hp.apply(DamageEvent::StageFailed);
        assert_eq!(hp.current, 0);
        assert!(hp.stage_failed);
    }
}
