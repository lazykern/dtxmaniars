//! Settings panel: placement engine invariants + control math.

use bevy::prelude::*;
use dtx_layout::{default_instance, Anchor9, Placement, WidgetKind};
use gameplay_drums::editor::drag::ensure_anchored;
use gameplay_drums::widget_layout::{transform_point, translation_for, untransform_rect};

#[test]
fn natural_default_is_identity() {
    let inst = default_instance(WidgetKind::Combo);
    assert_eq!(inst.placement, Placement::Natural);
    assert_eq!(inst.offset, (0.0, 0.0));
    assert_eq!(inst.scale, 1.0);
}

#[test]
fn anchored_conversion_then_anchor_rewrite_never_moves_widget() {
    let parent = (0.0, 0.0, 1280.0, 720.0);
    let visual = Vec2::new(900.0, 500.0);
    let size = Vec2::new(150.0, 80.0);
    let mut inst = default_instance(WidgetKind::Combo);
    ensure_anchored(&mut inst, visual, size, parent, 1.0);
    // Simulate the panel's anchor-cell rewrite to BottomRight.
    inst.anchor = Anchor9::BottomRight;
    inst.origin = Anchor9::BottomRight;
    let off = dtx_layout::offset_for_top_left(
        inst.anchor,
        inst.origin,
        (size.x, size.y),
        inst.scale,
        (visual.x, visual.y),
        parent,
    );
    inst.offset = (off.0, off.1);
    let tl = dtx_layout::resolve_top_left(
        inst.anchor,
        inst.origin,
        (size.x, size.y),
        inst.scale,
        (inst.offset.0, inst.offset.1),
        parent,
    );
    assert!((tl.0 - visual.x).abs() < 0.001 && (tl.1 - visual.y).abs() < 0.001);
}

#[test]
fn scale_about_screen_center_with_compensation_keeps_top_left() {
    let sc = Vec2::new(640.0, 360.0);
    let unscaled_min = Vec2::new(300.0, 200.0);
    let desired = Vec2::new(300.0, 200.0); // hold position while scaling 2×
    let t = translation_for(desired, unscaled_min, sc, 2.0);
    let vis = transform_point(unscaled_min, sc, t, 2.0);
    assert!((vis - desired).length() < 0.001);
    // And the inversion recovers the unscaled rect.
    let r = Rect::from_corners(unscaled_min, unscaled_min + Vec2::new(100.0, 40.0));
    let vis_rect = Rect::from_corners(
        transform_point(r.min, sc, t, 2.0),
        transform_point(r.max, sc, t, 2.0),
    );
    let back = untransform_rect(vis_rect, sc, t, 2.0);
    assert!((back.min - r.min).length() < 0.001 && (back.max - r.max).length() < 0.001);
}

#[test]
fn layout_file_with_placement_round_trips() {
    let mut map = dtx_layout::SceneSection::default().resolve();
    let combo = map.get_mut(&WidgetKind::Combo).unwrap();
    combo.placement = Placement::Anchored;
    combo.anchor = Anchor9::Center;
    combo.origin = Anchor9::Center;
    combo.offset = (10.0, -5.0);
    combo.scale = 1.5;
    let section = dtx_layout::SceneSection::from_map(&map);
    let back = section.resolve();
    assert_eq!(back[&WidgetKind::Combo].placement, Placement::Anchored);
    assert_eq!(back[&WidgetKind::Combo].anchor, Anchor9::Center);
    assert_eq!(back[&WidgetKind::Combo].scale, 1.5);
}

#[test]
fn v1_file_without_placement_parses_natural() {
    let toml_str = r#"
version = 1
[[scene.widgets]]
kind = "combo"
offset = [40.0, -20.0]
"#;
    let file = dtx_layout::parse_with_migrations(toml_str);
    let map = file.scene.resolve();
    assert_eq!(map[&WidgetKind::Combo].placement, Placement::Natural);
    assert_eq!(map[&WidgetKind::Combo].offset, (40.0, -20.0));
}
