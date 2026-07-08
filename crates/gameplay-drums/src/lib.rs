//! Drums gameplay vertical slice.
//!
//! Game layer. Owns the per-frame loop:
//!   input → LaneHit → judge vs GameplayClock → JudgmentEvent → score/combo
//!
//! Wires together: `dtx-core` (chart), `dtx-scoring` (judgment classify),
//! `dtx-timing` (audio clock), `dtx-audio` (BGM).
//!
//! v1 mechanics-only core loop + osu-style HUD (`hud.rs`, ADR-0014).
//! State machines (DrumsPad, DrumsDanger, DrumsFillingEffect) live here.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*`
//! Lane order: LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO (BocuD CActPerfDrumsLaneFlushD.cs).

pub mod autoplay;
pub mod beat_lines;
pub mod bgm_scheduler;
pub mod bindings;
pub mod components;
pub mod damage_level;
pub mod derived;
pub mod drum_groups;
pub mod drums_perf;
pub mod editor;
pub mod events;
pub mod gauge;
pub mod hit_feedback;
pub mod hit_sound;
pub mod hud;
pub mod hud_cache;
pub mod input;
pub mod interp;
pub mod judge;
pub mod keyboard_viz;
pub mod lane_map;
pub mod lanes;
pub mod layout;
pub mod miss;
pub mod orchestrator;
pub mod pause;
pub mod perf_common;
pub mod perf_hotkeys;
pub mod phrase;
pub mod practice;
pub mod resources;
pub mod score;
pub mod scroll;
pub mod se_scheduler;
pub mod seek;
pub mod skill;
pub mod sound_bank;
pub mod stage_end;
pub mod stage_rect;
pub mod timeline;
pub mod widget_layout;

use std::time::Duration;

use bevy::prelude::*;

pub const DRUMS_FIXED_TIMESTEP_HZ: f64 = 60.0;

/// Execution ordering for drums gameplay systems.
///
/// ClockSync → Input → NoteSpawn → Judge → Score
///
/// Guarantees:
/// - Clock updates before anything reads it
/// - Input reads fresh clock before judge processes hits
/// - Notes spawn with current clock before scroll/despawn
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrumsSets {
    ClockSync,
    Input,
    NoteSpawn,
    Judge,
    Score,
}

/// Root plugin: register all sub-plugins in dependency order.
pub fn plugin(app: &mut App) {
    app.insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(
        1.0 / DRUMS_FIXED_TIMESTEP_HZ,
    )))
    .init_resource::<resources::ActiveChart>()
    .init_resource::<resources::TimingLineList>()
    .init_resource::<resources::Score>()
    .init_resource::<resources::DrumScoring>()
    .init_resource::<resources::Combo>()
    .init_resource::<resources::GameStartMs>()
    .init_resource::<resources::InputOffsetMs>()
    .init_resource::<resources::BgmAdjustState>()
    .init_resource::<resources::ShowPerfInfo>()
    .init_resource::<resources::MetronomeEnabled>()
    .init_resource::<resources::ShowTimingLines>()
    .init_resource::<resources::JudgmentCounts>()
    .init_resource::<resources::ScrollSettings>()
    .init_resource::<resources::AudioRate>()
    .init_resource::<resources::GameplayClock>()
    .init_resource::<resources::DrumGameplaySettings>()
    .init_resource::<resources::DrumAudioSettings>()
    .init_resource::<resources::SkillValue>()
    .init_resource::<resources::FastSlowCount>()
    .init_resource::<resources::AccuracyHistory>()
    .init_resource::<phrase::PhraseMeter>()
    .init_resource::<derived::ChartDerived>()
    .init_resource::<dtx_audio::DrumPolyphony>()
    .init_resource::<lanes::Lanes>()
    .init_resource::<hud_cache::HudDisplayCache>()
    .init_resource::<dtx_input::midi::VirtualSource>()
    .init_resource::<timeline::ChipTimeline>()
    .init_resource::<seek::PendingBgmStart>()
    .init_resource::<seek::LastSeekFrom>()
    .add_systems(
        Startup,
        (
            load_scroll_settings,
            load_drum_audio_settings,
            load_lane_arrangement,
        ),
    )
    .add_systems(
        OnEnter(game_shell::AppState::Performance),
        apply_config_on_enter.before(orchestrator::DrumsEnterSet),
    )
    .add_systems(
        OnEnter(game_shell::AppState::Performance),
        timeline::build_chip_timeline.after(orchestrator::DrumsEnterSet),
    )
    .add_message::<events::LaneHit>()
    .add_message::<events::JudgmentEvent>()
    .add_message::<events::NoteMissed>()
    .add_message::<events::EmptyHit>()
    .add_message::<seek::SeekToChartTime>()
    .init_resource::<perf_common::PerformanceStageState>()
    .configure_sets(
        FixedUpdate,
        (
            DrumsSets::ClockSync.after(dtx_timing::update_audio_clock_system),
            DrumsSets::Input.after(DrumsSets::ClockSync),
            DrumsSets::NoteSpawn.after(DrumsSets::Input),
            DrumsSets::Judge.after(DrumsSets::NoteSpawn),
            DrumsSets::Score.after(DrumsSets::Judge),
        ),
    )
    .add_systems(
        FixedUpdate,
        (
            dtx_timing::update_audio_clock_system,
            sync_gameplay_clock.in_set(DrumsSets::ClockSync),
        )
            .chain()
            .run_if(in_state(game_shell::AppState::Performance))
            // Freeze the gameplay clock while paused.
            .run_if(in_state(game_shell::PauseState::Running)),
    )
    .add_systems(
        FixedUpdate,
        seek::apply_seek_system
            .before(dtx_timing::update_audio_clock_system)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        FixedUpdate,
        seek::start_pending_bgm
            .after(seek::apply_seek_system)
            .before(dtx_timing::update_audio_clock_system)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running)),
    )
    .add_plugins((
        layout::plugin,
        input::plugin,
        scroll::plugin,
        judge::plugin,
        score::plugin,
        miss::plugin,
        gauge::plugin,
        hud::plugin,
        widget_layout::plugin,
        keyboard_viz::plugin,
        orchestrator::plugin,
        autoplay::plugin,
        hit_sound::plugin,
        bgm_scheduler::plugin,
        interp::plugin,
    ))
    .add_plugins((
        bindings::plugin,
        beat_lines::plugin,
        se_scheduler::plugin,
        midi_consumer::plugin,
        pause::plugin,
        perf_hotkeys::plugin,
        stage_end::plugin,
        stage_rect::plugin,
        practice::plugin,
        editor::plugin,
        hit_feedback::plugin,
    ));
}

/// Load the user's lane arrangement from layout.toml (defaults on absence).
fn load_lane_arrangement(mut lanes: ResMut<lanes::Lanes>) {
    let file = dtx_layout::load(&dtx_layout::default_path());
    lanes.0 = file.lanes.resolve();
}

fn load_scroll_settings(mut settings: ResMut<resources::ScrollSettings>) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    *settings = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
}

/// Map the persisted `dtx_config::DamageLevel` onto the gameplay
/// `dtx_core::constants::DamageLevel` used by the gauge.
fn map_damage_level(level: dtx_config::DamageLevel) -> dtx_core::constants::DamageLevel {
    use dtx_config::DamageLevel as Cfg;
    use dtx_core::constants::DamageLevel as Core;
    match level {
        Cfg::None => Core::None,
        Cfg::Small => Core::Small,
        Cfg::Normal => Core::Normal,
        Cfg::High => Core::High,
    }
}

/// Re-read persisted config on entering a performance so edits made in the
/// Config screen (scroll speed, master volume, damage level) take effect
/// without an app restart.
fn apply_config_on_enter(
    mut scroll: ResMut<resources::ScrollSettings>,
    mut audio: ResMut<resources::DrumAudioSettings>,
    mut gauge: ResMut<gauge::StageGauge>,
    mut input_offset: ResMut<resources::InputOffsetMs>,
    mut bgm_adjust: ResMut<resources::BgmAdjustState>,
    mut show_perf_info: ResMut<resources::ShowPerfInfo>,
    mut metronome_on: ResMut<resources::MetronomeEnabled>,
    mut show_timing_lines: ResMut<resources::ShowTimingLines>,
    chart: Res<resources::ActiveChart>,
) {
    use dtx_config::{default_path, load, play_speed_multiplier};
    let cfg = load(&default_path());
    *scroll = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
    scroll.play_speed = play_speed_multiplier(cfg.gameplay.play_speed);
    audio.enabled = cfg.audio.drum_sound_enabled;
    audio.master_volume = cfg.audio.master_volume;
    audio.drum_volume = cfg.audio.drum_volume;
    gauge.damage_level = map_damage_level(cfg.gameplay.damage_level);
    input_offset.0 = cfg.gameplay.input_offset_ms;
    bgm_adjust.common_ms = cfg.gameplay.bgm_adjust_ms;
    show_perf_info.0 = cfg.system.show_perf_info;
    metronome_on.0 = cfg.system.metronome;
    show_timing_lines.0 = cfg.gameplay.lane_display.shows_timing_lines();
    bgm_adjust.song_ms = chart
        .source_path
        .as_ref()
        .map(|p| dtx_scoring::score_ini::read_bgm_adjust(dtx_scoring::score_ini::score_ini_path(p)))
        .unwrap_or(0);
}

fn load_drum_audio_settings(
    mut settings: ResMut<resources::DrumAudioSettings>,
    mut drum_cfg: ResMut<resources::DrumGameplaySettings>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    *settings = resources::DrumAudioSettings {
        enabled: cfg.audio.drum_sound_enabled,
        master_volume: cfg.audio.master_volume,
        drum_volume: cfg.audio.drum_volume,
    };
    drum_cfg.config = cfg.drums.clone();
    polyphony.set_voices(cfg.drums.polyphonic_sounds);
}

fn sync_gameplay_clock(
    audio_clock: Res<dtx_timing::AudioClock>,
    start_ms: Res<resources::GameStartMs>,
    rate: Res<resources::AudioRate>,
    time: Res<Time<Fixed>>,
    mut gameplay_clock: ResMut<resources::GameplayClock>,
) {
    // BGM position is stream-local; add the primary chip's chart time so the
    // clock matches drum chip `target_ms` (dtxpt: bgm.start_time + position).
    let chart_ms = audio_clock
        .current_ms
        .map(|pos| start_ms.0.saturating_add(pos));
    gameplay_clock.tick(time.delta_secs_f64() * rate.0, chart_ms);
}

mod midi_consumer {
    //! Polls `dtx_input::midi::VirtualSource` and emits gameplay-drums `LaneHit`s.

    use bevy::prelude::*;
    use dtx_input::midi::{MidiSource, VirtualSource};

    use super::events::LaneHit;
    use crate::resources::{ActiveChart, GameplayClock};

    pub(super) fn plugin(app: &mut App) {
        app.add_systems(
            FixedUpdate,
            poll_midi
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
    }

    fn poll_midi(
        mut source: ResMut<VirtualSource>,
        resolver: Res<crate::bindings::BindResolver>,
        chart: Res<ActiveChart>,
        clock: Res<GameplayClock>,
        mut hits: MessageWriter<LaneHit>,
    ) {
        if source.is_empty() {
            return;
        }
        if chart.chart.chips.is_empty() {
            return;
        }
        if !clock.is_ready() {
            return;
        }
        let mut buf: Vec<dtx_input::midi::MidiEvent> = Vec::new();
        (*source).poll(&mut buf);
        for ev in buf {
            // NoteOff / CC ignored (HH pedal CC handling is v2).
            let dtx_input::midi::MidiEvent::NoteOn {
                note,
                velocity,
                audio_ms,
            } = ev
            else {
                continue;
            };
            if velocity == 0 || velocity <= resolver.velocity_threshold {
                continue;
            }
            let Some(lane) = resolver.lane_for_note(note) else {
                continue;
            };
            hits.write(LaneHit {
                lane,
                audio_ms: if audio_ms != 0 {
                    audio_ms
                } else {
                    clock.current_ms
                },
            });
        }
    }
}

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as DrumsPlugin;

#[cfg(test)]
mod tests {
    #[test]
    fn drums_fixed_timestep_is_60hz() {
        assert_eq!(super::DRUMS_FIXED_TIMESTEP_HZ, 60.0);
    }
}
