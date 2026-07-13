# Distant-kit control — research

**Date:** 2026-07-13
**Status:** research complete; no design approved
**Supersedes:** the "Stream 5" section of `2026-07-12-streams-research.md`, whose framing (find a low-false-positive *gesture*) this document rejects.

Audit findings in scope: **F1** (pause from the kit during live play), **F3** (quit from the kit),
**F4** (cancel a load from the kit). F2 (practice trap) was already closed by the pause
unification merged in `fa3041c`.

---

## Conclusion first

**The gesture premise was wrong.** Three independent lines of research — the reference
implementation, the peer genre, and our own MIDI stack — converge on the same answer:

> Pause from the kit is a **binding**, not a gesture. DTXManiaNX already ships it that way.
> No drum game in the genre implements a pad gesture that pauses live gameplay.

The work F1 actually needs is a **non-lane bind target**. Everything else — hold detection,
double-hit windows, chord matching, `NoteOff` consumption, CC4 plumbing — is unnecessary.

F3 falls out of F1 for free. F4 is an unrelated one-line grammar extension.

---

## 1. What DTXManiaNX does (verified against C# source)

Source: `github.com/limyz/DTXmaniaNX` @ master. (`references/` is empty in this repo; the
source was cloned to scratch for this study, not vendored.)

**Pause is a pad-bindable key-assign slot.**

- Config row: `Stage/04.Config/CActConfigList.cs:2078` —
  `CItemBase("Pause", …, "Pause key assign:\n To assign key/pads for Pause button.")`
- The wording is deliberate. The *Capture* row directly above it reads: *"You can use keyboard
  only. You can't use pads to capture screenshot."* (`:2065`). Pads-for-Pause is an explicit,
  documented capability.
- Capture accepts keyboard / **MIDI-in** / joypad / mouse (`CActConfigKeyAssign.cs:128`).
- Runtime: `Stage/07.Performance/CStagePerfCommonScreen.cs:2347` —
  `if (CDTXMania.Pad.bPressed(EInstrumentPart.BASS, EPad.Help)) { this.bPAUSE = !this.bPAUSE; }`
- Default is keyboard (`Pause=K0110`, `CConfigIni.cs:4240`).
- Storage quirk: the slot is aliased onto the Bass part's `Help` pad slot, not onto a SYSTEM block.

**The catch, and why nobody uses it:** `CConfigIni.cs:1524 tDeleteAlreadyAssignedInputs()`
removes an input from every other slot when you assign it. In NX, binding Pause to a pad
**sacrifices that pad from gameplay.** That is the whole reason the default is keyboard.

**NX has no pad quit.** Abort is hardcoded `keyboard.bKeyPressed(SlimDXKey.Escape)`
(`CStagePerfCommonScreen.cs:2402`), and it is gated on `!bPAUSE` — you cannot even quit *while*
paused. NX has **no pause menu**; pause is a pure freeze.

**NX has a gesture engine — used only in menus.** `CCommandHistory`
(`CStageSongSelection.cs:935-1010`): a 16-entry pad-history ring, `EPadFlag` sequence matcher,
each step must be ≤500 ms from the last, history cleared on match. Drives `BD×2` → quick config
and `HH×2` → difficulty change. It is **never wired to the performance stage.**

NX's menu pad grammar (GITADORA-derived, per its own code comments): `HT`/`LT` up/down,
`CY`/`RD` confirm, `LC` back.

**ADR-0010 bearing:** pad-bindable pause *is* NX mechanics. A pause gesture is not.

---

## 2. What the peer genre does

| Game | Kit-only pause | Mechanism |
|---|---|---|
| Rock Band / Guitar Hero | Yes | **Physical Start button on the kit brain.** |
| Harmonix MIDI Pro-Adapter | Yes | A box of face buttons — shipped *because* a MIDI kit has none. |
| GITADORA (arcade) | **No player pause at all** | Coin-op. Cabinet START/select panel for menus. |
| Clone Hero + MIDI kit | No | Wiki: map a spare pad to Start, *"or keep a keyboard on standby so you can pause."* |
| Melodics | Gesture — but **restart**, not pause | Four-limb simultaneous chord (toms + both pedals). False-trigger immunity by physical improbability. |
| Aerodrums | Gesture — opens a **menu**, not pause | Dwell-to-arm in empty air, then strike. |

Ranked by frequency, the industry's answers are: **(1) dedicated physical button**,
**(2) accept that the keyboard is required**, **(3) spend an instrument input on it**. An
explicit modal arming state appears **nowhere**. Hold-to-confirm on drums appears **nowhere** —
consistent with §3, since it is not physically expressible.

No product implements a pad gesture that pauses live gameplay. Community search (Reddit
r/edrums, vdrums, DTXMania forums, GitHub) found **no player asking for one**.

---

## 3. What the hardware can actually emit

Verified twice, independently, against manufacturer documentation.

### Holding a pad is not merely unreliable — it is unrepresentable

A pad is a **piezo transient sensor**. It reports the impact spike. There is no contact or
pressure sensing on the head, so the module has nothing to tell it when the stick leaves.
The `NoteOff` is fired by a **timer inside the module** — the per-pad **Gate Time** parameter.

- Roland TD-6V manual p.65: *"For each pad, you can specify the length of time the note will
  'hold' during transmission from the MIDI OUT… At the factory settings, the Gate Time setting
  is set to the minimum value."* Range **0.1–8.0 s**, a user setting.
- Yamaha DTX-MULTI / DTX-PRO: *"Gate Time determines the length of time from Key On to Key Off"*,
  **0.0–9.9 s**.

So `NoteOff_time − NoteOn_time` measures **a config value on the drummer's module** — identical
for a 5 ms tap and a stick left resting for three seconds. A resting stick emits nothing at all.

Second, independent reason to never build on `NoteOff`: TD-9KX users report **no NoteOff is sent
at all** when two pads are struck simultaneously.

**Consuming `NoteOff` in `consume_midi_events` recovers no information the hardware ever sent.**

### Free input space — a chart can only produce lane notes

A DTX chart has 12 channels. These arrive from a real kit and **cannot** be charted:

| Signal | Emitted by | Reaches our parser? |
|---|---|---|
| **Unbound zone notes** — xstick 37, ride bell 53, HH edge 22/26, tom3 rim 58, aux 27/28 | Every kit (TD-17 default map) | **Yes** — parses fine, then dropped for want of a lane |
| Yamaha foot splash — note 83 | Yamaha DTX | **Yes** — same |
| Poly aftertouch `0xA0` (cymbal choke) | Roland, Yamaha, 2box, ATV; **not** portable | **No** — dies at `_ => None` |
| CC4 (hi-hat pedal position) | Roland/Yamaha/Alesis; TD-17 caps at **90**, not 127 | **No** — dies at `_ => None` |

Notes 22, 26, 27, 28, 37, 53, 58, 83 are **not in our default bind map** and are free today.

---

## 4. What our code does today

- **`dtx-input/src/midi.rs:111-132`** — `midi_bytes_to_event()`. `0x90` vel>0 → NoteOn;
  `0x90` vel 0 and `0x80` → NoteOff; **everything else → `_ => None`.** ControlChange, poly
  aftertouch, and channel aftertouch are destroyed at parse. A test enshrines it:
  `non_note_bytes_ignored` asserts `[0xB0, 4, 127] → None` (`:308-311`). The
  `MidiEvent::ControlChange` variant exists but is **never constructed from real bytes** —
  its only construction site in the workspace is a unit test.
- **`midi.rs:207`** — `mi.ignore(midir::Ignore::None)`. midir filters nothing; every byte the
  module sends reaches the parser. **The parser is the only filter.**
- **`midi.rs:115`** — `bytes[0] & 0xF0` discards the channel nibble. Nothing downstream
  distinguishes ch10 from ch1.
- **`gameplay-drums/src/lib.rs:566-607`** — `consume_midi_events` matches `NoteOn` only;
  NoteOff and CC hit `else { continue }`.
- **`lib.rs:594-595`** — an unbound note yields no lane, so no `InputHit` and no `PadNavHit`.
  It is **silently ignored** (asserted: `bindings.rs:385`, `lanes_for_note(99).count() == 0`).
- **`dtx-input/src/bindings.rs:37-42`** — `BindSource::{Key, Midi{note}}`; the map is keyed by
  `EChannel`, and `BindResolver` iterates only `BINDABLE_CHANNELS` — the 12 lanes (`:19-32`).
  **There is no non-lane bind target anywhere in the codebase.** This is the gap.
- **`gameplay-drums/src/menu_nav.rs:116-124`** — `active_context` returns `None` during unpaused
  `Performance`, so pads are gameplay input and nothing else. And `SongLoading` has no arm at
  all — it falls to `_ => None`.
- **`gameplay-drums/src/pause.rs`** — `toggle_pause` reads `KeyCode::Escape` only.
- **`game-menu/src/song_loading.rs:483-501`** — `watch_cancel_key` reads
  `ButtonInput<KeyCode>` only; no `PadNavHit` reader.

Pads can already *drive* the pause menu (`menu_nav.rs:42-51`, 80 ms debounce, 500 ms enter-grace).
They just cannot *open* it. **That single missing edge is the whole of F1.**

---

## 5. Chart-corpus measurements (kept for the fallback path)

Measured because the original framing needed it. Superseded as a design input, but the numbers
are sound and are the evidence base if a gesture is ever revisited.

Corpus: 15 charts / 4 song folders, **14,563 notes, 39.4 chart-minutes**, J-rock, 157–187 BPM.
Timing cross-validated against an independent grid integrator (max deviation 0.000 ms).

**Chord sizes** (20 ms clustering, 9,949 chords): 1 pad 58.5% · 2 pads 37.4% · 3 pads 3.4% ·
4 pads 0.8% · **5+ pads: never**.

**Lane pairs that never co-occur** — and how close they ever got:

| Lane pair | Occurrences ≤50 ms | Closest approach |
|---|---|---|
| HHC + RD | 0 | **2333 ms** |
| FT + RD | 0 | 646 ms |
| HHC + CY | 0 | 333 ms |
| HHC + FT | 0 | 287 ms |
| FT + CY | 0 | 80 ms — thin |

The margin, not the zero, is the robust signal: a right hand cannot be on the hi-hat and a
cymbal at once, so charters put crashes and toms in fills where the hats rest.

**Same-lane repeats:** SD is hopeless (907 natural sub-100 ms pairs, min IOI **41.7 ms**).
Clean doubles: **crash twice within 150 ms = 0**, left-crash = 0, hi-hat pedal min IOI **333 ms**.

**Two traps this data sprang:**

1. Every zero-collision set containing SD from the original shortlist ({SD,FT,CY}, {HH,SD,FT},
   {SD,BD,CY,FT}) is zero **because it needs three hands** — unplayable, not safe.
2. `verb_for_lane` **merges** HHC+HHO and CY+RD. That merge *costs margin*: the HHC+RD pair's
   2333 ms separation collapses to a ~160 ms nearest-miss once collapsed into verb groups, and
   the one clean double (crash) is polluted by 48 ride-16th collisions.

**Arming-on-rest is not viable:** rests >2 s are 4.9% of chart time, and Tsukinami has **zero**
gaps over 1 s across all four difficulties.

**Sample limits, stated plainly:** four songs, one genre. "Zero in 39 minutes" is not "never."
A ride-and-crash accent is idiomatic and would kill CY+RD; double-kick metal would kill any
BD-repeat gesture.

---

## 6. Where this leaves F1 / F3 / F4

Not a design — the shape the evidence points at, for a design cycle to accept or reject.

**F1 — pause from the kit.** Add a **non-lane bind target** and route it to `toggle_pause`.
This is what NX does, it is the only thing the genre does, and it has no false-positive surface
to reason about. We can improve on NX: because `tDeleteAlreadyAssignedInputs` forces NX users to
sacrifice a gameplay pad, nobody uses the feature — but binding to an **unbound zone note**
(xstick, ride bell) costs nothing, since a 12-channel chart cannot address it. Ships unbound by
default; Escape keeps working.

**F3 — quit from the kit.** No separate work. NX has no pad quit *and no pause menu*. We have a
pause menu with pad grammar and an exit row already (merged in `fa3041c`). Open it from the kit
and quit is reachable.

**F4 — cancel a load.** Independent of all the above and needs no hardware. `SongLoading` has no
`active_context` arm; `watch_cancel_key` is keyboard-only. SD-as-back is the established grammar
and loading is not live play, so the false-trigger surface is zero.

**Explicitly not worth building:** hold gestures (physically unrepresentable), `NoteOff`
consumption (carries no player information), CC4 plumbing (needed only for a gesture we no longer
need), chord/double-hit detection (a binding is strictly better). The §5 tables stand as the
fallback if a kit turns out to have no spare zone note.

**No hardware session is required to proceed.** The Session C protocol
(`2026-07-11-player-user-stories.md` §11) was gated on choosing a gesture. There is no gesture.

---

## 7. Unrelated defect found during this study

The three **`546 - TOGENASHI TOGEARI`** charts in the local song library are **UTF-16LE with BOM**.
`decode_dtx_text` (`dtx-core/src/parser.rs:64`) tries UTF-8, then Shift-JIS. Neither handles
UTF-16, so **all three charts parse to zero chips** — 3,736 notes, a song that loads silently
empty. Found independently by two analyses. Not fixed here; deserves its own change.
