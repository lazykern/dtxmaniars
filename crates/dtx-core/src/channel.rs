//! DTX channel identifiers.
//!
//! Ported from `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EChannel.cs`.
//! Covers the subset we need for drums (M2) and leaves guitar/bass available
//! for M6+ (parsed but unused by gameplay).

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EChannel {
    Nil = 0,
    BGM = 1,
    BarLength = 2,
    BPM = 3,
    BGALayer1 = 4,
    BGALayer2 = 7,
    BPMEx = 8,

    // Drums
    HiHatClose = 0x11,
    Snare = 0x12,
    BassDrum = 0x13,
    HighTom = 0x14,
    LowTom = 0x15,
    Cymbal = 0x16,
    FloorTom = 0x17,
    HiHatOpen = 0x18,
    RideCymbal = 0x19,
    // Open hi-hat family (BocuD EChannel.cs:27-30)
    LeftCymbal = 0x1A,
    LeftPedal = 0x1B,
    LeftBassDrum = 0x1C,

    // Drums fill-in
    DrumsFillin = 0x1F,

    // Guitar open / RGBxxx pattern
    GuitarOpen = 0x20,
    GuitarRxxBxx = 0x21,
    GuitarRxGxx = 0x22,
    GuitarRxGBxx = 0x23,
    GuitarRxxxx = 0x24,
    GuitarRxBxx = 0x25,
    GuitarRGxxx = 0x26,
    GuitarRGBxx = 0x27,
    // Y / P frets (BocuD EChannel.cs lines 109-131)
    GuitarYxxYx = 0x93,
    GuitarPxx = 0xA3,

    // BGA image/video layers (BocuD EChannel.cs lines 10-13, 64-76)
    BGALayer4 = 0x56,
    BGALayer5 = 0x57,
    BGALayer6 = 0x58,
    BGALayer7 = 0x59,
    BGALayer8 = 0x60,

    // Bar / Beat lines
    // Bonus-effect markers. These mark matching drum chips and reduce the XG
    // base-score pool by 500 each (BocuD EChannel.cs:56-59).
    BonusEffect1 = 0x4C,
    BonusEffect2 = 0x4D,
    BonusEffect3 = 0x4E,
    BonusEffect4 = 0x4F,
    BarLine = 0x50,
    BeatLine = 0x51,
    BeatLineShift = 0xC1,
    BeatLineDisplay = 0xC2,

    // SE / sfx
    SE01 = 0x61,
    SE02 = 0x62,
    SE03 = 0x63,
    SE04 = 0x64,
    SE05 = 0x65,
    SE06 = 0x66,
    SE07 = 0x67,
    SE08 = 0x68,
    SE09 = 0x69,
    SE10 = 0x70,
    SE11 = 0x71,
    SE12 = 0x72,
    SE13 = 0x73,
    SE14 = 0x74,
    SE15 = 0x75,
    SE16 = 0x76,
    SE17 = 0x77,
    SE18 = 0x78,
    SE19 = 0x79,
    SE20 = 0x80,
    SE21 = 0x81,
    SE22 = 0x82,
    SE23 = 0x83,
    SE24 = 0x84,
    SE25 = 0x85,
    SE26 = 0x86,
    SE27 = 0x87,
    SE28 = 0x88,
    SE29 = 0x89,
    SE30 = 0x90,
    SE31 = 0x91,
    SE32 = 0x92,

    // BGA / Movie (parsed but not rendered in M2)
    Movie = 0x54,
    BGALayer3 = 0x55,
    MovieFull = 0x5A,
}

impl EChannel {
    pub const fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Self::Nil,
            1 => Self::BGM,
            2 => Self::BarLength,
            3 => Self::BPM,
            4 => Self::BGALayer1,
            7 => Self::BGALayer2,
            8 => Self::BPMEx,
            0x11 => Self::HiHatClose,
            0x12 => Self::Snare,
            0x13 => Self::BassDrum,
            0x14 => Self::HighTom,
            0x15 => Self::LowTom,
            0x16 => Self::Cymbal,
            0x17 => Self::FloorTom,
            0x18 => Self::HiHatOpen,
            0x19 => Self::RideCymbal,
            0x1A => Self::LeftCymbal,
            0x1B => Self::LeftPedal,
            0x1C => Self::LeftBassDrum,
            0x1F => Self::DrumsFillin,
            0x20 => Self::GuitarOpen,
            0x21 => Self::GuitarRxxBxx,
            0x22 => Self::GuitarRxGxx,
            0x23 => Self::GuitarRxGBxx,
            0x24 => Self::GuitarRxxxx,
            0x25 => Self::GuitarRxBxx,
            0x26 => Self::GuitarRGxxx,
            0x27 => Self::GuitarRGBxx,
            0x93 => {
                Self::// Renamed: was Guitar_xxxYx
    GuitarYxxYx
            }
            0xA3 => {
                Self::// Renamed: was Guitar_xxxxP
    GuitarPxx
            }
            0x56 => Self::BGALayer4,
            0x57 => Self::BGALayer5,
            0x58 => Self::BGALayer6,
            0x59 => Self::BGALayer7,
            0x60 => Self::BGALayer8,
            0x4C => Self::BonusEffect1,
            0x4D => Self::BonusEffect2,
            0x4E => Self::BonusEffect3,
            0x4F => Self::BonusEffect4,
            0x50 => Self::BarLine,
            0x51 => Self::BeatLine,
            0xC1 => Self::BeatLineShift,
            0xC2 => Self::BeatLineDisplay,
            0x54 => Self::Movie,
            0x55 => Self::BGALayer3,
            0x5A => Self::MovieFull,
            0x61 => Self::SE01,
            0x62 => Self::SE02,
            0x63 => Self::SE03,
            0x64 => Self::SE04,
            0x65 => Self::SE05,
            0x66 => Self::SE06,
            0x67 => Self::SE07,
            0x68 => Self::SE08,
            0x69 => Self::SE09,
            0x70 => Self::SE10,
            0x71 => Self::SE11,
            0x72 => Self::SE12,
            0x73 => Self::SE13,
            0x74 => Self::SE14,
            0x75 => Self::SE15,
            0x76 => Self::SE16,
            0x77 => Self::SE17,
            0x78 => Self::SE18,
            0x79 => Self::SE19,
            0x80 => Self::SE20,
            0x81 => Self::SE21,
            0x82 => Self::SE22,
            0x83 => Self::SE23,
            0x84 => Self::SE24,
            0x85 => Self::SE25,
            0x86 => Self::SE26,
            0x87 => Self::SE27,
            0x88 => Self::SE28,
            0x89 => Self::SE29,
            0x90 => Self::SE30,
            0x91 => Self::SE31,
            0x92 => Self::SE32,
            _ => return None,
        })
    }

    /// Short display/config name for drum channels ("HH", "HHO", …).
    /// None for non-drum channels. Matches dtx-layout lane ids.
    pub const fn short_name(self) -> Option<&'static str> {
        Some(match self {
            Self::HiHatClose => "HH",
            Self::Snare => "SD",
            Self::BassDrum => "BD",
            Self::HighTom => "HT",
            Self::LowTom => "LT",
            Self::Cymbal => "CY",
            Self::FloorTom => "FT",
            Self::HiHatOpen => "HHO",
            Self::RideCymbal => "RD",
            Self::LeftCymbal => "LC",
            Self::LeftPedal => "LP",
            Self::LeftBassDrum => "LBD",
            _ => return None,
        })
    }

    /// Inverse of [`short_name`].
    pub fn from_short_name(name: &str) -> Option<Self> {
        Some(match name {
            "HH" => Self::HiHatClose,
            "SD" => Self::Snare,
            "BD" => Self::BassDrum,
            "HT" => Self::HighTom,
            "LT" => Self::LowTom,
            "CY" => Self::Cymbal,
            "FT" => Self::FloorTom,
            "HHO" => Self::HiHatOpen,
            "RD" => Self::RideCymbal,
            "LC" => Self::LeftCymbal,
            "LP" => Self::LeftPedal,
            "LBD" => Self::LeftBassDrum,
            _ => return None,
        })
    }

    pub const fn is_drum(self) -> bool {
        matches!(
            self,
            Self::HiHatClose
                | Self::Snare
                | Self::BassDrum
                | Self::HighTom
                | Self::LowTom
                | Self::Cymbal
                | Self::FloorTom
                | Self::HiHatOpen
                | Self::RideCymbal
                | Self::LeftCymbal
                | Self::LeftPedal
                | Self::LeftBassDrum
                | Self::DrumsFillin
        )
    }

    /// True for DTXManiaNX chart-timed sound-effect channels SE01 through SE32.
    pub const fn is_se(self) -> bool {
        matches!(
            self as u8,
            0x61..=0x69 | 0x70..=0x79 | 0x80..=0x89 | 0x90..=0x92
        )
    }

    /// True for DTXManiaNX bonus-effect marker channels 0x4C..=0x4F.
    pub const fn is_bonus_effect(self) -> bool {
        matches!(
            self,
            Self::BonusEffect1 | Self::BonusEffect2 | Self::BonusEffect3 | Self::BonusEffect4
        )
    }

    pub const fn is_guitar(self) -> bool {
        matches!(
            self,
            Self::GuitarOpen
                | Self::GuitarRxxBxx
                | Self::GuitarRxGxx
                | Self::GuitarRxGBxx
                | Self::GuitarRxxxx
                | Self::GuitarRxBxx
                | Self::GuitarRGxxx
                | Self::GuitarRGBxx
                | Self::GuitarYxxYx
                | Self::GuitarPxx
        )
    }

    pub const fn is_bga(self) -> bool {
        matches!(
            self,
            Self::BGALayer1
                | Self::BGALayer2
                | Self::BGALayer3
                | Self::BGALayer4
                | Self::BGALayer5
                | Self::BGALayer6
                | Self::BGALayer7
                | Self::BGALayer8
                | Self::Movie
                | Self::MovieFull
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known_channels() {
        assert_eq!(EChannel::from_byte(0x13), Some(EChannel::BassDrum));
        assert_eq!(EChannel::from_byte(0x11), Some(EChannel::HiHatClose));
        assert_eq!(EChannel::from_byte(0x4C), Some(EChannel::BonusEffect1));
        assert_eq!(EChannel::from_byte(0xFF), None);
    }

    #[test]
    fn all_nx_se_channel_values_round_trip() {
        let values = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76, 0x77, 0x78, 0x79, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88,
            0x89, 0x90, 0x91, 0x92,
        ];
        for value in values {
            let channel = EChannel::from_byte(value).expect("known NX SE channel");
            assert!(channel.is_se(), "0x{value:02X}");
            assert_eq!(channel as u8, value);
        }
        assert!(!EChannel::BGM.is_se());
        assert!(!EChannel::BassDrum.is_se());
    }

    #[test]
    fn bonus_effect_channels_are_recognized_without_becoming_drum_notes() {
        assert!(EChannel::BonusEffect1.is_bonus_effect());
        assert!(EChannel::BonusEffect4.is_bonus_effect());
        assert!(!EChannel::BonusEffect1.is_drum());
    }

    #[test]
    fn drums_recognized() {
        assert!(EChannel::Snare.is_drum());
        assert!(EChannel::BassDrum.is_drum());
        assert!(!EChannel::BGM.is_drum());
        assert!(!EChannel::GuitarOpen.is_drum());
    }

    #[test]
    fn drum_short_names_round_trip() {
        use EChannel::*;
        for ch in [
            HiHatClose,
            Snare,
            BassDrum,
            HighTom,
            LowTom,
            Cymbal,
            FloorTom,
            HiHatOpen,
            RideCymbal,
            LeftCymbal,
            LeftPedal,
            LeftBassDrum,
        ] {
            let name = ch.short_name().expect("drum channel has short name");
            assert_eq!(EChannel::from_short_name(name), Some(ch));
        }
    }

    #[test]
    fn short_name_values_match_layout_convention() {
        assert_eq!(EChannel::HiHatClose.short_name(), Some("HH"));
        assert_eq!(EChannel::HiHatOpen.short_name(), Some("HHO"));
        assert_eq!(EChannel::LeftBassDrum.short_name(), Some("LBD"));
    }

    #[test]
    fn non_drum_channel_has_no_short_name() {
        assert_eq!(EChannel::BGM.short_name(), None);
        assert_eq!(EChannel::from_short_name("NOPE"), None);
    }
}
