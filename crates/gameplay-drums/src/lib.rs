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
//! Reference: `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/*`
//! Lane order: LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO (BocuD CActPerfDrumsLaneFlushD.cs).

// Bevy systems take many params (queries/res/commands/events) and Bevy queries
// use deeply nested generic tuples; both trip these lints across nearly every
// system in this crate. Allowed crate-wide as bevy-idiomatic false-positives.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

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
pub mod menu_nav;
pub mod miss;
pub mod mixer_events;
pub mod orchestrator;
pub mod pause;
pub mod perf_common;
pub mod perf_hotkeys;
pub mod phrase;
pub(crate) mod playback_rate;
pub mod practice;
pub mod resources;
pub mod results_analysis;
pub mod score;
pub mod scroll;
pub mod se_scheduler;
pub mod seek;
pub mod skill;
pub mod sound_bank;
pub mod stage_end;
pub mod stage_rect;
pub mod system_events;
pub mod timeline;
pub mod ui_z;
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
    Mixer,
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
    .init_resource::<resources::LaneDisplayState>()
    .init_resource::<resources::NoFailEnabled>()
    .init_resource::<resources::JudgmentCounts>()
    .init_resource::<resources::ScrollSettings>()
    .init_resource::<resources::EffectivePlaybackRate>()
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
    .init_resource::<timeline::ChipTimeline>()
    .init_resource::<seek::PendingBgmStart>()
    .init_resource::<seek::PendingAudioStarts>()
    .init_resource::<seek::LastSeekFrom>()
    .init_resource::<seek::SeekAcknowledgement>()
    .init_resource::<seek::PreviewSkippedChips>()
    .init_resource::<seek::StoppedSeekRebuild>()
    .init_resource::<dtx_bga::BgaClock>()
    .init_resource::<dtx_bga::BgaSettings>()
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
        (
            seek::reset_preview_skipped_chips,
            seek::reset_seek_transients,
        ),
    )
    .add_systems(
        OnExit(game_shell::AppState::Performance),
        seek::reset_seek_transients,
    )
    .add_systems(
        OnEnter(game_shell::AppState::Performance),
        timeline::build_chip_timeline.after(orchestrator::DrumsEnterSet),
    )
    .add_systems(
        Update,
        sync_bga_clock
            .before(dtx_bga::BgaSystems)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(practice::chart_clock_active),
    )
    .add_message::<events::LaneHit>()
    .add_message::<events::InputHit>()
    .add_message::<events::JudgmentEvent>()
    .add_message::<events::NoteMissed>()
    .add_message::<events::EmptyHit>()
    .add_message::<seek::SeekToChartTime>()
    .init_resource::<perf_common::PerformanceStageState>()
    .configure_sets(
        FixedUpdate,
        (
            DrumsSets::ClockSync.after(dtx_timing::update_audio_clock_system),
            DrumsSets::Mixer.after(DrumsSets::ClockSync),
            DrumsSets::Input.after(DrumsSets::Mixer),
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
            // Freeze the gameplay clock while paused or wait-halted.
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(practice::wait::wait_flowing)
            .run_if(practice::chart_clock_active),
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
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(practice::chart_clock_active),
    )
    .add_systems(
        FixedUpdate,
        (scroll::spawn_notes_system, seek::clear_stopped_seek_rebuild)
            .chain()
            .after(seek::apply_seek_system)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(seek::stopped_seek_rebuild_pending),
    )
    .add_plugins(playback_rate::plugin)
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
    .add_plugins(results_analysis::plugin)
    .add_plugins((
        bindings::plugin,
        beat_lines::plugin,
        se_scheduler::plugin,
        midi_gate::plugin,
        pause::plugin,
        perf_hotkeys::plugin,
        stage_end::plugin,
        stage_rect::plugin,
        practice::plugin,
        editor::plugin,
        hit_feedback::plugin,
        menu_nav::plugin,
        system_events::plugin,
        mixer_events::plugin,
    ));
}

/// Load the user's lane arrangement. `lane-profiles.toml`'s active profile is
/// authoritative (migrated from layout.toml's `[lanes]` section on first
/// run); mirrors the keyboard/MIDI startup pattern in
/// `bindings::reload_profiles`. Pure/path-parameterized so it's unit
/// testable without touching the real XDG config dir.
fn resolve_startup_lane_arrangement(
    layout_path: &std::path::Path,
    lane_registry_path: &std::path::Path,
) -> dtx_layout::LaneArrangement {
    let startup = match dtx_layout::load_layout_with_lane_authority(layout_path, lane_registry_path)
    {
        Ok((_, startup)) => startup,
        Err(error) => {
            error!("layout load failed, using default lanes: {error}");
            return dtx_layout::classic();
        }
    };
    let registry = match startup {
        dtx_layout::LaneRegistryStartup::Ready(registry) => registry,
        dtx_layout::LaneRegistryStartup::LegacySession {
            registry,
            write_error,
        } => {
            error!("lane profile registry migration write failed: {write_error}");
            registry
        }
        dtx_layout::LaneRegistryStartup::ReadOnlyBuiltins(error) => {
            error!("lane profile registry unusable, using built-ins: {error}");
            dtx_layout::lane_registry()
        }
    };
    dtx_layout::active_lane_arrangement(&registry)
}

fn load_lane_arrangement(mut lanes: ResMut<lanes::Lanes>) {
    lanes.0 =
        resolve_startup_lane_arrangement(&dtx_layout::default_path(), &lanes::lane_registry_path());
}

#[cfg(test)]
mod lane_startup_tests {
    use super::resolve_startup_lane_arrangement;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("dtx-lane-startup-load")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("test dir");
        dir
    }

    /// Regression for the redesign: booting straight into Performance (no
    /// editor visit) must read the ACTIVE lane profile from
    /// `lane-profiles.toml`, not just whatever `[lanes]` happens to say in
    /// layout.toml. Simulates "picked NX Type-B, relaunched" by writing a
    /// lane registry whose active profile disagrees with layout.toml.
    #[test]
    fn startup_reads_active_profile_from_lane_registry_not_layout_toml() {
        let dir = temp_dir("registry-authority");
        let layout_path = dir.join("layout.toml");
        let lane_registry_path = dir.join("lane-profiles.toml");

        // layout.toml still says Classic (stale compatibility snapshot).
        std::fs::write(
            &layout_path,
            toml::to_string_pretty(&dtx_layout::LayoutFile::default()).expect("layout toml"),
        )
        .expect("write layout");

        // lane-profiles.toml is the authority and says NX Type-B.
        let registry = dtx_layout::LaneProfileRegistry {
            active: "NX Type-B".to_owned(),
            ..dtx_layout::lane_registry()
        };
        std::fs::write(
            &lane_registry_path,
            toml::to_string_pretty(&registry).expect("registry toml"),
        )
        .expect("write registry");

        let arrangement = resolve_startup_lane_arrangement(&layout_path, &lane_registry_path);
        assert_eq!(arrangement, dtx_layout::nx_type_b());
        assert_ne!(arrangement, dtx_layout::classic());

        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn load_scroll_settings(mut settings: ResMut<resources::ScrollSettings>) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    *settings = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
}

/// Map the persisted `dtx_config::DamageLevel` onto the gameplay
/// `dtx_core::constants::DamageLevel` used by the gauge.
pub(crate) fn map_damage_level(level: dtx_config::DamageLevel) -> dtx_core::constants::DamageLevel {
    use dtx_config::DamageLevel as Cfg;
    use dtx_core::constants::DamageLevel as Core;
    match level {
        Cfg::None => Core::Small,
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
    mut lane_display: ResMut<resources::LaneDisplayState>,
    mut no_fail: ResMut<resources::NoFailEnabled>,
    mut bga_settings: ResMut<dtx_bga::BgaSettings>,
    chart: Res<resources::ActiveChart>,
) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    *bga_settings = dtx_bga::BgaSettings::from_configs(&cfg.system, &cfg.accessibility);
    *scroll = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
    audio.bgm_enabled = cfg.audio.bgm_enabled;
    audio.drum_enabled = cfg.audio.drum_sound_enabled;
    audio.master_volume = cfg.audio.master_volume;
    audio.bgm_volume = cfg.audio.bgm_volume;
    audio.drum_volume = cfg.audio.drum_volume;
    gauge.damage_level = map_damage_level(cfg.gameplay.damage_level);
    input_offset.0 = cfg.gameplay.input_offset_ms;
    bgm_adjust.common_ms = cfg.gameplay.bgm_adjust_ms;
    show_perf_info.0 = cfg.system.show_perf_info;
    metronome_on.0 = cfg.system.metronome;
    show_timing_lines.0 = cfg.gameplay.lane_display.shows_timing_lines();
    lane_display.0 = cfg.gameplay.lane_display;
    no_fail.0 = cfg.gameplay.fail_mode() == dtx_config::FailMode::NoFail;
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
        bgm_enabled: cfg.audio.bgm_enabled,
        drum_enabled: cfg.audio.drum_sound_enabled,
        master_volume: cfg.audio.master_volume,
        bgm_volume: cfg.audio.bgm_volume,
        drum_volume: cfg.audio.drum_volume,
    };
    drum_cfg.config = cfg.drums.clone();
    drum_cfg.fillin_enabled = cfg.gameplay.fillin_enabled;
    polyphony.set_voices(cfg.drums.polyphonic_sounds);
}

/// Mirror the authoritative gameplay chart time into `dtx_bga::BgaClock` so
/// visual playback follows pause and practice seeks without a second clock.
fn sync_bga_clock(gameplay: Res<resources::GameplayClock>, mut visuals: ResMut<dtx_bga::BgaClock>) {
    visuals.current_ms = gameplay.current_ms;
}

fn sync_gameplay_clock(
    audio_clock: Res<dtx_timing::AudioClock>,
    start_ms: Res<resources::GameStartMs>,
    rate: Res<resources::EffectivePlaybackRate>,
    time: Res<Time<Fixed>>,
    mut gameplay_clock: ResMut<resources::GameplayClock>,
) {
    // BGM position is stream-local; add the primary chip's chart time so the
    // clock matches drum chip `target_ms` (dtxpt: bgm.start_time + position).
    let chart_ms = audio_clock
        .current_ms
        .map(|pos| start_ms.0.saturating_add(pos));
    gameplay_clock.tick(rate.scaled_delta_secs(time.delta_secs_f64()), chart_ms);
}

mod midi_gate {
    //! Gates dtx-input's `ResolvedInputHit` into gameplay `InputHit`.
    //!
    //! The pump (connection, drain, velocity filter, resolution) moved to
    //! `dtx_input::pump` (menu-nav extraction, 2026-07-15 spec). This module
    //! owns the only gameplay-specific part: deciding whether gameplay is
    //! ready and restamping with the gameplay clock.

    use bevy::prelude::*;
    use dtx_input::ResolvedInputHit;

    use super::events::InputHit;
    use crate::resources::GameplayClock;

    pub use dtx_input::{LastMidiHit, PadNavHit};

    pub(super) fn plugin(app: &mut App) {
        app.add_plugins(dtx_input::pump::plugin)
            .configure_sets(
                FixedUpdate,
                dtx_input::InputPumpSet.before(super::DrumsSets::Input),
            )
            .add_systems(
                FixedUpdate,
                convert_resolved_hits.in_set(super::DrumsSets::Input),
            );
    }

    /// Timestamp for an emitted hit: the event's own stamp if it has one,
    /// else the gameplay clock, else 0 (menus don't care about timing).
    pub(crate) fn stamp_audio_ms(clock_ms: Option<i64>, event_ms: i64) -> i64 {
        if event_ms != 0 {
            event_ms
        } else {
            clock_ms.unwrap_or(0)
        }
    }

    fn gameplay_ready(
        chart_ready: bool,
        clock_ready: bool,
        practice_ready: bool,
        pause: &game_shell::PauseState,
    ) -> bool {
        chart_ready && clock_ready && practice_ready && *pause == game_shell::PauseState::Running
    }

    fn convert_resolved_hits(
        chart: Res<crate::resources::ActiveChart>,
        clock: Res<GameplayClock>,
        flow: Option<Res<crate::practice::PracticeFlow>>,
        pause: Res<State<game_shell::PauseState>>,
        mut resolved: MessageReader<ResolvedInputHit>,
        mut hits: MessageWriter<InputHit>,
    ) {
        let ready = gameplay_ready(
            !chart.chart.chips.is_empty(),
            clock.is_ready(),
            crate::practice::gameplay_input_active(flow),
            pause.get(),
        );
        if !ready {
            // Drop, don't defer: an unread message replays next frame, and a
            // hit buffered while paused/not-ready must never judge later.
            resolved.clear();
            return;
        }
        for hit in resolved.read() {
            hits.write(InputHit {
                lanes: hit.lanes.clone(),
                audio_ms: stamp_audio_ms(Some(clock.current_ms), hit.audio_ms),
                captured_at: hit.captured_at,
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn paused_gameplay_is_never_ready() {
            assert!(!gameplay_ready(
                true,
                true,
                true,
                &game_shell::PauseState::Paused
            ));
            assert!(gameplay_ready(
                true,
                true,
                true,
                &game_shell::PauseState::Running
            ));
        }

        #[test]
        fn stamp_prefers_event_time_then_clock() {
            assert_eq!(stamp_audio_ms(Some(500), 2_000), 2_000);
            assert_eq!(stamp_audio_ms(Some(500), 0), 500);
            assert_eq!(stamp_audio_ms(None, 0), 0);
        }

        /// The old pump dropped not-ready hits instead of buffering them. The gate
        /// must do the same: a hit that arrives while not ready is cleared, not
        /// replayed once gameplay becomes ready.
        #[test]
        fn gated_hit_is_not_replayed_when_gameplay_becomes_ready() {
            use dtx_input::ResolvedInputHit;

            let mut app = App::new();
            app.add_plugins(bevy::state::app::StatesPlugin)
                .init_state::<game_shell::PauseState>()
                .init_resource::<crate::resources::ActiveChart>()
                .init_resource::<GameplayClock>()
                .add_message::<ResolvedInputHit>()
                .add_message::<InputHit>()
                .add_systems(Update, convert_resolved_hits);

            // Not ready: empty chart. The hit must be dropped.
            app.world_mut().write_message(ResolvedInputHit {
                lanes: vec![1],
                audio_ms: 0,
                captured_at: std::time::Instant::now(),
            });
            app.update();
            app.update(); // second frame: a buffered message would surface here
            let count = app
                .world()
                .resource::<Messages<InputHit>>()
                .iter_current_update_messages()
                .count();
            assert_eq!(count, 0, "not-ready hit must be dropped, not deferred");
        }
    }
}

pub use midi_gate::{LastMidiHit, PadNavHit};

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as DrumsPlugin;

#[cfg(test)]
mod tests {
    #[test]
    fn drums_fixed_timestep_is_60hz() {
        assert_eq!(super::DRUMS_FIXED_TIMESTEP_HZ, 60.0);
    }

    #[test]
    fn menu_hits_are_stamped_even_without_clock() {
        use super::midi_gate::stamp_audio_ms;
        assert_eq!(stamp_audio_ms(None, 123), 123);
        assert_eq!(stamp_audio_ms(None, 0), 0);
        assert_eq!(stamp_audio_ms(Some(5000), 0), 5000);
        assert_eq!(stamp_audio_ms(Some(5000), 123), 123);
    }

    #[test]
    fn normal_play_analysis_reports_timing_lane_and_loop() {
        use super::results_analysis::{analyze_normal_play, RecordedJudgment};
        use dtx_scoring::JudgmentKind;

        let events = vec![
            RecordedJudgment::new(3, JudgmentKind::Poor, -25, 0, 2_100),
            RecordedJudgment::new(3, JudgmentKind::Miss, 0, 1, 2_200),
            RecordedJudgment::new(3, JudgmentKind::Poor, -20, 2, 2_300),
            RecordedJudgment::new(1, JudgmentKind::Perfect, -15, 3, 4_100),
            RecordedJudgment::new(1, JudgmentKind::Great, -20, 4, 4_200),
            RecordedJudgment::new(1, JudgmentKind::Perfect, -10, 5, 4_300),
        ];

        let report = analyze_normal_play(&events, &[0, 2_000, 4_000, 6_000, 8_000]);
        assert_eq!(report.bias_ms, Some(-20));
        assert_eq!(report.spread_ms, Some(5));
        assert_eq!(report.weakest_lane.expect("weak lane").lane, 3);
        let section = report.weakest_section.expect("weak section");
        assert_eq!(section.loop_start_ms, 0);
        assert_eq!(section.loop_end_ms, 6_000);
    }

    #[test]
    fn normal_play_stream_keeps_a_bounded_prefix() {
        use super::results_analysis::{NormalPlayEventStream, RecordedJudgment};
        use dtx_scoring::JudgmentKind;

        let mut stream = NormalPlayEventStream::default();
        for chip_idx in 0..=8_192 {
            stream.push(RecordedJudgment::new(
                1,
                JudgmentKind::Perfect,
                0,
                chip_idx,
                chip_idx as i64,
            ));
        }

        assert_eq!(stream.events.len(), 8_192);
        assert_eq!(stream.events[0].chip_idx, 0);
        assert_eq!(stream.events.last().expect("kept event").chip_idx, 8_191);
        assert!(stream.truncated);
    }
}
