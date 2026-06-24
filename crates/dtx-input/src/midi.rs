//! MIDI input sources.
//!
//! `MidiSource` is the trait; `VirtualSource` is the in-memory test double.
//! Real-device impl via `midir` is gated on the `midi` feature.

use crate::events::{LaneHit, LaneHitKind};
use crate::mapping::midi_note_to_drum_lane;
use std::collections::VecDeque;

/// A source of MIDI events. Implementations may be real (via midir) or
/// virtual (for tests).
pub trait MidiSource: Send {
    /// Drain pending events into `out`. Returns the number of events pushed.
    fn poll(&mut self, out: &mut Vec<LaneHit>) -> usize;

    /// True if the source has data available without consuming it.
    fn has_events(&self) -> bool;
}

/// In-memory MIDI event source. Used by tests (no real device required).
#[derive(Debug, Default, Clone, bevy::prelude::Resource)]
pub struct VirtualSource {
    events: VecDeque<MidiEvent>,
}

impl VirtualSource {
    /// Construct an empty source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a MIDI note-on event.
    pub fn note_on(&mut self, note: u8, velocity: u8, audio_ms: i64) {
        self.events.push_back(MidiEvent::NoteOn {
            note,
            velocity,
            audio_ms,
        });
    }

    /// Push a MIDI note-off event.
    pub fn note_off(&mut self, note: u8, audio_ms: i64) {
        self.events.push_back(MidiEvent::NoteOff { note, audio_ms });
    }

    /// Push a raw event (mostly for tests that want non-note data).
    pub fn push(&mut self, ev: MidiEvent) {
        self.events.push_back(ev);
    }

    /// Total queued events (read + unread).
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True if no queued events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl MidiSource for VirtualSource {
    fn poll(&mut self, out: &mut Vec<LaneHit>) -> usize {
        let before = out.len();
        while let Some(ev) = self.events.pop_front() {
            if let Some(hit) = ev.to_lane_hit() {
                out.push(hit);
            }
        }
        out.len() - before
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Raw MIDI event. NoteOn/NoteOff map to LaneHit; other kinds are dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiEvent {
    /// Note on (channel implicit, velocity 1..127 for audible).
    NoteOn {
        /// MIDI note number (0..127).
        note: u8,
        /// Velocity (0 = silent, 127 = loudest).
        velocity: u8,
        /// AudioClock ms when event occurred.
        audio_ms: i64,
    },
    /// Note off.
    NoteOff {
        /// MIDI note number.
        note: u8,
        /// AudioClock ms when event occurred.
        audio_ms: i64,
    },
    /// Control change (ignored in M6c; landed for M6.1 if needed).
    ControlChange {
        /// CC number.
        controller: u8,
        /// CC value (0..127).
        value: u8,
        /// AudioClock ms.
        audio_ms: i64,
    },
}

impl MidiEvent {
    /// Convert to a LaneHit using the default drum mapping. Returns None if
    /// the event doesn't map to a drum lane.
    pub fn to_lane_hit(self) -> Option<LaneHit> {
        match self {
            MidiEvent::NoteOn { note, audio_ms, .. } => {
                let lane = midi_note_to_drum_lane(note)?;
                Some(LaneHit::press(lane, audio_ms))
            }
            MidiEvent::NoteOff { note, audio_ms } => {
                let lane = midi_note_to_drum_lane(note)?;
                Some(LaneHit {
                    lane,
                    audio_ms,
                    kind: LaneHitKind::Release,
                })
            }
            MidiEvent::ControlChange { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_source_starts_empty() {
        let s = VirtualSource::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn note_on_bd_emits_lane_hit_press() {
        let mut s = VirtualSource::new();
        s.note_on(midi_notes_check::BD, 100, 500);
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].lane, 2);
        assert_eq!(out[0].audio_ms, 500);
        assert_eq!(out[0].kind, LaneHitKind::Press);
    }

    #[test]
    fn note_off_emits_release() {
        let mut s = VirtualSource::new();
        s.note_off(midi_notes_check::BD, 700);
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].kind, LaneHitKind::Release);
    }

    #[test]
    fn unknown_note_filtered_out() {
        let mut s = VirtualSource::new();
        s.note_on(50, 100, 0); // tom — unmapped
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn control_change_filtered_out() {
        let mut s = VirtualSource::new();
        s.push(MidiEvent::ControlChange {
            controller: 7,
            value: 100,
            audio_ms: 0,
        });
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 0);
    }

    #[test]
    fn poll_drains_queue() {
        let mut s = VirtualSource::new();
        s.note_on(36, 100, 0);
        s.note_on(38, 100, 1);
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 2);
        assert!(s.is_empty());
    }

    // Local alias to keep test imports readable.
    mod midi_notes_check {
        pub use crate::mapping::midi_notes::*;
    }
}
