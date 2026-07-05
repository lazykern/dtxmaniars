# Song Select — GITADORA NEXT-AGE Layout

**Date:** 2026-07-05
**Status:** Approved
**Screen:** `AppState::SongSelect` — `crates/game-menu/src/song_select.rs`
**Supersedes:** the (deleted, never-committed) 2026-07-05 arcaea staircase spec.

## Goal

Rearrange the song-select screen to match the GITADORA NEXT-AGE reference
screenshot: skill/BPM badges on the left edge, jacket top-center, density
graph bottom-left, a two-box difficulty ladder in the center, and
full-fidelity wheel rows (jacket thumbnail + skill number + progress bar +
title). All existing data and input logic is kept; only layout and row
content change. Palette stays `theme.rs` — no new colors.

## Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ DTXMANIARS                                    search…      [SORT]    │
│                                                                      │
│ ┌SKILL BY SONG┐   ┌────────────────┐        ┌row: thumb|skill+bar────┤
│ │       79.17 │   │     JACKET     │        │        |title          │
│ └─────────────┘   │   top-center   │        ├────────────────────────┤
│ ┌BPM──────────┐   │  ~380×285 4:3  │        │  ═ selected: yellow ═  │
│ │         157 │   │  artist below  │        │  ═ frame, juts left ═  │
│ └─────────────┘   └────────────────┘        ├────────────────────────┤
│ ┌density┐  ┌──────────┬─────────────┐       │row                     │
│ │graph  │  │Compl.Rate│ DRUM MASTER │       ├────────────────────────┤
│ │End↑   │  │  (rank%) │        7.60 │       │row                     │
│ │       │  ├──────────┼─────────────┤       │        ⋮               │
│ │Start↓ │  │ EXTREME → BASIC rows…  │       │                        │
│ │ notes │  └──────────┴─────────────┘       │                        │
│ └───────┘                                   │                        │
│                          hint bar                                    │
└──────────────────────────────────────────────────────────────────────┘
```

## Sections

### 1. Left cluster

- SKILL BY SONG and BPM badge panels move to the left edge, stacked
  vertically, upper area below the header. Same `stage_panel` badge
  widgets, same data feed (`skill_points`, song BPM).
- Jacket moves top-center and grows to ~380×285 (keep 4:3, existing
  `ALBUM_ART_W/H` constants change). Existing crossfade
  (`AlbumArt` tween) and `preimage_path` → `asset_server.load` path
  unchanged.
- Artist renders as a small text line directly under the jacket
  (it leaves the wheel rows — see §3).
- Density graph moves bottom-left: End label top, Start label bottom,
  TOTAL NOTES at the bottom. Same `density_graph` widget and data.

### 2. Difficulty ladder (two-box rows)

Center-bottom, right of the density graph. Each of the 5 slots
(MASTER top → BASIC bottom, existing spawn order) becomes two boxes
side by side:

- **Left box — completion rate:** caption "COMPLETION RATE" (small,
  secondary), value line = rank + achievement % (e.g. `D 64.80%`),
  `— no play` when unplayed, empty/dim when the chart is absent.
  Splits the existing `score_text` content into its own box.
- **Right box — level:** tier color bar across the top
  (`DRUM · MASTER` etc., `Theme::difficulty_color`), big level number
  below (`level_text`). This is the current slot panel minus the score
  line.

Selected slot: yellow border around **both** boxes (reuse
`set_panel_selected` styling; apply to the row container).
`difficulty_grid.rs` widget spawn changes; component markers
(`DifficultySlotPanel/Label/Level/Score`) stay so the existing update
systems keep working.

### 3. Wheel rows (full fidelity)

Row layout (constant `ROW_H`/`ROW_H_SELECTED` heights, spring + arc
motion from `row_geometry` unchanged):

- **Left:** square jacket thumbnail (row-height sized).
  `preimage_path` → `asset_server.load`, same as the big jacket.
  Fallback: tinted placeholder panel when `preimage_path` is `None`.
  Only visible rows exist (2×`VISIBLE_HALF`+1), so at most ~11 images
  load; Bevy's asset cache dedupes across respawns.
- **Right, top line:** skill number (yellow, `{:.2}`, from
  `skill_points(dlevel, best acc)` for the folder's display chart) +
  yellow progress bar whose fill = best achievement % (0–100). Bar is
  a simple two-node fill (border track + `select_yellow` fill width %).
  No score → `0.00` and empty bar.
- **Right, bottom line:** title (primary, ellipsis-clipped). Artist is
  dropped from rows (GITADORA rows have none); it moves under the
  jacket (§1).
- Selected row: existing yellow border treatment; row juts toward the
  selection per current arc geometry.

Per-row score reads happen in `spawn_wheel_rows`/`respawn_wheel_on_change`
(already re-runs on selection-list change); ~11 small `.ini` reads per
respawn is acceptable.

### 4. Unchanged

Header, type-to-search, sort chip, hint bar, BGM preview, input logic,
enter choreography, `SongSelectSelection`/`Selection` resources, theme
palette, all other screens.

## Files touched

- `crates/game-menu/src/song_select.rs` — spawn-tree rearrangement,
  wheel-row content, per-row skill/achievement lookup.
- `crates/dtx-ui/src/widget/difficulty_grid.rs` — two-box slot spawn.
- `crates/dtx-ui/src/widget/song_wheel.rs` — only if row constants need
  retuning for thumbnails; geometry formula unchanged.

## Error handling

- Missing `preimage_path`: tinted placeholder (existing behavior).
- Missing score ini: skill `0.00`, empty bar, `— no play` rate box.
- Absent chart slot: both boxes render dim/empty (existing `present`
  flag behavior).

## Testing

- Existing widget unit tests keep passing (`score_text`, `level_text`,
  `row_geometry`, album-art tween).
- New/updated: `score_text` split (rate-box text), progress-bar fill
  percent clamp, spawn-order test if grid markup changes.
- Manual verify via BRP screenshot against the reference.

## Out of scope (YAGNI)

- Category tabs (Ranking / By Artist / Favorites), stage counter, timer,
  rival-data popup, clear/unlock badges from the reference screenshot.
- Jacket-based ambient background changes.
- Per-row clear medals.
