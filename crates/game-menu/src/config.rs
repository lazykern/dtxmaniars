//! CStageConfig — settings screen.
//!
//! Merged from `config.rs` (M4 minimum viable list) + `config_full.rs`
//! (strict-port-first position constants + ConfigTab enum + layout).
//! Single plugin, no double-spawn.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CStageConfig.cs` (531 lines)
//! DTXManiaNX has many sub-menus (Drums / Guitar / Bass / System / Skin /
//! Audio / Graphics / Gameplay / Menu / Velocity). Each is a CActConfigList.

use bevy::prelude::*;
use dtx_ui::ThemeResource;
use dtx_ui::theme::Theme;
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};

// === Layout positions (verbatim from CStageConfig.cs:45-85) ===

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

// === Types (from CStageConfig.cs + CActConfigList.cs) ===

/// The 5 top-level Config tabs (CStageConfig.cs:80-84).
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

/// M4 stub: hardcoded list of top-level menu groups.
/// Real CStageConfig has 11+ groups (System, Skin, Gameplay, Drums, ...).
const MENU_GROUPS: &[(&str, &str)] = &[
    ("Drums", "M6+ — drum key bindings, auto-play, velocity"),
    ("System", "M6+ — fullscreen, audio buffer, log"),
    ("Skin", "M6+ — skin directory + reload"),
    ("Gameplay", "M6+ — timing windows, scroll speed"),
];

// === Bevy components/resources for screen entities ===

#[derive(Component)]
pub struct ConfigEntity;

#[derive(Component)]
struct ConfigItemEntity(usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigLeftMenu;

#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigDescriptionPanel;

#[derive(Resource, Debug, Default, Clone, Copy)]
struct ConfigSelection(usize);

// === Plugin ===

pub fn plugin(app: &mut App) {
    app.init_resource::<ConfigSelection>()
        .init_resource::<ActiveConfigTab>()
        .add_systems(Startup, spawn_config_layout)
        .add_systems(
            OnEnter(AppState::Config),
            (show_config_chrome, populate_default_tab, spawn_config).chain(),
        )
        .add_systems(
            OnExit(AppState::Config),
            (hide_config_chrome, despawn_stage::<ConfigEntity>).chain(),
        )
        .add_systems(
            Update,
            (config_navigation, render_config_selection).run_if(in_state(AppState::Config)),
        );
}

/// Persistent layout spawned once at app start (CStageConfig.cs:45-85).
/// Stays visible across OnEnter/OnExit so the screen has stable chrome.
fn spawn_config_layout(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
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
        BackgroundColor(t.bg_bottom),
        Visibility::Hidden,
    ));

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
        BackgroundColor(t.panel_bg),
        Visibility::Hidden,
    ));

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
        BackgroundColor(t.panel_bg),
        Text::new("(no selection)"),
        Theme::font(17.0),
        TextColor(t.text_secondary),
        Visibility::Hidden,
    ));
}

fn show_config_chrome(
    mut menus: Query<&mut Visibility, With<ConfigLeftMenu>>,
    mut panels: Query<&mut Visibility, With<ConfigDescriptionPanel>>,
) {
    for mut vis in &mut menus {
        *vis = Visibility::Inherited;
    }
    for mut vis in &mut panels {
        *vis = Visibility::Inherited;
    }
}

fn hide_config_chrome(
    mut menus: Query<&mut Visibility, With<ConfigLeftMenu>>,
    mut panels: Query<&mut Visibility, With<ConfigDescriptionPanel>>,
) {
    for mut vis in &mut menus {
        *vis = Visibility::Hidden;
    }
    for mut vis in &mut panels {
        *vis = Visibility::Hidden;
    }
}

fn populate_default_tab(mut active: ResMut<ActiveConfigTab>) {
    if active.0.is_none() {
        active.0 = Some(ConfigTab::System);
    }
}

/// Per-state content (M4 stub: hardcoded list of top-level groups).
/// OnExit despawns; persistent layout in `spawn_config_layout` survives.
fn spawn_config(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands
        .spawn((
            ConfigEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(40.0)),
                row_gap: Val::Px(15.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|parent| {
            parent.spawn((Text::new("Config"), Theme::font(36.0), TextColor(t.accent)));
            parent.spawn((
                Text::new("↑↓: Navigate  ENTER: Drill in (stub)  ESC: Back"),
                Theme::font(14.0),
                TextColor(t.text_secondary),
            ));

            for (i, (name, _desc)) in MENU_GROUPS.iter().enumerate() {
                parent
                    .spawn((
                        ConfigItemEntity(i),
                        Node {
                            width: Val::Px(400.0),
                            height: Val::Px(28.0),
                            margin: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(if i == 0 {
                            t.accent.with_alpha(0.35)
                        } else {
                            t.panel_bg
                        }),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(*name),
                            Theme::font(16.0),
                            TextColor(t.text_primary),
                        ));
                    });
            }
        });
}

fn config_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<ConfigSelection>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    let max = MENU_GROUPS.len().saturating_sub(1);
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = selection.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::Enter) {
        info!("Config: drill into '{}' (stub)", MENU_GROUPS[selection.0].0);
    } else if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
    }
}

fn render_config_selection(
    theme: Res<ThemeResource>,
    selection: Res<ConfigSelection>,
    mut rows: Query<(&ConfigItemEntity, &mut BackgroundColor)>,
) {
    let t = theme.0;
    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.0 == selection.0 {
            t.accent.with_alpha(0.35)
        } else {
            t.panel_bg
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // From old config.rs
    #[test]
    fn menu_groups_not_empty() {
        assert!(
            !MENU_GROUPS.is_empty(),
            "Config must have at least one menu group"
        );
    }

    #[test]
    fn selection_index_starts_at_zero() {
        assert_eq!(ConfigSelection::default().0, 0);
    }

    #[test]
    fn arrow_up_saturates_at_zero() {
        let mut sel = ConfigSelection(0);
        sel.0 = sel.0.saturating_sub(1);
        assert_eq!(sel.0, 0);
    }

    #[test]
    fn arrow_down_within_bounds() {
        let mut sel = ConfigSelection(0);
        let max = MENU_GROUPS.len() - 1;
        sel.0 = (sel.0 + 1).min(max);
        assert_eq!(sel.0, 1.min(max));
    }

    // From config_full.rs
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
        let labels: Vec<_> = ConfigTab::all().iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            vec!["System", "Drums", "Guitar P1", "Guitar P2", "Exit"]
        );
    }

    #[test]
    fn config_tabs_setup_methods_match_reference() {
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
