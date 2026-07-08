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
use crate::widget_layout::{widget_visible, WidgetLayouts};

/// Logical-px AABB per widget, rebuilt each frame while the editor is open.
/// Entries persist across frames (last non-empty wins) so widgets that render
/// nothing this instant (e.g. judgment popup between hits) stay grabbable.
#[derive(Resource, Debug, Default)]
pub struct WidgetAabbs(pub HashMap<WidgetKind, Rect>);

/// Widget currently under the cursor (topmost by z, then smallest area).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct Hovered(pub Option<WidgetKind>);

/// Widgets hidden in the current preview mode. They keep an AABB (so a
/// sidebar-selected hidden widget still gets a dimmed selection box) but are
/// excluded from canvas hover/click so you can't grab an invisible widget.
#[derive(Resource, Debug, Default)]
pub struct CanvasHidden(pub std::collections::HashSet<WidgetKind>);

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

/// Logical-px rect of a laid-out UI node. UI nodes carry `UiGlobalTransform`
/// (an `Affine2`, not the 3D `GlobalTransform`); its `translation` is the node
/// center in physical px.
pub(crate) fn node_rect(node: &ComputedNode, gt: &bevy::ui::UiGlobalTransform) -> Rect {
    let inv = node.inverse_scale_factor();
    let center = gt.translation * inv;
    let size = node.size() * inv;
    Rect::from_center_size(center, size)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<WidgetAabbs>()
        .init_resource::<Hovered>()
        .init_resource::<CanvasHidden>()
        .init_resource::<CursorOverChrome>()
        .add_systems(
            Update,
            (
                collect_widget_aabbs,
                update_cursor_over_chrome,
                update_hover,
            )
                .chain()
                .in_set(super::EditorPickSet)
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// Visual AABB = unscaled geom pushed through the applied transform. The
/// traversal lives in `widget_layout::measure_widget_geoms` (always-on); the
/// editor just derives hit rects from it. Hidden-in-mode widgets keep their
/// AABB (so a sidebar-selected hidden widget still gets a dimmed selection box)
/// but are recorded in `CanvasHidden` so canvas hover/click skip them.
fn collect_widget_aabbs(
    mut aabbs: ResMut<WidgetAabbs>,
    mut hidden: ResMut<CanvasHidden>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    rect: Res<crate::stage_rect::StageRect>,
) {
    let sc = rect.center();
    let is_practice = practice.is_some();
    hidden.0.clear();
    for kind in dtx_layout::WidgetKind::ALL {
        if kind == dtx_layout::WidgetKind::Playfield {
            continue;
        }
        if !widget_visible(layouts.get(kind), is_practice) {
            hidden.0.insert(kind);
        }
        let Some(g) = geoms.0.get(&kind) else {
            continue;
        };
        let vis = Rect::from_corners(
            crate::widget_layout::transform_point(
                g.unscaled.min,
                sc,
                g.applied_translation,
                g.applied_scale,
            ),
            crate::widget_layout::transform_point(
                g.unscaled.max,
                sc,
                g.applied_translation,
                g.applied_scale,
            ),
        );
        let vis = Rect::from_center_size(vis.center(), vis.size().max(Vec2::splat(MIN_GRAB)));
        aabbs.0.insert(kind, vis);
    }
    // Playfield AABB straight from layout geometry (backboard incl. pad).
    aabbs.0.insert(
        WidgetKind::Playfield,
        Rect::new(
            pfl.backboard_left(),
            pfl.backboard_top(),
            pfl.backboard_left() + pfl.backboard_width(),
            pfl.backboard_top() + pfl.backboard_height(),
        ),
    );
}

fn update_cursor_over_chrome(
    mut over: ResMut<CursorOverChrome>,
    windows: Query<&Window>,
    chrome: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<EditorChrome>>,
) {
    over.0 = false;
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        return;
    };
    for (cn, gt) in &chrome {
        if node_rect(cn, gt).contains(pos) {
            over.0 = true;
            return;
        }
    }
}

fn update_hover(
    mut hovered: ResMut<Hovered>,
    over_chrome: Res<CursorOverChrome>,
    gesture: Res<super::drag::ActiveGesture>,
    aabbs: Res<WidgetAabbs>,
    hidden: Res<CanvasHidden>,
    layouts: Res<WidgetLayouts>,
    windows: Query<&Window>,
) {
    if over_chrome.0 || !matches!(gesture.0, super::drag::Gesture::None) {
        hovered.0 = None;
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    hovered.0 = candidates_at(&aabbs.0, |k| layouts.get(k).z, pos)
        .into_iter()
        .find(|k| !hidden.0.contains(k));
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
        let c = [
            WidgetKind::Combo,
            WidgetKind::ScorePanel,
            WidgetKind::Playfield,
        ];
        assert_eq!(cycle_pick(&c, None), Some(WidgetKind::Combo));
        assert_eq!(
            cycle_pick(&c, Some(WidgetKind::Combo)),
            Some(WidgetKind::ScorePanel)
        );
        assert_eq!(
            cycle_pick(&c, Some(WidgetKind::Playfield)),
            Some(WidgetKind::Combo)
        );
        assert_eq!(cycle_pick(&[], Some(WidgetKind::Combo)), None);
    }
}
