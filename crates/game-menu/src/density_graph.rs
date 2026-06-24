//! `DensityGraph` — port of `Stage/04.SongSelectionNew/DensityGraph.cs` (279 LOC).
//!
//! Strict-port-first. Note-density histogram per instrument.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/DensityGraph.cs:1-279`

use super::status_panel::EInstrumentPart;

/// Number of bars in the density graph (DensityGraph.cs).
pub const DENSITY_BARS: usize = 12;

/// Spacing between bars in px (DensityGraph.cs:42).
pub const DENSITY_BAR_DX: f32 = 12.0;
/// Bar-line x position (DensityGraph.cs:42: `36 + index * 12`).
pub const DENSITY_BAR_X0: f32 = 36.0;
/// Bar-line y position (DensityGraph.cs:42: `284`).
pub const DENSITY_BAR_Y: f32 = 284.0;
/// noteCountText position for Drums (DensityGraph.cs:17).
pub const DENSITY_NOTE_TEXT_X_DRUMS: f32 = 150.0;
pub const DENSITY_NOTE_TEXT_Y_DRUMS: f32 = 333.0;

/// A single density bar (one chip-count bracket).
#[derive(Debug, Clone, Default)]
pub struct DensityBar {
    /// Number of chips in this bracket.
    pub count: u32,
    /// Bar color (one of clDrumChipsBarColors for drums).
    pub color: [u8; 4],
    /// Bar height in px (0..max).
    pub height: f32,
}

impl DensityBar {
    pub fn new(count: u32, color: [u8; 4], height: f32) -> Self {
        Self { count, color, height }
    }
}

/// The full density graph for one instrument.
#[derive(Debug, Clone)]
pub struct DensityGraph {
    pub instrument: EInstrumentPart,
    /// DENSITY_BARS brackets.
    pub bars: Vec<DensityBar>,
    /// Display text (e.g. "Total: 1234 notes").
    pub note_text: String,
}

impl DensityGraph {
    pub fn new(instrument: EInstrumentPart) -> Self {
        Self {
            instrument,
            bars: vec![DensityBar::default(); DENSITY_BARS],
            note_text: String::new(),
        }
    }

    /// Update note_text from total count.
    pub fn update_total(&mut self, total: u32) {
        self.note_text = format!("Total: {total} notes");
    }

    /// Set bar count at index (clamps to DENSITY_BARS).
    pub fn set_bar(&mut self, index: usize, count: u32) {
        if let Some(bar) = self.bars.get_mut(index) {
            bar.count = count;
        }
    }

    /// Compute bar x position at index.
    pub fn bar_x(index: usize) -> f32 {
        DENSITY_BAR_X0 + (index as f32) * DENSITY_BAR_DX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_bars_count() {
        assert_eq!(DENSITY_BARS, 12);
    }

    #[test]
    fn density_bar_dx_matches_reference() {
        // DensityGraph.cs:42 — `36 + index * 12`
        assert_eq!(DENSITY_BAR_DX, 12.0);
        assert_eq!(DENSITY_BAR_X0, 36.0);
    }

    #[test]
    fn density_note_text_position_drums() {
        // DensityGraph.cs:17
        assert_eq!(DENSITY_NOTE_TEXT_X_DRUMS, 150.0);
        assert_eq!(DENSITY_NOTE_TEXT_Y_DRUMS, 333.0);
    }

    #[test]
    fn density_graph_default_has_12_bars() {
        let g = DensityGraph::new(EInstrumentPart::Drums);
        assert_eq!(g.bars.len(), DENSITY_BARS);
    }

    #[test]
    fn density_graph_update_total() {
        let mut g = DensityGraph::new(EInstrumentPart::Drums);
        g.update_total(1234);
        assert_eq!(g.note_text, "Total: 1234 notes");
    }

    #[test]
    fn density_graph_set_bar() {
        let mut g = DensityGraph::new(EInstrumentPart::Drums);
        g.set_bar(0, 50);
        g.set_bar(11, 100);
        assert_eq!(g.bars[0].count, 50);
        assert_eq!(g.bars[11].count, 100);
    }

    #[test]
    fn density_graph_set_bar_out_of_range() {
        let mut g = DensityGraph::new(EInstrumentPart::Drums);
        g.set_bar(99, 50); // out of range — no panic
        assert_eq!(g.bars.len(), DENSITY_BARS);
    }

    #[test]
    fn density_bar_x_position_formula() {
        // bar 0 at x=36, bar 1 at x=48, bar 11 at x=168
        assert!((DensityGraph::bar_x(0) - 36.0).abs() < 0.01);
        assert!((DensityGraph::bar_x(1) - 48.0).abs() < 0.01);
        assert!((DensityGraph::bar_x(11) - 168.0).abs() < 0.01);
    }
}
