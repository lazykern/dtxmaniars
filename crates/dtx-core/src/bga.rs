//! BGA (Background Animation) helpers.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfBGA.cs` (305 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfVideo.cs` (520 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EChannel.cs` (lines 10-13, 64-76)
//!
//! ## M7 scope
//!
//! Detect BGA chips from a parsed chart, classify them by layer, and produce
//! a sorted timeline of BGA events. Real image/video decoding is M7.1+
//! (port FFmpegCore / SoftwareVideoDecoder for AVI/MPG).
//!
//! DTX BGA channel mapping:
//! - BGALayer1 = 4    (small upper-left)
//! - BGALayer2 = 7    (small lower)
//! - BGALayer3 = 0x55 (fullscreen image)
//! - Movie     = 0x54 (AVI file reference)
//! - MovieFull = 0x5A (AVI fullscreen)
//! - BGALayer4..8 = 0x56..0x60 (additional image layers)

use crate::channel::EChannel;
use crate::chart::Chart;

/// Which BGA layer / kind a chip references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BgaLayer {
    /// Channel 4 — small upper-left image.
    Layer1,
    /// Channel 7 — small lower image.
    Layer2,
    /// Channel 0x55 — fullscreen background image.
    Layer3,
    /// Channel 0x56..0x60 — additional image layers.
    LayerN(u8),
    /// Channel 0x54 — AVI movie reference (M7.1+).
    Movie,
    /// Channel 0x5A — AVI fullscreen movie (M7.1+).
    MovieFull,
}

impl BgaLayer {
    /// Map an EChannel to its BGA layer kind.
    pub fn from_channel(channel: EChannel) -> Option<Self> {
        Some(match channel {
            EChannel::BGALayer1 => BgaLayer::Layer1,
            EChannel::BGALayer2 => BgaLayer::Layer2,
            EChannel::BGALayer3 => BgaLayer::Layer3,
            EChannel::BGALayer4 => BgaLayer::LayerN(4),
            EChannel::BGALayer5 => BgaLayer::LayerN(5),
            EChannel::BGALayer6 => BgaLayer::LayerN(6),
            EChannel::BGALayer7 => BgaLayer::LayerN(7),
            EChannel::BGALayer8 => BgaLayer::LayerN(8),
            EChannel::Movie => BgaLayer::Movie,
            EChannel::MovieFull => BgaLayer::MovieFull,
            _ => return None,
        })
    }

    /// Target layer for a BGA scope-swap channel.
    pub fn from_swap_channel(channel: EChannel) -> Option<Self> {
        Some(match channel {
            EChannel::BGALayer1Swap => Self::Layer1,
            EChannel::BGALayer2Swap => Self::Layer2,
            EChannel::BGALayer3Swap => Self::Layer3,
            EChannel::BGALayer4Swap => Self::LayerN(4),
            EChannel::BGALayer5Swap => Self::LayerN(5),
            EChannel::BGALayer6Swap => Self::LayerN(6),
            EChannel::BGALayer7Swap => Self::LayerN(7),
            EChannel::BGALayer8Swap => Self::LayerN(8),
            _ => return None,
        })
    }

    /// True if this layer requires real video decoding (M7.1+).
    pub fn is_movie(&self) -> bool {
        matches!(self, BgaLayer::Movie | BgaLayer::MovieFull)
    }

    /// True if this layer is image-only (can be shown as static overlay in M7).
    pub fn is_image(&self) -> bool {
        !self.is_movie()
    }

    /// Short label for HUD/debug.
    pub fn label(&self) -> &'static str {
        match self {
            BgaLayer::Layer1 => "BGA1",
            BgaLayer::Layer2 => "BGA2",
            BgaLayer::Layer3 => "BGA3",
            BgaLayer::LayerN(n) => match n {
                4 => "BGA4",
                5 => "BGA5",
                6 => "BGA6",
                7 => "BGA7",
                8 => "BGA8",
                _ => "BGA?",
            },
            BgaLayer::Movie => "MOVIE",
            BgaLayer::MovieFull => "MOVIE-FULL",
        }
    }
}

/// One BGA event in the chart timeline.
///
/// `bmp_index` is the BMP/AVI number referenced by the chip's value field.
/// For M7 we use this to render a placeholder overlay keyed to the layer.
/// M7.1 will resolve to actual file paths via `#BMPxx:` / `#AVIxx:` directives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BgaEvent {
    /// Measure index (0-based).
    pub measure: u32,
    /// Layer the event targets.
    pub layer: BgaLayer,
    /// BMP/AVI index (the chip value parsed as int).
    pub bmp_index: u32,
    /// Fractional position within the measure (0.0..1.0).
    pub fraction: f32,
}

impl BgaEvent {
    /// Estimated ms from chart start. Uses constant 120 BPM if metadata.bpm
    /// is None — M7 doesn't need precision here, M7.1 will use full timing.
    pub fn approx_ms(&self, bpm: f32) -> i64 {
        let bpm = if bpm > 0.0 { bpm } else { 120.0 };
        let ms_per_measure = 4.0_f64 * 60_000.0 / (bpm as f64);
        ((self.measure as f64) * ms_per_measure + (self.fraction as f64) * ms_per_measure) as i64
    }
}

/// Extract all BGA events from a parsed chart, sorted by `(measure, fraction)`
/// with stable source order preserved for equal timestamps.
pub fn bga_events(chart: &Chart) -> Vec<BgaEvent> {
    let mut events: Vec<BgaEvent> = chart
        .chips
        .iter()
        .filter_map(|c| {
            let layer = BgaLayer::from_channel(c.channel)?;
            Some(BgaEvent {
                measure: c.measure,
                layer,
                bmp_index: c.wav_slot,
                fraction: c.value,
            })
        })
        .collect();
    events.sort_by(|a, b| {
        a.measure
            .cmp(&b.measure)
            .then_with(|| a.fraction.total_cmp(&b.fraction))
    });
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::EChannel;
    use crate::chart::Chip;

    #[test]
    fn bga_layer_from_channel_maps_correctly() {
        assert_eq!(
            BgaLayer::from_channel(EChannel::BGALayer1),
            Some(BgaLayer::Layer1)
        );
        assert_eq!(
            BgaLayer::from_channel(EChannel::BGALayer2),
            Some(BgaLayer::Layer2)
        );
        assert_eq!(
            BgaLayer::from_channel(EChannel::BGALayer3),
            Some(BgaLayer::Layer3)
        );
        assert_eq!(
            BgaLayer::from_channel(EChannel::Movie),
            Some(BgaLayer::Movie)
        );
        assert_eq!(
            BgaLayer::from_channel(EChannel::MovieFull),
            Some(BgaLayer::MovieFull)
        );
    }

    #[test]
    fn bga_layer_non_bga_returns_none() {
        assert_eq!(BgaLayer::from_channel(EChannel::BGM), None);
        assert_eq!(BgaLayer::from_channel(EChannel::BarLine), None);
        assert_eq!(BgaLayer::from_channel(EChannel::BassDrum), None);
    }

    #[test]
    fn bga_layer_movie_classification() {
        assert!(BgaLayer::Movie.is_movie());
        assert!(BgaLayer::MovieFull.is_movie());
        assert!(!BgaLayer::Layer1.is_movie());
        assert!(!BgaLayer::Layer3.is_movie());
    }

    #[test]
    fn bga_layer_image_classification() {
        assert!(BgaLayer::Layer1.is_image());
        assert!(BgaLayer::Layer3.is_image());
        assert!(!BgaLayer::Movie.is_image());
    }

    #[test]
    fn bga_layer_labels_unique() {
        let labels = [
            BgaLayer::Layer1,
            BgaLayer::Layer2,
            BgaLayer::Layer3,
            BgaLayer::Movie,
            BgaLayer::MovieFull,
        ]
        .iter()
        .map(|l| l.label())
        .collect::<Vec<_>>();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len(), "labels not unique");
    }

    #[test]
    fn bga_events_filters_non_bga() {
        let chart = Chart {
            metadata: crate::chart::Metadata::default(),
            chips: vec![
                Chip::with_wav(0, EChannel::BGALayer1, 0.0, 1),
                Chip::new(0, EChannel::BassDrum, 1.0),
                Chip::with_wav(0, EChannel::Movie, 0.0, 2),
                Chip::with_wav(1, EChannel::BGALayer3, 0.0, 1),
                Chip::new(2, EChannel::BGM, 1.0),
            ],
            ..Default::default()
        };
        let events = bga_events(&chart);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].layer, BgaLayer::Layer1);
        assert_eq!(events[0].bmp_index, 1);
        assert_eq!(events[1].layer, BgaLayer::Movie);
        assert_eq!(events[1].bmp_index, 2);
        assert_eq!(events[2].layer, BgaLayer::Layer3);
    }

    #[test]
    fn bga_events_sorted_by_measure() {
        let chart = Chart {
            metadata: crate::chart::Metadata::default(),
            chips: vec![
                Chip::new(5, EChannel::BGALayer1, 1.0),
                Chip::new(0, EChannel::BGALayer3, 1.0),
                Chip::new(2, EChannel::Movie, 1.0),
            ],
            ..Default::default()
        };
        let events = bga_events(&chart);
        assert_eq!(events[0].measure, 0);
        assert_eq!(events[1].measure, 2);
        assert_eq!(events[2].measure, 5);
    }

    #[test]
    fn bga_event_approx_ms_uses_bpm() {
        let ev = BgaEvent {
            measure: 0,
            layer: BgaLayer::Layer1,
            bmp_index: 1,
            fraction: 0.0,
        };
        // 120 BPM = 2000 ms per measure
        assert_eq!(ev.approx_ms(120.0), 0);
        let ev2 = BgaEvent { measure: 1, ..ev };
        assert_eq!(ev2.approx_ms(120.0), 2000);
    }

    #[test]
    fn bga_event_approx_ms_zero_bpm_falls_back() {
        let ev = BgaEvent {
            measure: 0,
            layer: BgaLayer::Layer1,
            bmp_index: 1,
            fraction: 0.0,
        };
        assert_eq!(ev.approx_ms(0.0), 0);
    }
}
