//! Drums gameplay vertical slice.
//!
//! Game layer. Owns the per-frame loop:
//!   input → LaneHit → judge vs AudioClock → JudgmentEvent → score/combo
//!
//! Wires together: `dtx-core` (chart), `dtx-scoring` (judgment classify),
//! `dtx-timing` (audio clock), `dtx-audio` (BGM).
//!
//! v1 mechanics-only — no UI/skin, no commands.spawn. Visual layer lives
//! in the osu-style HUD crate (separate). The gameplay loop and state
//! machines (DrumsPad, DrumsDanger, DrumsFillingEffect) are kept here.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*`
//! Lane order: LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO (BocuD CActPerfDrumsLaneFlushD.cs).

pub mod autoplay;
pub mod components;
pub mod damage_level;
pub mod drums_perf;
pub mod events;
pub mod input;
pub mod judge;
pub mod lane_map;
pub mod miss;
pub mod orchestrator;
pub mod perf_common;
pub mod resources;
pub mod score;
pub mod scroll;

use bevy::prelude::*;

/// Root plugin: register all sub-plugins in dependency order.
pub fn plugin(app: &mut App) {
    app.init_resource::<resources::ActiveChart>()
        .init_resource::<resources::Score>()
        .init_resource::<resources::Combo>()
        .init_resource::<resources::GameStartMs>()
        .init_resource::<resources::JudgmentCounts>()
        .init_resource::<lane_map::LaneMap>()
        .init_resource::<dtx_input::midi::VirtualSource>()
        .add_message::<events::LaneHit>()
        .add_message::<events::JudgmentEvent>()
        .add_message::<events::NoteMissed>()
        .init_resource::<perf_common::PerformanceStageState>()
        .add_plugins((
            input::plugin,
            scroll::plugin,
            judge::plugin,
            score::plugin,
            miss::plugin,
            autoplay::plugin,
            midi_consumer::plugin,
        ));
}

mod midi_consumer {
    //! Polls `dtx_input::midi::VirtualSource` and emits gameplay-drums `LaneHit`s.
    //!
    //! M6c: gameplay-drums consumes dtx_input::MidiSource. The default impl is
    //! `VirtualSource` (no real MIDI device required). Real `midir` integration
    //! is gated on the `midi` feature in dtx-input.

    use bevy::prelude::*;
    use dtx_input::midi::{MidiSource, VirtualSource};

    use super::events::LaneHit;
    use crate::resources::ActiveChart;

    pub(super) fn plugin(app: &mut App) {
        app.add_systems(Update, poll_midi);
    }

    fn poll_midi(
        mut source: ResMut<VirtualSource>,
        chart: Res<ActiveChart>,
        mut hits: MessageWriter<LaneHit>,
    ) {
        // Only consume MIDI events when a chart is loaded (avoid noise in menus).
        if source.is_empty() {
            return;
        }
        if chart.chart.chips.is_empty() {
            return;
        }
        let mut buf: Vec<dtx_input::LaneHit> = Vec::new();
        (*source).poll(&mut buf);
        for h in buf {
            hits.write(LaneHit {
                lane: h.lane,
                audio_ms: h.audio_ms,
            });
        }
    }
}

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as DrumsPlugin;
