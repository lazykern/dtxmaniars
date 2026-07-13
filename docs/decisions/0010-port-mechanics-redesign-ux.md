# ADR-0010 — Port Mechanics, Redesign UX

Status: Accepted (reconstructed 2026-07-13)

## Context

The project needs DTX-compatible mechanics without inheriting a legacy arcade
layout, transition style, or input-device assumptions.

## Decision

Chart semantics, timing, judgment, scoring, lane behavior, and mechanical
defaults are reference-first. Navigation, visual hierarchy, animation,
accessibility, and responsive layout are product UX and may be redesigned as
long as they do not change the mechanic.

## Evidence

- [Product design principles](../../PRODUCT.md)
- [Root mechanics boundary](../../AGENTS.md)
- Strict-port comments in [`dtx-core`](../../crates/dtx-core/src/assets.rs)
- Independent UI implementation in [`dtx-ui`](../../crates/dtx-ui/src/lib.rs)

## Consequences

Every change must identify which side of the boundary it affects. UX records
cannot silently redefine gameplay, and reference screenshots do not bind the
new visual system.

## Supersedes / Superseded By

Reconstructs ADR-0010 comments across the workspace; superseded by none.
