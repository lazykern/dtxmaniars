# Decision Records

These records reconstruct only decisions supported by current code, product
documents, comments, tests, or vendored-reference evidence. No original date is
invented; “reconstructed” dates describe this repository repair.

## Accepted

- [ADR-0001 — Drums-first product scope](0001-drums-first-product-scope.md)
- [ADR-0002 — Gameplay audio-clock authority](0002-gameplay-audio-clock-authority.md)
- [ADR-0003 — Read-only reference inputs](0003-read-only-reference-inputs.md)
- [ADR-0004 — Reference-first mechanics workflow](0004-reference-first-mechanics-workflow.md)
- [ADR-0005 — Crate layering](0005-crate-layering.md)
- [ADR-0009 — Input profiles as source of truth](0009-input-profiles-source-of-truth.md)
- [ADR-0010 — Port mechanics, redesign UX](0010-port-mechanics-redesign-ux.md)
- [ADR-0014 — 300 ms OutQuint screen transitions](0014-outquint-screen-transitions.md)
- [ADR-0015 — Preview crossfade ownership](0015-preview-crossfade-ownership.md)
- [ADR-0016 — Qualified score persistence](0016-qualified-score-persistence.md)

The atomic multi-target input decision is also binding through its approved
[design](../superpowers/specs/2026-07-12-atomic-multi-target-bindings-design.md)
and executable lane-resolution tests; it has not been assigned a reconstructed
ADR number.

## Unreconstructed or superseded labels

- **ADR-0007 — Unreconstructed.** Only a legacy tween comment survives; there
  is not enough independent evidence to recreate the original decision.
- **ADR-0008 — Superseded label.** Some handbooks used this number for the
  reference-first workflow now reconstructed as ADR-0004. It is not separately
  binding.
- **ADR-0011 — Unreconstructed.** Historical comments mention framebuffer
  snapshots, but no binding original decision or implemented requirement is
  independently established.

New decisions use the complete Context / Decision / Evidence / Consequences /
Supersedes schema and must link from this index.
