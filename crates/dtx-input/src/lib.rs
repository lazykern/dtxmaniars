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
//! job (drums does it via `dtx-config` `InputBindings` → `BindResolver`).
//! dtx-input only knows "key pressed on lane X at audio_ms Y".

#![warn(missing_docs)]

use bevy::prelude::*;

pub mod events;
pub mod keyboard;
pub mod midi;
pub mod pad;

pub use events::{LaneHit, LaneHitKind, LaneId};

/// Re-export for config crates that serialize key bindings without a direct
/// bevy dependency.
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
