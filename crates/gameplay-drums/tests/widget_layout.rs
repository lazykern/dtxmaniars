//! Integration: widget layout resource drives container transform + visibility.

use bevy::prelude::Vec2;
use dtx_layout::{default_instance, WidgetKind, WidgetKindField};
use gameplay_drums::widget_layout::{repair_widget_top_left, widget_visible, WidgetLayouts};

#[test]
fn default_layouts_have_zero_offset_for_parity() {
    let l = WidgetLayouts::default();
    for k in WidgetKind::ALL {
        assert_eq!(
            l.get(k).offset,
            (0.0, 0.0),
            "{k:?} default offset must be 0 (parity)"
        );
        assert_eq!(
            l.get(k).z,
            0,
            "{k:?} default z must be 0 so applied stacking == original paint order"
        );
    }
}

#[test]
fn score_panel_hidden_in_practice_shown_in_play() {
    let l = WidgetLayouts::default();
    let t = l.get(WidgetKind::ScorePanel);
    assert!(widget_visible(t, false));
    assert!(!widget_visible(t, true));
}

#[test]
fn custom_offset_flows_through_resolve() {
    let section = dtx_layout::SceneSection {
        widgets: vec![dtx_layout::WidgetEntry {
            kind: WidgetKindField::Known(WidgetKind::Combo),
            space: dtx_layout::AnchorSpace::Screen,
            placement: dtx_layout::Placement::Natural,
            anchor: dtx_layout::Anchor9::TopLeft,
            origin: dtx_layout::Anchor9::TopLeft,
            anchor_auto: true,
            offset: [50.0, -30.0],
            scale: 1.0,
            z: 10,
            visible_play: true,
            visible_practice: true,
        }],
    };
    let map = section.resolve();
    let layouts = WidgetLayouts(map);
    assert_eq!(layouts.get(WidgetKind::Combo).offset, (50.0, -30.0));
    assert_eq!(
        *layouts.get(WidgetKind::ScorePanel),
        default_instance(WidgetKind::ScorePanel)
    );
}

#[test]
fn offscreen_saved_position_gets_a_runtime_only_repair() {
    let saved = Vec2::new(1400.0, 900.0);
    let repaired = repair_widget_top_left(saved, Vec2::new(200.0, 80.0), Vec2::new(1280.0, 720.0));
    assert_ne!(repaired, saved);
    assert_eq!(saved, Vec2::new(1400.0, 900.0));
}
