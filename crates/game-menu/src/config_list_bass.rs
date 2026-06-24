#![allow(missing_docs)]
//! `CActConfigList.Bass` — port of `Stage/03.Config/CActConfigList.Bass.cs` (427 LOC).
//!
//! Strict-port-first. Bass sub-menu items (first 25 of 39).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Bass.cs:1-427`

use super::config_list::ConfigListItem;

/// Number of items in the Bass sub-menu.
pub const BASS_ITEMS_PORTED: usize = 25;

/// Build the Bass sub-menu items.
pub fn build_bass_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("Card Name", ""),
        ConfigListItem::leaf("Group Name", ""),
        ConfigListItem::leaf("AutoPlay (All)", "Off"),
        ConfigListItem::leaf("    R", "OFF"), // bAutoPlay.BsR
        ConfigListItem::leaf("    G", "OFF"),
        ConfigListItem::leaf("    B", "OFF"),
        ConfigListItem::leaf("    Y", "OFF"),
        ConfigListItem::leaf("    P", "OFF"),
        ConfigListItem::leaf("    Pick", "OFF"),
        ConfigListItem::leaf("    Wailing", "OFF"),
        ConfigListItem::leaf("HID-SUD", "OFF"),
        ConfigListItem::leaf("       Dark", "OFF"),
        ConfigListItem::leaf("LaneDisp", "ON"),
        ConfigListItem::leaf("JudgeLineDisp", "ON"),
        ConfigListItem::leaf("LaneFlush", "ON"),
        ConfigListItem::leaf("AttackEffect", "ON"),
        ConfigListItem::leaf("Reverse", "OFF"),
        ConfigListItem::leaf("Position", "Center"),
        ConfigListItem::leaf("Light", "OFF"),
        ConfigListItem::leaf("Performance Mode", "Off"),
        ConfigListItem::leaf("Random", "Off"),
        ConfigListItem::leaf("Left", "OFF"),
        ConfigListItem::leaf("JudgeLinePos", "50"),
        ConfigListItem::leaf("ShutterInPos", "50"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bass_items_count() {
        let items = build_bass_items();
        assert_eq!(items.len(), BASS_ITEMS_PORTED);
    }

    #[test]
    fn bass_items_start_with_return() {
        let items = build_bass_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn bass_items_have_5_color_lane_autoplay() {
        // CActConfigList.Bass.cs:30-65 — R/G/B/Y/P
        let items = build_bass_items();
        for c in &["R", "G", "B", "Y", "P"] {
            let l = format!("    {c}");
            assert!(items.iter().any(|i| i.name == l), "missing color: {c}");
        }
    }

    #[test]
    fn bass_items_have_pick_and_wailing() {
        let items = build_bass_items();
        assert!(items.iter().any(|i| i.name == "    Pick"));
        assert!(items.iter().any(|i| i.name == "    Wailing"));
    }

    #[test]
    fn bass_items_have_specialist() {
        let items = build_bass_items();
        let s = items.iter().find(|i| i.name == "Performance Mode").unwrap();
        assert_eq!(s.value, "Off");
    }

    #[test]
    fn bass_items_have_judge_pos_default_50() {
        let items = build_bass_items();
        let p = items.iter().find(|i| i.name == "JudgeLinePos").unwrap();
        assert_eq!(p.value, "50");
    }
}
