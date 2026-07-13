# Compatibility

Purpose: the maintained executable support contract for discovered chart
formats, media, and recovery behavior.

Audience: players diagnosing a chart and contributors changing parsers,
loading, audio, visuals, or import.

Status: Maintained and backed by the executable tests listed below.

Neighboring guides: [player guide](player-guide.md),
[data and persistence](data-and-persistence.md), and
[contributing](contributing.md).

This page describes what the drums player actually discovers, parses, loads,
and plays. A filename extension by itself is not a support guarantee.

## Support states

- **Supported** — the tested drums contract is preserved through discovery,
  parsing, loading, gameplay, seeking/restart, and applicable rendering.
- **Degraded with Warning** — the drum timeline remains playable, but optional
  audio or visual media is missing, unsupported, or substituted. Song Loading
  shows the problem before play.
- **Rejected with Recovery** — faithful drums play is not possible. Song
  Loading returns to selection after showing the reason and recovery action.

## Chart formats

| Format | State | Contract and recovery |
|---|---|---|
| DTX (`.dtx`, any extension case) | Supported | Drums, timing, supported system channels, registered media, and supported BGA operations use the contract below. |
| GDA (`.gda`, any extension case) | Supported | Dedicated GDA channel aliases normalize to the same drums/timing model as equivalent DTX. Unknown aliases produce line diagnostics. |
| G2D (`.g2d`, any extension case) | Supported | Uses the same dedicated legacy alias normalization and executable equivalence contract as GDA. |
| BMS / BME | Rejected with Recovery | Keyboard-oriented BMS/BME gameplay is not implemented. Convert the chart to DTX, GDA, or G2D. |
| Other extensions | Not discovered | Rename only when the file really contains a supported format; otherwise convert it. |

Supported text encodings are UTF-8, Shift-JIS, UTF-16LE with BOM, and UTF-16BE
with BOM. Windows path separators and case differences in chart-relative asset
names are resolved component by component.

The parser supports deterministic conditional branches (`RANDOM`, `IF`, and
`ENDIF`), BPM changes, bar-length changes, `DLEVEL` / `PLAYLEVEL`, `DLVDEC`,
hidden-level metadata, WAV/BMP/AVI registrations, and SE01 through SE32.
Malformed recoverable directives produce structured line warnings; a selected
branch with no playable drum notes is rejected at load time.

## Drums and chart-time operations

The playable drums contract covers HH, SD, BD, HT, LT, CY, FT, HHO, RD, LC,
LP, and LBD. Hidden counterparts on channels `31`–`3C` update timed sound-lane
state but never render as notes, enter density, affect scoring, or require a
judgment.

The following non-note operations are consumed on the same chart clock and are
reconstructed after backward seek, forward seek, loop restart, and song
restart where applicable:

- MIDI chorus (`52`), fill-in (`53`), click (`EC`), and first-sound (`ED`);
- mixer add (`EE`) and remove (`EF`), with removal changing future eligibility
  without choking an already-playing voice;
- BGA swaps (`C4`, `C7`, `D5`–`D9`, `E0`);
- BGA/AVI pan definitions, including chart-time interpolation and bounded crop
  and destination geometry.

Invalid optional visual events are warned about and skipped without changing
the gameplay timeline. Reduced Background Motion resolves image pans to their
static end state and does not start movies.

## Media

| Media | State | Behavior |
|---|---|---|
| OGG, WAV, MP3 chart audio | Supported | Extensions and chart-relative filenames are matched case-insensitively. Decoder failures are diagnosed during Song Loading. |
| XA with same-stem OGG/WAV/MP3 | Degraded with Warning | XA is not decoded. The player substitutes, in order, same-stem OGG, then WAV, then MP3, with case-insensitive matching. |
| XA required by BGM, no fallback | Rejected with Recovery | Provide a same-directory, same-stem OGG, WAV, or MP3 file. The game and archive importer never run an XA converter. |
| XA used only by optional SE, no fallback | Degraded with Warning | The chart remains playable without that optional sound; provide a supported same-stem file to restore it. |
| Missing optional audio/image/movie | Degraded with Warning | The drum timeline remains playable and the missing path is reported. |
| Registered BGA images | Supported when the image decoder accepts the file | Image replacement, layers, swaps, pan geometry, seek reconstruction, alpha, and accessibility motion settings are applied. |
| Registered AVI/movie media | Supported when FFmpeg accepts the file | Decode runs off the main thread and follows chart time. Decoder errors are logged and gameplay continues without the movie. |

Archive import uses the same XA priority and required/optional classification.
It reports substitutions and unresolved XA references but never modifies or
converts archive media.

## Playback rate and score truth

Normal Play Speed and practice tempo move notes, chart sounds, BGM, BGA,
seeking, and stage completion on one effective chart-time rate. The current
implementation changes audio playback rate directly, so speeds other than
`1.00×` also change pitch. There is no pitch-preserving time-stretch mode.

A modified-speed normal result remains visible, but it does not write normal
history, personal-best/rank data, or compatible `score.ini` records. Practice
and No Fail results are also non-qualifying.

## Executable evidence

The declared contract is guarded by:

- `crates/dtx-core/tests/compatibility_matrix.rs` for encodings, format
  normalization, aliases, timing/media parsing, channels, and rejection
  boundaries;
- `crates/gameplay-drums/tests/system_events.rs`, `mixer_events.rs`, and
  `play_chart.rs` for non-note consumption, seek reconstruction, and XA load
  policy;
- `crates/dtx-bga/tests/integration_bga.rs` for pan/swap rendering and seek
  reconstruction;
- `crates/dtx-library` scanner/import tests for case-insensitive discovery,
  explicit BMS/BME rejection, and archive XA diagnostics.
