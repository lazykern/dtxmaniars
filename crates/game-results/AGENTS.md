# crates/game-results

Game-layer crate. CStageResult port — displays score, combo, per-judgment counts
after a song. Reads from gameplay-drums resources.

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/CStageResult.cs` (811 lines)
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/CActResultParameterPanel.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/ResultRankIcon.cs`
- `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs` (CScoreEntry)

## API

- `ResultScreen` plugin: OnEnter(AppState::Result) reads gameplay resources,
  spawns UI, ESC → SongSelect.
- Reads:
  - `gameplay_drums::resources::Score`
  - `gameplay_drums::resources::Combo`
  - `gameplay_drums::resources::JudgmentCounts`
  - `gameplay_drums::resources::ActiveChart` (for song title)

## M5 scope

- Display: title, difficulty, score, max combo, per-judgment counts + percentages
- Rank: computed from perfect percentage (S ≥ 95%, A ≥ 85%, B ≥ 70%, C ≥ 50%, else D)
- Save result to local JSON file (ScoreSave::save)
- ESC → SongSelect (was Title in M3 stub)

## Layer

Game. Depends on Game-layer `gameplay-drums` + `game-shell`.