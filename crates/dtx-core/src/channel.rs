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
    BarLine = 0x50,
    BeatLine = 0x51,

    // SE / sfx
    SE01 = 0x61,
    SE02 = 0x62,
    SE03 = 0x63,
    SE04 = 0x64,
    SE05 = 0x65,

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
            0x50 => Self::BarLine,
            0x51 => Self::BeatLine,
            0x54 => Self::Movie,
            0x55 => Self::BGALayer3,
            0x5A => Self::MovieFull,
            0x61 => Self::SE01,
            0x62 => Self::SE02,
            0x63 => Self::SE03,
            0x64 => Self::SE04,
            0x65 => Self::SE05,
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
                | Self::DrumsFillin
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
        assert_eq!(EChannel::from_byte(0xFF), None);
    }

    #[test]
    fn drums_recognized() {
        assert!(EChannel::Snare.is_drum());
        assert!(EChannel::BassDrum.is_drum());
        assert!(!EChannel::BGM.is_drum());
        assert!(!EChannel::GuitarOpen.is_drum());
    }
}
