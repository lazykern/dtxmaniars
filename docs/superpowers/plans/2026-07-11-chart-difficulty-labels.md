# Chart Difficulty Labels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display `mstr.dtx` and other conventional chart files under their actual difficulty names.

**Architecture:** Extend current `set.def` parser to retain each `L1..L5` chart's optional label. Resolve displayed names from that authoritative entry; for bare folders, recognize conventional filenames; then retain ordinal fallback. Use resolver in grid, loading card, and start log.

**Tech Stack:** Rust, Bevy UI, cargo test.

## Global Constraints
- Keep selection, sorting, and DTX parsing unchanged.
- Preserve unknown-file ordinal behavior.
- No new dependencies.

---

### Task 1: Resolve labels from chart source

**Files:**
- Modify: `crates/game-menu/src/song_select.rs:114-124, 216-265, 1128-1138, 1578-1584, 2061-2067`
- Modify: `crates/game-menu/src/song_loading.rs:334-347`
- Test: `crates/game-menu/src/song_select.rs:2061-2067`

**Interfaces:**
- Produces: `SongFolderView::difficulty_label_for(path: &Path, ordinal: u8) -> String`
- Consumes: `SongFolderView::difficulty_label(ordinal)` for unrecognized paths.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn difficulty_labels_use_chart_source() {
    assert_eq!(SongFolderView::difficulty_label_for(Path::new("mstr.dtx"), 0), "MAS");
    assert_eq!(SongFolderView::difficulty_label_for(Path::new("ext.dtx"), 0), "EXT");
    assert_eq!(SongFolderView::difficulty_label_for(Path::new("chart.dtx"), 1), "ADV");
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p game-menu difficulty_labels_use_chart_source`
Expected: compilation failure because `difficulty_label_for` does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn difficulty_label_for(path: &Path, ordinal: u8) -> String {
    if let Some(label) = set_def_label_for(path) { return label; }
    match path.file_stem().and_then(|stem| stem.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("bsc" | "bas" | "basic") => "BASIC",
        Some("adv" | "advanced") => "ADV",
        Some("ext" | "extreme") => "EXT",
        Some("mas" | "mst" | "mstr" | "master") => "MAS",
        Some("edit") => "EDIT",
        _ => Self::difficulty_label(ordinal),
    }.to_string()
}
```

Have `set_def_label_for` use parsed `#LxLABEL`, or the existing standard label for its `#LxFILE` slot. Replace grid, loading-card, and start-log label calls with the resolver.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p game-menu difficulty_labels_use_chart_source`
Expected: PASS.

- [ ] **Step 5: Verify package**

Run: `cargo test -p game-menu --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/game-menu/src/song_select.rs crates/game-menu/src/song_loading.rs docs/superpowers/plans/2026-07-11-chart-difficulty-labels.md
git commit -m "fix(menu): resolve chart difficulty labels"
```
