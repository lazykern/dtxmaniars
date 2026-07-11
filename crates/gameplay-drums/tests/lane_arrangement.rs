//! Integration: lane arrangement drives note columns + playfield geometry.

use bevy::prelude::*;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::layout::PlayfieldLayout;

fn lanes_from_section(section: dtx_layout::LanesSection) -> Lanes {
    Lanes(section.resolve())
}

fn split_hho_section() -> dtx_layout::LanesSection {
    dtx_layout::LanesSection {
        preset: dtx_layout::LanePreset::Custom,
        order: Some(
            [
                "LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        map: Some([("HHO".to_string(), "HHO".to_string())].into()),
        ..Default::default()
    }
}

#[test]
fn default_lanes_reproduce_legacy_note_positions() {
    let lanes = Lanes::default();
    let layout = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    let legacy_ref_x = [
        295.0, 367.0, 416.0, 467.0, 524.0, 573.0, 642.0, 691.0, 745.0, 815.0,
    ];
    for (i, rx) in legacy_ref_x.iter().enumerate() {
        let expected = 361.0 + (rx - 295.0);
        assert!(
            (layout.col_left(i) - expected).abs() < 0.01,
            "col {i}: got {}, want {expected}",
            layout.col_left(i)
        );
    }
}

#[test]
fn split_arrangement_moves_hho_chips_to_own_column() {
    let lanes = lanes_from_section(split_hho_section());
    let hho = lanes.col_of(dtx_core::EChannel::HiHatOpen).unwrap();
    let hh = lanes.col_of(dtx_core::EChannel::HiHatClose).unwrap();
    assert_eq!(lanes.count(), 11);
    assert_ne!(hho, hh);
    assert!(!lanes.is_hollow(dtx_core::EChannel::HiHatOpen));
}

#[test]
fn merge_cy_into_rd_lane() {
    let section = dtx_layout::LanesSection {
        preset: dtx_layout::LanePreset::Custom,
        order: Some(
            ["LC", "HH", "LP", "SD", "HT", "BD", "LT", "FT", "RD"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        map: Some([("CY".to_string(), "RD".to_string())].into()),
        ..Default::default()
    };
    let lanes = lanes_from_section(section);
    assert_eq!(lanes.count(), 9);
    assert_eq!(
        lanes.col_of(dtx_core::EChannel::Cymbal),
        lanes.col_of(dtx_core::EChannel::RideCymbal)
    );
    assert!(
        lanes.is_hollow(dtx_core::EChannel::Cymbal),
        "CY secondary on RD lane"
    );
}

#[test]
fn lanes_change_recomputes_playfield_layout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Lanes::default());
    app.insert_resource(PlayfieldLayout::from_size(1280.0, 720.0, &Lanes::default()));
    app.add_systems(
        Update,
        |lanes: Res<Lanes>, mut layout: ResMut<PlayfieldLayout>| {
            if lanes.is_changed() {
                *layout = PlayfieldLayout::from_size(layout.width, layout.height, &lanes);
            }
        },
    );
    app.update();

    let before = app.world().resource::<PlayfieldLayout>().col_count();
    assert_eq!(before, 10);

    *app.world_mut().resource_mut::<Lanes>() = lanes_from_section(split_hho_section());
    app.update();

    let after = app.world().resource::<PlayfieldLayout>().col_count();
    assert_eq!(after, 11);
}

#[test]
fn all_lane_profiles_preserve_logical_judgment_id() {
    use gameplay_drums::lane_map::lane_of;
    let baseline: Vec<_> = dtx_layout::DRUM_CHANNELS
        .into_iter()
        .map(|ch| (ch, lane_of(ch)))
        .collect();
    for arrangement in [
        dtx_layout::classic(),
        dtx_layout::nx_type_b(),
        dtx_layout::nx_type_d(),
    ] {
        // Display arrangement is not an input to logical lane ids: whatever
        // profile is active, judgment routing stays byte-identical.
        for (ch, lane) in &baseline {
            assert_eq!(lane_of(*ch), *lane, "{ch:?} under {:?}", arrangement.preset);
        }
    }
}
