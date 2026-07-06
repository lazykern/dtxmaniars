//! Editor logic integration (pure paths; UI systems need a display).

use bevy::prelude::*;
use dtx_layout::WidgetKind;
use gameplay_drums::editor::drag::apply_drag;
use gameplay_drums::editor::save::{layout_file_from, next_lane_preset, reset_all_widgets};
use gameplay_drums::editor::undo::{Snapshot, UndoStack};
use gameplay_drums::editor::EditorOpen;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::widget_layout::WidgetLayouts;

#[test]
fn drag_moves_selected_widget_offset() {
    let start = WidgetLayouts::default().get(WidgetKind::Combo).offset;
    let moved = apply_drag(start, Vec2::new(40.0, 20.0), 1.0);
    assert_eq!(moved, (40.0, 20.0));
}

#[test]
fn full_edit_save_reload_cycle() {
    let mut layouts = WidgetLayouts::default();
    layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (25.0, -10.0);
    let lanes = Lanes::default();
    let file = layout_file_from(&layouts, &lanes);
    let toml = toml::to_string_pretty(&file).unwrap();
    let back: dtx_layout::LayoutFile = toml::from_str(&toml).unwrap();
    assert_eq!(
        back.scene.resolve()[&WidgetKind::Combo].offset,
        (25.0, -10.0)
    );
}

#[test]
fn undo_restores_reset_all() {
    let mut layouts = WidgetLayouts::default();
    layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (50.0, 50.0);
    let lanes = Lanes::default();
    let mut stack = UndoStack::default();
    stack.push(&layouts, &lanes);
    reset_all_widgets(&mut layouts);
    assert_eq!(layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
    let snap = Snapshot {
        layouts: layouts.clone(),
        lanes: lanes.clone(),
    };
    let restored = stack.undo(snap).unwrap();
    assert_eq!(restored.layouts.get(WidgetKind::Combo).offset, (50.0, 50.0));
}

#[test]
fn preset_cycle_advances() {
    let mut lanes = Lanes::default();
    let c0 = lanes.count();
    next_lane_preset(&mut lanes);
    assert_eq!(lanes.0.preset, dtx_layout::LanePreset::NxTypeB);
    assert_eq!(lanes.count(), c0);
}

#[test]
fn editor_open_default_false() {
    assert!(!EditorOpen::default().0);
}
