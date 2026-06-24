//! `CConstants` — port of `references/DTXmaniaNX-BocuD/DTXMania/Core/CConstants.cs` (780 LoC).
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! Central enums for game configuration: lane type, dark mode, damage
//! level, random mode, instrument part, pad, lane, judgement, etc.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConstants.cs:1-780`

#![allow(non_camel_case_types)] // Verbatim port of C# PascalCase enum variants.

/// Lane type A/B/C/D/E (BocuD `EType` CConstants.cs:23-29).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LaneType {
    /// Type A (default 9-lane classic).
    #[default]
    A,
    /// Type B.
    B,
    /// Type C.
    C,
    /// Type D.
    D,
    /// Type E.
    E,
}

impl LaneType {
    /// All 5 types in reference order.
    pub fn all() -> [Self; 5] {
        [Self::A, Self::B, Self::C, Self::D, Self::E]
    }
}

/// Ride Cymbal position (BocuD `ERDPosition` CConstants.cs:31-35).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RdPosition {
    /// RD=RC position (default).
    #[default]
    RDRC,
    /// RD=CR position (swapped).
    RCRD,
}

/// Dark mode (BocuD `EDarkMode` CConstants.cs:37-42).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DarkMode {
    /// Dark mode off.
    #[default]
    OFF,
    /// Half dark mode.
    HALF,
    /// Full dark mode.
    FULL,
}

/// Damage level (BocuD `EDamageLevel` CConstants.cs:44-48).
///
/// 4 levels per Phase F6 goal spec:
/// - `None`: no damage taken at all (player can play with HP=0 forever)
/// - `Small`: minimal drain on miss (default — matches BocuD `Small = 0`)
/// - `Normal`: standard drain (matches BocuD `Normal = 1`)
/// - `High`: heavy drain + Extreme behavior (matches BocuD `High = 2`)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DamageLevel {
    /// No damage (Extreme behavior: HP=0 doesn't fail the song).
    None,
    /// Small damage (BocuD `Small = 0`).
    #[default]
    Small,
    /// Normal damage (BocuD `Normal = 1`).
    Normal,
    /// High damage (BocuD `High = 2`).
    High,
}

impl DamageLevel {
    /// Numeric value (BocuD `Small=0, Normal=1, High=2`).
    pub fn value(self) -> u8 {
        match self {
            Self::None => 255,
            Self::Small => 0,
            Self::Normal => 1,
            Self::High => 2,
        }
    }
}

/// Random mode (BocuD `ERandomMode` CConstants.cs:148-157).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RandomMode {
    /// No random.
    #[default]
    OFF,
    /// Mirror lanes.
    MIRROR,
    /// Random lanes.
    RANDOM,
    /// Super random.
    SUPERRANDOM,
    /// Hyper random.
    HYPERRANDOM,
    /// Master random.
    MASTERRANDOM,
    /// Another random mode.
    ANOTHERRANDOM,
}

/// Instrument part (BocuD `EInstrumentPart` CConstants.cs:159-166).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InstrumentPart {
    /// Drums.
    #[default]
    Drums,
    /// Guitar.
    Guitar,
    /// Bass.
    Bass,
    /// Unknown / unset.
    Unknown,
}

impl InstrumentPart {
    /// Numeric value (BocuD `DRUMS=0, GUITAR=1, BASS=2, UNKNOWN=99`).
    pub fn value(self) -> i32 {
        match self {
            Self::Drums => 0,
            Self::Guitar => 1,
            Self::Bass => 2,
            Self::Unknown => 99,
        }
    }
}

/// Judgement rank (BocuD `EJudgement` CConstants.cs:191-200).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Judgement {
    /// Perfect hit.
    #[default]
    Perfect,
    /// Great hit.
    Great,
    /// Good hit.
    Good,
    /// Poor hit.
    Poor,
    /// Miss.
    Miss,
    /// Bad (worse than miss).
    Bad,
    /// Auto-typed.
    Auto,
}

impl Judgement {
    /// Numeric value (BocuD `Perfect=0, Great=1, Good=2, Poor=3, Miss=4, Bad=5`).
    pub fn value(self) -> i32 {
        match self {
            Self::Perfect => 0,
            Self::Great => 1,
            Self::Good => 2,
            Self::Poor => 3,
            Self::Miss => 4,
            Self::Bad => 5,
            Self::Auto => 6,
        }
    }
}

/// Lane (BocuD `ELane` CConstants.cs:223-247).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Left Cymbal.
    #[default]
    LC = 0,
    /// Hi-Hat closed.
    HH,
    /// Snare Drum.
    SD,
    /// Bass Drum.
    BD,
    /// High Tom.
    HT,
    /// Low Tom.
    LT,
    /// Floor Tom.
    FT,
    /// Cymbal.
    CY,
    /// Left Pedal.
    LP,
    /// Ride Cymbal.
    RD,
    /// Left Bass Drum (BocuD `LBD`).
    LBD,
    /// Guitar.
    Guitar,
    /// Bass.
    Bass,
    /// Guitar R fret.
    GtR,
    /// Guitar G fret.
    GtG,
    /// Guitar B fret.
    GtB,
    /// Guitar Y fret.
    GtY,
    /// Guitar P fret.
    GtP,
}

impl Lane {
    /// Numeric value.
    pub fn value(self) -> i32 {
        match self {
            Self::LC => 0,
            Self::HH => 1,
            Self::SD => 2,
            Self::BD => 3,
            Self::HT => 4,
            Self::LT => 5,
            Self::FT => 6,
            Self::CY => 7,
            Self::LP => 8,
            Self::RD => 9,
            Self::LBD => 10,
            Self::Guitar => 11,
            Self::Bass => 12,
            Self::GtR => 13,
            Self::GtG => 14,
            Self::GtB => 15,
            Self::GtY => 16,
            Self::GtP => 17,
        }
    }
}

/// Cymbal group separation (BocuD `ECYGroup` CConstants.cs:3-7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CYGroup {
    /// Separate cymbals.
    #[default]
    Separate,
    /// Common cymbal.
    Common,
}

/// Floor tom group separation (BocuD `EFTGroup` CConstants.cs:8-12).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FTGroup {
    /// Separate.
    #[default]
    Separate,
    /// Common.
    Common,
}

/// Hi-Hat group separation (BocuD `EHHGroup` CConstants.cs:13-18).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum HHGroup {
    /// All separate.
    #[default]
    AllSeparate,
    /// Hi-Hat only separate.
    HHOnly,
    /// Left cymbal only separate.
    LCOnly,
    /// All common.
    AllCommon,
}

/// Bass drum group separation (BocuD `EBDGroup` CConstants.cs:19-25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BDGroup {
    /// Separate.
    #[default]
    Separate,
    /// BD and LP separate.
    BDAndLP,
    /// Left/right pedals only separate.
    LRPedalsOnly,
    /// Both are BD.
    BothBD,
}

/// Playback priority (BocuD `EPlaybackPriority` CConstants.cs:185-189).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PlaybackPriority {
    /// Chip over pad (BocuD `ChipOverPadPriority`).
    #[default]
    ChipOverPad,
    /// Pad over chip (BocuD `PadOverChipPriority`).
    PadOverChip,
}

#[cfg(test)]
mod tests {
    use super::*;

    // === LaneType ===

    #[test]
    fn lane_type_all_has_5() {
        assert_eq!(LaneType::all().len(), 5);
    }

    #[test]
    fn lane_type_default_is_a() {
        assert_eq!(LaneType::default(), LaneType::A);
    }

    // === DamageLevel ===

    #[test]
    fn damage_level_values_match_reference() {
        // CConstants.cs:44-48 — Small=0, Normal=1, High=2
        assert_eq!(DamageLevel::Small.value(), 0);
        assert_eq!(DamageLevel::Normal.value(), 1);
        assert_eq!(DamageLevel::High.value(), 2);
        // None = no damage level (sentinel 255, doesn't match reference)
        assert_eq!(DamageLevel::None.value(), 255);
    }

    // === InstrumentPart ===

    #[test]
    fn instrument_part_values_match_reference() {
        // CConstants.cs:159-166 — DRUMS=0, GUITAR=1, BASS=2, UNKNOWN=99
        assert_eq!(InstrumentPart::Drums.value(), 0);
        assert_eq!(InstrumentPart::Guitar.value(), 1);
        assert_eq!(InstrumentPart::Bass.value(), 2);
        assert_eq!(InstrumentPart::Unknown.value(), 99);
    }

    // === Judgement ===

    #[test]
    fn judgement_values_match_reference() {
        // CConstants.cs:191-200
        assert_eq!(Judgement::Perfect.value(), 0);
        assert_eq!(Judgement::Great.value(), 1);
        assert_eq!(Judgement::Good.value(), 2);
        assert_eq!(Judgement::Poor.value(), 3);
        assert_eq!(Judgement::Miss.value(), 4);
        assert_eq!(Judgement::Bad.value(), 5);
    }

    // === Lane ===

    #[test]
    fn lane_lc_value_zero() {
        // CConstants.cs:225
        assert_eq!(Lane::LC.value(), 0);
    }

    #[test]
    fn lane_hh_value_one() {
        assert_eq!(Lane::HH.value(), 1);
    }

    #[test]
    fn lane_rd_value_nine() {
        // CConstants.cs:233
        assert_eq!(Lane::RD.value(), 9);
    }

    // === DarkMode / RandomMode ===

    #[test]
    fn dark_mode_default_off() {
        assert_eq!(DarkMode::default(), DarkMode::OFF);
    }

    #[test]
    fn random_mode_default_off() {
        assert_eq!(RandomMode::default(), RandomMode::OFF);
    }

    #[test]
    fn random_mode_has_seven_variants() {
        let all = [
            RandomMode::OFF,
            RandomMode::MIRROR,
            RandomMode::RANDOM,
            RandomMode::SUPERRANDOM,
            RandomMode::HYPERRANDOM,
            RandomMode::MASTERRANDOM,
            RandomMode::ANOTHERRANDOM,
        ];
        assert_eq!(all.len(), 7);
    }

    // === HHGroup / BDGroup ===

    #[test]
    fn hh_group_default_all_separate() {
        assert_eq!(HHGroup::default(), HHGroup::AllSeparate);
    }

    #[test]
    fn bd_group_default_separate() {
        assert_eq!(BDGroup::default(), BDGroup::Separate);
    }
}
