# Atomic Multi-Target Input Bindings

Date: 2026-07-12
Status: Approved

## Goal

Replace shared-binding fan-out with one atomic input event. A key or MIDI note may accept several drum channels, but one physical press consumes at most one chart chip. Keep binding profiles, judgment rules, and visual lane profiles independent.

## Problem

The current runtime emits one `LaneHit` for every channel that owns a source. A key assigned to BD and LBD emits two hits, so both notes can disappear from one press. The Controls UI calls this a shared binding, which does not tell the player that the runtime treats it as a chord macro.

Lane profiles add a second source of confusion. A lane may display several channels in one column, but that layout choice should not change which inputs judge those channels.

## Decisions

| Area | Decision |
|---|---|
| Physical input | One key press or MIDI NoteOn emits one atomic input event. |
| Multiple targets | A source has one primary channel and zero or more ordered alternates. |
| Consumption | A multi-target event judges at most one chart chip. |
| Selection | Choose the eligible chip with the earliest playback time; the primary channel wins an exact-time tie. |
| Drum groups | Explicit alternates override a `Separate` group for that source. Ordinary single-target inputs retain the ported group behavior. |
| Visual lanes | Lane merging, order, width, and visibility affect rendering only. |
| Terminology | The UI uses “Primary pad” and “Also accepts” instead of “shared binding.” |

## Model boundaries

```text
Binding profile          Judgment policy             Lane profile
physical source          playable chart targets      rendering only
key/note -> targets      choose <= 1 chip            channels -> columns
```

`dtx-input` owns profile loading, validation, source lookup, and physical input events. `gameplay-drums` owns chart-aware target selection. `dtx-layout` owns channel placement and does not participate in input resolution.

## Binding schema

Persist bindings by source so the schema can store target priority without relying on map iteration order.

```text
KeyboardBinding
  source: KeyCode::Space
  targets: [BassDrum, LeftBassDrum]

MidiBinding
  source: note 36
  targets: [BassDrum, LeftBassDrum]
```

The first target is primary. The remaining entries are accepted alternates. Each profile enforces these invariants:

- each keyboard key or MIDI note appears in one binding record;
- each target list is non-empty, ordered, and duplicate-free;
- every target names a bindable drum channel.

Runtime code may wrap the non-empty target list in a dedicated type so callers cannot construct an invalid `InputHit`.

## Runtime event

Keyboard and MIDI sources emit the same event shape:

```text
InputHit
  targets: [BD, LBD]
  audio_ms: 123456
  kind: press
  source_kind: keyboard
```

`source_kind` supports diagnostics and feedback but does not affect scoring. One physical transition creates one event. Input capture keeps the existing compensated audio timestamp behavior.

Autoplay remains chart-channel based. It does not create multi-target physical input events.

## Judgment flow

The judge selects a path from the event target count:

```text
InputHit [BD]                  InputHit [BD, LBD]
      |                               |
existing drum-group resolver          explicit-target resolver
      |                               |
ported group result                   zero or one result
      +---------------+---------------+
                      |
               JudgmentEvent
```

A single-target event uses the existing BocuD-derived drum-group resolver. This preserves ordinary HH, cymbal, tom, and pedal grouping, including its existing tie behavior.

For a multi-target event, the explicit-target resolver:

1. Finds the closest unjudged chip inside the timing window for each target.
2. Applies the practice wait-mode halted-set filter before choosing a chip.
3. Selects the candidate with the earliest playback time.
4. Uses target order to break exact-time ties, so the primary wins.
5. Marks one chip judged and emits one `JudgmentEvent`.

If no target has an eligible chip, the judge emits one `EmptyHit` for the primary channel. Explicit targets apply even when the corresponding global drum group is `Separate`. They do not modify the group configuration or affect other controls.

Two physical inputs pressed together can judge both notes in a BD and LBD chord. One multi-target input pressed once cannot.

## Feedback and sound

- Immediate physical-input feedback flashes the primary pad once.
- A successful judgment animates and sounds the consumed chip’s channel.
- An empty hit uses the primary pad’s existing sound fallback policy.
- The Customize inspector highlights all targets while the player inspects a binding.
- A live test press in Customize flashes the primary once.

These rules prevent fan-out in scoring, lane flashes, and hit sounds.

## Controls UI

The Controls tab may keep channel rows as an inverted view of the source-centric data. Selecting a binding opens its source detail:

```text
SPACE
Primary pad       BD
Also accepts      LBD
Behavior          One press -> one note
```

The editor supports four operations:

- **Set primary** changes the first target and retains selected alternates.
- **Also accept here** appends the current channel as an alternate.
- **Remove target** removes one channel. Removing the primary requires confirmation and promotes the first alternate.
- **Move here only** makes the current channel primary and removes all alternates.

A multi-target binding shows a linked marker and a `1x` badge. Its tooltip reads: “One press accepts BD or LBD; only one note is judged.” Capture uses the same operations for keyboard and MIDI.

## Lanes UI

The Lanes tab calls its channel list “Displayed channels.” It does not show binding ownership or mutate controls.

```text
Merged lane: [BD + LBD]
Bindings:    unchanged
Judgment:    unchanged
```

BD and LBD remain independently playable when a lane displays both. A consumed note animates in the column that displays its chart channel. Separate visual columns also work with one multi-target binding.

## Migration and failure handling

Schema v2 stores source-centric records. The loader continues to read v1 channel-centric profiles. During v1 migration, canonical BocuD channel order selects the primary target and orders the alternates.

The first migrated session shows the converted profile as dirty and asks the player to save it. Loading does not rewrite the source file. A successful save writes v2 through the existing atomic persistence path. A conversion or save failure leaves the last valid file untouched.

The loader rejects empty target lists, duplicate targets, duplicate source records, and unknown channels with a clear profile error. Existing startup policy supplies built-ins when a profile cannot load.

## Testing

Pure resolver tests cover:

- BD-only and LBD-only eligible notes for `[BD, LBD]`;
- simultaneous BD and LBD notes choosing primary BD;
- an earlier LBD note winning over a later BD note;
- no eligible target producing one empty hit;
- two physical events consuming up to two notes;
- explicit alternates working with `BdGroup::Separate`;
- ordinary single-target inputs retaining existing group behavior;
- practice wait mode filtering candidates before selection.

Profile and editor tests cover:

- identical atomic target order from keyboard and MIDI profiles;
- set-primary, add-alternate, remove, promote, and move-only reducers;
- deterministic v1-to-v2 conversion without a load-time write;
- rejection of empty, duplicate, and unknown targets;
- save failure preserving the previous file.

Integration tests cover:

- one BD/LBD multi-target press consuming one of two simultaneous notes;
- two distinct controls consuming both simultaneous notes;
- lane merge, reorder, resize, and visibility changes producing identical judgment results;
- one flash and one hit sound per physical multi-target press.

## Compatibility note

DTXManiaNX-BocuD may process several tied chips for one grouped pad. DTXManiaRS retains that behavior for ordinary drum-group resolution. Multi-target bindings add an accessibility mapping that BocuD does not model, so they use the approved one-press, one-chip rule. This exception must be recorded alongside the implementation’s reference citations.

## Out of scope

- Changes to chart parsing, scoring windows, or score formulas.
- Changes to global drum-group options.
- Automatic binding changes when the player merges or splits visual lanes.
- Chord macros that consume several notes from one physical press.
