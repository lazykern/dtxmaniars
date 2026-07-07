# Practice v3: Training Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn practice mode from "a run with helpers" into "a room": no song end, no performance scoring, single-owner tempo/ramp state, correct stat attribution, and loop-boundary feedback.

**Architecture:** Practice keeps its resource-overlay design (`PracticeSession` present = practice). This plan (a) fixes three verified stat-attribution bugs, (b) splits `PracticeSession` into `transport`/`trainer` sub-structs with layered tempo ownership (`user_tempo` vs ramp `step_tempo`, `effective_tempo()` feeding all consumers), (c) rewrites the ramp protocol around a new `PracticeLoopCompleted` message with pass-at-target mastery, (d) makes practice loop the whole song implicitly so the stage never ends, and (e) ships the UX layer (wrap micro-report toast, span-filtered history, rail regroup, widget defaults, visible practice entry).

**Tech Stack:** Rust, Bevy 0.19 (`Message`/`MessageReader`/`MessageWriter`), bevy_kira_audio, serde/toml (dtx-layout).

**Spec:** `docs/superpowers/specs/2026-07-07-practice-v3-training-model-design.md`

**Worktree:** `/home/lazykern/lab/dtxmaniars-practice-v3`, branch `feat/practice-v3-training-model`. All paths below relative to the worktree root.

**Commands:** Test with `cargo test -p gameplay-drums` (and `-p dtx-layout` where noted). Format with `cargo fmt -p gameplay-drums` / `cargo fmt -p dtx-layout` ONLY — never `cargo fmt --all` (local rustfmt version drift reformats unrelated files).

**Bevy 0.19 traps for implementers:**
- Events are `Message`; use `#[derive(Message)]`, `app.add_message::<T>()`, `MessageReader`/`MessageWriter`, and `Messages<T>` for manual writes in tests.
- UI nodes carry `bevy::ui::UiGlobalTransform`, NOT `GlobalTransform`. A `&GlobalTransform` query on a UI node silently matches nothing (no panic, green tests).
- Integration tests wire systems manually (see `tests/practice_mode.rs::build_app`); they do NOT build the real plugin. The real FixedUpdate schedule is guarded by `tests/fixed_update_schedule_ordering.rs` — keep it in sync when system ordering changes.

---

## File structure

| File | Role after this plan |
|---|---|
| `crates/gameplay-drums/src/events.rs` | `NoteMissed` gains `chip_idx` |
| `crates/gameplay-drums/src/scroll.rs` | miss emitter passes `chip_idx` |
| `crates/gameplay-drums/src/practice/session.rs` | split: `PracticeTransport`, `PracticeTrainer`, new `RampConfig`/`RampState`, `effective_tempo()` |
| `crates/gameplay-drums/src/practice/ramp.rs` | protocol v2: pure `ramp_step`, `handle_toggle_ramp`, `apply_ramp` reading `PracticeLoopCompleted` |
| `crates/gameplay-drums/src/practice/ab_loop.rs` | implicit whole-song region + emits `PracticeLoopCompleted` |
| `crates/gameplay-drums/src/practice/stats.rs` | overhits, chip-idx miss exclusion, `LastFinalizedAttempt`, wrap micro-report |
| `crates/gameplay-drums/src/practice/actions.rs` | `TempoDown`/`TempoUp` (renamed), nudge-disarms, loop-change-disarms |
| `crates/gameplay-drums/src/practice/hud/full_hud.rs` | rail regroup, Tempo labels, streak row, span-filtered history |
| `crates/gameplay-drums/src/practice/hud/chip.rs` | effective tempo + span-filtered last accuracy |
| `crates/gameplay-drums/src/practice/hud/timeline_ui.rs` | snapped attempt start, drag-disarms-ramp |
| `crates/gameplay-drums/src/orchestrator.rs` | `detect_end_of_stage` skips practice entirely |
| `crates/dtx-layout/src/widgets.rs` | `PracticeTransport` variant removed |
| `crates/dtx-layout/src/scene.rs` | new visibility defaults, unknown-kind-tolerant TOML |
| `crates/game-menu/src/song_select.rs` | `SHIFT+ENTER PRACTICE` hint |
| `crates/gameplay-drums/tests/*` | updated + new coverage |

Task order: bug fixes (1–3) → state split (4) → ramp core (5) → tempo ownership (6) → loop-completed + implicit loop (7) → no-song-end (8) → micro-report (9) → span filter (10) → rail UX (11) → widget registry (12) → song select (13) → schedule guard + final sweep (14).

---

### Task 1: `NoteMissed.chip_idx` + pre-roll miss exclusion

**Files:**
- Modify: `crates/gameplay-drums/src/events.rs:28-32` (struct), `:66-74` (test)
- Modify: `crates/gameplay-drums/src/scroll.rs:245-248`
- Modify: `crates/gameplay-drums/src/practice/stats.rs:79-84`
- Modify: `crates/gameplay-drums/tests/practice_mode.rs:424-431` (existing `NoteMissed` construction)
- Test: `crates/gameplay-drums/tests/practice_mode.rs` (new test)

- [ ] **Step 1: Write the failing test** — append to `tests/practice_mode.rs`:

```rust
#[test]
fn pre_roll_miss_is_excluded_from_attempt() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    // Attempt starts at 4000ms; chip 1 (at 2000ms) is pre-roll.
    let mut s = PracticeSession::default();
    s.current_attempt.start_ms = 4_000;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(4_500));
    }
    app.world_mut()
        .resource_mut::<Messages<NoteMissed>>()
        .write(NoteMissed {
            lane: 3,
            audio_ms: 2_300,
            chip_idx: 1, // judge_ms 2000 < attempt start 4000 → pre-roll
        });
    app.world_mut()
        .resource_mut::<Messages<NoteMissed>>()
        .write(NoteMissed {
            lane: 3,
            audio_ms: 4_300,
            chip_idx: 2, // judge_ms 4000 >= 4000 → counts
        });
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(
        session.current_attempt.counts.miss, 1,
        "pre-roll miss must not count against the attempt"
    );
}
```

Also fix the existing `finish_loop_pass` helper (`tests/practice_mode.rs:424-431`) to include the new field — its miss is inside the loop, chip 1 sits at 2000ms == attempt start:

```rust
    if perfect_hits == 0 {
        app.world_mut()
            .resource_mut::<Messages<NoteMissed>>()
            .write(NoteMissed {
                lane: 3,
                audio_ms: 5_000,
                chip_idx: 2, // 4000ms, inside the 2000–6000 loop
            });
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode pre_roll_miss_is_excluded 2>&1 | tail -20`
Expected: COMPILE FAIL — `NoteMissed` has no field `chip_idx`.

- [ ] **Step 3: Add the field** — `events.rs`:

```rust
/// A chip that scrolled past the judgment line without being hit.
#[derive(Message, Debug, Clone, Copy)]
pub struct NoteMissed {
    pub lane: LaneId,
    pub audio_ms: i64,
    /// Index into `ActiveChart.chart.chips` for the missed chip.
    pub chip_idx: usize,
}
```

Update the emitter in `scroll.rs` (inside `despawn_missed_notes_system`):

```rust
                missed.write(NoteMissed {
                    lane: note.lane,
                    audio_ms: now,
                    chip_idx: note.chip_id,
                });
```

Update the construct test in `events.rs`:

```rust
    #[test]
    fn note_missed_construct() {
        let m = NoteMissed {
            lane: 1,
            audio_ms: 99999,
            chip_idx: 7,
        };
        assert_eq!(m.lane, 1);
        assert_eq!(m.chip_idx, 7);
    }
```

`score.rs` only counts `missed.read()` messages — adding a field compiles untouched. If any other constructor breaks, add the field there with the note's chip id.

- [ ] **Step 4: Exclude pre-roll misses** — `stats.rs`, replace the miss loop (`:79-84`):

```rust
    for m in missed.read() {
        let judge_ms = timeline
            .judge_ms_by_idx
            .get(m.chip_idx)
            .copied()
            .unwrap_or(i64::MIN);
        if judge_ms < session.current_attempt.start_ms {
            continue; // pre-roll chip: audible feedback only
        }
        session.current_attempt.counts.miss += 1;
        session.current_attempt.combo = 0;
    }
```

Delete the now-stale comment about `NoteMissed` carrying no chip index.

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass (312+).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "fix(practice): NoteMissed carries chip_idx; pre-roll misses excluded from attempts"
```

---

### Task 2: Overhit tracking

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (`AttemptStats`, `AttemptRecord`, `roll_attempt`)
- Modify: `crates/gameplay-drums/src/practice/stats.rs` (`track_attempt_stats` reads `EmptyHit`)
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs:103-111` (test record construction)
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

- [ ] **Step 1: Write the failing test** — append to `tests/practice_mode.rs` (add `EmptyHit` to the existing `use gameplay_drums::events::...` line and `.add_message::<gameplay_drums::events::EmptyHit>()` inside `add_ramp_wiring`):

```rust
#[test]
fn empty_hits_accumulate_as_overhits() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(1_000));
    }
    app.world_mut()
        .resource_mut::<Messages<gameplay_drums::events::EmptyHit>>()
        .write(gameplay_drums::events::EmptyHit { lane: 3, audio_ms: 1_000 });
    app.world_mut()
        .resource_mut::<Messages<gameplay_drums::events::EmptyHit>>()
        .write(gameplay_drums::events::EmptyHit { lane: 4, audio_ms: 1_100 });
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(session.current_attempt.overhits, 2);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode empty_hits_accumulate 2>&1 | tail -10`
Expected: COMPILE FAIL — no field `overhits`.

- [ ] **Step 3: Implement.** `session.rs` — add `pub overhits: u32,` to `AttemptStats` (after `max_combo`) and to `AttemptRecord` (after `max_combo`); in `roll_attempt` copy it: `overhits: a.overhits,`. Overhits do NOT touch accuracy — `accuracy_pct()` stays `counts.achievement_pct()`.

`stats.rs` — add param and loop in `track_attempt_stats` (after the missed loop):

```rust
    mut empty_hits: MessageReader<crate::events::EmptyHit>,
```

```rust
    for _ in empty_hits.read() {
        session.current_attempt.overhits += 1;
    }
```

Fix `chip.rs` test record construction (`AttemptRecord` literal gains `overhits: 0,`). Fix any other `AttemptRecord`/struct-literal build errors the same way.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(practice): track overhits (EmptyHit) per attempt"
```

---

### Task 3: Snapped click-seek attempt start

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/timeline_ui.rs:167-177`
- Test: inline behavioral note (headless mouse tests not practical; covered by code + review)

- [ ] **Step 1: Fix the seek emission.** In `timeline_mouse`, `GestureEffect::Seek` arm — the attempt must start at the *snapped* position (what the seek actually resolves to), not the raw click ms:

```rust
        GestureEffect::Seek { target_ms } => {
            let snapped = timeline.resolve_snap(target_ms, session.snap);
            session.scrub_cursor_ms = Some(snapped);
            seeks.write(SeekToChartTime {
                target_ms,
                snap: Some(session.snap),
                attempt_start_ms: Some(snapped),
            });
        }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "fix(practice): click-seek attempt starts at the snapped position"
```

---

### Task 4: Session state split (pure refactor)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (struct split)
- Modify (mechanical field-path updates): `practice/ab_loop.rs`, `practice/actions.rs`, `practice/ramp.rs`, `practice/rate.rs`, `practice/stats.rs`, `practice/hud/full_hud.rs`, `practice/hud/chip.rs`, `practice/hud/mini_strip.rs`, `practice/hud/timeline_ui.rs`, `src/orchestrator.rs` (`loop_armed` call is a method — unchanged), `tests/practice_mode.rs`, `tests/practice_hud.rs`
- Test: existing suite is the spec — NO behavior change in this task

This is a mechanical move. Ramp semantics change in Tasks 5–7; here only the shape moves. `rate` is renamed `user_tempo` now (one rename pass, not two).

- [ ] **Step 1: New session shape.** Replace the flat `PracticeSession` in `session.rs` with:

```rust
/// Transport state: what/where/how-fast the player chose. Only user
/// input mutates this.
#[derive(Debug, Clone)]
pub struct PracticeTransport {
    /// The player's chosen tempo. The ramp never writes this except on
    /// completion (graduation).
    pub user_tempo: f32,
    pub snap: SnapDivisor,
    pub preroll: PrerollSetting,
    pub loop_region: Option<LoopRegion>,
    /// Scrub cursor while paused (chart ms). None = cursor at playhead.
    pub scrub_cursor_ms: Option<i64>,
}

impl Default for PracticeTransport {
    fn default() -> Self {
        Self {
            user_tempo: 1.0,
            snap: SnapDivisor::Bar,
            preroll: PrerollSetting::OneBar,
            loop_region: None,
            scrub_cursor_ms: None,
        }
    }
}

/// Trainer state: the accuracy-gated ramp (future trainers live here).
#[derive(Debug, Clone, Default)]
pub struct PracticeTrainer {
    pub ramp_config: RampConfig,
    pub ramp: RampState,
}

/// Present only while the stage runs in practice mode. Absence = normal
/// play with zero behavior change.
#[derive(Resource, Debug, Clone, Default)]
pub struct PracticeSession {
    pub transport: PracticeTransport,
    pub trainer: PracticeTrainer,
    pub current_attempt: AttemptStats,
    pub attempt_history: Vec<AttemptRecord>,
}
```

Keep `RampConfig`/`RampState` byte-identical for now (fields move in Task 5). Delete the old flat `Default for PracticeSession`.

- [ ] **Step 2: Move the methods.** `step_rate` becomes `step_user_tempo` on `PracticeSession` (same quantized math against `self.transport.user_tempo`); `set_loop_start`/`set_loop_end`/`loop_armed` operate on `self.transport.loop_region`; `roll_attempt` reads `rate: self.transport.user_tempo` for the record (switches to `effective_tempo()` in Task 6). In-file tests: update field paths (`s.transport.user_tempo`, `s.transport.loop_region`, …).

- [ ] **Step 3: Sweep consumers.** Mechanical replacements across the files listed above:

| old | new |
|---|---|
| `session.rate` | `session.transport.user_tempo` |
| `session.step_rate(` | `session.step_user_tempo(` |
| `session.loop_region` | `session.transport.loop_region` |
| `session.snap` | `session.transport.snap` |
| `session.preroll` | `session.transport.preroll` |
| `session.scrub_cursor_ms` | `session.transport.scrub_cursor_ms` |
| `session.ramp_config` | `session.trainer.ramp_config` |
| `session.ramp` | `session.trainer.ramp` |

Tests constructing `PracticeSession { loop_region: …, rate, preroll, .. }` become:

```rust
    PracticeSession {
        transport: gameplay_drums::practice::session::PracticeTransport {
            loop_region: Some(LoopRegion { start_ms: 2_000, end_ms: 6_000 }),
            preroll: gameplay_drums::practice::session::PrerollSetting::Off,
            user_tempo: rate,
            ..Default::default()
        },
        ..Default::default()
    }
```

(`looped_session` in `tests/practice_mode.rs` builds this then sets `s.trainer.ramp.armed = true; s.trainer.ramp.current_rate = rate; s.current_attempt.start_ms = 2_000;`.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass — identical behavior, new paths.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor(practice): split PracticeSession into transport/trainer; rate -> user_tempo"
```

---

### Task 5: Ramp protocol v2 (pure core)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (`RampConfig`, `RampState`)
- Modify: `crates/gameplay-drums/src/practice/ramp.rs` (`ramp_step`, `ramp_step_index`, unit tests; minimal compile fixes in the two systems)
- Modify: `crates/gameplay-drums/tests/practice_mode.rs` (compile fixes only; behavior retested in Task 7)

- [ ] **Step 1: Write the failing unit tests** — replace the `#[cfg(test)]` module in `ramp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RampConfig {
        RampConfig::default() // 0.70 → 1.00, step 0.05, threshold 90%, streak 1
    }

    fn state(tempo: f32) -> RampState {
        RampState {
            armed: true,
            step_tempo: tempo,
            success_streak: 0,
            fail_streak: 0,
        }
    }

    #[test]
    fn clean_pass_steps_up() {
        let mut s = state(0.70);
        assert_eq!(ramp_step(&cfg(), &mut s, 95.0), RampDecision::StepUp { new_tempo: 0.75 });
        assert!((s.step_tempo - 0.75).abs() < 1e-6);
    }

    #[test]
    fn first_fail_holds_second_steps_down() {
        let mut s = state(0.80);
        assert_eq!(ramp_step(&cfg(), &mut s, 60.0), RampDecision::Hold);
        assert_eq!(s.fail_streak, 1);
        assert_eq!(ramp_step(&cfg(), &mut s, 60.0), RampDecision::StepDown { new_tempo: 0.75 });
        assert_eq!(s.fail_streak, 0, "fail counter resets after demotion");
    }

    #[test]
    fn step_down_floors_at_start_tempo() {
        let mut s = state(0.70);
        s.fail_streak = 1;
        assert_eq!(ramp_step(&cfg(), &mut s, 0.0), RampDecision::StepDown { new_tempo: 0.70 });
    }

    #[test]
    fn pass_below_target_promotes_to_target_without_completing() {
        // v2 bug: pass at 0.95 completed instantly. v3: it promotes to
        // 1.00 and the NEXT pass (at target) completes.
        let mut s = state(0.95);
        assert_eq!(ramp_step(&cfg(), &mut s, 92.0), RampDecision::StepUp { new_tempo: 1.00 });
        assert!(s.armed, "not complete until a pass AT target");
        assert_eq!(ramp_step(&cfg(), &mut s, 92.0), RampDecision::Complete { new_tempo: 1.00 });
        assert!(!s.armed);
    }

    #[test]
    fn fail_at_target_steps_back_down() {
        let mut s = state(1.00);
        assert_eq!(ramp_step(&cfg(), &mut s, 50.0), RampDecision::Hold);
        assert_eq!(ramp_step(&cfg(), &mut s, 50.0), RampDecision::StepDown { new_tempo: 0.95 });
    }

    #[test]
    fn required_successes_gate_promotion() {
        let mut c = cfg();
        c.required_successes = 2;
        let mut s = state(0.70);
        assert_eq!(ramp_step(&c, &mut s, 95.0), RampDecision::Hold);
        assert_eq!(s.success_streak, 1);
        assert_eq!(ramp_step(&c, &mut s, 95.0), RampDecision::StepUp { new_tempo: 0.75 });
        assert_eq!(s.success_streak, 0);
    }

    #[test]
    fn fail_resets_success_streak_and_vice_versa() {
        let mut c = cfg();
        c.required_successes = 2;
        let mut s = state(0.80);
        ramp_step(&c, &mut s, 95.0); // success 1
        ramp_step(&c, &mut s, 50.0); // fail 1 — success streak dies
        assert_eq!(s.success_streak, 0);
        assert_eq!(s.fail_streak, 1);
        ramp_step(&c, &mut s, 95.0); // success 1 again — fail streak dies
        assert_eq!(s.fail_streak, 0);
    }

    #[test]
    fn clamp_to_config_pulls_step_into_range() {
        let mut s = state(0.70);
        let mut c = cfg();
        c.start_tempo = 0.80;
        clamp_to_config(&c, &mut s);
        assert!((s.step_tempo - 0.80).abs() < 1e-6, "raised start pulls step up");
        c.target_tempo = 0.75; // below current step
        clamp_to_config(&c, &mut s);
        assert!((s.step_tempo - 0.75).abs() < 1e-6, "lowered target pulls step down");
    }

    #[test]
    fn step_index_display() {
        let c = cfg();
        assert_eq!(ramp_step_index(&c, 0.70), (0, 6));
        assert_eq!(ramp_step_index(&c, 0.85), (3, 6));
        assert_eq!(ramp_step_index(&c, 1.00), (6, 6));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --lib ramp 2>&1 | tail -10`
Expected: COMPILE FAIL — new field names / `clamp_to_config` missing.

- [ ] **Step 3: New config/state in `session.rs`:**

```rust
pub const RAMP_START_DEFAULT: f32 = 0.70;
pub const RAMP_TARGET_DEFAULT: f32 = 1.00;
pub const RAMP_STEP_DEFAULT: f32 = 0.05;
pub const RAMP_THRESHOLD_DEFAULT: f32 = 90.0;
pub const RAMP_STREAK_DEFAULT: u8 = 1;

/// Accuracy-gated tempo-ramp configuration (rail-editable).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampConfig {
    pub start_tempo: f32,
    pub target_tempo: f32,
    pub step: f32,
    pub threshold_pct: f32,
    /// Consecutive passes required per promotion (and for completion).
    pub required_successes: u8,
}

impl Default for RampConfig {
    fn default() -> Self {
        Self {
            start_tempo: RAMP_START_DEFAULT,
            target_tempo: RAMP_TARGET_DEFAULT,
            step: RAMP_STEP_DEFAULT,
            threshold_pct: RAMP_THRESHOLD_DEFAULT,
            required_successes: RAMP_STREAK_DEFAULT,
        }
    }
}

/// Live ramp state; meaningful only while `armed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampState {
    pub armed: bool,
    /// The ramp's current tempo step. Owns playback while armed
    /// (`PracticeSession::effective_tempo`).
    pub step_tempo: f32,
    pub success_streak: u8,
    pub fail_streak: u8,
}

impl Default for RampState {
    fn default() -> Self {
        Self {
            armed: false,
            step_tempo: RAMP_START_DEFAULT,
            success_streak: 0,
            fail_streak: 0,
        }
    }
}
```

(`skip_next_roll` is gone — Task 7's `PracticeLoopCompleted` makes it unnecessary.)

- [ ] **Step 4: New pure protocol in `ramp.rs`** — replace `RampDecision`, `ramp_step`, `ramp_step_index`:

```rust
/// Outcome of one completed loop pass while the ramp is armed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampDecision {
    StepUp { new_tempo: f32 },
    StepDown { new_tempo: f32 },
    /// Streak not yet met (first fail, or successes below the required
    /// streak): keep the tempo.
    Hold,
    /// Passed AT the target tempo: ramp disarms; caller graduates
    /// `user_tempo` to the target.
    Complete { new_tempo: f32 },
}

/// Pure ramp protocol. A pass (accuracy ≥ threshold) builds the success
/// streak; meeting it at the target completes, below it steps up. Two
/// consecutive fails step down once, floored at the start tempo.
pub fn ramp_step(cfg: &RampConfig, state: &mut RampState, accuracy_pct: f32) -> RampDecision {
    if accuracy_pct >= cfg.threshold_pct {
        state.fail_streak = 0;
        state.success_streak += 1;
        if state.success_streak < cfg.required_successes {
            return RampDecision::Hold;
        }
        state.success_streak = 0;
        if state.step_tempo >= cfg.target_tempo - 1e-6 {
            state.armed = false;
            RampDecision::Complete {
                new_tempo: cfg.target_tempo,
            }
        } else {
            let next = (state.step_tempo + cfg.step).min(cfg.target_tempo);
            state.step_tempo = next;
            RampDecision::StepUp { new_tempo: next }
        }
    } else {
        state.success_streak = 0;
        state.fail_streak += 1;
        if state.fail_streak >= 2 {
            state.fail_streak = 0;
            let next = (state.step_tempo - cfg.step).max(cfg.start_tempo);
            state.step_tempo = next;
            RampDecision::StepDown { new_tempo: next }
        } else {
            RampDecision::Hold
        }
    }
}

/// Clamp the live step into `[start, target]` after a config edit.
pub fn clamp_to_config(cfg: &RampConfig, state: &mut RampState) {
    state.step_tempo = state.step_tempo.clamp(cfg.start_tempo, cfg.target_tempo);
}

/// `(current, total)` step indices for display ("RAMP 3/6").
pub fn ramp_step_index(cfg: &RampConfig, tempo: f32) -> (u32, u32) {
    if cfg.step <= 0.0 {
        return (0, 0);
    }
    let total = ((cfg.target_tempo - cfg.start_tempo) / cfg.step).round().max(0.0) as u32;
    let cur = (((tempo - cfg.start_tempo) / cfg.step).round() as i64).clamp(0, total as i64) as u32;
    (cur, total)
}
```

- [ ] **Step 5: Minimal compile fixes** so the crate builds (full rewiring is Tasks 6–7): in `handle_toggle_ramp` build the new `RampState { armed: true, step_tempo: cfg.start_tempo, success_streak: 0, fail_streak: 0 }` (drop the `session.rate = …` line — dead after Task 6, but for now keep behavior by setting `session.transport.user_tempo = cfg.start_tempo;`); in `apply_ramp` replace field names (`current_rate`→`step_tempo`, decisions' `new_rate`→`new_tempo`) and DELETE the `skip_next_roll` block. In `full_hud.rs`/`chip.rs` rename `start_rate`→`start_tempo`, `target_rate`→`target_tempo`. In `tests/practice_mode.rs`: `s.ramp.current_rate = rate` → `s.trainer.ramp.step_tempo = rate`; DELETE the `skip_next_roll_ignores_the_stale_pre_arm_attempt` test (superseded in Task 7). `apply_ramp`'s decision arms keep writing `session.transport.user_tempo` for now so existing integration tests still observe tempo movement.

- [ ] **Step 6: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass. If `ramp_steps_rate_up_after_clean_pass` fails on the removed skip flag, that's Task 7 territory — it must still pass here because `apply_ramp` still reads seeks at this point.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(practice): ramp protocol v2 — streaks, pass-at-target completion, config clamp"
```

---

### Task 6: Tempo ownership (`effective_tempo`, disarm-restores, nudge/loop-change disarm)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (`effective_tempo`, `roll_attempt`)
- Modify: `crates/gameplay-drums/src/practice/rate.rs`
- Modify: `crates/gameplay-drums/src/practice/actions.rs` (variant rename + disarm rules)
- Modify: `crates/gameplay-drums/src/practice/ramp.rs` (`handle_toggle_ramp`, `apply_ramp` decision arms)
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` (Tempo row + config-edit clamp + loop-change disarm)
- Modify: `crates/gameplay-drums/src/practice/hud/timeline_ui.rs` (drag disarms)
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs` (effective tempo)
- Test: `session.rs` unit tests + `tests/practice_mode.rs`

- [ ] **Step 1: Write failing unit tests** — append to `session.rs` tests:

```rust
    #[test]
    fn effective_tempo_layers_ramp_over_user() {
        let mut s = PracticeSession::default();
        s.transport.user_tempo = 1.0;
        assert!((s.effective_tempo() - 1.0).abs() < 1e-6);
        s.trainer.ramp.armed = true;
        s.trainer.ramp.step_tempo = 0.70;
        assert!((s.effective_tempo() - 0.70).abs() < 1e-6);
        s.trainer.ramp.armed = false;
        assert!(
            (s.effective_tempo() - 1.0).abs() < 1e-6,
            "disarm restores the user's tempo untouched"
        );
    }

    #[test]
    fn loop_mutation_disarms_ramp() {
        let mut s = PracticeSession::default();
        s.set_loop_start(2_000);
        s.set_loop_end(4_000);
        s.trainer.ramp.armed = true;
        s.set_loop_start(6_000);
        assert!(!s.trainer.ramp.armed, "changing A disarms");
        s.trainer.ramp.armed = true;
        s.clear_loop();
        assert!(!s.trainer.ramp.armed, "clearing the loop disarms");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --lib session 2>&1 | tail -10`
Expected: FAIL — `effective_tempo`/`clear_loop` missing.

- [ ] **Step 3: Implement in `session.rs`:**

```rust
    /// The tempo playback actually runs at: the ramp's step while armed,
    /// the player's chosen tempo otherwise.
    pub fn effective_tempo(&self) -> f32 {
        if self.trainer.ramp.armed {
            self.trainer.ramp.step_tempo
        } else {
            self.transport.user_tempo
        }
    }

    /// Clear the A/B loop (disarms the ramp — the ramp is a claim about
    /// one specific section).
    pub fn clear_loop(&mut self) {
        self.transport.loop_region = None;
        self.trainer.ramp.armed = false;
    }
```

Add `self.trainer.ramp.armed = false;` as the first line of `set_loop_start` and `set_loop_end`. In `roll_attempt`, record `tempo: self.effective_tempo(),` — and rename the `AttemptRecord` field `rate` → `tempo` (sweep: `full_hud.rs` `attempt_history_text` `a.rate`→`a.tempo`, `chip.rs` test literal).

- [ ] **Step 4: Consumers.** `rate.rs`: `let target = session.effective_tempo() as f64;`. `chip.rs` `chip_text`: `format!("{:.2}×", session.effective_tempo())` and `ramp_step_index(&session.trainer.ramp_config, session.effective_tempo())`.

`actions.rs`: rename `RateDown`/`RateUp` → `TempoDown`/`TempoUp` (bindings table, test). Apply arm:

```rust
            PracticeAction::TempoDown | PracticeAction::TempoUp => {
                let dir: i8 = if matches!(action, PracticeAction::TempoUp) { 1 } else { -1 };
                if session.trainer.ramp.armed {
                    session.trainer.ramp.armed = false;
                    toasts.push("ramp off (manual tempo)");
                }
                session.step_user_tempo(dir);
                toasts.push(format!("tempo → {:.2}×", session.transport.user_tempo));
            }
```

Loop-change call sites in `actions.rs` (`SetLoopStart`, `SetLoopEnd`, `ClearLoop` → use `session.clear_loop()`):

```rust
                let was_armed = session.trainer.ramp.armed;
                session.set_loop_start(ms);
                if was_armed {
                    toasts.push("ramp off (loop changed)");
                }
```

(same pattern for `SetLoopEnd`/`ClearLoop`; and in `full_hud.rs` `SetA`/`SetB`/`ClearLoop` — full HUD has `toasts: ResMut<ToastQueue>` added as a param. `timeline_ui.rs` `LoopPreview` arm: `session.loop_region = …` becomes a direct write; precede with the same was-armed check pushing the toast — add `mut toasts: ResMut<ToastQueue>` param. Disarm happens once; subsequent drag frames see `armed == false`.)

Wait — `timeline_ui.rs` writes `session.transport.loop_region` directly (drag preview each frame, avoiding the min-bar logic in the setters). Change it to:

```rust
        GestureEffect::LoopPreview { anchor_ms } => {
            if session.trainer.ramp.armed {
                session.trainer.ramp.armed = false;
                toasts.push("ramp off (loop changed)");
            }
            session.transport.loop_region = Some(drag_region(&timeline, anchor_ms, cursor_ms));
        }
```

`ramp.rs` `handle_toggle_ramp` — arm no longer writes `user_tempo`; disarm restores implicitly:

```rust
        if session.trainer.ramp.armed {
            session.trainer.ramp.armed = false;
            toasts.push(format!("ramp off — tempo {:.2}×", session.transport.user_tempo));
            continue;
        }
        let cfg = session.trainer.ramp_config;
        session.trainer.ramp = RampState {
            armed: true,
            step_tempo: cfg.start_tempo,
            success_streak: 0,
            fail_streak: 0,
        };
```

`apply_ramp` decision arms: `StepUp`/`StepDown` no longer write `user_tempo` (the state mutation inside `ramp_step` already moved `step_tempo`; `effective_tempo` picks it up). `Complete` graduates:

```rust
        RampDecision::Complete { new_tempo } => {
            session.transport.user_tempo = new_tempo;
            toasts.push("ramp complete");
        }
```

`full_hud.rs` config-edit rows call the clamp after mutation (all four of `RampStart`/`RampTarget`/`RampStep`/`RampThreshold` arms):

```rust
            RailItem::RampStart => {
                let c = &mut session.trainer.ramp_config;
                c.start_tempo =
                    (c.start_tempo + dir as f32 * 0.05).clamp(0.5, c.target_tempo - 0.05);
                let cfg = session.trainer.ramp_config;
                crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
            }
```

(same clamp call after `RampTarget`; `RampStep`/`RampThreshold` need no clamp).

- [ ] **Step 5: Update integration tests** in `tests/practice_mode.rs`: assertions on `session.rate` become `session.effective_tempo()` (ramp step tests) — e.g. `ramp_steps_rate_up_after_clean_pass` asserts `(session.effective_tempo() - 0.75).abs() < 1e-6`. `toggle_ramp_with_loop_arms` also asserts `(session.effective_tempo() - 0.70).abs() < 1e-6` and that `session.transport.user_tempo` is still `1.0`. Add:

```rust
#[test]
fn tempo_nudge_while_armed_disarms_and_nudges_user_tempo() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.transport.user_tempo = 1.0;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Minus);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(!session.trainer.ramp.armed, "manual nudge disarms the ramp");
    assert!(
        (session.transport.user_tempo - 0.95).abs() < 1e-6,
        "nudge applies to the user tempo (1.00 → 0.95)"
    );
}
```

(`looped_session` sets `user_tempo` to the passed value — give it a second look: after this task it should set `trainer.ramp.step_tempo = rate` and leave `user_tempo` at 1.0 unless the test overrides.)

- [ ] **Step 6: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(practice): layered tempo ownership — effective_tempo, disarm restores, nudge/loop-change disarm"
```

---

### Task 7: `PracticeLoopCompleted` + implicit whole-song loop + `LastFinalizedAttempt`

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ab_loop.rs` (message, implicit region, emission)
- Modify: `crates/gameplay-drums/src/practice/stats.rs` (`LastFinalizedAttempt`)
- Modify: `crates/gameplay-drums/src/practice/ramp.rs` (`apply_ramp` reads completions; `handle_toggle_ramp` arms over implicit region)
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (register message + resource)
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

- [ ] **Step 1: Write the failing tests** — in `tests/practice_mode.rs`, add `.add_message::<gameplay_drums::practice::ab_loop::PracticeLoopCompleted>()` and `.init_resource::<gameplay_drums::practice::stats::LastFinalizedAttempt>()` to `add_ramp_wiring`, then append:

```rust
#[test]
fn manual_restart_does_not_step_the_ramp() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.70));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    // A clean partial attempt, then a manual restart (R) — NOT a wrap.
    app.world_mut()
        .resource_mut::<Messages<JudgmentEvent>>()
        .write(JudgmentEvent {
            lane: 3,
            kind: dtx_scoring::JudgmentKind::Perfect,
            delta_ms: 0,
            chip_idx: 0,
        });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.70).abs() < 1e-6,
        "manual restart must never count as a ramp pass"
    );
}

#[test]
fn empty_loop_pass_makes_no_ramp_decision() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    // Previous clean pass already in history at this loop's start.
    let mut s = looped_session(0.70);
    s.attempt_history.push(gameplay_drums::practice::session::AttemptRecord {
        start_ms: 2_000,
        end_ms: 6_000,
        tempo: 0.70,
        counts: Default::default(),
        overhits: 0,
        max_combo: 4,
        accuracy_pct: 100.0,
        mean_error_ms: 0.0,
    });
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 0); // wrap with ZERO judgments? No — 0 hits writes a miss.
    // Re-do: wrap with literally nothing judged:
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.70).abs() < 1e-6,
        "an empty pass must not re-apply the previous attempt's accuracy"
    );
}

#[test]
fn no_loop_set_wraps_at_chart_end_as_implicit_loop() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(4)); // end ≈ 8000ms
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(9_000)); // past chart end
    }
    app.update();
    let clock = app.world().resource::<GameplayClock>();
    assert!(
        clock.current_ms < 9_000,
        "reaching chart end in practice wraps to the start (implicit loop), got {}",
        clock.current_ms
    );
}
```

Note the `empty_loop_pass` test writes no judgments and no miss before the wrap — delete the misleading `finish_loop_pass(&mut app, 0)` call shown above and keep only the clock-sync + update (the comment shows the intent; final code has just the sync block).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode 2>&1 | tail -15`
Expected: COMPILE FAIL (`PracticeLoopCompleted` / `LastFinalizedAttempt` missing).

- [ ] **Step 3: Implement `ab_loop.rs`:**

```rust
//! A/B loop: when the clock passes B, seek back to A (with pre-roll).
//! With no explicit region armed, practice loops the whole song
//! implicitly — the stage never "ends" in practice.

use bevy::prelude::*;
use game_shell::{AppState, PauseState};

use super::session::{preroll_target, LoopRegion, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// One loop pass finished: the wrap seek was just emitted. The ONLY
/// trigger for ramp decisions and wrap reports — manual seeks and
/// restarts never produce this.
#[derive(Message, Debug, Clone, Copy)]
pub struct PracticeLoopCompleted {
    pub region_start_ms: i64,
    pub region_end_ms: i64,
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<PracticeLoopCompleted>().add_systems(
        FixedUpdate,
        loop_watcher
            .before(crate::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}

/// The region practice is looping right now: the armed A/B region, or
/// the whole song when none is set (A-only regions count as unset).
pub fn active_region(session: &PracticeSession, timeline: &ChipTimeline) -> LoopRegion {
    session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .unwrap_or(LoopRegion {
            start_ms: 0,
            end_ms: timeline.end_ms,
        })
}

pub fn loop_watcher(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut completed: MessageWriter<PracticeLoopCompleted>,
) {
    if !clock.is_ready() || timeline.end_ms <= 0 {
        return;
    }
    let region = active_region(&session, &timeline);
    if clock.current_ms >= region.end_ms {
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.transport.preroll, region.start_ms),
            snap: None,
            attempt_start_ms: Some(region.start_ms),
        });
        completed.write(PracticeLoopCompleted {
            region_start_ms: region.start_ms,
            region_end_ms: region.end_ms,
        });
    }
}
```

- [ ] **Step 4: `LastFinalizedAttempt` in `stats.rs`:**

```rust
/// The attempt finalized by the most recent seek this tick: `Some` when
/// it had data and was pushed to history, `None` when it was empty.
/// Read by `apply_ramp`/`wrap_micro_report` in the same tick.
#[derive(Resource, Debug, Default, Clone)]
pub struct LastFinalizedAttempt(pub Option<crate::practice::session::AttemptRecord>);
```

Make `roll_attempt` in `session.rs` return the record it pushed:

```rust
    pub fn roll_attempt(&mut self, end_ms: i64, next_start_ms: i64) -> Option<AttemptRecord> {
```

(build the `AttemptRecord` into a local, push a clone, return `Some(record)`; return `None` on the no-data path — update the three `roll_attempt` unit tests to ignore or assert the return.)

In `track_attempt_stats`, add `mut finalized: ResMut<LastFinalizedAttempt>` and set it on the seek branch:

```rust
    if let Some(seek) = seeks.read().last() {
        let end_ms = last_seek_from.0.take().unwrap_or(clock.current_ms);
        let next_start = seek.attempt_start_ms.unwrap_or(seek.target_ms);
        finalized.0 = session.roll_attempt(end_ms, next_start);
        combo.current = 0;
    }
```

Register `init_resource::<LastFinalizedAttempt>()` in `stats.rs`'s `plugin`.

- [ ] **Step 5: Rewire `apply_ramp` in `ramp.rs`:**

```rust
/// Apply one ramp decision per completed loop pass. Runs after
/// `track_attempt_stats` (same tick as the wrap's seek) so
/// `LastFinalizedAttempt` holds this pass's attempt. Manual seeks and
/// restarts emit no `PracticeLoopCompleted`, so they can never step
/// the ramp; an empty pass finalizes no attempt and is skipped.
pub fn apply_ramp(
    mut completions: MessageReader<super::ab_loop::PracticeLoopCompleted>,
    finalized: Res<super::stats::LastFinalizedAttempt>,
    mut session: ResMut<PracticeSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    let Some(done) = completions.read().last().copied() else {
        return;
    };
    if !session.trainer.ramp.armed {
        return;
    }
    let Some(att) = finalized.0.as_ref() else {
        return; // empty pass: nothing judged, no decision
    };
    if att.start_ms != done.region_start_ms {
        return; // attempt belongs to a different span
    }
    let accuracy = att.accuracy_pct;
    let cfg = session.trainer.ramp_config;
    match ramp_step(&cfg, &mut session.trainer.ramp, accuracy) {
        RampDecision::StepUp { new_tempo } => toasts.push(format!("ramp: {new_tempo:.2}×")),
        RampDecision::StepDown { new_tempo } => {
            toasts.push(format!("ramp: back to {new_tempo:.2}×"))
        }
        RampDecision::Hold => toasts.push("ramp: hold"),
        RampDecision::Complete { new_tempo } => {
            session.transport.user_tempo = new_tempo;
            toasts.push("ramp complete");
        }
    }
}
```

`handle_toggle_ramp`: arming works with or without an explicit loop (implicit whole-song region counts). Replace the `loop_armed` error branch:

```rust
        let cfg = session.trainer.ramp_config;
        session.trainer.ramp = RampState {
            armed: true,
            step_tempo: cfg.start_tempo,
            success_streak: 0,
            fail_streak: 0,
        };
        let a_ms = session
            .transport
            .loop_region
            .filter(|r| r.end_ms != i64::MAX)
            .map(|r| r.start_ms)
            .unwrap_or(0);
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.transport.preroll, a_ms),
            snap: None,
            attempt_start_ms: Some(a_ms),
        });
        toasts.push(format!("ramp armed @ {:.2}×", cfg.start_tempo));
```

Update the two toggle tests: `toggle_ramp_without_loop_is_a_noop_error_toast` becomes `toggle_ramp_without_loop_arms_over_whole_song` (assert `armed == true` and `effective_tempo() == 0.70`).

- [ ] **Step 6: Update test wiring.** `add_ramp_wiring` in `tests/practice_mode.rs` must mirror the new data flow — the existing `(track_attempt_stats, apply_ramp).chain().after(apply_seek_system)` stays; `loop_watcher` (already wired `.before(apply_seek_system)`) now also emits completions. `ramp_steps_rate_up_after_clean_pass` / `two_failed_passes_step_rate_down` keep passing because `finish_loop_pass` triggers a real wrap.

- [ ] **Step 7: Register in the real plugin.** `stats.rs` plugin: `init_resource` (Step 4). `ramp.rs` plugin unchanged in shape (`apply_ramp .after(track_attempt_stats)`), but `apply_ramp`'s params changed — `tests/fixed_update_schedule_ordering.rs` mirrors this pair; check it still compiles (it uses stand-in systems; only touch it if it references removed types).

- [ ] **Step 8: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat(practice): PracticeLoopCompleted drives the ramp; implicit whole-song loop"
```

---

### Task 8: Practice never ends (no Result, no end bonus)

**Files:**
- Modify: `crates/gameplay-drums/src/orchestrator.rs:404-410`
- Test: `crates/gameplay-drums/tests/practice_mode.rs:127-170` (two tests change meaning)

- [ ] **Step 1: Update the tests.** `a_only_loop_region_does_not_suppress_end_of_stage` and `cleared_loop_region_restores_end_of_stage` asserted practice reaches end-of-stage — v3 inverts that. Replace both with:

```rust
#[test]
fn practice_never_requests_end_of_stage() {
    // v3: practice is a room — the implicit whole-song loop wraps
    // instead; detect_end_of_stage must never fire while a
    // PracticeSession exists, loop or no loop.
    for region in [
        None,
        Some(LoopRegion { start_ms: 0, end_ms: i64::MAX }), // A-only
        Some(LoopRegion { start_ms: 0, end_ms: 2_000 }),    // armed
    ] {
        let mut app = build_app();
        enter_performance(&mut app, chart_with_measures(2));
        let mut s = PracticeSession::default();
        s.transport.loop_region = region;
        app.world_mut().insert_resource(s);
        {
            let mut clock = app.world_mut().resource_mut::<GameplayClock>();
            clock.start();
            clock.sync(Some(50_000));
        }
        app.update();
        assert!(
            !app.world().resource::<DrumsStageCompletion>().end_requested,
            "practice must never end the stage (region: {region:?})"
        );
    }
}
```

Keep `active_loop_region_suppresses_end_of_stage` (still true, weaker claim) or fold it into the loop above and delete it — folding preferred. `seek_is_inert_without_practice_in_normal_play` stays untouched (normal play still ends).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode practice_never_requests 2>&1 | tail -10`
Expected: FAIL — A-only/None cases still request end.

- [ ] **Step 3: Implement** — `orchestrator.rs`, replace the loop-armed gate in `detect_end_of_stage`:

```rust
    // Practice is a room, not a run: the implicit whole-song loop wraps
    // at the chart end (see practice::ab_loop), so the stage never ends
    // and the XG end bonus never applies while practicing.
    if practice.is_some() {
        return;
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(practice): stage never ends in practice — implicit loop owns the chart end"
```

---

### Task 9: Wrap micro-report toast

**Files:**
- Modify: `crates/gameplay-drums/src/practice/stats.rs` (new system + plugin registration)
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn loop_wrap_pushes_a_micro_report_toast() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.trainer.ramp.armed = false; // report fires with or without ramp
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let toasts = app.world().resource::<gameplay_drums::practice::toast::ToastQueue>();
    let report = toasts
        .0
        .iter()
        .find(|t| t.text.starts_with("pass "))
        .expect("wrap must push a micro-report toast");
    assert!(report.text.contains('%'), "report shows accuracy: {}", report.text);
    assert!(report.text.contains("ms"), "report shows mean error: {}", report.text);
}
```

Wire `wrap_micro_report` into `add_ramp_wiring`'s post-seek chain: `(track_attempt_stats, wrap_micro_report, apply_ramp).chain()`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode loop_wrap_pushes 2>&1 | tail -10`
Expected: COMPILE FAIL — `wrap_micro_report` missing.

- [ ] **Step 3: Implement in `stats.rs`:**

```rust
/// One-line feedback at each loop wrap: `pass 5 · 93.8% · 3 miss · +18ms`
/// (`+` = late, `−` = early). Pass count = attempts on this span in
/// history. Feedback lands at the loop boundary, never mid-play.
pub fn wrap_micro_report(
    mut completions: MessageReader<super::ab_loop::PracticeLoopCompleted>,
    finalized: Res<LastFinalizedAttempt>,
    session: Res<PracticeSession>,
    mut toasts: ResMut<super::toast::ToastQueue>,
) {
    let Some(done) = completions.read().last().copied() else {
        return;
    };
    let Some(att) = finalized.0.as_ref() else {
        return;
    };
    if att.start_ms != done.region_start_ms {
        return;
    }
    let n = session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == done.region_start_ms)
        .count();
    toasts.push(format!(
        "pass {n} · {:.1}% · {} miss · {:+.0}ms",
        att.accuracy_pct, att.counts.miss, att.mean_error_ms
    ));
}
```

Register in `stats.rs` `plugin` between the existing pair:

```rust
        (track_attempt_stats, wrap_micro_report)
            .chain()
            .after(crate::judge::judge_lane_hit_system)
```

(`apply_ramp` already runs `.after(track_attempt_stats)`; ordering between `wrap_micro_report` and `apply_ramp` is irrelevant — both only read.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(practice): loop-wrap micro-report toast"
```

---

### Task 10: Span-filtered attempt history

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` (`attempt_history_text`)
- Modify: `crates/gameplay-drums/src/practice/hud/chip.rs` (`chip_text` last-accuracy)
- Test: unit tests in both files

- [ ] **Step 1: Write the failing tests.** In `full_hud.rs` add a test module (none exists — create `#[cfg(test)] mod tests` at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{AttemptRecord, LoopRegion};

    fn record(start_ms: i64, acc: f32) -> AttemptRecord {
        AttemptRecord {
            start_ms,
            end_ms: start_ms + 4_000,
            tempo: 1.0,
            counts: Default::default(),
            overhits: 0,
            max_combo: 0,
            accuracy_pct: acc,
            mean_error_ms: 0.0,
        }
    }

    #[test]
    fn attempt_history_filters_to_current_span() {
        let mut s = PracticeSession::default();
        s.transport.loop_region = Some(LoopRegion { start_ms: 2_000, end_ms: 6_000 });
        s.attempt_history.push(record(0, 50.0));      // old free-play span
        s.attempt_history.push(record(2_000, 91.0));  // this loop
        s.attempt_history.push(record(8_000, 60.0));  // scrub junk
        s.attempt_history.push(record(2_000, 95.0));  // this loop
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("91.0%") && text.contains("95.0%"));
        assert!(!text.contains("50.0%") && !text.contains("60.0%"));
    }

    #[test]
    fn attempt_history_no_loop_uses_implicit_whole_song_span() {
        let mut s = PracticeSession::default();
        s.attempt_history.push(record(0, 88.0));     // implicit span
        s.attempt_history.push(record(4_000, 70.0)); // partial
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("88.0%"));
        assert!(!text.contains("70.0%"));
    }
}
```

In `chip.rs`, extend the existing test: push a second record with `start_ms: 999` (junk span) and accuracy `11.0` after the matching one and assert the chip still shows `94%` (span filter picks the loop's latest, not `.last()`).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --lib full_hud 2>&1 | tail -10`
Expected: COMPILE FAIL — `attempt_history_text` takes one arg.

- [ ] **Step 3: Implement.** `full_hud.rs`:

```rust
/// Attempts for the current span only (armed A/B region, or the
/// implicit whole-song span when none). `end_ms` = chart end.
pub fn attempt_history_text(session: &PracticeSession, end_ms: i64) -> String {
    let span_start = session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .map(|r| r.start_ms)
        .unwrap_or(0);
    let _ = end_ms; // span identity is start-keyed; end kept for future use
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == span_start)
        .enumerate()
        .rev()
        .take(8)
    {
        lines.push(format!(
            "#{}  {:.1}%  {:+.0}ms  x{:.2}",
            i + 1,
            a.accuracy_pct,
            a.mean_error_ms,
            a.tempo
        ));
    }
    lines.join("\n")
}
```

Callers (`spawn_full_hud`, `full_hud_input`) pass `timeline.end_ms` — `full_hud_input` already has `timeline: Res<ChipTimeline>`.

`chip.rs` `chip_text` — replace the `.last()` accuracy block:

```rust
    let span_start = session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .map(|r| r.start_ms)
        .unwrap_or(0);
    if let Some(last) = session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == span_start)
        .next_back()
    {
        parts.push(format!("{:.0}%", last.accuracy_pct));
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(practice): attempt history and chip filter to the current loop span"
```

---

### Task 11: Rail regroup, Tempo labels, streak row

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- Test: `crates/gameplay-drums/tests/practice_hud.rs` (existing spawn tests keep passing; extend rail-label assertions if present)

- [ ] **Step 1: Extend `RailItem`.** Add `RampStreak` after `RampThreshold`; `ORDER` becomes 16 entries grouped Transport → Loop → Trainer:

```rust
    pub const ORDER: [RailItem; 16] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::Rate,      // labeled "Tempo"
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::RampArm,
        RailItem::RampStart,
        RailItem::RampTarget,
        RailItem::RampStep,
        RailItem::RampThreshold,
        RailItem::RampStreak,
        RailItem::ExitPractice,
    ];
```

(Keep the variant name `Rate` — renaming the enum variant is churn without user value; only the label changes.)

- [ ] **Step 2: Labels** in `rail_label`:

```rust
        RailItem::Rate => {
            if session.trainer.ramp.armed {
                format!(
                    "Tempo  ◀ x{:.2} ▶   (ramp x{:.2})",
                    session.transport.user_tempo,
                    session.trainer.ramp.step_tempo
                )
            } else {
                format!("Tempo  ◀ x{:.2} ▶", session.transport.user_tempo)
            }
        }
```

`RampArm` uses `session.effective_tempo()` for the step index; `RampStart`/`RampTarget` labels say `Ramp start ◀ x… ▶` (field names now `start_tempo`/`target_tempo`); add:

```rust
        RailItem::RampStreak => format!(
            "Ramp streak  ◀ ×{} ▶",
            session.trainer.ramp_config.required_successes
        ),
```

- [ ] **Step 3: Input arms.** In `full_hud_input`'s left/right match:

```rust
            RailItem::RampStreak => {
                let c = &mut session.trainer.ramp_config;
                c.required_successes = (c.required_successes as i8 + dir).clamp(1, 3) as u8;
            }
```

Rate row already routes through `step_user_tempo` + disarm (Task 6). Enter on `RampStreak` is a no-op (add to the existing no-op arm). Add group headers: in `spawn_full_hud`'s rail loop, before items 3 (`Rate`), 6 (`SetA`), 9 (`RampArm`) spawn non-interactive header texts `TRANSPORT`, `LOOP`, `TRAINER` (plain `Text` + `Theme::label_font()` + `theme.text_secondary.with_alpha(0.6)`, `margin: UiRect::top(Val::Px(8.0))`). Simplest: iterate `RailItem::ORDER` with index and spawn the header before the matching indices.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: all pass (practice_hud spawn tests count entities loosely; if one asserts an exact rail-row count, update it to 16).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(practice): rail regroup with headers, Tempo labels, ramp streak row"
```

---

### Task 12: Widget registry — practice defaults + `PracticeTransport` removal + tolerant TOML

**Files:**
- Modify: `crates/dtx-layout/src/widgets.rs` (remove variant)
- Modify: `crates/dtx-layout/src/scene.rs` (defaults, `WidgetKindField`, tolerant resolve)
- Modify: `crates/gameplay-drums/src/widget_layout.rs:291-299` (test)
- Modify: `crates/gameplay-drums/tests/widget_layout.rs`
- Check/modify: `grep -rn "PracticeTransport" crates/` — fix every remaining reference (editor tests may construct it)
- Test: `crates/dtx-layout/src/scene.rs` unit tests

- [ ] **Step 1: Write the failing tests** in `scene.rs`:

```rust
    #[test]
    fn score_widgets_hidden_in_practice_by_default() {
        for kind in [
            WidgetKind::ScorePanel,
            WidgetKind::PhraseMeter,
            WidgetKind::LiveGraph,
            WidgetKind::SongProgress,
        ] {
            let d = default_instance(kind);
            assert!(d.visible_play, "{kind:?} visible in play");
            assert!(!d.visible_practice, "{kind:?} hidden in practice");
        }
        let combo = default_instance(WidgetKind::Combo);
        assert!(combo.visible_play && combo.visible_practice);
    }

    #[test]
    fn unknown_widget_kind_in_toml_is_skipped_not_fatal() {
        let toml = r#"
[[widgets]]
kind = "practice-transport"

[[widgets]]
kind = "combo"
offset = [10.0, 0.0]
"#;
        let section: SceneSection = toml::from_str(toml).expect("unknown kind must not fail parse");
        let map = section.resolve();
        assert_eq!(map.len(), WidgetKind::ALL.len());
        assert_eq!(map[&WidgetKind::Combo].offset, (10.0, 0.0));
    }
```

(If `toml` isn't a dev-dependency of dtx-layout, add `toml = "0.8"` under `[dev-dependencies]` in `crates/dtx-layout/Cargo.toml` — check what version the workspace uses first: `grep -rn "^toml" Cargo.toml crates/*/Cargo.toml`.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dtx-layout 2>&1 | tail -10`
Expected: FAIL / COMPILE FAIL.

- [ ] **Step 3: Remove the variant.** `widgets.rs`: delete `PracticeTransport` from the enum, from `ALL` (now `[WidgetKind; 10]`), from `display_name`. `scene.rs`: new defaults —

```rust
pub fn default_instance(kind: WidgetKind) -> WidgetInstance {
    let (vis_play, vis_practice) = match kind {
        WidgetKind::ScorePanel
        | WidgetKind::PhraseMeter
        | WidgetKind::LiveGraph
        | WidgetKind::SongProgress => (true, false),
        _ => (true, true),
    };
```

Delete the `practice_transport_hidden_in_play_by_default` test. Update the module doc comment ("Practice widgets are hidden in play" → "Score-centric widgets are hidden in practice by default").

- [ ] **Step 4: Tolerant kind field.** In `scene.rs`:

```rust
/// Widget kind as serialized: tolerates kinds this build doesn't know
/// (e.g. layouts saved by other versions) — unknown entries are skipped
/// with a warning instead of failing the whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WidgetKindField {
    Known(WidgetKind),
    Unknown(String),
}
```

`WidgetEntry.kind: WidgetKindField`. `resolve()` skips unknowns:

```rust
        for entry in &self.widgets {
            let kind = match &entry.kind {
                WidgetKindField::Known(k) => *k,
                WidgetKindField::Unknown(s) => {
                    eprintln!("dtx-layout: unknown widget kind '{s}' in [scene.gameplay], skipped");
                    continue;
                }
            };
            if !seen.insert(kind) { /* existing duplicate handling, keyed on kind */ }
            map.insert(kind, entry.to_instance(kind));
        }
```

`to_instance` takes the resolved `kind: WidgetKind` param (it can no longer read an unknown from `self`); `from_instance` writes `kind: WidgetKindField::Known(i.kind)`. Fix every `WidgetEntry { kind: WidgetKind::X, … }` literal in tests to `kind: WidgetKindField::Known(WidgetKind::X)` (scene.rs tests, `tests/widget_layout.rs:35`). Export `WidgetKindField` from `dtx-layout`'s lib.rs alongside `WidgetEntry`.

- [ ] **Step 5: Sweep remaining references.**

Run: `grep -rn "PracticeTransport" crates/`
Fix each: `widget_layout.rs:291-299` test `visibility_respects_mode` → use `ScorePanel` (visible play, hidden practice); `tests/widget_layout.rs:23-29` → same rename (`score_panel_hidden_in_practice_shown_in_play`). Editor tests referencing it (if any) → switch to another kind.

- [ ] **Step 6: Run tests**

Run: `cargo test -p dtx-layout -p gameplay-drums 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(layout): practice-mode widget defaults; remove PracticeTransport; tolerate unknown kinds in TOML"
```

---

### Task 13: Visible practice entry in song select

**Files:**
- Modify: `crates/game-menu/src/song_select.rs:749-757` (hint bar)

- [ ] **Step 1: Add the hint.** In the bottom hint bar array, after `("ENTER PLAY", true)`:

```rust
                            ("SHIFT+ENTER PRACTICE", false),
```

- [ ] **Step 2: Build + run menu tests**

Run: `cargo test -p game-menu 2>&1 | tail -5`
Expected: pass (no test asserts the hint list; visual-only change).

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat(song-select): show Shift+Enter practice hint"
```

---

### Task 14: Schedule guard + final sweep

**Files:**
- Modify: `crates/gameplay-drums/tests/fixed_update_schedule_ordering.rs`
- Test: full workspace practice-relevant crates

- [ ] **Step 1: Real-schedule smoke.** Green integration tests don't prove the real plugin schedule builds (they wire systems manually). Verify `tests/fixed_update_schedule_ordering.rs` still mirrors the actual `FixedUpdate` wiring: `loop_watcher .before(apply_seek_system)`, `track_attempt_stats .after(judge)`, `wrap_micro_report` chained after it, `apply_ramp .after(track_attempt_stats)`. Extend its mirror with the `wrap_micro_report` link if it lists the practice chain explicitly. Also confirm `tests/practice_hud.rs` still has the plugin-build smoke test (`gameplay_drums::practice::hud::plugin` on a headless app) passing — it exercises real system registration including changed params.

- [ ] **Step 2: Full suite + format**

Run: `cargo test -p gameplay-drums -p dtx-layout -p game-menu 2>&1 | tail -8`
Expected: all green.

Run: `cargo fmt -p gameplay-drums && cargo fmt -p dtx-layout && cargo fmt -p game-menu && git diff --stat`
Expected: no unrelated files touched.

- [ ] **Step 3: Spec cross-check.** Re-read `docs/superpowers/specs/2026-07-07-practice-v3-training-model-design.md` §§1–8 against the diff: implicit loop (T7), no Result (T8), tempo layering + all disarm rules (T6), ramp v2 + streak + pass-at-target (T5/T7), chip_idx/overhits/snapped-seek (T1–3), state split (T4), span filter (T10), micro-report (T9), rail UX (T11), widget defaults + removal + tolerant TOML (T12), hint (T13).

- [ ] **Step 4: Commit any stragglers**

```bash
git add -A && git commit -m "test(practice): schedule-ordering guard updated for v3 wiring"
```

---

## Self-review notes (already applied)

- Type consistency: `RampConfig { start_tempo, target_tempo, step, threshold_pct, required_successes }`, `RampState { armed, step_tempo, success_streak, fail_streak }`, `AttemptRecord.tempo`, `AttemptStats.overhits`, `PracticeLoopCompleted { region_start_ms, region_end_ms }`, `LastFinalizedAttempt(Option<AttemptRecord>)` — used identically across Tasks 5–11.
- `skip_next_roll` deleted in Task 5; its test deleted there; the arm-restart seek can't step the ramp after Task 7 because arming emits a seek but no `PracticeLoopCompleted`.
- Task 7's `empty_loop_pass` test: `LastFinalizedAttempt` is `None` for the wrap (roll_attempt returned None) — but note it retains the previous `Some` unless overwritten. `track_attempt_stats` overwrites it on EVERY seek (`finalized.0 = session.roll_attempt(...)`), including empty ones — that's what makes the stale-attempt bug die. Implementers: do NOT wrap the assignment in `if let Some`.
- `preroll_target` import in `ab_loop.rs` gains `LoopRegion` in the same `use` line.
