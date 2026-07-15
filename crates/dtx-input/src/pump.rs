//! MIDI device pump: connection, drain, velocity filter, and resolution into
//! device-level messages (`PadNavHit`, `ResolvedInputHit`, `SystemVerbHit`).
//!
//! Moved from gameplay-drums `midi_consumer` (menu-nav extraction,
//! 2026-07-15 spec). The consuming game crate adds [`plugin`] and orders
//! [`InputPumpSet`] against its own input sets; gameplay readiness gating is
//! the consumer's job, not the pump's.

use std::time::Instant;

use bevy::prelude::*;
#[cfg(feature = "midi")]
use bevy::time::common_conditions::on_real_timer;

use crate::events::LaneId;
use crate::midi::{MidiSource, VirtualSource};
use crate::resolver::BindResolver;
#[cfg(feature = "midi")]
use crate::resolver::LiveBindings;
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

/// True while a binding-capture surface owns raw input exclusively.
/// Written by the surface that owns it (the gameplay-drums editor); read by
/// the keyboard system-verb translator, which emits nothing while set. The
/// MIDI pump deliberately does NOT check this: `LastMidiHit` must keep
/// updating during capture (note capture reads it), and pad-nav/calibration
/// suppression happens at the context level instead.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RawInputOwned(pub bool);

/// FixedUpdate set the pump systems run in. Consumers order their input sets
/// after this (`configure_sets(FixedUpdate, InputPumpSet.before(MyInputSet))`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputPumpSet;

/// Holds the live real-MIDI connection. Stored as a **non-send** resource
/// because `midir` connections are not `Sync`; systems touching it run on
/// the main thread only.
#[cfg(feature = "midi")]
#[derive(Default)]
struct MidiConnection {
    source: Option<crate::midi::RealMidiSource>,
    port_filter: Option<String>,
}

/// Registers the pump. Deliberately a bare `fn` plugin: the consuming game
/// crate adds it and orders [`InputPumpSet`] against its own input sets.
/// The caller must also provide the [`BindResolver`] and [`LiveBindings`]
/// resources (gameplay-drums registers them via its `bindings` plugin) — the
/// pump reads them but does not own their lifecycle.
pub fn plugin(app: &mut App) {
    app.init_resource::<LastMidiHit>()
        .init_resource::<MidiConnected>()
        .init_resource::<RawInputOwned>()
        .init_resource::<VirtualSource>()
        .add_message::<PadNavHit>()
        .add_message::<ResolvedInputHit>()
        .add_message::<SystemVerbHit>()
        .add_systems(FixedUpdate, poll_midi.in_set(InputPumpSet));

    #[cfg(feature = "midi")]
    {
        app.insert_non_send(MidiConnection::default())
            .add_systems(Startup, connect_midi)
            .add_systems(
                Update,
                connect_midi.run_if(
                    resource_changed::<LiveBindings>
                        .or_else(on_real_timer(std::time::Duration::from_secs(1))),
                ),
            )
            .add_systems(
                FixedUpdate,
                drain_real_midi.in_set(InputPumpSet).before(poll_midi),
            );
    }
}

/// Connect (or reconnect) the real MIDI source using the port filter from
/// `LiveBindings`. Runs at startup, whenever the selected port changes,
/// and once per second so devices plugged in after boot are discovered.
/// Reconnect overwrites, dropping the old
/// connection. Non-send: runs on the main thread only.
#[cfg(feature = "midi")]
fn connect_midi(
    mut conn: NonSendMut<MidiConnection>,
    live: Res<LiveBindings>,
    mut connected: ResMut<MidiConnected>,
) {
    let filter = live.0.midi.port.clone();
    if conn.source.is_some() && conn.port_filter == filter {
        return;
    }
    match crate::midi::RealMidiSource::connect(filter.as_deref()) {
        Ok((src, name)) => {
            info!("MIDI connected: {name}");
            conn.source = Some(src);
            conn.port_filter = filter;
            connected.0 = true;
        }
        Err(e) => {
            warn!("MIDI connect failed: {e}");
            conn.source = None;
            conn.port_filter = filter;
            connected.0 = false;
        }
    }
}

/// Drain the real MIDI source into `VirtualSource` so real events are
/// indistinguishable from virtual ones downstream. Real events carry
/// `audio_ms == 0`; the consumer restamps them with its own clock.
/// Non-send: runs on the main thread only.
#[cfg(feature = "midi")]
fn drain_real_midi(mut conn: NonSendMut<MidiConnection>, mut virt: ResMut<VirtualSource>) {
    let Some(src) = conn.source.as_mut() else {
        return;
    };
    let mut buf: Vec<crate::midi::MidiEvent> = Vec::new();
    src.poll(&mut buf);
    for ev in buf {
        virt.push(ev);
    }
}

fn poll_midi(
    mut source: ResMut<VirtualSource>,
    resolver: Res<BindResolver>,
    mut hits: MessageWriter<ResolvedInputHit>,
    mut nav_hits: MessageWriter<PadNavHit>,
    mut verb_hits: MessageWriter<SystemVerbHit>,
    mut last: ResMut<LastMidiHit>,
) {
    if source.is_empty() {
        return;
    }
    let mut buf: Vec<crate::midi::MidiEvent> = Vec::new();
    (*source).poll(&mut buf);
    let consumed = consume_midi_events(buf, &resolver, &mut last);
    for hit in consumed.hits {
        hits.write(hit);
    }
    for lane in consumed.nav_lanes {
        nav_hits.write(PadNavHit { lane });
    }
    for verb in consumed.verbs {
        verb_hits.write(SystemVerbHit { verb });
    }
}

struct ConsumedMidi {
    hits: Vec<ResolvedInputHit>,
    /// Lanes for `PadNavHit`; emitted even when gameplay is not ready so
    /// pads can steer menus outside a run.
    nav_lanes: Vec<LaneId>,
    /// System verbs fired by this batch. Emitted on the same unconditional
    /// path as `nav_lanes` — the verb must work mid-song.
    verbs: Vec<SystemVerb>,
}

fn consume_midi_events(
    events: impl IntoIterator<Item = crate::midi::MidiEvent>,
    resolver: &BindResolver,
    last: &mut LastMidiHit,
) -> ConsumedMidi {
    let mut hits = Vec::new();
    let mut nav_lanes = Vec::new();
    let mut verbs = Vec::new();
    for ev in events {
        let crate::midi::MidiEvent::NoteOn {
            note,
            velocity,
            audio_ms,
            captured_at,
        } = ev
        else {
            continue;
        };
        *last = LastMidiHit {
            note,
            velocity,
            below_threshold: velocity <= resolver.velocity_threshold,
            at: Some(Instant::now()),
        };
        if velocity == 0 || velocity <= resolver.velocity_threshold {
            continue;
        }
        // Verbs fire before any gameplay gate: they must work mid-song, and a
        // system note was never gameplay input.
        verbs.extend(resolver.system_for_note(note));
        let lanes: Vec<_> = resolver.lanes_for_note(note).collect();
        if let Some(&lane) = lanes.first() {
            nav_lanes.push(lane);
            hits.push(ResolvedInputHit {
                lanes,
                audio_ms,
                captured_at,
            });
        }
    }
    ConsumedMidi {
        hits,
        nav_lanes,
        verbs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_updates_last_hit_and_always_resolves() {
        let resolver = BindResolver::default();
        let mut last = LastMidiHit::default();

        let hits = consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 38,
                velocity: 90,
                audio_ms: 0,
                captured_at: Instant::now(),
            }],
            &resolver,
            &mut last,
        );

        assert_eq!((last.note, last.velocity), (38, 90));
        assert!(last.at.is_some());
        assert_eq!(hits.hits.len(), 1);
        assert_eq!(hits.nav_lanes.len(), 1);
    }

    #[test]
    fn shared_note_emits_one_atomic_hit() {
        use crate::{BindSource, InputBindings};
        let mut b = InputBindings::default();
        b.bind_shared(
            dtx_core::EChannel::LeftBassDrum,
            BindSource::Midi { note: 36 },
        );
        let resolver = BindResolver::from_bindings(&b);
        let mut last = LastMidiHit::default();
        let captured_at = Instant::now();
        let out = consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 36,
                velocity: 100,
                audio_ms: 10,
                captured_at,
            }],
            &resolver,
            &mut last,
        );
        assert_eq!(out.hits.len(), 1, "one physical MIDI note is atomic");
        assert_eq!(out.hits[0].lanes, vec![2, 11]);
        assert_eq!(out.hits[0].captured_at, captured_at);
        assert_eq!(out.hits[0].audio_ms, 10);
        assert_eq!(out.nav_lanes, vec![2]);
    }

    #[test]
    fn system_verb_fires_and_never_resolves_a_lane() {
        use crate::{BindSource, InputBindings, SystemVerb};
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        let resolver = BindResolver::from_bindings(&b);
        let mut last = LastMidiHit::default();

        let out = consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 37,
                velocity: 90,
                audio_ms: 0,
                captured_at: Instant::now(),
            }],
            &resolver,
            &mut last,
        );

        assert_eq!(out.verbs, vec![SystemVerb::Pause]);
        assert!(out.hits.is_empty());
        assert!(out.nav_lanes.is_empty(), "a system note is not a lane");
    }

    #[test]
    fn sub_threshold_system_note_emits_nothing() {
        use crate::{BindSource, InputBindings, SystemVerb};
        let mut b = InputBindings::default();
        b.midi.velocity_threshold = 20;
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        let resolver = BindResolver::from_bindings(&b);
        let mut last = LastMidiHit::default();

        let out = consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 37,
                velocity: 15,
                audio_ms: 0,
                captured_at: Instant::now(),
            }],
            &resolver,
            &mut last,
        );

        assert!(out.verbs.is_empty(), "noise must not pause the song");
    }

    #[test]
    fn a_lane_note_never_emits_a_system_verb() {
        use crate::{BindSource, InputBindings, SystemVerb};
        let mut b = InputBindings::default();
        // The footgun: 38 is the Snare's note, also hand-bound to Pause.
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 38 });
        let resolver = BindResolver::from_bindings(&b);
        let mut last = LastMidiHit::default();

        let out = consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 38,
                velocity: 90,
                audio_ms: 0,
                captured_at: Instant::now(),
            }],
            &resolver,
            &mut last,
        );

        assert!(out.verbs.is_empty(), "a lane hit must never pause");
        assert_eq!(out.hits.len(), 1, "it still judges");
    }

    #[test]
    fn last_midi_hit_updates_regardless_of_raw_input_owned() {
        // RawInputOwned gates the keyboard verb translator only. The pump has no
        // such parameter at all — this test documents that on purpose: note
        // capture reads LastMidiHit, so the pump must never go quiet during
        // capture. consume_midi_events' signature is the proof.
        let resolver = crate::resolver::BindResolver::default();
        let mut last = LastMidiHit::default();
        consume_midi_events(
            [crate::midi::MidiEvent::NoteOn {
                note: 38,
                velocity: 90,
                audio_ms: 0,
                captured_at: Instant::now(),
            }],
            &resolver,
            &mut last,
        );
        assert_eq!((last.note, last.velocity), (38, 90));
    }
}
