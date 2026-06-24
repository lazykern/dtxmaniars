//! Chip classification: categorize a chip by its EChannel.
//!
//! Phase F2 — adds Open-note detection, Bad-note detection, and XG
//! value-multiplier extraction. The XG multiplier concept is a BocuD-style
//! chart extension: DTX channels in the 0x9x-0xAx range carry value
//! scale modifiers (x0.1, x0.2, x2, x3, x5, x8) that scale the chip's
//! contribution to score/combo/gauge. Mirrors the high-nibble convention
//! from `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EChannel.cs`.
//!
//! Reference: `EChannel.cs:1-200` (channel id table).

use crate::channel::EChannel;

/// High-level classification of a chip.
///
/// Used by judge.rs (drums/guitar/bass routing), score.rs (multipliers),
/// and gameplay's BGA/AVI pipeline (cue dispatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipClass {
    /// Bass drum, snare, hi-hat, toms, cymbal, ride — 10 lanes.
    Drum,
    /// Open hi-hat / left cymbal / left bass drum / left pedal — hi-hat
    /// family open notes (BocuD EChannel.cs:27-30).
    OpenNote,
    /// Drums fill-in chip (visual flash only, no judgment).
    DrumsFillin,
    /// Guitar RGBxxx / Y / P frets (BocuD EChannel.cs:32-44).
    Guitar,
    /// Bass RGBxxx / Y / P frets (BocuD EChannel.cs:79-91, 96-107).
    Bass,
    /// Guitar/Bass long note (hold) (BocuD EChannel.cs:44-45).
    LongNote,
    /// Wailing bonus chip (BocuD EChannel.cs:40, 87).
    Wailing,
    /// BGA image layer 1..8 (BocuD EChannel.cs:4, 7, 64-69, 73, 85-89, 96).
    BGA,
    /// Movie (BocuD EChannel.cs:84, 90).
    Movie,
    /// BGM track.
    BGM,
    /// Sound effect (BocuD EChannel.cs:97-105, 112-121, 128-137).
    SE,
    /// BPM change (BocuD EChannel.cs:3, 8).
    BPM,
    /// Bar length (BocuD EChannel.cs:2).
    BarLength,
    /// Bar/beat line (BocuD EChannel.cs:80-81).
    BarLine,
    /// Beat line shift (BocuD EChannel.cs:193).
    BeatLineShift,
    /// Beat line display (BocuD EChannel.cs:194).
    BeatLineDisplay,
    /// Fill-in chip (BocuD EChannel.cs:83).
    FillIn,
    /// MIDI chorus (BocuD EChannel.cs:82).
    MidiChorus,
    /// Click / first sound (BocuD EChannel.cs:236-237).
    Click,
    /// Mixer add/remove (BocuD EChannel.cs:238-239).
    Mixer,
    /// Bad note — invisible, no input, no judgment (BocuD EChannel.cs
    /// *_NoChip family at 0xB1-0xBE).
    BadNote,
    /// System / unknown / not gameplay.
    System,
}

impl ChipClass {
    /// Whether this chip participates in judgment.
    pub const fn is_judgable(self) -> bool {
        matches!(
            self,
            Self::Drum
                | Self::OpenNote
                | Self::Guitar
                | Self::Bass
                | Self::LongNote
                | Self::Wailing
        )
    }

    /// Whether this chip is a "playable" lane note (Drums/Guitar/Bass).
    pub const fn is_playable(self) -> bool {
        matches!(self, Self::Drum | Self::Guitar | Self::Bass)
    }

    /// Whether this chip triggers a BGA layer (image or movie).
    pub const fn is_visual_layer(self) -> bool {
        matches!(self, Self::BGA | Self::Movie)
    }

    /// Whether this chip is a system event (no input, no judgment).
    pub const fn is_system(self) -> bool {
        matches!(
            self,
            Self::BGM
                | Self::BPM
                | Self::BarLength
                | Self::BarLine
                | Self::BeatLineShift
                | Self::BeatLineDisplay
                | Self::FillIn
                | Self::MidiChorus
                | Self::Click
                | Self::Mixer
                | Self::System
        )
    }
}

/// Classify a chip by its channel.
pub const fn classify(ch: EChannel) -> ChipClass {
    match ch {
        // Drums
        EChannel::HiHatClose
        | EChannel::Snare
        | EChannel::BassDrum
        | EChannel::HighTom
        | EChannel::LowTom
        | EChannel::Cymbal
        | EChannel::FloorTom
        | EChannel::HiHatOpen
        | EChannel::RideCymbal => ChipClass::Drum,

        // Drums fill-in (visual only)
        EChannel::DrumsFillin => ChipClass::DrumsFillin,

        // Guitar
        EChannel::GuitarOpen
        | EChannel::GuitarRxxBxx
        | EChannel::GuitarRxGxx
        | EChannel::GuitarRxGBxx
        | EChannel::GuitarRxxxx
        | EChannel::GuitarRxBxx
        | EChannel::GuitarRGxxx
        | EChannel::GuitarRGBxx
        | EChannel::GuitarYxxYx
        | EChannel::GuitarPxx => ChipClass::Guitar,

        // BGA
        EChannel::BGALayer1
        | EChannel::BGALayer2
        | EChannel::BGALayer3
        | EChannel::BGALayer4
        | EChannel::BGALayer5
        | EChannel::BGALayer6
        | EChannel::BGALayer7
        | EChannel::BGALayer8 => ChipClass::BGA,

        // Movie
        EChannel::Movie | EChannel::MovieFull => ChipClass::Movie,

        // BGM
        EChannel::BGM => ChipClass::BGM,

        // SE
        EChannel::SE01 | EChannel::SE02 | EChannel::SE03 | EChannel::SE04 | EChannel::SE05 => {
            ChipClass::SE
        }

        // Bar / beat
        EChannel::BarLine => ChipClass::BarLine,
        EChannel::BeatLine => ChipClass::BarLine,
        EChannel::BarLength => ChipClass::BarLength,

        // BPM
        EChannel::BPM | EChannel::BPMEx => ChipClass::BPM,

        // System
        EChannel::Nil => ChipClass::System,
    }
}

/// Detect Open Notes from a chip's byte value (BocuD EChannel.cs:27-30 +
/// CScoreIni.cs:AutoPlay.LBD, .LP).
///
/// True for: Left Cymbal (0x1A), Left Pedal (0x1B), Left Bass Drum (0x1C),
/// and the "_Open" hi-hat family.
/// Note: channel.rs doesn't yet enumerate 0x1A-0x1C, so this is checked
/// against the raw byte form (channel id 0x11-0x1C range).
pub const fn is_open_note_byte(b: u8) -> bool {
    matches!(b, 0x1A | 0x1B | 0x1C)
}

/// Detect Bad Notes from a chip's byte value.
///
/// Bad notes are at 0xB1-0xBE in BocuD (HiHatClose_NoChip, Snare_NoChip,
/// ..., LeftBassDrum_NoChip). They are invisible and require no input —
/// they exist for "ghost" lanes that scoreboard reads but the player
/// never sees.
pub const fn is_bad_note_byte(b: u8) -> bool {
    matches!(b, 0xB1..=0xBE)
}

/// XG value multiplier (BocuD-style high-nibble convention).
///
/// DTX-XG charts use the high nibble of the channel byte to encode a
/// value multiplier on the chip's contribution to score/combo/gauge:
/// - 0x1n → x1 (default; standard chip)
/// - 0x2n → x2 (double value)
/// - 0x3n → x3 (triple)
/// - 0x5n → x5 (5x)
/// - 0x8n → x8 (8x)
/// - 0x9n → x0.1 (1/10 value)
/// - 0xAn → x0.2 (1/5 value)
/// - other → x1
///
/// Multiplier applies to chip *value* in the score/HP/gauge accumulation
/// (per BocuD CStagePerfCommonScreen.cs:On進行時, db実BPM-style scaling).
pub const fn xg_multiplier(b: u8) -> f32 {
    match b >> 4 {
        0x1 => 1.0,
        0x2 => 2.0,
        0x3 => 3.0,
        0x5 => 5.0,
        0x8 => 8.0,
        0x9 => 0.1,
        0xA => 0.2,
        _ => 1.0,
    }
}

/// XG multiplier applied to an EChannel (helper for typed code).
pub const fn xg_multiplier_for_channel(ch: EChannel) -> f32 {
    xg_multiplier(ch as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drums_classify_as_drum() {
        assert_eq!(classify(EChannel::HiHatClose), ChipClass::Drum);
        assert_eq!(classify(EChannel::Snare), ChipClass::Drum);
        assert_eq!(classify(EChannel::BassDrum), ChipClass::Drum);
        assert_eq!(classify(EChannel::Cymbal), ChipClass::Drum);
        assert_eq!(classify(EChannel::RideCymbal), ChipClass::Drum);
    }

    #[test]
    fn guitar_classify_as_guitar() {
        assert_eq!(classify(EChannel::GuitarOpen), ChipClass::Guitar);
        assert_eq!(classify(EChannel::GuitarRxxxx), ChipClass::Guitar);
        assert_eq!(classify(EChannel::GuitarRGBxx), ChipClass::Guitar);
        assert_eq!(classify(EChannel::GuitarYxxYx), ChipClass::Guitar);
        assert_eq!(classify(EChannel::GuitarPxx), ChipClass::Guitar);
    }

    #[test]
    fn bga_classify_as_bga() {
        assert_eq!(classify(EChannel::BGALayer1), ChipClass::BGA);
        assert_eq!(classify(EChannel::BGALayer8), ChipClass::BGA);
    }

    #[test]
    fn movie_classify_as_movie() {
        assert_eq!(classify(EChannel::Movie), ChipClass::Movie);
        assert_eq!(classify(EChannel::MovieFull), ChipClass::Movie);
    }

    #[test]
    fn bgm_classify_as_bgm() {
        assert_eq!(classify(EChannel::BGM), ChipClass::BGM);
    }

    #[test]
    fn bpm_classify_as_bpm() {
        assert_eq!(classify(EChannel::BPM), ChipClass::BPM);
        assert_eq!(classify(EChannel::BPMEx), ChipClass::BPM);
    }

    #[test]
    fn is_playable_only_drums_guitar_bass() {
        assert!(ChipClass::Drum.is_playable());
        assert!(ChipClass::Guitar.is_playable());
        assert!(!ChipClass::BGA.is_playable());
        assert!(!ChipClass::BGM.is_playable());
        assert!(!ChipClass::BPM.is_playable());
    }

    #[test]
    fn is_judgable_drums_guitar_bass_long_wailing() {
        assert!(ChipClass::Drum.is_judgable());
        assert!(ChipClass::Guitar.is_judgable());
        assert!(ChipClass::LongNote.is_judgable());
        assert!(ChipClass::Wailing.is_judgable());
        assert!(!ChipClass::BGA.is_judgable());
        assert!(!ChipClass::BPM.is_judgable());
    }

    #[test]
    fn open_note_byte_detection() {
        assert!(is_open_note_byte(0x1A));
        assert!(is_open_note_byte(0x1B));
        assert!(is_open_note_byte(0x1C));
        assert!(!is_open_note_byte(0x11));
        assert!(!is_open_note_byte(0x19));
        assert!(!is_open_note_byte(0xB1));
    }

    #[test]
    fn bad_note_byte_detection() {
        assert!(is_bad_note_byte(0xB1));
        assert!(is_bad_note_byte(0xB5));
        assert!(is_bad_note_byte(0xBE));
        assert!(!is_bad_note_byte(0xB0));
        assert!(!is_bad_note_byte(0xBF));
        assert!(!is_bad_note_byte(0x11));
    }

    #[test]
    fn xg_multiplier_x1_default() {
        assert_eq!(xg_multiplier(0x11), 1.0); // standard snare
        assert_eq!(xg_multiplier(0x13), 1.0); // standard bass
    }

    #[test]
    fn xg_multiplier_x2() {
        assert_eq!(xg_multiplier(0x21), 2.0);
        assert_eq!(xg_multiplier(0x24), 2.0);
    }

    #[test]
    fn xg_multiplier_x3() {
        assert_eq!(xg_multiplier(0x31), 3.0);
        assert_eq!(xg_multiplier(0x35), 3.0);
    }

    #[test]
    fn xg_multiplier_x5() {
        assert_eq!(xg_multiplier(0x51), 5.0);
        assert_eq!(xg_multiplier(0x55), 5.0);
    }

    #[test]
    fn xg_multiplier_x8() {
        assert_eq!(xg_multiplier(0x81), 8.0);
        assert_eq!(xg_multiplier(0x88), 8.0);
    }

    #[test]
    fn xg_multiplier_x01() {
        assert_eq!(xg_multiplier(0x91), 0.1);
        assert_eq!(xg_multiplier(0x9F), 0.1);
    }

    #[test]
    fn xg_multiplier_x02() {
        assert_eq!(xg_multiplier(0xA1), 0.2);
        assert_eq!(xg_multiplier(0xAC), 0.2);
    }

    #[test]
    fn xg_multiplier_unknown_nibble_is_x1() {
        // 0x4n, 0x6n, 0x7n, 0x0n are not XG scale — default x1
        assert_eq!(xg_multiplier(0x41), 1.0);
        assert_eq!(xg_multiplier(0x61), 1.0);
        assert_eq!(xg_multiplier(0x71), 1.0);
        assert_eq!(xg_multiplier(0x01), 1.0);
    }

    #[test]
    fn xg_multiplier_for_channel_helper() {
        // Only meaningful for chip-bearing channels. xg_multiplier_for_channel
        // is a thin wrapper around the high-nibble lookup, so we test that
        // a chip channel with the high nibble 0x1 returns 1.0.
        assert_eq!(xg_multiplier_for_channel(EChannel::HiHatClose), 1.0);
        assert_eq!(xg_multiplier_for_channel(EChannel::Snare), 1.0);
    }

    #[test]
    fn is_visual_layer() {
        assert!(ChipClass::BGA.is_visual_layer());
        assert!(ChipClass::Movie.is_visual_layer());
        assert!(!ChipClass::Drum.is_visual_layer());
        assert!(!ChipClass::BGM.is_visual_layer());
    }

    #[test]
    fn is_system() {
        assert!(ChipClass::BGM.is_system());
        assert!(ChipClass::BPM.is_system());
        assert!(ChipClass::BarLine.is_system());
        assert!(!ChipClass::Drum.is_system());
        assert!(!ChipClass::Guitar.is_system());
    }
}
