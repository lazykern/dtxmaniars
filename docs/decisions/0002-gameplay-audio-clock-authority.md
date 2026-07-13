# ADR-0002 — Gameplay Audio-Clock Authority

Status: Accepted (reconstructed 2026-07-13)

## Context

Frame delta alone drifts from decoded audio, while gating gameplay on an audio
position can freeze charts that have delayed, absent, or restarting BGM.

## Decision

`GameplayClock` is the authoritative chart-time clock. It advances from fixed
update time, snaps to the first measured BGM position, then uses later Kira
positions for bounded drift correction. Judgment never directly accumulates
render-frame `Time::delta()` and never requires a live BGM position to advance.

## Evidence

- [`GameplayClock::tick`](../../crates/gameplay-drums/src/resources.rs)
- [`AudioClock` bridge](../../crates/dtx-timing/src/lib.rs)
- `references/DTXmaniaNX/FDK/Sound/CSoundTimer.cs`

## Consequences

Notes, audio, visuals, seek, and stage completion can share chart time without
an audio-start deadlock. Clock changes require drift, restart, and no-BGM tests.

## Supersedes / Superseded By

Reconstructs ADR-0002 comments in timing/audio crates; superseded by none.
