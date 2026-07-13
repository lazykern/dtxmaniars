# crates/dtx-audio — agent scope

**Layer:** Engine (bevy + bevy_kira_audio).
**Milestone:** M1.
**Status:** Active.

## Purpose

Thin wrapper around `bevy_kira_audio`. Owns the [`BgmHandle`] resource.
`dtx-timing` reads it each frame to populate `AudioClock`.

## API

```rust
use dtx_audio::{plugin, play_bgm, stop_bgm, position_ms, BgmHandle};

app.add_plugins((DefaultPlugins, dtx_audio::plugin));

// In a system:
fn start_bgm(audio: Res<Audio>, mut bgm: ResMut<BgmHandle>) {
    play_bgm(&audio, &mut bgm, "songs/track1.ogg");
}

// Query playback position:
fn tick(audio: Res<Audio>, bgm: Res<BgmHandle>) {
    if let Some(ms) = position_ms(&audio, &bgm) { /* ... */ }
}
```

## Reference files

- `references/DTXmaniaNX/FDK/Sound/CSoundTimer.cs` — original wall-clock-based timing reference (92 LOC). Our approach is cleaner: kira position is already ms-accurate.
- bevy_kira_audio docs via `npx ctx7@latest docs /niklasei/bevy_kira_audio "<q>"`

## Design decisions

- One BGM stream at a time. Multi-track layering deferred (M6+ if needed).
- `BgmHandle` is `Option<Handle<AudioInstance>>` — None means "no BGM".
- `ChartSoundBank` caches chart WAV handles by slot; chart-specific slot
  collection lives in `gameplay-drums`.
- Hit-sound resolution (per-lane SEs, choke rules) lives in `gameplay-drums`.

## v1 scope (M1)

- `AudioPlugin` registration
- `BgmHandle` resource
- `play_bgm`, `stop_bgm`, `position_ms` helpers
- handle-based BGM/SFX/drum-hit helpers for preloaded chart WAVs
- `ChartSoundBank` + case-insensitive chart audio path resolution
- Looping BGM by default

## Deferred

- BGM fade-in/out (M3 with shell transitions)
- Async/background decode pool (M14+ polish)

## Rules

- Re-export kira/bevy_kira_audio types only when the wrapper adds value.
- Never use `Time::delta()` for audio-clock-equivalent decisions (see ADR-0002).