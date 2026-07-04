//! CStageConfig — settings screen.
//!
//! Per-tab item lists. Each tab owns a `&'static [ConfigItem]` and navigation
//! (↑/↓/←/→) operates only on the active tab's items.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CStageConfig.cs` (531 lines).
//! Each BocuD tab is a separate `CActConfigList` subclass; we render one row per item.
//!
//! Tabs (5) — System, Gameplay, Audio, Drums, Exit — cover the four sections
//! of [`dtx_config::Config`] plus a save-exit affordance.
//!
//! UI redesign per ADR-0014: contents are osu-style flat lists, not BocuD
//! pixel-positioned columns; position constants below match reference for
//! anchored chrome only (top menu bar + description panel).

use bevy::prelude::*;
use dtx_ui::ThemeResource;
use dtx_ui::theme::Theme;
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};
use std::sync::LazyLock;

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

// === Types ===

/// The 5 top-level Config tabs. Order = display order = `all()` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigTab {
    System,
    Gameplay,
    Audio,
    Drums,
    Exit,
}

impl ConfigTab {
    pub fn all() -> [Self; 5] {
        [Self::System, Self::Gameplay, Self::Audio, Self::Drums, Self::Exit]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Gameplay => "Gameplay",
            Self::Audio => "Audio",
            Self::Drums => "Drums",
            Self::Exit => "Exit",
        }
    }

    /// C# sub-action dispatch name (kept for log / DI parity with BocuD).
    pub fn setup_method(self) -> &'static str {
        match self {
            Self::System => "tSetupItemList_System",
            Self::Gameplay => "tSetupItemList_Gameplay",
            Self::Audio => "tSetupItemList_Audio",
            Self::Drums => "tSetupItemList_Drums",
            Self::Exit => "tSetupItemList_Exit",
        }
    }

    /// Editable items on this tab. Pointer rows render as text labels.
    /// Empty for `Exit` (it has its own handler, see [`config_navigation`]).
    /// ponytail: `LazyLock` because `format!()` value formatters aren't const-eval.
    pub fn items(self) -> &'static [ConfigItem] {
        match self {
            Self::System => &SYSTEM_ITEMS,
            Self::Gameplay => &GAMEPLAY_ITEMS,
            Self::Audio => &AUDIO_ITEMS,
            Self::Drums => &DRUMS_ITEMS,
            Self::Exit => &[],
        }
    }
}

/// Resource: which top-level tab is currently active.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ActiveConfigTab(pub Option<ConfigTab>);

/// One editable setting row. `label` is the row text; `value` reads current
/// value as a display string; `adjust` mutates `Config` with `dir = ±1` (←/→).
///
/// Ponytail: function pointers over trait objects — single concrete type,
/// no vtable, no boxing. Closure capture would work but `fn` keeps the
/// call site obvious and the data table declarative.
#[derive(Clone, Copy)]
pub struct ConfigItem {
    pub label: &'static str,
    pub value: fn(&dtx_config::Config) -> String,
    pub adjust: fn(&mut dtx_config::Config, i32),
}

// --- System tab ---
// ponytail: closures inside vec! need `LazyLock` (format! isn't const-eval-able).

static SYSTEM_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| vec![
    ConfigItem {
        label: "VSync",
        value: |c| bool_label(c.system.vsync).to_string(),
        adjust: |c, _| c.system.vsync ^= true,
    },
    ConfigItem {
        label: "BGA Alpha",
        value: |c| c.system.bg_alpha.to_string(),
        adjust: |c, d| {
            let v = (c.system.bg_alpha as i32 + d * 8).clamp(0, 255);
            c.system.bg_alpha = v as u8;
        },
    },
    ConfigItem {
        label: "Movie Alpha",
        value: |c| c.system.movie_alpha.to_string(),
        adjust: |c, d| {
            let v = (c.system.movie_alpha as i32 + d * 8).clamp(0, 255);
            c.system.movie_alpha = v as u8;
        },
    },
    ConfigItem {
        label: "BGA Enabled",
        value: |c| bool_label(c.system.bga_enabled).to_string(),
        adjust: |c, _| c.system.bga_enabled ^= true,
    },
    ConfigItem {
        label: "Movie Enabled",
        value: |c| bool_label(c.system.movie_enabled).to_string(),
        adjust: |c, _| c.system.movie_enabled ^= true,
    },
    ConfigItem {
        label: "Log Output",
        value: |c| bool_label(c.system.log_enabled).to_string(),
        adjust: |c, _| c.system.log_enabled ^= true,
    },
    ConfigItem {
        label: "Perf Info",
        value: |c| bool_label(c.system.show_perf_info).to_string(),
        adjust: |c, _| c.system.show_perf_info ^= true,
    },
    ConfigItem {
        label: "Metronome",
        value: |c| bool_label(c.system.metronome).to_string(),
        adjust: |c, _| c.system.metronome ^= true,
    },
]);

// --- Gameplay tab ---

static GAMEPLAY_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| vec![
    ConfigItem {
        label: "Tight Mode",
        value: |c| bool_label(c.gameplay.tight).to_string(),
        adjust: |c, _| c.gameplay.tight ^= true,
    },
    ConfigItem {
        label: "Reverse",
        value: |c| bool_label(c.gameplay.reverse).to_string(),
        adjust: |c, _| c.gameplay.reverse ^= true,
    },
    ConfigItem {
        label: "Scroll Speed",
        value: |c| format!("{:.1}x", c.gameplay.scroll_speed),
        adjust: |c, d| {
            c.gameplay.scroll_speed = (c.gameplay.scroll_speed + 0.5 * d as f32).clamp(0.5, 4.0);
        },
    },
    ConfigItem {
        label: "Dark Mode",
        value: |c| bool_label(c.gameplay.dark_mode).to_string(),
        adjust: |c, _| c.gameplay.dark_mode ^= true,
    },
    ConfigItem {
        label: "Fill-In",
        value: |c| bool_label(c.gameplay.fillin_enabled).to_string(),
        adjust: |c, _| c.gameplay.fillin_enabled ^= true,
    },
    ConfigItem {
        label: "Stage Failed",
        value: |c| bool_label(c.gameplay.stage_failed_enabled).to_string(),
        adjust: |c, _| c.gameplay.stage_failed_enabled ^= true,
    },
    ConfigItem {
        label: "Input Offset",
        value: |c| format!("{:+} ms", c.gameplay.input_offset_ms),
        adjust: |c, d| {
            c.gameplay.input_offset_ms = (c.gameplay.input_offset_ms + 10 * d)
                .clamp(-dtx_config::INPUT_OFFSET_CLAMP_MS, dtx_config::INPUT_OFFSET_CLAMP_MS);
        },
    },
    ConfigItem {
        label: "BGM Offset",
        value: |c| format!("{:+} ms", c.gameplay.bgm_adjust_ms),
        adjust: |c, d| {
            c.gameplay.bgm_adjust_ms = (c.gameplay.bgm_adjust_ms + 10 * d)
                .clamp(-dtx_config::BGM_ADJUST_CLAMP_MS, dtx_config::BGM_ADJUST_CLAMP_MS);
        },
    },
    ConfigItem {
        label: "Play Speed",
        value: |c| format!("{:.2}x", dtx_config::play_speed_multiplier(c.gameplay.play_speed)),
        adjust: |c, d| {
            let raw = (c.gameplay.play_speed as i32 + d)
                .clamp(dtx_config::PLAY_SPEED_MIN as i32, dtx_config::PLAY_SPEED_MAX as i32);
            c.gameplay.play_speed = raw as u8;
        },
    },
    ConfigItem {
        label: "Damage Level",
        value: |c| c.gameplay.damage_level.label().to_string(),
        adjust: |c, d| {
            let levels = dtx_config::DamageLevel::all();
            let cur = levels.iter().position(|l| *l == c.gameplay.damage_level).unwrap_or(0) as i32;
            let next = (cur + d).rem_euclid(levels.len() as i32) as usize;
            c.gameplay.damage_level = levels[next];
        },
    },
    ConfigItem {
        label: "Lane Display",
        value: |c| lane_display_label(c.gameplay.lane_display).to_string(),
        adjust: |c, d| {
            let opts = dtx_config::LaneDisplay::all();
            let cur = opts.iter().position(|l| *l == c.gameplay.lane_display).unwrap_or(0) as i32;
            let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
            c.gameplay.lane_display = opts[next];
        },
    },
]);

// --- Audio tab ---

static AUDIO_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| vec![
    ConfigItem {
        label: "BGM Enabled",
        value: |c| bool_label(c.audio.bgm_enabled).to_string(),
        adjust: |c, _| c.audio.bgm_enabled ^= true,
    },
    ConfigItem {
        label: "Drum Hit Sound",
        value: |c| bool_label(c.audio.drum_sound_enabled).to_string(),
        adjust: |c, _| c.audio.drum_sound_enabled ^= true,
    },
    ConfigItem {
        label: "Master Volume",
        value: |c| format!("{}%", (c.audio.master_volume * 100.0).round() as i32),
        adjust: |c, d| {
            c.audio.master_volume = (c.audio.master_volume + 0.05 * d as f32).clamp(0.0, 1.0);
        },
    },
    ConfigItem {
        label: "BGM Volume",
        value: |c| format!("{}%", (c.audio.bgm_volume * 100.0).round() as i32),
        adjust: |c, d| {
            c.audio.bgm_volume = (c.audio.bgm_volume + 0.05 * d as f32).clamp(0.0, 1.0);
        },
    },
    ConfigItem {
        label: "Drum Volume",
        value: |c| format!("{}%", (c.audio.drum_volume * 100.0).round() as i32),
        adjust: |c, d| {
            c.audio.drum_volume = (c.audio.drum_volume + 0.05 * d as f32).clamp(0.0, 1.0);
        },
    },
]);

// --- Drums tab ---

static DRUMS_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| vec![
    ConfigItem {
        label: "CY/RD Group",
        value: |c| cy_label(c.drums.cy_group).to_string(),
        adjust: |c, d| cycle_cy(c, d),
    },
    ConfigItem {
        label: "HH Group",
        value: |c| hh_label(c.drums.hh_group).to_string(),
        adjust: |c, d| cycle_hh(c, d),
    },
    ConfigItem {
        label: "FT Group",
        value: |c| ft_label(c.drums.ft_group).to_string(),
        adjust: |c, d| cycle_ft(c, d),
    },
    ConfigItem {
        label: "BD Group",
        value: |c| bd_label(c.drums.bd_group).to_string(),
        adjust: |c, d| cycle_bd(c, d),
    },
    ConfigItem {
        label: "Cymbal Free",
        value: |c| bool_label(c.drums.cymbal_free).to_string(),
        adjust: |c, _| c.drums.cymbal_free ^= true,
    },
    ConfigItem {
        label: "HH Priority",
        value: |c| hsp_label(c.drums.hit_sound_priority_hh).to_string(),
        adjust: |c, d| cycle_hsp(c, HspSlot::Hh, d),
    },
    ConfigItem {
        label: "FT Priority",
        value: |c| hsp_label(c.drums.hit_sound_priority_ft).to_string(),
        adjust: |c, d| cycle_hsp(c, HspSlot::Ft, d),
    },
    ConfigItem {
        label: "CY Priority",
        value: |c| hsp_label(c.drums.hit_sound_priority_cy).to_string(),
        adjust: |c, d| cycle_hsp(c, HspSlot::Cy, d),
    },
    ConfigItem {
        label: "LP Priority",
        value: |c| hsp_label(c.drums.hit_sound_priority_lp).to_string(),
        adjust: |c, d| cycle_hsp(c, HspSlot::Lp, d),
    },
    ConfigItem {
        label: "Polyphonic Sounds",
        value: |c| c.drums.polyphonic_sounds.to_string(),
        adjust: |c, d| {
            c.drums.polyphonic_sounds = (c.drums.polyphonic_sounds as i32 + d).clamp(1, 8) as u8;
        },
    },
]);

// === Display labels for enum types not yet carrying a `label()` ===

fn bool_label(v: bool) -> &'static str {
    if v { "On" } else { "Off" }
}

fn lane_display_label(v: dtx_config::LaneDisplay) -> &'static str {
    match v {
        dtx_config::LaneDisplay::AllOn => "All On",
        dtx_config::LaneDisplay::Half => "Half",
        dtx_config::LaneDisplay::LineOff => "Lines Off",
        dtx_config::LaneDisplay::AllOff => "All Off",
    }
}

// ponytail: 5 enum cycles, no shared trait — Rust won't synthesize
// `&mut T.member` from a generic.

fn cycle_cy(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::CyGroup::all();
    let cur = opts.iter().position(|x| *x == c.drums.cy_group).unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.cy_group = opts[next];
}
fn cycle_hh(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::HhGroup::all();
    let cur = opts.iter().position(|x| *x == c.drums.hh_group).unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.hh_group = opts[next];
}
fn cycle_ft(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::FtGroup::all();
    let cur = opts.iter().position(|x| *x == c.drums.ft_group).unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.ft_group = opts[next];
}
fn cycle_bd(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::BdGroup::all();
    let cur = opts.iter().position(|x| *x == c.drums.bd_group).unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.bd_group = opts[next];
}
fn cycle_hsp(c: &mut dtx_config::Config, slot: HspSlot, d: i32) {
    let opts = dtx_config::HitSoundPriority::all();
    let field: &mut dtx_config::HitSoundPriority = match slot {
        HspSlot::Hh => &mut c.drums.hit_sound_priority_hh,
        HspSlot::Ft => &mut c.drums.hit_sound_priority_ft,
        HspSlot::Cy => &mut c.drums.hit_sound_priority_cy,
        HspSlot::Lp => &mut c.drums.hit_sound_priority_lp,
    };
    let cur = opts.iter().position(|x| *x == *field).unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    *field = opts[next];
}

enum HspSlot { Hh, Ft, Cy, Lp }


fn cy_label(v: dtx_config::CyGroup) -> &'static str {
    match v {
        dtx_config::CyGroup::Separate => "Separate",
        dtx_config::CyGroup::Common => "Common",
    }
}

fn hh_label(v: dtx_config::HhGroup) -> &'static str {
    match v {
        dtx_config::HhGroup::SeparateAll => "All Separate",
        dtx_config::HhGroup::HhAndLc => "HH vs LC",
        dtx_config::HhGroup::HhAndHo => "HH vs HO",
        dtx_config::HhGroup::CommonAll => "All Common",
    }
}

fn ft_label(v: dtx_config::FtGroup) -> &'static str {
    match v {
        dtx_config::FtGroup::Separate => "Separate",
        dtx_config::FtGroup::Common => "Common",
    }
}

fn bd_label(v: dtx_config::BdGroup) -> &'static str {
    match v {
        dtx_config::BdGroup::Separate => "All Separate",
        dtx_config::BdGroup::BdAndLbd => "BD+LBD",
        dtx_config::BdGroup::PedalsOnly => "Pedals Only",
        dtx_config::BdGroup::AllBd => "All BD",
    }
}

fn hsp_label(v: dtx_config::HitSoundPriority) -> &'static str {
    match v {
        dtx_config::HitSoundPriority::ChipOverPad => "Chip > Pad",
        dtx_config::HitSoundPriority::PadOverChip => "Pad > Chip",
    }
}

// === Bevy components / resources ===

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

/// Per-tab row cursor (resets to 0 on tab switch).
#[derive(Resource, Debug, Default, Clone, Copy)]
struct ConfigSelection(usize);

/// Index of the active tab in the left menu.
#[derive(Resource, Debug, Default, Clone, Copy)]
struct ActiveTabIndex(usize);

// === Plugin ===

pub fn plugin(app: &mut App) {
    app.init_resource::<ConfigSelection>()
        .init_resource::<ActiveTabIndex>()
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
            (config_tab_navigation, config_row_navigation, render_config_selection)
                .run_if(in_state(AppState::Config)),
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

/// Per-state content: rows for the active tab. OnExit despawns; persistent
/// layout in `spawn_config_layout` survives.
fn spawn_config(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    draft: Res<ConfigDraft>,
    active: Res<ActiveConfigTab>,
    mut selection: ResMut<ConfigSelection>,
    mut tab_idx: ResMut<ActiveTabIndex>,
) {
    let t = theme.0;
    let tab = active.0.unwrap_or(ConfigTab::System);
    let items = tab.items();
    tab_idx.0 = ConfigTab::all().iter().position(|t| *t == tab).unwrap_or(0);
    selection.0 = 0;

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
            parent.spawn((
                Text::new(format!("Config — {}", tab.label())),
                Theme::font(36.0),
                TextColor(t.accent),
            ));
            parent.spawn((
                Text::new("Tab: ←→   Row: ↑↓   Adjust: ←→ row  |  Esc: Save & Back"),
                Theme::font(14.0),
                TextColor(t.text_secondary),
            ));

            let is_exit = matches!(tab, ConfigTab::Exit);
            if is_exit {
                parent.spawn((
                    Text::new("Save settings and return to Title."),
                    Theme::font(20.0),
                    TextColor(t.text_primary),
                ));
            } else {
                for (i, item) in items.iter().enumerate() {
                    parent
                        .spawn((
                            ConfigItemEntity(i),
                            Node {
                                width: Val::Px(560.0),
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
                            row.spawn((
                                Text::new(item.label),
                                Theme::font(16.0),
                                TextColor(t.text_primary),
                            ));
                            row.spawn((
                                ConfigValueText(i),
                                Text::new((item.value)(&draft.0)),
                                Theme::font(16.0),
                                TextColor(t.accent),
                            ));
                        });
                }
            }
        });
}

/// ←/→ switches top-level tab. Esc saves & exits. Tab switch respawns rows.
fn config_tab_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    active: ResMut<ActiveConfigTab>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
    }
    let _ = active;
}

/// ↑/↓/←/→ adjusts the current row. Tab switching and Esc handled in
/// `config_tab_navigation` for separation; this system only mutates the
/// draft for the current tab+selection.
fn config_row_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveConfigTab>,
    mut selection: ResMut<ConfigSelection>,
    mut draft: ResMut<ConfigDraft>,
) {
    let tab = match active.0 {
        Some(t) => t,
        None => return,
    };
    let items = tab.items();
    if items.is_empty() {
        return;
    }
    let max = items.len().saturating_sub(1);
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = selection.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        (items[selection.0].adjust)(&mut draft.0, 1);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        (items[selection.0].adjust)(&mut draft.0, -1);
    }
}

fn render_config_selection(
    theme: Res<ThemeResource>,
    selection: Res<ConfigSelection>,
    draft: Res<ConfigDraft>,
    mut rows: Query<(&ConfigItemEntity, &mut BackgroundColor)>,
    mut values: Query<(&ConfigValueText, &mut Text)>,
    active: Res<ActiveConfigTab>,
) {
    let t = theme.0;
    let items = match active.0 {
        Some(tab) => tab.items(),
        None => &[],
    };
    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.0 == selection.0 {
            t.accent.with_alpha(0.35)
        } else {
            t.panel_bg
        };
    }
    for (value, mut text) in &mut values {
        let i = value.0;
        let display = items
            .get(i)
            .map(|item| (item.value)(&draft.0))
            .unwrap_or_default();
        *text = Text::new(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ConfigDraft {
        ConfigDraft(dtx_config::Config::default())
    }

    // === Layout positions match BocuD reference ===

    #[test]
    fn config_left_menu_position_matches_reference() {
        assert_eq!(CONFIG_LEFT_MENU_X, 245.0);
        assert_eq!(CONFIG_LEFT_MENU_Y, 140.0);
    }

    #[test]
    fn config_cursor_size_matches_reference() {
        assert_eq!(CONFIG_CURSOR_W, 170.0);
        assert_eq!(CONFIG_CURSOR_H, 28.0);
    }

    #[test]
    fn config_description_position_matches_reference() {
        assert_eq!(CONFIG_DESC_X, 800.0);
        assert_eq!(CONFIG_DESC_Y, 270.0);
    }

    #[test]
    fn config_item_bar_matches_reference() {
        assert_eq!(CONFIG_ITEM_BAR_X, 400.0);
    }

    // === Tabs ===

    #[test]
    fn config_tabs_count_matches_reference() {
        assert_eq!(ConfigTab::all().len(), 5);
    }

    #[test]
    fn config_tabs_labels_match_reference() {
        let labels: Vec<_> = ConfigTab::all().iter().map(|t| t.label()).collect();
        assert_eq!(labels, vec!["System", "Gameplay", "Audio", "Drums", "Exit"]);
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
    fn config_tabs_setup_methods_match_reference() {
        let methods: Vec<_> = ConfigTab::all().iter().map(|t| t.setup_method()).collect();
        assert_eq!(
            methods,
            vec![
                "tSetupItemList_System",
                "tSetupItemList_Gameplay",
                "tSetupItemList_Audio",
                "tSetupItemList_Drums",
                "tSetupItemList_Exit",
            ]
        );
    }

    // === Per-tab item coverage (3.x schema fields bound to UI) ===

    #[test]
    fn system_tab_covers_all_fields() {
        // 8 SystemConfig fields.
        assert_eq!(SYSTEM_ITEMS.len(), 8);
        let labels: Vec<_> = SYSTEM_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"VSync"));
        assert!(labels.contains(&"BGA Alpha"));
        assert!(labels.contains(&"BGA Enabled"));
        assert!(labels.contains(&"Metronome"));
    }

    #[test]
    fn gameplay_tab_covers_all_fields() {
        // 11 GameplayConfig fields.
        assert_eq!(GAMEPLAY_ITEMS.len(), 11);
        let labels: Vec<_> = GAMEPLAY_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"Scroll Speed"));
        assert!(labels.contains(&"Input Offset"));
        assert!(labels.contains(&"BGM Offset"));
        assert!(labels.contains(&"Damage Level"));
        assert!(labels.contains(&"Play Speed"));
        assert!(labels.contains(&"Lane Display"));
    }

    #[test]
    fn audio_tab_covers_all_fields() {
        // 5 AudioConfig fields.
        assert_eq!(AUDIO_ITEMS.len(), 5);
        let labels: Vec<_> = AUDIO_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"Master Volume"));
        assert!(labels.contains(&"BGM Volume"));
        assert!(labels.contains(&"Drum Volume"));
        assert!(labels.contains(&"BGM Enabled"));
        assert!(labels.contains(&"Drum Hit Sound"));
    }

    #[test]
    fn drums_tab_covers_all_fields() {
        // 10 DrumsConfig fields.
        assert_eq!(DRUMS_ITEMS.len(), 10);
        let labels: Vec<_> = DRUMS_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"CY/RD Group"));
        assert!(labels.contains(&"HH Group"));
        assert!(labels.contains(&"FT Group"));
        assert!(labels.contains(&"BD Group"));
        assert!(labels.contains(&"Cymbal Free"));
        assert!(labels.contains(&"HH Priority"));
        assert!(labels.contains(&"FT Priority"));
        assert!(labels.contains(&"CY Priority"));
        assert!(labels.contains(&"LP Priority"));
        assert!(labels.contains(&"Polyphonic Sounds"));
    }

    #[test]
    fn exit_tab_has_no_items() {
        assert!(ConfigTab::Exit.items().is_empty());
    }

    #[test]
    fn tab_items_round_trip_via_config_tab() {
        for tab in ConfigTab::all() {
            assert_eq!(tab.items().as_ptr(), match tab {
                ConfigTab::System => SYSTEM_ITEMS.as_ptr(),
                ConfigTab::Gameplay => GAMEPLAY_ITEMS.as_ptr(),
                ConfigTab::Audio => AUDIO_ITEMS.as_ptr(),
                ConfigTab::Drums => DRUMS_ITEMS.as_ptr(),
                ConfigTab::Exit => [].as_ptr(),
            });
        }
    }

    // === Adjust clamping ===

    fn adjust_tab(tab: ConfigTab, row: usize, dir: i32) -> dtx_config::Config {
        let mut d = draft();
        (tab.items()[row].adjust)(&mut d.0, dir);
        d.0
    }

    #[test]
    fn scroll_speed_clamps_to_half_and_four() {
        let mut d = draft();
        d.0.gameplay.scroll_speed = 4.0;
        let items = ConfigTab::Gameplay.items();
        (items[2].adjust)(&mut d.0, 1);
        assert!((d.0.gameplay.scroll_speed - 4.0).abs() < f32::EPSILON);
        d.0.gameplay.scroll_speed = 0.5;
        (items[2].adjust)(&mut d.0, -1);
        assert!((d.0.gameplay.scroll_speed - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn input_offset_clamps_to_clamp_ms() {
        let cfg = adjust_tab(ConfigTab::Gameplay, 6, 100);
        assert_eq!(cfg.gameplay.input_offset_ms, dtx_config::INPUT_OFFSET_CLAMP_MS);
        let cfg = adjust_tab(ConfigTab::Gameplay, 6, -100);
        assert_eq!(cfg.gameplay.input_offset_ms, -dtx_config::INPUT_OFFSET_CLAMP_MS);
    }

    #[test]
    fn bgm_offset_clamps_to_clamp_ms() {
        let cfg = adjust_tab(ConfigTab::Gameplay, 7, 100);
        assert_eq!(cfg.gameplay.bgm_adjust_ms, dtx_config::BGM_ADJUST_CLAMP_MS);
    }

    #[test]
    fn master_volume_clamps_zero_to_one() {
        let cfg = adjust_tab(ConfigTab::Audio, 2, 100);
        assert!((cfg.audio.master_volume - 1.0).abs() < f32::EPSILON);
        let cfg = adjust_tab(ConfigTab::Audio, 2, -100);
        assert!((cfg.audio.master_volume - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn damage_level_cycles() {
        let mut d = draft();
        d.0.gameplay.damage_level = dtx_config::DamageLevel::None;
        let items = ConfigTab::Gameplay.items();
        (items[9].adjust)(&mut d.0, -1);
        assert_eq!(d.0.gameplay.damage_level, dtx_config::DamageLevel::High);
    }

    #[test]
    fn play_speed_clamps_to_bocud_range() {
        let mut d = draft();
        d.0.gameplay.play_speed = dtx_config::PLAY_SPEED_MAX;
        let items = ConfigTab::Gameplay.items();
        (items[8].adjust)(&mut d.0, 1);
        assert_eq!(d.0.gameplay.play_speed, dtx_config::PLAY_SPEED_MAX);
        d.0.gameplay.play_speed = dtx_config::PLAY_SPEED_MIN;
        (items[8].adjust)(&mut d.0, -1);
        assert_eq!(d.0.gameplay.play_speed, dtx_config::PLAY_SPEED_MIN);
    }

    #[test]
    fn polyphony_clamps_1_to_8() {
        let mut d = draft();
        d.0.drums.polyphonic_sounds = 1;
        let items = ConfigTab::Drums.items();
        (items[9].adjust)(&mut d.0, -1);
        assert_eq!(d.0.drums.polyphonic_sounds, 1);
        d.0.drums.polyphonic_sounds = 8;
        (items[9].adjust)(&mut d.0, 1);
        assert_eq!(d.0.drums.polyphonic_sounds, 8);
    }

    // === Display formatting ===

    #[test]
    fn damage_value_renders_label() {
        let d = draft();
        let items = ConfigTab::Gameplay.items();
        let text = (items[9].value)(&d.0);
        assert_eq!(text, "Small");
    }

    #[test]
    fn master_volume_value_renders_percent() {
        let mut d = draft();
        d.0.audio.master_volume = 0.5;
        let items = ConfigTab::Audio.items();
        assert_eq!((items[2].value)(&d.0), "50%");
    }

    #[test]
    fn scroll_speed_value_renders_x() {
        let mut d = draft();
        d.0.gameplay.scroll_speed = 1.5;
        let items = ConfigTab::Gameplay.items();
        assert_eq!((items[2].value)(&d.0), "1.5x");
    }

    // === Selection bookkeeping ===

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
    fn active_config_tab_default_is_none() {
        let a = ActiveConfigTab::default();
        assert!(a.0.is_none());
    }

    // === Total field coverage (no schema field left unbound) ===

    #[test]
    fn every_schema_field_has_a_ui_row() {
        // 8 + 11 + 5 + 10 = 34 rows; matches schema field count.
        assert_eq!(
            SYSTEM_ITEMS.len() + GAMEPLAY_ITEMS.len() + AUDIO_ITEMS.len() + DRUMS_ITEMS.len(),
            34
        );
    }
}
