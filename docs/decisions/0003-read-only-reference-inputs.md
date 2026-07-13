# ADR-0003 — Read-Only Reference Inputs

Status: Accepted (reconstructed 2026-07-13)

## Context

The vendored NX source is behavioral evidence, not maintained project source.
Editing it would erase provenance and make comparisons untrustworthy.

## Decision

Everything under `references/` is read-only input. Contributors may inspect and
cite it but must not edit, copy into active source wholesale, or include
reference-tree changes in project commits.

## Evidence

- [Root repository handbook](../../AGENTS.md)
- [Contributor guide](../contributing.md)
- The vendored root at `references/DTXmaniaNX/`

## Consequences

Scope audits include `git status --short references`. Required project changes
must be implemented outside the reference tree with an evidence citation.

## Supersedes / Superseded By

Reconstructs the read-only policy cited as ADR-0003; superseded by none.
