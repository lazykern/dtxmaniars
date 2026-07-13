//! `layout.toml` schema + resolution to runtime types.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
use crate::presets::{arrangement_for, classic, LanePreset};
use crate::scene::SceneSection;

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
    /// Channels intentionally hidden (`lane_edit::hide_lane`) — no lane, but
    /// still judged. Distinguishes "deliberately unassigned" from "corrupt
    /// map entry", which still gets auto-repaired onto `lanes[0]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<Vec<String>>,
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

        let mut seen = HashSet::new();
        let order: Vec<String> = self
            .order
            .clone()
            .unwrap_or_else(|| base.lanes.iter().map(|l| l.id.clone()).collect())
            .into_iter()
            .filter(|id| {
                if channel_from_short(id).is_none() {
                    eprintln!("dtx-layout: unknown lane id {id:?} dropped");
                    return false;
                }
                if !seen.insert(id.clone()) {
                    eprintln!("dtx-layout: duplicate lane id {id:?} dropped");
                    return false;
                }
                true
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
        let hidden: HashSet<dtx_core::EChannel> = self
            .hidden
            .iter()
            .flatten()
            .filter_map(|name| channel_from_short(name))
            .collect();
        for ch in DRUM_CHANNELS {
            if hidden.contains(&ch) {
                map.remove(&ch);
                continue;
            }
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
        let hidden: Vec<String> = DRUM_CHANNELS
            .into_iter()
            .filter(|ch| !arr.map.contains_key(ch))
            .filter_map(channel_short_name)
            .map(str::to_string)
            .collect();
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
            hidden: if hidden.is_empty() {
                None
            } else {
                Some(hidden)
            },
        }
    }
}

pub const LATEST_VERSION: u32 = 1;

/// Whole layout.toml. Plan 2 adds `scene: SceneSection` for HUD widgets —
/// the schema is intentionally a struct (not just lanes) from day one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub lanes: LanesSection,
    #[serde(default)]
    pub scene: SceneSection,
}

impl Default for LayoutFile {
    fn default() -> Self {
        Self {
            version: LATEST_VERSION,
            lanes: LanesSection::default(),
            scene: SceneSection::default(),
        }
    }
}

/// Parse raw TOML, running the version migration chain. Best-effort on
/// newer-than-known versions (parse what matches, warn).
pub fn parse_checked(raw: &str) -> Result<LayoutFile, toml::de::Error> {
    let mut file: LayoutFile = toml::from_str(raw)?;
    if file.version <= LATEST_VERSION {
        file.version = LATEST_VERSION;
    }
    Ok(file)
}

pub fn parse_with_migrations(raw: &str) -> LayoutFile {
    let mut file: LayoutFile = match parse_checked(raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dtx-layout: parse failed: {e}; using defaults");
            return LayoutFile::default();
        }
    };
    if file.version > LATEST_VERSION {
        eprintln!(
            "dtx-layout: layout.toml version {} newer than supported {}; best-effort load",
            file.version, LATEST_VERSION
        );
        return file;
    }
    #[allow(clippy::single_match)]
    match file.version {
        0 => file.version = 1,
        _ => {}
    }
    file
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
    fn duplicate_custom_lane_ids_keep_first_occurrence() {
        let section = LanesSection {
            preset: crate::presets::LanePreset::Custom,
            order: Some(vec!["HH".into(), "SD".into(), "HH".into(), "BD".into()]),
            ..Default::default()
        };

        let arr = section.resolve();
        let ids: Vec<&str> = arr.lanes.iter().map(|lane| lane.id.as_str()).collect();

        assert_eq!(ids, ["HH", "SD", "BD"]);
        assert_eq!(arr.lane_index_of(EChannel::HiHatClose), Some(0));
    }

    #[test]
    fn hidden_lane_survives_section_round_trip() {
        // Registry completeness: an arrangement with an intentionally hidden
        // channel (`lane_edit::hide_lane`) must NOT be "repaired" back onto
        // some lane on save/load — that would silently undo the hide.
        let mut arr = crate::presets::classic();
        crate::lane_edit::hide_lane(&mut arr, 0);
        assert!(!crate::lane_edit::unassigned_channels(&arr).is_empty());

        let section = LanesSection::from_arrangement(&arr);
        assert!(section.hidden.is_some(), "hidden channels are recorded");
        let resolved = section.resolve();
        assert_eq!(resolved, arr, "hide survives a full serde round trip");

        let raw = toml::to_string_pretty(&section).expect("section serializes");
        let parsed: LanesSection = toml::from_str(&raw).expect("section parses");
        assert_eq!(parsed.resolve(), arr, "hide survives TOML text round trip");
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

    #[test]
    fn layout_file_round_trip() {
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_b()),
            scene: SceneSection::default(),
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let back: LayoutFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(back, file);
    }

    #[test]
    fn missing_file_loads_defaults() {
        let loaded = crate::load(std::path::Path::new("/nonexistent/layout.toml"));
        assert_eq!(loaded.lanes.resolve(), crate::presets::classic());
    }

    #[test]
    fn corrupt_file_loads_defaults() {
        let dir = std::env::temp_dir().join("dtx-layout-test-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layout.toml");
        std::fs::write(&path, "this is [ not toml").unwrap();
        let loaded = crate::load(&path);
        assert_eq!(loaded.lanes.resolve(), crate::presets::classic());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join("dtx-layout-test-save");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layout.toml");
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_d()),
            scene: SceneSection::default(),
        };
        crate::save(&path, &file).unwrap();
        let loaded = crate::load(&path);
        assert_eq!(loaded, file);
    }

    #[test]
    fn version_zero_migrates_to_latest() {
        let loaded: LayoutFile = parse_with_migrations("[lanes]\npreset = \"classic\"\n");
        assert_eq!(loaded.version, LATEST_VERSION);
    }

    #[test]
    fn newer_version_still_parses_best_effort() {
        let loaded: LayoutFile =
            parse_with_migrations("version = 999\n[lanes]\npreset = \"nx-type-b\"\n");
        assert_eq!(loaded.lanes.preset, crate::presets::LanePreset::NxTypeB);
    }

    #[test]
    fn layout_file_round_trips_lanes_and_scene() {
        let mut scene = crate::scene::SceneSection::default().resolve();
        scene.get_mut(&crate::WidgetKind::Combo).unwrap().offset = (7.0, 8.0);
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_b()),
            scene: crate::scene::SceneSection::from_map(&scene),
        };
        let s = toml::to_string_pretty(&file).unwrap();
        let back: LayoutFile = toml::from_str(&s).unwrap();
        assert_eq!(back, file);
        assert_eq!(
            back.scene.resolve()[&crate::WidgetKind::Combo].offset,
            (7.0, 8.0)
        );
    }
}
