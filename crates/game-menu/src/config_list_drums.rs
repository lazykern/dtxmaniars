#![allow(missing_docs)]
//! `CActConfigList.Drums` — port of `Stage/03.Config/CActConfigList.Drums.cs` (879 LOC).
//!
//! Strict-port-first. Drums sub-menu items (first 30 of 66 ported).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Drums.cs:1-879`

use super::config_list::ConfigListItem;

/// Number of items in the Drums sub-menu.
pub const DRUMS_ITEMS_PORTED: usize = 30;

/// Build the Drums sub-menu items.
pub fn build_drums_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("Card Name", ""),
        ConfigListItem::leaf("Group Name", ""),
        ConfigListItem::leaf("AutoPlay (All)", "Off"), // 3-state
        ConfigListItem::leaf("    LeftCymbal", "OFF"),
        ConfigListItem::leaf("    HiHat", "OFF"),
        ConfigListItem::leaf("    LeftPedal", "OFF"),
        ConfigListItem::leaf("    LBassDrum", "OFF"),
        ConfigListItem::leaf("    Snare", "OFF"),
        ConfigListItem::leaf("    BassDrum", "OFF"),
        ConfigListItem::leaf("    HighTom", "OFF"),
        ConfigListItem::leaf("    LowTom", "OFF"),
        ConfigListItem::leaf("    FloorTom", "OFF"),
        ConfigListItem::leaf("    Cymbal", "OFF"),
        ConfigListItem::leaf("    Ride", "OFF"),
        ConfigListItem::leaf("HID-SUD", "OFF"),
        ConfigListItem::leaf("       Dark", "OFF"), // eDark enum
        ConfigListItem::leaf("LaneDisp", "ON"),
        ConfigListItem::leaf("JudgeLineDisp", "ON"),
        ConfigListItem::leaf("LaneFlush", "ON"),
        ConfigListItem::leaf("AttackEffect", "ON"),
        ConfigListItem::leaf("Reverse", "OFF"),
        ConfigListItem::leaf("JudgePosition", "Center"),
        ConfigListItem::leaf("Combo", "ON"),
        ConfigListItem::leaf("LaneType", "Type A"),
        ConfigListItem::leaf("RDPosition", "Normal"),
        ConfigListItem::leaf("Tight", "OFF"),
        ConfigListItem::leaf("FillIn", "ON"),
        ConfigListItem::leaf("FillInEffect", "ON"),
        ConfigListItem::submenu("Velocity"), // submenu
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drums_items_count() {
        let items = build_drums_items();
        assert_eq!(items.len(), DRUMS_ITEMS_PORTED);
    }

    #[test]
    fn drums_items_start_with_return() {
        let items = build_drums_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn drums_items_have_card_and_group_name() {
        // CActConfigList.Drums.cs:18-20 — CreateCardNameInputItem + CreateGroupNameInputItem
        let items = build_drums_items();
        assert!(items.iter().any(|i| i.name == "Card Name"));
        assert!(items.iter().any(|i| i.name == "Group Name"));
    }

    #[test]
    fn drums_items_have_all_11_lane_autoplay_toggles() {
        // CActConfigList.Drums.cs:30-150
        let items = build_drums_items();
        let lanes = [
            "LeftCymbal",
            "HiHat",
            "LeftPedal",
            "LBassDrum",
            "Snare",
            "BassDrum",
            "HighTom",
            "LowTom",
            "FloorTom",
            "Cymbal",
            "Ride",
        ];
        for lane in lanes {
            assert!(
                items.iter().any(|i| i.name.trim() == lane),
                "missing lane: {lane}"
            );
        }
    }

    #[test]
    fn drums_items_have_display_settings() {
        // CActConfigList.Drums.cs:155-200
        let items = build_drums_items();
        assert!(items.iter().any(|i| i.name.trim() == "Dark"));
        assert!(items.iter().any(|i| i.name == "LaneDisp"));
        assert!(items.iter().any(|i| i.name == "LaneFlush"));
        assert!(items.iter().any(|i| i.name == "AttackEffect"));
    }

    #[test]
    fn drums_items_have_lane_type() {
        // CActConfigList.Drums.cs:240-260 — LaneType
        let items = build_drums_items();
        let lt = items.iter().find(|i| i.name == "LaneType").unwrap();
        assert_eq!(lt.value, "Type A");
    }

    #[test]
    fn drums_velocity_submenu() {
        // CActConfigList.Drums.cs:300+ — submenu
        let items = build_drums_items();
        let v = items.iter().find(|i| i.name == "Velocity").unwrap();
        assert!(v.has_submenu);
    }

    #[test]
    fn drums_autoplay_all_default_off() {
        let items = build_drums_items();
        let a = items.iter().find(|i| i.name == "AutoPlay (All)").unwrap();
        assert_eq!(a.value, "Off");
    }
}
