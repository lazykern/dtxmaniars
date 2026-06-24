# 0013: Result screen minimal text-only display (M5)

Status: accepted (temporary)
Date: 2026-06-23

## Context

DTXManiaNX `CStageResult.cs` (811 lines) has many parallel panels:
- `CActResultParameterPanel` — score / combo / per-judgment %
- `CActResultImage` — background image
- `CActResultInformation` — song info
- `CActResultRank` — rank icon
- `CActResultDan` — dan course
- `CActResultGhost` — ghost playback
- `CActResultSongBar` — song name banner
- `CActResultExcellent` — full combo celebration

For M5 we ship the result screen with ONE text panel showing all key data.
No images, no ghost playback, no Dan course (we don't even have Dan courses
in M5).

## Decision

M5 ships `game-results` crate with a single text node displaying:
- Song title + artist + BPM + Drums level
- Score (cumulative)
- Max combo
- Rank (computed from perfect percentage, thresholds S≥95, A≥85, B≥70, C≥50, D≥25, else E)
- Perfect/Great/Good/Ok/Miss counts + total + perfect %
- ESC/ENTER → SongSelect

## Consequences

- Acceptable for M5: architecture proven (read gameplay-drums resources,
  compute summary, display). Real users will want images + ghost playback.
- `crates/game-results/src/lib.rs` has `Rank` enum + thresholds as unit
  tests. Verified verbatim from DTXManiaNX ConfigIni defaults.
- M5.1 (after M6) restores visual panels (rank icon, song bar, ghost).
- M5 doesn't persist results to CScoreIni (port deferred to M6+).

## When to revisit

- M5.1: visual panels (CActResultImage, CActResultRank).
- M6: CScoreIni persistence (`crates/dtx-scoring` save + load).
- M6+: ghost playback (replay input trace against chart).