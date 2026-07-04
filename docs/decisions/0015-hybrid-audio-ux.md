# 0015: Hybrid audio architecture — osu fluidity + DTX preview file

Status: accepted
Date: 2026-07-04

## Context

ADR-0014 established the *intent* of osu-lazer-grade fluidity but did not
specify the audio architecture. Today's song-select preview flow is rigid:

```
SelectionIndex.is_changed() → stop_bgm (hard cut) → play_bgm (instant) → looped
```

No crossfade, no scroll-velocity gate, no shared-handle cache, no direction
event, no fallback for charts without a `#PREVIEW:` file, no exit-fade tied
to the 300ms screen fade. The song-select audio path uses ~zero of the
techniques that make osu feel fluid.

ADR-0010 (port-first for mechanics) does not constrain audio UX — audio
crossfade is not a mechanical element. The BocaD `CActSelectPresound`
pattern is 5–10s `Skin.bgmSongSelectScreen` global volume ramps, a
single-sound-instance model, and a `ctWaitForPlayback` counter for scroll
debounce. osu-lazer's `MusicController` does two-track 150ms/220ms
crossfade, an `AudioEquals` short-circuit (no fade when audio file is
identical), a 30ms pre-roll, a direction event for UI parallax, and
configurable per-screen looping.

We need an architecture that takes the wins from osu while honoring DTX's
own preview-file convention (no `PreviewTime` offset, separate `#PREVIEW:`
file is the excerpt). The full design lives in
[`docs/notes/hybrid-audio-ux-plan.md`](../notes/hybrid-audio-ux-plan.md).
This ADR records the decision and the four resolved design questions.

## Decision

**Hybrid audio architecture: per-chart audio handle cache + osu-style
two-track crossfade, gated on BocaD's scroll-velocity debounce.**

1. **`AudioHandleCache`** (new Resource in `dtx-audio`). Path → `Handle<KiraAudioSource>`.
   Switching to a chart with the same resolved BGM path reuses the handle
   (no AssetServer reload, no playback restart). Cache key is the resolved
   path from `dtx_core::resolve_bgm_path` (per-chart, BocaD-compatible —
   see Q1 below).

2. **Two-track crossfade 150ms out / 220ms in, 30ms pre-roll.** osu's
   `changeTrack` constants, expressed via `bevy_kira_audio::AudioTween::linear`
   on `AudioInstance::set_decibels`. Eliminates BocaD's 5–10s BGM ramp.

3. **`PreviewState` state machine** (`Idle | Loading | Playing | FadingOut | FadingIn`)
   in `dtx-audio`. `is_busy()` gate rejects re-entrant swaps.

4. **`PreviewSwapEvent` Message.** Fired on every selection change with
   `direction: Next | Prev | None`. Consumed by `dtx-audio` (the swap) and
   `dtx-ui` widgets (parallax + album-art crossfade).

5. **`ScrollVelocity` Resource** (published by `dtx-ui`, consumed by
   `dtx-audio`). The crossfade defers when velocity > ε. Adapts BocaD's
   `!isScrolling` gate to the new state model.

6. **Loop flag per screen** (`PreviewPlayer::play(path, looping)`). `true`
   on SongSelect (loop excerpt), `false` on Title (autoplay-through).

7. **Fallback to full BGM from t=0** when a chart has no `#PREVIEW:` file
   (looping = false). Currently silent.

8. **Exit-fade tied to screen fade.** `PreviewPlayer` listens to
   `ScreenFade::start_fade_out` and tween-fades the preview instance to
   –60dB over 300ms OutQuint, matching the screen overlay exactly.

9. **Master × instance volume multiplication.** No code change. Kira's
   channel-level master multiplies with the per-instance volume the
   crossfade adjusts. Matches osu's `sampleVolume * trackVolume` model.

10. **Album-art crossfade on swap**, **guarded against in-flight tween**
    (hard-cut when the previous fade-in hasn't completed). Prevents
    partial-opacity ghosts on rapid scroll.

## Resolved design questions

1. **Multi-difficulty preview convention:** per-chart, BocaD-compatible.
   Multiple diffs with the same `#PREVIEW:` file share one cache entry
   automatically (cache key = resolved path). Mirrors BocaD's
   `CChartData.Presound` semantics.

2. **Master × instance volume:** multiply. Both systems already do their
   own thing; multiplication is a property of the audio graph, not extra
   code.

3. **Rapid-scroll album art:** guard against in-flight tween
   (`AlbumArtTween::is_flying()`). 3-line check in the widget.

4. **M14+ decode pool parallelism:** no. Finish the four audio phases
   first. Both touch `dtx-audio` Resources; merge conflicts would be
   unprofitable. Decode pool lands after phase 4 with no overlap.

## Crate / file changes (rollout)

| Crate | Files | Phases |
|---|---|---|
| `dtx-audio` | `preview.rs` (new), `crossfade.rs` (new), `lib.rs` (re-export) | 1, 2, 3, 4 |
| `game-menu` | `song_select.rs` (rewrite `bgm_preview_on_change`), `title.rs` (looping flag) | 1, 2, 3, 4 |
| `dtx-ui` | `parallax.rs` (new), `widget/album_art.rs` (modify), `lib.rs` (add `ScrollVelocity`) | 2, 3 |
| `game-shell` | `transition.rs` (publish `ScreenFadePhase` event) | 4 |

No new dependencies. No crate-layer violations (all changes are within
Engine + Game; `dtx-audio` remains Engine, no Pure/Engine crossings).

## Phased rollout

Each phase is a self-contained, demo-able improvement. Sequential.

| Phase | Deliverable | Verification |
|---|---|---|
| 1 | `AudioHandleCache` + path-key dedupe | Cache hit returns same handle; cache miss invokes AssetServer once; rapid row-cycle with same audio produces one load event |
| 2 | Crossfade (150/220ms, 30ms pre-roll) + `PreviewState` + `PreviewSwapEvent` + `ScrollVelocity` gate | State machine test: `Idle → Loading → Playing → FadingOut → FadingIn → Playing`; `busy_state_rejects_second_swap`; headless audio test confirms fade-in reaches 0dB within 220ms ±16ms |
| 3 | Album-art crossfade (guarded) + loop flag | BRP screenshot diff: album art at t=185ms shows `opacity = 0.5 ± 0.1` of new image; `set_looping_true_passes_loop_to_audio_play` |
| 4 | Fallback for missing `#PREVIEW:` + exit-fade aligned to screen fade | BRP test: SongSelect → SongLoading transition produces BGM fade-to-–60dB within 300ms ±32ms of `NextState::set` |

## Constants (named, not magic)

```rust
// crates/dtx-audio/src/crossfade.rs
pub const PREVIEW_FADE_OUT_MS:  u32 = 150;  // osu MusicController:520
pub const PREVIEW_FADE_IN_MS:   u32 = 220;  // osu MusicController:519
pub const PREVIEW_FADE_DELAY_MS: u32 = 30;  // osu MusicController:41 DELAY_BEFORE_FADE

// crates/dtx-audio/src/preview.rs
pub const SCROLL_DEBOUNCE_MS: u32 = 120;    // BocaD ctWaitForPlayback default
```

## Consequences

- **Plus:** Visible/perceptible fluidity in song select. 5–10s BGM ramps
  become 220ms crossfades. Switching diffs under one song no longer
  restarts the audio. Album art crossfade aligned to audio midpoint.
  Charts without `#PREVIEW:` no longer go silent.
- **Plus:** Foundation for `dtx-audio` M14+ work (decode pool, mod
  ducking) — `PreviewState` and the cache are the right shapes for those
  features to build on.
- **Plus:** Fills the "BGM fade-in/out (M3 with shell transitions)"
  deferral called out in `crates/dtx-audio/AGENTS.md`.
- **Minus:** One new Resource (`AudioHandleCache`) and one new Message
  (`PreviewSwapEvent`) to keep in sync across crates. `PreviewState`
  state machine is a small piece of logic that needs tests to stay
  correct.
- **Minus:** Crossfade constants are now a coupling point. If we later
  want a config-driven crossfade duration, the constants become a
  `Resource` — a 5-line change, but it touches every call site.
- **No change to mechanics** (judgment, scoring, lanes, channels,
  chart parsing — ADR-0010 still governs).
- **No change to screen-fade system** (already 300ms OutQuint, ADR-0014).
- **No change to skinning** (deferred to M14+).

## Alternatives considered

- **Full osu-port of `MusicController`.** Rejected. osu's reactive
  `Bindable<WorkingBeatmap>` doesn't translate to Bevy ECS; the value
  is decoupling screens, which we don't have. Beating BocaD's
  `CActSelectPresound` with a one-file Bevy system is cheaper.
- **Keep BocaD verbatim, polish timing constants only.** Rejected.
  BocaD's 5–10s BGM ramps are the loudest source of rigidity. Cutting
  them to 220ms is the single biggest fluidity win. The scroll-debounce
  is the only piece worth preserving.
- **Build a custom audio mixer on top of `bevy_audio` (kira under the
  hood).** Rejected. `bevy_kira_audio` already exposes
  `AudioTween::linear(Duration)` and per-instance `set_decibels` — the
  crossfade is a 6-line wrapper, not a system to build.
- **Reactive `Bindable<SelectedSong>` rewrite of `SelectionIndex`.**
  Rejected. `SelectionIndex::is_changed()` already works; the rewrite
  adds indirection without unlocking new functionality.

## Reference files

**osu-lazer (audio patterns we copy):**
- `references/osu-lazer/osu.Game/Overlays/MusicController.cs:41,519-532` —
  fade constants, `changeTrack` body, `DELAY_BEFORE_FADE`
- `references/osu-lazer/osu.Game/Overlays/MusicController.cs:170-204` —
  `changeBeatmap` with `TryTransferTrack` and direction detection
- `references/osu-lazer/osu.Game/Beatmaps/WorkingBeatmap.cs:123-141` —
  `PrepareTrackForPreview` with -30ms offset, 40% fallback
- `references/osu-lazer/osu.Game/Beatmaps/BeatmapMetadata.cs:55-59` —
  `PreviewTime` default = -1 → 40%

**DTXManiaNX-BocaD (audio patterns we keep):**
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CActSelectPresound.cs:38-58` —
  `tSelectionChanged` scroll-debounce pattern
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CActSelectPresound.cs:60-99` —
  `OnUpdateAndDraw` counter-driven fade completion
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs:393-401` —
  `ChangeSelection` fan-out (audio + status + density)

**Project state (what we modify):**
- `crates/dtx-audio/src/lib.rs:24-30` (`BgmHandle`), `:215-225` (`stop_bgm`), `:231-242` (`play_bgm`)
- `crates/game-menu/src/song_select.rs:565-583` (`bgm_preview_on_change` — current implementation)
- `crates/dtx-core/src/assets.rs:383-432` (`resolve_bgm_path` — preview is fallback #3)
- `crates/dtx-ui/src/transition.rs:14-86` (`ScreenFade` 300ms OutQuint — already shipped)

**Related ADRs:**
- ADR-0010 — port-first (mechanics only); this ADR is UX/audio and
  explicitly outside port-first scope
- ADR-0014 — osu-inspired UX redesign; this ADR is the audio-side
  implementation of that intent
- ADR-0002 — `dtx-timing::AudioClock` authoritative; this ADR does not
  change judgment timing, only the menu/song-select preview path

**Full design:** [`docs/notes/hybrid-audio-ux-plan.md`](../notes/hybrid-audio-ux-plan.md)
(447 lines, 9 sections, ASCII data-flow diagrams, per-phase verification
harness, full file-by-file change map).

## Verification

Before each phase lands:

- [ ] No new dependencies added (`Cargo.toml` deltas only)
- [ ] No Pure/Engine/Game layer crossings (this ADR is Engine + Game only)
- [ ] Mechanics tests still green (`cargo test --workspace`)
- [ ] Audio crate tests cover the new state machine transitions
- [ ] No `unwrap()` in `crates/*` (project rule)
- [ ] `bevy 0.19` gotchas respected (per memory: `Res<T>` doesn't impl
      `DerefMut`; `init_resource<T>()` needs `FromWorld`; use `Message`
      not `Event`)
- [ ] Per-crate `AGENTS.md` updated if the public surface changes
- [ ] `references/<path>:L<line>` cited in commit for any non-trivial
      behavior ported (ADR-0008)
- [ ] No AI co-author trailer in commit (project rule)
- [ ] Build green after each commit; one finding per commit
- [ ] BRP debug MCP (`bevy_brp_extras` + `.mcp.json`) used to visually
      verify each phase
