# 0002: Audio-clock authoritative for judgment timing

Status: accepted
Date: 2026-06-23

## Context

DTXmaniaNX uses an audio-buffer clock (`CSoundTimer`) interpolated between sound
buffer positions, **not** `Time::delta()` accumulated frame delta.

Frame-based timing drifts: vsync jitter, frame drops, paused game time, alt-tab
backgrounding all corrupt the judge clock.

## Decision

`dtx-timing::AudioClock` is the **only** clock for hit-window judgment.
Sourced from `bevy_kira_audio`'s position callback (BGM playback position in ms).
`Time::delta()` is for visuals/scroll only.

## Consequences

- Hit windows are exact regardless of frame rate.
- Alt-tabbing pauses BGM (kira) → judgment correctly pauses too.
- Replay validation works deterministically.
- `dtx-audio` must expose the clock as a `Resource` updated each frame.

## Alternatives considered

- **Frame clock (Time<Virtual>):** wrong, drifts under load.
- **SystemTime:** monotonic but not synced to audio buffer.

## Reference files

- `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs` — original implementation