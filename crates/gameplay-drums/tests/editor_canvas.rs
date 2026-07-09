//! Editor v2 canvas selection: AABB collection + hover/selection resources.

use bevy::prelude::*;
use dtx_layout::WidgetKind;
use gameplay_drums::editor::picking::{candidates_at, cycle_pick, pick_topmost, WidgetAabbs};

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

#[test]
fn stacked_widgets_pick_by_z_then_area() {
    let mut aabbs = WidgetAabbs::default();
    aabbs
        .0
        .insert(WidgetKind::Playfield, r(100.0, 0.0, 600.0, 700.0));
    aabbs
        .0
        .insert(WidgetKind::Combo, r(300.0, 50.0, 120.0, 60.0));
    aabbs
        .0
        .insert(WidgetKind::JudgmentPopup, r(320.0, 60.0, 80.0, 30.0));
    let z = |_k: WidgetKind| 0;
    // Point inside all three → smallest wins on equal z.
    assert_eq!(
        pick_topmost(&aabbs.0, z, Vec2::new(350.0, 70.0)),
        Some(WidgetKind::JudgmentPopup)
    );
    // Raise combo's z → combo wins.
    let z2 = |k: WidgetKind| if k == WidgetKind::Combo { 3 } else { 0 };
    assert_eq!(
        pick_topmost(&aabbs.0, z2, Vec2::new(350.0, 70.0)),
        Some(WidgetKind::Combo)
    );
}

#[test]
fn alt_cycle_walks_the_stack() {
    let mut aabbs = WidgetAabbs::default();
    aabbs
        .0
        .insert(WidgetKind::Playfield, r(0.0, 0.0, 500.0, 500.0));
    aabbs
        .0
        .insert(WidgetKind::Combo, r(10.0, 10.0, 100.0, 50.0));
    let cands = candidates_at(&aabbs.0, |_| 0, Vec2::new(20.0, 20.0));
    assert_eq!(cands.len(), 2);
    let first = cycle_pick(&cands, None);
    let second = cycle_pick(&cands, first);
    let third = cycle_pick(&cands, second);
    assert_ne!(first, second);
    assert_eq!(first, third, "cycle wraps");
}

#[test]
fn gesture_types_default_to_none() {
    use gameplay_drums::editor::drag::{ActiveGesture, Gesture};
    assert_eq!(ActiveGesture::default().0, Gesture::None);
}
