# Player Trust and Core DTX Compatibility Design

Date: 2026-07-13
Status: Program direction approved; written design pending review
Program: Cycles 1–2 of `docs/notes/2026-07-13-game-improvement-program.md`

## Goal

Make a supported DTX chart play the same timeline the player sees, keep
modified-speed results out of ordinary records, discover charts regardless of
filename case, and support common conditional/audio content without silent
corruption.

## Scope

This design includes:

1. one rate contract for normal play and practice;
2. modified-speed result classification and persistence protection;
3. case-insensitive `.dtx` discovery;
4. `#RANDOM`, `#IF`, and `#ENDIF` parsing;
5. SE01–SE32 channel support;
6. MP3 decoding and BGM discovery;
7. structured scan/load diagnostics;
8. a compatibility fixture matrix and integration tests.

It does not include GDA/BMS/BME, XA, pitch-preserving time stretch, extended
results analytics, calibration redesign, library filters, BGA swap/pan
semantics, hidden drum channels, or CI/CD. Those remain recorded in the master
program and are not dropped.

## 1. Playback-rate contract

### Authoritative coordinate

All gameplay systems use **chart milliseconds**. Chip target times are computed
once from BPM and bar-length data and are never divided by Play Speed. The
effective playback rate controls how quickly the gameplay clock advances
through chart milliseconds.

At rate `r`:

- `GameplayClock` advances by `wall_delta × r`;
- the main Kira audio channel and tracked BGM instance play at `r`;
- BGM/SE schedulers compare unscaled chip chart times to the accelerated clock;
- note scroll, judgment, BGA, metronome, stage completion, and HUD progress read
  the same accelerated chart clock;
- seeking accepts chart milliseconds and seeks audio to the corresponding
  source position without dividing the requested chart position;
- pause freezes both chart clock and audio; resume preserves the rate;
- restart rebuilds the same rate before audio begins.

This matches the architecture already used by practice tempo. The current
`ScrollSettings::play_speed` target-time division is removed. ScrollSettings
returns to visual scroll velocity only.

### Rate ownership

Introduce a single resource with explicit source:

```text
EffectivePlaybackRate {
    value: f64,
    source: Native | NormalPlaySetting | PracticeTempo,
}
```

- Normal play reads the persisted Play Speed and selects `Native` at 1.0 or
  `NormalPlaySetting` otherwise.
- Practice ignores the normal Play Speed value. Its existing user/ramp tempo
  selects `PracticeTempo` and defaults to 1.0.
- Entering Performance applies the initial rate before any chart audio starts.
- Practice tempo changes update the same resource and audio path.
- Exiting Performance resets the audio channel and resource to 1.0.

Pitch changes with speed in this cycle, matching NX's default Play Speed mode.
Pitch-preserving time stretch remains a later program item.

Before Performance transitions to StageClear or StageFailed, gameplay writes an
immutable `CompletedRunContext` in `game-shell` containing the run kind and the
effective rate. It survives the intermediate banner and is replaced only when
the next run begins. Results eligibility is derived from this snapshot, not
from the live rate resource, because the latter is reset on Performance exit.

### Drift correction

Kira's measured stream position is source/chart position. It is therefore
already expressed in accelerated chart milliseconds. The free-running clock
uses `wall_delta × rate`; first audio observation snaps to `GameStartMs +
position`, and later observations retain the existing bounded drift correction.
No second rate multiplier is applied to measured positions.

## 2. Modified-speed score integrity

The result screen always appears, regardless of rate. A normal play with rate
1.0 keeps the existing save behavior. A normal play with any other rate in its
`CompletedRunContext` receives `SaveStatus::ModifiedSpeed { rate }` and performs
no native-store or compatible `score.ini` write.

The results status reads, for example:

```text
0.75× play speed — result not saved as a normal record
```

Retry preserves the selected normal Play Speed. Entering Practice resets to
practice tempo 1.0. Practice remains `SaveStatus::Practice` and never writes a
normal result.

No score-store migration is needed because modified records are not inserted.
Existing records remain valid. A modifier-aware history schema is deliberately
deferred until the product needs assisted-run history.

## 3. Conditional DTX parsing

### API

Keep `parse(reader) -> Result<Chart>` for current callers. Add:

```text
ParseOptions { random_seed: u64 }
ParseReport { chart: Chart, warnings: Vec<ParseWarning> }
parse_with_options(reader, options) -> Result<ParseReport>
```

`parse` generates a per-call, process-local seed and returns only the chart.
Tests and callers that need reproducibility use `parse_with_options`; production
code must not depend on receiving the same branch from two separate parses.

### State machine

Preprocessing happens during the single line pass, before metadata, asset, or
chip parsing:

- `#RANDOM n` selects an integer in `1..=max(n, 1)`;
- `#IF n` pushes whether that branch is inactive, also respecting an inactive
  parent;
- `#ENDIF` pops one conditional level;
- any ordinary line under an inactive level is ignored completely;
- nesting is supported to the NX limit of 255;
- an unmatched `#ENDIF`, invalid argument, or unclosed `#IF` produces a
  structural warning but does not crash the library scan.

Directive recognition accepts the spacing/parameter form used by NX, not only
the colon form used by ordinary metadata. Exactly one selected branch reaches
the existing parser.

The selected branch is runtime content. Song-list metadata remains based on its
own scan parse; gameplay always uses the chart produced during SongLoading.

## 4. Channel and audio compatibility

### SE channels

Extend `EChannel` with SE06 through SE32 using the exact NX numeric values. Add
`EChannel::is_se()` and replace five-variant matches in classification,
preloading, scheduling, seeking, and tests. Every modeled SE channel uses the
existing chart-audio volume, pan, and replacement/voice behavior unless the NX
reference defines a distinct rule.

Unknown channels remain forward-compatible and are skipped; the compatibility
fixtures ensure known supported channels cannot regress into that path.

### MP3

Enable the existing `bevy_kira_audio` MP3 feature. Treat `.mp3` as a supported
chart audio extension everywhere OGG/WAV is currently accepted:

- explicit `#WAVxx` assets;
- `#BGMWAV` resolution;
- conventional fallback names such as `drums.mp3`, `bgm_d.mp3`, `bgm.mp3`,
  `1.mp3`, and `<chart-stem>.mp3`;
- preview playback and chart-sound preloading.

Case-insensitive nested asset resolution remains authoritative. XA is reported
as unsupported rather than passed to the decoder as if it were playable.

### Chart discovery

The library recognizes `.dtx` with ASCII case-insensitive comparison. This
change applies to recursive scans, rescan, and archive-import selection. GDA,
BMS, and BME remain unsupported in this cycle and must not be advertised.

## 5. Diagnostics and failure behavior

Library scanning returns a `ScanReport` containing elapsed time, discovered
chart count, loaded chart count, and structured problems with path and reason.
`SongDb` retains the latest report.

The song-select surface shows a compact summary only when problems exist, for
example `3 charts skipped — view log`. The existing notification system reports
rescan/import outcomes. Detailed paths and causes remain in logs for this cycle;
a full in-game problem browser is deferred.

SongLoading distinguishes:

- parse failure;
- missing audio;
- unsupported audio format;
- decoder failure;
- missing visual media that does not prevent gameplay.

Parse failure and a selected branch with no playable drum chips are fatal and
show the reason before returning to song select. Missing, unsupported, or
undecodable referenced audio is a visible warning but does not block a chart
that can otherwise be played; DTX charts are allowed to be intentionally
silent or keysound-only. Optional visual-media failures are also non-fatal. A
malformed conditional structure adds a warning rather than crashing the scan.

## 6. Component boundaries

| Component | Responsibility |
|---|---|
| `dtx-core` | Conditional preprocessing, complete SE identifiers, parse reports |
| `dtx-library` | Case-insensitive discovery, scan report aggregation |
| `dtx-audio` | MP3 decode feature and supported-format classification |
| `game-menu` | Scan/load problem summaries and fatal loading messages |
| `game-shell` | Cross-stage immutable completed-run qualification |
| `gameplay-drums` | Unified rate application, unscaled chart-time scheduling |
| `game-results` | Modified-speed save policy and explanation |
| `dtx-scoring` | No schema change; verifies no modified entry is inserted |

No Pure crate gains a Bevy dependency. Diagnostics are plain data until an
Engine/Game layer renders or logs them.

## 7. Verification

### Pure unit tests

- rate 0.5, 1.0, and 2.0 leave chip target chart times unchanged;
- the free-running clock advances by effective rate and accepts measured source
  position without double scaling;
- deterministic seeds select the expected conditional branch;
- inactive nested branches produce no metadata, assets, or chips;
- unmatched/invalid conditional directives produce warnings;
- all SE01–SE32 numeric values and `is_se()` classification match NX;
- extension matching accepts `.dtx`, `.DTX`, and mixed case.

### Fixture and integration tests

- conditional chart with mutually exclusive drum branches;
- nested conditional chart;
- uppercase filename discovery on a case-sensitive filesystem;
- high-numbered SE playback scheduling;
- MP3 preview, BGM, layer, and drum-sample loading;
- missing and unsupported audio diagnostics;
- rate-aware play from load through stage completion;
- pause/resume, restart, and practice seek at non-1.0 rates;
- modified-speed result creates no native or `score.ini` write;
- native-speed result still persists exactly once.

### Local completion gates

- relevant package unit and integration tests;
- `cargo check --workspace`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check` after the Cycle 0 baseline repair.

No CI/CD changes or workflow verification are part of this design.

## 8. Rollout and compatibility

The existing `play_speed` config value remains valid and begins behaving as its
UI already promises. No user migration is required. The first launch after the
change may discover uppercase charts that were previously invisible. Existing
scores and profiles are untouched.

New MP3 support increases binary codec surface but uses an already-supported
feature of the chosen audio backend. Conditional parsing can change previously
corrupted charts from “all branches combined” to one valid branch; this is a
correctness fix, not a migration.

## 9. Success criteria

- A 0.5× or 2.0× normal run keeps notes, BGM, chart sounds, BGA, and completion
  aligned from start through seek/restart.
- The same run cannot become a normal PB or history record.
- An uppercase `.DTX` chart appears after scan.
- A conditional chart activates exactly one branch.
- SE32 schedules and plays through the ordinary SE pipeline.
- MP3 preview and performance audio decode successfully.
- Unsupported or broken content names its failure instead of disappearing or
  launching an empty chart.
- Existing 1.0× OGG/WAV DTX behavior and scores remain unchanged.
