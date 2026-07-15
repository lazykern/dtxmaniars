//! Keyboard + MIDI input sources (Engine layer).
//!
//! Per ADR-0009 this crate was deferred from M2 to M6. M6c extracts
//! `gameplay-drums/src/input.rs` here and adds a MIDI source abstraction.
//!
//! ## Module map
//!
//! - [`events`] — `LaneHit`, `LaneHitKind` (moved here from gameplay-drums)
//! - [`keyboard`] — keyboard → LaneHit system (BEVY_SYSTEM)
//! - [`midi`] — `MidiSource` trait + `VirtualSource` + (optional) real-device impl
//!
//! ## LaneId is opaque
//!
//! `LaneHit::lane` is just a `u8`. Sources here emit raw events (keys,
//! `MidiEvent`s); resolving them to lanes is the consuming gameplay crate's
//! job (drums does it via this crate's `InputBindings` → `BindResolver`).
//! dtx-input only knows "key pressed on lane X at audio_ms Y".

#![warn(missing_docs)]

use bevy::prelude::*;

// Ported from dtx-config (which did not require item docs). Keep them exempt
// from this crate's `#![warn(missing_docs)]` rather than back-fill ~76 docs.
#[allow(missing_docs)]
pub mod bindings;
pub mod events;
pub mod keyboard;
pub mod lane_map;
pub mod midi;
pub mod pad;
#[allow(missing_docs)]
pub mod profiles;
pub mod pump;
pub mod resolver;

pub use events::{LaneHit, LaneHitKind, LaneId};
pub use pump::{
    InputPumpSet, LastMidiHit, MidiConnected, PadNavHit, RawInputOwned, ResolvedInputHit,
    SystemVerbHit,
};
pub use resolver::{ActiveInputProfiles, BindResolver, LiveBindings};

/// Key/MIDI binding schema (moved here from dtx-config: it serializes bevy's
/// `KeyCode`, so it belongs in the Engine layer, not Pure config).
pub use bindings::{
    default_bindings_path, lane_owner, load_bindings, save_bindings, BindSource, BindingsFile,
    InputBindings, MidiDeviceConfig, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS,
};

/// Re-export so binding/profile code (and external callers) name `KeyCode`
/// without a direct bevy dependency.
pub use bevy::input::keyboard::KeyCode;

/// Plugin assembly: registers LaneHit message. The keyboard-to-LaneHit
/// system lives in each gameplay crate (which owns the concrete LaneMap).
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LaneHit>();
    }
}

/// Re-export for callers that prefer `add_plugins(...)` syntax.
pub use InputPlugin as DtxInputPlugin;
