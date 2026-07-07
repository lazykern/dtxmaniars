//! Editor session + snap invariants.

use dtx_layout::{nearest_anchor, Anchor9, Placement, WidgetKind};

#[test]
fn anchor_auto_default_true_and_survives_file_round_trip() {
    let mut map = dtx_layout::SceneSection::default().resolve();
    assert!(map[&WidgetKind::Combo].anchor_auto);
    let c = map.get_mut(&WidgetKind::Combo).unwrap();
    c.placement = Placement::Anchored;
    c.anchor = Anchor9::BottomRight;
    c.origin = Anchor9::BottomRight;
    c.anchor_auto = false;
    c.offset = (5.0, 5.0);
    let section = dtx_layout::SceneSection::from_map(&map);
    let back = section.resolve();
    assert!(!back[&WidgetKind::Combo].anchor_auto);
}

#[test]
fn drag_path_across_thirds_walks_anchors_without_jumps() {
    // Simulate a widget center sweeping left→right at mid height; anchors
    // must walk Left→Center→Right and every rewrite must be position-exact.
    let parent = (0.0, 0.0, 1280.0, 720.0);
    let size = (100.0, 40.0);
    let mut anchor = Anchor9::CenterLeft;
    let mut offset =
        dtx_layout::offset_for_top_left(anchor, anchor, size, 1.0, (50.0, 340.0), parent);
    let mut seen = vec![anchor];
    for x in (50..1150).step_by(50) {
        let visual = (x as f32, 340.0);
        let frac_x = (visual.0 + size.0 / 2.0) / 1280.0;
        let frac_y = (visual.1 + size.1 / 2.0) / 720.0;
        let want = nearest_anchor(frac_x, frac_y);
        if want != anchor {
            // No-jump rewrite: recompute offset at the same visual position.
            offset = dtx_layout::offset_for_top_left(want, want, size, 1.0, visual, parent);
            anchor = want;
            seen.push(anchor);
        }
        let tl = dtx_layout::resolve_top_left(anchor, anchor, size, 1.0, offset, parent);
        assert!((tl.0 - visual.0).abs() < 0.001, "jump at x={x}");
        // (offset is only exact at rewrite points; between them the caller
        // adds drag deltas — emulate that:)
        offset = dtx_layout::offset_for_top_left(
            anchor,
            anchor,
            size,
            1.0,
            (visual.0 + 50.0, visual.1),
            parent,
        );
    }
    assert_eq!(
        seen,
        vec![Anchor9::CenterLeft, Anchor9::Center, Anchor9::CenterRight]
    );
}

#[test]
fn session_resource_defaults_off() {
    assert!(!game_shell::EditorSession::default().0);
}
