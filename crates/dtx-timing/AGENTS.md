# crates/dtx-timing

Engine-layer bridge from the active Kira BGM instance to the authoritative
`AudioClock`. Pure chart-time math remains owned by and re-exported from
`dtx-core::timing`.

## Current contract

- `AudioClock.current_ms` is `None` when no BGM is active and `Some(ms)` while
  the tracked instance is playing or paused.
- `update_audio_clock_system` polls `dtx_audio::BgmHandle` every update; it does
  not accumulate `Time::delta()`.
- Gameplay may define explicit no-BGM behavior, but BGM-backed judgment waits
  for this clock. Practice pause/seek/rate behavior must preserve the same
  chart-time authority.
- `math` covers BPM segments, fractional changes, bar-length changes, and
  monotonic chip-time conversion. Parser semantics belong in `dtx-core`.

Input offset is a judgment-layer correction, not an alternate clock. Sub-frame
render interpolation is owned by gameplay and must not feed back into judgment.

## Reference evidence

- `references/DTXmaniaNX/FDK/Sound/CSoundTimer.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/CChip.cs`

[ADR-0002](../../docs/decisions/0002-gameplay-audio-clock-authority.md) owns
the cross-crate clock decision.

## Verify

```sh
cargo test -p dtx-timing --lib
cargo test -p dtx-timing --test bpm_segment
cargo test -p dtx-timing --test compatibility_timing
cargo check -p dtx-timing
```
