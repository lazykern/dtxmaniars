//! Settings row tables for the Customize surface (System/Gameplay/Audio/Drums).
//!
//! Ported verbatim from the former `game-menu::config` screen — same
//! `value`/`adjust`/`desc` semantics, now rendered as Customize tabs.

use std::sync::LazyLock;

use game_shell::CustomizeTab;

/// One editable setting row. `value` reads the current value as a display
/// string; `adjust` mutates `Config` with `dir = ±1`; `desc` is the one-line
/// explanation.
#[derive(Clone, Copy)]
pub struct SettingItem {
    pub label: &'static str,
    pub value: fn(&dtx_config::Config) -> String,
    pub adjust: fn(&mut dtx_config::Config, i32),
    pub desc: &'static str,
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
        },
        SettingItem {
            label: "Perf Info",
            value: |c| bool_label(c.system.show_perf_info).to_string(),
            adjust: |c, _| c.system.show_perf_info ^= true,
            desc: "Show an FPS / frame-time overlay.",
        },
        SettingItem {
            label: "Metronome",
            value: |c| bool_label(c.system.metronome).to_string(),
            adjust: |c, _| c.system.metronome ^= true,
            desc: "Play a click on every beat during gameplay.",
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
        },
    ]
});

// --- Audio tab ---

static AUDIO_ITEMS: LazyLock<Vec<SettingItem>> = LazyLock::new(|| {
    vec![
        SettingItem {
            label: "Drum Hit Sound",
            value: |c| bool_label(c.audio.drum_sound_enabled).to_string(),
            adjust: |c, _| c.audio.drum_sound_enabled ^= true,
            desc: "Play a sound when a drum pad is hit.",
        },
        SettingItem {
            label: "Master Volume",
            value: |c| format!("{}%", (c.audio.master_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.master_volume = (c.audio.master_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Overall output volume.",
        },
        SettingItem {
            label: "Drum Volume",
            value: |c| format!("{}%", (c.audio.drum_volume * 100.0).round() as i32),
            adjust: |c, d| {
                c.audio.drum_volume = (c.audio.drum_volume + 0.05 * d as f32).clamp(0.0, 1.0);
            },
            desc: "Drum hit sound volume.",
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
        },
        SettingItem {
            label: "HH Group",
            value: |c| hh_label(c.drums.hh_group).to_string(),
            adjust: |c, d| cycle_hh(c, d),
            desc: "How hi-hat, left-cymbal and open-hi-hat pads are grouped for chip playback.",
        },
        SettingItem {
            label: "FT Group",
            value: |c| ft_label(c.drums.ft_group).to_string(),
            adjust: |c, d| cycle_ft(c, d),
            desc: "Whether floor tom and low tom pads trigger separate or shared chip sounds.",
        },
        SettingItem {
            label: "BD Group",
            value: |c| bd_label(c.drums.bd_group).to_string(),
            adjust: |c, d| cycle_bd(c, d),
            desc: "How bass drum and pedal pads are grouped for chip playback.",
        },
        SettingItem {
            label: "Cymbal Free",
            value: |c| bool_label(c.drums.cymbal_free).to_string(),
            adjust: |c, _| c.drums.cymbal_free ^= true,
            desc: "Allow cymbal pads to be hit freely without a matching chip on the chart.",
        },
        SettingItem {
            label: "HH Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_hh).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Hh, d),
            desc: "Whether chip or pad sound wins when both would play for hi-hat hits.",
        },
        SettingItem {
            label: "FT Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_ft).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Ft, d),
            desc: "Whether chip or pad sound wins when both would play for floor tom hits.",
        },
        SettingItem {
            label: "CY Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_cy).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Cy, d),
            desc: "Whether chip or pad sound wins when both would play for cymbal hits.",
        },
        SettingItem {
            label: "LP Priority",
            value: |c| hsp_label(c.drums.hit_sound_priority_lp).to_string(),
            adjust: |c, d| cycle_hsp(c, HspSlot::Lp, d),
            desc: "Whether chip or pad sound wins when both would play for left-pedal hits.",
        },
        SettingItem {
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
        CustomizeTab::Lanes | CustomizeTab::Widgets => &[],
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
}
