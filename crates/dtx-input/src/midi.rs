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

/// Parse a raw MIDI message into a note event. `audio_ms` stamps it.
/// Returns `None` for non-note messages or short (< 3 byte) messages.
pub fn midi_bytes_to_event(bytes: &[u8], audio_ms: i64) -> Option<MidiEvent> {
    if bytes.len() < 3 {
        return None;
    }
    match bytes[0] & 0xF0 {
        0x90 if bytes[2] > 0 => Some(MidiEvent::NoteOn {
            note: bytes[1],
            velocity: bytes[2],
            audio_ms,
        }),
        0x90 => Some(MidiEvent::NoteOff {
            note: bytes[1],
            audio_ms,
        }),
        0x80 => Some(MidiEvent::NoteOff {
            note: bytes[1],
            audio_ms,
        }),
        _ => None,
    }
}

/// Enumerate available MIDI input port names.
#[cfg(feature = "midi")]
pub fn available_ports() -> Vec<String> {
    let Ok(mi) = midir::MidiInput::new("dtxmaniars-scan") else {
        return vec![];
    };
    mi.ports()
        .iter()
        .filter_map(|p| mi.port_name(p).ok())
        .collect()
}

/// Enumerate available MIDI input port names (no-op without the `midi` feature).
#[cfg(not(feature = "midi"))]
pub fn available_ports() -> Vec<String> {
    vec![]
}

/// Real MIDI input source backed by `midir`. The connection callback runs on
/// midir's own OS thread and pushes parsed events into a shared inbox; `poll`
/// drains that inbox on the consumer's thread.
#[cfg(feature = "midi")]
pub struct RealMidiSource {
    _conn: midir::MidiInputConnection<()>,
    inbox: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<MidiEvent>>>,
}

#[cfg(feature = "midi")]
impl RealMidiSource {
    /// Connect to the first port whose name contains `port_filter` (or the
    /// first port if `None`). Returns the source plus the connected port name.
    /// All errors are mapped to `String`.
    pub fn connect(port_filter: Option<&str>) -> Result<(Self, String), String> {
        let mut mi = midir::MidiInput::new("dtxmaniars").map_err(|e| e.to_string())?;
        mi.ignore(midir::Ignore::None);
        let ports = mi.ports();
        let port = ports
            .iter()
            .find(|p| match (port_filter, mi.port_name(p)) {
                (Some(f), Ok(n)) => n.contains(f),
                (None, _) => true,
                _ => false,
            })
            .cloned()
            .ok_or_else(|| "no matching MIDI port".to_string())?;
        let name = mi.port_name(&port).map_err(|e| e.to_string())?;
        let inbox = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        let cb_inbox = inbox.clone();
        let conn = mi
            .connect(
                &port,
                "dtx-in",
                move |_ts, bytes, _| {
                    // audio_ms is stamped in poll_midi on drain, not here.
                    if let Some(ev) = midi_bytes_to_event(bytes, 0) {
                        if let Ok(mut q) = cb_inbox.lock() {
                            q.push_back(ev);
                        }
                    }
                },
                (),
            )
            .map_err(|e| e.to_string())?;
        Ok((Self { _conn: conn, inbox }, name))
    }
}

#[cfg(feature = "midi")]
impl MidiSource for RealMidiSource {
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize {
        let mut n = 0;
        if let Ok(mut q) = self.inbox.lock() {
            while let Some(e) = q.pop_front() {
                out.push(e);
                n += 1;
            }
        }
        n
    }

    fn has_events(&self) -> bool {
        self.inbox.lock().map(|q| !q.is_empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_bytes_parse() {
        let e = midi_bytes_to_event(&[0x90, 38, 100], 0);
        assert_eq!(
            e,
            Some(MidiEvent::NoteOn {
                note: 38,
                velocity: 100,
                audio_ms: 0
            })
        );
    }

    #[test]
    fn note_on_velocity_zero_is_note_off() {
        let e = midi_bytes_to_event(&[0x90, 38, 0], 5);
        assert_eq!(
            e,
            Some(MidiEvent::NoteOff {
                note: 38,
                audio_ms: 5
            })
        );
    }

    #[test]
    fn note_off_bytes_parse() {
        assert_eq!(
            midi_bytes_to_event(&[0x80, 40, 64], 0),
            Some(MidiEvent::NoteOff {
                note: 40,
                audio_ms: 0
            })
        );
    }

    #[test]
    fn non_note_bytes_ignored() {
        assert_eq!(midi_bytes_to_event(&[0xB0, 4, 127], 0), None);
    }

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
