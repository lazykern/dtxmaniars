//! Runtime HUD widget placement: per-widget container nodes driven by a
//! `WidgetLayouts` resource (code defaults ⊕ layout.toml `[scene.gameplay]`).
//!
//! Each HUD widget's children are parented to a full-screen `WidgetContainer`
//! node. Placement is applied through the container's `UiTransform` (a
//! render/hit transform that does NOT affect layout, so children keep their
//! natural size — measurements stay stable):
//!   - `Natural` (v1 semantics): translate by `offset·pfl.scale`, scale 1. At
//!     the default offset (0,0) the transform is identity, so every untouched
//!     widget renders byte-identically to before this system existed (parity).
//!   - `Anchored`: absolute `resolve_top_left` + uniform scale, applied as a
//!     scale-about-screen-center transform with a computed translation so the
//!     content lands exactly at the resolved top-left.
//!
//! `measure_widget_geoms` keeps `WidgetGeoms` in UNSCALED logical space by
//! inverting last frame's applied transform off the measured visual rects.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::{UiTransform, Val2};
use dtx_layout::{AnchorSpace, WidgetInstance, WidgetKind};
use game_shell::AppState;

use crate::layout::PlayfieldLayout;

/// Marks a per-widget container node (parent of one widget's children).
#[derive(Component, Debug, Clone, Copy)]
pub struct WidgetContainer(pub WidgetKind);

/// Resolved placement for every widget kind (defaults ⊕ file).
#[derive(Resource, Debug, Clone)]
pub struct WidgetLayouts(pub HashMap<WidgetKind, WidgetInstance>);

impl Default for WidgetLayouts {
    fn default() -> Self {
        Self(dtx_layout::SceneSection::default().resolve())
    }
}

impl WidgetLayouts {
    pub fn get(&self, kind: WidgetKind) -> &WidgetInstance {
        self.0.get(&kind).expect("WidgetLayouts missing a kind")
    }
}

/// Per-widget content geometry in UNSCALED logical px (children's natural
/// layout, before the container's UiTransform). `applied` is the transform we
/// set last frame, used to invert visual measurements back to unscaled space.
#[derive(Debug, Clone, Copy)]
pub struct WidgetGeom {
    pub unscaled: Rect,
    pub applied_translation: Vec2,
    pub applied_scale: f32,
}

#[derive(Resource, Debug, Default)]
pub struct WidgetGeoms(pub std::collections::HashMap<WidgetKind, WidgetGeom>);

/// A UiTransform (translation T, uniform scale s) maps an unscaled point p to
/// S + s·(p − S) + T, where S = screen center (full-screen container's center).
pub fn transform_point(p: Vec2, screen_center: Vec2, t: Vec2, s: f32) -> Vec2 {
    screen_center + s * (p - screen_center) + t
}

/// Compose two scale-about-the-same-center transforms: `outer(inner(p))`.
/// Both pivot on the screen center (Bevy `UiTransform` node-center convention
/// for full-screen nodes), so composition is linear in (T, s).
pub fn compose_about_center(
    t_outer: Vec2,
    s_outer: f32,
    t_inner: Vec2,
    s_inner: f32,
) -> (Vec2, f32) {
    (s_outer * t_inner + t_outer, s_outer * s_inner)
}

/// Inverse of `transform_point` for a whole rect (recover unscaled geometry
/// from a visual measurement under a known applied transform).
pub fn untransform_rect(measured: Rect, screen_center: Vec2, t: Vec2, s: f32) -> Rect {
    let inv = |m: Vec2| screen_center + (m - t - screen_center) / s.max(f32::EPSILON);
    Rect::from_corners(inv(measured.min), inv(measured.max))
}

/// Translation that puts the unscaled content top-left `u_min` at visual
/// position `desired` under scale `s` about `screen_center`.
pub fn translation_for(desired: Vec2, u_min: Vec2, screen_center: Vec2, s: f32) -> Vec2 {
    desired - screen_center - s * (u_min - screen_center)
}

/// Whether a widget is visible in the current mode (practice vs play).
pub fn widget_visible(inst: &WidgetInstance, practice: bool) -> bool {
    if practice {
        inst.visible_practice
    } else {
        inst.visible_play
    }
}

/// Screen-space parent rect from the stage rect (origin-aware).
fn screen_parent_rect(rect: crate::stage_rect::StageRect) -> (f32, f32, f32, f32) {
    (rect.origin.x, rect.origin.y, rect.size.x, rect.size.y)
}

/// Parent rect (logical px) for a widget's anchor space.
pub fn parent_rect_px(
    space: AnchorSpace,
    rect: crate::stage_rect::StageRect,
    pfl: &PlayfieldLayout,
) -> (f32, f32, f32, f32) {
    match space {
        AnchorSpace::Screen => screen_parent_rect(rect),
        AnchorSpace::Playfield => (
            pfl.strip_left(),
            pfl.lane_top(),
            pfl.strip_width(),
            pfl.lane_height(),
        ),
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WidgetLayouts>()
        .init_resource::<WidgetGeoms>()
        .add_systems(Startup, load_widget_layouts)
        .add_systems(
            Update,
            (
                measure_widget_geoms,
                apply_widget_layout.run_if(
                    resource_changed::<WidgetLayouts>
                        .or_else(resource_changed::<PlayfieldLayout>)
                        .or_else(any_anchored_widget)
                        .or_else(resource_changed::<crate::editor::PreviewState>),
                ),
            )
                .chain()
                .run_if(in_state(AppState::Performance)),
        )
        .add_systems(
            Update,
            hide_practice_hud_on_preview
                .run_if(in_state(AppState::Performance))
                .run_if(resource_changed::<crate::editor::PreviewState>),
        );
}

/// Anchored widgets need a per-frame apply (their resolved position depends on
/// measured geometry, which can change as content re-lays-out). Natural-only
/// scenes keep the v1 change-detection behavior.
fn any_anchored_widget(layouts: Res<WidgetLayouts>) -> bool {
    layouts
        .0
        .values()
        .any(|i| i.placement == dtx_layout::Placement::Anchored)
}

/// Load `[scene.gameplay]` from layout.toml at startup (defaults on absence).
fn load_widget_layouts(mut layouts: ResMut<WidgetLayouts>) {
    let file = dtx_layout::load(&dtx_layout::default_path());
    layouts.0 = file.scene.resolve();
}

/// Measure every widget container's visual content rect and invert the applied
/// transforms to keep `WidgetGeoms` in unscaled SCENE space. The measured
/// `UiGlobalTransform` includes BOTH the container's own `UiTransform` and the
/// inherited HudRoot stage transform, so the inversion must strip their
/// composition; `applied_*` still stores only the container's own transform
/// (consumers reconstruct scene-space visual rects with it). Runs every frame
/// in Performance (cheap: ~10 widgets, shallow trees).
fn measure_widget_geoms(
    mut geoms: ResMut<WidgetGeoms>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    roots: Query<&UiTransform, With<crate::hud::HudRoot>>,
    containers: Query<(Entity, &WidgetContainer, &UiTransform), Without<crate::hud::HudRoot>>,
    children_q: Query<&Children>,
    nodes: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let sc = Vec2::new(window.width() / 2.0, window.height() / 2.0);
    let (stage_t, stage_s) = roots.single().map(applied_of).unwrap_or((Vec2::ZERO, 1.0));
    for (entity, container, ui_tf) in &containers {
        let kind = container.0;
        if kind == WidgetKind::Playfield {
            continue;
        }
        let (t, s) = applied_of(ui_tf);
        let (t_comp, s_comp) = compose_about_center(stage_t, stage_s, t, s);
        let mut union: Option<Rect> = None;
        let mut stack: Vec<Entity> = children_q
            .get(entity)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        while let Some(e) = stack.pop() {
            // UI nodes carry `UiGlobalTransform` (not `GlobalTransform`); its
            // translation is the node center in physical px and already
            // includes the container's UiTransform AND the HudRoot stage
            // transform. Rendered size is the layout size times the composed
            // scale, so form the VISUAL rect here and let `untransform_rect`
            // (with the composed transform) recover unscaled scene space.
            if let Ok((cn, gt)) = nodes.get(e) {
                if cn.size().x > 0.0 && cn.size().y > 0.0 {
                    let inv = cn.inverse_scale_factor();
                    let center = gt.translation * inv;
                    let size = cn.size() * inv * s_comp;
                    let r = Rect::from_center_size(center, size);
                    union = Some(union.map_or(r, |u| u.union(r)));
                }
            }
            if let Ok(c) = children_q.get(e) {
                stack.extend(c.iter());
            }
        }
        if let Some(measured) = union.filter(|r| r.width() >= 1.0 && r.height() >= 1.0) {
            let unscaled = untransform_rect(measured, sc, t_comp, s_comp);
            geoms.0.insert(
                kind,
                WidgetGeom {
                    unscaled,
                    applied_translation: t,
                    applied_scale: s,
                },
            );
        } else if let Some(g) = geoms.0.get_mut(&kind) {
            // Keep last-known unscaled rect; just refresh the applied transform.
            g.applied_translation = t;
            g.applied_scale = s;
        }
    }
}

/// Extract (translation px, uniform scale) from a container's UiTransform.
fn applied_of(tf: &UiTransform) -> (Vec2, f32) {
    let t = match (tf.translation.x, tf.translation.y) {
        (Val::Px(x), Val::Px(y)) => Vec2::new(x, y),
        _ => Vec2::ZERO,
    };
    (t, tf.scale.x.max(f32::EPSILON))
}

/// Position + z-order + visibility for every widget container, via UiTransform.
fn apply_widget_layout(
    layouts: Res<WidgetLayouts>,
    geoms: Res<WidgetGeoms>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    state: Res<crate::editor::PreviewState>,
    mut containers: Query<(
        &WidgetContainer,
        &mut UiTransform,
        Option<&mut ZIndex>,
        &mut Visibility,
    )>,
) {
    // Widgets are placed in FULL-WINDOW space (their normal-play position); the
    // Customize "shrink into a miniature" rides the shared `HudRoot` transform,
    // so this anchors against the whole window, never the shrunk stage rect.
    let Ok(window) = windows.single() else {
        return;
    };
    let rect = crate::stage_rect::StageRect::full(Vec2::new(window.width(), window.height()));
    let sc = rect.center();
    let is_practice = practice.is_some();
    for (container, mut tf, z, mut vis) in &mut containers {
        let inst = layouts.get(container.0);
        match inst.placement {
            dtx_layout::Placement::Natural => {
                // v1 semantics: pure ref-px delta, scale inert.
                tf.translation = Val2::new(
                    Val::Px(inst.offset.0 * pfl.scale),
                    Val::Px(inst.offset.1 * pfl.scale),
                );
                tf.scale = Vec2::ONE;
            }
            dtx_layout::Placement::Anchored => {
                let Some(geom) = geoms.0.get(&container.0) else {
                    // Not measured yet (first frames): leave last transform.
                    continue;
                };
                let size = (geom.unscaled.width(), geom.unscaled.height());
                let parent = parent_rect_px(inst.space, rect, &pfl);
                let desired = dtx_layout::resolve_top_left(
                    inst.anchor,
                    inst.origin,
                    size,
                    inst.scale,
                    (inst.offset.0 * pfl.scale, inst.offset.1 * pfl.scale),
                    parent,
                );
                let t = translation_for(
                    Vec2::new(desired.0, desired.1),
                    geom.unscaled.min,
                    sc,
                    inst.scale,
                );
                tf.translation = Val2::new(Val::Px(t.x), Val::Px(t.y));
                tf.scale = Vec2::splat(inst.scale);
            }
        }
        if let Some(mut z) = z {
            *z = ZIndex(inst.z);
        }
        // On the Customize surface, non-Widgets tabs (settings/bindings/lanes)
        // preview ONLY lanes+notes (HudRoot children, unaffected here), so hide
        // every HUD widget container. The Playfield kind spawns no container, but
        // exempt it defensively in case one is ever added.
        let hide_for_preview = state.open
            && state.tab != game_shell::CustomizeTab::Widgets
            && container.0 != WidgetKind::Playfield;
        *vis = if hide_for_preview {
            Visibility::Hidden
        } else if widget_visible(inst, is_practice) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Hide the practice HUD roots (density strip + mini strip) when previewing a
/// non-Widgets Customize tab, so only lanes+notes show. These are separate roots
/// (not widget containers), so they need their own gate. When practice isn't
/// active the queries match nothing (harmless).
fn hide_practice_hud_on_preview(
    state: Res<crate::editor::PreviewState>,
    mut full_hud: Query<
        &mut Visibility,
        (
            With<crate::practice::hud::full_hud::FullHudRoot>,
            Without<crate::practice::hud::mini_strip::MiniStripRoot>,
        ),
    >,
    mut mini_strip: Query<
        &mut Visibility,
        (
            With<crate::practice::hud::mini_strip::MiniStripRoot>,
            Without<crate::practice::hud::full_hud::FullHudRoot>,
        ),
    >,
) {
    let hide = state.open && state.tab != game_shell::CustomizeTab::Widgets;
    let vis = if hide {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut full_hud {
        *v = vis;
    }
    for mut v in &mut mini_strip {
        *v = vis;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::default_instance;

    #[test]
    fn default_layouts_cover_all_kinds() {
        let l = WidgetLayouts::default();
        for k in WidgetKind::ALL {
            assert_eq!(*l.get(k), default_instance(k));
        }
    }

    #[test]
    fn visibility_respects_mode() {
        let transport = default_instance(WidgetKind::PracticeTransport);
        assert!(!widget_visible(&transport, false));
        assert!(widget_visible(&transport, true));
        let combo = default_instance(WidgetKind::Combo);
        assert!(widget_visible(&combo, false));
        assert!(widget_visible(&combo, true));
    }

    #[test]
    fn transform_math_round_trips() {
        let sc = Vec2::new(640.0, 360.0);
        let t = Vec2::new(37.0, -12.0);
        let s = 1.7;
        let r = Rect::new(100.0, 50.0, 300.0, 120.0);
        let vis = Rect::from_corners(
            transform_point(r.min, sc, t, s),
            transform_point(r.max, sc, t, s),
        );
        let back = untransform_rect(vis, sc, t, s);
        assert!((back.min - r.min).length() < 0.001);
        assert!((back.max - r.max).length() < 0.001);
    }

    #[test]
    fn compose_about_center_matches_nested_transforms() {
        let sc = Vec2::new(872.0, 545.0);
        let (t_in, s_in) = (Vec2::new(40.0, -20.0), 1.5);
        let (t_out, s_out) = (Vec2::new(-120.0, 60.0), 0.6);
        let p = Vec2::new(300.0, 700.0);
        let nested = transform_point(transform_point(p, sc, t_in, s_in), sc, t_out, s_out);
        let (t_c, s_c) = compose_about_center(t_out, s_out, t_in, s_in);
        let composed = transform_point(p, sc, t_c, s_c);
        assert!((nested - composed).length() < 1e-3);
    }

    #[test]
    fn translation_for_places_content() {
        let sc = Vec2::new(640.0, 360.0);
        let u_min = Vec2::new(200.0, 100.0);
        let desired = Vec2::new(50.0, 400.0);
        let s = 2.0;
        let t = translation_for(desired, u_min, sc, s);
        assert!((transform_point(u_min, sc, t, s) - desired).length() < 0.001);
    }

    #[test]
    fn screen_parent_rect_full_window_is_zero_origin() {
        let rect = crate::stage_rect::StageRect::full(Vec2::new(1600.0, 900.0));
        assert_eq!(screen_parent_rect(rect), (0.0, 0.0, 1600.0, 900.0));
    }

    #[test]
    fn screen_parent_rect_offset_uses_origin() {
        let rect = crate::stage_rect::StageRect {
            origin: Vec2::new(220.0, 10.0),
            size: Vec2::new(1000.0, 700.0),
        };
        assert_eq!(screen_parent_rect(rect), (220.0, 10.0, 1000.0, 700.0));
    }

    #[test]
    fn identity_transform_at_defaults() {
        let sc = Vec2::new(640.0, 360.0);
        let u_min = Vec2::new(123.0, 45.0);
        // Natural placement, offset 0 → desired == natural top-left → T == 0.
        let t = translation_for(u_min, u_min, sc, 1.0);
        assert!(t.length() < 0.001);
    }
}
