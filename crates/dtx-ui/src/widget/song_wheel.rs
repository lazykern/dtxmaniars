//! GITADORA song wheel: big rows arcing toward the selection.
//! Pure geometry here; the song-select screen owns spawning/content.

use bevy::prelude::*;

/// Wheel container marker.
#[derive(Component, Debug, Clone, Copy)]
pub struct SongWheel;

/// One wheel row; `index` is the folder index in the visible list.
#[derive(Component, Debug, Clone, Copy)]
pub struct WheelRow {
    pub index: usize,
}

/// Spring state for the wheel scroll (one per wheel).
#[derive(Resource, Debug, Clone)]
pub struct WheelSpring(pub crate::motion::SpringValue);

impl Default for WheelSpring {
    fn default() -> Self {
        Self(crate::motion::SpringValue::wheel(0.0))
    }
}

pub const ROW_H: f32 = 78.0;
pub const ROW_H_SELECTED: f32 = 122.0;
pub const ROW_GAP: f32 = 6.0;
pub const MAX_INDENT: f32 = 110.0;
/// Rows drawn above/below center.
pub const VISIBLE_HALF: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGeom {
    /// Offset in px from wheel vertical center to row center.
    pub center_y: f32,
    pub height: f32,
    /// Left indent (arc): 0 at selection, grows with distance.
    pub indent: f32,
    /// 1.0 at selection fading toward edges.
    pub alpha: f32,
}

/// Geometry for a row at signed `offset` slots from the (fractional)
/// selection. offset = row_index - spring_value.
pub fn row_geometry(offset: f32) -> RowGeom {
    let d = offset.abs();
    // Selected-row expansion blends in within half a slot.
    let sel = (1.0 - d.min(1.0)).clamp(0.0, 1.0);
    let height = ROW_H + (ROW_H_SELECTED - ROW_H) * sel;
    // Row centers: selected row is bigger, neighbors push outward.
    let base = offset * (ROW_H + ROW_GAP);
    let expand = (ROW_H_SELECTED - ROW_H) * 0.5 * sel_shift(offset);
    let center_y = base + expand;
    // Arc indent: quadratic ease by distance, capped.
    let indent = MAX_INDENT * ((d / VISIBLE_HALF as f32).min(1.0)).powf(1.4);
    let alpha = (1.0 - (d / (VISIBLE_HALF as f32 + 0.5)).powi(2)).clamp(0.15, 1.0);
    RowGeom {
        center_y,
        height,
        indent,
        alpha,
    }
}

/// Signed push away from center for neighbor rows (-1..1).
fn sel_shift(offset: f32) -> f32 {
    offset.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_row_is_biggest_and_flush_left() {
        let g = row_geometry(0.0);
        assert_eq!(g.height, ROW_H_SELECTED);
        assert_eq!(g.indent, 0.0);
        assert_eq!(g.center_y, 0.0);
        assert_eq!(g.alpha, 1.0);
    }

    #[test]
    fn distant_rows_shrink_indent_and_fade() {
        let g1 = row_geometry(1.0);
        let g4 = row_geometry(4.0);
        assert_eq!(g1.height, ROW_H);
        assert!(g4.indent > g1.indent);
        assert!(g4.alpha < g1.alpha);
    }

    #[test]
    fn geometry_symmetric() {
        let up = row_geometry(-2.0);
        let down = row_geometry(2.0);
        assert_eq!(up.height, down.height);
        assert_eq!(up.indent, down.indent);
        assert!((up.center_y + down.center_y).abs() < 0.001);
    }

    #[test]
    fn fractional_offset_interpolates_height() {
        let g = row_geometry(0.5);
        assert!(g.height > ROW_H && g.height < ROW_H_SELECTED);
    }
}
