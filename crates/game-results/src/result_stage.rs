//! `CStageResult` — port of `Stage/07.Result/CStageResult.cs` (811 LOC).
//!
//! Strict-port-first. Result screen orchestrator.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/CStageResult.cs:1-811`

use bevy::prelude::Resource;

/// Result screen size (CStageResult.cs).
pub const RESULT_SCREEN_W: f32 = 1280.0;
pub const RESULT_SCREEN_H: f32 = 720.0;

/// DGB = Drums/Guitar/Bass — 3 instruments.
pub const DGB_INSTRUMENTS: usize = 3;
/// 5 judgment kinds (Perfect/Great/Good/Poor/Miss).
pub const JUDGMENT_KINDS: usize = 5;
/// 6 ranks (S/A/B/C/D/E).
pub const RANK_COUNT: usize = 6;
/// 7 status icons (FullCombo/Excellent/Good/etc.).
pub const STATUS_ICON_COUNT: usize = 7;

/// Result state — matches CStageResult fields.
#[derive(Resource, Debug, Default, Clone)]
pub struct CStageResultState {
    /// New record flags (per instrument).
    pub new_record_skill: [bool; DGB_INSTRUMENTS],
    pub new_record_score: [bool; DGB_INSTRUMENTS],
    pub new_record_rank: [bool; DGB_INSTRUMENTS],
    /// Percentage per judgment (per instrument).
    pub perfect_pct: [f32; DGB_INSTRUMENTS],
    pub great_pct: [f32; DGB_INSTRUMENTS],
    pub good_pct: [f32; DGB_INSTRUMENTS],
    pub poor_pct: [f32; DGB_INSTRUMENTS],
    pub miss_pct: [f32; DGB_INSTRUMENTS],
    /// Auto-play flag per instrument (CStageResult.cs:bAuto).
    pub auto: [bool; DGB_INSTRUMENTS],
    /// Rank value per instrument (CStageResult.cs:nRankValue).
    pub rank_value: [i32; DGB_INSTRUMENTS],
    /// Overall result rank (CStageResult.cs:nResultRank).
    pub result_rank: i32,
    /// Training mode flag.
    pub is_training_mode: bool,
}

impl CStageResultState {
    pub fn new() -> Self {
        Self {
            new_record_skill: [false; DGB_INSTRUMENTS],
            new_record_score: [false; DGB_INSTRUMENTS],
            new_record_rank: [false; DGB_INSTRUMENTS],
            perfect_pct: [0.0; DGB_INSTRUMENTS],
            great_pct: [0.0; DGB_INSTRUMENTS],
            good_pct: [0.0; DGB_INSTRUMENTS],
            poor_pct: [0.0; DGB_INSTRUMENTS],
            miss_pct: [0.0; DGB_INSTRUMENTS],
            auto: [false; DGB_INSTRUMENTS],
            rank_value: [0; DGB_INSTRUMENTS],
            result_rank: 0,
            is_training_mode: false,
        }
    }

    /// Sum of percentages for one instrument (should be ~100).
    pub fn total_pct(&self, instrument: usize) -> f32 {
        if instrument >= DGB_INSTRUMENTS {
            return 0.0;
        }
        self.perfect_pct[instrument]
            + self.great_pct[instrument]
            + self.good_pct[instrument]
            + self.poor_pct[instrument]
            + self.miss_pct[instrument]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_screen_size() {
        assert_eq!(RESULT_SCREEN_W, 1280.0);
        assert_eq!(RESULT_SCREEN_H, 720.0);
    }

    #[test]
    fn dgb_instruments() {
        // CStageResult.cs uses STDGBVALUE (Drums/Guitar/Bass)
        assert_eq!(DGB_INSTRUMENTS, 3);
    }

    #[test]
    fn rank_count() {
        // 6 ranks (S/A/B/C/D/E)
        assert_eq!(RANK_COUNT, 6);
    }

    #[test]
    fn status_icon_count() {
        assert_eq!(STATUS_ICON_COUNT, 7);
    }

    #[test]
    fn result_state_default() {
        let s = CStageResultState::default();
        assert!(!s.new_record_skill[0]);
        assert!(!s.is_training_mode);
        assert_eq!(s.result_rank, 0);
    }

    #[test]
    fn result_state_new() {
        let s = CStageResultState::new();
        assert_eq!(s.perfect_pct.len(), 3);
    }

    #[test]
    fn total_pct_at_default_is_zero() {
        let s = CStageResultState::new();
        assert_eq!(s.total_pct(0), 0.0);
    }

    #[test]
    fn total_pct_out_of_range() {
        let s = CStageResultState::new();
        assert_eq!(s.total_pct(99), 0.0);
    }

    #[test]
    fn total_pct_sums() {
        let mut s = CStageResultState::new();
        s.perfect_pct[0] = 50.0;
        s.great_pct[0] = 30.0;
        s.good_pct[0] = 15.0;
        s.poor_pct[0] = 3.0;
        s.miss_pct[0] = 2.0;
        assert!((s.total_pct(0) - 100.0).abs() < 0.01);
    }
}
