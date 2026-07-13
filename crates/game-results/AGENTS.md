# crates/game-results

Game-layer Results screen. It snapshots the completed drums run, presents
analysis and actions, and owns qualification-aware native and compatible score
persistence.

## Current contract

- On Result entry, snapshot score/judgments/combo/chart, normal-play timing
  analysis, PB delta, and weakest-section recommendation before saving.
- Rank and skill use `dtx-scoring` reference-derived formulas.
- Save only normal `1.00x` Standard-fail runs. Practice, modified-speed, and No
  Fail runs show explicit non-saving status.
- Qualifying runs append the versioned native store and merge
  `<chart>.score.ini`; either write failure is visible as failed save status.
- The animated reveal is skippable. Continue/Retry/Practice share keyboard and
  kit navigation; a weak-section Practice action carries loop/pre-roll intent
  through `game-shell`.

## Ownership boundary

Reads gameplay snapshots but must not mutate gameplay mechanics. Navigation
emits `TransitionRequest`; only the shell transition director changes
`AppState`. Persistence format/math stays in `dtx-scoring`, and event collection
stays in `gameplay-drums`.

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Stage/07.Result/CStageResult.cs`
- `references/DTXmaniaNX/DTXMania/Stage/07.Result/CActResultParameterPanel.cs`
- `references/DTXmaniaNX/DTXMania/Stage/07.Result/ResultRankIcon.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/CScoreIni.cs`

Score qualification is [ADR-0016](../../docs/decisions/0016-qualified-score-persistence.md).

## Verify

```sh
cargo test -p game-results --lib
cargo check -p game-results
```
