//! MIDI device pump: shared device-level messages and resources.
//!
//! Moved from gameplay-drums `midi_consumer` (menu-nav extraction,
//! 2026-07-15 spec). The pump systems (connect/drain/poll) follow in the
//! same extraction; this module starts with the shared types.

use std::time::Instant;

use bevy::prelude::*;

use crate::events::LaneId;
use crate::SystemVerb;

/// A resolved hit from a real pad, for menu navigation only.
///
/// Separate from `LaneHit` on purpose: `LaneHit` is also written by autoplay
/// (which the Customize surface forces on) and by keyboard lane keys, and
/// neither should ever steer a menu.
#[derive(Debug, Clone, Copy, Message)]
pub struct PadNavHit {
    /// Lane id per `crate::lane_map::LANE_ORDER`.
    pub lane: LaneId,
}

/// A velocity-accepted MIDI hit resolved to lanes, before any gameplay gating.
///
/// The gameplay crate decides whether gameplay is ready and converts this to
/// its own judged input event with a clock restamp. Menus never read this —
/// they consume [`PadNavHit`].
#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct ResolvedInputHit {
    /// Primary lane followed by accepted alternates (atomic multi-target hit).
    pub lanes: Vec<LaneId>,
    /// The event's own stamp; 0 for real-device events (consumer restamps).
    pub audio_ms: i64,
    /// Monotonic wall-clock timestamp captured at the physical input.
    pub captured_at: Instant,
}

/// A bound system verb fired by a key or a pad. Emitted before any
/// gameplay-ready gate so it works during live play; consumers gate
/// themselves.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemVerbHit {
    /// The verb that fired.
    pub verb: SystemVerb,
}

/// Last MIDI NoteOn observed by the pump, written before the threshold gate.
/// Drives the bindings-tab velocity meter and MIDI note capture, avoiding a
/// second drain that would race the pump.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastMidiHit {
    /// Raw MIDI note number.
    pub note: u8,
    /// Raw NoteOn velocity.
    pub velocity: u8,
    /// True when the hit was at or below the profile's velocity threshold.
    pub below_threshold: bool,
    /// When the hit was observed; `None` until the first hit.
    pub at: Option<Instant>,
}

/// True while a real MIDI device is connected. Written by the pump's connect
/// system; read by legend bars (hidden when false).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MidiConnected(pub bool);

/// True while a capture/calibration surface owns raw input exclusively.
/// Written by the surface that owns it (the gameplay-drums editor); read by
/// the keyboard system-verb translator, which emits nothing while set. The
/// MIDI pump deliberately does NOT check this: `LastMidiHit` must keep
/// updating during capture (note capture reads it), and pad-nav suppression
/// happens at the context level instead.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RawInputOwned(pub bool);

/// FixedUpdate set the pump systems run in. Consumers order their input sets
/// after this (`configure_sets(FixedUpdate, InputPumpSet.before(MyInputSet))`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputPumpSet;
