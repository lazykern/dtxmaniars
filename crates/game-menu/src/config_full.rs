//! Full Config menu — port of `Stage/03.Config/CStageConfig.cs`.
//!
//! Strict-port-first (ADR-0010). Position constants verbatim from reference.
//!
//! ## Layout (CStageConfig.cs:45-85)
//!
//! - Left menu container at `(245, 140)`, renderOrder 30, panel `4_menu panel.png`
//! - 5 left-menu buttons (CStageConfig.cs:80-84): System, Drums, Guitar P1, Guitar P2, Exit
//! - Menu cursor: position `(-5, 2)`, size `(170, 28)`, sliceRect `(16, 0, 32, 28)`
//! - Description panel at `(800, 270)`, renderOrder 50 (CStageConfig.cs:113-117)
//! - Item bar at `(400, 0)`, renderOrder 20 (CStageConfig.cs:134)
//! - Header panel at `(0, 0)`, renderOrder 52 (CStageConfig.cs:139)
//! - Footer panel at `(0, 720-h)`, renderOrder 53 (CStageConfig.cs:144)
//!
//! ## Sub-acts (CStageConfig.cs:31-34)
//!
//! - `CActDFPFont` — DFP-rendered font (replaced by Bevy Text in v1)
//! - `CActConfigList` — the right-side item list (14 sub-acts from `CActConfigList.*.cs`)
//! - `CActConfigKeyAssign` — key→pad remap UI
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CStageConfig.cs` (531 LOC)

use bevy::prelude::*;
use game_shell::AppState;

/// Left menu position (CStageConfig.cs:48).
pub const CONFIG_LEFT_MENU_X: f32 = 245.0;
pub const CONFIG_LEFT_MENU_Y: f32 = 140.0;
/// List position offset (CStageConfig.cs:64).
pub const CONFIG_LIST_X_OFFSET: f32 = 95.0;
pub const CONFIG_LIST_Y_OFFSET: f32 = 4.0;
/// Menu cursor size (CStageConfig.cs:72-73).
pub const CONFIG_CURSOR_W: f32 = 170.0;
pub const CONFIG_CURSOR_H: f32 = 28.0;
/// Description panel (CStageConfig.cs:115-116).
pub const CONFIG_DESC_X: f32 = 800.0;
pub const CONFIG_DESC_Y: f32 = 270.0;
/// Item bar (CStageConfig.cs:134).
pub const CONFIG_ITEM_BAR_X: f32 = 400.0;
/// Header panel (CStageConfig.cs:139).
pub const CONFIG_HEADER_X: f32 = 0.0;
/// Footer panel (CStageConfig.cs:144).
pub const CONFIG_FOOTER_Y: f32 = 720.0;

/// The 5 top-level Config tabs (CStageConfig.cs:80-84).
///
/// Note: BocuD shows 5 buttons (System, Drums, Guitar P1, Guitar P2, Exit).
/// Drilling into each button opens the CActConfigList sub-tabs (System has
/// `System/Audio/...`, Drums has `Drums/Drums.Velocity`, etc.). M12 p1-1
/// only ships the 5 top-level buttons; the 14 sub-acts port in p1-2..p1-14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigTab {
    System,
    Drums,
    GuitarP1,
    GuitarP2,
    Exit,
}

impl ConfigTab {
    /// All 5 tabs in reference order (CStageConfig.cs:80-84).
    pub fn all() -> [Self; 5] {
        [
            Self::System,
            Self::Drums,
            Self::GuitarP1,
            Self::GuitarP2,
            Self::Exit,
        ]
    }

    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Drums => "Drums",
            Self::GuitarP1 => "Guitar P1",
            Self::GuitarP2 => "Guitar P2",
            Self::Exit => "Exit",
        }
    }

    /// C# sub-action dispatch (CStageConfig.cs:80-84).
    /// Maps to one of `CActConfigList.tSetupItemList_*` methods.
    pub fn setup_method(&self) -> &'static str {
        match self {
            Self::System => "tSetupItemList_System",
            Self::Drums => "tSetupItemList_Drums",
            Self::GuitarP1 => "tSetupItemList_Guitar",
            Self::GuitarP2 => "tSetupItemList_Bass",
            Self::Exit => "tSetupItemList_Exit",
        }
    }
}

/// Resource: which top-level tab is currently active.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActiveConfigTab(pub Option<ConfigTab>);

/// Marker for the left menu UI entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigLeftMenu;

/// Marker for the description panel entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigDescriptionPanel;

/// Plugin assembly.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveConfigTab>()
        .add_systems(Startup, spawn_config_layout)
        .add_systems(OnEnter(AppState::Config), populate_default_tab);
}

fn spawn_config_layout(mut commands: Commands) {
    // Background (CStageConfig.cs:127).
    commands.spawn((
        ConfigLeftMenu,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(1280.0),
            height: Val::Px(720.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
    ));

    // Item bar (CStageConfig.cs:133-135).
    commands.spawn((
        ConfigLeftMenu,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(CONFIG_ITEM_BAR_X),
            top: Val::Px(0.0),
            width: Val::Px(480.0),
            height: Val::Px(720.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.7)),
    ));

    // Left menu panel at (245, 140) per CStageConfig.cs:47-48.
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

    // 5 tab buttons (CStageConfig.cs:80-84) at list offset (95, 4) from menu.
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
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    }

    // Description panel (CStageConfig.cs:113-117).
    commands.spawn((
        ConfigDescriptionPanel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(CONFIG_DESC_X),
            top: Val::Px(CONFIG_DESC_Y),
            width: Val::Px(440.0),
            height: Val::Px(200.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Text::new("(no selection)"),
        TextFont {
            font_size: FontSize::Px(17.0),
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
    ));
}

fn populate_default_tab(mut active: ResMut<ActiveConfigTab>) {
    if active.0.is_none() {
        active.0 = Some(ConfigTab::System);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_left_menu_position_matches_reference() {
        // CStageConfig.cs:48
        assert_eq!(CONFIG_LEFT_MENU_X, 245.0);
        assert_eq!(CONFIG_LEFT_MENU_Y, 140.0);
    }

    #[test]
    fn config_cursor_size_matches_reference() {
        // CStageConfig.cs:72-73
        assert_eq!(CONFIG_CURSOR_W, 170.0);
        assert_eq!(CONFIG_CURSOR_H, 28.0);
    }

    #[test]
    fn config_description_position_matches_reference() {
        // CStageConfig.cs:115-116
        assert_eq!(CONFIG_DESC_X, 800.0);
        assert_eq!(CONFIG_DESC_Y, 270.0);
    }

    #[test]
    fn config_item_bar_matches_reference() {
        // CStageConfig.cs:134
        assert_eq!(CONFIG_ITEM_BAR_X, 400.0);
    }

    #[test]
    fn config_tabs_count_matches_reference() {
        // CStageConfig.cs:80-84 — 5 top-level buttons
        assert_eq!(ConfigTab::all().len(), 5);
    }

    #[test]
    fn config_tabs_labels_match_reference() {
        // CStageConfig.cs:80-84 verbatim
        let labels: Vec<_> = ConfigTab::all().iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            vec!["System", "Drums", "Guitar P1", "Guitar P2", "Exit"]
        );
    }

    #[test]
    fn config_tabs_setup_methods_match_reference() {
        // CStageConfig.cs:80-84 — the lambda bodies
        let methods: Vec<_> = ConfigTab::all().iter().map(|t| t.setup_method()).collect();
        assert_eq!(
            methods,
            vec![
                "tSetupItemList_System",
                "tSetupItemList_Drums",
                "tSetupItemList_Guitar",
                "tSetupItemList_Bass",
                "tSetupItemList_Exit",
            ]
        );
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
    fn active_config_tab_default_is_none() {
        let a = ActiveConfigTab::default();
        assert!(a.0.is_none());
    }
}
