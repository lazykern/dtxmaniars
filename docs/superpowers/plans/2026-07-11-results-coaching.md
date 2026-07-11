# Results Coaching + Practice Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Results explain the run in plain language — signed early/late histogram, timing spread, per-lane misses and bias, weak-section timeline, personal-best delta — and offer one action, "Practice weakest section", that enters the existing practice loop with section, lead-in, tempo, and reason. Practice attempts never write normal scores.

**Architecture:** Today per-hit deltas die at the judge: results read only aggregate counts (`spawn_result` reads `Score/Combo/JudgmentCounts/ActiveChart/DrumScoring` — no timing data). We add one accumulator resource `RunDiagnostics` in gameplay-drums, fed where the existing score systems already consume `JudgmentEvent`/`NoteMissed`, reusing practice's proven `LaneDiagnosis` for per-lane bias. The handoff extends `game_shell::PracticeIntent` (today a bare bool) with an optional focus payload; the existing gate — score persistence skips whenever a `PracticeSession` exists (`game-results/src/lib.rs:306-309`) — makes handed-off sessions score-safe with zero extra work.

**Tech Stack:** existing crates only.

**Source basis (verified 2026-07-11):**
- Events: `JudgmentEvent { lane, kind, delta_ms: i64, chip_idx }` (`crates/gameplay-drums/src/events.rs:18-25`; delta < 0 = early); `NoteMissed { lane, audio_ms, chip_idx }` (:29-34).
- Accumulation: `update_score_system` (`score.rs:31-86`) and `update_miss_system` (:88-114); coarse `FastSlowCount` exists (`resources.rs:205-209`) but is unused by results.
- Results: `crates/game-results/src/lib.rs` — `spawn_result` :110-192 (`stat_rows: Vec<(String, f32)>`, staggered reveal), `result_input` :276-290 (no retry/practice action), persist :295-343 with practice gate :306-309, `native_score_entry` :79-108. Pure helpers tested in-file :374+.
- PB: `ScoreStore::best_for_chart(canonical_hash)` (`store.rs:225-230`); `ScoreStoreResource` already a param of the persist system (:301). Canonical hash via `dtx_scoring::identity::canonical_chart_hash(chart)` (identity.rs:95-98).
- Practice: `PracticeIntent(pub bool)` (`game-shell/src/states.rs:100-101`), set at `song_select.rs:1518-1586`, consumed by `enter_practice_session` (`practice/mod.rs:70-76`). Section = `LoopRegion { start_ms, end_ms }` (`practice/session.rs:13-17`); lead-in = `PrerollSetting` + `preroll_target(timeline, preroll, intent_ms)` (:19-52); tempo bounds 0.5..1.5 (:8-10). Seek entry: `SeekToChartTime { target_ms, snap, attempt_start_ms }` (`seek.rs:24-35`); `RestartLoop` handler (`actions.rs:127-139`) is the exact shape to copy. Toasts: `practice/toast.rs`.
- Sections: `ChipTimeline { bar_ms, judge_ms_by_idx, density: [f32;128], end_ms, .. }` (`timeline.rs:50-239`), `bar_start_before(ms)`; `AccuracyHistory` 128-bucket precedent (`resources.rs:597-616`).
- Per-lane model to reuse: `practice::diagnosis::LaneDiagnosis`/`LaneAgg` (`diagnosis.rs`: `apply_judgment/apply_miss/mean_delta_ms/bias_label/sorted_rows`, `BIAS_THRESHOLD_MS=10.0`).
- No histogram utility exists anywhere; judgment windows are ±117 ms max (`hit_ranges.rs:76-102`).

---

### Task 1: `RunDiagnostics` accumulator (pure core)

**Files:**
- Create: `crates/gameplay-drums/src/run_diagnostics.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (`pub mod run_diagnostics;`)

- [ ] **Step 1: Write the file test-first**

```rust
//! Full-run timing diagnostics for the results screen. Unlike practice's
//! LaneDiagnosis (loop-scoped), this accumulates over the whole normal play.

use bevy::prelude::*;
use crate::practice::diagnosis::LaneDiagnosis;

/// 12 bins of 20 ms covering -120..+120 (judgment windows are <= ±117 ms).
pub const BIN_MS: i64 = 20;
pub const BIN_COUNT: usize = 12;
/// Song positions for the weak-section timeline.
pub const SECTIONS: usize = 32;

#[derive(Resource, Default)]
pub struct RunDiagnostics {
    pub histogram: [u32; BIN_COUNT],
    sum_ms: f64,
    sum_sq_ms: f64,
    pub judged: u32,
    pub lanes: LaneDiagnosis,
    pub section_hits: [u32; SECTIONS],
    pub section_misses: [u32; SECTIONS],
}

impl RunDiagnostics {
    pub fn record_hit(&mut self, lane: crate::LaneId, kind: crate::JudgmentKind, delta_ms: i64, judge_ms: i64, end_ms: i64) {
        let idx = bin_index(delta_ms);
        self.histogram[idx] += 1;
        self.sum_ms += delta_ms as f64;
        self.sum_sq_ms += (delta_ms * delta_ms) as f64;
        self.judged += 1;
        self.lanes.apply_judgment(lane, kind, delta_ms as f64);
        if let Some(s) = section_index(judge_ms, end_ms) {
            self.section_hits[s] += 1;
        }
    }

    pub fn record_miss(&mut self, lane: crate::LaneId, judge_ms: i64, end_ms: i64) {
        self.lanes.apply_miss(lane);
        if let Some(s) = section_index(judge_ms, end_ms) {
            self.section_misses[s] += 1;
        }
    }

    pub fn mean_ms(&self) -> f64 {
        if self.judged == 0 { 0.0 } else { self.sum_ms / f64::from(self.judged) }
    }

    /// Sample stddev of hit deltas ("timing spread").
    pub fn spread_ms(&self) -> f64 {
        if self.judged < 2 { return 0.0; }
        let n = f64::from(self.judged);
        ((self.sum_sq_ms - self.sum_ms * self.sum_ms / n) / (n - 1.0)).max(0.0).sqrt()
    }

    pub fn early_late_counts(&self) -> (u32, u32) {
        let early: u32 = self.histogram[..BIN_COUNT / 2].iter().sum();
        let late: u32 = self.histogram[BIN_COUNT / 2..].iter().sum();
        (early, late)
    }
}

pub fn bin_index(delta_ms: i64) -> usize {
    let clamped = delta_ms.clamp(-(BIN_MS * (BIN_COUNT as i64) / 2), BIN_MS * (BIN_COUNT as i64) / 2 - 1);
    ((clamped + BIN_MS * (BIN_COUNT as i64) / 2) / BIN_MS) as usize
}

pub fn section_index(judge_ms: i64, end_ms: i64) -> Option<usize> {
    if end_ms <= 0 || judge_ms < 0 { return None; }
    Some((((judge_ms as u128 * SECTIONS as u128) / end_ms as u128) as usize).min(SECTIONS - 1))
}

/// Worst section = highest miss share with at least one note. Returns None
/// when nothing was missed (no coaching to give).
pub fn weakest_section(hits: &[u32; SECTIONS], misses: &[u32; SECTIONS]) -> Option<usize> {
    let mut worst: Option<(usize, f64)> = None;
    for i in 0..SECTIONS {
        let total = hits[i] + misses[i];
        if total == 0 || misses[i] == 0 { continue; }
        let rate = f64::from(misses[i]) / f64::from(total);
        if worst.map_or(true, |(_, w)| rate > w) {
            worst = Some((i, rate));
        }
    }
    worst.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_are_signed_and_clamped() {
        assert_eq!(bin_index(-120), 0);
        assert_eq!(bin_index(-1), 5);
        assert_eq!(bin_index(0), 6);
        assert_eq!(bin_index(119), 11);
        assert_eq!(bin_index(-999), 0);
        assert_eq!(bin_index(999), 11);
    }

    #[test]
    fn spread_and_mean() {
        let mut d = RunDiagnostics::default();
        for delta in [-10i64, 0, 10] {
            d.record_hit(crate::LaneId::default(), crate::JudgmentKind::Perfect, delta, 0, 1000);
        }
        assert!(d.mean_ms().abs() < 1e-9);
        assert!((d.spread_ms() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn weakest_section_picks_highest_miss_rate() {
        let mut hits = [10u32; SECTIONS];
        let mut misses = [0u32; SECTIONS];
        misses[5] = 2;  // 2/12
        misses[20] = 9; // 9/19 -> worse
        assert_eq!(weakest_section(&hits, &misses), Some(20));
        misses = [0; SECTIONS];
        assert_eq!(weakest_section(&hits, &misses), None);
        let _ = &mut hits;
    }

    #[test]
    fn section_index_spans_song() {
        assert_eq!(section_index(0, 32_000), Some(0));
        assert_eq!(section_index(31_999, 32_000), Some(31));
        assert_eq!(section_index(0, 0), None);
    }
}
```

Adapt the small unknowns to reality before compiling: `LaneId`/`JudgmentKind` paths and whether `LaneId` implements `Default` (if not, use a concrete lane in tests — grep `enum LaneId`); `LaneDiagnosis::apply_judgment`'s exact signature (`diagnosis.rs:64-107`).

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums --lib run_diagnostics -j 2`
Expected: PASS after fixing the adaptation points. If `practice::diagnosis` is `pub(crate)`-scoped, widen `LaneDiagnosis` and its methods to `pub` (they are plain data + math).

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums
git commit -m "feat(results): RunDiagnostics accumulator — histogram, spread, lanes, sections"
```

---

### Task 2: Feed and reset the accumulator

**Files:**
- Modify: `crates/gameplay-drums/src/score.rs` (:31-86, :88-114)
- Modify: `crates/gameplay-drums/src/lib.rs` (init + reset registration)

- [ ] **Step 1: Feed from the existing consumers**

`update_score_system` already iterates `JudgmentEvent` — add `mut diag: ResMut<run_diagnostics::RunDiagnostics>` and `timeline: Res<...ChipTimeline...>` (grep the resource type/name used by practice systems for `judge_ms_by_idx`) and inside the loop:

```rust
let judge_ms = timeline.judge_ms_by_idx.get(ev.chip_idx).copied().unwrap_or(-1);
diag.record_hit(ev.lane, ev.kind, ev.delta_ms, judge_ms, timeline.end_ms);
```

Mirror in `update_miss_system` for `NoteMissed` → `diag.record_miss(ev.lane, judge_ms, timeline.end_ms)`.

CHECK: is `ChipTimeline` available as a `Res` in FixedUpdate where these systems run? Grep how `practice/stats.rs:65-122` accesses `judge_ms_by_idx` and copy that access pattern exactly (it may live inside another resource). If timeline is absent in non-practice play, fall back to `NoteMissed.audio_ms`/a `GameplayClock` read for the section position and note it in the code.

- [ ] **Step 2: Reset on entry**

`init_resource::<RunDiagnostics>` in the plugin; insert a fresh `RunDiagnostics::default()` in `OnEnter(AppState::Performance)` (colocate with `apply_config_on_enter` registration in lib.rs). The resource intentionally SURVIVES into `AppState::Result` (results read it) and gets replaced on the next Performance entry.

- [ ] **Step 3: Schedule guard + suite**

Run: `cargo test -p gameplay-drums -j 2` (must include `fixed_update_schedule_ordering`).
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums
git commit -m "feat(results): accumulate run diagnostics during play"
```

---

### Task 3: Plain-language result rows + PB delta

**Files:**
- Modify: `crates/game-results/src/lib.rs`

- [ ] **Step 1: Pure formatters, test-first (in-file tests, existing convention)**

```rust
/// "You hit 12 ms early on average (spread ±23 ms)" — no rhythm-game jargon.
pub fn timing_summary_line(mean_ms: f64, spread_ms: f64) -> String {
    let dir = if mean_ms <= -1.0 { "early" } else if mean_ms >= 1.0 { "late" } else { "on time" };
    if dir == "on time" {
        format!("Timing: on time on average (spread ±{spread_ms:.0} ms)")
    } else {
        format!("Timing: {:.0} ms {dir} on average (spread ±{spread_ms:.0} ms)", mean_ms.abs())
    }
}

pub fn early_late_line(early: u32, late: u32) -> String {
    format!("Early hits {early} · Late hits {late}")
}

/// Section index -> human bar-free label ("around 2:10" style needs ms).
pub fn section_label(index: usize, end_ms: i64) -> String {
    let start_s = (index as i64 * end_ms / super::SECTIONS_I64) / 1000;
    format!("around {}:{:02}", start_s / 60, start_s % 60)
}

pub fn pb_delta_line(new_score: u64, prev_best: Option<u32>) -> String {
    match prev_best {
        None => "First play of this chart".to_string(),
        Some(pb) if new_score > u64::from(pb) => format!("New personal best (+{})", new_score - u64::from(pb)),
        Some(pb) => format!("{} below your best", u64::from(pb) - new_score),
    }
}

#[cfg(test)]
mod coaching_tests {
    use super::*;
    #[test]
    fn timing_line_reads_naturally() {
        assert_eq!(timing_summary_line(-12.4, 23.0), "Timing: 12 ms early on average (spread ±23 ms)");
        assert!(timing_summary_line(0.2, 5.0).contains("on time"));
    }
    #[test]
    fn pb_lines() {
        assert_eq!(pb_delta_line(100, None), "First play of this chart");
        assert_eq!(pb_delta_line(150, Some(100)), "New personal best (+50)");
        assert_eq!(pb_delta_line(80, Some(100)), "20 below your best");
    }
}
```

(Define `SECTIONS_I64` or take `SECTIONS` from `gameplay_drums::run_diagnostics`; game-results already depends on gameplay-drums.)

- [ ] **Step 2: Run tests**

Run: `cargo test -p game-results -j 2 coaching`
Expected: PASS.

- [ ] **Step 3: Extend `spawn_result`**

Add params: `diag: Res<gameplay_drums::run_diagnostics::RunDiagnostics>`, `store: Res<ScoreStoreResource>`. After the existing rows (:145-192) append, in order:

```rust
// PB delta — computed BEFORE persistence (persist runs OnExit, so best_for_chart
// still returns the previous best here).
let canonical = dtx_scoring::identity::canonical_chart_hash(&active.chart); // adapt: reuse however the persist system derives identity
let prev_best = store.best_for_chart(&canonical).map(|e| e.score);
stat_rows.push((pb_delta_line(score.0, prev_best), 0.0));
stat_rows.push((timing_summary_line(diag.mean_ms(), diag.spread_ms()), 0.0));
let (early, late) = diag.early_late_counts();
stat_rows.push((early_late_line(early, late), 0.0));
// worst lane (only when it says something)
if let Some(row) = diag.lanes.sorted_rows().first() {
    stat_rows.push((format!("Weakest pad: {} — {} misses, {}", row.lane_label, row.misses, row.bias), 0.0));
}
if let Some(sec) = weakest_section(&diag.section_hits, &diag.section_misses) {
    stat_rows.push((format!("Trouble spot: {} — press P to practice it", section_label(sec, diag_end_ms)), 0.0));
}
```

Adapt `sorted_rows()`'s actual row shape from `diagnosis.rs` (`bias_label()` produces "rushing/dragging/on time" text — reuse verbatim); `diag_end_ms` needs `end_ms` carried on `RunDiagnostics` (add `pub end_ms: i64` set in Task 2's feed). Check how the persist system builds `ChartIdentity` (:295-343) and reuse the same derivation for `canonical` — do not invent a second identity path; if hashing there needs the parsed chart, stash the canonical hash on a resource at load instead (grep first: `canonical_chart_hash` callers).

Keep score and rank rows untouched and first (roadmap: keep score/rank visible).

- [ ] **Step 4: Build + run**

Run: `cargo test -p game-results -j 2 && cargo check -p game-results -j 2`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/game-results
git commit -m "feat(results): coaching rows — PB delta, timing bias/spread, weak lane/section"
```

---

### Task 4: PracticeIntent carries a focus payload

**Files:**
- Modify: `crates/game-shell/src/states.rs:100-101`
- Modify: `crates/game-menu/src/song_select.rs` (Practice verb site, :1518-1586)
- Modify: `crates/gameplay-drums/src/practice/mod.rs:70-80`

- [ ] **Step 1: Write the failing test**

In `game-shell` tests:

```rust
#[test]
fn practice_intent_default_inactive_no_focus() {
    let i = PracticeIntent::default();
    assert!(!i.active);
    assert!(i.focus.is_none());
}
```

- [ ] **Step 2: Implement the type**

Replace `PracticeIntent(pub bool)`:

```rust
/// Section handoff for "Practice weakest section" (results screen). Lives in
/// game-shell so menu, results, and gameplay can all speak it.
#[derive(Debug, Clone, PartialEq)]
pub struct PracticeFocus {
    pub start_ms: i64,
    pub end_ms: i64,
    /// 0.5..=1.5; practice clamps.
    pub tempo: f32,
    /// Shown as a toast: why this section ("6 misses around 1:20").
    pub reason: String,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct PracticeIntent {
    pub active: bool,
    pub focus: Option<PracticeFocus>,
}
```

Compiler drives the two existing users: song select sets `practice_intent.active = true; practice_intent.focus = None;` (:1518-1586); `enter_practice_session` (`practice/mod.rs:70-76`) tests `intent.active` instead of `intent.0`. `remove_practice_session` (:78-80) should also reset the intent (`*intent = PracticeIntent::default()`) so a stale focus never leaks into the next manual practice — check whether it already resets the bool; mirror.

- [ ] **Step 3: Run**

Run: `cargo test -p game-shell -j 2 && cargo check -p game-menu -p gameplay-drums -j 2`
Expected: PASS / clean.

- [ ] **Step 4: Commit**

```bash
git add crates/game-shell crates/game-menu crates/gameplay-drums
git commit -m "feat(practice): intent carries optional section/tempo/reason focus"
```

---

### Task 5: The handoff — results key → focused practice session

**Files:**
- Modify: `crates/game-results/src/lib.rs` (`result_input` :276-290, `spawn_result` hint row)
- Create: `apply_practice_focus` system in `crates/gameplay-drums/src/practice/mod.rs` (or `session.rs`)

- [ ] **Step 1: Compute the focus in results**

Pure fn in game-results, test-first:

```rust
/// Section ms range + lead-in-ready payload from diagnostics. Lead-in itself
/// is applied by practice's PrerollSetting; we hand over the raw section.
pub fn practice_focus_from_diag(
    section: usize,
    end_ms: i64,
    misses_in_section: u32,
) -> game_shell::PracticeFocus {
    let sections = gameplay_drums::run_diagnostics::SECTIONS as i64;
    let start_ms = section as i64 * end_ms / sections;
    let stop_ms = (section as i64 + 1) * end_ms / sections;
    game_shell::PracticeFocus {
        start_ms,
        end_ms: stop_ms,
        tempo: 0.8, // start slow; ramp/user keys take over from here
        reason: format!("{misses_in_section} misses in this section"),
    }
}

#[test]
fn focus_covers_the_section() {
    let f = practice_focus_from_diag(16, 320_000, 6);
    assert_eq!(f.start_ms, 160_000);
    assert_eq!(f.end_ms, 170_000);
    assert!(f.reason.contains("6 misses"));
}
```

- [ ] **Step 2: Wire the key**

In `result_input`, alongside the existing Confirm/Back handling:

```rust
if keys.just_pressed(KeyCode::KeyP) {
    if let Some(sec) = weakest_section(&diag.section_hits, &diag.section_misses) {
        intent.active = true;
        intent.focus = Some(practice_focus_from_diag(sec, diag.end_ms, diag.section_misses[sec]));
        request_transition(AppState::SongLoading); // same SelectedSong -> same chart
        return;
    }
}
```

(`result_input` gains `Res<RunDiagnostics>` + `ResMut<PracticeIntent>`; match how the existing transition request is issued in this fn. `SelectedSong` persists from the play, so SongLoading reloads the same chart — verify by grepping who clears `SelectedSong`; if it clears on Result entry, set it from `ActiveChart`'s source path here.) Only show the "press P" hint row (Task 3) when a weak section exists; also gate the key on `weakest_section(...).is_some()` (done above). Pad/MIDI parity: if `result_input` consumes `NavAction` verbs, map an unused verb or document keyboard-only for v1 with the hint text saying "P".

- [ ] **Step 3: Apply the focus on the practice side**

In `practice/mod.rs`, extend `enter_practice_session` or add an `OnEnter(Performance)` follow-up:

```rust
/// Consumes intent.focus into the freshly inserted session. Runs after
/// enter_practice_session; timeline-dependent seek is deferred to first
/// Update via SeekToChartTime (same shape as RestartLoop, actions.rs:127-139).
pub fn apply_practice_focus(
    mut intent: ResMut<game_shell::PracticeIntent>,
    session: Option<ResMut<PracticeSession>>,
    timeline: Option<Res</* ChipTimeline resource, same access as actions.rs */>>,
    mut seeks: MessageWriter<crate::seek::SeekToChartTime>,
    mut toasts: /* match practice/toast.rs's emission API */,
) {
    let (Some(mut session), Some(timeline)) = (session, timeline) else { return };
    let Some(focus) = intent.focus.take() else { return };
    // snap section edges outward to bar lines so the loop starts musically
    let start = timeline.bar_start_before(focus.start_ms);
    session.transport.loop_region = Some(LoopRegion { start_ms: start, end_ms: focus.end_ms });
    session.transport.user_tempo = focus.tempo.clamp(RATE_MIN, RATE_MAX);
    let target = preroll_target(&timeline, session.transport.preroll, start);
    seeks.write(SeekToChartTime { target_ms: target, snap: None, attempt_start_ms: Some(start) });
    // toast the reason (reuse practice/toast.rs)
}
```

Register in Update, `run_if(in_state(AppState::Performance))`, ordered after the session-insert system; the `Option` guards make it a no-op until both resources exist, and `focus.take()` makes it one-shot. Adapt names (`MessageWriter` vs `EventWriter`, toast API, timeline resource) to the exact forms used in `actions.rs:127-139` — copy that handler's imports wholesale.

- [ ] **Step 4: Headless flow test**

In `crates/gameplay-drums/tests/practice_mode.rs`:

```rust
#[test]
fn practice_focus_arms_loop_and_tempo() {
    let mut app = build_app();
    app.world_mut().resource_mut::<game_shell::PracticeIntent>().active = true;
    app.world_mut().resource_mut::<game_shell::PracticeIntent>().focus =
        Some(game_shell::PracticeFocus { start_ms: 8_000, end_ms: 12_000, tempo: 0.8, reason: "test".into() });
    // drive the app into Performance the way this file's other tests do, then:
    app.update();
    let session = app.world().resource::<gameplay_drums::practice::PracticeSession>();
    let region = session.transport.loop_region.expect("loop armed");
    assert!(region.start_ms <= 8_000); // bar-snapped outward
    assert_eq!(region.end_ms, 12_000);
    assert!((session.transport.user_tempo - 0.8).abs() < 1e-6);
    assert!(app.world().resource::<game_shell::PracticeIntent>().focus.is_none()); // consumed
}
```

(Mirror this file's existing state-entry bootstrapping exactly — it already inserts sessions and drives updates.)

- [ ] **Step 5: Run + schedule guard**

Run: `cargo test -p gameplay-drums -p game-results -j 2`
Expected: PASS incl. `fixed_update_schedule_ordering`.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums crates/game-results
git commit -m "feat(practice): one-action weakest-section handoff from results"
```

---

### Task 6: Manual verification (bevy-brp)

- [ ] **Step 1:** Play a chart badly on purpose (miss a cluster mid-song). Results show: score/rank first, then PB line, timing line ("N ms early/late on average (spread ±M ms)"), early/late counts, weakest pad with rushing/dragging wording, trouble spot with a timestamp and "press P".
- [ ] **Step 2:** Press P → practice loads the same chart, toast shows the reason, loop region brackets the trouble spot at 0.8× tempo with lead-in; loop/ramp/tempo keys all work as before.
- [ ] **Step 3:** Clear the practice loop, exit to song select, confirm `scores.json` gained NO entry from the practice run (only the original play).
- [ ] **Step 4:** Perfect a short chart: no "trouble spot" row, no P hint, P key inert.

---

## Success-check mapping (roadmap)

- "Results identify timing bias and weakest lane or section" → Tasks 1-3.
- "One action starts practice at the recommended section with lead-in" → Task 5.
- "Practice attempts never write normal scores" → inherited gate (`lib.rs:306-309`) + Task 6 Step 3.
- "diagnosis understandable without knowing rhythm-game jargon" → Task 3 formatters (plain words, no "UR"/"unstable rate").
