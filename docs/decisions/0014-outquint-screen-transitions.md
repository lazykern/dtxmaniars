# ADR-0014 — 300 ms OutQuint Screen Transitions

Status: Accepted (reconstructed 2026-07-13)

## Context

NX uses a 1500 ms linear snapshot fade. That is valid comparison evidence but
conflicts with the implemented responsive product transition.

## Decision

DTXManiaRS uses a 300 ms OutQuint black-overlay transition directed by
`TransitionRequest`. Reduced Motion may shorten the duration through the
accessibility policy. Stage mechanics remain reference-first; transition UX is
owned by this product decision.

## Evidence

- [`SCREEN_TRANSITION_MS`](../../crates/dtx-ui/src/theme.rs)
- [OutQuint fade reducer](../../crates/dtx-ui/src/transition.rs)
- [state transition director](../../crates/game-shell/src/transition.rs)
- `references/DTXmaniaNX/DTXMania/Core/StageManager.cs:29`

## Consequences

Screens request transitions instead of setting state directly. Handbooks must
not describe the NX 1500 ms behavior as the product runtime.

## Supersedes / Superseded By

Reconstructs ADR-0014 and supersedes contradictory milestone-era handbook
text; superseded by none.
