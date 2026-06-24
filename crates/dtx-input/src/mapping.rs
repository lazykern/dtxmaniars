//! MIDI note → lane helpers + keyboard digit mapping.

/// MIDI drum notes (General MIDI / 5-pin drum kit convention).
pub mod midi_notes {
    /// Bass Drum (kick).
    pub const BD: u8 = 36;
    /// Snare.
    pub const SD: u8 = 38;
    /// Closed Hi-Hat (CH).
    pub const CH: u8 = 42;
    /// Open Hi-Hat (HH open).
    pub const HH_OPEN: u8 = 46;
    /// Alternative closed HH.
    pub const HH_CLOSED_ALT: u8 = 49;
    /// Ride Cymbal.
    pub const RD: u8 = 51;
}

/// Map a MIDI note to a default drum lane (0..8). Returns None for unmapped
/// notes (e.g. toms, cymbals beyond the default kit).
///
/// M6c: only HH/SD/BD + ride mapped. M6.1 adds toms + cymbals.
pub fn midi_note_to_drum_lane(note: u8) -> Option<u8> {
    use midi_notes::*;
    Some(match note {
        BD => 2,                 // 0x13 → lane index 2 (BassDrum)
        SD => 1,                 // 0x12 → Snare
        CH | HH_CLOSED_ALT => 0, // HH close → HiHatClose (lane 0)
        HH_OPEN => 7,            // HiHatOpen (lane 7)
        RD => 8,                 // RideCymbal (lane 8)
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bd_maps_to_lane_2() {
        assert_eq!(midi_note_to_drum_lane(midi_notes::BD), Some(2));
    }

    #[test]
    fn sd_maps_to_lane_1() {
        assert_eq!(midi_note_to_drum_lane(midi_notes::SD), Some(1));
    }

    #[test]
    fn hh_open_maps_to_lane_7() {
        assert_eq!(midi_note_to_drum_lane(midi_notes::HH_OPEN), Some(7));
    }

    #[test]
    fn unknown_note_returns_none() {
        // Tom high (50) not mapped in M6c.
        assert_eq!(midi_note_to_drum_lane(50), None);
    }
}
