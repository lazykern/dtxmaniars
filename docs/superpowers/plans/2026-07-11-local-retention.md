# Local Retention Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Local weekly goals, PB pace ghost during gameplay, and shareable result cards — all offline, no accounts, no backend.

**Roadmap precondition:** Phase 4.2 explicitly runs "after history and results analytics prove useful". Execute this plan only after `2026-07-11-results-coaching.md` has shipped and been used. Hard code dependency: `ScoreEntry.cleared` from `2026-07-11-song-library-usability.md` Task 3 must exist (weekly clear counts read it). Depends on nothing else from Phase 4.

**Architecture:** Weekly stats are a pure fold over `ScoreStore.entries[].played_at` (u64 unix secs — the repo's only time currency) using a rolling 7-day window; the goal target persists in config. The PB "ghost" v1 is a pace ghost: live score delta versus the personal best projected by note progress (a true replay ghost needs per-run timelines the store doesn't record — `replay_ref` is reserved for that future). Share cards are a window screenshot of the results screen saved locally (the repo has zero render-to-texture infra; the screenshot API is the smallest honest v1).

**Tech Stack:** existing crates; bevy 0.19's screenshot API (`bevy::render::view::screenshot` — verify exact observer API at execution via ctx7, it changed across 0.15+).

**Source basis (verified 2026-07-11):**
- Scores: `ScoreStore { entries: Vec<ScoreEntry>, .. }` (`dtx-scoring/src/store.rs:17-30`); `ScoreEntry { score: u32, judgments, rank, played_at: u64, cleared (from library plan), .. }` (:44-69); `best_for_chart(canonical_hash)` (:225-230); wrapped as `ScoreStoreResource` (game-shell/src/score_store.rs:13).
- Day math precedent: `format_unix_date` (`dtx-ui/src/widget/play_history.rs:89-104`, `played_at / 86_400` civil-day conversion). No chrono anywhere — keep it that way.
- Data-resource widget pattern: `PlayHistoryData { rows }` + marker texts + fill/render split (`play_history.rs`, wired in song_select :733, :1193-1212, :1286-1316).
- Live score during play: `Score(u64)`, `DrumScoring { total_notes, .. }`, `JudgmentCounts` (gameplay-drums resources); HUD text via `HudDisplayCache` (hud.rs:481 region).
- Results: `spawn_result`/`result_input` (`game-results/src/lib.rs:110-290`).
- Screenshot infra: none in repo (grepped: no Screenshot/RenderTarget usage).
- Config: `SystemConfig`/`GameplayConfig` serde-default sections (`dtx-config/src/lib.rs`), settings rows in `settings_data.rs`.

---

### Task 1: Weekly stats (pure) + goal target in config

**Files:**
- Create: `crates/dtx-scoring/src/weekly.rs`
- Modify: `crates/dtx-scoring/src/lib.rs` (module + re-export)
- Modify: `crates/dtx-config/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

`crates/dtx-scoring/src/weekly.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entry(played_at: u64, cleared: bool) -> /* ScoreEntry */ {
        // build via the same constructor style dtx-scoring's store tests use,
        // varying played_at + cleared
    }

    #[test]
    fn counts_only_the_rolling_window() {
        let now = 1_800_000_000u64;
        let entries = vec![
            entry(now - 3 * 86_400, true),   // in window, cleared
            entry(now - 6 * 86_400, false),  // in window
            entry(now - 8 * 86_400, true),   // out
        ];
        let s = weekly_stats(&entries, now);
        assert_eq!(s.plays, 2);
        assert_eq!(s.clears, 1);
    }

    #[test]
    fn empty_store_is_zero() {
        let s = weekly_stats(&[], 1_800_000_000);
        assert_eq!((s.plays, s.clears), (0, 0));
    }

    #[test]
    fn goal_line_reads_naturally() {
        let s = WeeklyStats { plays: 12, clears: 3 };
        assert_eq!(goal_line(&s, 0), "This week: 12 plays · 3 clears");
        assert_eq!(goal_line(&s, 20), "This week: 12/20 plays · 3 clears");
        let done = WeeklyStats { plays: 21, clears: 3 };
        assert_eq!(goal_line(&done, 20), "This week: 21/20 plays — goal met! · 3 clears");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-scoring -j 2 weekly`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement**

```rust
//! Rolling 7-day local stats. No calendar weeks, no timezones — a rolling
//! window over played_at unix seconds keeps this dependency-free and testable.

use crate::ScoreEntry;

pub const WEEK_SECS: u64 = 7 * 86_400;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WeeklyStats {
    pub plays: u32,
    pub clears: u32,
}

pub fn weekly_stats(entries: &[ScoreEntry], now_unix: u64) -> WeeklyStats {
    let cutoff = now_unix.saturating_sub(WEEK_SECS);
    let mut s = WeeklyStats::default();
    for e in entries.iter().filter(|e| e.played_at >= cutoff && e.played_at <= now_unix) {
        s.plays += 1;
        if e.cleared {
            s.clears += 1;
        }
    }
    s
}

pub fn goal_line(stats: &WeeklyStats, target_plays: u32) -> String {
    let plays = if target_plays == 0 {
        format!("{} plays", stats.plays)
    } else if stats.plays >= target_plays {
        format!("{}/{} plays — goal met!", stats.plays, target_plays)
    } else {
        format!("{}/{} plays", stats.plays, target_plays)
    };
    format!("This week: {plays} · {} clears", stats.clears)
}
```

(`entries` needs public read access from game-menu — `ScoreStore.entries` is already `pub` per the struct def; if not, add `pub fn entries(&self) -> &[ScoreEntry]`.)

Config: add `weekly_play_target: u32` (serde default 0 = off) to `GameplayConfig` + round-trip test + a "Weekly Goal" stepper row (0..100 step 5, "0 = off") in the System tab of `settings_data.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-scoring -p dtx-config -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-scoring crates/dtx-config crates/gameplay-drums
git commit -m "feat(retention): rolling weekly stats with configurable play goal"
```

---

### Task 2: Show the weekly line in song select

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

- [ ] **Step 1: Spawn + fill (play_history widget pattern)**

- Spawn a one-line `Text` with marker `WeeklyGoalText` in the left cluster near the play-history panel (grep `spawn_play_history` call at :733 and place adjacent).
- Fill on `OnEnter(SongSelect)` and after each results round-trip (the enter system suffices — the store reloads/updates before re-entry): system reads `Res<ScoreStoreResource>` + config target, `SystemTime::now()` for `now_unix`, writes `goal_line(&weekly_stats(...), target)`.

- [ ] **Step 2: Test the glue as pure fns only (already done in Task 1); build + manual**

Run: `cargo check -p game-menu -j 2` → clean.
Manual: line shows plausible counts; play a song, return — plays increments; goal-met phrasing at target.

- [ ] **Step 3: Commit**

```bash
git add crates/game-menu
git commit -m "feat(retention): weekly goal line in song select"
```

---

### Task 3: PB pace ghost during gameplay

**Files:**
- Create: `crates/gameplay-drums/src/pb_pace.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module, plugin, enter-hook)

- [ ] **Step 1: Pure pace math, test-first**

```rust
//! PB pace ghost: compares the live score against the personal best scaled
//! by note progress. Not a replay ghost — the store has no per-run timeline
//! (replay_ref is reserved for that).

#[derive(Resource, Debug, Default)]
pub struct PbPace {
    /// Final score of the previous best; None = first play (ghost hidden).
    pub pb_score: Option<u32>,
}

/// Signed delta vs PB pace at the current progress point.
pub fn pace_delta(live_score: u64, pb_score: u32, judged: u32, total_notes: u32) -> i64 {
    if total_notes == 0 {
        return 0;
    }
    let expected = u64::from(pb_score) * u64::from(judged.min(total_notes)) / u64::from(total_notes);
    live_score as i64 - expected as i64
}

pub fn pace_label(delta: i64) -> String {
    if delta >= 0 { format!("+{delta} vs best") } else { format!("{delta} vs best") }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn on_pace_is_zero() {
        assert_eq!(pace_delta(500_000, 1_000_000, 50, 100), 0);
    }
    #[test]
    fn ahead_and_behind_are_signed() {
        assert!(pace_delta(600_000, 1_000_000, 50, 100) > 0);
        assert!(pace_delta(400_000, 1_000_000, 50, 100) < 0);
        assert_eq!(pace_label(-1200), "-1200 vs best");
    }
    #[test]
    fn zero_total_notes_safe() {
        assert_eq!(pace_delta(1, 1, 1, 0), 0);
    }
}
```

- [ ] **Step 2: Wire**

- On `OnEnter(AppState::Performance)`: look up the PB — reuse the same chart-identity derivation the results persist system uses (see results-coaching plan Task 3's `canonical` note; if the canonical hash isn't available pre-play, use `store.history_for_path(&source_path, 1).first().map(|e| e.score)` — path-based, cheap, already the song-select convention). Insert `PbPace`. Skip (None) in practice sessions.
- HUD: a small `Text` widget near the score display updated when `Score` changes: `judged` = sum of `JudgmentCounts` fields, `total_notes` from `DrumScoring.total_notes`. Hidden (`Visibility::Hidden` or empty string) when `pb_score.is_none()` or in practice. Placement: follow how the existing score text is spawned/updated in `hud.rs` (grep `HudDisplayCache` writes around :481) — attach to the same container, one line, `theme.text_secondary`, `label_font()`.

- [ ] **Step 3: Run tests + schedule guard**

Run: `cargo test -p gameplay-drums -j 2`
Expected: PASS incl. `fixed_update_schedule_ordering`.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums
git commit -m "feat(retention): PB pace ghost line during gameplay"
```

---

### Task 4: Shareable result card (local screenshot)

**Files:**
- Modify: `crates/game-results/src/lib.rs`

- [ ] **Step 1: Verify the bevy 0.19 screenshot API first**

Run: `npx ctx7@latest library "Bevy" "take a screenshot of the primary window and save to disk"` and confirm the current form (0.15+ pattern: `commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))` from `bevy::render::view::screenshot::{Screenshot, save_to_disk}`). Use exactly what the docs say for 0.19 — do not trust this plan's snippet over the fetched docs.

- [ ] **Step 2: Pure path builder, test-first**

```rust
/// ~/Pictures/dtxmaniars/<title>-<played_at>.png (title sanitized).
pub fn share_card_path(pictures_dir: &Path, title: &str, played_at: u64) -> PathBuf {
    let safe: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(60)
        .collect();
    pictures_dir.join("dtxmaniars").join(format!("{safe}-{played_at}.png"))
}

#[test]
fn share_path_is_sanitized() {
    let p = share_card_path(Path::new("/home/u/Pictures"), "夜/曲: A", 123);
    let s = p.to_string_lossy();
    assert!(s.ends_with("-123.png"));
    assert!(!s.contains(':'));
    assert!(s.contains("dtxmaniars"));
}
```

(Note: `is_alphanumeric` keeps CJK — intended; only path-hostile chars become `_`.) Pictures dir: `dirs`-style lookup without the dep — `std::env::var_os("XDG_PICTURES_DIR")` is rarely set; use `$HOME/Pictures` fallback, else the scores.json directory. One helper `pictures_dir() -> PathBuf`, no test (env-dependent).

- [ ] **Step 3: Wire the key**

In `result_input`: `KeyCode::KeyS` → `create_dir_all` the target dir, spawn the screenshot-to-disk command per the fetched API, and surface the saved path on screen (append a stat row is too late post-spawn — instead reuse whatever transient text results has, or spawn a small bottom-line `Text` "Saved: <path>" with a marker; keep it minimal). Log `info!("result card saved: {}", path.display())` regardless. Add "S save card" to the results hint row (:190 region).

- [ ] **Step 4: Run + manual**

Run: `cargo test -p game-results -j 2 && cargo check -p game-results -j 2` → PASS/clean.
Manual: finish a song, press S on results → PNG exists in `~/Pictures/dtxmaniars/`, shows the full results screen (score, rank, coaching rows — that IS the share card).

- [ ] **Step 5: Commit**

```bash
git add crates/game-results
git commit -m "feat(retention): save result card screenshot locally"
```

---

## Verification (whole plan)

1. `cargo test -p dtx-scoring -p dtx-config -p game-menu -p gameplay-drums -p game-results -j 2` green.
2. Manual week simulation: temporarily set `DTX_SCORES_PATH` to a fixture store with known `played_at` values → weekly line matches hand-count.
3. Second play of a chart shows the pace line swinging ahead/behind; first-ever play shows nothing; practice shows nothing.
4. Result card PNG opens in an image viewer and contains the full results panel.
5. No network code introduced anywhere (`grep -rn 'http\|reqwest' crates/ | grep -v test` unchanged).

## Deliberately out of scope

- True replay ghosts (needs per-run timelines; `replay_ref` is the future hook).
- Styled/branded card compositing (render-to-texture greenfield; screenshot v1 first).
- Calendar-aligned weeks, streaks, notifications.
