//! Closest-anchor auto-snap (osu `ApplyClosestAnchorOrigin` behavior): while a
//! widget drag is in progress and the widget has `anchor_auto`, the anchor
//! follows the widget's center across the parent's thirds. Every anchor
//! rewrite recomputes the offset so the resolved position never jumps.

use bevy::prelude::*;
use dtx_layout::{nearest_anchor, Placement, WidgetKind};

use super::drag::{ActiveGesture, Gesture, Selection};
use super::selection_box::EditorOverlay;
use crate::layout::PlayfieldLayout;
use crate::widget_layout::{parent_rect_px, transform_point, WidgetGeoms, WidgetLayouts};

/// Guide line at a parent-space third (spawned once with the overlay).
#[derive(Component)]
pub struct SnapGuide {
    pub vertical: bool,
    /// 1 or 2 (which third).
    pub which: u8,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (apply_anchor_snap, sync_snap_guides)
            .chain()
            .after(super::EditorGestureSet)
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        Update,
        spawn_guides_on_open.run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Guides are tagged `EditorOverlay`, so `selection_box::spawn_overlay_on_open`
/// despawns them on close and `despawn_overlay` on exit. This system only owns
/// (re)spawning them on open — despawning here too would double-despawn the
/// same entities and log a warning every close.
fn spawn_guides_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    existing: Query<Entity, With<SnapGuide>>,
) {
    if !open.is_changed() {
        return;
    }
    if !open.0 {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    for vertical in [true, false] {
        for which in [1u8, 2u8] {
            commands.spawn((
                EditorOverlay,
                SnapGuide { vertical, which },
                Node {
                    position_type: PositionType::Absolute,
                    width: if vertical { Val::Px(1.0) } else { Val::Px(0.0) },
                    height: if vertical { Val::Px(0.0) } else { Val::Px(1.0) },
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                Visibility::Hidden,
                GlobalZIndex(2050),
                Pickable::IGNORE,
            ));
        }
    }
}

/// While dragging with anchor_auto: nearest ninth from the widget's visual
/// center within its parent; on change rewrite anchor+origin and recompute
/// offset (no-jump).
fn apply_anchor_snap(
    gesture: Res<ActiveGesture>,
    selection: Res<Selection>,
    geoms: Res<WidgetGeoms>,
    pfl: Res<PlayfieldLayout>,
    rect: Res<crate::stage_rect::StageRect>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut layouts: ResMut<WidgetLayouts>,
) {
    if !matches!(gesture.0, Gesture::Move { .. }) {
        return;
    }
    let Some(kind) = selection.0 else { return };
    if kind == WidgetKind::Playfield {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(g) = geoms.0.get(&kind).copied() else {
        return;
    };
    let inst_ro = layouts.get(kind).clone();
    if !inst_ro.anchor_auto || inst_ro.placement != Placement::Anchored {
        // Natural widgets convert on gesture start (plan 2); if still Natural
        // here, offset-delta dragging continues un-snapped.
        return;
    }
    let wsize = Vec2::new(window.width(), window.height());
    let sc = wsize / 2.0;
    let vis_min = transform_point(g.unscaled.min, sc, g.applied_translation, g.applied_scale);
    let vis_max = transform_point(g.unscaled.max, sc, g.applied_translation, g.applied_scale);
    let center = (vis_min + vis_max) / 2.0;
    let (px, py, pw, ph) = parent_rect_px(inst_ro.space, *rect, &pfl);
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }
    let frac = ((center - Vec2::new(px, py)) / Vec2::new(pw, ph)).clamp(Vec2::ZERO, Vec2::ONE);
    let want = nearest_anchor(frac.x, frac.y);
    if want == inst_ro.anchor {
        return;
    }
    let Some(inst) = layouts.0.get_mut(&kind) else {
        return;
    };
    inst.anchor = want;
    inst.origin = want;
    let off_px = dtx_layout::offset_for_top_left(
        want,
        want,
        (g.unscaled.width(), g.unscaled.height()),
        inst.scale,
        (vis_min.x, vis_min.y),
        (px, py, pw, ph),
    );
    inst.offset = (off_px.0 / pfl.scale, off_px.1 / pfl.scale);
}

/// Guides visible only during a Move drag; positioned at the selected
/// widget's parent-space thirds.
fn sync_snap_guides(
    gesture: Res<ActiveGesture>,
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    pfl: Res<PlayfieldLayout>,
    rect: Res<crate::stage_rect::StageRect>,
    mut guides: Query<(&SnapGuide, &mut Node, &mut Visibility)>,
) {
    let dragging = matches!(gesture.0, Gesture::Move { .. });
    let show = dragging
        && selection
            .0
            .map(|k| k != WidgetKind::Playfield && layouts.get(k).anchor_auto)
            .unwrap_or(false);
    if !show {
        for (_, _, mut vis) in &mut guides {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let Some(kind) = selection.0 else { return };
    let (px, py, pw, ph) = parent_rect_px(layouts.get(kind).space, *rect, &pfl);
    for (guide, mut node, mut vis) in &mut guides {
        let t = guide.which as f32 / 3.0;
        if guide.vertical {
            node.left = Val::Px(px + pw * t);
            node.top = Val::Px(py);
            node.height = Val::Px(ph);
            node.width = Val::Px(1.0);
        } else {
            node.left = Val::Px(px);
            node.top = Val::Px(py + ph * t);
            node.width = Val::Px(pw);
            node.height = Val::Px(1.0);
        }
        *vis = Visibility::Visible;
    }
}

#[cfg(test)]
mod tests {
    use dtx_layout::{nearest_anchor, Anchor9};

    #[test]
    fn snap_rewrite_is_no_jump() {
        // Anchor rewrite + offset_for_top_left keeps resolve_top_left fixed.
        let parent = (0.0, 0.0, 1280.0, 720.0);
        let size = (150.0, 60.0);
        let visual = (900.0, 600.0); // bottom-right-ish → BottomRight anchor
        let frac = (
            (visual.0 + size.0 / 2.0) / 1280.0,
            (visual.1 + size.1 / 2.0) / 720.0,
        );
        let a = nearest_anchor(frac.0, frac.1);
        assert_eq!(a, Anchor9::BottomRight);
        let off = dtx_layout::offset_for_top_left(a, a, size, 1.0, visual, parent);
        let tl = dtx_layout::resolve_top_left(a, a, size, 1.0, off, parent);
        assert!((tl.0 - visual.0).abs() < 0.001 && (tl.1 - visual.1).abs() < 0.001);
    }
}
