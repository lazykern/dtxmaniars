# crates/dtx-bga

Engine-layer crate. BGA / video playback for DTXManiaNX charts.

## Reference (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfBGA.cs` (305 lines)
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs` (520 lines)
- `references/DTXmaniaNX/DTXMania/Core/Video/FFmpegCore.cs` (FFmpeg wrapper, M7.1+)
- `references/DTXmaniaNX/DTXMania/Core/Video/VideoPlayerController.cs`
- `references/DTXmaniaNX/DTXMania/Core/Video/UINewVideoRenderer.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs` (BGA channels)

## M7.1 scope (implemented)

- `chart::ActiveChartRes::from_chart` resolves `#BMP`/`#BGA`/`#AVI` registries
  to absolute paths (case-insensitive) and builds BPM/bar-length-aware
  `TimedVisualEvent`s on the same timeline drum chips use.
- Static `#BMP` image layers render as real `ImageNode`s (`BgaLayerOverlay`),
  replacing colored placeholders; each event replaces only its target layer.
- `#AVI` movies decode through `video-rs`/FFmpeg on a `MovieWorker` thread with
  a bounded (capacity two) frame queue; the newest due RGBA frame uploads into
  one reusable `Image` texture shown aspect-fit fullscreen behind lanes/HUD
  (`MovieOverlay`), synced to `BgaClock` (mirrored from `GameplayClock`), with a
  100 ms drift-seek threshold. Movie audio is ignored.
- `BgaSettings` (from `dtx_config::SystemConfig`) gates images/movie and applies
  opacity live; toggling and alpha update without respawn.
- Practice seek / restart rebuild the event cursor, static layers, and movie
  target; `clear_visuals` tears down workers, texture, and overlays on exit.

## Deferred

- `BGAPAN` / `AVIPAN` pan+zoom animation
- Windowed (non-fullscreen) movie mode
- Movie embedded-audio playback
- Hardware zero-copy texture import
- FFmpeg bundling for Windows/macOS release packaging

## Layer

Engine. Sits between `dtx-core` (Pure) and gameplay crates (Game).