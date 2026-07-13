# ADR-0005 — Crate Layering

Status: Accepted (reconstructed 2026-07-13)

## Context

The workspace separates pure chart/domain logic from Bevy engine adapters,
gameplay plugins, and executable assembly so core behavior stays testable.

## Decision

Dependencies flow Pure → Engine → Game → Binary. Pure crates do not depend on
Bevy or higher layers. Engine crates may use Bevy and pure crates. Game crates
compose domain and engine behavior. Binaries own application assembly only.

## Evidence

- [Workspace members and dependencies](../../Cargo.toml)
- [Layer map](../../AGENTS.md)
- Package manifests under [`crates/`](../../crates)

## Consequences

New dependencies must preserve the direction. Shared pure policy belongs in a
lower layer instead of introducing a cycle between gameplay crates.

## Supersedes / Superseded By

Reconstructs the established workspace layering decision; superseded by none.
