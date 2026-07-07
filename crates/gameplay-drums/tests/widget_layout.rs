//! Integration: widget layout resource drives container transform + visibility.

use dtx_layout::{default_instance, WidgetKind};
use gameplay_drums::widget_layout::{widget_visible, WidgetLayouts};

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
fn practice_transport_hidden_in_play_shown_in_practice() {
    let l = WidgetLayouts::default();
    let t = l.get(WidgetKind::PracticeTransport);
    assert!(!widget_visible(t, false));
    assert!(widget_visible(t, true));
}

#[test]
fn custom_offset_flows_through_resolve() {
    let section = dtx_layout::SceneSection {
        widgets: vec![dtx_layout::WidgetEntry {
            kind: WidgetKind::Combo,
            space: dtx_layout::AnchorSpace::Screen,
            placement: dtx_layout::Placement::Natural,
            anchor: dtx_layout::Anchor9::TopLeft,
            origin: dtx_layout::Anchor9::TopLeft,
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
