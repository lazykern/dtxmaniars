#![allow(missing_docs)]
//! `CActConfigList.System` — port of `Stage/03.Config/CActConfigList.System.cs` (396 LOC).
//!
//! Strict-port-first. Returns the System sub-menu items per the reference.
//!
//! ## Items (CActConfigList.System.cs:9-396)
//!
//! 1. Return to menu
//! 2. Graphics Options (folder → SystemGraphics)
//! 3. Skin Options (folder → SystemSkin)
//! 4. Audio Options (folder → SystemAudio)
//! 5. Gameplay Options (folder → SystemGameplay)
//! 6. Menu Options (folder → SystemMenu)
//! 7. Reload Songs (action → SongDb.StartScan)
//! 8. Reload Songs Full (action → SongDb.StartScanFull)
//! 9. Game Selection (list: 4 options)
//! 10. BufferedInput (toggle)
//! 11. Debug Info (toggle)
//! 12. Chip Timing Mode (list: 2 options)
//!
//! (Items 13-39 deferred — System Key Mapping, Import/Export Config, etc.)

use super::config_list::ConfigListItem;

/// Build the System sub-menu items (CActConfigList.System.cs:9-396).
pub fn build_system_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::submenu("Graphics Options"),
        ConfigListItem::submenu("Skin Options"),
        ConfigListItem::submenu("Audio Options"),
        ConfigListItem::submenu("Gameplay Options"),
        ConfigListItem::submenu("Menu Options"),
        ConfigListItem::leaf("Reload Songs", ""),
        ConfigListItem::leaf("Reload Songs (Full)", ""),
        ConfigListItem::leaf("Game Selection", "Both"), // 0..3: Drums / Guitar / Bass / Both
        ConfigListItem::leaf("BufferedInput", "OFF"),
        ConfigListItem::leaf("Debug Info", "OFF"),
        ConfigListItem::leaf("Chip Timing Mode", "Optimized"), // 0..1: Accurate / Optimized
    ]
}

/// Number of items in the System sub-menu (post-port).
/// Reference has 39; we port 12 (Return + 5 folders + 2 actions + 5 settings).
/// Items 13-39 are deferred to p1-3.1+.
pub const SYSTEM_ITEMS_PORTED: usize = 12;

// Re-export EMenuType so callers can use it.
pub use super::config_list::EMenuType as _EMenuType;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_items_count() {
        // First 12 items are ported; reference has 39.
        let items = build_system_items();
        assert_eq!(items.len(), SYSTEM_ITEMS_PORTED);
    }

    #[test]
    fn system_items_start_with_return() {
        // CActConfigList.cs:tAddReturnToMenuItem (always first)
        let items = build_system_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn system_items_have_5_folder_submenus() {
        // CActConfigList.System.cs:20-79 — Graphics/Skin/Audio/Gameplay/Menu
        let items = build_system_items();
        let folders: Vec<_> = items.iter().filter(|i| i.has_submenu).collect();
        assert_eq!(folders.len(), 5);
        let names: Vec<&str> = folders.iter().map(|i| i.name).collect();
        assert!(names.contains(&"Graphics Options"));
        assert!(names.contains(&"Skin Options"));
        assert!(names.contains(&"Audio Options"));
        assert!(names.contains(&"Gameplay Options"));
        assert!(names.contains(&"Menu Options"));
    }

    #[test]
    fn system_items_have_reload_songs() {
        // CActConfigList.System.cs:88-103
        let items = build_system_items();
        assert!(items.iter().any(|i| i.name == "Reload Songs"));
        assert!(items.iter().any(|i| i.name == "Reload Songs (Full)"));
    }

    #[test]
    fn system_items_have_setting_leaves() {
        // CActConfigList.System.cs:110-145 — Game Selection, BufferedInput, Debug Info
        let items = build_system_items();
        let leaves: Vec<&str> = items
            .iter()
            .filter(|i| {
                !i.has_submenu && i.name != "Return to Menu" && !i.name.starts_with("Reload")
            })
            .map(|i| i.name)
            .collect();
        assert!(leaves.contains(&"Game Selection"));
        assert!(leaves.contains(&"BufferedInput"));
        assert!(leaves.contains(&"Debug Info"));
        assert!(leaves.contains(&"Chip Timing Mode"));
    }

    #[test]
    fn system_items_dispatch_to_submenus() {
        // Folder items should map to EMenuType sub-acts (verified by name).
        let items = build_system_items();
        let graphics = items.iter().find(|i| i.name == "Graphics Options").unwrap();
        assert!(graphics.has_submenu);
        // The 5 folders in C#: Graphics, Skin, Audio, Gameplay, Menu
    }
}
