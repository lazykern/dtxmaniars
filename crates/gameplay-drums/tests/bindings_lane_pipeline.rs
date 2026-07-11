use dtx_core::EChannel;
use dtx_input::KeyCode;
use dtx_input::{BindSource, InputBindings};
use gameplay_drums::bindings::BindResolver;
use gameplay_drums::lane_map::{lane_channel, lane_of};
use gameplay_drums::lanes::Lanes;

#[test]
fn persisted_bindings_keep_logical_channels_across_lane_presets() {
    let dir = std::env::temp_dir().join(format!(
        "dtxmaniars-bindings-lane-pipeline-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("bindings.toml");

    let mut bindings = InputBindings::default();
    bindings.bind(EChannel::Snare, BindSource::Key(KeyCode::KeyQ));
    bindings.bind(EChannel::Snare, BindSource::Midi { note: 99 });
    dtx_input::save_bindings(&path, &bindings).expect("save test bindings");

    let loaded = dtx_input::load_bindings(&path);
    let resolver = BindResolver::from_bindings(&loaded);
    let snare_lane = lane_of(EChannel::Snare).expect("snare logical lane");

    assert_eq!(resolver.lane_for_key(KeyCode::KeyQ), Some(snare_lane));
    assert_eq!(resolver.lane_for_note(99), Some(snare_lane));
    assert_eq!(lane_channel(snare_lane), Some(EChannel::Snare));

    for arrangement in [
        dtx_layout::classic(),
        dtx_layout::nx_type_b(),
        dtx_layout::nx_type_d(),
    ] {
        let expected_col = arrangement
            .lane_index_of(EChannel::Snare)
            .expect("snare display column");
        let lanes = Lanes(arrangement);
        assert_eq!(lanes.col_of(EChannel::Snare), Some(expected_col));
    }

    let type_b = Lanes(dtx_layout::nx_type_b());
    assert_ne!(
        lane_of(EChannel::LeftPedal),
        lane_of(EChannel::LeftBassDrum)
    );
    assert_eq!(
        type_b.col_of(EChannel::LeftPedal),
        type_b.col_of(EChannel::LeftBassDrum)
    );

    let _ = std::fs::remove_dir_all(dir);
}
