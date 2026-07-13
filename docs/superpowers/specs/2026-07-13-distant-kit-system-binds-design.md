# Distant-kit system binds — design

**Date:** 2026-07-13
**Research base:** `docs/notes/2026-07-13-distant-kit-research.md`
**Audit findings closed:** F1 (pause from the kit during live play), F3 (quit from the kit),
F4 (cancel a load from the kit)

---

## Goal

A drummer seated at an electronic kit, out of reach of the keyboard, can pause, quit, restart,
and cancel a load without leaving the throne.

## What the research settled, so this spec does not relitigate it

- **Pause from the kit is a binding, not a gesture.** DTXManiaNX ships a pad-bindable Pause
  key-assign slot (`CActConfigList.cs:2078`, MIDI-capable capture at `CActConfigKeyAssign.cs:128`).
  No peer drum game implements a pad *gesture* that pauses live gameplay.
- **Hold gestures are physically unrepresentable on drums.** A pad is a piezo transient sensor;
  its `NoteOff` fires on a module-side Gate Time timer (Roland 0.1–8.0 s, user-configurable),
  independent of the stick. Nothing is built on `NoteOff`.
- **A 12-channel DTX chart cannot address every note a kit emits.** Zone notes (xstick 37, ride
  bell 53, hi-hat edge 22/26, tom3 rim 58) already reach our parser and are dropped for want of a
  lane (`gameplay-drums/src/lib.rs:594-595`). Binding a system verb to one of those costs no
  gameplay pad and has no chart-collision surface at all.

Consequently there is **no gesture detection, no `NoteOff` consumption, and no CC plumbing** in
this design. The chart-corpus collision tables in the research note stand only as the fallback
evidence base if a kit turns out to have no spare zone note.

## Decisions taken

| Question | Decision |
|---|---|
| Which system verbs get pad-bindable slots | **Pause and Restart.** Not NX's full system-key set — Skip/Loop/Speed stay keyboard-only until someone wants them from the kit. |
| A verb bound to a note that is already a lane | **Refused, with the owning lane named.** NX silently auto-unbinds the lane (`CConfigIni.cs:1524`); that is why its feature went unused, and we will not repeat it. |
| Default binding | **Unbound.** Escape keeps working unchanged. Note maps vary by brand — a note free on a TD-17 may be a real pad elsewhere — so we do not guess on the user's behalf. |

---

## Architecture

### The bind target

`InputBindings.map` is keyed by `EChannel`, which is a **DTX chart channel**. System verbs are not
chart channels, so `EChannel` gains no pseudo-variants. `InputBindings` gains a parallel map:

```rust
/// A non-lane action a key or pad can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemVerb {
    /// Toggle the pause overlay during a performance.
    Pause,
    /// Restart the current song from the top.
    Restart,
}

pub struct InputBindings {
    pub midi: MidiDeviceConfig,
    pub map: HashMap<EChannel, Vec<BindSource>>,       // unchanged
    pub system: HashMap<SystemVerb, Vec<BindSource>>,  // new — empty by default
}
```

`BindSource` is reused unchanged, so a verb binds to a `Key` or a `Midi { note }` with no new
source type. `BindingsFile` is `#[serde(default)]`, so an existing `bindings.toml` with no
`[system]` table loads clean: **no schema version bump, no migration.**

### Correction: the profiles are the source of truth, not `InputBindings`

*(Found while planning. The design above is necessary but not sufficient, and shipping only the
above would produce a dead feature.)*

`bindings.toml` is now only a **legacy migration input**. The live path is the profile registry:
`reload_profiles` (`gameplay-drums/src/bindings.rs:230-252`) runs at boot **and on every
`Performance` enter**, and does `live.0 = compose_bindings(&keyboard, &midi)` — rebuilding
`InputBindings` from the profiles' lane maps alone (`:204-225`). A `system` map that existed only
in `InputBindings` would therefore be **overwritten at the exact moment the drummer starts a
song**, and the pad would do nothing.

So the system map must live in the profiles too:

```rust
pub struct KeyboardProfile { /* … */ pub system: HashMap<SystemVerb, Vec<KeyCode>> }
pub struct MidiProfile     { /* … */ pub system: HashMap<SystemVerb, Vec<u8>> }
```

threaded through both `split_bindings` (`dtx-input/src/profiles.rs:310`) and `compose_bindings`.
`InputBindings.system` and the `BindingsFile` `[system]` table stay exactly as specced — they
remain the migration source and the `save_bindings` round-trip — but they are no longer the thing
the resolver ultimately reads from. Both halves are required.

### Resolution

`BindResolver` gains `note_to_system: HashMap<u8, Vec<SystemVerb>>` and
`key_to_system: HashMap<KeyCode, Vec<SystemVerb>>` beside the existing lane tables, built in both
`from_bindings` and `from_profiles`.

### The signal

`consume_midi_events` (`gameplay-drums/src/lib.rs:566-607`) already emits `PadNavHit`
**unconditionally**, before the `gameplay_ready` gate — that is the existing precedent for peeling
a non-scoring signal off the NoteOn stream. System verbs follow it exactly:

```rust
for verb in resolver.system_for_note(note) {
    verbs.push(SystemVerbHit { verb });
}
```

This is why the verb fires during live play even though `menu_nav::active_context` returns `None`
there: it never travels the `NavAction` path. Pads remain gameplay input; the system note was never
gameplay input to begin with.

`SystemVerbHit { verb: SystemVerb }` is a new `Message`, registered in the drums plugin, emitted
from the `DrumsSets::Input` set. Keyboard-bound system verbs are emitted from the existing keyboard
input system on the same message.

The velocity threshold applies to system notes exactly as it does to lanes — a sub-threshold hit is
noise, and noise must not pause the song.

### Consumers

**`Pause`** — a new `system_verb_pause` in `pause.rs`, reading `SystemVerbHit`, running under
`in_state(AppState::Performance)` and `editor_closed`, sharing `toggle_pause`'s body. It toggles:
firing it while paused resumes. Setting `PracticePauseSurface::Overlay` before pausing, as
`toggle_pause` already does, so the practice rail does not steal the surface.

This one slot closes **F1 and F3 together**: it opens the overlay that already carries pad grammar
(HH/CY/BD/SD) and a Quit row, merged in `fa3041c`. There is no separate quit binding, and none is
needed.

**`Restart`** — re-requests `SongLoading` via `request_transition`, the same action the pause
menu's Retry row takes, preserving `SelectedSong` and `PracticeIntent`. It fires during
`AppState::Performance` whether running or paused.

> **Named risk, accepted.** `Restart` fires during live play, so a stray hit on the bound note
> restarts the song. This matches NX, and the note is one the user deliberately chose and can
> unbind — but it is destructive in a way `Pause` is not. If it proves annoying in practice, the
> fix is to gate it to `PauseState::Paused`, at the cost of the "restart without opening the menu"
> convenience that motivated the slot.

### Collision refusal

One pure function, the single source of truth for the rule:

```rust
/// The lane channel that already owns `src`, if any. A system verb may not
/// share an input with a lane: the same hit would both judge and fire the verb.
pub fn lane_owner(bindings: &InputBindings, src: &BindSource) -> Option<EChannel>
```

- The editor's capture path calls it and **refuses the bind**, reporting the owning lane by name.
- `BindResolver::from_bindings` / `from_profiles` also **skip** any system source that collides,
  and `warn!` once. A hand-edited `bindings.toml` therefore cannot produce a note that both judges
  and pauses. The footgun is closed at the resolver, not merely in the UI.

Lane binds are never refused — lanes win ties. The rule is one-directional and stated once.

---

## F4 — cancel a load from the kit

Independent of the bind system; needs no hardware and no new grammar.

- `menu_nav::NavContext` gains a `Loading` variant, and `active_context` gains
  `AppState::SongLoading => Some(NavContext::Loading)` (today it falls to `_ => None`,
  `menu_nav.rs:116-124`).
- `watch_cancel_key` (`game-menu/src/song_loading.rs:483-501`) gains a `MessageReader<NavAction>`
  beside its existing `ButtonInput<KeyCode>` read, and cancels on `NavVerb::Back` — i.e. **SD**.

`NavAction` carries no context field (`{verb, source, coarse}`); consumers are gated by their own
`in_state`, and `watch_cancel_key` already runs only during `SongLoading`. So no message change is
required.

SD-as-back is the established grammar everywhere else, and loading is not live play, so the
false-trigger surface is zero. The existing 500 ms `ENTER_GRACE` in `NavGuard` prevents a hit that
confirmed the song from immediately cancelling its load.

---

## Editor surface

The Controls tab gains a **System** section below the twelve lane rows, with a row per verb
(Pause, Restart). Each row behaves exactly like a lane row: keyboard and MIDI segments, Enter to
capture, Backspace to remove the last source. Capture routes through `lane_owner` and refuses a
colliding note in place.

**Pads stay excluded from Controls-tab navigation.** `pad_excluded(Controls|Widgets)` and the
`pad_exclusion_matches_controls_contract` test stand unchanged — a stray pad hit while testing
bindings must not move focus. This spec does not touch that contract.

---

## Testing

Pure, no-Bevy tests:
- `lane_owner` returns the owning channel for a lane-bound note, `None` for a free note.
- `BindResolver` skips a colliding system source; a note bound to both a lane and a verb resolves
  to the lane only, never the verb.
- A note bound only to a verb resolves to no lane.

Bevy-app tests:
- A `NoteOn` on a verb-bound note emits `SystemVerbHit` **while `gameplay_ready` is false** — the
  live-play case, which is the whole feature.
- A sub-threshold `NoteOn` on a verb-bound note emits nothing.
- `SystemVerb::Pause` toggles `PauseState` in both directions.
- A lane note never emits `SystemVerbHit` (regression: the footgun).
- `NavVerb::Back` during `SongLoading` sets `CancelRequested`.
- Serde round-trip: a `bindings.toml` with no `[system]` table loads with an empty system map.

**BRP runtime smoke is mandatory, not optional.** Each of the last three streams shipped a bug that
the entire unit suite passed straight through — black text chips, rail ghosting, and a permanently
frozen Controls consumer. Drive the real binary: bind Pause to a note, hit it mid-song, confirm the
overlay opens and the drummer can quit from the pads alone.

## Out of scope

Gesture detection of any kind. `NoteOff` consumption. CC4 / hi-hat-pedal input. Poly-aftertouch /
cymbal choke. NX's Skip, Loop, and Speed system keys. Any change to the pad-exclusion contract on
the Controls tab.
