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
        timeline::build_chip_timeline.after(orchestrator::DrumsEnterSet),
    )
    .add_systems(
        Update,
        sync_bga_clock
            .before(dtx_bga::BgaSystems)
            .run_if(in_state(game_shell::AppState::Performance)),
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
            // Freeze the gameplay clock while paused or wait-halted.
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(practice::wait::wait_flowing),
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
        menu_nav::plugin,
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
        dtx_layout::LaneRegistryStartup::LegacySession { registry, write_error } => {
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
    lanes.0 = resolve_startup_lane_arrangement(&dtx_layout::default_path(), &lanes::lane_registry_path());
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
    mut bga_settings: ResMut<dtx_bga::BgaSettings>,
    chart: Res<resources::ActiveChart>,
) {
    use dtx_config::{default_path, load, play_speed_multiplier};
    let cfg = load(&default_path());
    *bga_settings = dtx_bga::BgaSettings::from(&cfg.system);
    *scroll = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
    scroll.play_speed = play_speed_multiplier(cfg.gameplay.play_speed);
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
    //!
    //! When the `midi` feature is enabled, a feature-gated `drain_real_midi`
    //! system drains a real `midir`-backed source into `VirtualSource` first,
    //! so real events flow through the exact same path as virtual ones and
    //! `poll_midi` handles everything uniformly.

    use bevy::prelude::*;
    #[cfg(feature = "midi")]
    use bevy::time::common_conditions::on_real_timer;
    use dtx_input::midi::{MidiSource, VirtualSource};

    use super::events::LaneHit;
    use crate::resources::GameplayClock;

    /// Last MIDI NoteOn observed by `poll_midi`, written before the threshold
    /// gate. Drives the bindings-tab velocity meter and MIDI note capture,
    /// avoiding a second drain that would race `poll_midi`.
    #[derive(Resource, Default, Debug, Clone, Copy)]
    pub struct LastMidiHit {
        pub note: u8,
        pub velocity: u8,
        pub below_threshold: bool,
        pub at: Option<std::time::Instant>,
    }

    /// Holds the live real-MIDI connection. Stored as a **non-send** resource
    /// because `midir` connections are not `Sync`; systems touching it run on
    /// the main thread only.
    #[cfg(feature = "midi")]
    #[derive(Default)]
    struct MidiConnection {
        source: Option<dtx_input::midi::RealMidiSource>,
        port_filter: Option<String>,
    }

    pub(super) fn plugin(app: &mut App) {
        // Not state-gated: pads navigate menus outside Performance too.
        app.init_resource::<LastMidiHit>()
            .add_message::<PadNavHit>()
            .add_systems(FixedUpdate, poll_midi.in_set(super::DrumsSets::Input));

        #[cfg(feature = "midi")]
        {
            app.insert_non_send(MidiConnection::default())
                .add_systems(Startup, connect_midi)
                .add_systems(
                    Update,
                    connect_midi.run_if(
                        resource_changed::<crate::bindings::LiveBindings>
                            .or_else(on_real_timer(std::time::Duration::from_secs(1))),
                    ),
                )
                .add_systems(
                    FixedUpdate,
                    drain_real_midi
                        .in_set(super::DrumsSets::Input)
                        .before(poll_midi),
                );
        }
    }

    /// Connect (or reconnect) the real MIDI source using the port filter from
    /// `LiveBindings`. Runs at startup, whenever the selected port changes,
    /// and once per second so devices plugged in after boot are discovered.
    /// Reconnect overwrites, dropping the old
    /// connection. Non-send: runs on the main thread only.
    #[cfg(feature = "midi")]
    fn connect_midi(
        mut conn: NonSendMut<MidiConnection>,
        live: Res<crate::bindings::LiveBindings>,
        mut connected: ResMut<game_shell::MidiConnected>,
    ) {
        let filter = live.0.midi.port.clone();
        if conn.source.is_some() && conn.port_filter == filter {
            return;
        }
        match dtx_input::midi::RealMidiSource::connect(filter.as_deref()) {
            Ok((src, name)) => {
                info!("MIDI connected: {name}");
                conn.source = Some(src);
                conn.port_filter = filter;
                connected.0 = true;
            }
            Err(e) => {
                warn!("MIDI connect failed: {e}");
                conn.source = None;
                conn.port_filter = filter;
                connected.0 = false;
            }
        }
    }

    /// Drain the real MIDI source into `VirtualSource` so real events are
    /// indistinguishable from virtual ones downstream. Real events carry
    /// `audio_ms == 0`; `poll_midi` restamps them with the current
    /// `GameplayClock`. Non-send: runs on the main thread only.
    #[cfg(feature = "midi")]
    fn drain_real_midi(mut conn: NonSendMut<MidiConnection>, mut virt: ResMut<VirtualSource>) {
        let Some(src) = conn.source.as_mut() else {
            return;
        };
        let mut buf: Vec<dtx_input::midi::MidiEvent> = Vec::new();
        src.poll(&mut buf);
        for ev in buf {
            virt.push(ev);
        }
    }

    /// A resolved hit from a real pad, for menu navigation only.
    ///
    /// Separate from `LaneHit` on purpose: `LaneHit` is also written by autoplay
    /// (which the Customize surface forces on) and by keyboard lane keys, and
    /// neither should ever steer a menu.
    #[derive(Debug, Clone, Copy, Message)]
    pub struct PadNavHit {
        /// Lane id per `crate::lane_map::LANE_ORDER`.
        pub lane: u8,
    }

    /// Timestamp for an emitted `LaneHit`: the event's own stamp if it has one,
    /// else the gameplay clock, else 0 (menus don't care about timing).
    pub(crate) fn stamp_audio_ms(clock_ms: Option<i64>, event_ms: i64) -> i64 {
        if event_ms != 0 {
            event_ms
        } else {
            clock_ms.unwrap_or(0)
        }
    }

    fn poll_midi(
        mut source: ResMut<VirtualSource>,
        resolver: Res<crate::bindings::BindResolver>,
        chart: Res<crate::resources::ActiveChart>,
        clock: Res<GameplayClock>,
        mut hits: MessageWriter<LaneHit>,
        mut nav_hits: MessageWriter<PadNavHit>,
        mut last: ResMut<LastMidiHit>,
    ) {
        if source.is_empty() {
            return;
        }
        let mut buf: Vec<dtx_input::midi::MidiEvent> = Vec::new();
        (*source).poll(&mut buf);
        let gameplay_ready = !chart.chart.chips.is_empty() && clock.is_ready();
        let consumed =
            consume_midi_events(buf, &resolver, gameplay_ready, clock.current_ms, &mut last);
        for hit in consumed.hits {
            hits.write(hit);
        }
        for lane in consumed.nav_lanes {
            nav_hits.write(PadNavHit { lane });
        }
    }

    struct ConsumedMidi {
        hits: Vec<LaneHit>,
        /// Lanes for `PadNavHit`; emitted even when gameplay is not ready so
        /// pads can steer menus outside a run.
        nav_lanes: Vec<u8>,
    }

    fn consume_midi_events(
        events: impl IntoIterator<Item = dtx_input::midi::MidiEvent>,
        resolver: &crate::bindings::BindResolver,
        gameplay_ready: bool,
        clock_ms: i64,
        last: &mut LastMidiHit,
    ) -> ConsumedMidi {
        let mut hits = Vec::new();
        let mut nav_lanes = Vec::new();
        for ev in events {
            let dtx_input::midi::MidiEvent::NoteOn {
                note,
                velocity,
                audio_ms,
            } = ev
            else {
                continue;
            };
            *last = LastMidiHit {
                note,
                velocity,
                below_threshold: velocity <= resolver.velocity_threshold,
                at: Some(std::time::Instant::now()),
            };
            if velocity == 0 || velocity <= resolver.velocity_threshold {
                continue;
            }
            for lane in resolver.lanes_for_note(note) {
                nav_lanes.push(lane);
                if gameplay_ready {
                    hits.push(LaneHit {
                        lane,
                        audio_ms: stamp_audio_ms(Some(clock_ms), audio_ms),
                    });
                }
            }
        }
        ConsumedMidi { hits, nav_lanes }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn midi_updates_last_hit_without_gameplay_readiness() {
            let resolver = crate::bindings::BindResolver::default();
            let mut last = LastMidiHit::default();

            let hits = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 38,
                    velocity: 90,
                    audio_ms: 0,
                }],
                &resolver,
                false,
                0,
                &mut last,
            );

            assert_eq!((last.note, last.velocity), (38, 90));
            assert!(last.at.is_some());
            assert!(hits.hits.is_empty());
            assert_eq!(hits.nav_lanes.len(), 1);
        }

        #[test]
        fn shared_note_emits_one_hit_per_owning_lane() {
            use dtx_input::{BindSource, InputBindings};
            let mut b = InputBindings::default();
            b.bind_shared(dtx_core::EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
            let resolver = crate::bindings::BindResolver::from_bindings(&b);
            let mut last = LastMidiHit::default();
            let out = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 36,
                    velocity: 100,
                    audio_ms: 10,
                }],
                &resolver,
                true,
                0,
                &mut last,
            );
            assert_eq!(out.hits.len(), 2, "BD and LBD both hit");
            assert_eq!(out.nav_lanes.len(), 2);
        }

        #[test]
        fn gated_midi_event_is_not_replayed_when_gameplay_becomes_ready() {
            let resolver = crate::bindings::BindResolver::default();
            let mut last = LastMidiHit::default();

            let gated = consume_midi_events(
                [dtx_input::midi::MidiEvent::NoteOn {
                    note: 38,
                    velocity: 90,
                    audio_ms: 0,
                }],
                &resolver,
                false,
                0,
                &mut last,
            );
            let next = consume_midi_events([], &resolver, true, 1234, &mut last);

            assert!(gated.hits.is_empty());
            assert!(next.hits.is_empty());
        }
    }
}

pub use midi_consumer::{LastMidiHit, PadNavHit};

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
        use super::midi_consumer::stamp_audio_ms;
        assert_eq!(stamp_audio_ms(None, 123), 123);
        assert_eq!(stamp_audio_ms(None, 0), 0);
        assert_eq!(stamp_audio_ms(Some(5000), 0), 5000);
        assert_eq!(stamp_audio_ms(Some(5000), 123), 123);
    }
}
