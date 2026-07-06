//! `layout.toml` schema + resolution to runtime types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
use crate::presets::{arrangement_for, classic, LanePreset};

/// `[lanes]` section of layout.toml. All fields optional — absent = preset default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LanesSection {
    #[serde(default)]
    pub preset: LanePreset,
    /// Display order (lane ids = channel short names). Only used when
    /// `preset = "custom"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Per-lane ref-px width overrides, keyed by lane id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widths: Option<HashMap<String, f32>>,
    /// Channel→lane overrides, keyed by channel short name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<HashMap<String, String>>,
}

impl LanesSection {
    /// Build the runtime arrangement. Named preset → its table verbatim.
    /// Custom → classic base + order/widths/map overrides, with graceful
    /// fallbacks (unknown ids dropped + warned, unmapped channels repaired).
    pub fn resolve(&self) -> LaneArrangement {
        if self.preset != LanePreset::Custom {
            return arrangement_for(self.preset);
        }

        let base = classic();

        let order: Vec<String> = self
            .order
            .clone()
            .unwrap_or_else(|| base.lanes.iter().map(|l| l.id.clone()).collect())
            .into_iter()
            .filter(|id| {
                let known = channel_from_short(id).is_some();
                if !known {
                    eprintln!("dtx-layout: unknown lane id {id:?} dropped");
                }
                known
            })
            .collect();
        let order = if order.is_empty() {
            base.lanes.iter().map(|l| l.id.clone()).collect()
        } else {
            order
        };

        let mut lanes: Vec<DisplayLane> = order
            .iter()
            .map(|id| {
                base.lanes
                    .iter()
                    .find(|l| &l.id == id)
                    .cloned()
                    .unwrap_or_else(|| {
                        let primary = channel_from_short(id).expect("filtered above");
                        DisplayLane {
                            id: id.clone(),
                            label: id.clone(),
                            width: default_lane_width(primary),
                            color: None,
                            primary,
                        }
                    })
            })
            .collect();

        if let Some(widths) = &self.widths {
            for lane in &mut lanes {
                if let Some(w) = widths.get(&lane.id) {
                    lane.width = w.clamp(MIN_LANE_WIDTH, MAX_LANE_WIDTH);
                }
            }
        }

        let mut map: HashMap<dtx_core::EChannel, String> = base.map.clone();
        if let Some(overrides) = &self.map {
            for (ch_name, lane_id) in overrides {
                let Some(ch) = channel_from_short(ch_name) else {
                    eprintln!("dtx-layout: unknown channel {ch_name:?} in map dropped");
                    continue;
                };
                if lanes.iter().any(|l| &l.id == lane_id) {
                    map.insert(ch, lane_id.clone());
                } else {
                    eprintln!("dtx-layout: map target lane {lane_id:?} unknown, dropped");
                }
            }
        }
        for ch in DRUM_CHANNELS {
            let id = map.get(&ch).cloned().unwrap_or_default();
            if !lanes.iter().any(|l| l.id == id) {
                map.insert(ch, lanes[0].id.clone());
            }
        }

        LaneArrangement {
            preset: LanePreset::Custom,
            lanes,
            map,
        }
    }

    /// Inverse of `resolve` for saving (always writes the explicit custom form
    /// unless the arrangement IS a named preset).
    pub fn from_arrangement(arr: &LaneArrangement) -> Self {
        if arr.preset != LanePreset::Custom {
            return Self {
                preset: arr.preset,
                ..Default::default()
            };
        }
        Self {
            preset: LanePreset::Custom,
            order: Some(arr.lanes.iter().map(|l| l.id.clone()).collect()),
            widths: Some(arr.lanes.iter().map(|l| (l.id.clone(), l.width)).collect()),
            map: Some(
                arr.map
                    .iter()
                    .filter_map(|(ch, id)| {
                        channel_short_name(*ch).map(|n| (n.to_string(), id.clone()))
                    })
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn empty_section_resolves_to_classic() {
        let arr = LanesSection::default().resolve();
        assert_eq!(arr, crate::presets::classic());
    }

    #[test]
    fn named_preset_wins_over_order() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::NxTypeB,
            order: Some(vec!["RD".into(), "LC".into()]),
            ..Default::default()
        };
        assert_eq!(section.resolve(), crate::presets::nx_type_b());
    }

    #[test]
    fn custom_order_reorders_and_splits() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
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
        };
        let arr = section.resolve();
        assert_eq!(arr.lanes.len(), 11);
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        assert_ne!(hho, hh, "HHO split out into its own lane");
        assert_eq!(arr.lanes[hho].id, "HHO");
        assert!(
            !arr.is_secondary(EChannel::HiHatOpen),
            "own lane => primary"
        );
    }

    #[test]
    fn custom_widths_clamped() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            widths: Some([("SD".to_string(), 999.0), ("BD".to_string(), 1.0)].into()),
            ..Default::default()
        };
        let arr = section.resolve();
        let sd = arr.lane_index_of(EChannel::Snare).unwrap();
        let bd = arr.lane_index_of(EChannel::BassDrum).unwrap();
        assert_eq!(arr.lanes[sd].width, crate::lanes::MAX_LANE_WIDTH);
        assert_eq!(arr.lanes[bd].width, crate::lanes::MIN_LANE_WIDTH);
    }

    #[test]
    fn unknown_lane_ids_and_channels_dropped() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(vec!["HH".into(), "NOPE".into(), "SD".into()]),
            map: Some(
                [
                    ("XX".to_string(), "HH".to_string()),
                    ("CY".to_string(), "NOPE".to_string()),
                ]
                .into(),
            ),
            ..Default::default()
        };
        let arr = section.resolve();
        assert!(arr.lanes.iter().all(|l| l.id != "NOPE"));
        assert!(arr.lane_index_of(EChannel::Cymbal).is_some());
    }

    #[test]
    fn channels_never_unmapped_even_when_lane_removed() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(vec!["SD".into(), "BD".into()]),
            ..Default::default()
        };
        let arr = section.resolve();
        for ch in crate::lanes::DRUM_CHANNELS {
            assert!(arr.lane_index_of(ch).is_some(), "{ch:?} must stay mapped");
        }
    }

    #[test]
    fn resolve_round_trips_through_section() {
        let arr = crate::presets::nx_type_d();
        let section = LanesSection::from_arrangement(&arr);
        assert_eq!(section.resolve(), arr);
    }
}
