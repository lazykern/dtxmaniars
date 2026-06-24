#![allow(missing_docs)]
//! `CActConfigList.Audio` — port of `Stage/03.Config/CActConfigList.Audio.cs` (189 LOC).
//!
//! Strict-port-first. Audio sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Audio.cs:80-189`

use super::config_list::ConfigListItem;

/// Number of items in the Audio sub-menu.
pub const AUDIO_ITEMS_PORTED: usize = 8;

/// Build the Audio sub-menu items.
pub fn build_audio_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::submenu("Audio Driver"),
        ConfigListItem::leaf("MasterVolume", "80"), // 0..100
        ConfigListItem::leaf("ChipVolume", "80"),   // 0..100, 手動再生音量
        ConfigListItem::leaf("AutoVolume", "80"),   // 0..100, 自動再生音量
        ConfigListItem::leaf("BGM Offset", "0"),    // -99..99 ms, nCommonBGMAdjustMs
        ConfigListItem::leaf("BGM Sound", "ON"),    // bBGM音を発声する
        ConfigListItem::leaf("Time Stretch", "OFF"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_items_count() {
        let items = build_audio_items();
        assert_eq!(items.len(), AUDIO_ITEMS_PORTED);
    }

    #[test]
    fn audio_items_start_with_return() {
        let items = build_audio_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn audio_items_have_audio_driver_submenu() {
        // CActConfigList.Audio.cs:118-127
        let items = build_audio_items();
        let driver = items.iter().find(|i| i.name == "Audio Driver").unwrap();
        assert!(driver.has_submenu);
    }

    #[test]
    fn audio_items_have_volume_items() {
        // CActConfigList.Audio.cs:99-117
        let items = build_audio_items();
        assert!(items.iter().any(|i| i.name == "MasterVolume"));
        assert!(items.iter().any(|i| i.name == "ChipVolume"));
        assert!(items.iter().any(|i| i.name == "AutoVolume"));
    }

    #[test]
    fn audio_items_have_bgm_settings() {
        // CActConfigList.Audio.cs:118-160
        let items = build_audio_items();
        assert!(items.iter().any(|i| i.name == "BGM Offset"));
        assert!(items.iter().any(|i| i.name == "BGM Sound"));
        assert!(items.iter().any(|i| i.name == "Time Stretch"));
    }

    #[test]
    fn master_volume_default_is_80() {
        let items = build_audio_items();
        let v = items.iter().find(|i| i.name == "MasterVolume").unwrap();
        assert_eq!(v.value, "80");
    }

    #[test]
    fn bgm_sound_default_on() {
        let items = build_audio_items();
        let s = items.iter().find(|i| i.name == "BGM Sound").unwrap();
        assert_eq!(s.value, "ON");
    }
}
