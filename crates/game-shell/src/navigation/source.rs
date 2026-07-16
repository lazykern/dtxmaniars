//! Last-intentional input source tracking and prompt-source preference.

use bevy::prelude::*;

/// Which device produced an intentional action. Replaces `NavSource` once the
/// router migration completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    /// Physical keyboard.
    Keyboard,
    /// Mouse click, wheel, or drag (never bare pointer motion).
    Mouse,
    /// MIDI kit pad/zone.
    MidiKit,
    /// Gamepad (reserved; no producer yet).
    Gamepad,
}

/// Last accepted intentional input source. Plain resource: survives every
/// AppState transition. Pointer motion must never write it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastIntentionalInputSource(pub InputSource);

impl Default for LastIntentionalInputSource {
    fn default() -> Self {
        Self(InputSource::Keyboard)
    }
}

/// Screens report intentional mouse interactions (click/wheel/drag).
#[derive(Message, Debug, Clone, Copy)]
pub struct MouseIntent;

/// Accessibility: lock prompts to one source, or follow the last one.
/// Persistence lands with the Settings draft; a plain resource until then.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptSourcePreference {
    /// Prompts follow [`LastIntentionalInputSource`].
    #[default]
    Automatic,
    /// Prompts always render for this source.
    Always(InputSource),
}
