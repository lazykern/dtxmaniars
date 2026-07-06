//! `[scene.gameplay]` layout.toml section for HUD widget placement.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::widgets::{
    Anchor9, AnchorSpace, WidgetInstance, WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE,
};

/// Default instance for a kind (offset 0 ⇒ today's on-screen position via the
/// container-at-origin technique). z stays 0 for every kind so the applied
/// stacking collapses to spawn/tree order — byte-identical to the pre-registry
/// paint order, keeping runtime chips/beat-lines (z 0 on HudRoot) in front of
/// the text widgets exactly as before. The editor sets non-zero z only when the
/// user reorders. Practice widgets are hidden in play.
pub fn default_instance(kind: WidgetKind) -> WidgetInstance {
    let (vis_play, vis_practice) = match kind {
        WidgetKind::PracticeTransport => (false, true),
        _ => (true, true),
    };
    WidgetInstance {
        kind,
        space: AnchorSpace::Screen,
        anchor: Anchor9::TopLeft,
        origin: Anchor9::TopLeft,
        offset: (0.0, 0.0),
        scale: 1.0,
        z: 0,
        visible_play: vis_play,
        visible_practice: vis_practice,
    }
}

/// One serialized widget entry ([[scene.gameplay.widgets]]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetEntry {
    pub kind: WidgetKind,
    #[serde(default = "default_space")]
    pub space: AnchorSpace,
    #[serde(default = "default_anchor")]
    pub anchor: Anchor9,
    #[serde(default = "default_anchor")]
    pub origin: Anchor9,
    #[serde(default)]
    pub offset: [f32; 2],
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub z: i32,
    #[serde(default = "default_true")]
    pub visible_play: bool,
    #[serde(default = "default_true")]
    pub visible_practice: bool,
}

fn default_space() -> AnchorSpace {
    AnchorSpace::Screen
}
fn default_anchor() -> Anchor9 {
    Anchor9::TopLeft
}
fn default_scale() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}

impl WidgetEntry {
    fn to_instance(&self) -> WidgetInstance {
        WidgetInstance {
            kind: self.kind,
            space: self.space,
            anchor: self.anchor,
            origin: self.origin,
            offset: (self.offset[0], self.offset[1]),
            scale: self.scale.clamp(MIN_WIDGET_SCALE, MAX_WIDGET_SCALE),
            z: self.z,
            visible_play: self.visible_play,
            visible_practice: self.visible_practice,
        }
    }

    fn from_instance(i: &WidgetInstance) -> Self {
        Self {
            kind: i.kind,
            space: i.space,
            anchor: i.anchor,
            origin: i.origin,
            offset: [i.offset.0, i.offset.1],
            scale: i.scale,
            z: i.z,
            visible_play: i.visible_play,
            visible_practice: i.visible_practice,
        }
    }
}

/// `[scene.gameplay]` section: a list of widget entries (v1: ≤1 per kind).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<WidgetEntry>,
}

impl SceneSection {
    /// Full resolved map: every `WidgetKind` present, file entries overriding
    /// code defaults. Unknown/duplicate kinds: first wins, extras warned+dropped.
    pub fn resolve(&self) -> HashMap<WidgetKind, WidgetInstance> {
        let mut map: HashMap<WidgetKind, WidgetInstance> = WidgetKind::ALL
            .into_iter()
            .map(|k| (k, default_instance(k)))
            .collect();
        let mut seen = std::collections::HashSet::new();
        for entry in &self.widgets {
            if !seen.insert(entry.kind) {
                eprintln!(
                    "dtx-layout: duplicate widget {:?} in [scene.gameplay], extra dropped",
                    entry.kind
                );
                continue;
            }
            map.insert(entry.kind, entry.to_instance());
        }
        map
    }

    /// Build a section from a resolved map, writing only entries that differ
    /// from the code default (keeps the file minimal).
    pub fn from_map(map: &HashMap<WidgetKind, WidgetInstance>) -> Self {
        let mut widgets: Vec<WidgetEntry> = WidgetKind::ALL
            .into_iter()
            .filter_map(|k| {
                let inst = map.get(&k)?;
                if *inst != default_instance(k) {
                    Some(WidgetEntry::from_instance(inst))
                } else {
                    None
                }
            })
            .collect();
        widgets.sort_by_key(|w| format!("{:?}", w.kind));
        Self { widgets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scene_resolves_all_kinds_to_defaults() {
        let map = SceneSection::default().resolve();
        assert_eq!(map.len(), WidgetKind::ALL.len());
        for k in WidgetKind::ALL {
            assert_eq!(map[&k], default_instance(k));
        }
    }

    #[test]
    fn practice_transport_hidden_in_play_by_default() {
        let d = default_instance(WidgetKind::PracticeTransport);
        assert!(!d.visible_play);
        assert!(d.visible_practice);
    }

    #[test]
    fn file_entry_overrides_default() {
        let section = SceneSection {
            widgets: vec![WidgetEntry {
                kind: WidgetKind::Combo,
                space: AnchorSpace::Screen,
                anchor: Anchor9::TopLeft,
                origin: Anchor9::TopLeft,
                offset: [40.0, -20.0],
                scale: 1.5,
                z: 12,
                visible_play: true,
                visible_practice: true,
            }],
        };
        let map = section.resolve();
        assert_eq!(map[&WidgetKind::Combo].offset, (40.0, -20.0));
        assert_eq!(map[&WidgetKind::Combo].scale, 1.5);
        assert_eq!(
            map[&WidgetKind::ScorePanel],
            default_instance(WidgetKind::ScorePanel)
        );
    }

    #[test]
    fn scale_clamped_on_resolve() {
        let section = SceneSection {
            widgets: vec![WidgetEntry {
                kind: WidgetKind::Combo,
                space: AnchorSpace::Screen,
                anchor: Anchor9::TopLeft,
                origin: Anchor9::TopLeft,
                offset: [0.0, 0.0],
                scale: 99.0,
                z: 0,
                visible_play: true,
                visible_practice: true,
            }],
        };
        assert_eq!(
            section.resolve()[&WidgetKind::Combo].scale,
            MAX_WIDGET_SCALE
        );
    }

    #[test]
    fn duplicate_kind_first_wins() {
        let mk = |offx: f32| WidgetEntry {
            kind: WidgetKind::Combo,
            space: AnchorSpace::Screen,
            anchor: Anchor9::TopLeft,
            origin: Anchor9::TopLeft,
            offset: [offx, 0.0],
            scale: 1.0,
            z: 0,
            visible_play: true,
            visible_practice: true,
        };
        let section = SceneSection {
            widgets: vec![mk(10.0), mk(99.0)],
        };
        assert_eq!(section.resolve()[&WidgetKind::Combo].offset, (10.0, 0.0));
    }

    #[test]
    fn from_map_only_writes_non_default_entries() {
        let mut map = SceneSection::default().resolve();
        map.get_mut(&WidgetKind::Combo).unwrap().offset = (5.0, 5.0);
        let section = SceneSection::from_map(&map);
        assert_eq!(section.widgets.len(), 1);
        assert_eq!(section.widgets[0].kind, WidgetKind::Combo);
    }

    #[test]
    fn scene_round_trips() {
        let mut map = SceneSection::default().resolve();
        map.get_mut(&WidgetKind::NowPlaying).unwrap().offset = (12.0, 34.0);
        map.get_mut(&WidgetKind::NowPlaying).unwrap().anchor = Anchor9::TopRight;
        let section = SceneSection::from_map(&map);
        let back = section.resolve();
        assert_eq!(back[&WidgetKind::NowPlaying].offset, (12.0, 34.0));
        assert_eq!(back[&WidgetKind::NowPlaying].anchor, Anchor9::TopRight);
    }
}
