//! `CActConfigList.Skin` — port of `Stage/03.Config/CActConfigList.Skin.cs` (161 LOC).
//!
//! Strict-port-first. Skin sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Skin.cs:1-161`

use super::config_list::ConfigListItem;

/// Number of items in the Skin sub-menu.
pub const SKIN_ITEMS_PORTED: usize = 4;

/// Build the Skin sub-menu items.
pub fn build_skin_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("Skin (Legacy)", "Default"), // nSkinIndex
        ConfigListItem::leaf("Skin (New)", "Default"),    // nNewSkinIndex
        ConfigListItem::leaf("Skin (Box)", "OFF"),        // bUseBoxDefSkin
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_items_count() {
        let items = build_skin_items();
        assert_eq!(items.len(), SKIN_ITEMS_PORTED);
    }

    #[test]
    fn skin_items_start_with_return() {
        let items = build_skin_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn skin_items_have_three_skin_selectors() {
        // CActConfigList.Skin.cs:11-50
        let items = build_skin_items();
        assert!(items.iter().any(|i| i.name == "Skin (Legacy)"));
        assert!(items.iter().any(|i| i.name == "Skin (New)"));
        assert!(items.iter().any(|i| i.name == "Skin (Box)"));
    }

    #[test]
    fn skin_legacy_default_is_default() {
        // CActConfigList.Skin.cs:11-30
        let items = build_skin_items();
        let s = items.iter().find(|i| i.name == "Skin (Legacy)").unwrap();
        assert_eq!(s.value, "Default");
    }

    #[test]
    fn skin_box_default_off() {
        // CActConfigList.Skin.cs:50-65
        let items = build_skin_items();
        let s = items.iter().find(|i| i.name == "Skin (Box)").unwrap();
        assert_eq!(s.value, "OFF");
    }
}
