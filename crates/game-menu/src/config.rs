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
//! GITADORA redesign (ADR-0014 revision): a fixed left section rail
//! (SYSTEM/GAMEPLAY/AUDIO/DRUMS-KEYS/EXIT) replaces the BocuD pixel-anchored
//! chrome. `Tab` cycles the rail section, respawning the row list + description
//! panel for the new tab. All user-facing text says "Settings".

use bevy::prelude::*;
use dtx_ui::ThemeResource;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::theme::Theme;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::{panel, set_panel_selected};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};
use std::sync::LazyLock;

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
        [
            Self::System,
            Self::Gameplay,
            Self::Audio,
            Self::Drums,
            Self::Exit,
        ]
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
/// `desc` is the one-line explanation shown in the description panel when
/// the row is selected.
///
/// Ponytail: function pointers over trait objects — single concrete type,
/// no vtable, no boxing. Closure capture would work but `fn` keeps the
/// call site obvious and the data table declarative.
#[derive(Clone, Copy)]
pub struct ConfigItem {
    pub label: &'static str,
    pub value: fn(&dtx_config::Config) -> String,
    pub adjust: fn(&mut dtx_config::Config, i32),
    pub desc: &'static str,
}

// --- System tab ---
// ponytail: closures inside vec! need `LazyLock` (format! isn't const-eval-able).

static SYSTEM_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| {
    vec![
        ConfigItem {
            label: "VSync",
            value: |c| bool_label(c.system.vsync).to_string(),
            adjust: |c, _| c.system.vsync ^= true,
            desc: "Lock framerate to display refresh. Reduces tearing; adds up to one frame of latency.",
        },
        ConfigItem {
            label: "Perf Info",
            value: |c| bool_label(c.system.show_perf_info).to_string(),
            adjust: |c, _| c.system.show_perf_info ^= true,
            desc: "Show an FPS / frame-time overlay.",
        },
        ConfigItem {
            label: "Metronome",
            value: |c| bool_label(c.system.metronome).to_string(),
            adjust: |c, _| c.system.metronome ^= true,
            desc: "Play a click on every beat during gameplay.",
        },
    ]
});

// --- Gameplay tab ---

static GAMEPLAY_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| {
    vec![
        ConfigItem {
            label: "Scroll Speed",
            value: |c| format!("{:.1}x", c.gameplay.scroll_speed),
            adjust: |c, d| {
                c.gameplay.scroll_speed =
                    (c.gameplay.scroll_speed + 0.5 * d as f32).clamp(0.5, 9.0);
            },
            desc: "Note scroll speed multiplier during gameplay.",
        },
        ConfigItem {
            label: "Input Offset",
            value: |c| format!("{:+} ms", c.gameplay.input_offset_ms),
            adjust: |c, d| {
                c.gameplay.input_offset_ms = (c.gameplay.input_offset_ms + 10 * d).clamp(
                    -dtx_config::INPUT_OFFSET_CLAMP_MS,
                    dtx_config::INPUT_OFFSET_CLAMP_MS,
                );
            },
            desc: "Shift the judgement clock to compensate for input lag, in milliseconds.",
        },
        ConfigItem {
            label: "BGM Offset",
            value: |c| format!("{:+} ms", c.gameplay.bgm_adjust_ms),
            adjust: |c, d| {
                c.gameplay.bgm_adjust_ms = (c.gameplay.bgm_adjust_ms + 10 * d).clamp(
                    -dtx_config::BGM_ADJUST_CLAMP_MS,
                    dtx_config::BGM_ADJUST_CLAMP_MS,
                );
            },
            desc: "Shift background music timing relative to notes, in milliseconds.",
        },
        ConfigItem {
            label: "Play Speed",
            value: |c| {
                format!(
                    "{:.2}x",
                    dtx_config::play_speed_multiplier(c.gameplay.play_speed)
                )
            },
            adjust: |c, d| {
                let raw = (c.gameplay.play_speed as i32 + d).clamp(
                    dtx_config::PLAY_SPEED_MIN as i32,
                    dtx_config::PLAY_SPEED_MAX as i32,
                );
                c.gameplay.play_speed = raw as u8;
            },
            desc: "Chart playback speed multiplier (0.5x-2.0x); affects both notes and audio.",
        },
        ConfigItem {
            label: "Damage Level",
            value: |c| c.gameplay.damage_level.label().to_string(),
            adjust: |c, d| {
                let levels = dtx_config::DamageLevel::all();
                let cur = levels
                    .iter()
                    .position(|l| *l == c.gameplay.damage_level)
                    .unwrap_or(0) as i32;
                let next = (cur + d).rem_euclid(levels.len() as i32) as usize;
                c.gameplay.damage_level = levels[next];
            },
            desc: "How much life is lost per miss.",
        },
        ConfigItem {
            label: "Lane Display",
            value: |c| lane_display_label(c.gameplay.lane_display).to_string(),
            adjust: |c, d| {
                let opts = dtx_config::LaneDisplay::all();
                let cur = opts
                    .iter()
                    .position(|l| *l == c.gameplay.lane_display)
                    .unwrap_or(0) as i32;
                let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
                c.gameplay.lane_display = opts[next];
            },
            desc: "Toggle visibility of lane backgrounds and bar/beat lines.",
        },
    ]
});

// --- Audio tab ---

static AUDIO_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| {
    vec![
        ConfigItem {
            label: "Drum Hit Sound",
            value: |c| bool_label(c.audio.drum_sound_enabled).to_string(),
            adjust: |c, _| c.audio.drum_sound_enabled ^= true,
            desc: "Play a sound when a drum pad is hit.",
        },
        ConfigItem {
            label: "Master Volume",
            value: |c| format!("{}%", (c.audio.master_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.master_volume = (c.audio.master_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Overall output volume.",
        },
        ConfigItem {
            label: "BGM Volume",
            value: |c| format!("{}%", (c.audio.bgm_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.bgm_volume = (c.audio.bgm_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Chart BGM and song preview volume.",
        },
        ConfigItem {
            label: "Drum Volume",
            value: |c| format!("{}%", (c.audio.drum_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.drum_volume = (c.audio.drum_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Drum hit sound volume.",
        },
        ConfigItem {
            label: "BGM Sound",
            value: |c| bool_label(c.audio.bgm_enabled).to_string(),
            adjust: |c, _| c.audio.bgm_enabled ^= true,
            desc: "Play chart BGM and song previews.",
        },
    ]
});

// --- Drums tab ---

static DRUMS_ITEMS: LazyLock<Vec<ConfigItem>> = LazyLock::new(|| {
    vec![
        ConfigItem {
            label: "CY/RD Group",
            value: |c| cy_label(c.drums.cy_group).to_string(),
            adjust: |c, d| cycle_cy(c, d),
            desc: "Whether the CY and RD pads trigger separate or shared chip sounds.",
        },
        ConfigItem {
            label: "HH Group",
            value: |c| hh_label(c.drums.hh_group).to_string(),
            adjust: |c, d| cycle_hh(c, d),
            desc: "How hi-hat, left-cymbal and open-hi-hat pads are grouped for chip playback.",
        },
        ConfigItem {
            label: "FT Group",
            value: |c| ft_label(c.drums.ft_group).to_string(),
            adjust: |c, d| cycle_ft(c, d),
            desc: "Whether floor tom and low tom pads trigger separate or shared chip sounds.",
        },
        ConfigItem {
            label: "BD Group",
            value: |c| bd_label(c.drums.bd_group).to_string(),
            adjust: |c, d| cycle_bd(c, d),
            desc: "How bass drum and pedal pads are grouped for chip playback.",
        },
        ConfigItem {
            label: "Cymbal Free",
            value: |c| bool_label(c.drums.cymbal_free).to_string(),
            adjust: |c, _| c.drums.cymbal_free ^= true,
            desc: "Allow cymbal pads to be hit freely without a matching chip on the chart.",
        },
        ConfigItem {
            label: "HH Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_hh).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Hh, d),
            desc: "Whether chip or pad sound wins when both would play for hi-hat hits.",
        },
        ConfigItem {
            label: "FT Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_ft).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Ft, d),
            desc: "Whether chip or pad sound wins when both would play for floor tom hits.",
        },
        ConfigItem {
            label: "CY Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_cy).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Cy, d),
            desc: "Whether chip or pad sound wins when both would play for cymbal hits.",
        },
        ConfigItem {
            label: "LP Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_lp).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Lp, d),
            desc: "Whether chip or pad sound wins when both would play for left-pedal hits.",
        },
        ConfigItem {
            label: "Polyphonic Sounds",
            value: |c| c.drums.polyphonic_sounds.to_string(),
            adjust: |c, d| {
                c.drums.polyphonic_sounds =
                    (c.drums.polyphonic_sounds as i32 + d).clamp(1, 8) as u8;
            },
            desc: "Maximum simultaneous drum hit sounds (1-8).",
        },
    ]
});

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
    let cur = opts
        .iter()
        .position(|x| *x == c.drums.cy_group)
        .unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.cy_group = opts[next];
}
fn cycle_hh(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::HhGroup::all();
    let cur = opts
        .iter()
        .position(|x| *x == c.drums.hh_group)
        .unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.hh_group = opts[next];
}
fn cycle_ft(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::FtGroup::all();
    let cur = opts
        .iter()
        .position(|x| *x == c.drums.ft_group)
        .unwrap_or(0) as i32;
    let next = (cur + d).rem_euclid(opts.len() as i32) as usize;
    c.drums.ft_group = opts[next];
}
fn cycle_bd(c: &mut dtx_config::Config, d: i32) {
    let opts = dtx_config::BdGroup::all();
    let cur = opts
        .iter()
        .position(|x| *x == c.drums.bd_group)
        .unwrap_or(0) as i32;
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

enum HspSlot {
    Hh,
    Ft,
    Cy,
    Lp,
}

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

/// Left-rail section label, tagged with its index into `ConfigTab::all()`.
#[derive(Component, Debug, Clone, Copy)]
struct RailTabLabel(usize);

/// Description panel text, updated to the selected row's `ConfigItem::desc`.
#[derive(Component, Debug, Clone, Copy)]
struct SettingsDescText;

/// Per-tab row cursor (resets to 0 on tab switch).
#[derive(Resource, Debug, Default, Clone, Copy)]
struct ConfigSelection(usize);

/// Index of the active tab in the left rail.
#[derive(Resource, Debug, Default, Clone, Copy)]
struct ActiveTabIndex(usize);

// === Plugin ===

pub fn plugin(app: &mut App) {
    app.init_resource::<ConfigSelection>()
        .init_resource::<ActiveTabIndex>()
        .init_resource::<ActiveConfigTab>()
        .init_resource::<ConfigDraft>()
        .add_systems(
            OnEnter(AppState::Config),
            (load_config_draft, populate_default_tab, spawn_config).chain(),
        )
        .add_systems(
            OnExit(AppState::Config),
            (save_config_draft, despawn_stage::<ConfigEntity>).chain(),
        )
        .add_systems(
            Update,
            (
                config_tab_navigation,
                config_row_navigation,
                render_config_selection,
                highlight_active_rail_tab,
            )
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

fn populate_default_tab(mut active: ResMut<ActiveConfigTab>) {
    if active.0.is_none() {
        active.0 = Some(ConfigTab::System);
    }
}

/// Per-state content: rail + rows for the active tab, description panel,
/// hint bar. OnEnter spawn; `config_tab_navigation` despawns + rebuilds this
/// on Tab press.
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
    tab_idx.0 = ConfigTab::all().iter().position(|x| *x == tab).unwrap_or(0);
    selection.0 = 0;
    build_config_content(&mut commands, &t, &draft.0, tab);
}

/// Builds the settings screen root (stage background, left rail, rows,
/// description panel, hint bar) for `tab`. Shared by `spawn_config` (OnEnter)
/// and `config_tab_navigation` (Tab-press respawn) so both paths stay in
/// sync; despawn + spawn are queued as `Commands` on the same buffer, so
/// they apply in order without a cross-system flush race.
fn build_config_content(
    commands: &mut Commands,
    t: &Theme,
    draft: &dtx_config::Config,
    tab: ConfigTab,
) {
    let items = tab.items();

    commands
        .spawn((
            ConfigEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, t);

            // left rail
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(220.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-220.0, 0.0), 0.0, 220.0),
            ))
            .with_children(|rail| {
                rail.spawn((
                    Text::new("SETTINGS"),
                    Theme::font(24.0),
                    TextColor(t.text_primary),
                ));
                for (i, tab_i) in ConfigTab::all().iter().enumerate() {
                    let is_active = *tab_i == tab;
                    rail.spawn((
                        RailTabLabel(i),
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            margin: UiRect::top(Val::Px(if i == 0 { 24.0 } else { 0.0 })),
                            ..default()
                        },
                        BackgroundColor(if is_active {
                            t.select_yellow
                        } else {
                            Color::NONE
                        }),
                        Text::new(tab_i.label().to_uppercase()),
                        Theme::font(15.0),
                        TextColor(if is_active {
                            Color::BLACK
                        } else {
                            t.text_secondary
                        }),
                    ));
                }
            });

            // rows
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(250.0),
                top: Val::Px(50.0),
                width: Val::Px(680.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|list| {
                if matches!(tab, ConfigTab::Exit) {
                    list.spawn((
                        Text::new("Save settings and return to Title. (ENTER)"),
                        Theme::font(18.0),
                        TextColor(t.text_primary),
                    ));
                } else {
                    for (i, item) in items.iter().enumerate() {
                        list.spawn((
                            ConfigItemEntity(i),
                            panel(
                                t,
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                                    ..default()
                                },
                            ),
                            UiTransform::default(),
                            EnterChoreo::slide(Vec2::new(240.0, 0.0), i as f32 * 20.0, 200.0),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(item.label),
                                Theme::font(16.0),
                                TextColor(t.text_primary),
                            ));
                            row.spawn((
                                ConfigValueText(i),
                                Text::new(format!("◂ {} ▸", (item.value)(draft))),
                                Theme::font(16.0),
                                TextColor(t.clear_green),
                            ));
                        });
                    }
                }
            });

            // description panel
            root.spawn((
                panel(
                    t,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(250.0),
                        bottom: Val::Px(60.0),
                        width: Val::Px(680.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                ),
                SettingsDescText,
                Text::new(items.first().map(|i| i.desc).unwrap_or("")),
                Theme::font(14.0),
                TextColor(t.text_secondary),
            ));

            // hint bar
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(18.0),
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|bar| {
                for (label, hot) in [
                    ("↑↓ ROW", false),
                    ("←→ ADJUST", false),
                    ("TAB SECTION", false),
                    ("ESC SAVE & BACK", true),
                ] {
                    bar.spawn((
                        Text::new(label),
                        Theme::font(12.0),
                        TextColor(if hot {
                            t.select_yellow
                        } else {
                            t.text_secondary
                        }),
                    ));
                }
            });
        });
}

/// Tab cycles the active section: despawns the current screen content and
/// rebuilds it for the new tab (rows re-enter with the stagger choreography).
/// Both operations are queued as `Commands` on this system's own buffer, so
/// they are guaranteed to apply in order — no cross-system flush timing to
/// get wrong. Esc saves (via `OnExit`) and returns to Title.
#[allow(clippy::too_many_arguments)]
fn config_tab_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<ActiveConfigTab>,
    mut requests: MessageWriter<TransitionRequest>,
    mut commands: Commands,
    roots: Query<Entity, With<ConfigEntity>>,
    theme: Res<ThemeResource>,
    draft: Res<ConfigDraft>,
    mut selection: ResMut<ConfigSelection>,
    mut tab_idx: ResMut<ActiveTabIndex>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let all = ConfigTab::all();
        let cur = active.0.unwrap_or(ConfigTab::System);
        let idx = all.iter().position(|t| *t == cur).unwrap_or(0);
        let next_idx = (idx + 1) % all.len();
        let next = all[next_idx];
        active.0 = Some(next);
        tab_idx.0 = next_idx;
        selection.0 = 0;
        for e in &roots {
            commands.entity(e).despawn();
        }
        let t = theme.0;
        build_config_content(&mut commands, &t, &draft.0, next);
    }
}

/// ↑/↓/←/→ adjusts the current row. On the Exit tab, Enter saves (via
/// `OnExit`) and returns to Title. Tab switching and Esc handled in
/// `config_tab_navigation` for separation; this system only mutates the
/// draft for the current tab+selection.
fn config_row_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveConfigTab>,
    mut selection: ResMut<ConfigSelection>,
    mut draft: ResMut<ConfigDraft>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    let tab = match active.0 {
        Some(t) => t,
        None => return,
    };
    if matches!(tab, ConfigTab::Exit) {
        if keys.just_pressed(KeyCode::Enter) {
            request_transition(&mut requests, AppState::Title);
        }
        return;
    }
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

/// Applies selection chrome (yellow border + glow) to the selected row,
/// wraps its value in `◂ ▸` arrows, and writes its description into
/// `SettingsDescText`. `values` is disjoint from `desc` via `Without` —
/// both would otherwise alias `&mut Text` on the same system and panic at
/// startup.
fn render_config_selection(
    theme: Res<ThemeResource>,
    selection: Res<ConfigSelection>,
    draft: Res<ConfigDraft>,
    active: Res<ActiveConfigTab>,
    mut rows: Query<(
        &ConfigItemEntity,
        &mut BorderColor,
        &mut BoxShadow,
        &mut BackgroundColor,
    )>,
    mut values: Query<(&ConfigValueText, &mut Text), Without<SettingsDescText>>,
    mut desc: Query<&mut Text, With<SettingsDescText>>,
) {
    let t = theme.0;
    let items = match active.0 {
        Some(tab) => tab.items(),
        None => &[],
    };
    for (row, mut border, mut shadow, mut bg) in &mut rows {
        let selected = row.0 == selection.0;
        set_panel_selected(&t, selected, &mut border, &mut shadow);
        bg.0 = t.stage_panel_bg;
    }
    for (value, mut text) in &mut values {
        let display = items
            .get(value.0)
            .map(|i| (i.value)(&draft.0))
            .unwrap_or_default();
        *text = Text::new(if value.0 == selection.0 {
            format!("◂ {display} ▸")
        } else {
            display
        });
    }
    if let Some(item) = items.get(selection.0) {
        for mut text in &mut desc {
            *text = Text::new(item.desc);
        }
    }
}

/// Keeps rail section highlighting in sync with `ActiveTabIndex`, independent
/// of the bake-at-spawn colors in `build_config_content`.
fn highlight_active_rail_tab(
    theme: Res<ThemeResource>,
    tab_idx: Res<ActiveTabIndex>,
    mut rail: Query<(&RailTabLabel, &mut BackgroundColor, &mut TextColor)>,
) {
    let t = theme.0;
    for (label, mut bg, mut color) in &mut rail {
        let is_active = label.0 == tab_idx.0;
        bg.0 = if is_active {
            t.select_yellow
        } else {
            Color::NONE
        };
        *color = TextColor(if is_active {
            Color::BLACK
        } else {
            t.text_secondary
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ConfigDraft {
        ConfigDraft(dtx_config::Config::default())
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

    // === Per-tab item coverage (only settings wired to actual game behavior) ===

    #[test]
    fn system_tab_covers_all_fields() {
        assert_eq!(SYSTEM_ITEMS.len(), 3);
        let labels: Vec<_> = SYSTEM_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"VSync"));
        assert!(labels.contains(&"Perf Info"));
        assert!(labels.contains(&"Metronome"));
    }

    #[test]
    fn gameplay_tab_covers_all_fields() {
        assert_eq!(GAMEPLAY_ITEMS.len(), 6);
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
        assert_eq!(AUDIO_ITEMS.len(), 5);
        let labels: Vec<_> = AUDIO_ITEMS.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"Master Volume"));
        assert!(labels.contains(&"BGM Volume"));
        assert!(labels.contains(&"Drum Volume"));
        assert!(labels.contains(&"BGM Sound"));
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
            assert_eq!(
                tab.items().as_ptr(),
                match tab {
                    ConfigTab::System => SYSTEM_ITEMS.as_ptr(),
                    ConfigTab::Gameplay => GAMEPLAY_ITEMS.as_ptr(),
                    ConfigTab::Audio => AUDIO_ITEMS.as_ptr(),
                    ConfigTab::Drums => DRUMS_ITEMS.as_ptr(),
                    ConfigTab::Exit => [].as_ptr(),
                }
            );
        }
    }

    // === Descriptions ===

    #[test]
    fn every_item_has_description() {
        for tab in ConfigTab::all() {
            for item in tab.items() {
                assert!(!item.desc.is_empty(), "{} missing desc", item.label);
            }
        }
    }

    // === Adjust clamping ===

    fn adjust_tab(tab: ConfigTab, row: usize, dir: i32) -> dtx_config::Config {
        let mut d = draft();
        (tab.items()[row].adjust)(&mut d.0, dir);
        d.0
    }

    #[test]
    fn scroll_speed_clamps_to_half_and_nine() {
        let mut d = draft();
        d.0.gameplay.scroll_speed = 9.0;
        let items = ConfigTab::Gameplay.items();
        (items[0].adjust)(&mut d.0, 1);
        assert!((d.0.gameplay.scroll_speed - 9.0).abs() < f32::EPSILON);
        d.0.gameplay.scroll_speed = 0.5;
        (items[0].adjust)(&mut d.0, -1);
        assert!((d.0.gameplay.scroll_speed - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn input_offset_clamps_to_clamp_ms() {
        let cfg = adjust_tab(ConfigTab::Gameplay, 1, 100);
        assert_eq!(
            cfg.gameplay.input_offset_ms,
            dtx_config::INPUT_OFFSET_CLAMP_MS
        );
        let cfg = adjust_tab(ConfigTab::Gameplay, 1, -100);
        assert_eq!(
            cfg.gameplay.input_offset_ms,
            -dtx_config::INPUT_OFFSET_CLAMP_MS
        );
    }

    #[test]
    fn bgm_offset_clamps_to_clamp_ms() {
        let cfg = adjust_tab(ConfigTab::Gameplay, 2, 100);
        assert_eq!(cfg.gameplay.bgm_adjust_ms, dtx_config::BGM_ADJUST_CLAMP_MS);
    }

    #[test]
    fn master_volume_clamps_zero_to_one() {
        let cfg = adjust_tab(ConfigTab::Audio, 1, 100);
        assert!((cfg.audio.master_volume - 1.0).abs() < f32::EPSILON);
        let cfg = adjust_tab(ConfigTab::Audio, 1, -100);
        assert!((cfg.audio.master_volume - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bgm_volume_clamps_zero_to_one() {
        let cfg = adjust_tab(ConfigTab::Audio, 2, 100);
        assert!((cfg.audio.bgm_volume - 1.0).abs() < f32::EPSILON);
        let cfg = adjust_tab(ConfigTab::Audio, 2, -100);
        assert!((cfg.audio.bgm_volume - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn damage_level_cycles() {
        let mut d = draft();
        d.0.gameplay.damage_level = dtx_config::DamageLevel::None;
        let items = ConfigTab::Gameplay.items();
        (items[4].adjust)(&mut d.0, -1);
        assert_eq!(d.0.gameplay.damage_level, dtx_config::DamageLevel::High);
    }

    #[test]
    fn play_speed_clamps_to_bocud_range() {
        let mut d = draft();
        d.0.gameplay.play_speed = dtx_config::PLAY_SPEED_MAX;
        let items = ConfigTab::Gameplay.items();
        (items[3].adjust)(&mut d.0, 1);
        assert_eq!(d.0.gameplay.play_speed, dtx_config::PLAY_SPEED_MAX);
        d.0.gameplay.play_speed = dtx_config::PLAY_SPEED_MIN;
        (items[3].adjust)(&mut d.0, -1);
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
        let text = (items[4].value)(&d.0);
        assert_eq!(text, "Small");
    }

    #[test]
    fn master_volume_value_renders_percent() {
        let mut d = draft();
        d.0.audio.master_volume = 0.5;
        let items = ConfigTab::Audio.items();
        assert_eq!((items[1].value)(&d.0), "50%");
    }

    #[test]
    fn bgm_volume_value_renders_percent() {
        let mut d = draft();
        d.0.audio.bgm_volume = 0.5;
        let items = ConfigTab::Audio.items();
        assert_eq!((items[2].value)(&d.0), "50%");
    }

    #[test]
    fn scroll_speed_value_renders_x() {
        let mut d = draft();
        d.0.gameplay.scroll_speed = 1.5;
        let items = ConfigTab::Gameplay.items();
        assert_eq!((items[0].value)(&d.0), "1.5x");
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

    // === Total row count (only settings wired to actual game behavior) ===

    #[test]
    fn every_schema_field_has_a_ui_row() {
        // 3 + 6 + 5 + 10 = 24 rows.
        assert_eq!(
            SYSTEM_ITEMS.len() + GAMEPLAY_ITEMS.len() + AUDIO_ITEMS.len() + DRUMS_ITEMS.len(),
            24
        );
    }
}
