//! `CChip` (644 LOC) — chip with rich metadata.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CChip.cs:1-644`
//!
//! v1 strict-port: chip with channel + value + state for playback.
//! Models the runtime chip list that drives judgment + visualization.

use crate::channel::EChannel;

/// Maximum chip count per chart (BocuD CChip.cs:20).
pub const MAX_CHIPS_PER_CHART: usize = 65535;

/// Chip playback state (BocuD CChip.cs:30-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChipState {
    /// Not yet eligible for playback (waiting for time).
    #[default]
    Pending = 0,
    /// Active — visible + can be hit.
    Active = 1,
    /// Already judged (hit or missed).
    Judged = 2,
    /// Disabled (skipped, e.g. due to a render error).
    Disabled = 3,
}

impl ChipState {
    pub fn as_int(&self) -> i32 {
        *self as i32
    }
    pub fn from_int(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Pending),
            1 => Some(Self::Active),
            2 => Some(Self::Judged),
            3 => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// One CChip (BocuD CChip.cs:60-100).
///
/// Per-chip data fields (12 total):
///   nChannel, nValue, nPlaybackTimeMs, nPlaybackPosition, nDistanceFromBar,
///   nTotalRollDistance, nTotalChipDistance, nMoveDelta, nMoveDeltaSum,
///   eChipState, bVisible, bProcessed
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CChip {
    /// Channel id (EChannel).
    pub nChannel: EChannel,
    /// Raw value (WAV idx for SE, BGA idx for BGA, fraction for binary, etc.).
    pub nValue: i32,
    /// Absolute playback time in ms.
    pub nPlaybackTimeMs: i64,
    /// Current playback position (0-100% of measure).
    pub nPlaybackPosition: f32,
    /// Distance from the judgment bar (BocuD CChip.cs:nDistanceFromBar).
    pub nDistanceFromBar: f32,
    /// Roll note total distance (BocuD CChip.cs:nTotalRollDistance).
    pub nTotalRollDistance: f32,
    /// Total chip distance (BocuD CChip.cs:nTotalChipDistance).
    pub nTotalChipDistance: f32,
    /// Movement delta this frame.
    pub nMoveDelta: f32,
    /// Cumulative movement delta sum.
    pub nMoveDeltaSum: f32,
    /// Playback state.
    pub eChipState: ChipState,
    /// Visibility flag.
    pub bVisible: bool,
    /// Whether chip has been processed by judgment.
    pub bProcessed: bool,
}

impl Default for CChip {
    fn default() -> Self {
        Self {
            nChannel: EChannel::BGM,
            nValue: 0,
            nPlaybackTimeMs: 0,
            nPlaybackPosition: 0.0,
            nDistanceFromBar: 0.0,
            nTotalRollDistance: 0.0,
            nTotalChipDistance: 0.0,
            nMoveDelta: 0.0,
            nMoveDeltaSum: 0.0,
            eChipState: ChipState::Pending,
            bVisible: true,
            bProcessed: false,
        }
    }
}

impl CChip {
    /// Build a chip at a specific time and channel.
    pub fn at(nChannel: EChannel, nPlaybackTimeMs: i64) -> Self {
        Self {
            nChannel,
            nPlaybackTimeMs,
            ..Default::default()
        }
    }

    /// Number of populated fields (BocuD CChip.cs:GetFieldCount = 12).
    pub fn field_count() -> usize {
        12
    }

    /// Whether this chip is a drum note (used for judgment routing).
    pub fn is_drum(&self) -> bool {
        self.nChannel.is_drum()
    }

    /// Whether this chip is a guitar/bass note.
    pub fn is_guitar(&self) -> bool {
        self.nChannel.is_guitar()
    }

    /// Whether this chip is BGM.
    pub fn is_bgm(&self) -> bool {
        self.nChannel == EChannel::BGM
    }

    /// Whether this chip is a BGA layer (1..8) or movie.
    pub fn is_bga(&self) -> bool {
        self.nChannel.is_bga()
    }

    /// Whether this chip is BPM-related.
    pub fn is_bpm(&self) -> bool {
        matches!(self.nChannel, EChannel::BPM | EChannel::BPMEx)
    }

    /// Mark as judged.
    pub fn mark_judged(&mut self) {
        self.eChipState = ChipState::Judged;
        self.bProcessed = true;
    }

    /// Mark as missed.
    pub fn mark_missed(&mut self) {
        self.eChipState = ChipState::Judged;
        self.bProcessed = true;
    }

    /// Hide chip.
    pub fn hide(&mut self) {
        self.bVisible = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_chip_default() {
        let c = CChip::default();
        assert_eq!(c.eChipState, ChipState::Pending);
        assert!(c.bVisible);
        assert!(!c.bProcessed);
        assert_eq!(c.nPlaybackTimeMs, 0);
    }

    #[test]
    fn c_chip_field_count_12() {
        // Reference: CChip.cs:GetFieldCount = 12
        assert_eq!(CChip::field_count(), 12);
    }

    #[test]
    fn c_chip_at_helper() {
        let c = CChip::at(EChannel::BassDrum, 1234);
        assert_eq!(c.nChannel, EChannel::BassDrum);
        assert_eq!(c.nPlaybackTimeMs, 1234);
    }

    #[test]
    fn max_chips_per_chart() {
        // Reference: CChip.cs:20
        assert_eq!(MAX_CHIPS_PER_CHART, 65535);
    }

    #[test]
    fn chip_state_round_trip() {
        for s in [
            ChipState::Pending,
            ChipState::Active,
            ChipState::Judged,
            ChipState::Disabled,
        ] {
            assert_eq!(ChipState::from_int(s.as_int()), Some(s));
        }
        assert_eq!(ChipState::from_int(99), None);
    }

    #[test]
    fn c_chip_mark_judged() {
        let mut c = CChip::default();
        c.mark_judged();
        assert_eq!(c.eChipState, ChipState::Judged);
        assert!(c.bProcessed);
    }

    #[test]
    fn c_chip_mark_missed() {
        let mut c = CChip::default();
        c.mark_missed();
        assert_eq!(c.eChipState, ChipState::Judged);
    }

    #[test]
    fn c_chip_hide() {
        let mut c = CChip::default();
        c.hide();
        assert!(!c.bVisible);
    }

    #[test]
    fn c_chip_is_drum_guitar_bgm_bga() {
        assert!(CChip::at(EChannel::BassDrum, 0).is_drum());
        assert!(!CChip::at(EChannel::BassDrum, 0).is_guitar());
        assert!(CChip::at(EChannel::GuitarRxxxx, 0).is_guitar());
        assert!(!CChip::at(EChannel::GuitarRxxxx, 0).is_drum());
        assert!(CChip::at(EChannel::BGM, 0).is_bgm());
        assert!(CChip::at(EChannel::BGALayer1, 0).is_bga());
        assert!(!CChip::at(EChannel::BGM, 0).is_bga());
    }

    #[test]
    fn c_chip_is_bpm() {
        assert!(CChip::at(EChannel::BPM, 0).is_bpm());
        assert!(CChip::at(EChannel::BPMEx, 0).is_bpm());
        assert!(!CChip::at(EChannel::BassDrum, 0).is_bpm());
    }
}
