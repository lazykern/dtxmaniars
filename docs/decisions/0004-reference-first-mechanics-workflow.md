# ADR-0004 — Reference-First Mechanics Workflow

Status: Accepted (reconstructed 2026-07-13)

## Context

Rhythm mechanics are sensitive to small guessed differences. The repository
contains a known NX implementation and many source comments already cite it.

## Decision

Before changing chart parsing, timing, judgment, scoring, lane semantics, or
default input behavior, inspect the relevant file under
`references/DTXmaniaNX/`. Port the proven mechanic first, cite the evidence,
then document any deliberate deviation.

## Evidence

- [Mandatory reference workflow](../../AGENTS.md)
- [Mechanics/UX boundary](0010-port-mechanics-redesign-ux.md)
- Parser provenance in [`dtx-core`](../../crates/dtx-core/src/parser.rs)

## Consequences

Mechanics reviews start from source comparison rather than memory. This policy
does not require copying legacy presentation or obsolete implementation limits.

## Supersedes / Superseded By

Reconstructs ADR-0004 and supersedes the undocumented ADR-0008 label previously
used by some crate handbooks.
