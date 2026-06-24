//! `CActConfigList.Graphics` — port of `Stage/03.Config/CActConfigList.Graphics.cs` (119 LOC).
//!
//! Strict-port-first. Graphics sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Graphics.cs:1-119`

use super::config_list::ConfigListItem;

/// Number of items in the Graphics sub-menu.
pub const GRAPHICS_ITEMS_PORTED: usize = 9;

/// Build the Graphics sub-menu items.
pub fn build_graphics_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("AVI (ffmpeg)", "ON"),
        ConfigListItem::leaf("Movie Mode", "Both"),
        ConfigListItem::leaf("BGA", "ON"),
        ConfigListItem::leaf("BG Alpha", "255"),
        ConfigListItem::leaf("LaneAlpha", "0%"),
        ConfigListItem::leaf("Fullscreen", "ON"),
        ConfigListItem::leaf("Exclusive Fullscreen", "ON"),
        ConfigListItem::leaf("Vertical Sync", "ON"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_items_count() {
        let items = build_graphics_items();
        assert_eq!(items.len(), GRAPHICS_ITEMS_PORTED);
    }

    #[test]
    fn graphics_items_start_with_return() {
        let items = build_graphics_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn graphics_items_have_video_settings() {
        // CActConfigList.Graphics.cs:13-37
        let items = build_graphics_items();
        assert!(items.iter().any(|i| i.name == "AVI (ffmpeg)"));
        assert!(items.iter().any(|i| i.name == "Movie Mode"));
        assert!(items.iter().any(|i| i.name == "BGA"));
    }

    #[test]
    fn graphics_items_have_transparency_settings() {
        // CActConfigList.Graphics.cs:50-66
        let items = build_graphics_items();
        assert!(items.iter().any(|i| i.name == "BG Alpha"));
        assert!(items.iter().any(|i| i.name == "LaneAlpha"));
    }

    #[test]
    fn graphics_items_have_screen_mode() {
        // CActConfigList.Graphics.cs:67-101
        let items = build_graphics_items();
        assert!(items.iter().any(|i| i.name == "Fullscreen"));
        assert!(items.iter().any(|i| i.name == "Exclusive Fullscreen"));
        assert!(items.iter().any(|i| i.name == "Vertical Sync"));
    }

    #[test]
    fn bg_alpha_default_255() {
        let items = build_graphics_items();
        let v = items.iter().find(|i| i.name == "BG Alpha").unwrap();
        assert_eq!(v.value, "255");
    }
}
