//! Full Config menu + ChangeSkin stage — port of `Stage/03.Config/` + `Stage/09.ChangeSkin/`.
//!
//! Strict-port-first (ADR-0010). Position constants verbatim from reference.
//!
//! ## Config tabs ported (CStageConfig.cs:91-99)
//!
//! | Tab | Reference | Items |
//! |-----|-----------|-------|
//! | System | CActConfigList.System.cs (396 LOC) | volume, fullscreen, language |
//! | Audio | CActConfigList.Audio.cs (189 LOC) | sound device, buffer size |
//! | Audio Driver | CActConfigList.Audio.Driver.cs (128 LOC) | ASIO/WASAPI/DirectSound |
//! | Graphics | CActConfigList.Graphics.cs (119 LOC) | resolution, vsync, fps |
//! | Gameplay | CActConfigList.Gameplay.cs (185 LOC) | scroll speed, dark mode |
//! | Menu | CActConfigList.Menu.cs (74 LOC) | menu cursor, font |
//! | Drums | CActConfigList.Drums.cs (879 LOC) | auto, velocity, lane type |
//! | Guitar | CActConfigList.Guitar.cs (439 LOC) | auto, color |
//! | Bass | CActConfigList.Bass.cs (427 LOC) | auto, color |
//! | Skin | CActConfigList.Skin.cs (161 LOC) | skin name, subfolder |
//! | Key Assign | CActConfigKeyAssign.cs (564 LOC) | key → pad remap |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/`
//! ChangeSkin reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/09.ChangeSkin/CStageChangeSkin.cs` (96 LOC)

use bevy::prelude::Component as _;
use bevy::prelude::*;
use game_shell::AppState;

/// Left menu position (CStageConfig.cs:45-46).
pub const CONFIG_LEFT_MENU_X: f32 = 245.0;
pub const CONFIG_LEFT_MENU_Y: f32 = 140.0;
/// List position offset (CStageConfig.cs:54).
pub const CONFIG_LIST_X_OFFSET: f32 = 95.0;
pub const CONFIG_LIST_Y_OFFSET: f32 = 4.0;
/// Menu cursor size (CStageConfig.cs:64-65).
pub const CONFIG_CURSOR_W: f32 = 170.0;
pub const CONFIG_CURSOR_H: f32 = 28.0;

/// ChangeSkin stage title font size (no source constant — derived from layout).
pub const CHANGE_SKIN_THUMB_SIZE: f32 = 96.0;

/// One tab in the Config left menu (CStageConfig.cs:91-99).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigTab {
    System,
    Audio,
    AudioDriver,
    Graphics,
    Gameplay,
    Menu,
    Drums,
    DrumsVelocity,
    Guitar,
    Bass,
    Skin,
    KeyAssign,
}

impl ConfigTab {
    /// All tabs in the same order as the reference's left menu (CStageConfig.cs:91-99).
    pub fn all() -> [Self; 12] {
        [
            Self::System,
            Self::Audio,
            Self::AudioDriver,
            Self::Graphics,
            Self::Gameplay,
            Self::Menu,
            Self::Drums,
            Self::DrumsVelocity,
            Self::Guitar,
            Self::Bass,
            Self::Skin,
            Self::KeyAssign,
        ]
    }

    /// Display label for the tab.
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Audio => "Audio",
            Self::AudioDriver => "Audio Driver",
            Self::Graphics => "Graphics",
            Self::Gameplay => "Gameplay",
            Self::Menu => "Menu",
            Self::Drums => "Drums",
            Self::DrumsVelocity => "Drums Velocity",
            Self::Guitar => "Guitar",
            Self::Bass => "Bass",
            Self::Skin => "Skin",
            Self::KeyAssign => "Key Assign",
        }
    }
}

/// One option item in a config tab (CItemBase analog).
#[derive(Debug, Clone)]
pub struct ConfigItem {
    /// Display name (e.g. "Master Volume").
    pub name: &'static str,
    /// Current value (rendered as text).
    pub value: String,
    /// Min/max for numeric items (None for enum/text).
    pub range: Option<(f32, f32)>,
}

impl ConfigItem {
    /// Volume-style 0..100.
    pub fn volume(name: &'static str, value: u32) -> Self {
        Self {
            name,
            value: format!("{}%", value),
            range: Some((0.0, 100.0)),
        }
    }

    /// Enum-style (cycle through options).
    pub fn cycle(name: &'static str, options: &[&'static str], index: usize) -> Self {
        let i = index.min(options.len() - 1);
        Self {
            name,
            value: options.get(i).copied().unwrap_or("?").to_string(),
            range: None,
        }
    }

    /// Boolean toggle.
    pub fn toggle(name: &'static str, on: bool) -> Self {
        Self {
            name,
            value: if on { "ON" } else { "OFF" }.into(),
            range: None,
        }
    }
}

/// Resource: which tab is currently active in the Config menu.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActiveConfigTab(pub Option<ConfigTab>);

/// Resource: per-tab option list (re-populated when the tab changes).
#[derive(Resource, Default, Debug, Clone)]
pub struct ConfigTabItems {
    pub current: Vec<ConfigItem>,
}

impl ConfigTabItems {
    /// Load default items for a given tab (M12 strict-port: placeholders; M12.1 reads dtx-config).
    pub fn load(tab: ConfigTab) -> Self {
        let items = match tab {
            ConfigTab::System => vec![
                ConfigItem::volume("Master Volume", 80),
                ConfigItem::toggle("Fullscreen", true),
                ConfigItem::cycle("Language", &["en", "ja", "zh"], 0),
            ],
            ConfigTab::Audio => vec![
                ConfigItem::cycle("Sound Device", &["Default", "WASAPI", "ASIO"], 0),
                ConfigItem::volume("BGM Volume", 70),
                ConfigItem::volume("Drum Volume", 80),
            ],
            ConfigTab::AudioDriver => vec![
                ConfigItem::cycle("Driver", &["DirectSound", "WASAPI", "ASIO"], 1),
                ConfigItem::cycle("Buffer Size", &["256", "512", "1024", "2048"], 1),
            ],
            ConfigTab::Graphics => vec![
                ConfigItem::cycle("Resolution", &["1280x720", "1920x1080"], 0),
                ConfigItem::toggle("VSync", true),
                ConfigItem::cycle("FPS", &["60", "120", "240", "Unlimited"], 0),
            ],
            ConfigTab::Gameplay => vec![
                ConfigItem::cycle("Scroll Speed (Drums)", &["1.0x", "1.5x", "2.0x", "3.0x"], 1),
                ConfigItem::toggle("Dark Mode", false),
                ConfigItem::toggle("Reverse (Drums)", false),
            ],
            ConfigTab::Menu => vec![
                ConfigItem::cycle("Menu Cursor", &["Default", "Mini"], 0),
                ConfigItem::cycle("Menu Font", &["texgyreadventor", "arial"], 0),
            ],
            ConfigTab::Drums => vec![
                ConfigItem::toggle("Auto Play (All)", false),
                ConfigItem::cycle("Lane Type", &["Type A", "Type B", "Type C", "Type D"], 0),
                ConfigItem::toggle("RD Position", false),
            ],
            ConfigTab::DrumsVelocity => vec![
                ConfigItem::volume("Velocity Min", 30),
                ConfigItem::volume("Velocity Max", 100),
            ],
            ConfigTab::Guitar => vec![
                ConfigItem::toggle("Auto Play (All)", false),
                ConfigItem::cycle("Pick Color", &["Red", "Blue", "Green"], 0),
            ],
            ConfigTab::Bass => vec![
                ConfigItem::toggle("Auto Play (All)", false),
                ConfigItem::cycle("Pick Color", &["Red", "Blue", "Green"], 0),
            ],
            ConfigTab::Skin => vec![ConfigItem::cycle("Skin", &["Default"], 0)],
            ConfigTab::KeyAssign => vec![ConfigItem::toggle("Mode: System", false)],
        };
        Self { current: items }
    }
}

/// Marker for the left menu UI entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigLeftMenu;

/// Marker for the right items list UI entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigItemsList;

/// ChangeSkin: one available skin entry (CStageChangeSkin.cs:75-83).
#[derive(Debug, Clone)]
pub struct SkinEntry {
    /// Skin subfolder name.
    pub name: String,
    /// Path to box.def thumbnail.
    pub thumbnail: std::path::PathBuf,
}

/// Resource: list of available skins for ChangeSkin stage.
#[derive(Resource, Default, Debug, Clone)]
pub struct AvailableSkins {
    pub skins: Vec<SkinEntry>,
    pub selected_idx: usize,
}

impl AvailableSkins {
    /// Scan a directory for skin subfolders (each containing box.def).
    /// M12 minimal: scan DTX_SONG_DIR parent + /graphics (typical DTXManiaNX layout).
    pub fn scan(base: &std::path::Path) -> Self {
        let mut skins = Vec::new();
        if let Ok(entries) = std::fs::read_dir(base) {
            for e in entries.flatten() {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                let box_def = p.join("box.def");
                if box_def.exists() {
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_string();
                    skins.push(SkinEntry {
                        name,
                        thumbnail: box_def,
                    });
                }
            }
        }
        Self {
            skins,
            selected_idx: 0,
        }
    }
}

/// Plugin assembly.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveConfigTab>()
        .init_resource::<ConfigTabItems>()
        .init_resource::<AvailableSkins>()
        .add_systems(Startup, spawn_config_left_menu)
        .add_systems(OnEnter(AppState::Config), populate_config_default)
        .add_systems(OnEnter(AppState::ChangeSkin), scan_skins_on_enter)
        .add_systems(
            Update,
            update_config_items_text.run_if(in_state(AppState::Config)),
        )
        .add_systems(
            Update,
            update_skin_thumbnails.run_if(in_state(AppState::ChangeSkin)),
        );
}

fn spawn_config_left_menu(mut commands: Commands) {
    // Left menu container at (245, 140) per CStageConfig.cs:45-46.
    commands.spawn((
        ConfigLeftMenu,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(CONFIG_LEFT_MENU_X),
            top: Val::Px(CONFIG_LEFT_MENU_Y),
            width: Val::Px(180.0),
            height: Val::Px(500.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.85)),
    ));

    // One row per tab (CStageConfig.cs:91-99 — 12 tabs).
    for (i, tab) in ConfigTab::all().iter().enumerate() {
        commands.spawn((
            ConfigLeftMenu,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(CONFIG_LEFT_MENU_X + CONFIG_LIST_X_OFFSET),
                top: Val::Px(CONFIG_LEFT_MENU_Y + CONFIG_LIST_Y_OFFSET + (i as f32) * 32.0),
                width: Val::Px(140.0),
                height: Val::Px(28.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            Text::new(tab.label()),
            TextFont {
                font_size: 18.0.into(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    }

    // Items list (right side) — placeholder; text filled by update_config_items_text.
    commands.spawn((
        ConfigItemsList,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(440.0),
            top: Val::Px(160.0),
            width: Val::Px(400.0),
            height: Val::Px(480.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Text::new("(no tab)"),
        TextFont {
            font_size: 18.0.into(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn populate_config_default(mut active: ResMut<ActiveConfigTab>, mut items: ResMut<ConfigTabItems>) {
    if active.0.is_none() {
        active.0 = Some(ConfigTab::System);
        *items = ConfigTabItems::load(ConfigTab::System);
    }
}

fn update_config_items_text(
    items: Res<ConfigTabItems>,
    mut q: Query<&mut Text, With<ConfigItemsList>>,
) {
    if !items.is_changed() {
        return;
    }
    let body = items
        .current
        .iter()
        .map(|i| format!("{}: {}", i.name, i.value))
        .collect::<Vec<_>>()
        .join("\n");
    for mut t in &mut q {
        *t = Text::new(body.clone());
    }
}

fn scan_skins_on_enter(mut skins: ResMut<AvailableSkins>) {
    if skins.skins.is_empty() {
        // Default scan path: DTX_SONG_DIR parent, falling back to CWD.
        let base = std::env::var("DTX_SONG_DIR")
            .map(std::path::PathBuf::from)
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        *skins = AvailableSkins::scan(&base);
    }
}

fn update_skin_thumbnails(
    skins: Res<AvailableSkins>,
    mut q: Query<&mut Text, With<ConfigItemsList>>,
) {
    if !skins.is_changed() {
        return;
    }
    if skins.skins.is_empty() {
        for mut t in &mut q {
            *t = Text::new("(no skins found in scan path)");
        }
        return;
    }
    let body = skins
        .skins
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == skins.selected_idx {
                format!("> {}", s.name)
            } else {
                format!("  {}", s.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    for mut t in &mut q {
        *t = Text::new(body.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_left_menu_position_matches_reference() {
        // CStageConfig.cs:45-46
        assert_eq!(CONFIG_LEFT_MENU_X, 245.0);
        assert_eq!(CONFIG_LEFT_MENU_Y, 140.0);
        assert_eq!(CONFIG_LIST_X_OFFSET, 95.0);
    }

    #[test]
    fn config_cursor_size_matches_reference() {
        // CStageConfig.cs:64-65
        assert_eq!(CONFIG_CURSOR_W, 170.0);
        assert_eq!(CONFIG_CURSOR_H, 28.0);
    }

    #[test]
    fn config_tabs_count_matches_reference() {
        // CStageConfig.cs:91-99 — 12 tabs
        assert_eq!(ConfigTab::all().len(), 12);
    }

    #[test]
    fn config_tabs_labels_unique() {
        let labels: Vec<_> = ConfigTab::all().iter().map(|t| t.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len());
    }

    #[test]
    fn config_items_load_system_has_three_items() {
        // CActConfigList.System.cs — at least 3 items in the System tab
        let items = ConfigTabItems::load(ConfigTab::System);
        assert!(items.current.len() >= 3);
    }

    #[test]
    fn config_items_load_for_every_tab() {
        for tab in ConfigTab::all() {
            let items = ConfigTabItems::load(tab);
            assert!(!items.current.is_empty(), "tab {:?} has no items", tab);
        }
    }

    #[test]
    fn config_volume_formats_percent() {
        let v = ConfigItem::volume("Vol", 50);
        assert_eq!(v.value, "50%");
        assert_eq!(v.range, Some((0.0, 100.0)));
    }

    #[test]
    fn config_toggle_formats_on_off() {
        assert_eq!(ConfigItem::toggle("X", true).value, "ON");
        assert_eq!(ConfigItem::toggle("X", false).value, "OFF");
    }

    #[test]
    fn config_cycle_clamps_index() {
        let opts = ["A", "B", "C"];
        let v = ConfigItem::cycle("X", &opts, 99);
        assert_eq!(v.value, "C");
    }

    #[test]
    fn available_skins_default_empty() {
        let s = AvailableSkins::default();
        assert!(s.skins.is_empty());
        assert_eq!(s.selected_idx, 0);
    }

    #[test]
    fn available_skins_scan_empty_dir() {
        let tmp = std::env::temp_dir().join("dtxmaniars_test_no_skins");
        let _ = std::fs::create_dir_all(&tmp);
        let s = AvailableSkins::scan(&tmp);
        assert!(s.skins.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn active_config_tab_default() {
        let a = ActiveConfigTab::default();
        assert!(a.0.is_none());
    }
}
