# ADR-0009 — Input Profiles as Source of Truth

Status: Accepted (reconstructed 2026-07-13)

## Context

Legacy single-file bindings and layout snapshots cannot safely represent named
keyboard, MIDI, and lane configurations or transactional Customize edits.

## Decision

Versioned keyboard and MIDI `ProfileRegistry` files and the lane
`LaneProfileRegistry` are authoritative. Legacy files are migration input only.
Customize edits drafts and commits registries explicitly; lane arrangement
changes display mapping without changing logical judgment identity.

## Evidence

- [Input profile registries and migrations](../../crates/dtx-input/src/profiles.rs)
- [Lane profile registry](../../crates/dtx-layout/src/profiles.rs)
- [Customize profile transactions](../../crates/gameplay-drums/src/editor/mod.rs)

## Consequences

Failed saves preserve drafts, migrations are idempotent, and compatibility
snapshots cannot silently overwrite authoritative registries.

## Supersedes / Superseded By

Supersedes the historical “input deferred” interpretation of ADR-0009;
superseded by none.
