# crates/dtx-timing — agent scope

**Layer:** Engine (bevy + kira position polling).
**Milestone:** M1.
**Status:** Active.

## Purpose

Owns the authoritative `AudioClock` resource. Hit-window judgment reads this
— never `Time::delta()`. See ADR-0002.

## API

```rust
use dtx_timing::{plugin, AudioClock};

app.add_plugins((dtx_audio::plugin, dtx_timing::plugin));

// In a system:
fn judge_lane(clock: Res<AudioClock>, lane_hit: EventReader<LaneHit>) {
    for hit in lane_hit.read() {
        let delta = clock.ms_or_zero() - hit.target_ms;
        let kind = dtx_scoring::classify(delta); // ← see crates/dtx-scoring
    }
}
```

## Reference files

- `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs:1` — original wall-clock-based timing (92 LOC). Our approach: kira gives position-in-seconds directly.

## Design decisions

- `AudioClock.current_ms: Option<i64>` — None means "no BGM / not playing".
- `gameplay-drums::GameplayClock` only falls back to wall-clock for explicit
  no-BGM charts; BGM-backed charts wait for audio position.
- Update system runs every Update; cost = 2 Res reads + match. No allocations.
- Time-math helpers are defined in `dtx_core::timing` and re-exported as `dtx_timing::math`.

## v1 scope (M1)

- `AudioClock` resource
- `update_audio_clock_system` 
- `plugin` to register
- Pure helpers: `delta_ms`, `chip_time_ms`

## Deferred

- BPM-change chip handling (M2)
- Audio latency compensation (M2, after measuring kira + output buffer)
- Sub-frame interpolation between kira position callbacks (M2 if jitter visible)

## Rules

- `dtx-timing` depends on `dtx-audio` (Engine → Engine, allowed).
- Pure math lives in `dtx-core`; keep the `dtx_timing::math` re-export for API compatibility.
- **Port-first (ADR-0010):** AudioClock approach must match DTXManiaNX's
  `CSoundTimer` semantics (BocuD simplified via kira position, that's fine).
  But do not introduce your own frame-based fallback.