use crate::channel::EChannel;

/// Metadata parsed from DTX header commands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub maker: Option<String>,
    pub comment: Option<String>,
    pub preview_filename: Option<String>,
    pub preimage_filename: Option<String>,
    pub bpm: Option<f32>,
    pub dlevel: Option<u32>,
    pub glevel: Option<u32>,
    pub blevel: Option<u32>,
}

/// One chip in the chart (note, BPM change, bar-length change, etc.).
///
/// `measure` (0-indexed) + `channel` identify it; `value` is the parsed payload:
/// - For most chip channels: chip volume (0..1) or raw byte
/// - For BPM/BPMEx: BPM value
/// - For BarLength: fraction of a whole note
#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub measure: u32,
    pub channel: EChannel,
    pub value: f32,
}

impl Chip {
    pub fn new(measure: u32, channel: EChannel, value: f32) -> Self {
        Self {
            measure,
            channel,
            value,
        }
    }
}

/// Parsed DTX chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chart {
    pub metadata: Metadata,
    pub chips: Vec<Chip>,
}

impl Chart {
    /// Chips for a given channel, sorted by measure (then value as tiebreaker).
    pub fn chips_in(&self, channel: EChannel) -> impl Iterator<Item = &Chip> {
        self.chips.iter().filter(move |c| c.channel == channel)
    }

    /// All drum chips (the M2 subset).
    pub fn drum_chips(&self) -> impl Iterator<Item = &Chip> {
        self.chips.iter().filter(|c| c.channel.is_drum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drum_chips_filter() {
        let chart = Chart {
            metadata: Metadata::default(),
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 1.0),
                Chip::new(0, EChannel::BGM, 1.0),
                Chip::new(1, EChannel::Snare, 1.0),
            ],
        };
        let drums: Vec<_> = chart.drum_chips().collect();
        assert_eq!(drums.len(), 2);
        assert!(drums.iter().all(|c| c.channel.is_drum()));
    }
}
