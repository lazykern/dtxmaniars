#![allow(missing_docs)]
//! `CActConfigList.Menu` — port of `Stage/03.Config/CActConfigList.Menu.cs` (74 LOC).
//!
//! Strict-port-first. Menu sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Menu.cs:1-74`

use super::config_list::ConfigListItem;

/// Number of items in the Menu sub-menu.
pub const MENU_ITEMS_PORTED: usize = 6;

/// Build the Menu sub-menu items.
pub fn build_menu_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("MusicNameDispDEF", "OFF"), // b曲名表示をdefのものにする
        ConfigListItem::leaf("Difficulty Display", "XG"), // bDisplayDifficultyXGStyle
        ConfigListItem::leaf("RandSubBox", "OFF"),       // bランダムセレクトで子BOXを検索対象とする
        ConfigListItem::leaf("PreSoundWait", "1000"), // 0..10000 ms, nSongSelectSoundPreviewWaitTimeMs
        ConfigListItem::leaf("PreImageWait", "1000"), // 0..10000 ms, nSongSelectImagePreviewWaitTimeMs
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_items_count() {
        let items = build_menu_items();
        assert_eq!(items.len(), MENU_ITEMS_PORTED);
    }

    #[test]
    fn menu_items_start_with_return() {
        let items = build_menu_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn menu_items_have_music_name_disp() {
        // CActConfigList.Menu.cs:9-19
        let items = build_menu_items();
        let m = items.iter().find(|i| i.name == "MusicNameDispDEF").unwrap();
        assert_eq!(m.value, "OFF");
    }

    #[test]
    fn menu_items_have_difficulty_display() {
        // CActConfigList.Menu.cs:21-34
        let items = build_menu_items();
        let d = items
            .iter()
            .find(|i| i.name == "Difficulty Display")
            .unwrap();
        assert_eq!(d.value, "XG");
    }

    #[test]
    fn menu_items_have_random_subbox() {
        // CActConfigList.Menu.cs:36-44
        let items = build_menu_items();
        let r = items.iter().find(|i| i.name == "RandSubBox").unwrap();
        assert_eq!(r.value, "OFF");
    }

    #[test]
    fn menu_items_have_preview_waits() {
        // CActConfigList.Menu.cs:46-65
        let items = build_menu_items();
        let s = items.iter().find(|i| i.name == "PreSoundWait").unwrap();
        assert_eq!(s.value, "1000");
        let i = items.iter().find(|i| i.name == "PreImageWait").unwrap();
        assert_eq!(i.value, "1000");
    }
}
