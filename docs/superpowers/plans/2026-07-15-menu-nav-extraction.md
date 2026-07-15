# Menu Navigation Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move menu-navigation input plumbing out of `gameplay-drums` into `dtx-input` (device layer) and `game-shell` (context/routing layer), behavior-preserving, per `docs/superpowers/specs/2026-07-15-menu-nav-extraction-design.md`.

**Architecture:** dtx-input gains the fixed lane order, bind resolution, and the MIDI pump (connection, drain, velocity filter, resolution → device messages). game-shell gains a `navigation` module (NavContext, NavGuard, pad→verb mapper, ActiveNavContext). gameplay-drums becomes a consumer: it gates `ResolvedInputHit` into `InputHit`, publishes `ActiveNavContext`, writes `RawInputOwned`, and keeps compat re-exports so nothing downstream changes.

**Tech Stack:** Rust workspace, Bevy 0.19 (`Message`/`MessageReader`, `States`, `FixedUpdate` sets).

**Hard rules:**
- No behavior change. Every moved block is verbatim unless a step shows the edited code.
- Two atomic-swap commits (Tasks 5 and 8) prevent double emission: the pump is not wired until Task 5; the game-shell pad mapper is not registered until Task 8.
- Never `.any()`/`.next()` a `MessageReader`; discard with `.clear()` (unread messages replay next frame).
- Each task ends with the workspace green and a commit.

**Verification commands (used throughout):**
- `cargo test -p dtx-input` / `-p game-shell` / `-p gameplay-drums`
- `cargo check -p dtx-input --features midi` (feature-gated pump code)
- Final: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`

---

### Task 0: Branch

- [ ] **Step 1: Create branch**

```bash
cd /home/lazykern/lab/dtxmaniars
git checkout main && git pull && git checkout -b refactor/menu-nav-extraction
```

---

### Task 1: Fixed lane order → dtx-input

The BocuD lane order is pure `EChannel → u8` data. The resolver (Task 2) needs it inside dtx-input.

**Files:**
- Create: `crates/dtx-input/src/lane_map.rs`
- Modify: `crates/dtx-input/src/lib.rs`
- Modify: `crates/gameplay-drums/src/lane_map.rs` (becomes shim)

- [ ] **Step 1: Create `crates/dtx-input/src/lane_map.rs`**

Copy the entire contents of `crates/gameplay-drums/src/lane_map.rs` (all 81 lines: `LaneId`, `LANE_COUNT`, `LANE_ORDER`, `lane_of`, `lane_channel`, the `tests` module) unchanged, except replace the module doc comment with:

```rust
//! Fixed BocuD lane order + channel↔lane helpers.
//!
//! Moved from gameplay-drums (menu-nav extraction, 2026-07-15 spec). Owned by
//! dtx-input so bind resolution can map channels to lanes without a gameplay
//! dependency. Lane visual order matches DTXmania BocuD
//! (CActPerfDrumsLaneFlushD.cs).
```

The file already only imports `dtx_core::EChannel`; dtx-input depends on dtx-core.

- [ ] **Step 2: Export from `crates/dtx-input/src/lib.rs`**

After the existing `pub mod keyboard;` line add:

```rust
pub mod lane_map;
```

Note: `lane_map::LaneId` and the existing `events::LaneId` are both `u8` aliases; no conflict because neither is glob-exported at the root.

- [ ] **Step 3: Turn `crates/gameplay-drums/src/lane_map.rs` into a shim**

Replace the whole file with:

```rust
//! Re-export of the fixed lane order, which moved to dtx-input
//! (menu-nav extraction, 2026-07-15 spec).

pub use dtx_input::lane_map::*;
```

All ~23 `crate::lane_map::…` call sites in gameplay-drums resolve through this re-export unchanged.

- [ ] **Step 4: Verify**

Run: `cargo test -p dtx-input && cargo test -p gameplay-drums`
Expected: PASS (lane_map tests now run in dtx-input).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor(input): move fixed lane order to dtx-input"
```

---

### Task 2: Bind resolution → dtx-input

**Files:**
- Create: `crates/dtx-input/src/resolver.rs`
- Modify: `crates/dtx-input/src/lib.rs`
- Modify: `crates/gameplay-drums/src/bindings.rs` (becomes shim + schedule wiring)

- [ ] **Step 1: Create `crates/dtx-input/src/resolver.rs`**

Move from `crates/gameplay-drums/src/bindings.rs`, verbatim: `ActiveInputProfiles`, `LiveBindings`, `BindResolver` (struct + `Default` + full impl), `keyboard_registry_path`, `midi_registry_path`, `startup_registry`, `active_keyboard_profile`, `active_midi_profile`, `compose_bindings`, `reload_profiles`, `apply_live_bindings`, and the entire `tests` module. Do NOT move the `plugin` function (it references `game_shell::AppState`; scheduling stays in gameplay-drums).

Required edits while moving:
1. Module doc:

```rust
//! Runtime bind resolution: `InputBindings` → per-frame lookup tables.
//!
//! Moved from gameplay-drums (menu-nav extraction, 2026-07-15 spec).
//! `BindResolver` flattens channel-keyed bindings into KeyCode→LaneIds and
//! note→LaneId maps using the fixed BocuD lane order (`crate::lane_map`).
//! Scheduling (when to reload/apply) is wired by the consuming game crate.
```

2. Imports: `dtx_input::profiles::…` → `crate::profiles::…`; `dtx_input::{…}` → `crate::{…}`; `crate::lane_map::{lane_of, LaneId}` → `use crate::lane_map::{lane_of, LaneId};` (path is the same name, now local); `dtx_config::default_path` stays.
3. Visibility: `active_keyboard_profile`, `active_midi_profile`, `compose_bindings` change `pub(crate)` → `pub` (gameplay-drums profile bar UI calls them through the shim). `reload_profiles` and `apply_live_bindings` change from private → `pub` (gameplay-drums wires them into schedules). Add a doc line to each of the five noting the external caller.
4. In the moved tests, `dtx_input::` paths → `crate::`, `crate::lane_map::lane_of` → `crate::lane_map::lane_of` (unchanged), `crate::bindings::BindResolver` → `BindResolver`.

- [ ] **Step 2: Export from `crates/dtx-input/src/lib.rs`**

```rust
pub mod resolver;
```

and to the root re-exports add:

```rust
pub use resolver::{ActiveInputProfiles, BindResolver, LiveBindings};
```

- [ ] **Step 3: Shrink `crates/gameplay-drums/src/bindings.rs` to shim + wiring**

Replace the whole file with:

```rust
//! Bind-resolution re-exports + schedule wiring.
//!
//! The resolver moved to `dtx_input::resolver` (menu-nav extraction,
//! 2026-07-15 spec). This module keeps the `crate::bindings::…` paths alive
//! and owns the *when*: dtx-input cannot reference `game_shell::AppState`.

use bevy::prelude::*;

pub use dtx_input::resolver::{
    active_keyboard_profile, active_midi_profile, apply_live_bindings, compose_bindings,
    keyboard_registry_path, midi_registry_path, reload_profiles, ActiveInputProfiles,
    BindResolver, LiveBindings,
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BindResolver>()
        .init_resource::<LiveBindings>()
        .init_resource::<ActiveInputProfiles>()
        // Seeded at boot too: pads navigate menus before any Performance enter.
        .add_systems(Startup, reload_profiles)
        .add_systems(OnEnter(game_shell::AppState::Performance), reload_profiles)
        .add_systems(
            Update,
            apply_live_bindings
                .run_if(resource_changed::<LiveBindings>)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}
```

(This is the old plugin body verbatim; only the system/type sources changed.)

- [ ] **Step 4: Verify**

Run: `cargo test -p dtx-input && cargo test -p gameplay-drums && cargo check --workspace`
Expected: PASS. The 20 resolver tests now run in dtx-input.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor(input): move bind resolution to dtx-input"
```

---

### Task 3: Device messages/resources → dtx-input

Types only; systems move in Task 4. Everything keeps compiling because gameplay-drums re-exports.

**Files:**
- Create: `crates/dtx-input/src/pump.rs` (types + set; systems arrive in Task 4)
- Modify: `crates/dtx-input/src/lib.rs`
- Modify: `crates/game-shell/Cargo.toml`, `crates/game-shell/src/nav.rs`
- Modify: `crates/gameplay-drums/src/events.rs`, `crates/gameplay-drums/src/lib.rs`

- [ ] **Step 1: Create `crates/dtx-input/src/pump.rs` with the device types**

```rust
//! MIDI device pump: shared device-level messages and resources.
//!
//! Moved from gameplay-drums `midi_consumer` (menu-nav extraction,
//! 2026-07-15 spec). Systems: see Task 4 additions below this header.

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
```

- [ ] **Step 2: Export from `crates/dtx-input/src/lib.rs`**

```rust
pub mod pump;
```

and root re-exports:

```rust
pub use pump::{
    InputPumpSet, LastMidiHit, MidiConnected, PadNavHit, RawInputOwned, ResolvedInputHit,
    SystemVerbHit,
};
```

- [ ] **Step 3: game-shell re-exports `MidiConnected`**

`crates/game-shell/Cargo.toml`, under `[dependencies]`:

```toml
dtx-input = { workspace = true }
```

In `crates/game-shell/src/nav.rs`: delete the local `MidiConnected` struct (lines 48–51) and its `midi_connected_defaults_false` test; add near the top:

```rust
pub use dtx_input::MidiConnected;
```

The `nav::plugin` body keeps `.init_resource::<MidiConnected>()` — same type via re-export, idempotent when the pump also inits it later.

- [ ] **Step 4: gameplay-drums re-exports instead of defining**

In `crates/gameplay-drums/src/events.rs`: delete the `SystemVerbHit` struct (lines 58–64) and add:

```rust
pub use dtx_input::SystemVerbHit;
```

In `crates/gameplay-drums/src/lib.rs` inside `mod midi_consumer`: delete the local `LastMidiHit` and `PadNavHit` definitions and add at the top of the module:

```rust
pub use dtx_input::{LastMidiHit, PadNavHit};
```

The lib-root line `pub use midi_consumer::{LastMidiHit, PadNavHit};` still works through the nested re-export. `.add_message::<events::SystemVerbHit>()` and `.add_message::<PadNavHit>()` in the drums plugin still work (same types).

- [ ] **Step 5: Verify + commit**

Run: `cargo check --workspace && cargo test -p gameplay-drums -p game-shell -p dtx-input`
Expected: PASS.

```bash
git add -A && git commit -m "refactor(input): move device messages and resources to dtx-input"
```

---### Task 4: Pump systems → dtx-input (NOT wired yet)

Adds the moved systems and a `pump::plugin`, but nothing calls the plugin until Task 5 — wiring it now would run two pumps draining the same `VirtualSource`.

**Files:**
- Modify: `crates/dtx-input/src/pump.rs`

- [ ] **Step 1: Move the pump systems into `pump.rs`**

From `crates/gameplay-drums/src/lib.rs` `mod midi_consumer`, move: `MidiConnection` (cfg midi), `connect_midi` (cfg midi), `drain_real_midi` (cfg midi), `poll_midi`, `ConsumedMidi`, `consume_midi_events`. Leave behind (they move to drums' `midi_gate` in Task 5): `gameplay_ready`, `stamp_audio_ms`.

Edits while moving:

1. Imports at top of `pump.rs` grow to:

```rust
#[cfg(feature = "midi")]
use bevy::time::common_conditions::on_real_timer;

use crate::midi::{MidiSource, VirtualSource};
use crate::resolver::{BindResolver, LiveBindings};
```

2. `connect_midi`: `live: Res<crate::bindings::LiveBindings>` → `live: Res<LiveBindings>`; `connected: ResMut<game_shell::MidiConnected>` → `connected: ResMut<MidiConnected>`; `dtx_input::midi::RealMidiSource` → `crate::midi::RealMidiSource`. Body otherwise verbatim.

3. `poll_midi` loses all gameplay parameters and the ready computation:

```rust
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
```

4. `ConsumedMidi.hits` becomes `Vec<ResolvedInputHit>`; `consume_midi_events` drops the `gameplay_ready: bool` and `clock_ms: i64` parameters and always pushes the resolved hit (the gameplay gate moves to the consumer):

```rust
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
```

- [ ] **Step 2: Add `pump::plugin`**

```rust
/// Registers the pump. Deliberately a bare `fn` plugin: the consuming game
/// crate adds it and orders [`InputPumpSet`] against its own input sets.
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
```

(Same shape as the old `midi_consumer::plugin`; `DrumsSets::Input` membership is replaced by `InputPumpSet`, which Task 5 orders before `DrumsSets::Input`.)

- [ ] **Step 3: Move + adapt the pump tests**

Into `#[cfg(test)] mod tests` in `pump.rs`. From the old `midi_consumer::tests`:

- `midi_updates_last_hit_without_gameplay_readiness`: drop the `false, 0` args; the readiness half no longer applies here — rename to `midi_updates_last_hit_and_always_resolves` and assert `hits.hits.len() == 1` (the pump always resolves; gating is the consumer's job) plus the existing `last`/`nav_lanes` asserts.
- `shared_note_emits_one_atomic_hit`: drop `true, 0` args; `out.hits[0]` is now `ResolvedInputHit` — assert `lanes == vec![2, 11]`, `captured_at == captured_at`, `audio_ms == 10`, `nav_lanes == vec![2]`.
- `system_verb_fires_while_gameplay_is_not_ready`: drop `false, 0` args; keep all three asserts (verb fires, no resolved hit, no nav lane). Rename to `system_verb_fires_and_never_resolves_a_lane`.
- `sub_threshold_system_note_emits_nothing`: drop `true, 0` args; keep assert.
- `a_lane_note_never_emits_a_system_verb`: drop `true, 0` args; keep asserts (`hits.len() == 1` still valid — resolved hit).
- `paused_midi_keeps_navigation_and_verbs_but_never_gameplay` and `gated_midi_event_is_not_replayed_when_gameplay_becomes_ready`: do NOT move — they test the gameplay gate; Task 5 recreates them in drums.

All `crate::bindings::BindResolver` → `crate::resolver::BindResolver`; `dtx_input::midi::MidiEvent` → `crate::midi::MidiEvent`; `dtx_input::{BindSource, InputBindings, SystemVerb}` → `crate::{BindSource, InputBindings, SystemVerb}`.

Add one new test pinning the spec amendment:

```rust
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
```

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p dtx-input && cargo check -p dtx-input --features midi && cargo check --workspace`
Expected: PASS. gameplay-drums still runs its own old pump; the new one is dormant (dead-code warnings are acceptable only if `plugin` is `pub`, which it is).

```bash
git add -A && git commit -m "refactor(input): move MIDI pump systems to dtx-input (unwired)"
```

---

### Task 5: ATOMIC SWAP — drums consumes the dtx-input pump

Deletes the old `midi_consumer` systems and wires the new pump in the same commit.

**Files:**
- Modify: `crates/gameplay-drums/src/lib.rs`

- [ ] **Step 1: Replace `mod midi_consumer` with `mod midi_gate`**

Delete from `mod midi_consumer`: `MidiConnection`, `connect_midi`, `drain_real_midi`, `poll_midi`, `ConsumedMidi`, `consume_midi_events`, its `plugin`, and the moved tests. Rename the module to `midi_gate` with this content (keeping `gameplay_ready` and `stamp_audio_ms` verbatim):

```rust
mod midi_gate {
    //! Gates dtx-input's `ResolvedInputHit` into gameplay `InputHit`.
    //!
    //! The pump (connection, drain, velocity filter, resolution) moved to
    //! `dtx_input::pump` (menu-nav extraction, 2026-07-15 spec). This module
    //! owns the only gameplay-specific part: deciding whether gameplay is
    //! ready and restamping with the gameplay clock.

    use bevy::prelude::*;
    use dtx_input::ResolvedInputHit;

    use super::events::InputHit;
    use crate::resources::GameplayClock;

    pub use dtx_input::{LastMidiHit, PadNavHit};

    pub(super) fn plugin(app: &mut App) {
        app.add_plugins(dtx_input::pump::plugin)
            .configure_sets(
                FixedUpdate,
                dtx_input::InputPumpSet.before(super::DrumsSets::Input),
            )
            .add_systems(
                FixedUpdate,
                convert_resolved_hits.in_set(super::DrumsSets::Input),
            );
    }

    /// Timestamp for an emitted hit: the event's own stamp if it has one,
    /// else the gameplay clock, else 0 (menus don't care about timing).
    pub(crate) fn stamp_audio_ms(clock_ms: Option<i64>, event_ms: i64) -> i64 {
        if event_ms != 0 {
            event_ms
        } else {
            clock_ms.unwrap_or(0)
        }
    }

    fn gameplay_ready(
        chart_ready: bool,
        clock_ready: bool,
        practice_ready: bool,
        pause: &game_shell::PauseState,
    ) -> bool {
        chart_ready && clock_ready && practice_ready && *pause == game_shell::PauseState::Running
    }

    fn convert_resolved_hits(
        chart: Res<crate::resources::ActiveChart>,
        clock: Res<GameplayClock>,
        flow: Option<Res<crate::practice::PracticeFlow>>,
        pause: Res<State<game_shell::PauseState>>,
        mut resolved: MessageReader<ResolvedInputHit>,
        mut hits: MessageWriter<InputHit>,
    ) {
        let ready = gameplay_ready(
            !chart.chart.chips.is_empty(),
            clock.is_ready(),
            crate::practice::gameplay_input_active(flow),
            pause.get(),
        );
        if !ready {
            // Drop, don't defer: an unread message replays next frame, and a
            // hit buffered while paused/not-ready must never judge later.
            resolved.clear();
            return;
        }
        for hit in resolved.read() {
            hits.write(InputHit {
                lanes: hit.lanes.clone(),
                audio_ms: stamp_audio_ms(Some(clock.current_ms), hit.audio_ms),
                captured_at: hit.captured_at,
            });
        }
    }
}
```

- [ ] **Step 2: Fix drums plugin wiring in `lib.rs`**

1. In the plugin list, replace `midi_consumer::plugin,` with `midi_gate::plugin,`.
2. Delete `.init_resource::<dtx_input::midi::VirtualSource>()` (line ~124) — the pump plugin owns it.
3. Delete `.add_message::<events::SystemVerbHit>()` (~line 175) — the pump plugin registers it. Keep the other `add_message` lines (`InputHit` etc. are still drums types).
4. Update the lib-root re-export line 881 to `pub use midi_gate::{LastMidiHit, PadNavHit};` (or directly `pub use dtx_input::{LastMidiHit, PadNavHit};` — pick one, delete the other).
5. Any `stamp_audio_ms`/`midi_consumer::` references elsewhere in the crate → `midi_gate::`.

- [ ] **Step 3: Recreate the two gate tests in `midi_gate::tests`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_gameplay_is_never_ready() {
        assert!(!gameplay_ready(true, true, true, &game_shell::PauseState::Paused));
        assert!(gameplay_ready(true, true, true, &game_shell::PauseState::Running));
    }

    #[test]
    fn stamp_prefers_event_time_then_clock() {
        assert_eq!(stamp_audio_ms(Some(500), 2_000), 2_000);
        assert_eq!(stamp_audio_ms(Some(500), 0), 500);
        assert_eq!(stamp_audio_ms(None, 0), 0);
    }

    /// The old pump dropped not-ready hits instead of buffering them. The gate
    /// must do the same: a hit that arrives while not ready is cleared, not
    /// replayed once gameplay becomes ready.
    #[test]
    fn gated_hit_is_not_replayed_when_gameplay_becomes_ready() {
        use dtx_input::ResolvedInputHit;

        let mut app = App::new();
        app.init_state::<game_shell::PauseState>()
            .init_resource::<crate::resources::ActiveChart>()
            .init_resource::<GameplayClock>()
            .add_message::<ResolvedInputHit>()
            .add_message::<InputHit>()
            .add_systems(Update, convert_resolved_hits);

        // Not ready: empty chart. The hit must be dropped.
        app.world_mut().write_message(ResolvedInputHit {
            lanes: vec![1],
            audio_ms: 0,
            captured_at: std::time::Instant::now(),
        });
        app.update();
        app.update(); // second frame: a buffered message would surface here
        let count = app
            .world()
            .resource::<Messages<InputHit>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(count, 0, "not-ready hit must be dropped, not deferred");
    }
}
```

(If `ActiveChart`/`GameplayClock` lack `Default` for `init_resource`, insert them with the same empty-chart construction used by existing drums tests — copy the pattern from `tests/practice_mode.rs` app setup.)

- [ ] **Step 4: Verify**

Run: `cargo test -p gameplay-drums && cargo test -p dtx-input && cargo test --workspace`
Expected: PASS, including `tests/practice_mode.rs` (pump now reached via `midi_gate::plugin` → `dtx_input::pump::plugin`, same `VirtualSource` path).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor(drums): consume dtx-input MIDI pump via midi_gate"
```

---

### Task 6: Keyboard system verbs + RawInputOwned

**Files:**
- Modify: `crates/dtx-input/src/keyboard.rs`
- Modify: `crates/gameplay-drums/src/input.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

- [ ] **Step 1: Add the translator to `crates/dtx-input/src/keyboard.rs`**

```rust
/// Keyboard-bound system verbs → [`crate::SystemVerbHit`], on the same message
/// the MIDI pump writes. Carries NO state gating — the consuming game crate
/// wires run conditions (e.g. only during Performance, and deliberately not
/// gated on pause: the key that paused the song has to un-pause it). Emits
/// nothing while [`crate::RawInputOwned`] is set: a capture flow owns the
/// keyboard.
pub fn keyboard_system_verbs(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<crate::resolver::BindResolver>,
    owned: Res<crate::RawInputOwned>,
    mut out: MessageWriter<crate::SystemVerbHit>,
) {
    if owned.0 {
        return;
    }
    for key in keys.get_just_pressed() {
        for verb in resolver.system_for_key(*key) {
            out.write(crate::SystemVerbHit { verb });
        }
    }
}
```

Move the two tests from `gameplay-drums/src/input.rs` (`bound_key_emits_the_system_verb`, `an_armed_capture_swallows_the_system_verb`) into `keyboard.rs` tests, adapted: replace `CaptureState` setup with `RawInputOwned` —

```rust
#[test]
fn bound_key_emits_the_system_verb() {
    use crate::{BindSource, InputBindings, RawInputOwned, SystemVerb, SystemVerbHit};

    let mut bindings = InputBindings::default();
    bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<RawInputOwned>()
        .insert_resource(crate::resolver::BindResolver::from_bindings(&bindings))
        .add_message::<SystemVerbHit>()
        .add_systems(Update, keyboard_system_verbs);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F9);
    app.update();

    let hits: Vec<SystemVerbHit> = app
        .world()
        .resource::<Messages<SystemVerbHit>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(hits, vec![SystemVerbHit { verb: SystemVerb::Pause }]);
}

#[test]
fn owned_raw_input_swallows_the_system_verb() {
    use crate::{BindSource, InputBindings, RawInputOwned, SystemVerb, SystemVerbHit};

    let mut bindings = InputBindings::default();
    bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(RawInputOwned(true))
        .insert_resource(crate::resolver::BindResolver::from_bindings(&bindings))
        .add_message::<SystemVerbHit>()
        .add_systems(Update, keyboard_system_verbs);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F9);
    app.update();

    assert_eq!(
        app.world()
            .resource::<Messages<SystemVerbHit>>()
            .iter_current_update_messages()
            .count(),
        0,
        "a key pressed while capture owns input must not fire a verb"
    );
}
```

- [ ] **Step 2: Rewire `crates/gameplay-drums/src/input.rs`**

Delete the local `keyboard_system_verbs` fn and its two tests. In the plugin, replace the system reference:

```rust
        .add_systems(
            PreUpdate,
            // NO PauseState gate: this is the key that has to un-pause the song.
            dtx_input::keyboard::keyboard_system_verbs
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance)),
        )
```

(Identical run conditions to today; only the fn source and the capture gate moved.)

- [ ] **Step 3: Editor publishes `RawInputOwned`**

In `crates/gameplay-drums/src/editor/mod.rs`, add to the editor plugin:

```rust
        .add_systems(Update, sync_raw_input_owned)
```

and the system:

```rust
/// Publish "a capture/calibration surface owns raw input" to dtx-input.
/// Same-frame semantics as the old direct `CaptureState` read: the keyboard
/// translator runs in PreUpdate and saw last frame's capture state either way.
fn sync_raw_input_owned(
    capture: Res<bindings_capture::CaptureState>,
    calibration: Res<calibration::CalibrationState>,
    mut owned: ResMut<dtx_input::RawInputOwned>,
) {
    let next = !matches!(*capture, bindings_capture::CaptureState::Idle)
        || !matches!(*calibration, calibration::CalibrationState::Idle);
    if owned.0 != next {
        owned.0 = next;
    }
}
```

(Equality-guarded write — never write a resource/component unconditionally per frame.)

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p dtx-input -p gameplay-drums && cargo check --workspace`
Expected: PASS.

```bash
git add -A && git commit -m "refactor(input): keyboard system verbs + RawInputOwned to dtx-input"
```

---

### Task 7: game-shell navigation module (mapper NOT registered yet)

**Files:**
- Create: `crates/game-shell/src/navigation.rs`
- Delete: `crates/game-shell/src/nav.rs`
- Modify: `crates/game-shell/src/lib.rs`

- [ ] **Step 1: Create `crates/game-shell/src/navigation.rs`**

Assemble from two sources:

(a) All of current `nav.rs` (`NavVerb`, `NavSource`, `NavAction`, the `MidiConnected` re-export, `nav_action_is_copy_and_comparable` test), verbatim.

(b) From `gameplay-drums/src/menu_nav.rs`, verbatim: `NavContext`, `DEBOUNCE`, `ENTER_GRACE`, `NavGuard` (whole impl), `verb_for_lane`, and the tests `lane_verbs_follow_gitadora_convention`, `toms_supply_explicit_quick_setting_adjustment_verbs`, `guard_enforces_grace_then_debounce`, `guard_resets_grace_on_context_change`, `confirm_hit_cannot_cancel_the_load_it_started`. (The `active_context` fn and its tests stay in gameplay-drums — Task 8.)

Plus the new pieces:

```rust
/// Which menu surface currently owns pad navigation. `None` = pads are
/// gameplay input, or a capture/calibration overlay owns raw hits. Written by
/// the crate that knows the surface state (gameplay-drums publishes it every
/// frame, before [`NavMapSet`]); consumed by [`pad_nav_mapper`].
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveNavContext(pub Option<NavContext>);

/// Update-schedule set the pad mapper runs in. Context writers order
/// themselves `.before(NavMapSet)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavMapSet;

fn pad_nav_mapper(
    ctx: Res<ActiveNavContext>,
    mut hits: MessageReader<dtx_input::PadNavHit>,
    mut guard: ResMut<NavGuard>,
    mut out: MessageWriter<NavAction>,
) {
    let now = Instant::now();
    let Some(ctx) = ctx.0 else {
        guard.clear_context();
        hits.clear();
        return;
    };
    guard.enter_context(ctx, now);
    for hit in hits.read() {
        let Some(verb) = verb_for_lane(hit.lane) else {
            continue;
        };
        if !guard.accept(now) {
            continue;
        }
        out.write(NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        });
    }
}

/// Registers messages/resources. The mapper system itself is registered in a
/// follow-up commit — while gameplay-drums' old `menu_nav` mapper is still
/// alive, registering a second reader of `PadNavHit` would emit every
/// `NavAction` twice.
pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>()
        .init_resource::<MidiConnected>()
        .init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>();
}
```

Module doc for the file:

```rust
//! Semantic menu-navigation: actions, contexts, guard, and the pad mapper.
//!
//! Producers: per-screen keyboard systems and dtx-input's MIDI pump
//! (`PadNavHit`). Consumers: song select, title, pause menu, results,
//! settings overlay. Moved/merged from game-shell `nav.rs` and
//! gameplay-drums `menu_nav.rs` (menu-nav extraction, 2026-07-15 spec).
```

Also move the source-scrape test, adapted to the new file:

```rust
/// The mapper must read `PadNavHit`, never `LaneHit` — autoplay (forced on
/// by the Customize surface) and keyboard lane keys write `LaneHit`, and a
/// chart's autoplay notes would otherwise navigate and close the overlay.
#[test]
fn mapper_consumes_pad_nav_hits_not_lane_hits() {
    let src = include_str!("navigation.rs");
    let body = src
        .split("fn pad_nav_mapper(")
        .nth(1)
        .expect("pad_nav_mapper exists");
    let signature = body.split(") {").next().unwrap();
    assert!(
        signature.contains("PadNavHit"),
        "mapper must read PadNavHit"
    );
    assert!(
        !signature.contains("LaneHit"),
        "mapper must not read LaneHit (autoplay + keyboard write those)"
    );
}
```

`use` block for the file: `std::time::{Duration, Instant}`, `bevy::prelude::*`, `pub use dtx_input::MidiConnected;`.

- [ ] **Step 2: Update `crates/game-shell/src/lib.rs`**

- `pub mod nav;` → `pub mod navigation;`
- Keep a compat alias so `game_shell::nav::…` paths (if any appear later) and root names both survive:

```rust
pub use navigation::{MidiConnected, NavAction, NavSource, NavVerb};
```

(Replaces the old `pub use nav::{…}` line. Grep confirmed no `game_shell::nav::` path users exist; the root re-export is the public surface.)
- In `GameShellPlugin`: `nav::plugin` → `navigation::plugin`.
- Delete `nav.rs`.

- [ ] **Step 3: Verify + commit**

Run: `cargo test -p game-shell && cargo test --workspace`
Expected: PASS. Old drums mapper still the only NavAction emitter.

```bash
git add -A && git commit -m "refactor(shell): add game_shell::navigation (context, guard, pad mapper)"
```

---

### Task 8: ATOMIC SWAP — drums publishes context; game-shell maps

**Files:**
- Modify: `crates/game-shell/src/navigation.rs` (register mapper)
- Modify: `crates/gameplay-drums/src/menu_nav.rs` (gut to writer + compat)

- [ ] **Step 1: Register the mapper in `navigation::plugin`**

```rust
pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>()
        .init_resource::<MidiConnected>()
        .init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>()
        .add_systems(Update, pad_nav_mapper.in_set(NavMapSet));
}
```

Also update the plugin's doc comment (drop the "follow-up commit" note).

- [ ] **Step 2: Rewrite `crates/gameplay-drums/src/menu_nav.rs`**

Replace the whole file with (the `active_context` fn, `publish_nav_context`'s parameter list, and the tests are today's code verbatim — only the output changed from "emit NavAction" to "write ActiveNavContext"):

```rust
//! Publishes which menu surface owns pad navigation.
//!
//! The pad→verb mapper, `NavContext`, and `NavGuard` moved to
//! `game_shell::navigation` (menu-nav extraction, 2026-07-15 spec). This
//! module keeps the one job game-shell cannot do: computing the active
//! context from gameplay-drums' own surface states (editor, capture,
//! calibration, practice phase) and publishing it each frame, ordered before
//! the mapper.

use bevy::prelude::*;
use game_shell::navigation::{ActiveNavContext, NavMapSet};
use game_shell::{AppState, PauseState};

use crate::editor::bindings_capture::CaptureState;
use crate::editor::calibration::CalibrationState;

// Compat adapter (migration): keeps `gameplay_drums::menu_nav::…` paths alive
// for the Practice branch and integration tests.
pub use game_shell::navigation::{NavAction, NavContext, NavGuard, NavSource, NavVerb};

pub(super) fn plugin(app: &mut App) {
    // NavGuard/ActiveNavContext are normally registered by game-shell's
    // navigation plugin; init here too (idempotent) so drums-only test apps
    // that poke `menu_nav::NavGuard` keep working without GameShellPlugin.
    app.init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>()
        .add_systems(Update, publish_nav_context.before(NavMapSet));
}

/// `None` = pads are gameplay input, or a capture/calibration overlay owns raw hits.
fn active_context(
    app_state: &AppState,
    pause: &PauseState,
    editor_open: bool,
    capture_armed: bool,
    calibrating: bool,
    practice_phase: Option<crate::practice::PracticePhase>,
) -> Option<NavContext> {
    if capture_armed || calibrating {
        return None;
    }
    match app_state {
        AppState::Title => Some(NavContext::Title),
        AppState::SongSelect => Some(NavContext::SongSelect),
        AppState::Result => Some(NavContext::Result),
        AppState::SongLoading => Some(NavContext::Loading),
        AppState::Performance => {
            if editor_open {
                Some(NavContext::Editor)
            } else if *pause == PauseState::Paused {
                Some(NavContext::Paused)
            } else if matches!(
                practice_phase,
                Some(
                    crate::practice::PracticePhase::Setup | crate::practice::PracticePhase::Editing
                )
            ) {
                Some(NavContext::PracticeSetup)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn publish_nav_context(
    app_state: Res<State<AppState>>,
    pause: Res<State<PauseState>>,
    editor_open: Res<crate::editor::EditorOpen>,
    capture: Res<CaptureState>,
    calibration: Res<CalibrationState>,
    practice: Option<Res<crate::practice::PracticeFlow>>,
    mut ctx: ResMut<ActiveNavContext>,
) {
    let next = active_context(
        app_state.get(),
        pause.get(),
        editor_open.0,
        !matches!(*capture, CaptureState::Idle),
        !matches!(*calibration, CalibrationState::Idle),
        practice.as_deref().map(|flow| flow.phase),
    );
    if ctx.0 != next {
        ctx.0 = next;
    }
}
```

Keep, verbatim in `#[cfg(test)] mod tests`: `no_context_during_live_play_or_capture` and `practice_setup_and_editing_own_pad_navigation_but_running_does_not` (they test `active_context`, unchanged). Delete the guard/lane/scrape tests here — they moved in Task 7.

- [ ] **Step 3: Verify — the double-emission check**

Run: `cargo test --workspace`
Expected: PASS, especially `tests/practice_mode.rs` (uses `gameplay_drums::menu_nav::{NavGuard, NavContext}` via the compat re-export, and drives pad nav end-to-end: pump → PadNavHit → drums-published context → game-shell mapper → NavAction).

Manual sanity (optional but recommended, per repo BRP smoke memory): launch the game, hit a virtual pad twice fast on Song Select — exactly one wheel step (debounce), none within 500 ms of screen enter (grace).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor(drums): publish nav context; game-shell owns pad mapping"
```

---

### Task 9: Docs, sweep, PR

**Files:**
- Modify: `crates/dtx-input/src/lib.rs` (module docs)
- Modify: any stale doc comments found by grep

- [ ] **Step 1: Retire the "LaneId is opaque" contract in `crates/dtx-input/src/lib.rs`**

Replace the `## LaneId is opaque` doc section with:

```rust
//! ## Lane resolution lives here
//!
//! Since the menu-nav extraction (2026-07-15 spec), this crate owns the fixed
//! BocuD lane order (`lane_map`), bind resolution (`resolver`), and the MIDI
//! pump (`pump`). It emits device-level messages (`PadNavHit`,
//! `ResolvedInputHit`, `SystemVerbHit`); it knows nothing about application
//! contexts (SongSelect, Settings, Practice) — those belong to game-shell.
```

Also update the module map list at the top of the file to mention `lane_map`, `resolver`, `pump`.

- [ ] **Step 2: Stale-reference sweep**

```bash
grep -rn "midi_consumer\|crate::bindings::BindResolver moved\|dtx-config::bindings" crates --include="*.rs"
grep -rn "menu_nav" crates/game-menu crates/game-results --include="*.rs"
```

Fix any comment that still describes the old layout (known: `crates/gameplay-drums/src/input.rs` header says "MIDI is handled in the lib.rs `midi_consumer` module" → now `dtx_input::pump` + `midi_gate`; `crates/game-menu/src/song_loading.rs:750` mentions `menu_nav` emitting — still true via the new path, reword to `game_shell::navigation`).

- [ ] **Step 3: Full verification**

```bash
cargo test --workspace
cargo check -p dtx-input --features midi
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: all green.

- [ ] **Step 4: Cross-check the spec's preserved-behavior checklist**

Walk `docs/superpowers/specs/2026-07-15-menu-nav-extraction-design.md` §4 item by item; each maps to an unchanged constant, moved test, or untouched file. Confirm `game-menu` and `game-results` have zero diff in this branch (`git diff main --stat -- crates/game-menu crates/game-results` → empty).

- [ ] **Step 5: Push + PR**

```bash
git push -u origin refactor/menu-nav-extraction
gh pr create --title "refactor: extract menu navigation to dtx-input + game-shell" --body-file - <<'EOF'
Behavior-preserving foundation PR per docs/superpowers/specs/2026-07-15-menu-nav-extraction-design.md.

Ownership after this PR:
- dtx-input: lane order, bind resolution, MIDI pump, device messages, keyboard verb translation
- game-shell: navigation module (NavContext, NavGuard, pad mapper, ActiveNavContext)
- gameplay-drums: consumes ResolvedInputHit, publishes context, compat re-exports

Preserved (checklist from spec §4): 80 ms debounce, 500 ms entry grace,
velocity threshold, GITADORA pad map, per-screen keyboard nav untouched,
Practice Setup nav, MIDI reconnect, lane-wins-ties system binds,
capture/calibration input ownership, PadNavHit-not-LaneHit invariant.
No changes under crates/game-menu or crates/game-results.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
```

---

## Task → spec traceability

| Spec section | Tasks |
|---|---|
| §1 dtx-input pump/resolution/messages | 1, 2, 3, 4, 5 |
| §1 keyboard verbs + RawInputOwned | 6 |
| §2 game-shell navigation | 7, 8 |
| §3 drums consumer/context writer/adapter | 5, 6, 8 |
| §4 preserved-behavior checklist | 9 (verified), enforced throughout |
| §5 test moves | 1, 2, 4, 5, 6, 7, 8 |
| §6 commit structure | one commit per task; groups 1=T1–4, 2=T5–6, 3=T7–8 |
