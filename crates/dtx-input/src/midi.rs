//! MIDI input sources.
//!
//! `MidiSource` is the trait; `VirtualSource` is the in-memory test double.
//! Real-device impl via `midir` is gated on the `midi` feature.

use std::collections::VecDeque;

/// A source of MIDI events. Implementations may be real (via midir) or
/// virtual (for tests). Note→channel mapping is the consumer's job
/// (see dtx-config `InputBindings`).
pub trait MidiSource: Send {
    /// Drain pending events into `out`. Returns the number of events pushed.
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize;

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
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize {
        let before = out.len();
        while let Some(ev) = self.events.pop_front() {
            out.push(ev);
        }
        out.len() - before
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Raw MIDI event. The consumer maps notes to lanes (see dtx-config
/// `InputBindings`); this type carries data verbatim.
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
    fn poll_drains_all_events_verbatim() {
        let mut s = VirtualSource::new();
        s.note_on(36, 100, 500);
        s.note_off(36, 700);
        s.push(MidiEvent::ControlChange {
            controller: 4,
            value: 90,
            audio_ms: 800,
        });
        let mut out = Vec::new();
        let n = s.poll(&mut out);
        assert_eq!(n, 3);
        assert!(s.is_empty());
        assert_eq!(
            out[0],
            MidiEvent::NoteOn {
                note: 36,
                velocity: 100,
                audio_ms: 500
            }
        );
        assert_eq!(
            out[1],
            MidiEvent::NoteOff {
                note: 36,
                audio_ms: 700
            }
        );
    }

    #[test]
    fn has_events_reflects_queue() {
        let mut s = VirtualSource::new();
        assert!(!s.has_events());
        s.note_on(38, 90, 0);
        assert!(s.has_events());
    }
}
