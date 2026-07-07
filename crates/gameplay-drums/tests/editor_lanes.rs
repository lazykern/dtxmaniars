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
    let file = layout_file_from(&WidgetLayouts::default(), &lanes);
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
