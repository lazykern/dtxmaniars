# Extended DTX and Media Compatibility — Design

Date: 2026-07-13
Status: Approved
Program cycle: 7
Product scope: drums

## Goal

Complete the remaining drum-relevant DTXManiaNX chart and media semantics while
making format claims evidence-based. A chart or format is never labelled
supported because its extension was discovered or its channels were added to
an enum; it must survive the complete player path.

## Compatibility contract

Every chart load produces one of three outcomes:

- **Supported:** discovery, parsing, normalization, timeline construction,
  required media, play, seek/restart, and diagnostics are faithful.
- **Degraded with Warning:** judgeable timing remains faithful, but a named
  optional asset/effect is unavailable. The loading screen identifies it.
- **Rejected with Recovery:** faithful play is impossible. Loading stops before
  performance and explains the unsupported structure plus the next action.

The compatibility guide and UI use these exact terms. Unknown required
gameplay structures never silently disappear. Unknown optional directives are
retained as structured warnings with path and line when available.

## Source format and normalization

Add `ChartFormat::{Dtx, Gda, G2d}` to parsed chart provenance. All supported
sources normalize into the existing `Chart`, `Chip`, asset registry, and timing
model. `ChartFormat` is diagnostic provenance; canonical chart identity is
computed from normalized gameplay semantics so an equivalent chart does not
become a different record solely because it was converted between formats.

The library discovers `.dtx`, `.gda`, and `.g2d` case-insensitively. `.bms` and
`.bme` are detected so the player receives a specific rejection, but are not
inserted as playable songs.

## DTX metadata completion

Implement the NX level contract from
`references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:4794-4800` and
`:4838-4882`:

- `PLAYLEVEL` aliases drum `DLEVEL`;
- values 0–1000 are clamped as NX does;
- packed values of 100 or greater split into integer and decimal digits;
- `DLVDEC` explicitly supplies the drum decimal digit;
- `GLVDEC`/`BLVDEC` may be retained for provenance but do not expand the
  drums-only product scope;
- `HIDDENLEVEL`, `BACKGROUND`, `BACKGROUND_GR`, and `WALL` aliases are parsed
  into explicit metadata fields.

Song Select, loading, results, skill calculation, and score persistence consume
one normalized drum display level. Adding an alias or decimal representation
must not change canonical identity when effective gameplay content is the same.

## Drum and system channels

Expand `EChannel` only for semantics actually consumed by the drums player:

- hidden drum channels 0x31–0x3C;
- MIDI chorus 0x52 and fill-in 0x53;
- BGA swap channels 0xC4, 0xC7, 0xD5–0xD9, and 0xE0;
- click 0xEC and first-sound 0xED;
- mixer add/remove 0xEE/0xEF;
- any intermediate system value needed to preserve NX ordering without
  pretending it is judgeable.

Hidden drum chips follow NX behavior from
`references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:3084-3095`:
they remain timed, invisible, and non-judgeable, expire without a miss, and do
not contribute to note count, combo, gauge, score, density, or results analysis.
Where NX consults them for sound/empty-hit state, the sound scheduler does the
same without promoting them to visible notes.

MIDI chorus is a recognized timed no-op, matching NX's current consume-only
behavior (`CStagePerfCommonScreen.cs:3116-3122`). Fill-in retains its visual/
section marker role. Click/first-sound and mixer events are system events and
never enter judgment routing.

## Mixer lifetime

Mixer add/remove events update which chart sound handles are resident/eligible
at a chart time. They use the existing bounded sound-bank ownership rather than
creating a second audio mixer abstraction. Repeated add/remove is idempotent;
remove never stops a voice already sounding, matching the event's lifetime
purpose rather than becoming a musical choke.

Seek and practice restart rebuild mixer eligibility from events at or before
the target time. Normal forward play processes each event once. Missing slots
produce a structured optional-audio warning rather than a panic.

## BGA swaps and pan animation

Complete parser registries for the full NX `BGAPAN`/`AVIPAN` argument model:
source asset, start/end crop size, source start/end position, destination
start/end position, and movement duration. Invalid arity or numeric fields
produce line-specific parse diagnostics.

`dtx-bga` extends `TimedVisualEvent` with:

- direct layer replace;
- layer-scope swap;
- image pan/zoom;
- movie pan/zoom.

At event time, geometry interpolates in chart time, not frame count. It clamps
source rectangles to media bounds and destination rectangles to the stage safe
area. Zero duration applies the end state immediately. The implementation
follows NX event dispatch at
`references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:2980`
and `:3146`, with swaps from `:3358-3365`, while retaining the current
DTXManiaRS rendering architecture.

Practice seek, restart, and backward seek reconstruct the latest event and its
interpolated state at the target chart time. Reduced Background Motion from
Cycle 6 suppresses interpolation/movie playback but preserves the latest static
image state.

## GDA and G2D

GDA/G2D support is implemented through a dedicated front end that maps legacy
commands and channel notation to normalized DTX drum channels. It does not add
format conditionals throughout gameplay. Format-specific warnings retain the
original command and line.

Acceptance requires representative fixtures for:

- every supported drum lane and BGM;
- BPM and bar-length changes;
- WAV/OGG/MP3 assets and case-insensitive resolution;
- Shift-JIS and UTF-8 metadata;
- malformed commands and unsupported channels;
- score/note-count equivalence with a normalized DTX fixture.

`references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:3204-3207` and
`references/DTXmaniaNX/DTXMania/SongDb/SongDb.cs:282-285` establish that NX
recognizes these extensions, but
DTXManiaRS support is granted only by the fixture contract above.

## Explicit BMS/BME rejection

BMS and BME are keyboard-oriented formats outside the approved drums product.
The scanner detects them and reports `Unsupported chart format: BMS/BME is not
supported by the drums player`; archive import reports the same outcome. They
are not parsed as DTX, do not appear as zero-note songs, and are not called
supported in documentation.

## XA recovery

XA decoding is not added. NX relies on native `bjxa` through
`references/DTXmaniaNX/FDK/Sound/Cxa.cs`; this workspace forbids unsafe code in
library crates and has no compatible decoder backend.

When a chart references `.xa`, resolution checks for a case-insensitive
same-stem `.ogg`, `.wav`, then `.mp3` fallback. If found, it is used and the
load report records the substitution. Without a fallback:

- an XA BGM rejects the load because silent rhythm playback is not faithful;
- optional SE/preview/media becomes Degraded with Warning and names the slot,
  path, and conversion guidance;
- archive import succeeds but reports that conversion is required before the
  affected chart can play faithfully.

No native library is vendored and no external converter is executed
automatically.

## Playback-rate scope

The current NX-compatible resampling behavior remains the default and changes
pitch with speed. Pitch-preserving time stretch is not exposed in this cycle:
the current audio path has no backend that can guarantee one shared position
across BGM, chart sounds, seeking, practice loops, and rate changes. A future
backend must first pass sync drift, seek, loop-boundary, CPU, and artifact
benchmarks. Documentation states the present behavior plainly.

## Deliberate exclusions

- Guitar/bass long-note and expanded fret-channel gameplay.
- BMS/BME gameplay.
- XA decoding or automatic conversion.
- Folder/box hierarchy and a song cache; Cycle 5 found no demonstrated need.
- Embedded movie audio and hardware zero-copy video.

## Error handling

- Required unsupported structures reject before entering Performance.
- Optional failures carry `path`, `line`, `kind`, and recovery text through scan
  and load reports.
- Multiple occurrences are grouped in player UI but remain individually logged.
- Seek/restart encountering an invalid visual/mixer event skips only that
  optional event and preserves the faithful gameplay timeline.
- A normalized format never panics on a raw channel it does not understand.

## Compatibility matrix and tests

Extend the matrix so each row declares expected discovery, parse, load,
playback/render, and diagnostic outcome. Include:

- PLAYLEVEL, packed level, DLVDEC, aliases, and identity stability;
- every added channel plus malformed/unknown variants;
- hidden-chip exclusion from every gameplay/stat total;
- MIDI-chorus no-op and fill-in behavior;
- mixer forward play, repeated events, backward seek, and practice restart;
- BGA swap and pan state at start/mid/end/zero duration and after seek;
- GDA/G2D equivalence and malformed diagnostics;
- BMS/BME discovery rejection;
- XA fallback priority, required-BGM rejection, and optional warning;
- encoding, case, BPM/bar changes, and mixed media regressions.

Package gates cover `dtx-core`, `dtx-library`, `dtx-assets`, `dtx-audio`,
`dtx-bga`, `game-menu`, and `gameplay-drums`, followed by the workspace local
quality gates.

## Acceptance criteria

- Every support claim has an end-to-end fixture row.
- GDA/G2D drum charts normalize and play equivalently to their DTX fixtures.
- BMS/BME and unsupported XA never fail silently.
- Added system/hidden channels cannot pollute scoring or analysis.
- Pan/swap/mixer state is deterministic under normal play, seek, and practice.
- The published compatibility guide matches executable outcomes.
