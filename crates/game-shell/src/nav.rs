//! Semantic menu-navigation actions shared by all UI crates.
//!
//! Producers: per-screen keyboard systems and the gameplay-drums pad mapper.
//! Consumers: song select, title, pause menu, results, settings overlay.

use bevy::prelude::*;

/// What the input means, not what produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavVerb {
    /// Move focus up / previous. Pads: HH.
    Up,
    /// Move focus down / next. Pads: CY/RD.
    Down,
    /// Enter / select / apply. Pads: BD.
    Confirm,
    /// Back out / cancel. Pads: SD.
    Back,
    /// Decrement focused value (keyboard Left; pads reuse Up in adjust mode).
    Dec,
    /// Increment focused value (keyboard Right; pads reuse Down in adjust mode).
    Inc,
    /// Start practice mode (keyboard Shift+Enter; pads FT at difficulty level).
    Practice,
}

/// Which device produced the action. Consumers may branch on this: keyboard
/// keeps its flat navigation model, pads use the two-level model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSource {
    /// Physical keyboard.
    Keyboard,
    /// Drum pad / MIDI device.
    Pad,
}

/// One navigation action. Screens consume these instead of raw keys/pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct NavAction {
    /// Semantic meaning of the action.
    pub verb: NavVerb,
    /// Device that produced it.
    pub source: NavSource,
    /// Shift held (keyboard only) — consumers multiply steps by 10.
    pub coarse: bool,
}

/// True while a real MIDI device is connected. Written by gameplay-drums'
/// `connect_midi`; read by legend bars (hidden when false).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MidiConnected(pub bool);

/// Registers the `NavAction` message and `MidiConnected` resource.
pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>()
        .init_resource::<MidiConnected>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_action_is_copy_and_comparable() {
        let a = NavAction {
            verb: NavVerb::Up,
            source: NavSource::Pad,
            coarse: false,
        };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn midi_connected_defaults_false() {
        assert!(!MidiConnected::default().0);
    }
}
