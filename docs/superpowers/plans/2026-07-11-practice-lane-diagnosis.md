# Practice Per-Lane Diagnosis Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Per-lane accuracy + signed timing bias panel in the paused practice full HUD, accumulated across all attempts on the current loop region.

**Architecture:** A pure `LaneDiagnosis` accumulator lives on `PracticeSession`, fed from the same event loops in `practice/stats.rs` that already gate pre-roll chips — so exclusion rules are inherited, not duplicated. It clears on every loop-region change (the same lifecycle as the span-filtered attempt history). The full HUD renders it as one text block, rows sorted worst-first.

**Tech Stack:** Bevy 0.19; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-11-practice-lane-diagnosis-design.md`

**Build notes (repo conventions):**
- Never run bare `cargo fmt --all`.
- Test command: `cargo test -p gameplay-drums`.
- After system wiring, run guards: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering` and `--test practice_hud`.

**Sign convention** (matches wrap report in `stats.rs`: "`+` = late, `−` = early"): `mean_delta_ms < -BIAS_THRESHOLD_MS` → "rushing", `> +BIAS_THRESHOLD_MS` → "dragging".

---

## File Structure

- Create: `crates/gameplay-drums/src/practice/diagnosis.rs` — `LaneAgg`, `LaneDiagnosis`, row formatting (pure)
- Modify: `crates/gameplay-drums/src/practice/session.rs` — `lane_diag` field + clear on region change
- Modify: `crates/gameplay-drums/src/practice/stats.rs` — feed the accumulator
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` — panel spawn + refresh
- Modify: `crates/gameplay-drums/src/practice/mod.rs` — `pub mod diagnosis;`

---

### Task 1: Pure accumulator

**Files:**
- Create: `crates/gameplay-drums/src/practice/diagnosis.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (add `pub mod diagnosis;`)

- [ ] **Step 1: Create the module with tests**

```rust
//! Per-lane diagnosis: accuracy + signed timing bias per drum lane,
//! accumulated over all attempts on the current loop region.
//! Spec: docs/superpowers/specs/2026-07-11-practice-lane-diagnosis-design.md

use std::collections::HashMap;

use dtx_scoring::JudgmentKind;

use crate::lane_map::{lane_channel, LaneId};

/// Bias smaller than this (ms, absolute) reads as "on time".
pub const BIAS_THRESHOLD_MS: f32 = 10.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LaneAgg {
    /// Judgments counted (pre-roll chips excluded by the caller).
    pub judged: u32,
    pub misses: u32,
    /// EmptyHit (whiff) on this lane.
    pub overhits: u32,
    /// Signed delta sum, hits only (miss delta excluded).
    pub delta_sum_ms: i64,
    pub delta_count: u32,
}

impl LaneAgg {
    pub fn hit_pct(&self) -> f32 {
        if self.judged == 0 {
            0.0
        } else {
            (self.judged - self.misses) as f32 / self.judged as f32 * 100.0
        }
    }

    pub fn mean_delta_ms(&self) -> f32 {
        if self.delta_count == 0 {
            0.0
        } else {
            self.delta_sum_ms as f32 / self.delta_count as f32
        }
    }

    /// "−18ms rushing" / "+14ms dragging" / "on time".
    pub fn bias_label(&self) -> String {
        let mean = self.mean_delta_ms();
        if self.delta_count == 0 {
            "—".into()
        } else if mean < -BIAS_THRESHOLD_MS {
            format!("{mean:+.0}ms rushing")
        } else if mean > BIAS_THRESHOLD_MS {
            format!("{mean:+.0}ms dragging")
        } else {
            "on time".into()
        }
    }
}

/// Per-lane aggregates for the current loop region.
#[derive(Debug, Clone, Default)]
pub struct LaneDiagnosis {
    pub lanes: HashMap<LaneId, LaneAgg>,
}

impl LaneDiagnosis {
    pub fn clear(&mut self) {
        self.lanes.clear();
    }

    pub fn apply_judgment(&mut self, lane: LaneId, kind: JudgmentKind, delta_ms: i64) {
        let agg = self.lanes.entry(lane).or_default();
        agg.judged += 1;
        if kind == JudgmentKind::Miss {
            agg.misses += 1;
        } else {
            agg.delta_sum_ms += delta_ms;
            agg.delta_count += 1;
        }
    }

    pub fn apply_miss(&mut self, lane: LaneId) {
        let agg = self.lanes.entry(lane).or_default();
        agg.judged += 1;
        agg.misses += 1;
    }

    pub fn apply_overhit(&mut self, lane: LaneId) {
        self.lanes.entry(lane).or_default().overhits += 1;
    }

    /// Rows sorted worst-first: lowest hit%, ties by larger |mean delta|.
    pub fn sorted_rows(&self) -> Vec<(LaneId, LaneAgg)> {
        let mut rows: Vec<(LaneId, LaneAgg)> =
            self.lanes.iter().map(|(&l, &a)| (l, a)).collect();
        rows.sort_by(|a, b| {
            a.1.hit_pct()
                .partial_cmp(&b.1.hit_pct())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    b.1.mean_delta_ms()
                        .abs()
                        .partial_cmp(&a.1.mean_delta_ms().abs())
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        rows
    }
}

/// Panel text: one row per lane with data, worst-first.
/// `HH   82%  −18ms rushing   3 miss  1 over`
pub fn diagnosis_text(diag: &LaneDiagnosis) -> String {
    let rows = diag.sorted_rows();
    if rows.is_empty() {
        return "LANES\n(no data yet)".into();
    }
    let mut out = String::from("LANES");
    for (lane, agg) in rows {
        let name = lane_channel(lane)
            .and_then(dtx_layout::channel_short_name)
            .unwrap_or("?");
        out.push_str(&format!(
            "\n{name:<4} {:>3.0}%  {}  {} miss  {} over",
            agg.hit_pct(),
            agg.bias_label(),
            agg.misses,
            agg.overhits
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgments_accumulate_per_lane() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(0, JudgmentKind::Perfect, -20);
        d.apply_judgment(0, JudgmentKind::Great, -16);
        d.apply_judgment(1, JudgmentKind::Perfect, 2);
        let hh = d.lanes[&0];
        assert_eq!(hh.judged, 2);
        assert_eq!(hh.mean_delta_ms(), -18.0);
        assert_eq!(hh.bias_label(), "-18ms rushing");
        assert_eq!(d.lanes[&1].bias_label(), "on time");
    }

    #[test]
    fn miss_counts_without_polluting_bias() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(2, JudgmentKind::Perfect, 4);
        d.apply_miss(2);
        let bd = d.lanes[&2];
        assert_eq!(bd.judged, 2);
        assert_eq!(bd.misses, 1);
        assert_eq!(bd.delta_count, 1, "miss must not enter mean delta");
        assert_eq!(bd.hit_pct(), 50.0);
    }

    #[test]
    fn overhit_tracked_without_touching_judged() {
        let mut d = LaneDiagnosis::default();
        d.apply_overhit(1);
        assert_eq!(d.lanes[&1].overhits, 1);
        assert_eq!(d.lanes[&1].judged, 0);
    }

    #[test]
    fn dragging_label_positive() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(3, JudgmentKind::Good, 14);
        assert_eq!(d.lanes[&3].bias_label(), "+14ms dragging");
    }

    #[test]
    fn sorted_rows_worst_first() {
        let mut d = LaneDiagnosis::default();
        // lane 0: 100%, lane 1: 50%, lane 2: 0%.
        d.apply_judgment(0, JudgmentKind::Perfect, 0);
        d.apply_judgment(1, JudgmentKind::Perfect, 0);
        d.apply_miss(1);
        d.apply_miss(2);
        let order: Vec<u8> = d.sorted_rows().iter().map(|(l, _)| *l).collect();
        assert_eq!(order, vec![2, 1, 0]);
    }

    #[test]
    fn clear_empties_everything() {
        let mut d = LaneDiagnosis::default();
        d.apply_judgment(0, JudgmentKind::Perfect, 0);
        d.clear();
        assert!(d.lanes.is_empty());
        assert!(diagnosis_text(&d).contains("no data"));
    }
}
```

Add `pub mod diagnosis;` to `practice/mod.rs`.

Note: `dtx_layout::channel_short_name` is already used from this crate (`editor/panel.rs:637`), so the dependency exists. If the function signature differs (`Option<&str>` vs `&str`), adapt the `and_then`. Note the test expects `format!("{:+.0}", -18.0)` = `"-18ms"` (Rust renders the sign without a Unicode minus).

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums diagnosis`
Expected: 6 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/diagnosis.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(practice): pure per-lane diagnosis accumulator"
```

---

### Task 2: Session wiring + region-change reset

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (`PracticeSession` struct, `clear_loop`, `set_loop_start`, `set_loop_end`)

- [ ] **Step 1: Write the failing tests**

Append to `session.rs` tests:

```rust
    #[test]
    fn region_change_clears_lane_diag() {
        use dtx_scoring::JudgmentKind;
        let mut s = PracticeSession::default();
        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.set_loop_start(2_000);
        assert!(s.lane_diag.lanes.is_empty(), "Set A must clear diagnosis");

        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.set_loop_end(6_000);
        assert!(s.lane_diag.lanes.is_empty(), "Set B must clear diagnosis");

        s.lane_diag.apply_judgment(0, JudgmentKind::Perfect, 0);
        s.clear_loop();
        assert!(s.lane_diag.lanes.is_empty(), "Clear loop must clear diagnosis");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums region_change_clears_lane_diag`
Expected: FAIL — no field `lane_diag`.

- [ ] **Step 3: Implement**

In `PracticeSession` (session.rs:183-189), add after `pub attempt_history: Vec<AttemptRecord>,`:

```rust
    /// Per-lane diagnosis for the current loop region (full-HUD panel).
    pub lane_diag: super::diagnosis::LaneDiagnosis,
```

Add `self.lane_diag.clear();` as the first line of each of `clear_loop`, `set_loop_start`, and `set_loop_end`.

- [ ] **Step 4: Verify no region mutation bypasses the session methods**

Run: `grep -rn "loop_region = " crates/gameplay-drums/src --include='*.rs' | grep -v session.rs`
Expected: no hits outside `session.rs` (drag-loop and rail rows call `set_loop_start`/`set_loop_end`). If any hit assigns `transport.loop_region` directly, route it through the session methods (or add `lane_diag.clear()` beside it) and note it in the commit message.

- [ ] **Step 5: Run tests + commit**

Run: `cargo test -p gameplay-drums session`
Expected: PASS.

```bash
git add crates/gameplay-drums/src/practice/session.rs
git commit -m "feat(practice): lane diagnosis state resets with loop region"
```

---

### Task 3: Feed from the stats event loops

**Files:**
- Modify: `crates/gameplay-drums/src/practice/stats.rs:77-102` (`track_attempt_stats`)

- [ ] **Step 1: Implement (three one-line insertions; the pre-roll gate is the existing `continue`)**

In `track_attempt_stats`:

1. In the judgments loop, directly after `apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms);`:

```rust
        session.lane_diag.apply_judgment(ev.lane, ev.kind, ev.delta_ms);
```

2. In the missed loop, after `session.current_attempt.combo = 0;`:

```rust
        session.lane_diag.apply_miss(m.lane);
```

3. Replace the empty-hits loop:

```rust
    for eh in empty_hits.read() {
        session.current_attempt.overhits += 1;
        session.lane_diag.apply_overhit(eh.lane);
    }
```

- [ ] **Step 2: Write the pre-roll exclusion guard test**

Append to `stats.rs` tests (this guards the wiring: a judgment on a pre-roll chip must not reach the diagnosis):

```rust
    #[test]
    fn preroll_gate_also_guards_lane_diag() {
        // The pre-roll `continue` sits before BOTH apply_judgment calls;
        // this pins that ordering at the source level.
        let src = include_str!("stats.rs");
        let gate = src.find("continue; // pre-roll chip: audible feedback only").unwrap();
        let diag = src.find("session.lane_diag.apply_judgment").unwrap();
        assert!(gate < diag, "lane_diag feed must come after the pre-roll gate");
    }
```

(Source-order pin instead of a full ECS harness: `track_attempt_stats` needs eight system params to drive directly; the integration path is already covered by `tests/practice_mode.rs` patterns and the pure accumulator has its own tests. If `tests/practice_mode.rs` already has a harness that emits `JudgmentEvent` with a pre-roll chip, prefer extending that instead — check before settling for the source pin.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p gameplay-drums stats`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/practice/stats.rs
git commit -m "feat(practice): feed per-lane diagnosis from attempt stats"
```

---

### Task 4: Full-HUD panel

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs`

- [ ] **Step 1: Implement**

1. Add a marker component near the other markers (full_hud.rs:30-34):

```rust
#[derive(Component)]
pub struct LaneDiagnosisText;
```

2. In `spawn_full_hud` (the `.with_children(|rail| ...)` block that ends with `AttemptHistoryText`, full_hud.rs:302-311), add a sibling block directly after the `AttemptHistoryText` spawn:

```rust
                rail.spawn((
                    LaneDiagnosisText,
                    Text::new(crate::practice::diagnosis::diagnosis_text(&session.lane_diag)),
                    Theme::label_font(),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
```

3. In the rail input/refresh system (the one ending with the `AttemptHistoryText` refresh, full_hud.rs:590-592), add a query param:

```rust
    mut diag_text: Query<
        &mut Text,
        (With<LaneDiagnosisText>, Without<RailItem>, Without<AttemptHistoryText>),
    >,
```

and after the history refresh:

```rust
    if let Ok(mut t) = diag_text.single_mut() {
        t.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
```

If the system is at Bevy's 16-param ceiling, bundle the two text queries into a `ParamSet` or a `#[derive(SystemParam)]` struct (see `seek.rs:110-132` for the house pattern).

- [ ] **Step 2: Run guards**

Run: `cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --test fixed_update_schedule_ordering`
Expected: PASS — real HUD schedule still builds headlessly.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud/full_hud.rs
git commit -m "feat(practice): per-lane diagnosis panel in full HUD"
```

---

### Task 5: Verification

- [ ] **Step 1: Full suite**

Run: `cargo test -p gameplay-drums`
Expected: all green.

- [ ] **Step 2: Manual check (if a display is available)**

Practice a chart, play a few loop passes with deliberate early hi-hat hits, open the full HUD (Tab): panel lists lanes worst-first, hi-hat shows negative bias with "rushing". Set a new A marker: panel resets to "(no data yet)".

- [ ] **Step 3: Spec deviation review note**

The spec says lanes with notes in the region but zero judgments show "—" rows. This plan renders only lanes that have data (simpler; no timeline scan in the UI path). Flag for user review; adding placeholder rows from a `timeline.entries` scan over the active region is a small follow-up if wanted.
