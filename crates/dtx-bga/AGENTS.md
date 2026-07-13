# crates/dtx-bga

Engine-layer chart visual renderer. It converts parsed visual registrations and
chips into deterministic chart-time state, renders image layers, and decodes
movies with `video-rs`/system FFmpeg.

## Current contract

- `ActiveChartRes::from_chart` resolves BMP/AVI registries and builds timed
  replace, eight-scope swap, BGAPAN, and AVIPAN events with BPM/bar timing.
- `visual_state_at*` reconstructs the exact image/movie state after forward or
  backward seeks, loop restarts, and song restarts.
- Pan state interpolates bounded source/destination rectangles on the authored
  1280x720 stage. With Background Motion disabled, pans resolve to their end
  state and movies do not start.
- Static images render as layer-specific `ImageNode`s. A bounded movie worker
  decodes off-thread, reuses one RGBA texture, follows `BgaClock`, and seeks on
  discontinuity. Movie audio is ignored.
- `BgaSettings` gates images/movies and applies alpha live; `BgaParent` lets the
  Game layer attach visuals to its stage root.

## Ownership boundary

Depends on Pure chart/config data and Engine timing. `gameplay-drums` bridges
its authoritative `GameplayClock` into `BgaClock`; do not create a second
visual clock. Invalid optional visuals degrade with a warning and must not alter
the playable drum timeline. Windowed movie modes, embedded movie audio,
hardware zero-copy, and bundled FFmpeg packaging are not implemented.

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfBGA.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs`
- `references/DTXmaniaNX/DTXMania/Core/Video/FFmpegCore.cs`
- `references/DTXmaniaNX/DTXMania/Core/Video/VideoPlayerController.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs`

## Verify

```sh
cargo test -p dtx-bga --lib
cargo test -p dtx-bga --test integration_bga
cargo check -p dtx-bga
```

Actual movie presentation and installed FFmpeg codecs require a manual GUI
check.
