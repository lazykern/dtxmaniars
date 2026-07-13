# crates/dtx-core — agent scope

**Layer:** Pure (no bevy).
**Milestone:** M0.
**Status:** Active.

## Purpose

Parse `.dtx` files into [`Chart`] / [`Chip`] / [`EChannel`].
Testable in isolation, no GPU, no bevy.

## Test

```sh
cargo test -p dtx-core
```

## Reference files

- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1` — original parser (7295 LOC)
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs:1` — channel enum
- `references/DTXmaniaNX/DTXMania/Score,Song/CChip.cs:1` — chip data model
- `references/DTXmaniaNX/DTXMania/Score,Song/CChartData.cs:1` — chart container

## v1 scope (M0 → M2)

- Header commands: TITLE, ARTIST, GENRE, MAKER, COMMENT, BPM, DLEVEL/GLEVEL/BLEVEL, PREVIEW, PREIMAGE
- Chip lines `#MMMCC:` for all channels in `EChannel`
- BGM channel emits a marker chip (no WAV filename binding yet)
- BPM/BPMEx/BarLength parse as `f32`
- Skip unknown channels silently (forward compatibility)

## Deferred (later milestones)

- BPM base64 decoding (M1)
- Long-note (`#LNTYPE`/`#LNOBJ`) tracking (M6+)
- Guitar wailing channels (M6+)
- BGA / AVI bindings (M6+)

## Rules

- No `bevy` dependency.
- Errors via `DtxError` (`thiserror`).
- Public API stable; internal refactors free.
- **Port-first (ADR-0010):** parser semantics must match DTXManiaNX's
  `CDTX.cs` and `EChannel.cs` verbatim. Do not invent commands or simplify
  away edge cases (BPM base64, BGM markers, etc.) without a documented reason.