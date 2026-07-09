//! Settings row tables for the Customize surface (System/Gameplay/Audio/Drums).
//!
//! Ported verbatim from the former `game-menu::config` screen — same
//! `value`/`adjust`/`desc` semantics, now rendered as Customize tabs. Each row
//! also carries a `group` section label, a `control` kind (draggable Slider vs
//! discrete Stepper), and `raw`/`set`/`reset` hooks so sliders can read/write
//! the underlying number and RESET TAB can restore defaults uniformly.

use std::sync::LazyLock;

use game_shell::CustomizeTab;

/// How a settings row is rendered on the right side of its row.
#[derive(Clone, Copy)]
pub enum SettingControl {
    /// Draggable slider over a continuous numeric field (`raw`/`set` used).
    Slider { min: f32, max: f32, step: f32 },
    /// Discrete `◂ value ▸` stepper driven by `adjust`.
    Stepper,
}

/// One editable setting row. `value` reads the current value as a display
/// string; `adjust` mutates `Config` with `dir = ±1`; `desc` is the one-line
/// explanation. `group` is a section header ("" = no header). `control` picks
/// slider vs stepper. `raw`/`set` read/write the underlying number for slider
/// rows (steppers leave them as no-ops). `reset` copies this row's field from a
/// provided default `Config`.
#[derive(Clone, Copy)]
pub struct SettingItem {
    pub label: &'static str,
    pub value: fn(&dtx_config::Config) -> String,
    pub adjust: fn(&mut dtx_config::Config, i32),
    pub desc: &'static str,
    pub group: &'static str,
    pub control: SettingControl,
    pub raw: fn(&dtx_config::Config) -> f32,
    pub set: fn(&mut dtx_config::Config, f32),
    pub reset: fn(&mut dtx_config::Config, &dtx_config::Config),
}

// --- System tab ---
// ponytail: closures inside vec! need `LazyLock` (format! isn't const-eval-able).

static SYSTEM_ITEMS: LazyLock<Vec<SettingItem>> = LazyLock::new(|| {
    vec![
        SettingItem {
            label: "VSync",
            value: |c| bool_label(c.system.vsync).to_string(),
            adjust: |c, _| c.system.vsync ^= true,
            desc: "Lock framerate to display refresh. Reduces tearing; adds up to one frame of latency.",
            group: "DISPLAY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.system.vsync = d.system.vsync,
        },
        SettingItem {
            label: "Perf Info",
            value: |c| bool_label(c.system.show_perf_info).to_string(),
            adjust: |c, _| c.system.show_perf_info ^= true,
            desc: "Show an FPS / frame-time overlay.",
            group: "DISPLAY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.system.show_perf_info = d.system.show_perf_info,
        },
        SettingItem {
            label: "Metronome",
            value: |c| bool_label(c.system.metronome).to_string(),
            adjust: |c, _| c.system.metronome ^= true,
            desc: "Play a click on every beat during gameplay.",
            group: "GAMEPLAY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.system.metronome = d.system.metronome,
        },
    ]
});

// --- Gameplay tab ---

static GAMEPLAY_ITEMS: LazyLock<Vec<SettingItem>> = LazyLock::new(|| {
    vec![
        SettingItem {
            label: "Scroll Speed",
            value: |c| format!("{:.1}x", c.gameplay.scroll_speed),
            adjust: |c, d| {
                c.gameplay.scroll_speed =
                    (c.gameplay.scroll_speed + 0.5 * d as f32).clamp(0.5, 9.0);
            },
            desc: "Note scroll speed multiplier during gameplay.",
            group: "FEEL",
            control: SettingControl::Slider {
                min: 0.5,
                max: 9.0,
                step: 0.5,
            },
            raw: |c| c.gameplay.scroll_speed,
            set: |c, v| c.gameplay.scroll_speed = v.clamp(0.5, 9.0),
            reset: |c, d| c.gameplay.scroll_speed = d.gameplay.scroll_speed,
        },
        SettingItem {
            label: "Input Offset",
            value: |c| format!("{:+} ms", c.gameplay.input_offset_ms),
            adjust: |c, d| {
                c.gameplay.input_offset_ms = (c.gameplay.input_offset_ms + 10 * d).clamp(
                    -dtx_config::INPUT_OFFSET_CLAMP_MS,
                    dtx_config::INPUT_OFFSET_CLAMP_MS,
                );
            },
            desc: "Shift the judgement clock to compensate for input lag, in milliseconds.",
            group: "FEEL",
            control: SettingControl::Slider {
                min: -(dtx_config::INPUT_OFFSET_CLAMP_MS as f32),
                max: dtx_config::INPUT_OFFSET_CLAMP_MS as f32,
                step: 10.0,
            },
            raw: |c| c.gameplay.input_offset_ms as f32,
            set: |c, v| {
                c.gameplay.input_offset_ms = (v.round() as i32).clamp(
                    -dtx_config::INPUT_OFFSET_CLAMP_MS,
                    dtx_config::INPUT_OFFSET_CLAMP_MS,
                );
            },
            reset: |c, d| c.gameplay.input_offset_ms = d.gameplay.input_offset_ms,
        },
        SettingItem {
            label: "BGM Offset",
            value: |c| format!("{:+} ms", c.gameplay.bgm_adjust_ms),
            adjust: |c, d| {
                c.gameplay.bgm_adjust_ms = (c.gameplay.bgm_adjust_ms + 10 * d).clamp(
                    -dtx_config::BGM_ADJUST_CLAMP_MS,
                    dtx_config::BGM_ADJUST_CLAMP_MS,
                );
            },
            desc: "Shift background music timing relative to notes, in milliseconds.",
            group: "FEEL",
            control: SettingControl::Slider {
                min: -(dtx_config::BGM_ADJUST_CLAMP_MS as f32),
                max: dtx_config::BGM_ADJUST_CLAMP_MS as f32,
                step: 10.0,
            },
            raw: |c| c.gameplay.bgm_adjust_ms as f32,
            set: |c, v| {
                c.gameplay.bgm_adjust_ms = (v.round() as i32).clamp(
                    -dtx_config::BGM_ADJUST_CLAMP_MS,
                    dtx_config::BGM_ADJUST_CLAMP_MS,
                );
            },
            reset: |c, d| c.gameplay.bgm_adjust_ms = d.gameplay.bgm_adjust_ms,
        },
        SettingItem {
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
            group: "",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.gameplay.play_speed = d.gameplay.play_speed,
        },
        SettingItem {
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
            group: "RULES",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.gameplay.damage_level = d.gameplay.damage_level,
        },
        SettingItem {
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
            group: "RULES",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.gameplay.lane_display = d.gameplay.lane_display,
        },
    ]
});

// --- Audio tab ---

static AUDIO_ITEMS: LazyLock<Vec<SettingItem>> = LazyLock::new(|| {
    vec![
        SettingItem {
            label: "BGM Sound",
            value: |c| bool_label(c.audio.bgm_enabled).to_string(),
            adjust: |c, _| c.audio.bgm_enabled ^= true,
            desc: "Play chart background music.",
            group: "",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.audio.bgm_enabled = d.audio.bgm_enabled,
        },
        SettingItem {
            label: "Drum Hit Sound",
            value: |c| bool_label(c.audio.drum_sound_enabled).to_string(),
            adjust: |c, _| c.audio.drum_sound_enabled ^= true,
            desc: "Play a sound when a drum pad is hit.",
            group: "",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.audio.drum_sound_enabled = d.audio.drum_sound_enabled,
        },
        SettingItem {
            label: "Master Volume",
            value: |c| format!("{}%", (c.audio.master_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.master_volume = (c.audio.master_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Overall output volume.",
            group: "LEVELS",
            control: SettingControl::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            raw: |c| c.audio.master_volume,
            set: |c, v| c.audio.master_volume = v.clamp(0.0, 1.0),
            reset: |c, d| c.audio.master_volume = d.audio.master_volume,
        },
        SettingItem {
            label: "BGM Volume",
            value: |c| format!("{}%", (c.audio.bgm_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.bgm_volume = (c.audio.bgm_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Chart background music volume.",
            group: "LEVELS",
            control: SettingControl::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            raw: |c| c.audio.bgm_volume,
            set: |c, v| c.audio.bgm_volume = v.clamp(0.0, 1.0),
            reset: |c, d| c.audio.bgm_volume = d.audio.bgm_volume,
        },
        SettingItem {
            label: "Drum Volume",
            value: |c| format!("{}%", (c.audio.drum_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.drum_volume = (c.audio.drum_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Drum hit sound volume.",
            group: "LEVELS",
            control: SettingControl::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            raw: |c| c.audio.drum_volume,
            set: |c, v| c.audio.drum_volume = v.clamp(0.0, 1.0),
            reset: |c, d| c.audio.drum_volume = d.audio.drum_volume,
        },
    ]
});

// --- Drums tab ---

static DRUMS_ITEMS: LazyLock<Vec<SettingItem>> = LazyLock::new(|| {
    vec![
        SettingItem {
            label: "CY/RD Group",
            value: |c| cy_label(c.drums.cy_group).to_string(),
            adjust: |c, d| cycle_cy(c, d),
            desc: "Whether the CY and RD pads trigger separate or shared chip sounds.",
            group: "GROUPING",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.cy_group = d.drums.cy_group,
        },
        SettingItem {
            label: "HH Group",
            value: |c| hh_label(c.drums.hh_group).to_string(),
            adjust: |c, d| cycle_hh(c, d),
            desc: "How hi-hat, left-cymbal and open-hi-hat pads are grouped for chip playback.",
            group: "GROUPING",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.hh_group = d.drums.hh_group,
        },
        SettingItem {
            label: "FT Group",
            value: |c| ft_label(c.drums.ft_group).to_string(),
            adjust: |c, d| cycle_ft(c, d),
            desc: "Whether floor tom and low tom pads trigger separate or shared chip sounds.",
            group: "GROUPING",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.ft_group = d.drums.ft_group,
        },
        SettingItem {
            label: "BD Group",
            value: |c| bd_label(c.drums.bd_group).to_string(),
            adjust: |c, d| cycle_bd(c, d),
            desc: "How bass drum and pedal pads are grouped for chip playback.",
            group: "GROUPING",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.bd_group = d.drums.bd_group,
        },
        SettingItem {
            label: "Cymbal Free",
            value: |c| bool_label(c.drums.cymbal_free).to_string(),
            adjust: |c, _| c.drums.cymbal_free ^= true,
            desc: "Allow cymbal pads to be hit freely without a matching chip on the chart.",
            group: "GROUPING",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.cymbal_free = d.drums.cymbal_free,
        },
        SettingItem {
            label: "HH Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_hh).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Hh, d),
            desc: "Whether chip or pad sound wins when both would play for hi-hat hits.",
            group: "PRIORITY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.hit_sound_priority_hh = d.drums.hit_sound_priority_hh,
        },
        SettingItem {
            label: "FT Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_ft).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Ft, d),
            desc: "Whether chip or pad sound wins when both would play for floor tom hits.",
            group: "PRIORITY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.hit_sound_priority_ft = d.drums.hit_sound_priority_ft,
        },
        SettingItem {
            label: "CY Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_cy).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Cy, d),
            desc: "Whether chip or pad sound wins when both would play for cymbal hits.",
            group: "PRIORITY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.hit_sound_priority_cy = d.drums.hit_sound_priority_cy,
        },
        SettingItem {
            label: "LP Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_lp).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Lp, d),
            desc: "Whether chip or pad sound wins when both would play for left-pedal hits.",
            group: "PRIORITY",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.hit_sound_priority_lp = d.drums.hit_sound_priority_lp,
        },
        SettingItem {
            label: "Polyphonic Sounds",
            value: |c| c.drums.polyphonic_sounds.to_string(),
            adjust: |c, d| {
                c.drums.polyphonic_sounds =
                    (c.drums.polyphonic_sounds as i32 + d).clamp(1, 8) as u8;
            },
            desc: "Maximum simultaneous drum hit sounds (1-8).",
            group: "PLAYBACK",
            control: SettingControl::Stepper,
            raw: |_| 0.0,
            set: |_, _| {},
            reset: |c, d| c.drums.polyphonic_sounds = d.drums.polyphonic_sounds,
        },
    ]
});

// === Display labels for enum types not yet carrying a `label()` ===

fn bool_label(v: bool) -> &'static str {
    if v {
        "On"
    } else {
        "Off"
    }
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

/// Rows for a settings tab. Non-settings tabs (Lanes/Widgets) return `&[]`.
pub fn settings_items(tab: CustomizeTab) -> &'static [SettingItem] {
    match tab {
        CustomizeTab::System => &SYSTEM_ITEMS,
        CustomizeTab::Gameplay => &GAMEPLAY_ITEMS,
        CustomizeTab::Audio => &AUDIO_ITEMS,
        CustomizeTab::Drums => &DRUMS_ITEMS,
        CustomizeTab::Bindings | CustomizeTab::Lanes | CustomizeTab::Widgets => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tabs_have_rows() {
        for tab in game_shell::CustomizeTab::SETTINGS {
            assert!(!settings_items(tab).is_empty(), "{tab:?} has no rows");
        }
    }

    #[test]
    fn scroll_speed_adjust_changes_value() {
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::Gameplay);
        let scroll = items.iter().find(|i| i.label == "Scroll Speed").unwrap();
        let before = (scroll.value)(&cfg);
        (scroll.adjust)(&mut cfg, 1);
        let after = (scroll.value)(&cfg);
        assert_ne!(before, after);
    }

    #[test]
    fn vsync_toggle_round_trips() {
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::System);
        let vsync = items.iter().find(|i| i.label == "VSync").unwrap();
        let start = (vsync.value)(&cfg);
        (vsync.adjust)(&mut cfg, 1);
        (vsync.adjust)(&mut cfg, 1);
        assert_eq!(start, (vsync.value)(&cfg));
    }

    #[test]
    fn slider_set_and_raw_round_trip() {
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::Gameplay);
        let scroll = items.iter().find(|i| i.label == "Scroll Speed").unwrap();
        (scroll.set)(&mut cfg, 4.0);
        assert_eq!((scroll.raw)(&cfg), 4.0);
    }

    #[test]
    fn reset_restores_default() {
        let def = dtx_config::Config::default();
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::Gameplay);
        let scroll = items.iter().find(|i| i.label == "Scroll Speed").unwrap();
        (scroll.adjust)(&mut cfg, 1);
        assert_ne!((scroll.value)(&cfg), (scroll.value)(&def));
        (scroll.reset)(&mut cfg, &def);
        assert_eq!((scroll.value)(&cfg), (scroll.value)(&def));
    }
}
