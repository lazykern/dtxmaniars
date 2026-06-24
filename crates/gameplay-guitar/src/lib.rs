//! Guitar mode vertical slice (M6b).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`
//!
//! 5-lane standard (R/G/B/Y/P). M6b ships a full playable mode: scroll +
//! judge + score + input + HUD, gated on `EGameMode::Guitar`. Chord
//! judgment + hold notes land in M6.1.
//!
//! Layer: Game. Mirrors `gameplay-drums` so M6.1 can extract shared bits.

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
pub use resources::{ActiveChart, Combo, GameStartMs, JudgmentCounts, Score};

/// Plugin assembly. Mirrors `gameplay_drums::plugin` shape so M6.1 can
/// extract shared systems cleanly.
pub fn plugin(app: &mut App) {
    app.init_resource::<resources::ActiveChart>()
        .init_resource::<resources::Score>()
        .init_resource::<resources::Combo>()
        .init_resource::<resources::GameStartMs>()
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
