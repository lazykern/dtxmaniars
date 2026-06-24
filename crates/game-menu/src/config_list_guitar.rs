//! `CActConfigList.Guitar` — port of `Stage/03.Config/CActConfigList.Guitar.cs` (439 LOC).
//!
//! Strict-port-first. Guitar sub-menu items (first 25 of 39).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Guitar.cs:1-439`

use super::config_list::ConfigListItem;

/// Number of items in the Guitar sub-menu.
pub const GUITAR_ITEMS_PORTED: usize = 25;

/// Build the Guitar sub-menu items.
pub fn build_guitar_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("Card Name", ""),
        ConfigListItem::leaf("Group Name", ""),
        ConfigListItem::leaf("AutoPlay (All)", "Off"),
        ConfigListItem::leaf("    R", "OFF"), // bAutoPlay.GtR
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
        ConfigListItem::leaf("Performance Mode", "Off"), // bSpecialist
        ConfigListItem::leaf("Random", "Off"),
        ConfigListItem::leaf("Left", "OFF"),
        ConfigListItem::leaf("JudgeLinePos", "50"), // nJudgeLine.Guitar
        ConfigListItem::leaf("ShutterInPos", "50"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guitar_items_count() {
        let items = build_guitar_items();
        assert_eq!(items.len(), GUITAR_ITEMS_PORTED);
    }

    #[test]
    fn guitar_items_start_with_return() {
        let items = build_guitar_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn guitar_items_have_5_color_lane_autoplay() {
        // CActConfigList.Guitar.cs:30-65 — R/G/B/Y/P
        let items = build_guitar_items();
        for c in &["R", "G", "B", "Y", "P"] {
            let l = format!("    {c}");
            assert!(items.iter().any(|i| i.name == l), "missing color: {c}");
        }
    }

    #[test]
    fn guitar_items_have_pick_and_wailing() {
        // CActConfigList.Guitar.cs:65-80
        let items = build_guitar_items();
        assert!(items.iter().any(|i| i.name == "    Pick"));
        assert!(items.iter().any(|i| i.name == "    Wailing"));
    }

    #[test]
    fn guitar_items_have_specialist() {
        // CActConfigList.Guitar.cs:200+ — Performance Mode
        let items = build_guitar_items();
        let s = items.iter().find(|i| i.name == "Performance Mode").unwrap();
        assert_eq!(s.value, "Off");
    }

    #[test]
    fn guitar_items_have_shutter_positions() {
        let items = build_guitar_items();
        let in_p = items.iter().find(|i| i.name == "ShutterInPos").unwrap();
        assert_eq!(in_p.value, "50");
    }
}
