//! `CActConfigList.Audio.Driver` — port of `Stage/03.Config/CActConfigList.Audio.Driver.cs` (128 LOC).
//!
//! Strict-port-first. Audio Driver sub-menu items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Audio.Driver.cs:90-128`

use super::config_list::ConfigListItem;

/// Number of items in the Audio Driver sub-menu.
pub const AUDIO_DRIVER_ITEMS_PORTED: usize = 7;

/// Build the Audio Driver sub-menu items.
pub fn build_audio_driver_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("Return to Menu", ""),
        ConfigListItem::leaf("AdjustWaves", "ON"),
        ConfigListItem::leaf("ASIO device", "(none)"),
        ConfigListItem::leaf("WASAPIBufSize", "0"), // 0..99999 ms
        ConfigListItem::leaf("WASAPIEventDriven", "OFF"),
        ConfigListItem::leaf("UseOSTimer", "OFF"), // bUseOSTimer
        ConfigListItem::leaf("Audio Driver", "WASAPI"), // 0..3 DirectSound/ASIO/ExclWASAPI/SharedWASAPI
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_driver_items_count() {
        let items = build_audio_driver_items();
        assert_eq!(items.len(), AUDIO_DRIVER_ITEMS_PORTED);
    }

    #[test]
    fn audio_driver_items_start_with_return() {
        let items = build_audio_driver_items();
        assert_eq!(items[0].name, "Return to Menu");
    }

    #[test]
    fn audio_driver_items_have_wasapi_settings() {
        // CActConfigList.Audio.Driver.cs:100-115
        let items = build_audio_driver_items();
        assert!(items.iter().any(|i| i.name == "WASAPIBufSize"));
        assert!(items.iter().any(|i| i.name == "WASAPIEventDriven"));
        assert!(items.iter().any(|i| i.name == "UseOSTimer"));
    }

    #[test]
    fn audio_driver_items_have_asio_device() {
        // CActConfigList.Audio.Driver.cs:67-83
        let items = build_audio_driver_items();
        let asio = items.iter().find(|i| i.name == "ASIO device").unwrap();
        assert!(asio.value.starts_with("(") || !asio.value.is_empty());
    }

    #[test]
    fn audio_driver_items_have_audio_driver_select() {
        // CActConfigList.Audio.Driver.cs:123
        let items = build_audio_driver_items();
        let drv = items.iter().find(|i| i.name == "Audio Driver").unwrap();
        assert_eq!(drv.value, "WASAPI");
    }

    #[test]
    fn audio_driver_items_have_adjust_waves() {
        // CActConfigList.Audio.Driver.cs:21-32
        let items = build_audio_driver_items();
        let aw = items.iter().find(|i| i.name == "AdjustWaves").unwrap();
        assert_eq!(aw.value, "ON");
    }
}
