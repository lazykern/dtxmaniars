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

/// Editable settings rows. Each maps to a `dtx_config` field adjusted with
/// ←/→ and persisted on exit. (Full CStageConfig has many more tabs; this is
/// the playable subset per the roadmap: scroll, offset, volume, damage.)
const SETTINGS: [&str; 5] = [
    "Scroll Speed",
    "Input Offset (ms)",
    "BGM Offset (ms)",
    "Master Volume",
    "Damage Level",
];

// === Bevy components/resources for screen entities ===

#[derive(Component)]
pub struct ConfigEntity;

#[derive(Component)]
struct ConfigItemEntity(usize);

/// Text node showing the current value of setting row `usize`.
#[derive(Component)]
struct ConfigValueText(usize);

/// In-memory editable copy of the persisted config. Loaded on enter, written
/// back to disk on exit.
#[derive(Resource, Default)]
struct ConfigDraft(dtx_config::Config);

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
        .init_resource::<ConfigDraft>()
        .add_systems(Startup, spawn_config_layout)
        .add_systems(
            OnEnter(AppState::Config),
            (load_config_draft, show_config_chrome, populate_default_tab, spawn_config).chain(),
        )
        .add_systems(
            OnExit(AppState::Config),
            (save_config_draft, hide_config_chrome, despawn_stage::<ConfigEntity>).chain(),
        )
        .add_systems(
            Update,
            (config_navigation, render_config_selection).run_if(in_state(AppState::Config)),
        );
}

/// Load the persisted config into the editable draft on entering the screen.
fn load_config_draft(mut draft: ResMut<ConfigDraft>) {
    draft.0 = dtx_config::load(&dtx_config::default_path());
}

/// Persist the edited draft to disk on leaving the screen.
fn save_config_draft(draft: Res<ConfigDraft>) {
    let path = dtx_config::default_path();
    if let Err(e) = dtx_config::save(&path, &draft.0) {
        error!("Config: failed to save {}: {e}", path.display());
    } else {
        info!("Config: saved to {}", path.display());
    }
}

/// Human-readable value string for setting row `index`.
fn setting_value(cfg: &dtx_config::Config, index: usize) -> String {
    match index {
        0 => format!("{:.1}x", cfg.gameplay.scroll_speed),
        1 => format!("{:+} ms", cfg.gameplay.input_offset_ms),
        2 => format!("{:+} ms", cfg.gameplay.bgm_adjust_ms),
        3 => format!("{}%", (cfg.audio.master_volume * 100.0).round() as i32),
        4 => cfg.gameplay.damage_level.label().to_string(),
        _ => String::new(),
    }
}

/// Adjust setting row `index` by one step. `dir` is +1 (→) or -1 (←).
fn adjust_setting(cfg: &mut dtx_config::Config, index: usize, dir: i32) {
    use dtx_config::{BGM_ADJUST_CLAMP_MS, INPUT_OFFSET_CLAMP_MS};
    match index {
        0 => {
            let next = cfg.gameplay.scroll_speed + 0.5 * dir as f32;
            cfg.gameplay.scroll_speed = next.clamp(0.5, 4.0);
        }
        1 => {
            let next = cfg.gameplay.input_offset_ms + 10 * dir;
            cfg.gameplay.input_offset_ms = next.clamp(-INPUT_OFFSET_CLAMP_MS, INPUT_OFFSET_CLAMP_MS);
        }
        2 => {
            let next = cfg.gameplay.bgm_adjust_ms + 10 * dir;
            cfg.gameplay.bgm_adjust_ms = next.clamp(-BGM_ADJUST_CLAMP_MS, BGM_ADJUST_CLAMP_MS);
        }
        3 => {
            let next = cfg.audio.master_volume + 0.05 * dir as f32;
            cfg.audio.master_volume = next.clamp(0.0, 1.0);
        }
        4 => {
            let levels = dtx_config::DamageLevel::all();
            let cur = levels
                .iter()
                .position(|l| *l == cfg.gameplay.damage_level)
                .unwrap_or(0) as i32;
            let len = levels.len() as i32;
            let next = (cur + dir).rem_euclid(len) as usize;
            cfg.gameplay.damage_level = levels[next];
        }
        _ => {}
    }
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

/// Per-state content: one editable row per setting, showing its current value.
/// OnExit despawns; persistent layout in `spawn_config_layout` survives.
fn spawn_config(mut commands: Commands, theme: Res<ThemeResource>, draft: Res<ConfigDraft>) {
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
                Text::new("↑↓: Select   ←→: Adjust   ESC: Save & Back"),
                Theme::font(14.0),
                TextColor(t.text_secondary),
            ));

            for (i, name) in SETTINGS.iter().enumerate() {
                parent
                    .spawn((
                        ConfigItemEntity(i),
                        Node {
                            width: Val::Px(520.0),
                            height: Val::Px(32.0),
                            margin: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(if i == 0 {
                            t.accent.with_alpha(0.35)
                        } else {
                            t.panel_bg
                        }),
                    ))
                    .with_children(|row| {
                        row.spawn((Text::new(*name), Theme::font(16.0), TextColor(t.text_primary)));
                        row.spawn((
                            ConfigValueText(i),
                            Text::new(setting_value(&draft.0, i)),
                            Theme::font(16.0),
                            TextColor(t.accent),
                        ));
                    });
            }
        });
}

fn config_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<ConfigSelection>,
    mut draft: ResMut<ConfigDraft>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    let max = SETTINGS.len().saturating_sub(1);
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = selection.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        adjust_setting(&mut draft.0, selection.0, 1);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        adjust_setting(&mut draft.0, selection.0, -1);
    } else if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
    }
}

fn render_config_selection(
    theme: Res<ThemeResource>,
    selection: Res<ConfigSelection>,
    draft: Res<ConfigDraft>,
    mut rows: Query<(&ConfigItemEntity, &mut BackgroundColor)>,
    mut values: Query<(&ConfigValueText, &mut Text)>,
) {
    let t = theme.0;
    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.0 == selection.0 {
            t.accent.with_alpha(0.35)
        } else {
            t.panel_bg
        };
    }
    for (value, mut text) in &mut values {
        *text = Text::new(setting_value(&draft.0, value.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // From old config.rs
    #[test]
    fn settings_not_empty() {
        assert!(
            !SETTINGS.is_empty(),
            "Config must have at least one editable setting"
        );
    }

    #[test]
    fn adjust_scroll_speed_clamps() {
        let mut cfg = dtx_config::Config::default();
        cfg.gameplay.scroll_speed = 4.0;
        adjust_setting(&mut cfg, 0, 1);
        assert!((cfg.gameplay.scroll_speed - 4.0).abs() < f32::EPSILON);
        cfg.gameplay.scroll_speed = 0.5;
        adjust_setting(&mut cfg, 0, -1);
        assert!((cfg.gameplay.scroll_speed - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn adjust_damage_level_cycles() {
        let mut cfg = dtx_config::Config::default();
        cfg.gameplay.damage_level = dtx_config::DamageLevel::None;
        adjust_setting(&mut cfg, 4, -1);
        assert_eq!(cfg.gameplay.damage_level, dtx_config::DamageLevel::High);
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
        let max = SETTINGS.len() - 1;
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
