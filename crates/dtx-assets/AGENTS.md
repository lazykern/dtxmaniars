# crates/dtx-assets

Engine-layer adapter for loading supported chart files into
`dtx_core::Chart` values. It owns file I/O and an in-memory `DtxCache`; parsing
semantics stay in `dtx-core` and media loading stays in `dtx-audio`/`dtx-bga`.

## Current contract

- `load_dtx` / `load_dtx_report` force DTX parsing.
- `load_chart` / `load_chart_report` select DTX, GDA, or G2D from a
  case-insensitive extension and reject other formats.
- Report APIs preserve recoverable parser warnings.
- `DtxCache::get_or_load` caches by `PathBuf`; `DtxAssetsPlugin` registers it.
- There is no Bevy `AssetLoader` implementation and no audio/image decoding in
  this crate.

## Ownership boundary

May depend on Bevy and Pure `dtx-core`. Do not add Game-crate dependencies or
duplicate format semantics from `dtx-core`. Unsupported/degraded policy must
remain consistent with [Compatibility](../../docs/compatibility.md).

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs` — chart loading/parsing
- `references/DTXmaniaNX/DTXMania/Score,Song/CChartData.cs` — chart container
- `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` — loading-stage behavior

Mechanics use [ADR-0004](../../docs/decisions/0004-reference-first-mechanics-workflow.md);
the reference tree is read-only.

## Verify

```sh
cargo test -p dtx-assets
cargo check -p dtx-assets
```
