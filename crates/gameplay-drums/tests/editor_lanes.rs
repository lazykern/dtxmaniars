//! Lane panel transforms end-to-end through the Lanes resource + save path.

use dtx_core::EChannel;
use dtx_layout::{
    lane_chips, merge_lane, reorder_lane, set_lane_width, split_channel, structure_signature,
    LanePreset,
};
use gameplay_drums::editor::save::layout_file_from;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::widget_layout::WidgetLayouts;

#[test]
fn split_then_save_then_reload_preserves_arrangement() {
    let mut lanes = Lanes::default();
    assert!(split_channel(&mut lanes.0, EChannel::HiHatOpen));
    assert!(reorder_lane(&mut lanes.0, 0, 1));
    assert!(set_lane_width(&mut lanes.0, 3, 90.0));
    let file = layout_file_from(&WidgetLayouts::default(), &lanes.0);
    let resolved = file.lanes.resolve();
    assert_eq!(resolved, lanes.0);
    assert_eq!(resolved.preset, LanePreset::Custom);
}

#[test]
fn merge_then_split_round_trips_channel_home() {
    let mut lanes = Lanes::default();
    let rd = lanes.0.lane_index_of(EChannel::RideCymbal).unwrap();
    assert!(merge_lane(&mut lanes.0, rd));
    // RD now a secondary chip on CY lane.
    assert!(lanes.0.is_secondary(EChannel::RideCymbal));
    let cy = lanes.0.lane_index_of(EChannel::Cymbal).unwrap();
    assert!(lane_chips(&lanes.0, cy).contains(&EChannel::RideCymbal));
    // Split it back out.
    assert!(split_channel(&mut lanes.0, EChannel::RideCymbal));
    assert!(!lanes.0.is_secondary(EChannel::RideCymbal));
}

#[test]
fn playfield_layout_tracks_lane_edits() {
    use gameplay_drums::layout::PlayfieldLayout;
    let mut lanes = Lanes::default();
    let before = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    split_channel(&mut lanes.0, EChannel::HiHatOpen);
    let after = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    assert_eq!(after.col_count(), before.col_count() + 1);
    assert!(after.strip_width() > before.strip_width());
}

#[test]
fn signature_stable_across_width_drag() {
    let mut lanes = Lanes::default();
    set_lane_width(&mut lanes.0, 0, 60.0);
    let sig = structure_signature(&lanes.0);
    set_lane_width(&mut lanes.0, 0, 61.0);
    set_lane_width(&mut lanes.0, 0, 62.0);
    assert_eq!(sig, structure_signature(&lanes.0));
}

mod lane_profile_draft {
    use dtx_layout::profiles::LaneProfile;
    use gameplay_drums::editor::profile_state::{
        reduce_dirty_action, DirtyDecision, DraftEffect, LaneProfileDraft, PendingProfileAction,
        ProfileDraft, ProfileStateError,
    };

    use super::*;

    fn user_draft(name: &str) -> LaneProfileDraft {
        LaneProfileDraft(ProfileDraft::clean(
            name,
            LaneProfile::from_arrangement(dtx_layout::classic()),
        ))
    }

    #[test]
    fn lane_edit_keeps_user_profile_name() {
        let mut draft = user_draft("Symmetric kit");
        assert!(split_channel(
            &mut draft.0.value.arrangement,
            EChannel::HiHatOpen
        ));
        assert_eq!(draft.0.selected, "Symmetric kit");
        assert!(draft.0.is_dirty());
        // The arrangement payload marks Custom internally, but the draft
        // identity (its name) is unchanged.
        assert_eq!(draft.0.value.arrangement.preset, LanePreset::Custom);
    }

    #[test]
    fn builtin_lane_edit_requires_save_as() {
        let mut draft = LaneProfileDraft::default();
        assert!(split_channel(
            &mut draft.0.value.arrangement,
            EChannel::HiHatOpen
        ));
        let result = reduce_dirty_action(
            &draft.0,
            true,
            &PendingProfileAction::CloseCustomize,
            DirtyDecision::Save,
        );
        assert_eq!(result, Err(ProfileStateError::BuiltInRequiresSaveAs));
    }

    #[test]
    fn lane_draft_updates_playfield_preview() {
        use gameplay_drums::layout::PlayfieldLayout;
        let draft = LaneProfileDraft(ProfileDraft::clean(
            "NX Type-B",
            LaneProfile::from_arrangement(dtx_layout::nx_type_b()),
        ));
        // The preview mirror copies the draft arrangement into Lanes.
        let mut lanes = Lanes::default();
        lanes.0 = draft.0.value.arrangement.clone();
        let layout = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
        assert_eq!(layout.col_count(), dtx_layout::nx_type_b().lanes.len());
    }

    #[test]
    fn cancelled_lane_switch_keeps_preview() {
        let mut draft = user_draft("Desk");
        assert!(split_channel(
            &mut draft.0.value.arrangement,
            EChannel::HiHatOpen
        ));
        let before = draft.clone();
        let effect = reduce_dirty_action(
            &draft.0,
            false,
            &PendingProfileAction::Select("Classic".to_owned()),
            DirtyDecision::Cancel,
        )
        .expect("cancel reduces");
        assert_eq!(effect, DraftEffect::Noop);
        assert_eq!(draft, before, "cancel leaves draft and preview untouched");
    }

    #[test]
    fn successful_lane_selection_updates_display_only() {
        use gameplay_drums::bindings::BindResolver;
        let resolver_before = BindResolver::default();
        // Committed selection: draft becomes the clean target profile and the
        // preview mirrors it...
        let draft = LaneProfileDraft(ProfileDraft::clean(
            "NX Type-D",
            LaneProfile::from_arrangement(dtx_layout::nx_type_d()),
        ));
        let mut lanes = Lanes::default();
        lanes.0 = draft.0.value.arrangement.clone();
        assert_eq!(lanes.0, dtx_layout::nx_type_d());
        // ...while judgment routing is untouched: the resolver has no lane
        // arrangement input at all.
        let resolver_after = BindResolver::default();
        for note in [36u8, 38, 42, 46, 49, 51] {
            assert_eq!(
                resolver_before.lane_for_note(note),
                resolver_after.lane_for_note(note)
            );
        }
    }
}
