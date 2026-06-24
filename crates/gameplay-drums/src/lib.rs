//! Drums gameplay vertical slice (M2).
//!
//! Game layer. Owns the per-frame loop:
//!   input → LaneHit → judge vs AudioClock → JudgmentEvent → score/combo → HUD
//!
//! Wires together: `dtx-core` (chart), `dtx-scoring` (judgment classify),
//! `dtx-timing` (audio clock), `dtx-audio` (BGM).
//!
//! v1 minimal — no skins, no animations, just colored note entities + text score.
//! Visual richness lands in M3 alongside `game-shell` (osu-style fades).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*`
//! Lane order: LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO (BocuD CActPerfDrumsLaneFlushD.cs).

pub mod components;
pub mod events;
pub mod hud;
pub mod input;
pub mod judge;
pub mod lane_map;
pub mod miss;
pub mod resources;
pub mod score;
pub mod scroll;

use bevy::prelude::*;
use bevy::prelude::{Component as _, Message as _, Resource as _};

/// Root plugin: register all sub-plugins in dependency order.
///
/// Usage:
/// ```ignore
/// app.add_plugins((
///     dtx_audio::plugin,
///     dtx_timing::plugin,
///     gameplay_drums::plugin,
/// ));
/// ```
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
        .add_plugins((
            input::plugin,
            scroll::plugin,
            judge::plugin,
            score::plugin,
            miss::plugin,
            hud::plugin,
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
        (&mut *source).poll(&mut buf);
        for h in buf {
            // Convert dtx_input::LaneHit to gameplay-drums LaneHit.
            hits.write(LaneHit {
                lane: h.lane,
                audio_ms: h.audio_ms,
            });
        }
    }
}

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as DrumsPlugin;
