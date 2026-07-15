# Menu Navigation Extraction — Design

Date: 2026-07-15
Status: Approved

## Purpose

Behavior-preserving foundation PR that moves menu-navigation input plumbing out
of `gameplay-drums` so the unified-navigation program (Agent A: `dtx-input`,
Agent B: `game-shell`/`dtx-ui`) can proceed without touching the Practice
agent's crate. **No new UX behavior.** Serialized against Practice work: no
concurrent edits to `gameplay-drums/src/menu_nav.rs` or the `midi_consumer`
module.

## Ownership after extraction

| Crate | Owns |
|---|---|
| `dtx-input` | MIDI connection + event pump, raw MIDI events, velocity filtering, binding/profile resolution, keyboard system-verb translation, device messages |
| `game-shell` | `NavContext` type, `ActiveNavContext` resource, `NavGuard` (debounce + grace), pad→verb mapping, `NavAction` emission, `NavVerb`/`NavSource` |
| `gameplay-drums` | Gameplay judgments, gameplay lane keyboard input, `ResolvedInputHit → InputHit` gating/stamping, writing `ActiveNavContext` and `RawInputOwned`, compat re-exports |

`dtx-input` knows nothing about `SongReady`, `PracticeSetup`, or any
application context. Those live in `game-shell`.

## 1. dtx-input (device layer)

Moves in from `gameplay-drums`:

- **Pump** (`midi_consumer` module + `connect_midi` + `drain_real_midi`):
  `MidiConnection` non-send resource, reconnect on port change / 1 s retry,
  drain real source into `VirtualSource`, poll. `midi` feature already
  forwards.
- **Resolution** (`bindings.rs`): `BindResolver`, `LiveBindings`,
  `ActiveInputProfiles`, registry load/startup/compose helpers. The fixed
  BocuD lane order (`lane_of` / `LANE_ORDER` from `lane_map`) moves here —
  pure `EChannel → u8` data; `dtx-input` already declares `LaneId`.
  This retires the "LaneId is opaque to dtx-input" doc contract.
- **Messages/resources**: `PadNavHit`, `SystemVerbHit`, `LastMidiHit`,
  `MidiConnected` (type moves out of `game-shell/nav.rs`).
- **New message** `ResolvedInputHit { lanes, audio_ms, captured_at }` — the
  current `InputHit` payload, emitted by the pump *before* any gameplay
  gating. `gameplay-drums` converts it downstream.
- **Keyboard system verbs** (`keyboard_system_verbs` from
  `gameplay-drums/src/input.rs`): same message, moved verbatim.
- **New resource** `RawInputOwned(bool)`: a binding-capture surface owns raw
  input. Gates the **keyboard system-verb translator only** (the one place
  that checks `CaptureState` today). The MIDI pump stays unconditional,
  exactly like today: `LastMidiHit` must keep updating during capture (note
  capture reads it), and MIDI system verbs already fire during capture with
  consumers gating themselves. Pad-nav suppression during capture AND
  calibration continues to happen at the context level
  (`ActiveNavContext = None`). `gameplay-drums` writes `RawInputOwned` from
  `CaptureState` **only** — the old keyboard gate never checked
  `CalibrationState`, and folding calibration in would suppress keyboard
  Pause/Restart during the tap test, a real behavior change deferred to the
  navigation program. Same gating as today, dependency direction inverted.

Unchanged logic that moves along: velocity threshold filter, `lane_owner`
lane-wins-ties collision rule, `stamp_audio_ms`.

## 2. game-shell — new `navigation` module

- `NavAction` / `NavVerb` / `NavSource` move from `nav.rs` into a new
  `navigation.rs`; `nav.rs` is deleted and `lib.rs` keeps the same public
  names via `pub use navigation::*`, so `game_shell::NavAction` paths keep
  compiling. `MidiConnected` moves to `dtx-input`; `game-shell` re-exports it
  (`pub use dtx_input::MidiConnected`) for existing legend-bar readers.
- From `gameplay-drums/src/menu_nav.rs`, verbatim: `NavContext`, `NavGuard`
  (80 ms debounce, 500 ms entry grace, incl. all tests), `verb_for_lane`
  (GITADORA map), `pad_nav_mapper`.
- Mapper now reads `dtx_input::PadNavHit` + new
  `ActiveNavContext(pub Option<NavContext>)` resource; `None` clears the
  guard and drains hits, exactly like today's `active_context() == None`
  path.
- `game-shell` gains a `dtx-input` dependency (`dtx-input` has no
  `game-shell` dep; no cycle).

**Deliberate deviation**: the 80 ms debounce stays inside `NavGuard` in
`game-shell` rather than moving to `dtx-input` as "device-specific debounce".
Its reset is coupled to context entry (grace clears on context change — the
"BD that confirmed a song must not cancel the load" invariant). Splitting a
device-side guard out is Agent A/B follow-up once per-device policy exists.

## 3. gameplay-drums (consumer only)

- `active_context()` logic stays **verbatim** as a writer system that sets
  `game_shell::navigation::ActiveNavContext` every frame (it already reads
  `AppState`, `PauseState`, `EditorOpen`, `CaptureState`, `CalibrationState`,
  `PracticeFlow`).
- New consumer system: `ResolvedInputHit → InputHit`, gated on
  `gameplay_ready` (chart non-empty, clock ready, practice input active,
  `PauseState::Running`) with clock restamp — the only gameplay-specific part
  of the old `poll_midi`.
- Editor writes `RawInputOwned` from its capture/calibration states.
- Keyboard **lane** input (`capture_key_to_lane_input`,
  `emit_pending_lane_hits`, GameplayClock stamping) stays — gameplay input,
  not a UI action.
- **Compat adapter** so the Practice branch and editor panels compile
  untouched:

  ```rust
  pub use game_shell::navigation::{NavAction, NavContext, NavSource};
  pub use dtx_input::{PadNavHit, SystemVerbHit, LastMidiHit, MidiConnected};
  // plus BindResolver / LiveBindings / ActiveInputProfiles / lane_map re-exports
  ```

## 4. Preserved-behavior checklist (goes in PR description)

- 80 ms pad-nav debounce; 500 ms state-entry grace
- Velocity threshold filtering (incl. `LastMidiHit.below_threshold` meter feed)
- GITADORA pad map (HH↑ CY/RD↓ BD✓ SD← HT/LT ∓ FT practice)
- Existing keyboard behavior: per-screen keyboard→NavAction systems in
  `game-menu` untouched
- Practice Setup navigation unchanged
- MIDI connect/reconnect behavior (startup, port change, 1 s rescan)
- System binds: lane wins ties; verbs fire mid-song, before gameplay gate
- Capture/calibration swallow raw input (keyboard and MIDI)
- `PadNavHit`-not-`LaneHit` invariant (autoplay/keyboard must not steer menus)
- No score/judgment path changes

## 5. Tests

- All existing tests move with their code: `menu_nav` tests → `game-shell`,
  `bindings.rs` tests → `dtx-input`, `midi_consumer` tests → `dtx-input`.
- Source-scrape test (`mapper_consumes_pad_nav_hits_not_lane_hits`) updated
  for the new file path; invariant unchanged.
- New: `RawInputOwned(true)` ⇒ keyboard-verb translator emits nothing
  (replaces the `CaptureState`-typed gating tests). Pump tests assert
  `LastMidiHit` updates regardless of `RawInputOwned`.
- `active_context()` tests stay in `gameplay-drums` with the writer system.

## 6. Commit structure

Per-task commits (each green), in three logical groups:

1. dtx-input: lane order → resolver → device messages → pump
2. drums pump swap + keyboard verbs + `RawInputOwned`
3. game-shell navigation module + drums context writer + compat adapter

Double-emission hazards force atomic swap commits: the pump is moved but not
wired until the drums swap commit (else two pumps drain `VirtualSource`), and
the game-shell pad mapper is not registered until the same commit that deletes
`menu_nav` (else two mappers each emit a `NavAction` per hit).

## Out of scope (explicitly)

- SystemVerb vocabulary growth / NavVerb→SystemVerb unification (Agent A)
- Input-source tracking beyond existing `NavSource` (Agent B)
- Context stack / modal routing, dynamic prompts (Agent B)
- Any mapping, timing, or UX change
