//! Built-in lane presets. More presets added in the presets task.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanePreset {
    /// Current default: NX Type-A geometry, 10 columns.
    #[default]
    Classic,
    /// NX Type-B: pedals share one lane, SD left of pedals.
    NxTypeB,
    /// NX Type-D: symmetric pedals-center arrangement.
    NxTypeD,
    Custom,
}

/// One column of the classic arrangement: id, width, RGB color, source channel.
type LaneSpec = (&'static str, f32, (f32, f32, f32), dtx_core::EChannel);

/// Classic preset — ground-truth port of the old `lane_geometry::COLUMNS`.
pub fn classic() -> crate::lanes::LaneArrangement {
    use crate::lanes::{DisplayLane, LaneArrangement};
    use dtx_core::EChannel;
    use std::collections::HashMap;

    let spec: [LaneSpec; 10] = [
        ("LC", 72.0, (0.945, 0.247, 0.725), EChannel::LeftCymbal),
        ("HH", 49.0, (0.000, 0.541, 1.000), EChannel::HiHatClose),
        ("LP", 51.0, (1.000, 0.353, 0.627), EChannel::LeftPedal),
        ("SD", 57.0, (0.941, 0.824, 0.000), EChannel::Snare),
        ("HT", 49.0, (0.157, 0.765, 0.157), EChannel::HighTom),
        ("BD", 69.0, (0.588, 0.353, 0.941), EChannel::BassDrum),
        ("LT", 49.0, (0.882, 0.176, 0.176), EChannel::LowTom),
        ("FT", 54.0, (1.000, 0.659, 0.000), EChannel::FloorTom),
        ("CY", 70.0, (1.000, 0.471, 0.000), EChannel::Cymbal),
        ("RD", 38.0, (0.000, 0.541, 1.000), EChannel::RideCymbal),
    ];

    let lanes = spec
        .iter()
        .map(|(id, w, c, primary)| DisplayLane {
            id: (*id).to_string(),
            label: (*id).to_string(),
            width: *w,
            color: Some(*c),
            primary: *primary,
        })
        .collect();

    let mut map = HashMap::new();
    for (id, _, _, primary) in &spec {
        map.insert(*primary, (*id).to_string());
    }
    map.insert(EChannel::HiHatOpen, "HH".to_string());
    map.insert(EChannel::LeftBassDrum, "BD".to_string());

    LaneArrangement {
        preset: LanePreset::Classic,
        lanes,
        map,
    }
}

use dtx_core::EChannel;

fn arrangement_from(
    preset: LanePreset,
    order: &[&str],
    extra_map: &[(EChannel, &str)],
) -> crate::lanes::LaneArrangement {
    use crate::lanes::{channel_from_short, default_lane_width, DisplayLane};
    use std::collections::HashMap;

    let classic = classic();
    let lanes: Vec<DisplayLane> = order
        .iter()
        .map(|id| {
            classic
                .lanes
                .iter()
                .find(|l| l.id == *id)
                .cloned()
                .unwrap_or_else(|| {
                    let primary =
                        channel_from_short(id).expect("preset lane ids are channel short names");
                    DisplayLane {
                        id: (*id).to_string(),
                        label: (*id).to_string(),
                        width: default_lane_width(primary),
                        color: None,
                        primary,
                    }
                })
        })
        .collect();

    let mut map: HashMap<EChannel, String> = classic
        .map
        .iter()
        .map(|(ch, id)| (*ch, id.clone()))
        .collect();
    for (ch, id) in extra_map {
        map.insert(*ch, (*id).to_string());
    }
    for ch in crate::lanes::DRUM_CHANNELS {
        let id = map.get(&ch).cloned().unwrap_or_default();
        if !lanes.iter().any(|l| l.id == id) {
            map.insert(ch, lanes[0].id.clone());
        }
    }

    crate::lanes::LaneArrangement { preset, lanes, map }
}

/// NX Type-B ("summarize 2 pedals"): LBD joins LP's lane, SD moves left of
/// the pedals. Reference x-table `{370,419,533,596,645,748,694,373,815,298,476,476}`.
pub fn nx_type_b() -> crate::lanes::LaneArrangement {
    arrangement_from(
        LanePreset::NxTypeB,
        &["LC", "HH", "SD", "LP", "BD", "HT", "LT", "FT", "CY", "RD"],
        &[(EChannel::LeftBassDrum, "LP")],
    )
}

/// NX Type-D (left-right symmetric, pedals center). Reference x-table
/// `{370,419,582,476,645,748,694,373,815,298,525,527}`.
pub fn nx_type_d() -> crate::lanes::LaneArrangement {
    arrangement_from(
        LanePreset::NxTypeD,
        &["LC", "HH", "SD", "HT", "LP", "BD", "LT", "FT", "CY", "RD"],
        &[(EChannel::LeftBassDrum, "LP")],
    )
}

/// Table lookup used by file resolution + (later) the editor preset dropdown.
pub fn arrangement_for(preset: LanePreset) -> crate::lanes::LaneArrangement {
    match preset {
        LanePreset::Classic => classic(),
        LanePreset::NxTypeB => nx_type_b(),
        LanePreset::NxTypeD => nx_type_d(),
        LanePreset::Custom => classic(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::DRUM_CHANNELS;

    fn assert_complete(arr: &crate::lanes::LaneArrangement) {
        for ch in DRUM_CHANNELS {
            let idx = arr.lane_index_of(ch);
            assert!(idx.is_some(), "{ch:?} must map to an existing lane");
        }
        for lane in &arr.lanes {
            assert!(
                crate::lanes::channel_from_short(&lane.id).is_some(),
                "lane id {} must be a channel short name",
                lane.id
            );
        }
    }

    #[test]
    fn all_presets_are_complete() {
        for arr in [classic(), nx_type_b(), nx_type_d()] {
            assert_complete(&arr);
        }
    }

    #[test]
    fn classic_matches_legacy_columns() {
        let arr = classic();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD"]
        );
        assert!((arr.strip_ref_width() - 558.0).abs() < 0.01);
    }

    #[test]
    fn type_b_merges_pedals_into_one_lane() {
        use dtx_core::EChannel;
        let arr = nx_type_b();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "SD", "LP", "BD", "HT", "LT", "FT", "CY", "RD"]
        );
        assert_eq!(
            arr.lane_index_of(EChannel::LeftBassDrum),
            arr.lane_index_of(EChannel::LeftPedal),
            "Type-B shares one pedal lane"
        );
        assert_ne!(
            arr.lane_index_of(EChannel::BassDrum),
            arr.lane_index_of(EChannel::LeftPedal)
        );
    }

    #[test]
    fn type_d_is_pedals_center_symmetric_order() {
        let arr = nx_type_d();
        let labels: Vec<&str> = arr.lanes.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            labels,
            ["LC", "HH", "SD", "HT", "LP", "BD", "LT", "FT", "CY", "RD"]
        );
    }

    #[test]
    fn preset_serde_names_are_kebab() {
        assert_eq!(
            toml::to_string(&std::collections::BTreeMap::from([(
                "p",
                LanePreset::NxTypeB
            )]))
            .unwrap()
            .trim(),
            r#"p = "nx-type-b""#
        );
    }
}
