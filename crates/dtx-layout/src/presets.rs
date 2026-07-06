//! Built-in lane presets. More presets added in the presets task.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanePreset {
    /// Current default: NX Type-A geometry, 10 columns.
    Classic,
    /// NX Type-B: pedals share one lane, SD left of pedals.
    NxTypeB,
    /// NX Type-D: symmetric pedals-center arrangement.
    NxTypeD,
    Custom,
}

impl Default for LanePreset {
    fn default() -> Self {
        Self::Classic
    }
}

/// Classic preset — ground-truth port of the old `lane_geometry::COLUMNS`.
pub fn classic() -> crate::lanes::LaneArrangement {
    use crate::lanes::{DisplayLane, LaneArrangement};
    use dtx_core::EChannel;
    use std::collections::HashMap;

    let spec: [(&str, f32, (f32, f32, f32), EChannel); 10] = [
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
