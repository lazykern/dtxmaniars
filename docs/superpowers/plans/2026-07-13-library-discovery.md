# Library Discovery and Measured Scanning — Implementation Plan

> **For Codex:** Execute with test-driven development: write each listed test,
> observe it fail, then implement the smallest passing change.

**Goal:** Make Song Select practical for larger libraries with durable favorites,
combinable discovery filters, deterministic random selection, and transparent
scan/stat timing—without a cache or database.

## Task 1: Persist favorites in the library layer

**Files:** `crates/dtx-library/Cargo.toml`, `crates/dtx-library/src/lib.rs`,
`crates/dtx-library/src/preferences.rs`

1. Add failing tests for versioned save/load, a malformed file recovering to
   empty preferences, and path-keyed favorite toggling.
2. Add the minimal serde dependencies and `LibraryPreferences` resource with a
   versioned JSON file under `user_data_dir`, safe load/save, and a normalized
   chart-path key.
3. Initialize it in `SongDbPlugin`; log non-fatal persistence failures.
4. Run `cargo test -p dtx-library`.

## Task 2: Report scan and chart-stat work

**Files:** `crates/dtx-library/src/lib.rs`, `crates/game-menu/src/chart_stats.rs`,
`crates/game-menu/src/song_select.rs`

1. Add failing tests proving directory and parsed/skipped counts are exposed by
   `ScanReport`, and that stat measurements carry the selected path and elapsed
   duration.
2. Count visited directories during scans and log startup/rescan elapsed,
   parsed, skipped, and directory totals.
3. Publish the completion duration of the existing asynchronous chart-stat
   parse through a small resource; display it alongside scan diagnostics.
4. Run library and menu package tests.

## Task 3: Implement pure discovery eligibility and selection

**Files:** `crates/game-menu/src/discovery.rs`, `crates/game-menu/src/lib.rs`,
`crates/game-menu/src/song_select.rs`

1. Add failing unit tests for favorite/unplayed/recent/near-level intersections,
   empty history, deterministic random candidate bounds, and active-filter copy.
2. Implement `DiscoveryFilters`: favorites and unplayed booleans; recent and
   near-level toggles. Derive recent from most-recent ScoreStore entries and
   near level from the median played chart level (±1.0 display level).
3. Use normalized chart paths for score-history matching. A chart is unplayed
   when no score entry references it; imported history counts as played.
4. Filter before SongSelect folder grouping, preserving the selected chart by
   path whenever it remains visible. Random picks only within filtered results.
5. Run `cargo test -p game-menu`.

## Task 4: Wire Song Select controls and feedback

**Files:** `crates/game-menu/src/song_select.rs`

1. Add failing focused tests for selection preservation and descriptive empty
   state/reset copy.
2. Render a compact filter/status row and empty-result message. Add keyboard
   commands: F7 favorite toggle; Ctrl+1..4 toggle favorite/unplayed/recent/
   near-level filters; Ctrl+R selects a deterministic random visible result;
   Ctrl+0 resets filters. Existing search, sorting, selected difficulty, and
   F5 rescan continue to compose with them.
3. Save a favorite change immediately; warn instead of blocking if saving
   fails.
4. Run `cargo test -p game-menu`.

## Task 5: Verify, document, and integrate

**Files:** `docs/notes/2026-07-13-game-improvement-program.md`

1. Update the program ledger with Cycle 5 scope, test evidence, and the
   explicit no-cache decision.
2. Run `cargo fmt --all -- --check`, workspace `cargo check`, Clippy with
   warnings denied, and workspace library tests using the shared target.
3. Commit all Cycle 5 work, inspect both worktrees, then fast-forward merge
   `codex/game-improvement-cycles-0-2` into `main`.
