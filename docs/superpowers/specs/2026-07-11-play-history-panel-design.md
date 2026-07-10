# Play History Panel on Song Select — Design

Date: 2026-07-11
Status: Approved (Approach A)

## Goal

Fill the empty left column of the song-select screen with a play-history
panel for the currently selected chart (song + difficulty), ordered by
score descending.

## Background

- Every completed play is already persisted as a `ScoreEntry` in
  `ScoreStore` (`crates/dtx-scoring/src/store.rs`), serialized to
  `scores.json`. Each entry carries score, rank, judgment totals, max
  combo, and `played_at` (unix seconds).
- The results screen appends entries via `ScoreStoreResource`
  (`crates/game-results/src/lib.rs`), a Bevy resource initialized and
  loaded at startup in `app/dtxmaniars-desktop/src/main.rs`.
- Song select identifies charts only by file path (`SongInfo.path`);
  `ScoreEntry.chart.source_path_hint` records the chart path for native
  plays.
- The left column of song select currently shows only the SKILL BY SONG
  and BPM badges (`spawn_badge_row`, `crates/game-menu/src/song_select.rs`),
  leaving dead space below.

No new persistence and no schema change are required.

## UI

A "PLAY HISTORY" panel below the existing left-column badges:

```
+------------------+
| SKILL BY SONG    |
| 103.50           |
+------------------+
| BPM       157    |
+------------------+
| PLAY HISTORY     |   <- header
+------------------+
| S  982340  95.2% |  2026-07-10
| A  941200  91.8% |  2026-07-08
| B  851000  84.3% |  2026-06-30
| ...              |
+------------------+
```

- Up to 8 rows, sorted by score descending. Ties break by most recent
  `played_at` first.
- Row contents: rank letter (colored with the existing rank color
  scheme), score, perfect percentage (`ScoreEntry::perfect_pct`), and
  local date formatted `YYYY-MM-DD`.
- The top (best) row gets a highlight treatment consistent with the
  theme's existing emphasis style.
- Empty state: a single dim "NO PLAYS" label.
- Panel styling follows the existing badge/panel look in
  `song_select.rs` / `dtx-ui` theme.

## Behavior

- The panel refreshes whenever the selected song or difficulty changes,
  using the same trigger path that refreshes the SKILL BY SONG badge
  today.
- Data source: filter `ScoreStore.entries` where
  `chart.source_path_hint` equals the selected chart's path.
- Imported NX entries without a path hint will not appear. Accepted for
  v1; a hash-based fallback can be added later if it matters.

## Architecture

1. **Move `ScoreStoreResource` from `game-results` to `game-shell`.**
   `game-menu` must not depend on `game-results`. `game-shell` gains a
   `dtx-scoring` dependency (it already depends on Bevy, and both
   `game-menu` and `game-results` already depend on `game-shell`).
   `game-results` imports the resource from `game-shell`;
   `app/dtxmaniars-desktop/src/main.rs` updates its import.
2. **Query helper in `dtx-scoring`:** a pure function on `ScoreStore`
   (e.g. `history_for_path(path, limit) -> Vec<&ScoreEntry>`) returning
   entries matching the path hint, sorted score-desc with recency
   tie-break. Unit-testable without Bevy.
3. **Panel widget + refresh system in `game-menu`:** spawn the panel in
   the song-select left column; a system rebuilds its rows on selection
   change, reading `Res<ScoreStoreResource>`. If `song_select.rs` growth
   is a concern, the row widget can live alongside the other widgets in
   `dtx-ui` (`difficulty_grid.rs` is the precedent).

## Error handling

- Missing/empty store: empty state renders; no panics.
- Charts never played: empty state.
- Timestamp formatting must not panic on out-of-range values; fall back
  to "--" on conversion failure.

## Testing

- Unit tests for the `ScoreStore` query helper: ordering, tie-break,
  limit, path mismatch, empty store.
- Existing menu tests must stay green; the FixedUpdate ordering guard
  test covers schedule integrity.
- Manual verification via BRP screenshot loop (launch, navigate song
  select, screenshot) per the established workflow.

## Out of scope (YAGNI)

- Chronological sort toggle.
- Play/clear count stats in the panel.
- Global (cross-song) recent-plays view.
- Hash-based matching for imported NX scores.
