# Practice Wait Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wait-at-line trainer: the clock halts at any note that reaches its target unhit, resumes when the correct pad(s) clear it; tempo-free stats (waited count + flow%).

**Architecture:** A pure `check_halt` scans `ChipTimeline` + `JudgedChips` for the earliest pending drum note at/before the clock inside the attempt span. One FixedUpdate system (after Judge) drives the halt/resume transitions: halting freezes the gameplay clock via a run-condition on the clock-sync chain and pauses BGM/voices with the same helpers the pause overlay uses; clearing happens through the NORMAL judge path (the frozen clock sits at the note's target, so correct pads judge as ~0ms hits and enter `JudgedChips`) — stats reclassify those judgments as "waited" instead of counting them. Any seek resets to Flowing.

**Tech Stack:** Bevy 0.19, bevy_kira_audio 0.26; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-11-practice-wait-mode-design.md`

**Build notes (repo conventions):**
- Never run bare `cargo fmt --all`.
- Test command: `cargo test -p gameplay-drums`.
- After ANY system/ordering change run both guards: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering` and `cargo test -p gameplay-drums --test practice_hud`.

**Key mechanism (why no custom hit-matching):** while halted the clock is frozen a frame past the wait target, so a correct pad hit resolves through `judge_lane_hit_system` against exactly the waiting chip (delta ≈ frame overshoot, inside the 117ms window) and lands in `JudgedChips`. Wrong pads produce `EmptyHit` (overhit) as usual. Resume condition is simply "all wait-set chips ∈ `JudgedChips`". Misses cannot fire while halted: `despawn_missed_notes_system` compares against the frozen clock.

**Known caveat (document, don't fix):** autoplay also writes `LaneHit` (memory: LaneHit has three producers), so autoplay would clear wait-sets by itself. Wait mode is for a human drummer; acceptable.

---

## File Structure

- Create: `crates/gameplay-drums/src/practice/wait.rs` — pure core + systems + plugin
- Modify: `crates/gameplay-drums/src/practice/session.rs` — `wait_enabled` on trainer; `waited`/`flow_pct` on attempt types
- Modify: `crates/gameplay-drums/src/practice/stats.rs` — reclassify waited judgments; wrap report variant
- Modify: `crates/gameplay-drums/src/practice/ramp.rs` — arming ramp disables wait
- Modify: `crates/gameplay-drums/src/pause.rs` — extract reusable audio pause/resume helpers
- Modify: `crates/gameplay-drums/src/lib.rs` — clock-sync run condition
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` — rail row
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs` — WAIT chip segment
- Modify: `crates/gameplay-drums/src/practice/mod.rs` — register module/plugin

---

### Task 1: Pure halt detection

**Files:**
- Create: `crates/gameplay-drums/src/practice/wait.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (add `pub mod wait;`)

- [ ] **Step 1: Create the module with pure core + tests**

```rust
//! Wait mode: halt the clock at any unhit note until the correct pads
//! clear it. Spec: docs/superpowers/specs/2026-07-11-practice-wait-mode-design.md

use std::collections::HashSet;

use crate::lane_map::lane_of;
use crate::timeline::ChipTimeline;

/// The chips the halt is waiting on (a chord shares one target time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitSet {
    pub target_ms: i64,
    pub chips: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WaitPhase {
    #[default]
    Flowing,
    Halted(WaitSet),
}

/// Earliest pending (unjudged) drum note at/before `clock_ms` inside the
/// attempt span, plus every unjudged drum chip sharing its target time.
pub fn check_halt(
    timeline: &ChipTimeline,
    judged: &HashSet<usize>,
    clock_ms: i64,
    span_start_ms: i64,
) -> Option<WaitSet> {
    let pending = timeline.entries.iter().find(|e| {
        lane_of(e.channel).is_some()
            && e.judge_ms >= span_start_ms
            && e.judge_ms <= clock_ms
            && !judged.contains(&e.chip_idx)
    })?;
    let target_ms = pending.judge_ms;
    let chips: Vec<usize> = timeline
        .entries
        .iter()
        .filter(|e| {
            e.judge_ms == target_ms
                && lane_of(e.channel).is_some()
                && !judged.contains(&e.chip_idx)
        })
        .map(|e| e.chip_idx)
        .collect();
    Some(WaitSet { target_ms, chips })
}

/// True once every chip in the set has been judged (hit).
pub fn is_cleared(set: &WaitSet, judged: &HashSet<usize>) -> bool {
    set.chips.iter().all(|c| judged.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms. Chips: BD@0, SD@2000, HH@2000 (chord),
    // BGM@0 (non-drum), BD@4000.
    fn timeline() -> ChipTimeline {
        let mut assets = dtx_core::assets::DtxAssets::default();
        assets.wav.insert(1, "bgm.ogg".into());
        let chart = Chart {
            metadata: Metadata { bpm: Some(120.0), ..Default::default() },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),   // 0: not a drum lane
                Chip::new(0, EChannel::BassDrum, 0.0),      // 1: 0ms
                Chip::new(1, EChannel::Snare, 0.0),         // 2: 2000ms
                Chip::new(1, EChannel::HiHatClose, 0.0),    // 3: 2000ms (chord)
                Chip::new(2, EChannel::BassDrum, 0.0),      // 4: 4000ms
            ],
            assets,
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 6_000)
    }

    #[test]
    fn no_halt_while_notes_still_ahead() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        assert_eq!(check_halt(&tl, &judged, 1_500, 0), None);
    }

    #[test]
    fn halts_at_earliest_unhit_note() {
        let tl = timeline();
        let judged = HashSet::new();
        let set = check_halt(&tl, &judged, 5, 0).unwrap();
        assert_eq!(set.target_ms, 0);
        assert_eq!(set.chips, vec![1]);
    }

    #[test]
    fn chord_collects_all_unjudged_chips_at_target() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        let set = check_halt(&tl, &judged, 2_003, 0).unwrap();
        assert_eq!(set.target_ms, 2_000);
        assert_eq!(set.chips.len(), 2);
        assert!(set.chips.contains(&2) && set.chips.contains(&3));
    }

    #[test]
    fn partially_cleared_chord_keeps_only_pending_chips() {
        let tl = timeline();
        let judged: HashSet<usize> = [1, 2].into();
        let set = check_halt(&tl, &judged, 2_003, 0).unwrap();
        assert_eq!(set.chips, vec![3]);
    }

    #[test]
    fn preroll_notes_never_halt() {
        let tl = timeline();
        let judged = HashSet::new();
        // Span starts at 2000; BD@0 is pre-roll and must not halt.
        let set = check_halt(&tl, &judged, 2_001, 2_000).unwrap();
        assert_eq!(set.target_ms, 2_000, "halt on span note, not pre-roll note");
    }

    #[test]
    fn non_drum_channels_ignored() {
        let tl = timeline();
        let judged: HashSet<usize> = [1].into();
        // Only BGM@0 is "pending" before 1500 — no halt.
        assert_eq!(check_halt(&tl, &judged, 1_500, 0), None);
    }

    #[test]
    fn is_cleared_requires_every_chip() {
        let set = WaitSet { target_ms: 2_000, chips: vec![2, 3] };
        assert!(!is_cleared(&set, &[2].into()));
        assert!(is_cleared(&set, &[2, 3].into()));
    }
}
```

Add `pub mod wait;` to `practice/mod.rs`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums wait`
Expected: 7 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/wait.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(practice): pure wait-mode halt detection"
```

---

### Task 2: Session state — toggle, waited count, flow%

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs` (AttemptRecord literals in tests)

- [ ] **Step 1: Write the failing tests**

Append to `session.rs` tests:

```rust
    #[test]
    fn wait_defaults_off_and_flow_pct_computes() {
        let s = PracticeSession::default();
        assert!(!s.trainer.wait_enabled);

        let mut a = AttemptStats::default();
        a.counts.perfect = 3;
        a.waited = 1;
        assert_eq!(a.flow_pct(), 75.0);

        let empty = AttemptStats::default();
        assert_eq!(empty.flow_pct(), 0.0);
    }

    #[test]
    fn roll_attempt_carries_waited_and_flow() {
        let mut s = PracticeSession::default();
        s.current_attempt.counts.perfect = 3;
        s.current_attempt.waited = 1;
        let rec = s.roll_attempt(4_000, 0).unwrap();
        assert_eq!(rec.waited, 1);
        assert_eq!(rec.flow_pct, 75.0);
        assert_eq!(s.current_attempt.waited, 0, "fresh attempt starts clean");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums flow_pct`
Expected: FAIL — no field `wait_enabled` / `waited`.

- [ ] **Step 3: Implement**

1. `PracticeTrainer` (session.rs:175-179): add

```rust
    /// Wait mode: halt at unhit notes (mutually exclusive with the ramp).
    pub wait_enabled: bool,
```

(`#[derive(Default)]` covers `false`.)

2. `AttemptStats` (session.rs:105-116): add field `pub waited: u32,` and method:

```rust
    /// Notes cleared without halting / all notes seen (%). The wait-mode
    /// analogue of achievement%.
    pub fn flow_pct(&self) -> f32 {
        let total = self.counts.total() + self.waited;
        if total == 0 {
            0.0
        } else {
            self.counts.total() as f32 / total as f32 * 100.0
        }
    }
```

Also update `has_data` so wait-only passes still finalize:

```rust
    pub fn has_data(&self) -> bool {
        self.counts.total() > 0 || self.waited > 0
    }
```

3. `AttemptRecord` (session.rs:136-146): add `pub waited: u32,` and `pub flow_pct: f32,`. In `roll_attempt`, set `waited: a.waited, flow_pct: a.flow_pct(),` in the record literal.

4. Fix every `AttemptRecord { ... }` literal that now misses fields — known sites: `chip.rs` tests (two literals, session.rs test helpers if any). Run the compiler to find the rest: `cargo test -p gameplay-drums --no-run` and add `waited: 0, flow_pct: 0.0,`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/session.rs crates/gameplay-drums/src/practice/hud/chip.rs
git commit -m "feat(practice): wait toggle + waited/flow%% attempt fields"
```

---

### Task 3: Reusable audio pause helpers

**Files:**
- Modify: `crates/gameplay-drums/src/pause.rs:92-116`

- [ ] **Step 1: Extract the instance-level bodies (pure refactor)**

Replace `pause_chart_audio` / `resume_chart_audio` with thin wrappers over new `pub(crate)` helpers:

```rust
pub(crate) fn pause_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::pause_audio_instance(instances, handle);
    }
    dtx_audio::pause_polyphony(instances, polyphony);
    active.pause_all(instances);
}

pub(crate) fn resume_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::resume_audio_instance(instances, handle);
    }
    dtx_audio::resume_polyphony(instances, polyphony);
    active.resume_all(instances);
}

fn pause_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn resume_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}
```

- [ ] **Step 2: Run tests + commit**

Run: `cargo test -p gameplay-drums`
Expected: PASS (behavior-preserving refactor).

```bash
git add crates/gameplay-drums/src/pause.rs
git commit -m "refactor(pause): extract reusable chart-audio pause helpers"
```

---

### Task 4: Halt/resume systems + clock gating

**Files:**
- Modify: `crates/gameplay-drums/src/practice/wait.rs`
- Modify: `crates/gameplay-drums/src/lib.rs:149-158` (clock-sync chain)
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (register plugin)

- [ ] **Step 1: Implement the runtime state + systems**

Add to `wait.rs`:

```rust
use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use game_shell::{AppState, PauseState};

use super::session::PracticeSession;
use crate::judge::JudgedChips;
use crate::resources::{ActiveDrumSounds, GameplayClock};
use crate::seek::SeekToChartTime;

/// Runtime wait state. `waited_chips` accumulates the chips of every
/// halt this attempt; stats reclassify their judgments as "waited".
#[derive(Resource, Debug, Default)]
pub struct WaitState {
    pub phase: WaitPhase,
    pub waited_chips: std::collections::HashSet<usize>,
}

impl WaitState {
    pub fn halted(&self) -> bool {
        matches!(self.phase, WaitPhase::Halted(_))
    }
}

/// Run condition for the clock-sync chain: tick only while not halted.
pub fn wait_flowing(state: Option<Res<WaitState>>) -> bool {
    state.is_none_or(|s| !s.halted())
}

/// Drive halt/resume. Runs after Judge so this tick's hits are already
/// in `JudgedChips` (no one-tick spurious halts on exact hits).
#[allow(clippy::too_many_arguments)]
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
    if !clock.is_ready() {
        return;
    }
    if !session.trainer.wait_enabled {
        if state.halted() {
            crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
            state.phase = WaitPhase::Flowing;
        }
        return;
    }
    // Clone the phase before matching: matching on `&state.phase` would
    // hold a borrow of `state` across the arm bodies (E0502).
    match state.phase.clone() {
        WaitPhase::Flowing => {
            let span_start = session.current_attempt.start_ms;
            if let Some(set) = check_halt(&timeline, &judged.0, clock.current_ms, span_start) {
                state.waited_chips.extend(set.chips.iter().copied());
                crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Halted(set);
            }
        }
        WaitPhase::Halted(set) => {
            if is_cleared(&set, &judged.0) {
                crate::pause::resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                state.phase = WaitPhase::Flowing;
            }
        }
    }
}

/// Any seek resets to Flowing (audio ownership passes back to the seek
/// engine, which stops/restarts instances itself) and starts a fresh
/// waited-chip set for the new attempt.
pub fn reset_wait_on_seek(
    mut seeks: MessageReader<SeekToChartTime>,
    mut state: ResMut<WaitState>,
) {
    if seeks.read().last().is_some() {
        state.phase = WaitPhase::Flowing;
        state.waited_chips.clear();
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<WaitState>().add_systems(
        FixedUpdate,
        (
            reset_wait_on_seek.after(crate::seek::apply_seek_system),
            wait_watcher.after(crate::judge::judge_lane_hit_system),
        )
            .chain()
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}
```

Note: `crate::pause` helper visibility — Task 3 made them `pub(crate)`. `PauseOverlay`'s module is `pause` (private mod in lib.rs) — if `crate::pause::` paths fail, adjust the `mod pause;` declaration in lib.rs to `pub(crate) mod pause;`.

Register in `practice/mod.rs` plugin list: add `wait::plugin,`.

- [ ] **Step 2: Gate the clock-sync chain**

In `lib.rs` (the FixedUpdate block at ~149-158 with the "Freeze the gameplay clock while paused" comment), add one more run condition to that chain:

```rust
            // Freeze the gameplay clock while paused or wait-halted.
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(practice::wait::wait_flowing),
```

Do NOT gate `apply_seek_system` or `start_pending_bgm` — a restart during a halt must still seek (and `reset_wait_on_seek` unfreezes).

Also gate `loop_watcher` implicitly: it reads the frozen clock, so no change needed.

- [ ] **Step 3: Run guards**

Run: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering && cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums wait`
Expected: PASS. The ordering guard proves the real FixedUpdate schedule still builds with the new run condition and systems.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/practice/wait.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(practice): wait-mode halt/resume drives clock freeze"
```

---

### Task 5: Stats reclassification + wrap report

**Files:**
- Modify: `crates/gameplay-drums/src/practice/stats.rs`

- [ ] **Step 1: Reclassify waited judgments**

In `track_attempt_stats`, add a param:

```rust
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
```

In the judgments loop, after the pre-roll `continue` and BEFORE `apply_judgment`:

```rust
        if wait_state
            .as_ref()
            .is_some_and(|w| w.waited_chips.contains(&ev.chip_idx))
        {
            session.current_attempt.waited += 1;
            continue; // cleared while halted: tempo-free, not timing-judged
        }
```

(If Task "lane diagnosis" is already merged, the `continue` also correctly skips `lane_diag` — waited notes carry no timing signal.)

- [ ] **Step 2: Wait-mode wrap report**

In `wrap_micro_report`, add `session: Res<PracticeSession>` is already a param; branch the toast:

```rust
    if session.trainer.wait_enabled {
        toasts.push(format!(
            "pass {n} · flow {:.0}% · {} waited",
            att.flow_pct, att.waited
        ));
    } else {
        toasts.push(format!(
            "pass {n} · {:.1}% · {} miss · {:+.0}ms",
            att.accuracy_pct, att.counts.miss, att.mean_error_ms
        ));
    }
```

- [ ] **Step 3: Write the source-order guard test**

Append to `stats.rs` tests:

```rust
    #[test]
    fn waited_reclassification_precedes_apply_judgment() {
        let src = include_str!("stats.rs");
        let waited = src.find("session.current_attempt.waited += 1").unwrap();
        let apply = src
            .find("apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms)")
            .unwrap();
        assert!(waited < apply, "waited check must gate apply_judgment");
    }
```

(Same rationale as the lane-diagnosis plan: the system needs many params to harness directly; the pure pieces are tested, this pins the gating order. If `tests/practice_mode.rs` has a reusable ECS harness that can emit judgments with a `WaitState` present, prefer a real integration test there.)

- [ ] **Step 4: Run tests + guards**

Run: `cargo test -p gameplay-drums stats && cargo test -p gameplay-drums --test fixed_update_schedule_ordering`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/stats.rs
git commit -m "feat(practice): waited notes bypass timing stats; flow%% wrap report"
```

---

### Task 6: Mutual exclusion with the ramp

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ramp.rs:88-130` (`handle_toggle_ramp`)
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` (wait toggle lives in the rail — Task 7 — but exclusion logic is here)

- [ ] **Step 1: Arming the ramp disables wait**

In `handle_toggle_ramp`, in the arm branch (after `session.trainer.ramp = RampState { armed: true, ... };`):

```rust
        if session.trainer.wait_enabled {
            session.trainer.wait_enabled = false;
            toasts.push("wait off (ramp armed)");
        }
```

- [ ] **Step 2: Write the failing test first if `ramp.rs` has a testable pure path** — `handle_toggle_ramp` is a system; the exclusion in the other direction (rail toggle) is pure and tested in Task 7. For this direction add a source-order pin OR extend an existing ramp test harness if one exists in `ramp.rs` tests (check the file; it has pure `ramp_step` tests). If no harness: accept the one-liner with the Task 7 test covering the invariant from the session side.

- [ ] **Step 3: Run + commit**

Run: `cargo test -p gameplay-drums ramp`
Expected: PASS.

```bash
git add crates/gameplay-drums/src/practice/ramp.rs
git commit -m "feat(practice): arming ramp disables wait mode"
```

---

### Task 7: Rail row + status chip

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs`

- [ ] **Step 1: Write the failing tests**

`full_hud.rs` tests:

```rust
    #[test]
    fn wait_rail_label_reflects_toggle() {
        let mut s = crate::practice::session::PracticeSession::default();
        assert_eq!(rail_label(RailItem::WaitMode, &s, false), "Wait  off  (Enter: on)");
        s.trainer.wait_enabled = true;
        assert_eq!(rail_label(RailItem::WaitMode, &s, false), "Wait  ON");
    }
```

`chip.rs` tests:

```rust
    #[test]
    fn chip_text_shows_wait_and_flow() {
        let mut s = PracticeSession::default();
        s.trainer.wait_enabled = true;
        s.attempt_history.push(AttemptRecord {
            start_ms: 0,
            end_ms: 4_000,
            tempo: 1.0,
            counts: Default::default(),
            max_combo: 0,
            overhits: 0,
            accuracy_pct: 0.0,
            mean_error_ms: 0.0,
            waited: 2,
            flow_pct: 60.0,
        });
        let bar_ms = vec![0, 2_000];
        let text = chip_text(&s, &bar_ms);
        assert!(text.contains("WAIT"), "{text}");
        assert!(text.contains("flow 60%"), "{text}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums wait_rail_label chip_text_shows_wait`
Expected: FAIL — no `WaitMode` variant / no WAIT segment.

- [ ] **Step 3: Implement**

1. `RailItem`: add `WaitMode,` after `RampStreak,`. Insert `RailItem::WaitMode,` in `ORDER` after `RailItem::RampStreak,`; bump the length (16→17 — or 17→18 if the metronome plan landed first; use the compiler).
2. Header indices: `WaitMode` sits inside TRAINER (after RampStreak, before ExitPractice) — the `0/6/9` (or metronome-shifted `0/7/10`) header match indices are unchanged by an insert AFTER them; verify by counting `ORDER`.
3. `rail_label`:

```rust
        RailItem::WaitMode => {
            if session.trainer.wait_enabled {
                "Wait  ON".into()
            } else {
                "Wait  off  (Enter: on)".into()
            }
        }
```

4. Enter handler:

```rust
            RailItem::WaitMode => {
                session.trainer.wait_enabled = !session.trainer.wait_enabled;
                if session.trainer.wait_enabled && session.trainer.ramp.armed {
                    session.trainer.ramp.armed = false;
                }
            }
```

5. `chip.rs` `chip_text`: after the ramp segment block, add:

```rust
    if session.trainer.wait_enabled {
        parts.push("WAIT".into());
    }
```

and change the last-attempt segment to show flow when wait is on:

```rust
    if let Some(last) = /* existing span-filtered next_back() lookup */ {
        if session.trainer.wait_enabled {
            parts.push(format!("flow {:.0}%", last.flow_pct));
        } else {
            parts.push(format!("{:.0}%", last.accuracy_pct));
        }
    }
```

- [ ] **Step 4: Run tests + guards**

Run: `cargo test -p gameplay-drums && cargo test -p gameplay-drums --test practice_hud`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud/full_hud.rs crates/gameplay-drums/src/practice/hud/chip.rs
git commit -m "feat(practice): wait-mode rail toggle + WAIT/flow%% status chip"
```

---

### Task 8: Verification

- [ ] **Step 1: Full suite + both guards**

Run: `cargo test -p gameplay-drums`
Expected: all green.

- [ ] **Step 2: Manual check (if a display is available)**

Practice a chart, enable Wait in the rail (Tab), resume. Stop playing: chart freezes at the next note, BGM pauses, note sits at the judge line (this frozen note IS the "what do I hit" indicator — spec's lane highlight, implemented by the freeze itself; flag for user review if it reads unclear). Hit the correct pad: keysound fires, chart resumes. Chord: needs all pads. Wrong pad: nothing advances. R (restart): unfreezes and seeks. Wrap toast reads `pass n · flow X% · Y waited`. Arm ramp: "wait off (ramp armed)" toast.

- [ ] **Step 3: Spec deviation review note**

The spec's "pending pads highlighted via lane-flash visuals" is implemented as "the frozen unhit note at the judge line" (no extra widget — YAGNI). Confirm with the user during review; a pulse tint on the pending lanes is a small follow-up if wanted.
