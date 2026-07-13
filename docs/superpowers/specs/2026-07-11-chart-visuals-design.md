# Chart Visuals: BGA Images, Movies, and Song Cover

**Date:** 2026-07-11  
**Status:** Approved design

## Goal

Render chart-authored `#BMP` images and `#AVI` movies during drum performance, make the four existing visual settings functional, and show the chart `#PREIMAGE` in the top-right Now Playing card.

Movies fill the gameplay background with aspect-fit scaling. Lanes and HUD remain above them. Chart audio remains the only audio source.

## Scope

- Parse standard timed BGA and movie chip sequences with their asset IDs and fractional positions.
- Resolve chart-relative visual assets case-insensitively.
- Render static `#BMP` layers instead of colored placeholders.
- Decode `#AVI` assets through `video-rs` 0.11 and FFmpeg.
- Synchronize movie frames to the gameplay chart clock.
- Support pause, practice seek, restart, and stage cleanup.
- Wire these existing config fields into Customize and runtime behavior:
  - `system.bga_enabled`
  - `system.movie_enabled`
  - `system.bg_alpha`
  - `system.movie_alpha`
- Load `#PREIMAGE` into the top-right Now Playing card.

## Non-goals

- Movie audio playback
- `BGAPAN` animation
- `AVIPAN` animation
- Hardware zero-copy texture import
- New visual settings
- Bundled FFmpeg installers or cross-platform release packaging
- Windowed movie mode

## Reference behavior

DTXManiaNX resolves performance cover art from the chart folder plus `PREIMAGE`, then uses default art when the file is missing (`references/DTXmaniaNX/DTXMania/Stage/06.Performance/InfoBox.cs:20-34`).

DTXManiaNX starts a movie at its chart event time and resolves the movie path relative to the chart (`references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:16-34`, `:465-472`). It compares video time with performance time and seeks when drift exceeds 100 ms (`references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:266-285`). Its AVI setting gates renderer visibility (`references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:319-332`).

DTXManiaNX resolves image events through BMP, BGA, and BGAPAN registries and starts each layer at the chip playback time (`references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1370-1476`, `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfBGA.cs:61-96`). This implementation covers direct BMP/BGA image display. Pan animation remains deferred.

Bevy has no built-in video texture support in 0.19. The upstream request remains open and marked as needing design: <https://github.com/bevyengine/bevy/issues/5221>. The thread recommends `video-rs` when FFmpeg and LGPL requirements are acceptable. `video-rs` 0.11 is independent of Bevy versions and exposes timestamps, seek operations, resizing, and decoder output suitable for texture upload.

## Architecture

```text
DTX file
  |
  +-- dtx-core
  |     parse #BMPxx/#AVIxx and timed visual chip sequences
  |     resolve chart-relative files case-insensitively
  |
  +-- game-menu::song_loading
  |     publish source directory, registries, and timed visual events
  |
  +-- dtx-bga
  |     static image layer renderer
  |     video-rs decode worker
  |     bounded latest-frame queue
  |     reusable Bevy Image texture
  |
  +-- gameplay-drums
        mirror GameplayClock into BgaClock
        apply visual settings live
        load #PREIMAGE into Now Playing
```

### `dtx-core`

The current parser treats a whole BGA channel value as one decimal asset index and loses fractional timing. Standard DTX visual channels use the same two-character slot sequence as other chip channels. Parsing will store:

- `Chip.wav_slot`: BMP or AVI asset ID
- `Chip.value`: fractional position within the measure
- `Chip.measure`: measure index

`BgaEvent` will copy the asset ID and fraction from those fields. Event sorting will use measure, fraction, then stable source order. Timing conversion will use existing BPM and bar-length timing data rather than `BgaEvent::approx_ms`.

A generic chart-asset resolver will perform direct lookup first, then a case-insensitive filename match in the chart directory. Audio and visual callers should share it instead of maintaining two filesystem algorithms.

### `game-menu`

`SongLoading` will publish enough data for visual playback:

- chart source directory
- BPM-aware target time for each visual event
- resolved BMP paths by ID
- resolved AVI paths by ID

Image handles can start loading during SongLoading. Movie decoding stays on a worker and must not block Bevy's render thread.

### `dtx-bga`

`dtx-bga` owns visual event state, image layers, movie worker state, and the texture receiving decoded frames.

A movie worker opens one referenced movie through `video-rs`, decodes frames outside the main thread, and sends timestamped RGB/RGBA frames through a bounded channel with capacity two. The producer or consumer drops stale frames when the queue is full. The Bevy system reuses one `Image` handle and replaces its pixel bytes when a due frame arrives.

The movie uses aspect-fit scaling across the gameplay viewport. Letterbox regions remain transparent or black according to the existing stage background. Movie and BGA entities use negative UI Z indices so lanes and HUD remain visible.

### Gameplay clock bridge

`dtx-bga` must follow `GameplayClock`, not raw `AudioClock`. Raw audio position is stream-local, can be absent when BGM is disabled, and does not represent practice seeks by itself.

`gameplay-drums` will copy current chart time into a small `dtx_bga::BgaClock` resource. Existing pause and seek logic already controls `GameplayClock`, so visual playback receives the same timeline without creating a second clock model.

## Runtime flow

### Song entry

1. Parse the chart and build BPM-aware visual events.
2. Resolve BMP and AVI paths relative to the chart.
3. Begin loading referenced BMP images.
4. Reset BGA layer state and movie worker state.
5. Enter Performance through the existing loading gate.

### Static image event

1. Ignore the event when `bga_enabled` is false.
2. Find the resolved BMP handle for its asset ID.
3. Replace the entity assigned to that BGA layer.
4. Apply `bg_alpha / 255.0` to the image color.
5. Warn once and leave the layer unchanged or transparent when the asset is unavailable.

### Movie event

1. Ignore the event when `movie_enabled` is false.
2. Start or retarget the decode worker for the referenced AVI path.
3. Treat frame timestamps as offsets from the movie event's chart time.
4. Display the newest decoded frame whose timestamp is due.
5. Drop older queued frames.
6. Seek and flush the queue when movie time differs from desired time by more than 100 ms.
7. Apply `movie_alpha / 255.0` to the rendered image.

The movie's embedded audio stream is ignored. DTX chart BGM and keysounds remain authoritative.

### Pause and seek

Pause freezes `GameplayClock`; the renderer keeps the last frame. The bounded queue prevents unbounded predecode. Practice seek resets the event cursor, reconstructs active static layers up to the destination, and sends a movie seek command for the active movie. Restart resets all event and decoder state.

### Exit

Stage exit sends a stop command, releases worker and texture state, and despawns visual entities. Cleanup must remain idempotent because several transition routes can leave Performance.

## Settings

Customize will expose four rows under System visuals:

| Label | Config field | Control |
|---|---|---|
| BGA Images | `system.bga_enabled` | On/Off |
| Chart Movie | `system.movie_enabled` | On/Off |
| BGA Opacity | `system.bg_alpha` | 0-100% slider mapped to 0-255 |
| Movie Opacity | `system.movie_alpha` | 0-100% slider mapped to 0-255 |

The current comments that associate `bga_enabled` with AVI and `movie_enabled` with BGA are reversed. The implementation will correct those comments and consistently map BGA to static images and Movie to AVI playback. Serialized field names will not change.

`apply_draft_live` will update a `dtx_bga::BgaSettings` resource. Existing save-on-close behavior will persist the draft. Turning a setting off immediately hides its renderer; turning it on resumes from current chart time.

## Now Playing cover

`NowPlayingArt` will become an image node inside the existing 60 by 60 area. When `ActiveChart` changes, the HUD will resolve `metadata.preimage_filename` against `source_path.parent()` and load it through Bevy. The image uses a square cover crop. Missing metadata, missing files, and load failures leave the current neutral tile visible.

This work will not add a default binary cover asset.

## Failure handling

- Missing BMP: warn once, keep the affected layer transparent or unchanged.
- Missing AVI: warn once, continue static BGA and gameplay.
- Unsupported codec or decoder failure: stop the current movie, preserve gameplay.
- Frame size change: recreate the reusable texture for the new dimensions.
- Decode worker disconnect: clear worker state and log one error.
- FFmpeg initialization failure: disable movie playback for the process; static images still work.

The desktop build will require FFmpeg development and runtime libraries. Linux CI will install `libavcodec-dev`, `libavformat-dev`, `libavutil-dev`, and `libswscale-dev`. Packaging those libraries for Windows and macOS remains separate release work.

## Testing

Implementation follows test-first development.

### `dtx-core`

- Parse multi-slot BGA and AVI channel lines into correct IDs and fractions.
- Preserve event order at equal times.
- Compute timing through BPM and bar-length changes.
- Resolve exact and case-insensitive visual filenames.

### `dtx-bga`

- Gate images and movies independently.
- Map opacity bytes to render alpha.
- Replace only the targeted image layer.
- Select the newest due frame from a bounded queue.
- Drop stale frames without blocking.
- Trigger seek when drift exceeds 100 ms.
- Reset worker, texture, and event state on exit.
- Decode a small committed AVI fixture through `video-rs`.

### `gameplay-drums`

- Apply all four visual settings live.
- Keep settings persistence behavior unchanged.
- Set cover image for a valid `#PREIMAGE`.
- Keep fallback tile for missing cover art.
- Mirror paused and sought gameplay time into `BgaClock`.

## Performance limits

- Decode never runs on Bevy's main thread.
- Frame queue capacity is two.
- Movie rendering reuses one texture handle.
- The main thread uploads at most one movie frame per Bevy frame.
- The consumer drops frames rather than delaying gameplay.
- Initial implementation uses software RGB conversion and texture upload. Hardware zero-copy needs profiling evidence before implementation.

## Dependencies and build

- Add `video-rs = { version = "0.11", features = ["ndarray"] }` to workspace dependencies.
- Add `video-rs` to `dtx-bga`.
- Keep `unsafe_code = "forbid"` for workspace code. Native decoder dependencies may contain unsafe internals behind their safe APIs.
- Add FFmpeg development packages to Linux CI.

`bevy_movie_player` is not used because its current release targets Bevy 0.18, wraps the same decoder, lacks chart-clock synchronization, and contains assumptions this project would need to replace. `bevy_av1` is not used because DTX libraries contain legacy AVI and MPEG assets that cannot be required to transcode.

## Verification

Before handoff:

```sh
cargo test -p dtx-core --lib
cargo test -p dtx-bga
cargo test -p gameplay-drums --lib
cargo check -p game-menu
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Manual verification uses a chart with `#PREIMAGE`, at least one `#BMP` event, and one `#AVI` event. Evidence must show:

- cover visible in the top-right card
- movie aspect-fit behind lanes and HUD
- BGA and Movie toggles work during performance
- both opacity controls apply live
- pause freezes the frame
- practice seek resumes the correct movie time
- missing visual files do not stop gameplay

## Acceptance criteria

1. Standard chart BGA and movie events retain correct asset IDs and fractional timing.
2. Static chart images render instead of placeholders.
3. Chart movies decode through FFmpeg and remain synchronized to gameplay time.
4. Lanes and HUD stay above fullscreen movie output.
5. Existing four visual settings work live and persist.
6. The Now Playing card shows `#PREIMAGE` when available and a neutral fallback otherwise.
7. Pause, seek, restart, and stage exit leave no stale visual or decoder state.
8. Missing or invalid visual assets never interrupt gameplay.
9. Relevant package tests, workspace check, and clippy pass.
