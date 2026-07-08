# Real MIDI Device (Phase 3b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps.

**Goal:** Make MIDI input actually work in the app: a real `midir`-backed `MidiSource` (feature-gated), MIDI port enumeration, a device-box port dropdown + rescan, a velocity live-meter, and real MIDI note capture in the Bindings tab (completing the 3a keyboard-only capture).

**Architecture:** `dtx-input` already defines `MidiSource` (trait, `poll`/`has_events`) + `VirtualSource` (test double); the app's `poll_midi` (gameplay-drums) drains `VirtualSource` and emits `LaneHit`. Today nothing feeds real notes. Add a **feature-gated (`midi`)** `RealMidiSource`: a `midir::MidiInputConnection` whose callback (running on midir's own OS thread) pushes `MidiEvent`s into a shared `Arc<Mutex<VecDeque>>`; `poll_midi` drains that alongside `VirtualSource`. Port list via a `midir::MidiInput` scratch instance. The connection is stored as a **NonSend** Bevy resource (midir connections aren't `Sync`); the shared inbox is a normal `Send+Sync` resource. A `LastMidiHit` resource (velocity + note, written by the poll path before the threshold gate) drives both the meter and MIDI capture (capture reads `LastMidiHit`, avoiding a second drain that would race `poll_midi`).

**Tech Stack:** Rust, Bevy 0.19, `midir` (cross-platform MIDI, ALSA/CoreMIDI/WinMM). Crates: `dtx-input` (real source + feature), `gameplay-drums` (poll wiring, LastMidiHit, capture completion), `gameplay-drums/editor/bindings_panel.rs` + `bindings_capture.rs` (device box UI), `app/dtxmaniars-desktop` (enable `midi` feature).

**Spec:** design §5 (device box: MIDI port dropdown + rescan + velocity meter; capture listens keyboard AND MIDI, first wins).

**Investigation anchors:**
- `dtx-input/src/midi.rs`: `MidiSource` trait (`poll(&mut Vec<MidiEvent>)`, `has_events`), `VirtualSource` (Resource), `MidiEvent::{NoteOn{note,velocity,audio_ms}, NoteOff, ControlChange}`. Doc already says "Real-device impl via `midir` is gated on the `midi` feature."
- `gameplay-drums/src/lib.rs` `mod midi_consumer` `poll_midi` (~:309): drains `Res<VirtualSource>`, skips `vel==0 || vel<=resolver.velocity_threshold` (~:337), maps `resolver.lane_for_note`, emits `LaneHit`. **Velocity is read then discarded** — capture the last one here.
- `dtx-config/src/bindings.rs` `MidiDeviceConfig { port: Option<String>, velocity_threshold: u8 }` — `port` = substring filter (None = first). Persisted but currently unread.
- 3a: `crate::bindings::LiveBindings` (`.0.midi.port`, `.0.midi.velocity_threshold`), `editor/bindings_panel.rs` (DEVICE box — add dropdown + meter), `editor/bindings_capture.rs` `capture_binding` (has a `// TODO(3b): MIDI capture` hook).

**midir API (v0.10-ish, stable):**
```rust
use midir::{MidiInput, MidiInputConnection, Ignore};
let mut mi = MidiInput::new("dtxmaniars")?;   // MidiInit
mi.ignore(Ignore::None);
let ports = mi.ports();                        // Vec<MidiInputPort>
let name = mi.port_name(&ports[i])?;           // String
// connect CONSUMES `mi`; callback runs on a midir thread:
let conn: MidiInputConnection<()> = mi.connect(&port, "dtx-in",
    move |_ts_micros, bytes: &[u8], _data| { /* bytes = [status, d1, d2] */ }, ())?;
// keep `conn` alive; drop() disconnects. Status 0x90|ch = NoteOn (vel>0), 0x80|ch or 0x90 vel0 = NoteOff.
```

**Critical conventions:**
- NEVER `cargo fmt`/`--all`/`-p`. ONLY `rustfmt --edition <ed> <files>` (`dtx-input`/`gameplay-drums` = 2021; check `app/dtxmaniars-desktop` edition).
- Format-daemon reorders imports — checkout target files clean first; stage only intended files (never save.rs/selection_box.rs/undo.rs).
- Worktree `/home/lazykern/lab/dtxmaniars-customize` (branch `feat/customize-surface`).
- **The `midi` feature must be OFF by default** so `cargo test --workspace` (no hardware) stays green and CI doesn't need ALSA/CoreMIDI. The desktop app turns it ON. All midir code is `#[cfg(feature = "midi")]`; provide a no-op fallback so non-`midi` builds compile + behave as today (VirtualSource only).
- Real-device behavior is **NOT unit-testable headless** — verify via the user's e-drum kit (manual). Unit tests cover the byte→MidiEvent parse + velocity-capture + threshold, all with VirtualSource.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/dtx-input/Cargo.toml` | Modify | optional `midir` dep + `midi` feature |
| `crates/dtx-input/src/midi.rs` | Modify | `#[cfg(feature="midi")] RealMidiSource` + `midi_bytes_to_event` parse (always compiled + tested) + `available_ports()` |
| `crates/gameplay-drums/src/lib.rs` | Modify | `poll_midi` drains real source too; write `LastMidiHit`; connect/reconnect system |
| `crates/gameplay-drums/src/editor/bindings_panel.rs` | Modify | device box: port dropdown + rescan + velocity meter bar |
| `crates/gameplay-drums/src/editor/bindings_capture.rs` | Modify | complete MIDI capture via `LastMidiHit` |
| `app/dtxmaniars-desktop/Cargo.toml` | Modify | enable `dtx-input/midi` (via a desktop `midi` feature, default-on) |

---

### Task 1: `midi` feature + byte-parse + port enumeration

**Files:** `crates/dtx-input/Cargo.toml`, `crates/dtx-input/src/midi.rs`.

Context: The byte→`MidiEvent` parser and `available_ports()` are the always-testable core. `RealMidiSource` is feature-gated. Keep the parse fn NON-gated so it's unit-tested without hardware.

- [ ] **Step 1: Cargo** — add to `dtx-input/Cargo.toml`:
```toml
[features]
midi = ["dep:midir"]
[dependencies]
midir = { version = "0.10", optional = true }
```
(Check the workspace's dep-version convention; use `workspace = true` if there's a `[workspace.dependencies]` midir — there isn't, so pin `0.10`.)

- [ ] **Step 2: Parser test** (non-gated, into midi.rs `#[cfg(test)]`):
```rust
#[test]
fn note_on_bytes_parse() {
    let e = midi_bytes_to_event(&[0x90, 38, 100], 0);
    assert_eq!(e, Some(MidiEvent::NoteOn { note: 38, velocity: 100, audio_ms: 0 }));
}
#[test]
fn note_on_velocity_zero_is_note_off() {
    let e = midi_bytes_to_event(&[0x90, 38, 0], 5);
    assert_eq!(e, Some(MidiEvent::NoteOff { note: 38, audio_ms: 5 }));
}
#[test]
fn note_off_bytes_parse() {
    assert_eq!(midi_bytes_to_event(&[0x80, 40, 64], 0), Some(MidiEvent::NoteOff { note: 40, audio_ms: 0 }));
}
#[test]
fn non_note_bytes_ignored() {
    assert_eq!(midi_bytes_to_event(&[0xB0, 4, 127], 0), None); // CC — not a note (or map to ControlChange if you prefer)
}
```

- [ ] **Step 3: Implement parser** (non-gated):
```rust
/// Parse a raw MIDI message into a note event. `audio_ms` stamps it.
/// Returns None for non-note messages.
pub fn midi_bytes_to_event(bytes: &[u8], audio_ms: i64) -> Option<MidiEvent> {
    if bytes.len() < 3 { return None; }
    match bytes[0] & 0xF0 {
        0x90 if bytes[2] > 0 => Some(MidiEvent::NoteOn { note: bytes[1], velocity: bytes[2], audio_ms }),
        0x90 => Some(MidiEvent::NoteOff { note: bytes[1], audio_ms }), // vel 0 = note off
        0x80 => Some(MidiEvent::NoteOff { note: bytes[1], audio_ms }),
        _ => None,
    }
}
```

- [ ] **Step 4: `available_ports()`** (feature-gated; provide a non-`midi` fallback returning `vec![]`):
```rust
#[cfg(feature = "midi")]
pub fn available_ports() -> Vec<String> {
    let Ok(mi) = midir::MidiInput::new("dtxmaniars-scan") else { return vec![]; };
    mi.ports().iter().filter_map(|p| mi.port_name(p).ok()).collect()
}
#[cfg(not(feature = "midi"))]
pub fn available_ports() -> Vec<String> { vec![] }
```

- [ ] **Step 5:** `cargo test -p dtx-input` (default, no `midi`) → PASS (parser tests). `cargo build -p dtx-input --features midi` → compiles (needs ALSA headers on Linux; if the sandbox lacks them, note it — the default build is what CI runs). **Step 6: Commit** `feat(dtx-input): midi feature, byte parser, port enumeration`.

---

### Task 2: `RealMidiSource` (feature-gated)

**Files:** `crates/dtx-input/src/midi.rs`.

Context: Holds the connection + a shared inbox the callback fills; `poll()` drains the inbox.

- [ ] **Step 1: Implement** (all `#[cfg(feature = "midi")]`):
```rust
#[cfg(feature = "midi")]
pub struct RealMidiSource {
    _conn: midir::MidiInputConnection<()>,
    inbox: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<MidiEvent>>>,
}
#[cfg(feature = "midi")]
impl RealMidiSource {
    /// Connect to the first port whose name contains `port_filter` (or the
    /// first port if None). Returns the source + the connected port name.
    pub fn connect(port_filter: Option<&str>) -> Result<(Self, String), String> {
        let mut mi = midir::MidiInput::new("dtxmaniars").map_err(|e| e.to_string())?;
        mi.ignore(midir::Ignore::None);
        let ports = mi.ports();
        let port = ports.iter().find(|p| match (port_filter, mi.port_name(p)) {
            (Some(f), Ok(n)) => n.contains(f),
            (None, _) => true,
            _ => false,
        }).cloned().ok_or_else(|| "no matching MIDI port".to_string())?;
        let name = mi.port_name(&port).map_err(|e| e.to_string())?;
        let inbox = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        let cb_inbox = inbox.clone();
        let conn = mi.connect(&port, "dtx-in", move |_ts, bytes, _| {
            if let Some(ev) = midi_bytes_to_event(bytes, 0) {  // audio_ms stamped in poll_midi, not here
                if let Ok(mut q) = cb_inbox.lock() { q.push_back(ev); }
            }
        }, ()).map_err(|e| e.to_string())?;
        Ok((Self { _conn: conn, inbox }, name))
    }
}
#[cfg(feature = "midi")]
impl MidiSource for RealMidiSource {
    fn poll(&mut self, out: &mut Vec<MidiEvent>) -> usize {
        let mut n = 0;
        if let Ok(mut q) = self.inbox.lock() { while let Some(e) = q.pop_front() { out.push(e); n += 1; } }
        n
    }
    fn has_events(&self) -> bool { self.inbox.lock().map(|q| !q.is_empty()).unwrap_or(false) }
}
```
Note: the callback stamps `audio_ms: 0`; `poll_midi` restamps with the current AudioClock when it drains (matches how keyboard capture compensates). If restamping is awkward, stamp in the callback with a shared clock — but 0 + restamp-on-drain is simplest.

- [ ] **Step 2:** `cargo build -p dtx-input --features midi` → compiles (or note ALSA-header absence). Default `cargo test -p dtx-input` → PASS (unchanged). **Step 3: Commit** `feat(dtx-input): RealMidiSource (midir, feature-gated)`.

---

### Task 3: Wire the real source into `poll_midi` + `LastMidiHit`

**Files:** `crates/gameplay-drums/src/lib.rs`.

Context: `poll_midi` must drain the real source (when present) as well as `VirtualSource`, restamp `audio_ms`, capture the last velocity/note, and (feature-gated) connect on song start / on config change.

- [ ] **Step 1: `LastMidiHit` resource** (non-gated — the meter + capture read it):
```rust
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastMidiHit { pub note: u8, pub velocity: u8, pub below_threshold: bool, pub at: Option<std::time::Instant> }
```
(Use `Instant` for meter decay; if `Instant::now` is banned in this context it isn't — only workflow scripts ban it. Systems may call it.)

- [ ] **Step 2: Real-source resource** (NonSend, feature-gated) + connect system:
```rust
#[cfg(feature = "midi")]
#[derive(Default)]
struct MidiConnection(Option<dtx_input::midi::RealMidiSource>);
```
Insert as a **non-send** resource (`app.insert_non_send_resource(MidiConnection::default())` under `#[cfg(feature="midi")]`). A system `connect_midi_on_change` (main-thread, non-send access) runs on `OnEnter(Performance)` and when `LiveBindings.0.midi.port` changes: `RealMidiSource::connect(port_filter)` → store in `MidiConnection.0`; on error `warn!` and leave disconnected. (Reconnect = drop old + connect new.)

- [ ] **Step 3: Drain in `poll_midi`**: after draining `VirtualSource`, also drain `MidiConnection.0` (feature-gated) into the same event vec. For each NoteOn drained: set `LastMidiHit { note, velocity, below_threshold: vel<=threshold, at: Some(Instant::now()) }` BEFORE the threshold skip. Restamp `audio_ms` = current AudioClock ms (mirror the keyboard path). Keep the existing threshold/lane/LaneHit logic. `poll_midi` gains `Res<LastMidiHit>`→`ResMut`, and (feature-gated) `NonSend<MidiConnection>`.
   - NOTE: a system taking `NonSend` won't parallelize, but `poll_midi` is small; acceptable. If mixing NonSend into `poll_midi` is awkward, split: a separate feature-gated `drain_real_midi` system that runs before `poll_midi` and pushes real events into `VirtualSource` (reusing its queue!) — THIS IS CLEANER: `drain_real_midi` (NonSend, feature-gated) drains `MidiConnection` into `ResMut<VirtualSource>` via `note_on`/`push`; then the existing `poll_midi` handles everything uniformly and only needs `ResMut<LastMidiHit>` added for the velocity capture. **Prefer this approach** — minimal change to `poll_midi`, real events flow through the same path as virtual.

- [ ] **Step 4:** `init_resource::<LastMidiHit>()`; register systems. `cargo test -p gameplay-drums` (default) → PASS. `cargo build -p gameplay-drums --features dtx-input/midi` (or the app feature) → compiles. **Step 5: Commit** `feat(gameplay-drums): drain real MIDI into poll path + LastMidiHit capture`.

---

### Task 4: Device box UI — port dropdown + rescan + velocity meter

**Files:** `crates/gameplay-drums/src/editor/bindings_panel.rs`.

Context: The DEVICE box (3a has only the threshold row) gains: a port selector (cycle through `available_ports()` — a `◂ name ▸` cycler is simplest in this UI toolkit, or a click-to-cycle button), a rescan button, and a velocity meter bar (reads `LastMidiHit`: a horizontal bar whose fill = last velocity/127 with a threshold tick; dim/"ignored" styling when `below_threshold`).

- [ ] **Step 1:** Components `PortCycle(i32)` (±1 to cycle ports), `RescanPorts`. Store the port list in a resource `#[derive(Resource, Default)] MidiPortList(Vec<String>)`, refreshed on rescan + on open. Render current port from `LiveBindings.0.midi.port` (or "first available"/"none").
- [ ] **Step 2:** Cycling sets `LiveBindings.0.midi.port = Some(name)` (triggers Task 3 reconnect). Rescan calls `available_ports()` → `MidiPortList`.
- [ ] **Step 3:** Velocity meter — a bar Node whose child fill-width % = `LastMidiHit.velocity as f32/127.0`, updated each frame (a system reading `LastMidiHit`), with a threshold marker at `velocity_threshold/127`. Below-threshold → amber/dim.
- [ ] **Step 4:** `cargo test -p gameplay-drums` → PASS. **Step 5: Commit** `feat(gameplay-drums): MIDI device box (port select + rescan + velocity meter)`.

---

### Task 5: Complete MIDI capture

**Files:** `crates/gameplay-drums/src/editor/bindings_capture.rs`.

Context: 3a left `capture_binding` keyboard-only (draining the source would race poll_midi). Now `LastMidiHit` gives the last note without a second drain — capture reads it.

- [ ] **Step 1:** In `capture_binding` `Capturing(ch)`: alongside keyboard scan, check `LastMidiHit` — if it changed THIS frame (`at` newer than a stored "capture started at", or a fresh-flag) and `velocity > 0`, candidate = `BindSource::Midi { note }`. First candidate (key or MIDI) wins. Use a small guard so the SAME held note isn't re-consumed (compare `LastMidiHit.at`/note to a per-capture "last seen"). Keep the steal-confirm + Idle transitions.
- [ ] **Step 2:** Remove the `// TODO(3b)` hook. `cargo test -p gameplay-drums` → PASS. **Step 3: Commit** `feat(gameplay-drums): MIDI note capture in bindings tab`.

---

### Task 6: Enable `midi` in the desktop app + verify

**Files:** `app/dtxmaniars-desktop/Cargo.toml`, verification.

- [ ] **Step 1:** In `app/dtxmaniars-desktop/Cargo.toml`, enable the feature on the dtx-input dep (via whatever crate pulls it — likely `gameplay-drums` re-exposes a `midi` feature that forwards to `dtx-input/midi`; add a `midi` feature to gameplay-drums = `["dtx-input/midi"]` and enable it in the desktop app, default-on for desktop). Ensure `cargo build -p dtxmaniars-desktop` compiles WITH midi (needs ALSA dev headers on Linux — if the build env lacks them, document the apt/pacman package; the feature graph must be correct regardless).
- [ ] **Step 2:** `cargo test --workspace` (default, no midi) → PASS (1293+). `cargo clippy` → clean.
- [ ] **Step 3: Manual smoke (USER — needs real e-drum kit):** plug in the MIDI kit; F1→Bindings tab; DEVICE box lists the kit's port; hitting a pad drives the velocity meter + flashes the lane + plays a sound; `+` on a channel → capture → hit a pad → binds that note; threshold slider gates soft hits (meter shows "ignored"); Esc saves to `bindings.toml`; reopen → binds persist; close & play → the kit triggers the bound channels.
- [ ] **Step 4:** Final fixups commit.

---

## Self-review notes

- **Spec §5 coverage:** real device → Tasks 1-3; port dropdown + rescan → Task 4; velocity meter → Tasks 3(capture)+4(bar); MIDI capture (first-wins with keyboard) → Task 5. Persist port/threshold → already in `MidiDeviceConfig` (3a saves on close).
- **Default build stays hardware-free:** everything midir is `#[cfg(feature="midi")]` with non-`midi` fallbacks; `cargo test --workspace` needs no device/ALSA. Only the desktop app + explicit `--features midi` pull midir. **This is the key safety property** — CI + all existing tests unaffected.
- **Race-free capture:** MIDI events flow through ONE drain (`drain_real_midi`→VirtualSource→poll_midi); the meter + capture read `LastMidiHit` (written by poll_midi), never a second drain. No lost/duplicated notes.
- **Threading:** midir callback thread → `Arc<Mutex<VecDeque>>` → drained on the Bevy main thread; connection held in a NonSend resource (not `Sync`).
- **Untestable-headless (stated):** the real connection + meter-under-real-hits need the user's kit; unit tests cover parse + threshold + capture logic via VirtualSource.
- **Risk:** ALSA/CoreMIDI headers for the `--features midi` build; and the NonSend `poll_midi` split — mitigated by the `drain_real_midi`→VirtualSource approach (Task 3 Step 3 preferred path) which keeps `poll_midi` mostly unchanged.
```
