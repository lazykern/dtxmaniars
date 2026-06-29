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
pub mod bgm_scheduler;
pub mod components;
pub mod damage_level;
pub mod drum_groups;
pub mod drums_perf;
pub mod events;
pub mod gauge;
pub mod hit_sound;
pub mod hud;
pub mod hud_cache;
pub mod input;
pub mod judge;
pub mod keyboard_viz;
pub mod lane_map;
pub mod layout;
pub mod miss;
pub mod orchestrator;
pub mod perf_common;
pub mod playfield_viz;
pub mod resources;
pub mod score;
pub mod scroll;
pub mod se_scheduler;
pub mod sound_bank;

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
    .init_resource::<resources::Score>()
    .init_resource::<resources::Combo>()
    .init_resource::<resources::GameStartMs>()
    .init_resource::<resources::JudgmentCounts>()
    .init_resource::<resources::ScrollSettings>()
    .init_resource::<resources::GameplayClock>()
    .init_resource::<resources::DrumGameplaySettings>()
    .init_resource::<resources::DrumAudioSettings>()
    .init_resource::<dtx_audio::DrumPolyphony>()
    .init_resource::<lane_map::LaneMap>()
    .init_resource::<hud_cache::HudDisplayCache>()
    .init_resource::<dtx_input::midi::VirtualSource>()
    .add_systems(Startup, (load_scroll_settings, load_drum_audio_settings))
    .add_message::<events::LaneHit>()
    .add_message::<events::JudgmentEvent>()
    .add_message::<events::NoteMissed>()
    .add_message::<events::EmptyHit>()
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
            .run_if(in_state(game_shell::AppState::Performance)),
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
        keyboard_viz::plugin,
        playfield_viz::plugin,
        orchestrator::plugin,
        autoplay::plugin,
        hit_sound::plugin,
        bgm_scheduler::plugin,
    ))
    .add_plugins((se_scheduler::plugin, midi_consumer::plugin));
}

fn load_scroll_settings(mut settings: ResMut<resources::ScrollSettings>) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    *settings = resources::ScrollSettings::from_scroll_speed(cfg.gameplay.scroll_speed);
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
    mut gameplay_clock: ResMut<resources::GameplayClock>,
) {
    gameplay_clock.sync(audio_clock.current_ms);
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
        let mut buf: Vec<dtx_input::LaneHit> = Vec::new();
        (*source).poll(&mut buf);
        for h in buf {
            hits.write(LaneHit {
                lane: h.lane,
                audio_ms: if h.audio_ms != 0 {
                    h.audio_ms
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
