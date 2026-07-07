# Practice v3: Training Model — Design

**Date:** 2026-07-07
**Status:** Approved via discussion (this session)
**Builds on:** practice v2 (two-tier HUD + accuracy ramp, merged in `edbc7ab`)
**Companion notes:** `docs/notes/2026-07-07-practice-training-research.md`
(adopted/rejected/parked rationale), `docs/notes/2026-07-07-practice-future-ideas.md`

## Goal

Practice mode becomes a *room*, not a *run*. Cut the cord to
performance-run semantics (song end, Result screen, global score),
give the rate/ramp interaction a single-owner state model, fix the
verified stat-attribution bugs, and add loop-boundary feedback.

Performance answers "can I clear this and score." Practice answers
"is this attempt better than the last one." Practice never touches
normal scores, ranks, or records (the existing save gate in
`game-results` stays).

## Non-goals (parked, see companion notes)

Segments/favorites, lane focus / limb isolation, per-lane and per-bar
stat *display*, weak-spot analyzer, generated drills, persistence /
mastery records (blocked on foundation phase 0), count-in metronome,
pre-entry setup screen, multi-criteria pass conditions.

We do however **collect** the data these need (miss chip identity,
overhits) — cheap now, expensive to retrofit.

## Design

### 1. Practice is a room: implicit whole-song loop, no song end

```
PERFORMANCE (a run)          PRACTICE (a room)
start → play → end           enter → [play|scrub|loop]* → exit
         │       │
         ▼       ▼           no end. no Result. no records.
      score   Result         time = a scrubber, not progression
```

- When no explicit A/B loop is set, practice behaves as an implicit
  loop over `[0, chart_end_ms]`: reaching chart end wraps back to the
  start (with pre-roll), exactly like a B→A wrap. One code path: the
  loop watcher falls back to the implicit region when
  `session.loop_region` is `None` or unarmed.
- `detect_end_of_stage` (`orchestrator.rs`) early-returns whenever
  `PracticeSession` exists (today it only defers to an *armed* loop).
  Practice never transitions to `AppState::Result`. The XG end bonus
  never applies in practice.
- Because a wrap is a seek, the running attempt finalizes through the
  existing `roll_attempt` path — no song-end special case, no summary
  screen needed. Exit stays Esc → out (existing flow).

### 2. Rate ownership: user tempo vs ramp layer

Single mutable `session.rate` with three writers (user, ramp, rail)
is replaced by layered ownership:

```
transport.user_tempo : f32     ← only user edits (rail, hotkeys)
trainer.ramp         : Disarmed | Armed { step_tempo }
                                 ← only ramp logic edits step_tempo

effective_tempo() = Armed { step_tempo } → step_tempo
                    Disarmed             → user_tempo
```

All playback-rate consumers (`rate.rs` → `AudioRate`/Kira, chip
display, attempt records) read `effective_tempo()`.

Transitions:

```
             arm (needs loop; implicit whole-song loop counts)
 Disarmed ────────────────────► Armed { step_tempo = cfg.start }
    ▲                              │
    │ disarm → user_tempo          │ pass/fail decisions (§3)
    │ UNTOUCHED, playback          │
    │ returns to it                ▼
    └────────── complete: user_tempo := cfg.target, disarm
                ("graduation" — only case ramp writes user_tempo)
```

Rules while armed:
- **Manual tempo nudge disarms the ramp** (toast: `ramp off (manual
  tempo)`), then applies the nudge to `user_tempo`. One owner at a
  time; v2's silent "adoption" behavior is removed.
- Editing `Ramp start` / `Ramp target` clamps `step_tempo` into
  `[start, target]` immediately (raising start above the current step
  pulls the step up with it; lowering target below it pulls it down —
  it still must *pass at* target to complete, §3).
- Editing step/threshold/streak affects future decisions only.

User-facing label is **Tempo** everywhere (rail rows, toasts); "rate"
remains an internal identifier only where renaming is churn without
value. The status chip keeps its compact `0.85×` form.

### 3. Ramp protocol v2: loop completions only, mastery at target

Two defects fixed:

**a. Ramp fires only on completed loop passes.** New message:

```rust
#[derive(Message)]
pub struct PracticeLoopCompleted {
    pub region_start_ms: i64,
    pub region_end_ms: i64,
}
```

Written by the loop watcher at the moment it emits the wrap seek
(explicit or implicit region). `apply_ramp` reads *this*, not raw
`SeekToChartTime`. Manual restarts, scrub seeks, and the arm-restart
seek therefore never count as passes — v2's `skip_next_roll` hack and
the stale-attempt bug (empty pass re-reading old history) both die:
the ramp decision uses the attempt finalized by *this* wrap, and if
that attempt has no data (`has_data() == false`, nothing was pushed
to history) the pass is ignored entirely.

**b. Complete = pass AT target, not on reaching it.** v2 disarmed the
moment `step_tempo` hit target (a pass at 0.95 promoted to 1.00 and
declared victory without ever playing 1.00).

```rust
pub struct RampConfig {
    pub start_tempo: f32,        // default 0.70
    pub target_tempo: f32,       // default 1.00
    pub step: f32,               // default 0.05
    pub threshold_pct: f32,      // default 90.0
    pub required_successes: u8,  // default 1
}

pub struct RampState {           // only meaningful while armed
    pub armed: bool,
    pub step_tempo: f32,
    pub success_streak: u8,
    pub fail_streak: u8,
}
```

Decision per completed loop pass (pure function, unit-tested):

```
pass (accuracy ≥ threshold):
    success_streak += 1; fail_streak = 0
    if success_streak >= required_successes:
        success_streak = 0
        if step_tempo >= target:  → Complete
              (user_tempo := target, disarm)
        else: step_tempo = min(step_tempo + step, target)  → StepUp

fail:
    fail_streak += 1; success_streak = 0
    if fail_streak >= 2:
        fail_streak = 0
        step_tempo = max(step_tempo - step, start)  → StepDown
    else: → Hold ("one more fail steps down")
```

Accuracy stays GITADORA `achievement_pct` (Perfect 100 / Great 80 /
Good 60 / Poor 40 / Miss 0) with the 90% default threshold —
deliberate: the gate grades timing quality, not just hit rate.

### 4. Session state split

`PracticeSession` stops being a flat bag; fields group by owner so
future features (segments, trainers) have a home:

```rust
pub struct PracticeSession {
    pub transport: PracticeTransport,   // user_tempo, snap, preroll,
                                        // loop_region, scrub_cursor_ms
    pub trainer: PracticeTrainer,       // ramp_config, ramp: RampState
    pub current_attempt: AttemptStats,
    pub attempt_history: Vec<AttemptRecord>,  // cap 20, as today
}
```

Pure refactor plus the new fields above — behavior changes come from
§§1–3, not the move. `PracticeSession`-present-means-practice stays
the mode flag. Domain logic stays a module (`practice/`); no new
crate yet.

### 5. Stat-attribution fixes (verified bugs)

1. **`NoteMissed` gains `chip_idx: usize`** (emitter: `scroll.rs`).
   Practice stats then exclude pre-roll misses with the same
   `judge_ms >= attempt.start_ms` check used for judgments (today
   *every* miss is attributed to the attempt, including pre-roll
   misses when pre-roll chips are judgeable).
2. **Overhits tracked.** `track_attempt_stats` reads `EmptyHit`;
   `AttemptStats`/`AttemptRecord` gain `overhits: u32` (per-lane
   split parked). Overhits do NOT affect accuracy or the ramp gate in
   v3 — recorded and displayed only.
3. **Snapped-seek attempt start.** `timeline_ui.rs` click-seek already
   computes `snapped = timeline.resolve_snap(...)` for the cursor but
   sends `attempt_start_ms: None` with the *unsnapped* target →
   attempt start ≠ playback start; chips in the gap are wrongly
   excluded as pre-roll. Fix: send `attempt_start_ms: Some(snapped)`.

### 6. Attempt history: span-keyed display

History stays a flat capped list, but consumers filter by span:

- Full-HUD attempt list shows only attempts whose `start_ms` matches
  the current loop region's start (implicit region included). No
  loop-region change mid-session pollutes the visible comparison set.
- Free-play (no explicit loop) shows attempts of the implicit region.
- The ramp reads the attempt finalized by the triggering
  `PracticeLoopCompleted` (§3), so it needs no separate filter.

### 7. UX changes

**Loop-wrap micro-report (new).** At each loop completion, one toast
(existing `ToastQueue`, 1.5 s):

```
pass 5 · 93.8% · 3 miss · +18ms
```

(pass counter = attempts on this span this session; signed mean error
with `+` = late, `−` = early). Feedback lands at the loop boundary,
never mid-play.

**Widget defaults in practice.** `dtx_layout::scene::default_instance`
visibility `(vis_play, vis_practice)` changes:

| Widget        | play | practice | note                        |
|---------------|------|----------|-----------------------------|
| ScorePanel    | ✓    | ✗        | score meaningless in room   |
| PhraseMeter   | ✓    | ✗        |                             |
| LiveGraph     | ✓    | ✗        |                             |
| SongProgress  | ✓    | ✗        | mini loop-strip covers this |
| all others    | ✓    | ✓        | unchanged                   |

Users can re-enable any of them per-layout in the editor (existing
per-mode visibility flags — this is only a *defaults* change).

**`WidgetKind::PracticeTransport` removed.** v2 replaced the v1
transport widget with fixed overlays; the registry variant is a dead
halfway state. Remove the variant from the enum, `ALL`, and
`default_instance`; layout deserialization must tolerate unknown
widget kinds in saved TOML (skip with a warning) so existing layout
files don't break.

**Visible practice entry.** Song select renders the hint
`[Enter] Play   [Shift+Enter] Practice` (footer/hint area). No new
menu; Shift+Enter stays the mechanism, it just stops being secret.

**Full-HUD rail regroup + rename.** Rows grouped under headers
TRANSPORT (Tempo, Snap, Pre-roll, Restart) / LOOP (A, B, Clear) /
TRAINER (Ramp arm, start, target, step, pass ≥, streak, progress
`3/6`). `Rate` label → `Tempo`. New rail row: `Ramp streak` (edits
`required_successes`, 1–3).

### 8. Testing

Same discipline as v2 (pure functions + headless app tests):

- Ramp protocol v2 pure tests: streaks, pass-at-target completion,
  step-down floor, clamp-on-config-edit, nudge-disarms.
- `PracticeLoopCompleted` only from loop wraps: manual seek / restart
  / arm-seek emit none (headless).
- Implicit loop: no region set → wrap seek at chart end; no
  `TransitionRequest` to Result while `PracticeSession` exists.
- Miss attribution: pre-roll miss excluded via `chip_idx`.
- Overhit counting; snapped-seek attempt start == snapped ms.
- Effective-tempo: disarm restores `user_tempo`; complete graduates.
- Layout TOML with unknown widget kind loads with the widget skipped.
- FixedUpdate ordering guard extended for the new/renamed systems
  (real-schedule build test, per `tests-skip-real-plugin-schedule`).
