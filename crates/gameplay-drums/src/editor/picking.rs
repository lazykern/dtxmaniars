//! On-canvas widget hit-testing for the layout editor.
//!
//! No bevy_picking on gameplay nodes (containers carry `Pickable::IGNORE`);
//! instead a per-frame AABB per widget is built from the union of the widget
//! container's descendant `ComputedNode` rects (logical px). Hover/click test
//! the cursor against those AABBs, masked by editor chrome (sidebar/panel).

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_layout::WidgetKind;

use crate::layout::PlayfieldLayout;
use crate::widget_layout::{widget_visible, WidgetContainer, WidgetLayouts};

/// Logical-px AABB per widget, rebuilt each frame while the editor is open.
/// Entries persist across frames (last non-empty wins) so widgets that render
/// nothing this instant (e.g. judgment popup between hits) stay grabbable.
#[derive(Resource, Debug, Default)]
pub struct WidgetAabbs(pub HashMap<WidgetKind, Rect>);

/// Widget currently under the cursor (topmost by z, then smallest area).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct Hovered(pub Option<WidgetKind>);

/// True while the cursor is over editor chrome (sidebar/panel) — canvas
/// hover/click are masked.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct CursorOverChrome(pub bool);

/// Marker for editor UI surfaces that mask the canvas (sidebar root; the
/// settings panel adds itself in plan 2).
#[derive(Component)]
pub struct EditorChrome;

/// Minimum grab box (logical px) for widgets whose content AABB is empty.
pub const MIN_GRAB: f32 = 24.0;

/// Candidates under `cursor`, best-first: higher z wins, ties → smaller area.
pub fn candidates_at(
    aabbs: &HashMap<WidgetKind, Rect>,
    z_of: impl Fn(WidgetKind) -> i32,
    cursor: Vec2,
) -> Vec<WidgetKind> {
    let mut hits: Vec<(WidgetKind, i32, f32)> = aabbs
        .iter()
        .filter(|(_, r)| r.contains(cursor))
        .map(|(k, r)| (*k, z_of(*k), r.width() * r.height()))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.total_cmp(&b.2)));
    hits.into_iter().map(|(k, _, _)| k).collect()
}

/// Topmost candidate, or None on empty canvas.
pub fn pick_topmost(
    aabbs: &HashMap<WidgetKind, Rect>,
    z_of: impl Fn(WidgetKind) -> i32,
    cursor: Vec2,
) -> Option<WidgetKind> {
    candidates_at(aabbs, z_of, cursor).into_iter().next()
}

/// Alt+click cycling: next candidate after `current` in the priority order
/// (wraps). If `current` isn't among the candidates, behaves like topmost.
pub fn cycle_pick(candidates: &[WidgetKind], current: Option<WidgetKind>) -> Option<WidgetKind> {
    if candidates.is_empty() {
        return None;
    }
    match current.and_then(|c| candidates.iter().position(|&k| k == c)) {
        Some(i) => Some(candidates[(i + 1) % candidates.len()]),
        None => Some(candidates[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    #[test]
    fn topmost_prefers_higher_z() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Combo, r(0.0, 0.0, 100.0, 100.0));
        m.insert(WidgetKind::ScorePanel, r(0.0, 0.0, 100.0, 100.0));
        let z = |k: WidgetKind| if k == WidgetKind::Combo { 5 } else { 0 };
        assert_eq!(
            pick_topmost(&m, z, Vec2::new(50.0, 50.0)),
            Some(WidgetKind::Combo)
        );
    }

    #[test]
    fn equal_z_prefers_smaller_area() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Playfield, r(0.0, 0.0, 500.0, 500.0));
        m.insert(WidgetKind::Combo, r(10.0, 10.0, 50.0, 50.0));
        assert_eq!(
            pick_topmost(&m, |_| 0, Vec2::new(20.0, 20.0)),
            Some(WidgetKind::Combo)
        );
    }

    #[test]
    fn miss_returns_none() {
        let mut m = HashMap::new();
        m.insert(WidgetKind::Combo, r(0.0, 0.0, 10.0, 10.0));
        assert_eq!(pick_topmost(&m, |_| 0, Vec2::new(99.0, 99.0)), None);
    }

    #[test]
    fn cycle_wraps_through_candidates() {
        let c = [WidgetKind::Combo, WidgetKind::ScorePanel, WidgetKind::Playfield];
        assert_eq!(cycle_pick(&c, None), Some(WidgetKind::Combo));
        assert_eq!(cycle_pick(&c, Some(WidgetKind::Combo)), Some(WidgetKind::ScorePanel));
        assert_eq!(cycle_pick(&c, Some(WidgetKind::Playfield)), Some(WidgetKind::Combo));
        assert_eq!(cycle_pick(&[], Some(WidgetKind::Combo)), None);
    }
}
