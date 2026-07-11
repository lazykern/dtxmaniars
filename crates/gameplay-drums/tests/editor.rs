//! Editor logic integration (pure paths; UI systems need a display).

use bevy::prelude::*;
use dtx_layout::WidgetKind;
use gameplay_drums::editor::drag::apply_drag;
use gameplay_drums::editor::save::{layout_file_from, reset_all_widgets};
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
    let file = layout_file_from(&layouts, &dtx_layout::classic());
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

mod compatibility_snapshot {
    use gameplay_drums::editor::profile_state::{
        dirty_profile_kinds, LaneProfileDraft, ProfileDraft, ProfileKind, ProfileSession,
    };

    use super::*;

    #[test]
    fn widget_save_keeps_scene_and_active_lane_snapshot() {
        let mut layouts = WidgetLayouts::default();
        layouts.0.get_mut(&WidgetKind::Combo).unwrap().offset = (7.0, 8.0);
        let committed = dtx_layout::nx_type_b();
        let file = layout_file_from(&layouts, &committed);
        assert_eq!(file.scene.resolve()[&WidgetKind::Combo].offset, (7.0, 8.0));
        assert_eq!(file.lanes.resolve(), committed);
    }

    #[test]
    fn ctrl_s_does_not_clean_lane_profile_draft() {
        // The save path reads only the committed (saved) arrangement; the
        // dirty draft value is not an input and stays dirty.
        let mut draft = LaneProfileDraft::default();
        dtx_layout::split_channel(
            &mut draft.0.value.arrangement,
            dtx_core::EChannel::HiHatOpen,
        );
        assert!(draft.0.is_dirty());
        let file = layout_file_from(&WidgetLayouts::default(), &draft.0.saved.arrangement);
        assert_eq!(file.lanes.resolve(), dtx_layout::classic());
        assert!(
            draft.0.is_dirty(),
            "widget save never cleans profile drafts"
        );
    }

    #[test]
    fn layout_close_save_does_not_commit_profile_drafts() {
        let mut session = ProfileSession::default();
        dtx_layout::split_channel(
            &mut session.lanes.value.arrangement,
            dtx_core::EChannel::HiHatOpen,
        );
        let file = layout_file_from(&WidgetLayouts::default(), &session.lanes.saved.arrangement);
        // Snapshot equals the last committed profile, not the edited draft.
        assert_eq!(file.lanes.resolve(), dtx_layout::classic());
        assert_eq!(dirty_profile_kinds(&session), vec![ProfileKind::Lanes]);
    }

    #[test]
    fn registry_remains_authoritative_after_compatibility_snapshot_write() {
        let dir = std::env::temp_dir()
            .join("dtx-compat-snapshot")
            .join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout_path = dir.join("layout.toml");
        let registry_path = dir.join("lane-profiles.toml");
        // Registry holds NX Type-D; the compatibility snapshot writes B.
        let mut registry = dtx_layout::profiles::lane_registry();
        registry.active = "NX Type-D".to_owned();
        dtx_layout::profiles::save_lane_registry(&registry_path, &registry).expect("registry");
        let file = layout_file_from(&WidgetLayouts::default(), &dtx_layout::nx_type_b());
        dtx_layout::save(&layout_path, &file).expect("layout");
        // Startup keeps the registry authoritative and ignores the snapshot.
        let (_, startup) =
            dtx_layout::profiles::load_layout_with_lane_authority(&layout_path, &registry_path)
                .expect("load");
        let dtx_layout::profiles::LaneRegistryStartup::Ready(loaded) = startup else {
            panic!("registry loads");
        };
        assert_eq!(loaded.active, "NX Type-D");
        assert_eq!(
            dtx_layout::profiles::active_lane_arrangement(&loaded),
            dtx_layout::nx_type_d()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn editor_open_default_false() {
    assert!(!EditorOpen::default().0);
}
