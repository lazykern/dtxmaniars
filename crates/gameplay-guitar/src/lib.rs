//! Guitar mode vertical slice.
//!
//! 5-lane standard (R/G/B/Y/P). Mechanics-only port: scroll + judge +
//! score + input. Visual layer (HUD) is in the osu-style HUD crate.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`
//!
//! Layer: Game. Mirrors `gameplay-drums`.

#![warn(missing_docs)]

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod guitar_perf;
pub mod hud;
pub mod input;
pub mod judge;
pub mod lane_map;
pub mod orchestrator;
pub mod resources;
pub mod score;
pub mod scroll;

pub use events::{JudgmentEvent, LaneHit, NoteMissed};
pub use lane_map::{lane_channel, lane_of, LaneId, LaneMap, GUITAR_LANES};
pub use resources::{ActiveChart, BgmAdjustState, Combo, GameStartMs, JudgmentCounts, Score};

/// Plugin assembly. Mirrors `gameplay_drums::plugin` shape.
pub fn plugin(app: &mut App) {
    app.init_resource::<resources::ActiveChart>()
        .init_resource::<resources::Score>()
        .init_resource::<resources::Combo>()
        .init_resource::<resources::GameStartMs>()
        .init_resource::<resources::BgmAdjustState>()
        .init_resource::<resources::JudgmentCounts>()
        .init_resource::<lane_map::LaneMap>()
        .add_message::<events::LaneHit>()
        .add_message::<events::JudgmentEvent>()
        .add_message::<events::NoteMissed>()
        .add_plugins((
            input::plugin,
            judge::plugin,
            score::plugin,
            scroll::plugin,
            hud::plugin,
        ));
}

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as GuitarPlugin;
