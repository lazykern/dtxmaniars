# ADR-0016 — Qualified Score Persistence

Status: Accepted (reconstructed 2026-07-13)

## Context

Scores have no modifier-aware schema. Persisting assisted or speed-modified
runs beside native play would misrepresent personal bests and compatible
`score.ini` history.

## Decision

Only normal play at 1.00× without No Fail writes native history and compatible
`score.ini`. Modified-speed, Practice, and No Fail results stay visible with an
explicit non-qualifying reason but do not mutate ordinary records. Future
assists remain non-qualifying until a modifier-aware schema is designed.

## Evidence

- [`CompletedRunContext` and modifiers](../../crates/game-shell/src/states.rs)
- [result persistence gate](../../crates/game-results/src/lib.rs)
- [non-qualifying result copy](../../crates/game-results/src/ui.rs)
- [playback-rate source](../../crates/gameplay-drums/src/playback_rate.rs)

## Consequences

Personal-best comparison remains meaningful. Any new modifier must make an
explicit qualification decision and carry that status into Results.

## Supersedes / Superseded By

Introduces the binding record for implemented score qualification; superseded
by none.
