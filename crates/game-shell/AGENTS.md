# crates/game-shell

Game-layer owner of application states, pause state, cross-screen navigation
types, shared run/practice intent, score-store resource, and the product
transition director.

## Current contract

- `AppState` models Startup, Title, Config, SongSelect, SongLoading,
  Performance, Result, and End.
- `PauseState` is orthogonal to `AppState`; gameplay owns pause mechanics.
- `TransitionRequest` is the only stage-to-stage request path. The director
  owns `NextState<AppState>`, serializes requests, fades out, switches state,
  and fades in.
- Product transitions are 300 ms OutQuint black overlays under
  [ADR-0014](../../docs/decisions/0014-outquint-screen-transitions.md). Reduced
  Motion shortens the product policy.
- `PracticeIntent`, `PracticeRecommendation`, `CompletedRunContext`, and
  qualification modifiers are dependency-neutral handoff types.
- `EGameMode` retains Drums/Guitar runtime vocabulary, but the maintained
  player product defaults to Drums and currently exposes no Guitar selection
  workflow. Enum reachability is not a public support claim.

## Reference comparison

DTXManiaNX uses a 1500 ms linear captured-snapshot transition in
`StageManager.cs`. That is comparison evidence, not the DTXManiaRS UX contract.

- `references/DTXmaniaNX/DTXMania/Core/StageManager.cs`
- `references/DTXmaniaNX/DTXMania/Stage/CStage.cs`
- `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs`

Mechanics follow [ADR-0004](../../docs/decisions/0004-reference-first-mechanics-workflow.md);
transition presentation follows ADR-0014.

## Ownership boundary

Stage systems emit requests and tag stage entities for cleanup; they never
write `NextState<AppState>` directly. Keep shell types free of concrete
`gameplay-drums` internals. Current dependencies are `dtx-ui`, `dtx-audio`, and
`dtx-scoring`; add new Game/Engine edges only for shell-owned coordination.

## Verify

```sh
cargo test -p game-shell --lib
cargo test -p game-shell --test all_stages_reachable
cargo check -p game-shell
```
