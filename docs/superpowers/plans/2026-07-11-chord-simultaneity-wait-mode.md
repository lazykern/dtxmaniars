# Chord Simultaneity in Wait Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wait mode chords only clear when every note is hit within a 50ms
window of each other; hits outside that window are undone and the player
retries the same chord in place.

**Architecture:** A new `ChordHitTimes` resource (chip_idx → adjusted hit
timestamp) is populated by the existing judge system only for chips inside
the currently-halted `WaitSet`. The existing `wait_watcher` system, once it
sees every chip in the set judged, computes the spread
(`max(times) - min(times)`) and either clears the chord (spread ≤ 50ms) or
un-judges every chip in the set and stays halted (spread > 50ms), pushing a
practice toast either way for feedback on reject.

**Tech Stack:** Rust, Bevy 0.19 ECS (Resources, `FixedUpdate` systems),
existing `crates/gameplay-drums` practice module.

---

## Context for the engineer

- Spec: `docs/superpowers/specs/2026-07-11-chord-simultaneity-wait-mode-design.md`
  — read it, it has the full rationale.
- The wait-mode halt/resume mechanism already exists in
  `crates/gameplay-drums/src/practice/wait.rs`. `WaitSet { target_ms, chips: Vec<usize> }`
  names the chip indices (indices into `ActiveChart.chart.chips`) that must
  all be judged before the halt releases. `JudgedChips(HashSet<usize>)`
  (in `crates/gameplay-drums/src/judge.rs`) is the set of judged chip
  indices; `judge_lane_hit_system` inserts into it.
- A prior fix (already on `main`) filters `judge_lane_hit_system` so that
  while halted, only chips in the current `WaitSet` can be judged at all —
  see `judge.rs`'s `halted_chips` / `filter_to_halted_set`. This plan
  builds on top of that; you do not need to touch that filter.
- `crates/gameplay-drums/src/practice/toast.rs` has `ToastQueue::push(&mut self, impl Into<String>)`
  for one-line practice feedback (already wired into a UI system, you only
  need to push text).
- Build/test this workspace with a pinned `CARGO_TARGET_DIR` to avoid
  filling disk (worktree `target/` dirs have grown to tens of GB before):
  `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums`
- Format only this crate, never the whole workspace:
  `cargo fmt -p gameplay-drums` (bare `cargo fmt --all` reformats unrelated
  files under this repo's local rustfmt version).

---

### Task 1: `ChordHitTimes` resource + pure spread function

**Files:**
- Modify: `crates/gameplay-drums/src/practice/wait.rs`

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in
`crates/gameplay-drums/src/practice/wait.rs` (after the
`is_cleared_requires_every_chip` test, before the closing `}` of the
module):

```rust
    #[test]
    fn chord_spread_accepts_within_window() {
        let times: HashMap<usize, i64> = [(2, 1_000), (3, 1_030)].into();
        assert_eq!(chord_spread(&times, &[2, 3]), Some(30));
        assert!(chord_spread(&times, &[2, 3]).unwrap() <= CHORD_WINDOW_MS);
    }

    #[test]
    fn chord_spread_flags_outside_window() {
        let times: HashMap<usize, i64> = [(2, 1_000), (3, 1_080)].into();
        assert_eq!(chord_spread(&times, &[2, 3]), Some(80));
        assert!(chord_spread(&times, &[2, 3]).unwrap() > CHORD_WINDOW_MS);
    }

    #[test]
    fn chord_spread_uses_full_range_for_three_notes() {
        // earliest 1000, latest 1090 -> spread 90, even though each
        // consecutive pair is only 45ms apart (not pairwise distance).
        let times: HashMap<usize, i64> = [(2, 1_000), (3, 1_045), (4, 1_090)].into();
        assert_eq!(chord_spread(&times, &[2, 3, 4]), Some(90));
    }

    #[test]
    fn chord_spread_none_when_a_chip_not_yet_hit() {
        let times: HashMap<usize, i64> = [(2, 1_000)].into();
        assert_eq!(chord_spread(&times, &[2, 3]), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait:: 2>&1 | tail -30`

Expected: compile error — `chord_spread` and `CHORD_WINDOW_MS` not found.

- [ ] **Step 3: Implement `CHORD_WINDOW_MS` and `chord_spread`**

In `crates/gameplay-drums/src/practice/wait.rs`, add directly after the
`is_cleared` function (which currently ends the pure, non-Bevy section of
the file, right before the `use bevy::prelude::*;` line):

```rust
/// Max acceptable spread between the earliest and latest hit in a chord
/// for it to count as "played together" (spec: 50ms, matches a Perfect
/// judge window).
pub const CHORD_WINDOW_MS: i64 = 50;

/// Spread (`max - min`) across every `chips` entry's recorded hit time.
/// `None` if any chip in `chips` has no recorded hit yet.
pub fn chord_spread(times: &HashMap<usize, i64>, chips: &[usize]) -> Option<i64> {
    let mut min = i64::MAX;
    let mut max = i64::MIN;
    for chip in chips {
        let t = *times.get(chip)?;
        min = min.min(t);
        max = max.max(t);
    }
    Some(max - min)
}
```

- [ ] **Step 4: Add the `ChordHitTimes` resource**

In the same file, directly after the `WaitState` struct's `impl` block
(right after the `halted()` method's closing `}`, before
`/// Run condition for the clock-sync chain...`), add:

```rust
/// Adjusted hit timestamp for every chip judged while wait mode is
/// halted, keyed by chip index. Only ever holds entries for the chord
/// currently being evaluated — cleared on chord-clear, chord-reject, and
/// seek.
#[derive(Resource, Debug, Default)]
pub struct ChordHitTimes(pub HashMap<usize, i64>);
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait:: 2>&1 | tail -30`

Expected: all `practice::wait::` tests PASS, including the 4 new ones.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/practice/wait.rs
git commit -m "feat(practice): add chord-hit-spread calculation for wait mode"
```

---

### Task 2: Record hit timestamps into `ChordHitTimes` from the judge system

**Files:**
- Modify: `crates/gameplay-drums/src/judge.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in
`crates/gameplay-drums/src/judge.rs` (after the existing
`halted_filter_drops_future_note_entirely` test):

```rust
    #[test]
    fn records_hit_time_only_for_halted_chips() {
        use crate::practice::wait::ChordHitTimes;
        let mut chord_hits = ChordHitTimes::default();
        record_chord_hit_times(&mut chord_hits, &[(2, 0_i64), (5, 3)], Some(&[2, 3]), 12_345);
        assert_eq!(chord_hits.0.get(&2), Some(&12_345));
        assert_eq!(chord_hits.0.get(&5), Some(&12_345));
        // not halted -> nothing recorded
        let mut chord_hits2 = ChordHitTimes::default();
        record_chord_hit_times(&mut chord_hits2, &[(2, 0_i64)], None, 12_345);
        assert!(chord_hits2.0.is_empty());
    }
```

Note: this test intentionally records for chip `5` too even though the
earlier `halted_chips` filter already dropped it from `results` in
practice — the helper itself doesn't re-check membership against
`halted_chips`'s chip list, it only checks whether wait mode is halted at
all (`Some(_)` vs `None`). By the time this helper runs, `results` has
already been through `filter_to_halted_set`, so in real use every chip
passed in is guaranteed to be in the halted set. The test documents this
contract directly.

- [ ] **Step 2: Run test to verify it fails to compile**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums judge:: 2>&1 | tail -30`

Expected: compile error — `record_chord_hit_times` not found.

- [ ] **Step 3: Implement `record_chord_hit_times`**

In `crates/gameplay-drums/src/judge.rs`, add directly after
`filter_to_halted_set`:

```rust
/// Record `hit_ms` for every `(chip_idx, _delta)` in `results` into
/// `chord_hits`, but only while wait mode is halted (`halted_chips` is
/// `Some`) — outside a halt there is no chord being evaluated, so there
/// is nothing to record against.
fn record_chord_hit_times(
    chord_hits: &mut crate::practice::wait::ChordHitTimes,
    results: &[(usize, i64)],
    halted_chips: Option<&[usize]>,
    hit_ms: i64,
) {
    if halted_chips.is_none() {
        return;
    }
    for (idx, _delta) in results {
        chord_hits.0.insert(*idx, hit_ms);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums judge:: 2>&1 | tail -30`

Expected: `records_hit_time_only_for_halted_chips` PASSES.

- [ ] **Step 5: Wire it into `judge_lane_hit_system`**

In `crates/gameplay-drums/src/judge.rs`, `judge_lane_hit_system`'s
signature currently ends with:

```rust
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
    mut empty_hits: MessageWriter<EmptyHit>,
) {
```

Add a `ChordHitTimes` resource param right after `wait_state`:

```rust
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
    mut chord_hits: ResMut<crate::practice::wait::ChordHitTimes>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
    mut empty_hits: MessageWriter<EmptyHit>,
) {
```

Then, inside the `for hit in lane_hits.read() { ... }` loop, the current
code reads:

```rust
        let results = filter_to_halted_set(results, halted_chips);

        if results.is_empty() {
```

Insert a call right after the filter, before the emptiness check:

```rust
        let results = filter_to_halted_set(results, halted_chips);
        record_chord_hit_times(&mut chord_hits, &results, halted_chips, adjusted_hit_ms);

        if results.is_empty() {
```

- [ ] **Step 6: Register `ChordHitTimes` as a resource**

`ChordHitTimes` needs to exist before any system touches it. Open
`crates/gameplay-drums/src/practice/wait.rs`, find `pub(crate) fn plugin`:

```rust
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<WaitState>().add_systems(
```

Change to:

```rust
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<WaitState>()
        .init_resource::<ChordHitTimes>()
        .add_systems(
```

(Adjust the trailing `.add_systems(...)` call's indentation to match — it
is now chained off two `init_resource` calls instead of one; `cargo fmt`
in the final task will normalize this regardless.)

- [ ] **Step 7: Run full crate build + tests**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums 2>&1 | tail -20`

Expected: all tests PASS (492+ prior tests plus the new ones), no compile
errors. If `ChordHitTimes` isn't found in scope in `judge.rs`, confirm
`wait.rs`'s struct is `pub` (it is, from Task 1) and the path
`crate::practice::wait::ChordHitTimes` is correct.

- [ ] **Step 8: Commit**

```bash
git add crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/practice/wait.rs
git commit -m "feat(practice): record chord hit timestamps during wait halt"
```

---

### Task 3: Accept/reject logic in `wait_watcher`

**Files:**
- Modify: `crates/gameplay-drums/src/practice/wait.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `wait.rs`, after the tests
added in Task 1:

```rust
    #[test]
    fn wait_watcher_clears_chord_hit_within_window() {
        let mut app = App::new();
        app.add_message::<crate::seek::SeekToChartTime>();
        app.insert_resource(PracticeSession::default());
        app.insert_resource(crate::timeline::ChipTimeline::default());
        let mut judged = JudgedChips::default();
        judged.0.insert(2);
        judged.0.insert(3);
        app.insert_resource(judged);
        app.insert_resource(GameplayClock::default());
        app.insert_resource(BgmHandle::default());
        app.insert_resource(DrumPolyphony::default());
        app.insert_resource(ActiveDrumSounds::default());
        app.init_resource::<Assets<AudioInstance>>();
        app.insert_resource(ChordHitTimes([(2, 1_000), (3, 1_030)].into()));
        app.insert_resource(WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![2, 3],
            }),
            waited_chips: [2, 3].into(),
        });
        app.init_resource::<crate::practice::toast::ToastQueue>();
        app.add_systems(Update, wait_watcher);
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(!state.halted(), "spread within window must clear the halt");
        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "cleared chord's hit times must be dropped"
        );
    }

    #[test]
    fn wait_watcher_rejects_chord_hit_outside_window() {
        let mut app = App::new();
        app.add_message::<crate::seek::SeekToChartTime>();
        app.insert_resource(PracticeSession::default());
        app.insert_resource(crate::timeline::ChipTimeline::default());
        let mut judged = JudgedChips::default();
        judged.0.insert(2);
        judged.0.insert(3);
        app.insert_resource(judged);
        app.insert_resource(GameplayClock::default());
        app.insert_resource(BgmHandle::default());
        app.insert_resource(DrumPolyphony::default());
        app.insert_resource(ActiveDrumSounds::default());
        app.init_resource::<Assets<AudioInstance>>();
        app.insert_resource(ChordHitTimes([(2, 1_000), (3, 1_080)].into()));
        app.insert_resource(WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![2, 3],
            }),
            waited_chips: [2, 3].into(),
        });
        app.init_resource::<crate::practice::toast::ToastQueue>();
        app.add_systems(Update, wait_watcher);
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(state.halted(), "spread outside window must reject, stay halted");
        assert!(
            matches!(&state.phase, WaitPhase::Halted(set) if set.chips == vec![2, 3]),
            "same wait-set stays active for retry"
        );
        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&2) && !judged.0.contains(&3),
            "rejected chips must be un-judged so the player can retry"
        );
        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "rejected chord's hit times must be cleared for the retry"
        );
        assert_eq!(
            app.world().resource::<crate::practice::toast::ToastQueue>().0.len(),
            1,
            "reject should surface one feedback toast"
        );
    }
```

`GameplayClock`, `BgmHandle`, `DrumPolyphony`, `ActiveDrumSounds`,
`ChipTimeline` all implement `Default` (used identically in the existing
`chord_collects_all_unjudged_chips_at_target`-style tests elsewhere in
this crate) — if any of these fails to derive `Default` when you run the
build, check that crate's definition before assuming the test is wrong.

- [ ] **Step 2: Run tests to verify they fail**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait:: 2>&1 | tail -40`

Expected: `wait_watcher_clears_chord_hit_within_window` PASSES already
(current `is_cleared`-only logic happens to clear on any completed set);
`wait_watcher_rejects_chord_hit_outside_window` FAILS — it stays cleared
today instead of rejecting (the bug this task fixes).

- [ ] **Step 3: Implement the accept/reject branch**

In `crates/gameplay-drums/src/practice/wait.rs`, the current
`wait_watcher` function signature is:

```rust
pub fn wait_watcher(
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    judged: Res<JudgedChips>,
    clock: Res<GameplayClock>,
    mut state: ResMut<WaitState>,
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
```

Change `judged: Res<JudgedChips>` to `mut judged: ResMut<JudgedChips>`,
and add two more params:

```rust
pub fn wait_watcher(
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    mut judged: ResMut<JudgedChips>,
    clock: Res<GameplayClock>,
    mut state: ResMut<WaitState>,
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut chord_hits: ResMut<ChordHitTimes>,
    mut toasts: ResMut<super::toast::ToastQueue>,
) {
```

The current `WaitPhase::Halted(set) => { ... }` arm is:

```rust
        WaitPhase::Halted(set) => {
            if is_cleared(&set, &judged.0) {
                crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Flowing;
            }
        }
```

Replace it with:

```rust
        WaitPhase::Halted(set) => {
            if !is_cleared(&set, &judged.0) {
                return;
            }
            match chord_spread(&chord_hits.0, &set.chips) {
                Some(spread) if spread <= CHORD_WINDOW_MS => {
                    for chip in &set.chips {
                        chord_hits.0.remove(chip);
                    }
                    crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                    state.phase = WaitPhase::Flowing;
                }
                _ => {
                    for chip in &set.chips {
                        judged.0.remove(chip);
                        chord_hits.0.remove(chip);
                    }
                    toasts.push("Hit together — retry the chord");
                }
            }
        }
```

Note the `return` inside a non-unit-returning system: `wait_watcher`
returns `()`, so a bare `return` inside the `match` arm is valid Rust —
it just exits the system for this tick, same as the original `if` guard
falling through to nothing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait:: 2>&1 | tail -40`

Expected: both `wait_watcher_clears_chord_hit_within_window` and
`wait_watcher_rejects_chord_hit_outside_window` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/wait.rs
git commit -m "feat(practice): reject and retry chords hit outside the simultaneity window"
```

---

### Task 4: Clear `ChordHitTimes` on seek

**Files:**
- Modify: `crates/gameplay-drums/src/practice/wait.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block, after the Task 3 tests:

```rust
    #[test]
    fn seek_clears_chord_hit_times() {
        let mut app = App::new();
        app.add_message::<crate::seek::SeekToChartTime>();
        app.insert_resource(WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![2],
            }),
            waited_chips: [2].into(),
        });
        app.insert_resource(ChordHitTimes([(2, 1_000)].into()));
        app.add_systems(Update, reset_wait_on_seek);
        app.world_mut().write_message(crate::seek::SeekToChartTime {
            target_ms: 0,
            snap: None,
            attempt_start_ms: None,
        });
        app.update();

        assert!(
            app.world().resource::<ChordHitTimes>().0.is_empty(),
            "seek must drop any in-flight chord hit times"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait::seek_clears_chord_hit_times 2>&1 | tail -30`

Expected: compile error (missing `ChordHitTimes` param in
`reset_wait_on_seek`) or assertion failure (map not cleared).

- [ ] **Step 3: Implement**

The current `reset_wait_on_seek` is:

```rust
pub fn reset_wait_on_seek(mut seeks: MessageReader<SeekToChartTime>, mut state: ResMut<WaitState>) {
    if seeks.read().last().is_some() {
        state.phase = WaitPhase::Flowing;
        state.waited_chips.clear();
    }
}
```

Change to:

```rust
pub fn reset_wait_on_seek(
    mut seeks: MessageReader<SeekToChartTime>,
    mut state: ResMut<WaitState>,
    mut chord_hits: ResMut<ChordHitTimes>,
) {
    if seeks.read().last().is_some() {
        state.phase = WaitPhase::Flowing;
        state.waited_chips.clear();
        chord_hits.0.clear();
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums practice::wait:: 2>&1 | tail -40`

Expected: all `practice::wait::` tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/wait.rs
git commit -m "fix(practice): clear chord hit times on seek during wait mode"
```

---

### Task 5: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Run the full crate test suite**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums 2>&1 | tail -20`

Expected: all tests PASS, no failures, no ignored regressions.

- [ ] **Step 2: Format**

Run: `cargo fmt -p gameplay-drums`

Do NOT run bare `cargo fmt --all` — this repo's local rustfmt version
reformats unrelated files across the workspace when run unscoped.

- [ ] **Step 3: Confirm formatting is stable**

Run: `cargo fmt -p gameplay-drums -- --check`

Expected: no output (clean).

- [ ] **Step 4: Clippy**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo clippy -p gameplay-drums --all-targets 2>&1 | tail -40`

Expected: no warnings/errors ("no issues found" or equivalent clean exit).

- [ ] **Step 5: Re-run tests after fmt (in case fmt touched test code)**

Run: `CARGO_TARGET_DIR=$(git rev-parse --show-toplevel)/target cargo test -p gameplay-drums 2>&1 | tail -20`

Expected: all tests still PASS.

- [ ] **Step 6: Commit any formatting fixups**

If Step 2 changed any files:

```bash
git add -u crates/gameplay-drums
git commit -m "chore(practice): fmt chord-simultaneity changes"
```

If Step 2 changed nothing, skip this commit — there is nothing to commit.

---

## Out of scope (per spec, do not implement)

- Configurable window size (fixed 50ms).
- Visual countdown/timer during a halt.
- Simultaneity checks outside wait mode.
- Stat tracking of desync/reject count in the wrap report.
