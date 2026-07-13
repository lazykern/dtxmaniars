# crates/dtx-audio

Engine-layer wrapper around `bevy_kira_audio`. It owns runtime audio handles,
chart-sound caching and polyphony, preview crossfades, and the BGM position
polled by `dtx-timing`.

## Current contract

- `BgmHandle` tracks the single authoritative BGM instance and asset path.
- BGM helpers support start offsets, volume/mix, pause/resume, stop, and
  playback-rate changes. Non-`1.00x` playback changes pitch.
- `ChartSoundBank` resolves Windows separators and path components
  case-insensitively, caches OGG/WAV/MP3 handles by WAV slot, and retains chart
  volume/pan.
- `DrumPolyphony` owns 1–8 round-robin voices per WAV slot; gameplay owns lane
  selection, choke, and hit-sound priority.
- `PreviewPlayer` and `AudioHandleCache` own song-select preview fade/swap
  behavior. Screen transitions publish intent; audio owns preview fade state
  per [ADR-0015](../../docs/decisions/0015-preview-crossfade-ownership.md).
- XA classification/substitution policy is decided by loading/import callers;
  this crate deliberately has no XA decoder.

## Ownership boundary

Never derive judgment time from frame delta. Expose playback position for
`dtx-timing`; do not move game-specific lane or result policy here. Movie audio
is not part of this crate's current chart-movie path.

## Reference evidence

- `references/DTXmaniaNX/FDK/Sound/CSoundTimer.cs` — timer semantics
- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs` — chart WAV playback and polyphony

Audio-clock authority is [ADR-0002](../../docs/decisions/0002-gameplay-audio-clock-authority.md).

## Verify

```sh
cargo test -p dtx-audio
cargo test -p dtx-audio --test mp3_decode
cargo check -p dtx-audio
```

Audible output, output latency, and device switching require a manual hardware
check.
