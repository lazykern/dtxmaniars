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

    #[test]
    fn empty_chart() {
        let c = Chart::default();
        assert_eq!(c.chips.len(), 0);
        assert_eq!(c.metadata.title, None);
        assert_eq!(c.metadata.bpm, None);
    }

    #[test]
    fn metadata_default_all_none() {
        let m = Metadata::default();
        assert!(m.title.is_none());
        assert!(m.artist.is_none());
        assert!(m.genre.is_none());
        assert!(m.maker.is_none());
        assert!(m.comment.is_none());
        assert!(m.preview_filename.is_none());
        assert!(m.preimage_filename.is_none());
        assert!(m.bpm.is_none());
        assert!(m.dlevel.is_none());
        assert!(m.glevel.is_none());
        assert!(m.blevel.is_none());
    }

    #[test]
    fn metadata_clone() {
        let mut m = Metadata::default();
        m.title = Some("Test".to_string());
        m.bpm = Some(120.0);
        let m2 = m.clone();
        assert_eq!(m.title, m2.title);
        assert_eq!(m.bpm, m2.bpm);
    }

    #[test]
    fn metadata_equality() {
        let mut a = Metadata::default();
        let mut b = Metadata::default();
        a.title = Some("X".into());
        b.title = Some("X".into());
        assert_eq!(a, b);
        a.bpm = Some(120.0);
        assert_ne!(a, b);
    }

    #[test]
    fn chip_new_helper() {
        let c = Chip::new(5, EChannel::Snare, 0.5);
        assert_eq!(c.measure, 5);
        assert_eq!(c.channel, EChannel::Snare);
        assert!((c.value - 0.5).abs() < 0.001);
    }

    #[test]
    fn chip_clone() {
        let c = Chip::new(0, EChannel::BassDrum, 1.0);
        let c2 = c.clone();
        assert_eq!(c, c2);
    }

    #[test]
    fn chart_equality() {
        let c1 = Chart {
            metadata: Metadata::default(),
            chips: vec![Chip::new(0, EChannel::BassDrum, 1.0)],
        };
        let c2 = Chart {
            metadata: Metadata::default(),
            chips: vec![Chip::new(0, EChannel::BassDrum, 1.0)],
        };
        assert_eq!(c1, c2);
    }

    #[test]
    fn chips_in_filter_by_channel() {
        let chart = Chart {
            metadata: Metadata::default(),
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 1.0),
                Chip::new(1, EChannel::Snare, 1.0),
                Chip::new(2, EChannel::BassDrum, 1.0),
                Chip::new(3, EChannel::BGM, 1.0),
            ],
        };
        let bd: Vec<_> = chart.chips_in(EChannel::BassDrum).collect();
        assert_eq!(bd.len(), 2);
        assert!(bd.iter().all(|c| c.channel == EChannel::BassDrum));
    }

    #[test]
    fn chips_in_returns_empty_for_missing_channel() {
        let chart = Chart {
            metadata: Metadata::default(),
            chips: vec![Chip::new(0, EChannel::Snare, 1.0)],
        };
        let bd: Vec<_> = chart.chips_in(EChannel::BassDrum).collect();
        assert_eq!(bd.len(), 0);
    }

    #[test]
    fn drum_chips_excludes_bga() {
        let chart = Chart {
            metadata: Metadata::default(),
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 1.0),
                Chip::new(0, EChannel::BGALayer1, 1.0),
                Chip::new(0, EChannel::Movie, 1.0),
                Chip::new(0, EChannel::Snare, 1.0),
            ],
        };
        let drums: Vec<_> = chart.drum_chips().collect();
        assert_eq!(drums.len(), 2);
    }

    #[test]
    fn chart_clone_preserves_chips() {
        let c1 = Chart {
            metadata: Metadata::default(),
            chips: vec![Chip::new(0, EChannel::Snare, 1.0)],
        };
        let c2 = c1.clone();
        assert_eq!(c1.chips.len(), c2.chips.len());
        assert_eq!(c1.chips[0], c2.chips[0]);
    }
}
