# crates/game-shell

Game-layer crate. Owns the AppState machine and the DTXManiaNX fade transition
(per ADR-0010 port-first rule, matched verbatim from `references/DTXmaniaNX-BocuD/`).

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs` — full file (699 lines).
  - Line 29: `private float FadeDurationMs = 1500f;` ← single source for fade duration
  - Lines 645-665: `BeginFadeTransition` — capture snapshot, swap stage, latch fade
  - Lines 670-699: `DrawFadeOverlay` — linear alpha decay, snapshot-on-top
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs` — `EStage` enum (8 stages)
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines)
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
AppState::ChangeSkin     ← CStageChangeSkin
AppState::End            ← CStageEnd
```

## Fade transition (verbatim from StageManager.cs:29)

- **Duration**: 1500 ms (constant)
- **Curve**: linear (NOT OutQuint or any easing) — `_fadeAlpha = clamp(1 - elapsed/1500, 0, 1)`
- **Direction**: fade-out only (the new stage's first frame is what we're
  revealing; we fade the OLD visual to transparent)
- **Spike handling**: fade starts AFTER new stage's first frame completes,
  so the new stage's initialization spike doesn't show through. In bevy
  this is naturally handled by the black overlay being at full alpha on
  the first frame.
- **Snapshot approximation**: M3 uses a fullscreen black overlay instead
  of capturing the actual framebuffer. From the user's perspective the
  result is identical (cover spike, fade to reveal). True framebuffer
  snapshot is M3.1 (ADR-0011).

## Rules

- One `Plugin` per stage file, aggregated in `lib::plugin`.
- Stages register their `OnEnter` / `OnExit` systems via the per-stage plugin.
- State transitions go through `NextState<AppState>` — never mutate
  `Res<State<AppState>>` directly.
- Per-stage UI entities tagged with a marker component (e.g.
  `TitleEntity`, `LoadingEntity`) so `OnExit` can despawn them via a
  generic `despawn_stage::<T>` system.

## Layer

Game. May depend on `dtx-ui` (Engine) and any other Engine/Game crate.
Must NOT depend on `dtx-core` directly — game crates use `dtx-scoring`/`dtx-timing`
through Engine crates.