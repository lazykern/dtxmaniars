# crates/gameplay-guitar

Game-layer experimental Guitar scaffold. It is compiled and tested, but it is
not part of the maintained drums-first player compatibility contract and there
is currently no player-facing mode-selection workflow.

## Implemented scaffold

- Five logical lanes R/G/B/Y/P with default A/S/D/F/G keyboard input.
- Mode-gated input, scroll, nearest-note judgment, score/combo resources, a
  minimal HUD, and end-of-chart orchestration.
- Reference-derived state reducers for lane flush, RGB, danger, gauge,
  wailing-bonus, bonus, and a `HoldNote` data type.
- `EGameMode::Guitar` allows tests/embedding code to select the path; the
  desktop application defaults to Drums.

This is not faithful full Guitar gameplay. Chord channels collapse to one
lowest lane, hold/wailing state types are not a complete playable pipeline,
Bass is absent, and Results/persistence/product discovery are drums-owned. Do
not advertise Guitar as Supported or extend the player guide without a new
approved product scope and executable compatibility contract.

## Ownership boundary

Mirrors drums only where useful for isolated mechanics. Shared primitives
belong in existing Pure/Engine crates; do not make drums depend on this
experimental scaffold. Cross-screen state remains in `game-shell`.

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CActPerfGuitarLaneFlushGB.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CActPerfGuitarScore.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs`

The product-scope decision is [ADR-0001](../../docs/decisions/0001-drums-first-product-scope.md).

## Verify

```sh
cargo test -p gameplay-guitar --lib
cargo test -p gameplay-guitar --test guitar_play
cargo check -p gameplay-guitar
```
