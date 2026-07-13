# crates/dtx-core

Pure layer: chart parser, normalized chart/chip/channel model, timing math, and
reference-derived mechanics data. It must remain Bevy-free.

## Current contract

- Parse DTX, GDA, and G2D from UTF-8, Shift-JIS, UTF-16LE BOM, or UTF-16BE BOM.
- Normalize legacy GDA/G2D drum aliases into the DTX model and retain the
  source `ChartFormat`.
- Parse metadata, drum/system/hidden channels, BPM and bar changes,
  WAV/BMP/AVI/BGA registrations, pan definitions, mixer events, and SE01–SE32.
- Resolve deterministic RANDOM/IF/ENDIF branches through `ParseOptions`.
- Return structured line diagnostics for recoverable input and hard errors for
  non-recoverable parsing. Discovery/rejection policy lives in callers.
- Provide chart timing, chip classification/transforms, asset resolution, and
  compatibility structures without runtime I/O dependencies beyond parser
  input.

Long-note and full guitar/bass semantics are outside the maintained drums
compatibility contract. Do not advertise enum presence as playable support.

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs` — parser and playback data
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs` — channel mapping
- `references/DTXmaniaNX/DTXMania/Score,Song/CChip.cs` — chip model/timing
- `references/DTXmaniaNX/DTXMania/Score,Song/CChartData.cs` — chart container
- `references/DTXmaniaNX/DTXMania/Core/CConstants.cs` — instrument constants

Follow [ADR-0004](../../docs/decisions/0004-reference-first-mechanics-workflow.md)
and [ADR-0010](../../docs/decisions/0010-port-mechanics-redesign-ux.md). Keep
`references/` read-only and cite exact reference lines for ported behavior.

## Verify

```sh
cargo test -p dtx-core --lib
cargo test -p dtx-core --test compatibility_matrix
cargo test -p dtx-core --tests
cargo check -p dtx-core
```

`compatibility_matrix` is the primary executable format/encoding/channel
contract; update [Compatibility](../../docs/compatibility.md) when its public
support boundary changes.
