# crates/dtx-bga

Engine-layer crate. BGA / video playback for DTXManiaNX charts.

## Reference (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfBGA.cs` (305 lines)
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfVideo.cs` (520 lines)
- `references/DTXmaniaNX-BocuD/DTXMania/Core/Video/FFmpegCore.cs` (FFmpeg wrapper, M7.1+)
- `references/DTXmaniaNX-BocuD/DTXMania/Core/Video/VideoPlayerController.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Core/Video/UINewVideoRenderer.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EChannel.cs` (BGA channels)

## M7 scope (this milestone)

- `BgaPlayer` resource: state machine (Idle → Cueing → Displaying → Ended)
- `BgaPlugin`: scans `dtx_core::bga::bga_events(&chart)`, ticks player each frame
- When a BGA event is due (BPM-aware timing via `dtx_timing`), spawn a placeholder `BgaLayer` UI entity
- Movie channels are logged + skipped (M7.1: real FFmpeg decode)

## M7.1+ deferred

- Parse `#BMPxx: filename` and `#AVIxx: filename` directives from DTX header
- Load actual image files via bevy asset server
- Layer1/Layer2/Layer3 positioned overlays with display coords from chip
- BGAPAN animation (size + position tweens)
- FFmpeg-based movie decoding for Movie/MovieFull channels

## Layer

Engine. Sits between `dtx-core` (Pure) and gameplay crates (Game).