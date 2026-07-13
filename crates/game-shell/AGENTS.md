# crates/game-shell

Game-layer crate. Owns the AppState machine and the product screen-transition
director.

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX/DTXMania/Core/StageManager.cs` — full file (699 lines).
  - Line 29: `private float FadeDurationMs = 1500f;` ← single source for fade duration
  - Lines 645-665: `BeginFadeTransition` — capture snapshot, swap stage, latch fade
  - Lines 670-699: `DrawFadeOverlay` — linear alpha decay, snapshot-on-top
- `references/DTXmaniaNX/DTXMania/Stage/CStage.cs` — `EStage` enum (8 stages)
- `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines)
  - Loading screen behavior (BGM preview, asset preload, progress bar)
- Per-stage refs as work lands:
  - `Stage/01.Startup/CStageStartup.cs`
  - `Stage/02.Title/CStageTitle.cs`
  - `Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`

## Stages (must match DTXManiaNX EStage enum verbatim)

```
AppState::Startup        ← CStageStartup
AppState::Title          ← CStageTitle
AppState::Config         ← CStageConfig
AppState::SongSelect     ← CStageSongSelectionNew (in BocuD)
AppState::SongLoading    ← CStageSongLoading
AppState::Performance    ← CStagePerfDrumsScreen
AppState::Result         ← CStageResult
AppState::End            ← CStageEnd
```

## Fade transition

- **Reference comparison:** DTXManiaNX uses a 1500 ms linear snapshot
  transition.
- **Product decision:** DTXManiaRS uses a 300 ms OutQuint black overlay through
  `TransitionRequest` and `dtx_ui::ScreenFade`.
- **Boundary:** Stage mechanics remain reference-first; transition UX follows
  [ADR-0014](../../docs/decisions/0014-outquint-screen-transitions.md).
- Reduced Motion may shorten the product transition through
  `AccessibilityPolicy`; it does not switch back to the NX transition.

## Rules

- One `Plugin` per stage file, aggregated in `lib::plugin`.
- Stages register their `OnEnter` / `OnExit` systems via the per-stage plugin.
- Stage systems emit `TransitionRequest`; the transition director alone writes
  `NextState<AppState>` after fade-out.
- Per-stage UI entities tagged with a marker component (e.g.
  `TitleEntity`, `LoadingEntity`) so `OnExit` can despawn them via a
  generic `despawn_stage::<T>` system.

## Layer

Game. May depend on `dtx-ui` (Game) and any Engine/Game crate.
Must NOT depend on `dtx-core` directly — game crates use `dtx-scoring`/`dtx-timing`
through Engine crates.
