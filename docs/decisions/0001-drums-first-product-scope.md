# ADR-0001 — Drums-First Product Scope

Status: Accepted (reconstructed 2026-07-13)

## Context

The reference application exposes drums, guitar, and bass instrument parts.
DTXManiaRS has working drums and a narrower guitar mode, but the approved
product program and distance/accessibility work target electronic drummers.

## Decision

Drums is the default and primary product scope. Guitar may remain available as
an implemented secondary mode; bass and broader instrument expansion do not
block drums roadmap completion.

## Evidence

- [Product purpose and users](../../PRODUCT.md)
- [`EGameMode` default and available modes](../../crates/game-shell/src/states.rs)
- `references/DTXmaniaNX/DTXMania/Core/CConstants.cs:158`

## Consequences

Player documentation leads with drum-kit workflows, tests protect drums first,
and proposed cross-instrument work must preserve the stable drum contract.

## Supersedes / Superseded By

Reconstructs the decision cited by `EGameMode`; superseded by none.
