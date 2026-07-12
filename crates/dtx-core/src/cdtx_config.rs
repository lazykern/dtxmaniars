//! CDTX config fields + chip timing model (Phase F8).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1070-1080`
//! (configurable per-chart fields).
//!
//! Adds ScoreMode (Type1/Type2/3), LagAdjustment, and `chip_to_ms` helper
//! that converts a chip's (measure, value) to absolute ms using the
//! chart's BPM and any BPM change list.

use crate::chart::Chip;
use crate::constants::RandomMode;
use crate::timing::{chip_time_ms_with_bpm_changes, BpmChange};

/// DTXMania score mode (BocuD `nScoreMode`).
///
/// - Type1: BassDrum × 2, others × 1 (default)
/// - Type2: 5×5 mode (expert/legacy)
/// - Type3: 0-9 mode (drums classic)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScoreMode {
    /// Default scoring: BD × 2, others × 1.
    #[default]
    Type1,
    /// 5×5 mode (BocuD: notes 0-49 + open 0-49).
    Type2,
    /// 0-9 mode (drums classic, BD/LBD × 1).
    Type3,
}

impl ScoreMode {
    /// Parse from a DTX `#SCOREMODE` directive value.
    pub const fn from_dtx_value(v: i32) -> Self {
        match v {
            2 => Self::Type2,
            3 => Self::Type3,
            _ => Self::Type1,
        }
    }

    /// As BocuD int (0/1/2/3 — Type1 maps to 0 or 1).
    pub const fn as_dtx_value(self) -> i32 {
        match self {
            Self::Type1 => 1,
            Self::Type2 => 2,
            Self::Type3 => 3,
        }
    }

    /// Per-chip score multiplier for a given channel.
    /// (BocuD CDTX.cs:nScoreMode-related multipliers.)
    pub fn chip_multiplier(self, channel: crate::channel::EChannel) -> u32 {
        use crate::channel::EChannel;
        match self {
            ScoreMode::Type1 => match channel {
                EChannel::BassDrum | EChannel::LeftBassDrum => 2,
                EChannel::LeftPedal => 2,
                _ => 1,
            },
            ScoreMode::Type2 => 1,
            ScoreMode::Type3 => 1,
        }
    }
}

/// Per-chart config — extracted from DTX directives at parse time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CDtxConfig {
    /// Score mode (BocuD `nScoreMode`).
    pub score_mode: ScoreMode,
    /// Lag adjustment in ms (BocuD `nLagAdjustment`). Applied to every
    /// chip's target_ms as `target_ms += lag_ms`.
    pub lag_ms: i32,
    /// Random mode (BocuD `nRandom`).
    pub random_mode: RandomMode,
    /// Auto-Play flag (BocuD `bAutoPlay`).
    pub auto_play: bool,
    /// Force XG chart interpretation.
    pub force_xg: bool,
    /// Vol 137 → 100 mapping.
    pub vol_137_to_100: bool,
}

impl CDtxConfig {
    /// Construct a config with DTX-Mania defaults.
    pub fn defaults() -> Self {
        Self {
            score_mode: ScoreMode::default(),
            lag_ms: 0,
            random_mode: RandomMode::default(),
            auto_play: false,
            force_xg: false,
            vol_137_to_100: false,
        }
    }
}

/// Convert a chip to its absolute playback time in ms.
///
/// Applies lag adjustment on top of the BPM-aware timing computation.
pub fn chip_to_ms(chip: &Chip, base_bpm: f32, bpm_changes: &[BpmChange], lag_ms: i32) -> i64 {
    let t = chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, bpm_changes);
    t + lag_ms as i64
}

/// Score awarded for a single chip, given score mode + judgment kind.
///
/// (BocuD `CStagePerfCommonScreen.cs:OnChip発声時`.)
pub fn score_for_chip(
    channel: crate::channel::EChannel,
    score_mode: ScoreMode,
    judgment_pts: u32,
) -> u32 {
    score_mode.chip_multiplier(channel) * judgment_pts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::EChannel;
    use crate::constants::RandomMode;

    #[test]
    fn score_mode_default_is_type1() {
        assert_eq!(ScoreMode::default(), ScoreMode::Type1);
    }

    #[test]
    fn score_mode_from_dtx_value() {
        assert_eq!(ScoreMode::from_dtx_value(0), ScoreMode::Type1);
        assert_eq!(ScoreMode::from_dtx_value(1), ScoreMode::Type1);
        assert_eq!(ScoreMode::from_dtx_value(2), ScoreMode::Type2);
        assert_eq!(ScoreMode::from_dtx_value(3), ScoreMode::Type3);
        assert_eq!(ScoreMode::from_dtx_value(99), ScoreMode::Type1);
    }

    #[test]
    fn score_mode_as_dtx_value() {
        assert_eq!(ScoreMode::Type1.as_dtx_value(), 1);
        assert_eq!(ScoreMode::Type2.as_dtx_value(), 2);
        assert_eq!(ScoreMode::Type3.as_dtx_value(), 3);
    }

    #[test]
    fn type1_bass_drum_is_2x() {
        assert_eq!(ScoreMode::Type1.chip_multiplier(EChannel::BassDrum), 2);
    }

    #[test]
    fn type1_left_bass_drum_is_2x() {
        assert_eq!(ScoreMode::Type1.chip_multiplier(EChannel::LeftBassDrum), 2);
    }

    #[test]
    fn type1_left_pedal_is_2x() {
        assert_eq!(ScoreMode::Type1.chip_multiplier(EChannel::LeftPedal), 2);
    }

    #[test]
    fn type1_snare_is_1x() {
        assert_eq!(ScoreMode::Type1.chip_multiplier(EChannel::Snare), 1);
    }

    #[test]
    fn type2_all_1x() {
        assert_eq!(ScoreMode::Type2.chip_multiplier(EChannel::BassDrum), 1);
        assert_eq!(ScoreMode::Type2.chip_multiplier(EChannel::Snare), 1);
    }

    #[test]
    fn type3_all_1x() {
        assert_eq!(ScoreMode::Type3.chip_multiplier(EChannel::BassDrum), 1);
        assert_eq!(ScoreMode::Type3.chip_multiplier(EChannel::Snare), 1);
    }

    #[test]
    fn cdtx_config_defaults() {
        let c = CDtxConfig::defaults();
        assert_eq!(c.score_mode, ScoreMode::Type1);
        assert_eq!(c.lag_ms, 0);
        assert_eq!(c.random_mode, RandomMode::OFF);
        assert!(!c.auto_play);
        assert!(!c.force_xg);
        assert!(!c.vol_137_to_100);
    }

    #[test]
    fn chip_to_ms_at_120bpm() {
        let chip = Chip::new(4, EChannel::BassDrum, 0.0);
        let t = chip_to_ms(&chip, 120.0, &[], 0);
        assert_eq!(t, 8000);
    }

    #[test]
    fn chip_to_ms_with_lag() {
        let chip = Chip::new(4, EChannel::BassDrum, 0.0);
        let t = chip_to_ms(&chip, 120.0, &[], -50);
        assert_eq!(t, 7950);
    }

    #[test]
    fn chip_to_ms_with_lag_positive() {
        let chip = Chip::new(4, EChannel::BassDrum, 0.0);
        let t = chip_to_ms(&chip, 120.0, &[], 25);
        assert_eq!(t, 8025);
    }

    #[test]
    fn chip_to_ms_with_bpm_change() {
        let chip = Chip::new(6, EChannel::BassDrum, 0.0);
        // 120 BPM for [0,2), 240 BPM for [2,6)
        let changes = vec![BpmChange {
            measure: 2,
            bpm: 240.0,
            fraction: 0.0,
        }];
        // [0,2) at 120 = 4000
        // [2,6) at 240 = 4000
        // Total = 8000
        let t = chip_to_ms(&chip, 120.0, &changes, 0);
        assert_eq!(t, 8000);
    }

    #[test]
    fn score_for_chip_bass_drum_2x_perfect() {
        // Perfect = 1000 pts base, BD x2 = 2000
        assert_eq!(
            score_for_chip(EChannel::BassDrum, ScoreMode::Type1, 1000),
            2000
        );
    }

    #[test]
    fn score_for_chip_snare_1x() {
        assert_eq!(
            score_for_chip(EChannel::Snare, ScoreMode::Type1, 1000),
            1000
        );
    }

    #[test]
    fn score_for_chip_type2_no_multiplier() {
        assert_eq!(
            score_for_chip(EChannel::BassDrum, ScoreMode::Type2, 1000),
            1000
        );
    }

    #[test]
    fn score_for_chip_zero_judgment() {
        assert_eq!(score_for_chip(EChannel::BassDrum, ScoreMode::Type1, 0), 0);
    }

    #[test]
    fn cdtx_config_default_struct() {
        let c = CDtxConfig::default();
        assert_eq!(c, CDtxConfig::defaults());
    }
}
