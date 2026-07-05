# Live Autoplay Song-Select Preview — Design

Date: 2026-07-05
Status: Approved

## Goal

Replace the `#PREVIEW` clip at song select with a preview of the actual song:
full BGM plus autoplayed drum and SE chip sounds, starting at an inferred
"good part" of the song, with osu!-grade smoothness (debounce, crossfade,
loop back to the preview point).

## Background research

- **osu!(lazer)**: plays the full track seeked to `PreviewTime` metadata;
  fallback 40% of track length. 150 ms selection debounce (300 ms while a key
  is held). Crossfade 150 ms out / 220 ms in / 30 ms pre-delay;
  `RestartPoint = PreviewTime − 30 ms`; loops back to the preview point.
- **drum-game**: no drum autoplay in preview — merges chip-sliced BGM into a
  single `.ogg` at import (FFmpeg, per-chip delays) and seeks that one track.
  `PreviewTime` from song.ini or manual editing; same 40% fallback. No audio
  similarity analysis anywhere.
- **DTXManiaNX**: `#PREVIEW` clip only, played from its start, looped at fixed
  volume 80 with select-BGM ducking. The DTX format has **no preview-time
  field**, so a start point must be inferred.
- **Our codebase**: `crates/dtx-audio/src/preview.rs` already ports the osu
  fade constants (`crossfade.rs`). `play_bgm_from_seconds`
  (`dtx-audio/src/lib.rs`) supports mid-file starts. `dtx-timing` chip-time
  math is pure and reusable. Gameplay schedulers
  (`gameplay-drums/src/bgm_scheduler.rs`, `se_scheduler.rs`) contain the
  walk-chips-play-when-due loop but are hardwired to gameplay resources.
  `dtx-library` scan already parses every chart and discards the `Chart`.

## Decisions

| Decision | Choice |
|---|---|
| Approach | Live mini autoplay engine at song select (no offline pre-render) |
| Preview start point | Note-density heuristic from chart data, cached in `SongInfo` |
| Playback extent | Play from preview point to chart end, then loop back to the point |
| Mix content | BGM chips (0x01) + drum-lane chips + auto-SE chips (0x61–0x65) |
| Dwell debounce | 250 ms after selection settles |
| Volume model | Live `master`/`bgm`/`drum` sliders from `dtx-config`, same as gameplay |
| Same-song re-select | No restart — keep the preview playing when selection moves but resolves to the same song |
| Round-trip resume | Freeze preview clock on exit to gameplay; resume in place on return for the same song |
| Audio similarity | Out of scope for v1 (revisit only if the density heuristic picks bad spots) |

## Architecture

```
selection change (wheel)
      │  (wheel UI updates instantly, nothing loads)
      ▼
[dwell 250ms] ──selection changed again──▶ reset timer, drop stale work
      │
      ▼
async parse chart (dtx_assets::load_dtx, task pool, generation-tagged)
      │
      ▼
build preview plan:
  • preview_point_ms  ← SongInfo (cached at scan)
  • chips filtered: BGM(0x01) + drum lanes + SE(0x61–0x65), time ≥ point
  • BGM chip spanning the point → seek via play_bgm_from_seconds
      │
      ▼
async load WAV handles (PreviewSoundBank, separate from gameplay bank)
      │
      ▼
start when primary BGM handle ready ──▶ drums/SE layer in as their
      │                                  handles finish loading
      ▼
PreviewScheduler (Update, PreviewClock):
  walk sorted chips, play when due, fade-in 220ms (existing constants)
      │
      ▼
chart end ──▶ loop back to preview_point_ms (osu style)
```

## Components

### 1. Preview point at scan time (`dtx-library`)

During the existing scan parse (which currently discards the `Chart`),
compute `preview_point_ms` and store it on `SongInfo`:

- Slide a 20-second window over drum-lane chip times (computed with
  `dtx-timing` BPM/bar-change-aware chip math).
- Pick the window start with the highest chip count; on ties prefer the
  earlier window; clamp the result to at most 70% of chart duration.
- Charts with no drum chips fall back to 40% of chart duration.
- Pure function, unit-testable in isolation.

Until a persistent song cache (sqlite) lands, the offset is recomputed on
each scan.

### 2. `PreviewScheduler` (new, in `dtx-audio`)

Extraction of the "walk sorted chips, compare to a clock, play when due"
loop from `bgm_scheduler`/`se_scheduler`, with the gameplay dependencies
(`GameplayClock`, `AppState::Performance`, `PauseState`, `DrumsSets`)
removed:

- Owns a `PreviewClock` advanced by frame delta time.
- Sorted chip list with a cursor; chips fire when `clock >= chip_time`.
- A chip whose WAV handle has not finished loading at fire time is skipped —
  this yields progressive layering (BGM first, drums/SE join as they load)
  without extra machinery.
- The BGM chip whose file spans the preview point is started mid-file via
  `play_bgm_from_seconds`; non-BGM chips before the point are dropped.
- On reaching chart end (last chip fired and primary BGM finished), restart
  at `preview_point_ms` with the standard fade-in.
- Gameplay schedulers are untouched in v1; unification is a possible later
  refactor.

### 3. Song-select wiring (`game-menu/src/song_select.rs`)

`bgm_preview_on_change` becomes a dwell-timer system:

- Selection change resets a 250 ms timer and bumps a generation counter.
- When the timer elapses, spawn the async parse tagged with the generation;
  results from stale generations are discarded.
- The existing `#PREVIEW` clip path (`preview.rs`) is kept as a fallback,
  used when the chart fails to parse or contains no audio chips.

### 4. Volume and fades

- Per-instance kira volume tweens using the existing crossfade constants
  (150 ms out / 220 ms in / 30 ms pre-delay).
- Every play call applies the live `master`/`bgm`/`drum` volumes from
  `dtx-config`, matching the gameplay mix.
- Stopping the preview fades all active preview instances out over 150 ms.

### 5. Session continuity (osu-style resume)

osu never restarts the preview unless the track genuinely changed
(`SongSelect.ensurePlayingSelected` gated on `isNewTrack`; `OnResuming`
calls `music.Play(restart:false)` — resume in place, no re-seek). We mirror
this feel without a single continuous track:

- **Same-song re-select**: when the selection changes but resolves to the
  same chart identity (path), do not restart — the running preview keeps
  playing. Only a different chart starts a new preview at its preview point.
- **Round-trip resume**: on exit from song select into gameplay for chart X,
  freeze the `PreviewClock` position (and the chip cursor / active BGM
  offset) rather than discarding it. On return to song select:
  - selection is still chart X → resume the preview from the frozen clock
    position (pause/resume semantics), re-arming the fade-in;
  - selection is a different chart → start that chart's preview fresh at its
    preview point.
- The frozen position is preview-clock time, not gameplay-clock time — we do
  not couple to the gameplay clock. Resuming re-derives the BGM seek offset
  and chip cursor from the frozen `PreviewClock` value.

### 6. Memory / lifetimes

- `PreviewSoundBank` (separate from the gameplay `ChartSoundBank`) holds
  handles for the currently previewed song only.
- Cleared on song switch and on `OnExit(AppState::SongSelect)`; dropping the
  handles lets Bevy free the decoded audio.

## Error handling

- Chart parse failure → fall back to the `#PREVIEW` clip path.
- Missing WAV files → skip those chips (same policy as gameplay).
- No BGM chip at or after the preview point → drums/SE-only preview plays.

## Testing

- Unit: density-heuristic window selection on crafted chip sets (dense
  chorus, uniform density, no drums, short charts); scheduler cursor and
  seek math; resolution of the BGM chip spanning the preview point.
- Manual: fast wheel scrolling churn (no stutter, no stale audio),
  chip-sliced vs single-file BGM songs, loop seam quality, fallback path on
  a broken chart.

## Out of scope (v1)

- Audio-similarity refinement of the preview point (`#PREVIEW` clip vs BGM
  cross-correlation).
- Offline pre-rendered preview audio cache.
- `#PREVIEW` clip as an instant first layer with crossfade into the live
  engine.
- Persistent SongDb cache for the computed preview point.
- Other osu song-select audio polish, deferred as orthogonal to the preview
  engine (revisit individually later):
  - playback-rate/pitch preview for mods (osu DT/HT `ApplyModTrackAdjustments`);
  - low-pass duck when a confirmation dialog/overlay opens;
  - window-unfocus global volume fade;
  - carousel hover / invalid-selection UI samples;
  - low-pass muffle handoff of the preview into the gameplay loader.
