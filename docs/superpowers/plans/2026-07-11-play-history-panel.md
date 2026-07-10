# Play History Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a "PLAY HISTORY" panel in the song-select left column listing up to 8 plays of the selected chart, ordered by score descending.

**Architecture:** A pure query helper on `ScoreStore` (dtx-scoring) supplies entries; `ScoreStoreResource` moves from `game-results` to `game-shell` so `game-menu` can read it without a menu→results dependency; a new `play_history` widget in `dtx-ui` owns the panel entities and a `PlayHistoryData` resource; `update_left_cluster` in `game-menu` fills the resource on selection change and a `render_play_history` system writes it into the widget (same data-resource → render pattern as `difficulty_grid`).

**Tech Stack:** Rust (edition 2021 workspace), Bevy 0.19 UI. No new dependencies — dates are formatted with an inline civil-from-days conversion (no chrono in the workspace).

**Build discipline (project memory):**
- Never run bare `cargo fmt --all` (local rustfmt version drift reformats unrelated files).
- Green unit tests do not prove the FixedUpdate schedule builds; this plan touches no schedules, only `Update` systems in existing state gates.
- Subagents must not wait on long cargo builds; the orchestrating session runs cargo commands.

---

### Task 1: `ScoreStore::history_for_path` query helper

**Files:**
- Create: `crates/dtx-scoring/tests/history.rs`
- Modify: `crates/dtx-scoring/src/store.rs` (add method to `impl ScoreStore`, after `best_for` around line 234)

- [ ] **Step 1: Write the failing tests**

Create `crates/dtx-scoring/tests/history.rs`:

```rust
//! `ScoreStore::history_for_path` ordering, filtering, and limits.

use std::path::{Path, PathBuf};

use dtx_scoring::identity::ChartIdentity;
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource, ScoreStore};

fn entry(path: &str, score: u32, played_at: u64) -> ScoreEntry {
    ScoreEntry {
        id: format!("test:{path}:{score}:{played_at}"),
        chart: ChartIdentity::new(
            format!("dtx1:{path}"),
            None,
            Some(PathBuf::from(path)),
        ),
        title: "Title".into(),
        artist: "Artist".into(),
        score,
        max_combo: 0,
        judgments: JudgmentTotals::default(),
        rank: Rank::A,
        played_at,
        source: ScoreSource::Native,
        replay_ref: None,
    }
}

fn store_with(entries: Vec<ScoreEntry>) -> ScoreStore {
    let mut store = ScoreStore::default();
    for e in entries {
        store.add(e);
    }
    store
}

#[test]
fn orders_by_score_descending() {
    let store = store_with(vec![
        entry("a.dtx", 100, 1),
        entry("a.dtx", 300, 2),
        entry("a.dtx", 200, 3),
    ]);
    let scores: Vec<u32> = store
        .history_for_path(Path::new("a.dtx"), 8)
        .iter()
        .map(|e| e.score)
        .collect();
    assert_eq!(scores, vec![300, 200, 100]);
}

#[test]
fn score_ties_break_most_recent_first() {
    let store = store_with(vec![
        entry("a.dtx", 200, 10),
        entry("a.dtx", 200, 30),
        entry("a.dtx", 200, 20),
    ]);
    let played: Vec<u64> = store
        .history_for_path(Path::new("a.dtx"), 8)
        .iter()
        .map(|e| e.played_at)
        .collect();
    assert_eq!(played, vec![30, 20, 10]);
}

#[test]
fn respects_limit() {
    let store = store_with((0..12).map(|i| entry("a.dtx", i, i as u64)).collect());
    assert_eq!(store.history_for_path(Path::new("a.dtx"), 8).len(), 8);
}

#[test]
fn filters_by_path_hint() {
    let mut store = store_with(vec![entry("a.dtx", 100, 1), entry("b.dtx", 200, 2)]);
    let mut no_hint = entry("a.dtx", 999, 3);
    no_hint.chart.source_path_hint = None;
    store.add(no_hint);
    let hits = store.history_for_path(Path::new("a.dtx"), 8);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].score, 100);
}

#[test]
fn empty_store_returns_empty() {
    let store = ScoreStore::default();
    assert!(store.history_for_path(Path::new("a.dtx"), 8).is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-scoring --test history`
Expected: compile error — `no method named history_for_path found for struct ScoreStore`

- [ ] **Step 3: Implement the method**

In `crates/dtx-scoring/src/store.rs`, change the imports line at the top:

```rust
use std::path::{Path, PathBuf};
```

Then add inside `impl ScoreStore`, directly after `best_for`:

```rust
    /// Plays whose `source_path_hint` matches `path`, best score first
    /// (ties: most recent first), truncated to `limit`.
    pub fn history_for_path(&self, path: &Path, limit: usize) -> Vec<&ScoreEntry> {
        let mut hits: Vec<&ScoreEntry> = self
            .entries
            .iter()
            .filter(|e| e.chart.source_path_hint.as_deref() == Some(path))
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(b.played_at.cmp(&a.played_at))
        });
        hits.truncate(limit);
        hits
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-scoring --test history`
Expected: 5 passed

- [ ] **Step 5: Run the rest of the crate's tests**

Run: `cargo test -p dtx-scoring`
Expected: all pass (store_v2, nx_import, edge_cases unaffected)

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-scoring/src/store.rs crates/dtx-scoring/tests/history.rs
git commit -m "feat(scoring): history_for_path query on ScoreStore"
```

---

### Task 2: Move `ScoreStoreResource` from game-results to game-shell

`game-menu` must read the store but must not depend on `game-results`. Both crates already depend on `game-shell`.

**Files:**
- Create: `crates/game-shell/src/score_store.rs`
- Modify: `crates/game-shell/Cargo.toml` (add dtx-scoring dependency)
- Modify: `crates/game-shell/src/lib.rs` (register module + re-export)
- Modify: `crates/game-results/src/lib.rs:28-30` (delete local definition, import instead)
- Modify: `app/dtxmaniars-desktop/src/main.rs:15` (import from game_shell)

No new test — this is a pure move; existing tests plus `cargo check` cover it.

- [ ] **Step 1: Add dtx-scoring dependency to game-shell**

In `crates/game-shell/Cargo.toml` `[dependencies]`, add (path style matches other crates):

```toml
dtx-scoring = { path = "../dtx-scoring" }
```

- [ ] **Step 2: Create the module**

Create `crates/game-shell/src/score_store.rs`:

```rust
//! Shared Bevy wrapper around `dtx_scoring::ScoreStore`.
//!
//! Lives in game-shell so both game-results (writes after a play)
//! and game-menu (reads for song-select display) can use it without
//! depending on each other. Initialized and loaded at startup by the
//! desktop app.

use bevy::prelude::*;
use dtx_scoring::ScoreStore;

/// Bevy wrapper around `dtx_scoring::ScoreStore`.
#[derive(Resource, Deref, DerefMut, Default, Debug, Clone)]
pub struct ScoreStoreResource(pub ScoreStore);
```

- [ ] **Step 3: Register in game-shell lib**

In `crates/game-shell/src/lib.rs`, next to the existing `pub mod nav;` / `pub mod states;` lines add:

```rust
pub mod score_store;
```

and next to the existing `pub use` re-exports add:

```rust
pub use score_store::ScoreStoreResource;
```

- [ ] **Step 4: Point game-results at the shared resource**

In `crates/game-results/src/lib.rs`:

Delete lines 28–30:

```rust
/// Bevy wrapper around `dtx_scoring::ScoreStore`.
#[derive(Resource, Deref, DerefMut, Default, Debug, Clone)]
pub struct ScoreStoreResource(pub ScoreStore);
```

Change the game_shell import (line 7) to include it:

```rust
use game_shell::{despawn_stage, request_transition, AppState, ScoreStoreResource, TransitionRequest};
```

No re-export from game-results — main.rs is updated instead (Step 5). The move may leave `ScoreStore` unused in game-results' `use dtx_scoring::...` list (line 5); if the compiler warns, trim exactly what it names.

- [ ] **Step 5: Update the desktop app import**

In `app/dtxmaniars-desktop/src/main.rs` line 15:

```rust
use game_results::GameResultsPlugin;
```

and add to the existing `use game_shell::...` import (or add a new line if none):

```rust
use game_shell::ScoreStoreResource;
```

- [ ] **Step 6: Check compilation and tests**

Run: `cargo check -p game-shell -p game-results -p game-menu -p dtxmaniars-desktop`
Expected: clean (fix any unused-import warnings the move created)

Run: `cargo test -p game-shell -p game-results`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add crates/game-shell crates/game-results app/dtxmaniars-desktop/src/main.rs
git commit -m "refactor(shell): move ScoreStoreResource to game-shell"
```

---

### Task 3: `play_history` widget in dtx-ui

Follows the `difficulty_grid` pattern: widget module owns marker components, a data resource, and a spawn function; the screen system fills the resource; a render system (Task 4) writes it into entities. Fixed 8 pre-spawned row Text entities — no despawn/respawn on refresh.

**Files:**
- Create: `crates/dtx-ui/src/widget/play_history.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs` (add `pub mod play_history;` in alphabetical order)
- Modify: `crates/dtx-ui/src/lib.rs` (init `PlayHistoryData` resource next to the existing `.init_resource::<widget::difficulty_grid::DifficultyGridData>()` around line 98)

- [ ] **Step 1: Write the widget with failing tests**

Create `crates/dtx-ui/src/widget/play_history.rs`:

```rust
//! Song-select play-history panel: header + fixed row slots.
//!
//! Screen systems fill [`PlayHistoryData`] on selection change and a
//! render system writes rows into the `HistoryRowText` entities
//! (same data-resource pattern as `difficulty_grid`).

use bevy::prelude::*;

use crate::theme::Theme;

/// Maximum rows shown in the panel.
pub const HISTORY_MAX_ROWS: usize = 8;

/// One display row: a single past play of the selected chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryRow {
    /// Rank label ("SS".."E", "--" for unknown).
    pub rank: String,
    /// Score value.
    pub score: u32,
    /// Perfect percentage (0..100).
    pub perfect_pct: f32,
    /// Pre-formatted UTC date, `YYYY-MM-DD`.
    pub date: String,
}

/// Rows for the selected chart, best score first. Filled by the
/// song-select screen, rendered by `render_play_history`.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PlayHistoryData {
    /// At most [`HISTORY_MAX_ROWS`] rows.
    pub rows: Vec<HistoryRow>,
}

/// Row line text entity (slot index `0..HISTORY_MAX_ROWS`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HistoryRowText(pub usize);

/// "NO PLAYS" empty-state label.
#[derive(Component, Debug, Clone, Copy)]
pub struct HistoryEmptyText;

/// Spawn the panel contents: header, empty-state label, and
/// `HISTORY_MAX_ROWS` blank row slots.
pub fn spawn_play_history(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("PLAY HISTORY"),
                Theme::font(12.0),
                TextColor(theme.clear_green),
            ));
            col.spawn((
                HistoryEmptyText,
                Text::new("NO PLAYS"),
                Theme::font(12.0),
                TextColor(theme.text_secondary),
            ));
            for i in 0..HISTORY_MAX_ROWS {
                col.spawn((
                    HistoryRowText(i),
                    Text::new(""),
                    Theme::font(12.0),
                    TextColor(theme.text_primary),
                ));
            }
        });
}

/// Render one row as a single line: `S   982340   95.2%  2026-07-10`.
pub fn history_row_line(row: &HistoryRow) -> String {
    format!(
        "{:<2} {:>7}  {:>5.1}%  {}",
        row.rank, row.score, row.perfect_pct, row.date
    )
}

/// Format unix seconds as a UTC `YYYY-MM-DD` date string.
///
/// Uses the days-to-civil algorithm (Howard Hinnant) — the workspace
/// has no date dependency and this panel only needs a day stamp.
pub fn format_unix_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_epoch_start() {
        assert_eq!(format_unix_date(0), "1970-01-01");
    }

    #[test]
    fn date_day_boundary() {
        assert_eq!(format_unix_date(86_399), "1970-01-01");
        assert_eq!(format_unix_date(86_400), "1970-01-02");
    }

    #[test]
    fn date_modern() {
        // 2026-07-11 00:00:00 UTC
        assert_eq!(format_unix_date(1_783_728_000), "2026-07-11");
    }

    #[test]
    fn date_leap_day() {
        // 2024-02-29 00:00:00 UTC
        assert_eq!(format_unix_date(1_709_164_800), "2024-02-29");
    }

    #[test]
    fn row_line_layout() {
        let row = HistoryRow {
            rank: "S".into(),
            score: 982_340,
            perfect_pct: 95.234,
            date: "2026-07-10".into(),
        };
        assert_eq!(history_row_line(&row), "S   982340   95.2%  2026-07-10");
    }

    #[test]
    fn spawns_header_rows_and_empty_label() {
        let mut app = bevy::app::App::new();
        let theme = Theme::default();
        let world = app.world_mut();
        {
            let mut commands = world.commands();
            commands.spawn(Node::default()).with_children(|p| {
                spawn_play_history(p, &theme);
            });
        }
        world.flush();

        let row_count = world.query::<&HistoryRowText>().iter(world).count();
        assert_eq!(row_count, HISTORY_MAX_ROWS);

        let empty_count = world.query::<&HistoryEmptyText>().iter(world).count();
        assert_eq!(empty_count, 1);

        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|t| t.0.clone())
            .collect();
        assert!(texts.iter().any(|t| t == "PLAY HISTORY"));
        assert!(texts.iter().any(|t| t == "NO PLAYS"));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/dtx-ui/src/widget/mod.rs`, add in alphabetical order (between `phrase_meter` and `playfield_speed`):

```rust
pub mod play_history;
```

In `crates/dtx-ui/src/lib.rs`, next to the existing line `.init_resource::<widget::difficulty_grid::DifficultyGridData>()` add:

```rust
        .init_resource::<widget::play_history::PlayHistoryData>()
```

- [ ] **Step 3: Run the widget tests**

Run: `cargo test -p dtx-ui play_history`
Expected: 6 passed

Note on `date_modern` / `date_leap_day` expectations: if either fails, the constant is wrong, not the algorithm — verify with `date -u -d @1783728000 +%F` before touching the code.

- [ ] **Step 4: Run all dtx-ui tests**

Run: `cargo test -p dtx-ui`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui/src/widget/play_history.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs
git commit -m "feat(ui): play-history panel widget"
```

---

### Task 4: Wire the panel into song select

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`
  - imports (~line 37–49)
  - left-column spawn (`spawn_song_select`, after the BPM panel, ~line 637)
  - `update_left_cluster` (~line 1007): fill `PlayHistoryData`
  - new system `render_play_history` (place directly after `render_difficulty_grid`, ~line 1160)
  - plugin registration (~line 444): add `render_play_history` after `render_difficulty_grid`

No new unit test in this task: sorting/limits are covered by Task 1, formatting/spawning by Task 3, and the fill logic is a straight map over `history_for_path` output. Verification is compile + existing tests + manual BRP screenshot (Task 5).

- [ ] **Step 1: Add imports**

In `crates/game-menu/src/song_select.rs`, next to the existing `use dtx_ui::widget::...` imports add:

```rust
use dtx_ui::widget::play_history::{
    format_unix_date, history_row_line, spawn_play_history, HistoryEmptyText, HistoryRow,
    HistoryRowText, PlayHistoryData, HISTORY_MAX_ROWS,
};
```

And extend the existing `use game_shell::...` import with `ScoreStoreResource` (it already imports `AppState` etc.).

Also add `use dtx_scoring::Rank;` if not already imported (check the file's existing `dtx_scoring` uses first — it currently calls `dtx_scoring::score_ini::...` with full paths; matching that style, use the full path `dtx_scoring::Rank` inline instead of importing).

- [ ] **Step 2: Spawn the panel in the left column**

In `spawn_song_select`, inside the far-left column `with_children` closure, directly after the BPM panel block (`spawn_badge_row(p, &t, "BPM", "---", false);` closes at line ~637), add a third panel:

```rust
                        left.spawn(panel(
                            &t,
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                        ))
                        .with_children(|p| {
                            spawn_play_history(p, &t);
                        });
```

- [ ] **Step 3: Fill `PlayHistoryData` in `update_left_cluster`**

Add two parameters to `update_left_cluster`:

```rust
    store: Res<ScoreStoreResource>,
    mut history: ResMut<PlayHistoryData>,
```

At the end of the function body (after the artist-text loop), add:

```rust
    // play history for the selected chart, best score first
    let rows: Vec<HistoryRow> = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|song| {
            store
                .history_for_path(&song.path, HISTORY_MAX_ROWS)
                .into_iter()
                .map(|e| HistoryRow {
                    rank: match e.rank {
                        dtx_scoring::Rank::Unknown => "--".into(),
                        ref r => r.to_string(),
                    },
                    score: e.score,
                    perfect_pct: e.perfect_pct(),
                    date: format_unix_date(e.played_at),
                })
                .collect()
        })
        .unwrap_or_default();
    if history.rows != rows {
        history.rows = rows;
    }
```

The `!=` guard keeps `PlayHistoryData` change detection quiet when the selection moves between charts with identical (usually empty) histories.

- [ ] **Step 4: Add the render system**

Directly after `render_difficulty_grid` (ends ~line 1160), add:

```rust
/// Write play-history rows into the panel's text entities. Top row
/// (best score) gets the selection yellow; the empty-state label
/// shows only when there are no rows.
fn render_play_history(
    data: Res<PlayHistoryData>,
    theme: Res<ThemeResource>,
    mut rows: Query<(&HistoryRowText, &mut Text, &mut TextColor)>,
    mut empty: Query<&mut Node, With<HistoryEmptyText>>,
) {
    if !data.is_changed() {
        return;
    }
    let t = theme.0;
    for (row, mut text, mut color) in &mut rows {
        match data.rows.get(row.0) {
            Some(r) => {
                *text = Text::new(history_row_line(r));
                color.0 = if row.0 == 0 {
                    t.select_yellow
                } else {
                    t.text_primary
                };
            }
            None => *text = Text::new(""),
        }
    }
    for mut node in &mut empty {
        node.display = if data.rows.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
    }
}
```

If the compiler reports query conflicts between `rows` and other `Text` queries in the same system set, this system has its own function scope so conflicts can only be internal; `HistoryRowText` and `HistoryEmptyText` sit on different entities, and the queries touch disjoint components (`Text`/`TextColor` vs `Node`), so no `Without` filters are needed.

- [ ] **Step 5: Register the system**

In `plugin()` (line ~444), in the `Update` tuple after `render_difficulty_grid,` add:

```rust
                render_play_history,
```

- [ ] **Step 6: Check compilation and tests**

Run: `cargo check -p game-menu`
Expected: clean

Run: `cargo test -p game-menu`
Expected: all pass (includes the schedule ordering guard)

- [ ] **Step 7: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(menu): play-history panel on song select"
```

---

### Task 5: Full verification

- [ ] **Step 1: Workspace check + tests**

Run: `cargo check --workspace`
Expected: clean

Run: `cargo test --workspace`
Expected: all pass

- [ ] **Step 2: Manual verification via BRP**

Per the established BRP loop (project memory `brp-drive-customize`): launch the `dtxmaniars` binary with the brp feature from the repo, navigate to song select, take a screenshot with `mcp__bevy-brp__brp_extras_screenshot`, and confirm:

1. "PLAY HISTORY" panel renders below the BPM badge.
2. A chart with recorded plays (e.g. the one in `scores.json`) lists rows, best score first, top row yellow.
3. A never-played chart shows "NO PLAYS".
4. Moving the wheel / changing difficulty refreshes the panel.

- [ ] **Step 3: Commit any fixes uncovered by verification**

Each fix is its own small commit with a `fix(menu):` / `fix(ui):` prefix.
