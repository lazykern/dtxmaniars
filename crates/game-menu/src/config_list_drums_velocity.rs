//! `CActConfigList.Drums.Velocity` — port of `Stage/03.Config/CActConfigList.Drums.Velocity.cs` (85 LOC).
//!
//! Strict-port-first. Per-drum velocity min threshold settings.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.Drums.Velocity.cs:1-85`

use super::config_list::ConfigListItem;

/// Number of items in the Drums.Velocity sub-menu.
pub const DRUMS_VELOCITY_ITEMS_PORTED: usize = 12;

/// Per-drum velocity defaults from reference (CActConfigList.Drums.Velocity.cs:8-18).
/// `nVelocityMin.HH = 20; others = 0;`
pub fn default_velocity_min(lane: &str) -> u32 {
    match lane {
        "Hi-hat" => 20,
        _ => 0,
    }
}

/// Build the Drums.Velocity sub-menu items.
pub fn build_drums_velocity_items() -> Vec<ConfigListItem> {
    vec![
        ConfigListItem::leaf("<< Return to Menu", ""),
        ConfigListItem::leaf(
            "Left cymbal",
            &default_velocity_min("Left cymbal").to_string(),
        ),
        ConfigListItem::leaf("Hi-hat", &default_velocity_min("Hi-hat").to_string()),
        ConfigListItem::leaf("Snare drum", "0"),
        ConfigListItem::leaf("Bass drum", "0"),
        ConfigListItem::leaf("High tom", "0"),
        ConfigListItem::leaf("Low tom", "0"),
        ConfigListItem::leaf("Floor tom", "0"),
        ConfigListItem::leaf("Cymbal", "0"),
        ConfigListItem::leaf("Ride cymbal", "0"),
        ConfigListItem::leaf("Left pedal", "0"),
        ConfigListItem::leaf("Left bass drum", "0"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drums_velocity_items_count() {
        let items = build_drums_velocity_items();
        assert_eq!(items.len(), DRUMS_VELOCITY_ITEMS_PORTED);
    }

    #[test]
    fn drums_velocity_items_start_with_return() {
        let items = build_drums_velocity_items();
        assert_eq!(items[0].name, "<< Return to Menu");
    }

    #[test]
    fn drums_velocity_items_have_all_11_drums() {
        // CActConfigList.Drums.Velocity.cs:32-78 — AddDrumVelocityItem x 11
        let items = build_drums_velocity_items();
        let names = [
            "Left cymbal",
            "Hi-hat",
            "Snare drum",
            "Bass drum",
            "High tom",
            "Low tom",
            "Floor tom",
            "Cymbal",
            "Ride cymbal",
            "Left pedal",
            "Left bass drum",
        ];
        for n in names {
            assert!(items.iter().any(|i| i.name == n), "missing: {n}");
        }
    }

    #[test]
    fn hi_hat_default_velocity_is_20() {
        // CActConfigList.Drums.Velocity.cs:9
        assert_eq!(default_velocity_min("Hi-hat"), 20);
    }

    #[test]
    fn other_drums_default_velocity_is_0() {
        // CActConfigList.Drums.Velocity.cs:8-18
        assert_eq!(default_velocity_min("Snare drum"), 0);
        assert_eq!(default_velocity_min("Bass drum"), 0);
        assert_eq!(default_velocity_min("Left cymbal"), 0);
    }

    #[test]
    fn hi_hat_in_items_has_value_20() {
        let items = build_drums_velocity_items();
        let hh = items.iter().find(|i| i.name == "Hi-hat").unwrap();
        assert_eq!(hh.value, "20");
    }
}
