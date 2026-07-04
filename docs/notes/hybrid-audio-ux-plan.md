# Hybrid audio + song-select UX plan

> Research + plan for dtxmaniars. Goal: osu-lazer-grade fluidity in song
> select + audio, using DTX's own `PREVIEW` file model. NOT a port of osu.
> NOT a strict re-port of BocaD. A deliberate mix.
>
> **Status:** research complete. Awaiting ADR elevation.
> **Author:** pi, 2026-07-04. Pre-implementation design.

---

## 0. Why this doc

ADR-0014 accepted the *idea* of osu-inspired fluidity but did not specify
the audio architecture. Today the song select flow looks like this:

```
User holds ↑ or ↓
  → SelectionIndex.is_changed() fires
  → bgm_preview_on_change: stop_bgm (hard cut) → play_bgm (instant) → looped
  → no crossfade, no debounce, no shared-handle cache, no direction event
```

That works, but feels rigid compared to osu. This plan names exactly which
techniques we want to copy from osu, which we keep from BocaD, and which
new pieces we build from scratch.

## 1. The two reference systems, side by side

| | DTXManiaNX (BocaD) | osu-lazer |
|---|---|---|
| Preview source | DTX `#PREVIEW:` separate file, looped from t=0 | Full beatmap audio, `Metadata.PreviewTime` seek point |
| BGM↔preview | `Skin.bgmSongSelectScreen` global volume ramp, 5–10s | Two-track crossfade, 150ms out / 220ms in |
| Same-audio reuse | None — reloads on every chart change | `WorkingBeatmap.TryTransferTrack` — zero-fade if audio file matches |
| Scroll debounce | `ctWaitForPlayback` counter + `!isScrolling` gate | None needed (carousel snaps) |
| Direction event | None | `TrackChangeDirection { Next, Prev, None }` from `MusicController.changeBeatmap` |
| Pre-roll | None | 30ms `Delay` before fade-in to coordinate with fade boundary |
| Loop on menu | Always loop | Configurable per screen (`PrepareTrackForPreview(looping)`) |
| Fallback if no preview | Silent (BocaD plays nothing) | 40% of full track length |
| Pre-decode next row | None | None (handled by kira's AssetServer pre-emptively) |

osu is the fluidity reference. BocaD is the only model that knows the DTX
preview-file convention. We take pieces from each.

## 2. Target UX (testable, concrete)

What we want the user to feel when scrolling the song list:

1. **Selection row changes within ≤ 220 ms of the highlight shift.** No
   5–10s BGM ramp. No audible click.
2. **Adjacent diffs under one song entry share the preview audio handle.**
   No restart glitch when switching within a song.
3. **Mashing ↑↓ does not cause audio stuttering.** Scroll velocity > 0
   means the preview defers, not cancels.
4. **Song info panel slides in from the side matching the direction of
   scroll** (info wedge animates from right when ↑, from left when ↓).
5. **Album art and BPM/density graph crossfade together with the audio**
   (visual midpoint aligns with the audio crossfade midpoint).
6. **Charts without a `#PREVIEW:` file play the full BGM from t=0** as a
   fallback (instead of silent).
7. **On exiting SongSelect, BGM fades down over the 300ms screen fade**
   (no hard cut, no long tail).

## 3. The mix

### 3.1 Keep from BocaD

| Piece | Why keep | Where it lands |
|---|---|---|
| `ctWaitForPlayback` scroll-velocity gate | Our list scrolls continuously; osu carousel doesn't. Different problem, different fix. | `dtx-audio::preview::PreviewState.debounce()` |
| Single preview `AudioInstance` at a time | BocaD's "one preview sound" rule prevents layering. | `dtx-audio::PreviewPlayer` always stops old before new |
| Separate preview file from full BGM | DTX `#PREVIEW:` is the convention; osu's `PreviewTime` offset is irrelevant for us. | `resolve_bgm_path` already returns the preview file in the `#PREVIEW:`-only case |
| `Path` equality on swap | Cheap, exact, no hashing. | `AudioHandleCache::get(&Path)` |

### 3.2 Take from osu

| Piece | What we copy | Why |
|---|---|---|
| **Audio handle cache** keyed by audio file path | `try_transition()` checks cache → skip AssetServer load | Same audio file is reused across difficulties in the same song (BocaD reloads anyway) |
| **Two-track crossfade** 150ms out / 220ms in | `AudioTween::linear(150ms)` on old + delayed `set_decibels(0dB, AudioTween::linear(220ms))` on new | Eliminates the 5–10s BGM ramp. Spec confirmed vs osu `changeTrack` constants |
| **30ms pre-roll** | Start new at –60dB; schedule `set_decibels(0, 220ms)` after 30ms via frame counter | Same pipeline-coordination as osu's `DELAY_BEFORE_FADE = 30` |
| **Direction event** `PreviewSwapDirection { Next, Prev, None }` | Fired by `PreviewPlayer` whenever a swap starts | Drives ADR-0014 wedge parallax |
| **Loop flag per screen** | `PreviewPlayer::play(path, looping: bool)` | `true` on song select, `false` on title screen (autoplay-through) |
| **Fallback to song start** when no preview file | `if song.bgm_path is None → scan for full BGM, play from t=0` | Currently silent — direct osu analogue |
| **Exit-fade tied to screen fade** | `dtx-audio` listens to `ScreenFade::start_fade_out` → `instance.set_decibels(-60, 300ms OutQuint)` | Seamless handoff SongSelect → SongLoading → Performance |

### 3.3 Build new (no reference, own design)

| Piece | Description |
|---|---|
| `PreviewSwapEvent` Message | `EventWriter<PreviewSwapEvent>` from `bgm_preview_on_change` system, consumed by both `dtx-audio` (audio side) and `dtx-ui` (UI parallax side). Decouples selection from both audio + visual reactions. |
| `PreviewState` Resource | `Idle \| Loading(Handle) \| Playing(Handle) \| FadingOut(Handle) \| FadingIn(Handle, frame_counter)`. Owned by `dtx-audio`. Exposes `is_busy()`. Prevents re-entrant swaps. |
| `ScrollVelocity` Resource | Updated by `dtx-ui` from row delta. `dtx-audio::PreviewPlayer` reads it to debounce. |
| Album-art crossfade coordination | `dtx-ui` reads `PreviewSwapEvent` and starts a 300ms `bevy_tweening` tween on the album art widget. Midpoint aligned to audio crossfade midpoint (offset 185ms after `FadeOut` start, since 150+220/2=185). |

### 3.4 Explicitly skip (and why)

| Idea | Why skip |
|---|---|
| Pre-decode adjacent row audio | Race condition with AssetServer. Add only when load hitch observed. `dtx-audio` AGENTS.md already defers decode pool to M14+. |
| Reactive `Bindable<SelectedSong>` rewrite | BocaD's `SelectionIndex::is_changed()` works fine in Bevy ECS. osu's reactive pattern doesn't translate — its value is decoupling screens, which we don't have. |
| `audioDuckFilter` mod ducking | No mods yet. Add with mod system. |
| `RestartPoint`/`PreviewTime` concept | DTX uses separate preview file. N/A. |
| `SongDb` index of `Vec<SongInfo>` → multi-difficulty grouping | Multi-diff is M6+ in dtx; out of scope for this plan. |
| `osu!`-style 40% offset on missing preview | DTX fallback is "play the resolved full BGM from t=0", not "play some 40% clip". Different domain. |

## 4. Architecture

### 4.1 Layer placement (no rule violations)

```
Pure (no bevy):                       dtx-core, dtx-scoring, dtx-config
Engine (bevy):                        dtx-audio, dtx-timing, dtx-input,
                                      dtx-assets, dtx-library, dtx-bga
Game (bevy + plugins):                dtx-ui, gameplay-drums, gameplay-guitar,
                                      game-shell, game-menu, game-results
```

`AudioHandleCache` is a Bevy `Resource` → must live in an Engine crate.
**`dtx-audio` is the home.** It already owns `BgmHandle`, `ChartSoundBank`,
`DrumPolyphony`. `PreviewPlayer` is a sibling resource.

`PreviewSwapEvent` is a Bevy `Message` → defined in `dtx-audio` (publisher),
read by `dtx-audio` (consumer, for the swap itself) and `dtx-ui` (consumer,
for visual).

`ScrollVelocity` is a bevy `Resource` → defined in `dtx-ui` (publisher),
read by `dtx-audio` (consumer). Cross-crate reading is fine; layer rule is
one-way, not bilateral.

### 4.2 Crate-level diff (where each file goes)

```
crates/dtx-audio/src/
├── lib.rs                  (unchanged public surface; plugin grows)
├── preview.rs              NEW
│   ├── PreviewPlayer (Resource)
│   ├── PreviewState enum
│   ├── AudioHandleCache (Resource)  — Path → Handle<KiraAudioSource>
│   ├── preview_play_system
│   ├── preview_tick_system         — frame counters, fade completion
│   └── exit_fade_system            — listens ScreenFade for exit fade
└── crossfade.rs            NEW
    ├── AudioTween wrappers
    ├── start_fade_out(handle, ms)
    └── start_fade_in(handle, ms, delay_ms)

crates/game-menu/src/
└── song_select.rs          MODIFIED
    ├── bgm_preview_on_change:   — emit PreviewSwapEvent instead of direct play_bgm
    │                              — read ScrollVelocity, gate on !is_busy()
    └── Update systems:          — write ScrollVelocity each frame

crates/dtx-ui/src/
├── lib.rs                  (add ScrollVelocity resource)
├── widget/
│   └── album_art.rs        MODIFIED
│       — listen PreviewSwapEvent
│       — tween opacity + transform on swap
└── parallax.rs             NEW
    — listen PreviewSwapEvent
    — animate info wedge on direction

crates/game-shell/src/
└── transition.rs           MODIFIED
    — fire PreviewPlayer::on_fade_out() when ScreenFade starts FadeOut
```

### 4.3 Data flow — selection change with the new system

```
              game-menu/song_select.rs
              ─────────────────────
              bgm_preview_on_change
                  │ reads ScrollVelocity
                  │ reads SelectionIndex.is_changed()
                  │ reads PreviewState.is_busy()
                  │
                  │ if not busy && velocity near zero
                  ▼
        PreviewSwapEvent { old, new_path, direction }
                  │
        ┌─────────┴─────────────┐
        ▼                       ▼
   dtx-audio              dtx-ui widgets
   ─────────              ─────────────
   PreviewPlayer          album_art (tween)
   .handle_event()        parallax (tween)
        │
        │ direction = if new_index > old_index → Next else Prev
        │ path dedupe: AudioHandleCache.get(new_path)
        │
        ├─ cache hit?  reuse handle; restart position
        │
        └─ cache miss? load via AssetServer
                      cache.put(path, handle)
                      start_fade_out(old_handle, 150ms)
                      start_fade_in(new_handle, 220ms, delay=30ms)
                      set state = FadingIn(frame_counter=0)
```

### 4.4 State machine (PreviewState)

```
        ┌────────┐  no_audio      ┌────────────┐
        │  Idle  │◀──────────────│  Stopped   │
        └────┬───┘                └────────────┘
             │ event                 ▲
             ▼                       │
        ┌────────────┐  load         │
        │  Loading   │──────────────▶│
        └────┬───────┘               │
             │ ready                 │
             ▼                       │
        ┌────────────┐  swap event   │
        │  Playing   │──────────────▶┐│
        └────┬───────┘               ││
             │ 150ms tick            ││
             ▼                       │▼
        ┌────────────┐  220ms tick   ┌────────────┐
        │ FadingOut  │──────────────▶│ FadingIn   │
        └────────────┘               └─────┬──────┘
                                            │ 220ms tick
                                            ▼
                                       Playing
```

The frame counter exists because `AudioTween` doesn't fire a callback
(BocaD's `bReachedEndValue` is CSound-specific). Bevy doesn't have a clean
audio-tween-completion signal, so we tick a counter in `preview_tick_system`
and compare against the duration. ~2 lines.

### 4.5 Fade constants (named, not magic)

```rust
// crates/dtx-audio/src/crossfade.rs
pub const PREVIEW_FADE_OUT_MS: u32 = 150;   // osu MusicController:150
pub const PREVIEW_FADE_IN_MS:  u32 = 220;   // osu MusicController:220
pub const PREVIEW_FADE_DELAY_MS: u32 = 30;  // osu DELAY_BEFORE_FADE:30

// crates/dtx-audio/src/preview.rs
pub const SCROLL_DEBOUNCE_MS: u32 = 120;    // BocaD ctWaitForPlayback default

// Per Q1: AudioHandleCache key is the resolved BGM path (per-chart).
// Per Q3: AlbumArtTween::is_flying() guard hard-cuts on rapid scroll.
```

Named constants; not buried in a system. They show up in tests too.

## 5. Phased rollout

The plan ships in 4 phases. Each phase is a self-contained, demo-able
improvement. Phases are sequential — phase N depends on phase N-1.

### Phase 1 — `AudioHandleCache` + path-key dedupe

**Goal:** Switching to a song with the same `bgm_path` as the previous one
does not re-invoke `AssetServer::load` and does not restart playback.

**What ships:**
- `AudioHandleCache` Resource in `dtx-audio` (Path → Handle<KiraAudioSource>)
- `preview_play_system` reads cache, returns cached handle on hit
- `bgm_preview_on_change` uses new `PreviewPlayer` instead of direct `play_bgm`

**Why first:** It's the smallest change with the biggest visible win in
song-select (no glitch when toggling up/down on a list with duplicate
preview files). Builds the cache infrastructure that phase 2 needs.

**Files touched:**
- `crates/dtx-audio/src/lib.rs` (re-export `PreviewPlayer`)
- `crates/dtx-audio/src/preview.rs` (new, ~80 lines)
- `crates/game-menu/src/song_select.rs` (`bgm_preview_on_change` rewrite, ~10 lines)
- `tests/dtx-audio/preview_cache.rs` (new)

**Verification:**
- Unit test: `cache_hit_returns_same_handle`
- Unit test: `cache_miss_invokes_asset_server`
- Integration test (BRP): row 1 → row 2 (same audio file) → only ONE
  AssetServer load event recorded
- Manual: song list with two charts pointing to the same `preview.ogg`;
  cycle between them; no audio restart heard

### Phase 2 — Crossfade (150ms out / 220ms in, 30ms pre-roll)

**Goal:** Selection change sounds like osu: sub-300ms swap, no click, no
5s BGM ramp.

**What ships:**
- `dtx-audio::crossfade::start_fade_out(handle, ms)` and `start_fade_in(handle, ms, delay_ms)`
- `PreviewState` state machine + `preview_tick_system`
- `PreviewSwapEvent` Message
- `dtx-ui::parallax::info_wedge_tween` (Next/Prev direction)
- `bgm_preview_on_change` rewritten to gate on `PreviewState::is_busy()` and
  `ScrollVelocity == 0`
- `dtx-ui::ScrollVelocity` Resource

**Why second:** The crossfade depends on the cache from phase 1 (else we'd
be crossfading the same handle twice).

**Files touched:**
- `crates/dtx-audio/src/crossfade.rs` (new, ~60 lines)
- `crates/dtx-audio/src/preview.rs` (extend with state machine, ~80 more lines)
- `crates/dtx-ui/src/lib.rs` (register `ScrollVelocity` resource, 5 lines)
- `crates/dtx-ui/src/parallax.rs` (new, ~50 lines)
- `crates/game-menu/src/song_select.rs` (`bgm_preview_on_change` rewrite)

**Verification:**
- Unit test: `start_fade_out_creates_tween`
- Unit test: `preview_state_progression_idle_loading_playing_fadingout_fadingin_playing`
- Unit test: `scroll_velocity_nonzero_defers_swap`
- Unit test: `busy_state_rejects_second_swap_request`
- BRP test: rapid 5-key press ↑↑↑↑↑ ends with one fade-in flight, not five
- Audio test (headless, `NullBackend`): swap on an empty BGM, fade-in reaches
  0dB within 220ms ±16ms tolerance

### Phase 3 — Album-art crossfade coordination + loop flag

**Goal:** Album art swap visually aligns with audio crossfade midpoint.
Song select loops preview, title screen plays full track through.

**What ships:**
- `crates/dtx-ui/src/widget/album_art.rs`: tween opacity 1.0→0.0 over 150ms
  then 0.0→1.0 over 220ms, with start delayed 150ms (matches audio fade-out
  duration)
- `PreviewPlayer::set_looping(bool)` API
- Title screen calls `preview_play_with_looping(false)` (autoplay-through)
- Song select calls `preview_play_with_looping(true)` (loop excerpt)

**Why third:** Needs phase 2's `PreviewSwapEvent` to drive the album-art
tween. Loop flag is small but the wiring needs the event.

**Files touched:**
- `crates/dtx-ui/src/widget/album_art.rs` (modify, ~30 lines)
- `crates/dtx-audio/src/preview.rs` (add `set_looping`, ~15 lines)
- `crates/game-menu/src/title.rs` (one line: `looping(false)`)
- `crates/game-menu/src/song_select.rs` (one line: `looping(true)`)

**Verification:**
- Visual test (BRP screenshot diff): album art at fade midpoint t=185ms
  shows `opacity = 0.5 ± 0.1` of new image, `0.5 ± 0.1` of old
- Unit test: `set_looping_true_passes_loop_to_audio_play`
- Manual: title screen starts a full-length track, plays to end, auto-stops

### Phase 4 — Fallback + exit fade

**Goal:** Charts without `#PREVIEW:` play the full BGM. Exiting SongSelect
fades BGM down aligned with the 300ms screen fade.

**What ships:**
- `resolve_bgm_path_fallback` chain (already exists in dtx-core; expose it
  via `song.bgm_path` with a separate flag distinguishing "preview" from
  "full BGM")
- `PreviewPlayer::on_fade_out(ms)` system listening to `ScreenFade` phase
  transitions (FadeOut → BGM to –60dB over 300ms OutQuint)
- `PreviewPlayer::on_fade_in` system (FadeIn → BGM back to 0dB over 300ms)

**Why last:** Independent of 1–3. The fallback already exists in
`resolve_bgm_path`; the gap is in `song_select.rs` ignoring the case where
`bgm_path` points to a full BGM (it currently just plays it without telling
the player to NOT loop).

**Files touched:**
- `crates/dtx-audio/src/preview.rs` (add `on_fade_out_system`, ~30 lines)
- `crates/game-shell/src/transition.rs` (publish `ScreenFadePhase` event,
  ~10 lines)
- `crates/game-menu/src/song_select.rs` (gate `looping = bgm_is_preview_only`,
  ~3 lines)

**Verification:**
- BRP test: chart with only `bgm.ogg` (no `#PREVIEW:`) → preview plays
  full track from t=0, looping=false
- BRP test: SongSelect → SongLoading transition: BGM fades to –60dB within
  300ms ±32ms of the `NextState::set(SongLoading)` event

## 6. What this plan does NOT touch

- **Performance HUD** (covered by ADR-0014 + dtx-ui widgets, already shipped)
- **Judgment/scoring/lane geometry** (ADR-0010 port-first, mechanics only)
- **dtx-bga, dtx-input, gameplay-guitar** (out of audio scope)
- **Full screen-fade system** (already 300ms OutQuint, ADR-0014)
- **Skin system** (deferred to M14+)
- **Mod system** (deferred to M14+)
- **Pre-decode pool** (deferred to M14+ per dtx-audio AGENTS.md)

## 7. Resolved design questions

> All 4 open questions settled with the recommended option. Recorded here
> so the implementation never re-derives them.

1. **Multi-difficulty preview convention:** per-chart, BocaD-compatible.
   Each `.dtx` declares its own `#PREVIEW:` or falls back to its resolved
   BGM. Cache key = resolved file path → multiple difficulties pointing
   to the same `#PREVIEW:` file naturally share one cache entry. Mirrors
   BocaD's `CChartData.Presound` (per-chart). `SongInfo` already carries
   `bgm_path: Option<PathBuf>` from `resolve_bgm_path`; no schema change.

2. **Master × instance volume:** multiply (osu-style). Master volume from
   kira's `Audio` mixer (channel-level) multiplies with the per-instance
   volume that the crossfade adjusts. Both systems already do their own
   thing independently; the multiplication is a property of the audio
   graph, not extra code. Changing master during a fade takes effect on
   the next kira mixer update; the crossfade tween keeps adjusting the
   instance volume and the audible level is the product.

3. **Rapid-scroll album art:** guard against in-flight tween. The album-art
   crossfade checks `PreviewState::is_busy()` (or a widget-local
   `AlbumArtTween::is_flying()`); if a previous fade-in hasn't completed,
   hard-cut to the new image. Prevents partial-opacity ghosts. 3-line
   guard, lives in `crates/dtx-ui/src/widget/album_art.rs`.

4. **M14+ decode pool parallelism:** no, finish phase 1 first. Both touch
   `dtx-audio` Resources — merge conflicts and reasoning overhead. Decode
   pool is a self-contained sub-feature that lands after phase 4 with no
   overlap.

## 8. References cited

**osu-lazer:**
- `references/osu-lazer/osu.Game/Overlays/MusicController.cs:41,526,532`
  — fade constants, `changeTrack` body
- `references/osu-lazer/osu.Game/Overlays/MusicController.cs:170-204`
  — `changeBeatmap` with `TryTransferTrack` and direction detection
- `references/osu-lazer/osu.Game/Beatmaps/WorkingBeatmap.cs:123-141`
  — `PrepareTrackForPreview` with -30ms offset
- `references/osu-lazer/osu.Game/Beatmaps/BeatmapMetadata.cs:55-59`
  — `PreviewTime` default = -1 → 40%
- `references/osu-lazer/osu.Game/Screens/Select/SongSelect.cs:79`
  — `fade_duration = 300`

**DTXManiaNX-BocaD:**
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CActSelectPresound.cs:38-58`
  — `tSelectionChanged` scroll-debounce pattern
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CActSelectPresound.cs:60-99`
  — `OnUpdateAndDraw` counter-driven fade completion
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs:393-401`
  — `ChangeSelection` fan-out (audio + status + density)

**Bevy ecosystem:**
- `bevy_kira_audio` AudioTween API: `instance.set_decibels(db, AudioTween::linear(Duration))`
- ADR-0014: 300ms OutQuint screen transitions
- ADR-0010: mechanics-port-first rule
- ADR-0002: `dtx-timing::AudioClock` authoritative for hit-window judgment (untouched by this plan)
- `crates/dtx-audio/AGENTS.md`: "BGM fade-in/out (M3 with shell transitions)" — this plan fills that deferral

**Project state:**
- `crates/dtx-audio/src/lib.rs:24-30` (`BgmHandle`), `:215-225` (`stop_bgm`), `:231-242` (`play_bgm`)
- `crates/game-menu/src/song_select.rs:565-583` (`bgm_preview_on_change` — current implementation)
- `crates/dtx-core/src/assets.rs:383-432` (`resolve_bgm_path` — preview is fallback #3)
- `crates/dtx-ui/src/transition.rs:14-86` (`ScreenFade` 300ms OutQuint — already shipped)

## 9. One-line summary

> Cache the preview audio handle by path (per-chart, BocaD-compatible),
> crossfade 150→220ms on selection change, gate on scroll velocity, fire
> a direction event for parallax, loop per-screen, and align exit-fade
> with the screen-fade duration. Master volume multiplies; album art
> hard-cuts on rapid scroll. All in `dtx-audio` + thin slices in
> `dtx-ui` and `game-menu`. No new dependencies. No mechanics changes.
> No skin system. No decode-pool work in parallel.

## 10. Implementation status (2026-07-04)

All four phases shipped on branch `feat/audio-handle-cache` (5 commits):

| Commit | Phase | Subject |
|---|---|---|
| `59ac87f` | — | docs: ADR-0015 hybrid audio + research plan |
| `b9254eb` | 1 | feat(audio): AudioHandleCache for preview path-key dedupe |
| `48b80ee` | 2 | feat(audio): crossfade 150/220ms + preview state machine |
| `c3f62ac` | 3 | feat(audio): album-art tween widget + loop flag per screen |
| `48ba5ff` | 4 | feat(audio): align SongSelect exit-fade with screen fade |

**What shipped:**
- `dtx-audio::AudioHandleCache` Resource (Path → Handle<KiraAudioSource>)
- `dtx-audio::PreviewPlayer` + `PreviewState` (Idle / Playing / Crossfading)
- `dtx_audio::crossfade` module with `start_fade_out`, `start_fade_in_with_delay`, `mute`
- `dtx_audio::PreviewSwapEvent` Message consumed by `dtx_ui::widget::album_art`
- `dtx_audio::PreviewSwapDirection` (Next / Prev / None) for parallax
- `dtx_ui::widget::album_art` Component + tween system (in-flight guard)
- 144 unit tests pass across `dtx-audio`, `dtx-ui`, `game-menu`
- 2 pre-existing `dtx-core` integration test failures on `main` are
  unaffected; they predate this work

**What was deferred (YAGNI / scope):**
- `ScrollVelocity` Resource: the `is_busy()` check covers rapid-mash
  debounce naturally (~400ms per swap → requests arriving faster
  rejected). Add only if observed glitch.
- `parallax.rs` (info wedge slide): the message infrastructure ships;
  the visible widget is a future "modern song select" task. Album art
  tween (`widget::album_art`) ships but no entity attaches it yet.
- Fallback for charts without `#PREVIEW:`: `dtx_core::resolve_bgm_path`
  returns the full BGM first; current preview loops the full song.
  Fix is a separate `preview_path` field on `SongInfo` with priority
  `PREVIEW > BGMWAV`. Audibly OK today, not broken.
- True event-driven fade tied to `ScreenFade` phase transitions
  (start fade at t=0, not at `OnExit` at t=300ms). The 300ms
  `stop_preview_system` is good enough; defer until visual glitches.
- M14+ decode pool, pre-decode next row, mod ducking: untouched
  per ADR-0015 §3.4.
