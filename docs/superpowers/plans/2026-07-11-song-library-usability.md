# Song Library Usability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep type-to-search; add favorites, played/unplayed, clear state, difficulty bands, recent additions, near-my-level, and random-within-filter; instrument scan duration/chart count/parse failures BEFORE any caching; preserve the remembered song and difficulty.

**Architecture:** All filtering funnels through the single existing choke point `SongSelectSelection::recompute` (`song_select.rs:156`), extended with a `LibraryFilter` and a precomputed per-song `LibraryStatus`. Played/cleared derive from the in-memory `ScoreStore` (we add a `cleared` flag to `ScoreEntry` — serde-defaulted, no store version bump). Favorites persist as a new `library` section in the existing config.toml. Recent additions use file mtime captured during scan. Scan gets a `ScanReport` (measure first — caching is a roadmap non-goal until numbers justify it).

**Tech Stack:** existing crates only. Randomness via the repo's wall-clock-nanos precedent (`title.rs:pick_editor_song`, :156-177) — no `rand` dependency.

**Source basis (verified 2026-07-11):**
- `crates/dtx-library/src/lib.rs`: `SongInfo` (:32-58, has NO mtime), `scan_directory` (:145-149) → `walk_dtx` (:151-176, synchronous, failures logged-and-swallowed at :162-172, never counted), `SongDb` resource (:209-215), `SortMode` (:178-187), `startup_scan_system` (:317-332). Reparse-on-demand: `notes_total` (:113-122) and `chart_stats.rs:76-106` (async per-selection) — instrument, do not cache.
- Song select: `crates/game-menu/src/song_select.rs` (2322 lines). `SongSelectSelection::recompute(&mut self, all: &[SongInfo])` (:156-202) with `matches_search` (:145-151); callers `recompute_visible` (:1789, OnEnter) and `maybe_recompute_visible` (:1801, on `dirty`/db change), both already `clamp_to_visible` after (:1795, :1809). Type-to-search: `search_input` (:1606-1646), `apply_search_char` (:399-404). Sort: TAB cycles in `song_select_hotkeys` (:1475-1477). Remembered selection: `persist_hovered_selection` (:562-587) / `restore_last_selection` (:532-560) via `dtx-config` `last_selected`/`last_selected_difficulty` (`dtx-config/src/lib.rs:222-228`).
- Scores: `ScoreStore` (`dtx-scoring/src/store.rs:16-30`, v2), `ScoreEntry` (:44-69 — no cleared/modifier field), `history_for_path(path, limit)` (:239-252, matches `chart.source_path_hint`), `best_for_chart` (:225-230). Bevy wrapper `ScoreStoreResource` (`game-shell/src/score_store.rs:13`). Write site: `game-results/src/lib.rs:295-343`; `LastStageOutcome { cleared }` set in `stage_end.rs` (:30-33, :93/:103).
- Level display: `SongInfo.dlevel: Option<u32>`, `dtx_core::display_dlevel` (used song_select.rs:206,1141). Per-chart skill `chart_stats::skill_points` (:26-29). No aggregate player level exists.
- Favorites: no per-song user store exists; config is the established persistence (loaded+saved already on hover change).
- Random precedent: `title.rs:172-176` `SystemTime nanos % len`.
- Test conventions: pure free functions unit-tested in-file (`make_song` helper at song_select.rs:1842-1857); dtx-library tests in lib.rs (:344-568).

**Non-goals (roadmap):** cache/DB/async scan machinery (measure first), collections/playlists UI beyond favorites, fuzzy search changes.

---

### Task 1: Scan instrumentation (before any caching — roadmap gate)

**Files:**
- Modify: `crates/dtx-library/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

In the `#[cfg(test)]` mod of `lib.rs`:

```rust
#[test]
fn scan_report_counts_parses_and_failures() {
    // fixture dir contains valid .dtx files only -> failures empty
    let (songs, report) = scan_directory_with_report(&fixture_dir()).unwrap();
    assert_eq!(report.parsed, songs.len());
    assert!(report.failures.is_empty());
    assert!(report.duration_ms < 60_000);
}

#[test]
fn scan_report_records_failure_paths_and_reasons() {
    let dir = std::env::temp_dir().join("dtxlib_scan_fail_test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.dtx"), b"\xff\xfe garbage \x00").unwrap();
    let (_songs, report) = scan_directory_with_report(&dir).unwrap();
    // parse either fails (counted) or tolerantly succeeds — assert the invariant:
    assert_eq!(report.parsed + report.failures.len(), 1);
    for (path, reason) in &report.failures {
        assert!(path.ends_with("broken.dtx"));
        assert!(!reason.is_empty());
    }
    std::fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-library -j 2 scan_report`
Expected: FAIL — `scan_directory_with_report`/`ScanReport` not found.

- [ ] **Step 3: Implement**

```rust
/// Scan telemetry. Roadmap rule: measure before caching — any future cache
/// must be justified by these numbers on a representative library.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub duration_ms: u64,
    pub parsed: usize,
    pub failures: Vec<(PathBuf, String)>,
}

pub fn scan_directory_with_report(root: &Path) -> Result<(Vec<SongInfo>, ScanReport), ScanError> {
    let start = std::time::Instant::now();
    let mut songs = Vec::new();
    let mut report = ScanReport::default();
    walk_dtx(root, &mut songs, &mut report)?;
    report.parsed = songs.len();
    report.duration_ms = start.elapsed().as_millis() as u64;
    Ok((songs, report))
}
```

Change `walk_dtx` (:151-176) to take `&mut ScanReport` and, in the two warn branches (:162-172), additionally push `(path.clone(), err.to_string())` into `report.failures`. Keep `scan_directory` as a thin wrapper (`Ok(scan_directory_with_report(root)?.0)`) so existing callers/tests compile.

Add `pub last_report: ScanReport` to `SongDb` (:209-215); set it in `rescan`/`refresh`; in `startup_scan_system` (:317-332) log the summary:

```rust
bevy::log::info!(
    "library scan: {} charts in {} ms, {} failures",
    db.last_report.parsed, db.last_report.duration_ms, db.last_report.failures.len()
);
for (path, reason) in &db.last_report.failures {
    bevy::log::warn!("skipped chart {}: {}", path.display(), reason);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-library -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library
git commit -m "feat(library): scan report with duration, chart count, and parse failures"
```

---

### Task 2: Capture file mtime during scan (recent additions)

**Files:**
- Modify: `crates/dtx-library/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn scan_captures_mtime() {
    let (songs, _) = scan_directory_with_report(&fixture_dir()).unwrap();
    assert!(!songs.is_empty());
    for s in &songs {
        let m = s.mtime_unix_secs.expect("fixtures have mtimes");
        assert!(m > 1_000_000_000, "plausible unix secs, got {m}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dtx-library -j 2 captures_mtime`
Expected: FAIL — field missing.

- [ ] **Step 3: Implement**

Add to `SongInfo` (:32-58): `pub mtime_unix_secs: Option<u64>,`. In `walk_dtx`, after a successful parse:

```rust
let mtime_unix_secs = entry
    .metadata()
    .ok()
    .and_then(|m| m.modified().ok())
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs());
```

and set it on the pushed `SongInfo` (default `None` in `from_chart`; set after construction or add a builder param — match existing style). Fix every `SongInfo` literal in tests (`make_song` in song_select.rs:1842 included) — compiler drives.

- [ ] **Step 4: Run tests across affected crates**

Run: `cargo test -p dtx-library -j 2 && cargo check -p game-menu -j 2`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library crates/game-menu
git commit -m "feat(library): capture chart file mtime for recent-additions filtering"
```

---

### Task 3: `cleared` flag on ScoreEntry (clear-state without per-file I/O)

Clear state currently lives only in per-chart `.score.ini` files — filtering the whole list would mean hundreds of file reads per recompute. Persist it in the JSON store instead; serde default keeps old stores loading (no version bump: absent field = false).

**Files:**
- Modify: `crates/dtx-scoring/src/store.rs` (`ScoreEntry`, :44-69)
- Modify: `crates/game-results/src/lib.rs` (`native_score_entry` :79-108 and its caller)

- [ ] **Step 1: Write the failing tests**

In `crates/dtx-scoring/tests/store_v2.rs`:

```rust
#[test]
fn cleared_defaults_false_on_legacy_entries() {
    // an existing v2 entry JSON without the field must load
    let json = r#"{"version":2,"entries":[{
        "id":"x","chart":{"canonical_hash":"dtx1:aa","raw_sha256":"bb",
        "raw_sha256_aliases":[],"source_path_hint":null},
        "title":"t","artist":"a","score":100,"max_combo":5,
        "judgments":{"perfect":1,"great":0,"good":0,"poor":0,"miss":0},
        "rank":"A","played_at":1783728000,"source":"native","replay_ref":null
    }],"nx_imports":[]}"#;
    let store: ScoreStore = serde_json::from_str(json).unwrap();
    assert!(!store.entries[0].cleared);
}

#[test]
fn cleared_round_trips() {
    let mut e = /* build a ScoreEntry the way existing tests in this file do */;
    e.cleared = true;
    let s = serde_json::to_string(&e).unwrap();
    let back: ScoreEntry = serde_json::from_str(&s).unwrap();
    assert!(back.cleared);
}
```

(Adapt the field spellings in the raw JSON to the actual serde output — copy a serialized entry from an existing round-trip test and delete the `cleared` key. If `Rank`/`ScoreSource` serialize differently, fix the literal, not the code.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-scoring -j 2 cleared_`
Expected: FAIL — field missing.

- [ ] **Step 3: Implement**

In `ScoreEntry` (:44-69) add:

```rust
/// True when the run reached StageClear (gauge never failed out).
#[serde(default)]
pub cleared: bool,
```

In `game-results/src/lib.rs`: `native_score_entry` gains a `cleared: bool` param, sets the field; the persist system (:295-343) already reads `LastStageOutcome` for the `.score.ini` write (:365 `outcome.cleared && total > 0`) — pass the same expression into `native_score_entry`. Fix the in-file `native_score_entry` unit test (lib.rs:374+).

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-scoring -p game-results -j 2`
Expected: PASS (including all migration/round-trip suites).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-scoring crates/game-results
git commit -m "feat(scoring): persist cleared flag on score entries (serde-default back-compat)"
```

---

### Task 4: Favorites persistence in config

**Files:**
- Modify: `crates/dtx-config/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn favorites_default_empty_and_round_trip() {
    let mut cfg = Config::default();
    assert!(cfg.library.favorites.is_empty());
    cfg.library.favorites.push(PathBuf::from("/songs/a/b.dtx"));
    let s = toml::to_string_pretty(&cfg).unwrap();
    let back: Config = toml::from_str(&s).unwrap();
    assert_eq!(back.library.favorites.len(), 1);
}

#[test]
fn old_config_without_library_section_loads() {
    let cfg: Config = toml::from_str("").unwrap();
    assert!(cfg.library.favorites.is_empty());
}

#[test]
fn toggle_favorite_adds_then_removes() {
    let mut cfg = Config::default();
    let p = PathBuf::from("/x.dtx");
    assert!(cfg.library.toggle_favorite(&p)); // now favorite
    assert!(cfg.library.is_favorite(&p));
    assert!(!cfg.library.toggle_favorite(&p)); // removed
    assert!(!cfg.library.is_favorite(&p));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-config -j 2 favorite`
Expected: FAIL.

- [ ] **Step 3: Implement**

New sub-struct following the existing `#[serde(default)]` section pattern (`Config` at lib.rs:33-47):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    /// Favorite charts by .dtx path.
    pub favorites: Vec<PathBuf>,
}

impl LibraryConfig {
    pub fn is_favorite(&self, path: &Path) -> bool {
        self.favorites.iter().any(|p| p == path)
    }
    /// Returns the new state (true = now favorite).
    pub fn toggle_favorite(&mut self, path: &Path) -> bool {
        if let Some(i) = self.favorites.iter().position(|p| p == path) {
            self.favorites.remove(i);
            false
        } else {
            self.favorites.push(path.to_path_buf());
            true
        }
    }
}
```

Add `pub library: LibraryConfig` to `Config` with `#[serde(default)]`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-config -j 2`
Expected: PASS (including existing save/load round-trips).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-config
git commit -m "feat(config): library section with favorites list"
```

---

### Task 5: Filter engine (pure) + recompute integration

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

- [ ] **Step 1: Define the model and write the failing tests**

Filter is a single-active cycle (matches the TAB-sort precedent; no hidden chords — the active filter renders as a visible chip, Task 6):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryFilter {
    #[default]
    All,
    Favorites,
    Unplayed,
    Played,
    Cleared,
    NotCleared,
    Recent,      // added in the last 30 days (file mtime)
    NearMyLevel, // ±1.0 display level around the player's estimate
    BandLow,     // display level < 4.0
    BandMid,     // 4.0..=6.9
    BandHigh,    // >= 7.0
}

impl LibraryFilter {
    pub fn next(self) -> Self { /* cycle in declaration order, BandHigh -> All */ }
    pub fn prev(self) -> Self { /* reverse */ }
    pub fn label(self) -> &'static str {
        // "ALL", "FAV", "UNPLAYED", "PLAYED", "CLEAR", "NOT CLEAR",
        // "NEW 30D", "MY LEVEL", "LV <4", "LV 4-6", "LV 7+"
    }
}

/// Everything a filter decision needs about one chart, precomputed per frame
/// of recompute (no I/O here).
pub struct SongStatus {
    pub favorite: bool,
    pub played: bool,
    pub cleared: bool,
}

pub fn passes_filter(
    song: &SongInfo,
    status: &SongStatus,
    filter: LibraryFilter,
    now_unix: u64,
    player_level: Option<f32>,
) -> bool {
    let display = song.dlevel.map(dtx_core::display_dlevel);
    match filter {
        LibraryFilter::All => true,
        LibraryFilter::Favorites => status.favorite,
        LibraryFilter::Unplayed => !status.played,
        LibraryFilter::Played => status.played,
        LibraryFilter::Cleared => status.cleared,
        LibraryFilter::NotCleared => status.played && !status.cleared,
        LibraryFilter::Recent => song
            .mtime_unix_secs
            .is_some_and(|m| now_unix.saturating_sub(m) < 30 * 86_400),
        LibraryFilter::NearMyLevel => match (player_level, display) {
            (Some(lvl), Some(d)) => (d - lvl).abs() <= 1.0,
            _ => false,
        },
        LibraryFilter::BandLow => display.is_some_and(|d| d < 4.0),
        LibraryFilter::BandMid => display.is_some_and(|d| (4.0..7.0).contains(&d)),
        LibraryFilter::BandHigh => display.is_some_and(|d| d >= 7.0),
    }
}
```

NOTE: check `display_dlevel`'s signature first (`grep -n 'fn display_dlevel' crates/dtx-core/src`) — it may return a formatted value or f32; adapt the band math to its actual numeric form (song_select.rs:206 shows current usage).

Tests (same file, using the existing `make_song` helper, extended for dlevel/mtime):

```rust
#[test]
fn filter_cycle_is_total_and_returns() {
    let mut f = LibraryFilter::All;
    for _ in 0..11 { f = f.next(); }
    assert_eq!(f, LibraryFilter::All);
}

#[test]
fn unplayed_and_cleared_filters() {
    let song = make_song("t", "a");
    let played = SongStatus { favorite: false, played: true, cleared: false };
    let fresh = SongStatus { favorite: false, played: false, cleared: false };
    assert!(passes_filter(&song, &fresh, LibraryFilter::Unplayed, 0, None));
    assert!(!passes_filter(&song, &played, LibraryFilter::Unplayed, 0, None));
    assert!(passes_filter(&song, &played, LibraryFilter::NotCleared, 0, None));
    assert!(!passes_filter(&song, &fresh, LibraryFilter::NotCleared, 0, None)); // never played ≠ failed
}

#[test]
fn recent_filter_uses_mtime_window() {
    let mut song = make_song("t", "a");
    let now = 2_000_000_000;
    song.mtime_unix_secs = Some(now - 86_400); // yesterday
    let s = SongStatus { favorite: false, played: false, cleared: false };
    assert!(passes_filter(&song, &s, LibraryFilter::Recent, now, None));
    song.mtime_unix_secs = Some(now - 90 * 86_400);
    assert!(!passes_filter(&song, &s, LibraryFilter::Recent, now, None));
}

#[test]
fn near_level_needs_both_levels() {
    let mut song = make_song("t", "a");
    song.dlevel = Some(50); // adapt to display_dlevel's input scale
    let s = SongStatus { favorite: false, played: false, cleared: false };
    assert!(!passes_filter(&song, &s, LibraryFilter::NearMyLevel, 0, None));
}
```

- [ ] **Step 2: Run tests to verify they fail, then implement, then pass**

Run: `cargo test -p game-menu -j 2 filter`
FAIL → implement → PASS.

- [ ] **Step 3: Player level estimate (pure)**

```rust
/// Mean display level of the player's 10 best cleared charts; None until
/// 3 clears exist (avoid a garbage estimate for new players).
pub fn player_level_estimate(store: &ScoreStore, db: &[SongInfo]) -> Option<f32> {
    let mut cleared_levels: Vec<f32> = db
        .iter()
        .filter_map(|s| {
            let cleared = store
                .history_for_path(&s.path, 1)
                .first()
                .is_some_and(|e| e.cleared);
            (cleared).then(|| s.dlevel.map(dtx_core::display_dlevel)).flatten()
        })
        .collect();
    if cleared_levels.len() < 3 {
        return None;
    }
    cleared_levels.sort_by(|a, b| b.partial_cmp(a).unwrap());
    cleared_levels.truncate(10);
    Some(cleared_levels.iter().sum::<f32>() / cleared_levels.len() as f32)
}
```

CAREFUL: `history_for_path` sorts best-score-first — "first entry cleared" is a proxy. Better: `store.entries` scan filtered by `source_path_hint == Some(path)` with `.any(|e| e.cleared)`; if `entries` isn't public, add `pub fn any_cleared_for_path(&self, path: &Path) -> bool` to `ScoreStore` (one-liner + unit test in dtx-scoring). Prefer the accessor.

Test with a hand-built `ScoreStore` (same construction as `dtx-scoring/tests/history.rs`).

- [ ] **Step 4: Wire into recompute**

Extend the signature (:156):

```rust
pub fn recompute(
    &mut self,
    all: &[SongInfo],
    filter: LibraryFilter,
    favorites: &dtx_config::LibraryConfig,
    store: &ScoreStore,
    now_unix: u64,
)
```

Inside, before grouping, compute `player_level = player_level_estimate(store, all)` once, and skip songs where `!passes_filter(...)` with `SongStatus` built per song (`favorite: favorites.is_favorite(&song.path)`, `played: !store.history_for_path(&song.path, 1).is_empty()`, `cleared: store.any_cleared_for_path(&song.path)`). Keep `matches_search` exactly as is (filters compose with search).

Add `filter: LibraryFilter` to `SongSelectSelection` (:128-142). Update the two callers (`recompute_visible` :1789, `maybe_recompute_visible` :1801) to pass the new args — both systems gain `Res<ScoreStoreResource>` and read favorites from a config resource. IMPORTANT: `persist_hovered_selection` (:562) loads config from disk per change; for favorites, mirror that pattern (load-mutate-save on toggle) and keep an in-memory `Res<LibraryFavorites>`-style resource (new `#[derive(Resource, Default)] pub struct LibraryFavorites(pub dtx_config::LibraryConfig)`) initialized on enter so recompute never touches disk. `now_unix` from `SystemTime::now()` at the call site.

`history_for_path` per song per recompute is O(entries × songs) — fine at current scale (measure via Task 1's report; cache only if it shows up).

- [ ] **Step 5: Run all song_select tests**

Run: `cargo test -p game-menu -j 2`
Expected: PASS (existing recompute tests updated to pass `LibraryFilter::All, &LibraryConfig::default(), &ScoreStore::default(), 0`).

- [ ] **Step 6: Commit**

```bash
git add crates/game-menu crates/dtx-scoring
git commit -m "feat(song-select): filter engine with favorites/played/clear/recent/level filters"
```

---

### Task 6: Keys + visible chips + random-within-filter

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

- [ ] **Step 1: Check key availability**

Run: `grep -n 'KeyCode::' crates/game-menu/src/song_select.rs | grep -oE 'KeyCode::[A-Za-z0-9]+' | sort -u`
Free keys needed: `KeyF` (cycle filter; Shift = reverse), `KeyV` (toggle favorite on hovered song), `KeyR` (random within filter). If any collide with existing hotkeys or type-to-search input, note this: **type-to-search consumes `Key::Character` events** (`search_input`, :1606-1646) — plain letters conflict! Therefore bind the three actions to `F1`-independent chords that do NOT collide with search: use `Ctrl+F` (filter), `Ctrl+D` (favorite), `Ctrl+R` (random) via `ButtonInput<KeyCode>` + `ControlLeft/Right` held, OR — better — check how TAB/existing hotkeys coexist with search (`song_select_hotkeys` :1475 uses `ButtonInput<KeyCode>` while search reads `KeyboardInput` events): mirror the TAB approach with non-character keys that search ignores. Concrete choice: `Tab` = sort (existing), `BackTab`/`Shift+Tab` untouched, filter = `F2`, favorite = `F3`, random = `F4`. Function keys never collide with typing. Verify F2/F3/F4 unused: `grep -n 'F2\|F3\|F4' crates/game-menu/src crates/game-shell/src` (F2 opens the layout editor from SongSelect per memory — if the grep confirms, shift to F5/F6/F7 and update the legend text below accordingly).

- [ ] **Step 2: Implement the systems**

In `song_select_hotkeys` (or a sibling system, matching its run conditions):

```rust
// filter cycle
if keys.just_pressed(KeyCode::F5) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    sel.filter = if shift { sel.filter.prev() } else { sel.filter.next() };
    sel.dirty = true;
}
// favorite toggle on the hovered chart
if keys.just_pressed(KeyCode::F6) {
    if let Some(idx) = chart_index(&selection) {
        let path = db.songs[idx].path.clone(); // adapt: use the visible-row -> song resolution used elsewhere
        let mut cfg = dtx_config::load();
        cfg.library.toggle_favorite(&path);
        let _ = dtx_config::save(&cfg);
        favorites.0 = cfg.library.clone();
        sel.dirty = true;
    }
}
// random within the current filtered+searched view
if keys.just_pressed(KeyCode::F7) && !sel.visible.is_empty() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    selection.folder = nanos % sel.visible.len();
    selection.difficulty = 0;
}
```

(Adapt: `chart_index` (:359) resolves the hovered chart; `dtx_config::load/save` matches the `persist_hovered_selection` pattern (:576-584); clamp runs via the existing recompute path.)

- [ ] **Step 3: Visible chips + scan footer**

- Filter chip: clone the sort-chip rendering (`update_left_cluster` :1172-1181) — a second chip showing `sel.filter.label()`, updated in the same system, with a `FilterChipText` marker component spawned next to the sort chip.
- Favorite marker: in the wheel-row text construction, prefix `"★ "` when `favorites.0.is_favorite(...)` (find where row titles are built — the `SongFolderView` rendering; keep it text-only, no new entities).
- Scan footer: one line near the search text (spawn beside `SearchText`, :663-667): `"{parsed} charts · {failures} skipped · {duration_ms} ms"` from `db.last_report`, with a `ScanFooterText` marker, filled on enter. When `failures > 0` render in the theme's warning-ish secondary color (`theme.text_secondary`).
- Legend: if song select renders a `nav_legend`/hint line, add `F5 filter · F6 fav · F7 random` (grep `nav_legend` usage in song_select).

- [ ] **Step 4: Preserve remembered selection under filters**

`restore_last_selection` (:532-560) matches against `visible` — with a filter active at restore time the remembered folder may be filtered out; the existing fallback (no match → cursor stays clamped at 0) is the correct behavior. Add a unit test:

```rust
#[test]
fn restore_with_filter_missing_song_clamps_gracefully() {
    // build visible WITHOUT the remembered folder, call restore_last_selection,
    // assert folder == 0 and no panic (mirror the existing restore tests' setup)
}
```

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p game-menu -j 2 && cargo check -p game-menu -j 2`
Expected: PASS / clean.

- [ ] **Step 6: Commit**

```bash
git add crates/game-menu
git commit -m "feat(song-select): filter/favorite/random keys, visible chips, scan footer"
```

---

### Task 7: Manual verification (bevy-brp)

- [ ] **Step 1:** Launch with a real library. Confirm: scan footer shows counts + duration; log lists any skipped charts with reasons.
- [ ] **Step 2:** Cycle F5 through all 11 filter states — chip updates, list narrows, search still composes (type "a" while FAV active → favorites matching "a").
- [ ] **Step 3:** F6 stars/unstars the hovered song (★ prefix appears; survives restart via config.toml).
- [ ] **Step 4:** F7 jumps to a random visible row repeatedly; with a 1-row filter result it stays put.
- [ ] **Step 5:** Play + clear a chart, return: PLAYED/CLEAR filters include it; UNPLAYED excludes it.
- [ ] **Step 6:** Remembered song/difficulty still restores on re-enter with filter ALL.

---

## Success-check mapping (roadmap)

- "Song selection exposes routine actions without hidden key chords" → visible chips + legend (Task 6; function keys, no chords).
- "Library scan reports duration and failures; caching follows measured need" → Task 1 (and NO cache in this plan).
- "Skip malformed charts, show failure count, log paths and reasons" → Tasks 1 + 6 footer.
- Remembered song and difficulty preserved → Task 6 Step 4.
