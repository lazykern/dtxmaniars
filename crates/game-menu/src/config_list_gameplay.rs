#![allow(missing_docs)]
//! `CActConfigList.Gameplay` — port of `Stage/03.Config/CActConfigList.Gameplay.cs` (185 LOC).
//!
//! Strict-port-first. Gameplay sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Gameplay.cs:1-185`

use super::config_list::ConfigListItem;

/// Number of items in the Gameplay sub-menu.
pub const GAMEPLAY_ITEMS_PORTED: usize = 12;

/// Build the Gameplay sub-menu items.
pub fn build_gameplay_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("Risky", "0"),            // 0..10
        ConfigListItem::leaf("DamageLevel", "Normal"), // Small/Normal/Large
        ConfigListItem::leaf("PlaySpeed", "1.00"),     // PLAYSPEED_MIN..MAX
        ConfigListItem::leaf("SkillMode", "CLASSIC"),  // CLASSIC/XG
        ConfigListItem::leaf("CLASSIC Notes", "OFF"),
        ConfigListItem::leaf("AutoAddGage", "OFF"),
        ConfigListItem::leaf("StageFailed", "ON"), // bSTAGEFAILEDEnabled
        ConfigListItem::leaf("ShowScore", "ON"),
        ConfigListItem::leaf("ShowMusicInfo", "ON"),
        ConfigListItem::leaf("StageEffect", "ON"),
        ConfigListItem::leaf("ShowLagTime", "OFF"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_items_count() {
        let items = build_gameplay_items();
        assert_eq!(items.len(), GAMEPLAY_ITEMS_PORTED);
    }

    #[test]
    fn gameplay_items_start_with_return() {
        let items = build_gameplay_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn gameplay_items_have_risky() {
        // CActConfigList.Gameplay.cs:9-21
        let items = build_gameplay_items();
        let r = items.iter().find(|i| i.name == "Risky").unwrap();
        assert_eq!(r.value, "0");
    }

    #[test]
    fn gameplay_items_have_skill_mode() {
        // CActConfigList.Gameplay.cs:42-58
        let items = build_gameplay_items();
        let s = items.iter().find(|i| i.name == "SkillMode").unwrap();
        assert_eq!(s.value, "CLASSIC");
    }

    #[test]
    fn gameplay_items_have_damage_level() {
        let items = build_gameplay_items();
        let d = items.iter().find(|i| i.name == "DamageLevel").unwrap();
        assert_eq!(d.value, "Normal");
    }

    #[test]
    fn gameplay_items_have_stage_settings() {
        // CActConfigList.Gameplay.cs:80-105
        let items = build_gameplay_items();
        assert!(items.iter().any(|i| i.name == "StageFailed"));
        assert!(items.iter().any(|i| i.name == "ShowScore"));
        assert!(items.iter().any(|i| i.name == "ShowMusicInfo"));
    }

    #[test]
    fn gameplay_items_have_playspeed_default_1_0() {
        let items = build_gameplay_items();
        let p = items.iter().find(|i| i.name == "PlaySpeed").unwrap();
        assert_eq!(p.value, "1.00");
    }
}
