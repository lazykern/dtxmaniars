# Play-Speed Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Play Speed honest: it must change note AND audio speed together (NX behavior — playback-frequency change, pitch shifts) instead of silently compressing chart time while audio runs at 1×, and modified-speed plays must never write normal score records.

**Architecture:** The repo already contains the correct machinery: practice tempo scales the kira playback rate AND the gameplay clock together via `AudioRate` (`gameplay clock advances by dt*rate; chart-time math and judge windows stay in chart-ms`). We route `play_speed` through that same path and delete the divergent chart-time-compression path (`chip_target_ms_with_speed` / `t / play_speed`), which today moves drum targets but not BGM auto-chips or audio — the desync the roadmap flags. Practice tempo, when a `PracticeSession` exists, takes over the rate entirely (practice already never writes scores).

**Tech Stack:** Bevy 0.19, bevy_kira_audio 0.26 (existing `set_playback_rate` — a resampling rate change; pitch shifts, exactly like DTXManiaNX's frequency-based nPlaySpeed).

**Source basis (verified 2026-07-11):**
- `ScrollSettings { pixels_per_ms, play_speed }` at `crates/gameplay-drums/src/resources.rs:225-253`; doc at :229-232 states the caveat this plan removes ("audio playback rate does NOT rescale").
- Chart-time compression consumers: `crates/gameplay-drums/src/scroll.rs:125-126` (spawn window) and `crates/gameplay-drums/src/judge.rs:146-158` (`chip_target_ms_with_speed`, `t / play_speed`); math in `crates/dtx-timing/src/lib.rs:95-118` (`chip_time_ms_with_speed`, `chip_time_ms_with_bpm_changes_and_speed`).
- BGM auto-chips use `chip_target_ms` WITHOUT play_speed (`judge.rs:163+`) — the smoking gun: at play_speed≠1 the BGM schedule and drum targets diverge today.
- Config: `GameplayConfig::play_speed: u8` raw `0x0A..0x28`, `play_speed_multiplier(raw) = clamp/20.0` → 0.5×..2.0×, default 1.0× (`crates/dtx-config/src/lib.rs:141-155,212-213`).
- Set into resource at stage entry `crates/gameplay-drums/src/lib.rs:249` and live from draft `crates/gameplay-drums/src/editor/tabs.rs:89`.
- Rate machinery: `AudioRate(pub f64)` (`resources.rs:255-263`); `sync_gameplay_clock` ticks `dt * rate.0` (`lib.rs:286-299`); practice applies rate in `crates/gameplay-drums/src/practice/rate.rs:38-42` (`audio.set_playback_rate(target)` channel-wide + per-instance BGM tween), dedupe via `Local<f64>`; `reset_audio_rate` on `OnExit(Performance)`.
- Practice layered tempo: `PracticeSession::effective_tempo()` (`practice/session.rs:214-222`).
- Settings row "Play Speed" `crates/gameplay-drums/src/editor/settings_data.rs:153-178` — desc currently (wrongly) claims "affects both notes and audio"; this plan makes the claim true.
- Score persistence: ONE write site, `save_result_then_despawn` in `crates/game-results/src/lib.rs:295-343`, already gated on `PracticeSession` presence (:306-309). Also writes `.score.ini` (:345-368).
- Kira `position()` reports source-time, which advances at rate× wall speed — identical to how practice tempo already keeps `measured_ms` and `dt*rate` consistent. No clock changes needed.

**Contract after this plan:** `rate = practice effective_tempo if PracticeSession exists, else play_speed multiplier`. Chart timeline (targets, judge windows, BGM schedule) stays in chart-ms and is never compressed. Audio and clock both run at `rate`. `play_speed != 1.0` plays skip normal score records.

---

### Task 1: Pure rate-selection function

**Files:**
- Modify: `crates/gameplay-drums/src/practice/rate.rs`

- [ ] **Step 1: Write the failing tests**

Append to (or create) the `#[cfg(test)] mod tests` in `practice/rate.rs`:

```rust
#[test]
fn play_speed_drives_rate_outside_practice() {
    assert_eq!(target_rate(1.5, None), 1.5);
    assert_eq!(target_rate(0.5, None), 0.5);
}

#[test]
fn practice_session_overrides_play_speed() {
    let mut session = crate::practice::PracticeSession::default();
    session.transport.user_tempo = 0.7;
    assert_eq!(target_rate(2.0, Some(&session)), 0.7);
}

#[test]
fn defaults_are_unity() {
    assert_eq!(target_rate(1.0, None), 1.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums --lib practice::rate -j 2`
Expected: FAIL — `target_rate` not found.

- [ ] **Step 3: Implement**

In `practice/rate.rs`:

```rust
/// The playback rate for audio AND clock. Practice owns the rate while a
/// session exists (practice never writes scores); otherwise normal-play
/// Play Speed applies. Chart-time math stays in chart-ms either way.
pub fn target_rate(play_speed: f32, session: Option<&super::PracticeSession>) -> f64 {
    match session {
        Some(s) => f64::from(s.effective_tempo()),
        None => f64::from(play_speed),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gameplay-drums --lib practice::rate -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/rate.rs
git commit -m "feat(rate): pure target_rate combining play speed and practice tempo"
```

---

### Task 2: Apply the rate for normal play (generalize `apply_practice_rate`)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/rate.rs`
- Modify: wherever the system is registered (check `crates/gameplay-drums/src/practice/mod.rs:47-67`)

- [ ] **Step 1: Generalize the system**

`apply_practice_rate` currently runs only when a `PracticeSession` exists and computes `session.effective_tempo()`. Change it to:

```rust
/// Applies target_rate() to the kira channel, the BGM instance, and AudioRate.
/// Runs for ALL performance play, not just practice.
pub fn apply_playback_rate(
    scroll: Res<crate::resources::ScrollSettings>,
    session: Option<Res<super::PracticeSession>>,
    mut audio_rate: ResMut<crate::resources::AudioRate>,
    // ...keep the existing audio/bgm-instance params exactly as they are...
    mut last: Local<f64>,
) {
    let target = target_rate(scroll.play_speed, session.as_deref());
    if (*last - target).abs() < f64::EPSILON {
        return;
    }
    *last = target;
    audio_rate.0 = target;
    // existing body: audio.set_playback_rate(target) + BGM instance tween
}
```

Keep the existing body's kira calls untouched; only the target computation and the gating change. Rename the registration site: the system must now be registered `.run_if(in_state(AppState::Performance))` WITHOUT the `PracticeSession`-exists condition (grep the registration in `practice/mod.rs` and drop that run_if; `Option<Res<...>>` handles absence).

`reset_audio_rate` on `OnExit(Performance)` stays as is (menus run at 1×). Note the `Local<f64>` starts at 0.0, so the first Performance frame always applies — correct.

- [ ] **Step 2: Build + existing practice tests**

Run: `cargo check -p gameplay-drums -j 2 && cargo test -p gameplay-drums --test practice_mode -j 2`
Expected: clean / PASS (practice behavior unchanged: with a session, `target_rate` returns exactly what `effective_tempo` returned before).

- [ ] **Step 3: Headless test for normal-play rate**

Add to `crates/gameplay-drums/tests/practice_mode.rs` (reuse its `build_app()` harness; if the harness force-inserts a `PracticeSession`, insert/remove accordingly):

```rust
#[test]
fn play_speed_sets_audio_rate_without_practice_session() {
    let mut app = build_app();
    app.world_mut().remove_resource::<gameplay_drums::practice::PracticeSession>();
    app.world_mut().resource_mut::<gameplay_drums::ScrollSettings>().play_speed = 1.5;
    app.update();
    let rate = app.world().resource::<gameplay_drums::AudioRate>().0;
    assert!((rate - 1.5).abs() < 1e-9, "got {rate}");
}
```

If `ScrollSettings`/`AudioRate` are not re-exported from the crate root, add `pub use resources::{AudioRate, ScrollSettings};` to `crates/gameplay-drums/src/lib.rs` (grep first — `ScrollSettings` may already be exported for tests).

- [ ] **Step 4: Run the new test**

Run: `cargo test -p gameplay-drums --test practice_mode play_speed_sets_audio_rate -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums
git commit -m "feat(play-speed): drive audio+clock rate from play speed in normal play"
```

---

### Task 3: Delete the chart-time compression path

**Files:**
- Modify: `crates/gameplay-drums/src/judge.rs:146-158`
- Modify: `crates/gameplay-drums/src/scroll.rs:125-126`
- Modify: `crates/dtx-timing/src/lib.rs:95-118`
- Modify: `crates/gameplay-drums/src/resources.rs:225-253`

- [ ] **Step 1: Inventory every `_with_speed` caller**

Run: `grep -rn 'chip_target_ms_with_speed\|chip_time_ms_with_speed\|chip_time_ms_with_bpm_changes_and_speed' crates/ --include='*.rs'`
Record all hits. Expected: `judge.rs`, `scroll.rs`, the dtx-timing definitions + their tests, and possibly `dtx-timing/tests/comprehensive.rs`.

- [ ] **Step 2: Replace call sites with the plain functions**

- `judge.rs`: `chip_target_ms_with_speed(chip, base_bpm, timing, play_speed)` → `chip_target_ms(chip, base_bpm, timing)`. Delete the now-unused `play_speed` plumbing into the judge system (params, `Res<ScrollSettings>` if it was only read for this).
- `scroll.rs:125-126`: same substitution for the spawn window. NOTE: `pixels_per_ms` scroll velocity is untouched — visual scroll speed and play speed remain independent settings.
- Delete `chip_target_ms_with_speed` in `judge.rs:163` region if it's a local wrapper (keep `auto_chip_target_ms` — it never had speed).

- [ ] **Step 3: Remove the dead math in dtx-timing**

Delete `chip_time_ms_with_speed` and `chip_time_ms_with_bpm_changes_and_speed` (`dtx-timing/src/lib.rs:95-118`) and their unit tests, plus any coverage in `dtx-timing/tests/comprehensive.rs` (from Step 1's inventory). If anything outside gameplay-drums still calls them, STOP and reassess (Step 1 said no).

- [ ] **Step 4: Update the `ScrollSettings` contract doc**

Replace the doc comment at `resources.rs:229-232` with:

```rust
/// Playback-speed multiplier (nPlaySpeed/20.0; 1.0 = native). Applied as an
/// audio+clock rate (see practice::rate::target_rate): audio pitch shifts,
/// chart-time math and judge windows stay in chart-ms. Never compresses the
/// chart timeline. Modified speeds skip normal score records.
pub play_speed: f32,
```

- [ ] **Step 5: Build + full package tests**

Run: `cargo test -p dtx-timing -p gameplay-drums -j 2`
Expected: PASS. Timing tests that asserted compressed targets die with the functions they tested; every remaining test must be green with NO chart-time change at any play_speed.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums crates/dtx-timing
git commit -m "refactor(play-speed): remove chart-time compression path"
```

---

### Task 4: Modified-speed plays never write normal scores

**Files:**
- Modify: `crates/game-results/src/lib.rs` (`save_result_then_despawn`, :295-343)
- Modify: `crates/game-results/src/lib.rs` (`spawn_result` stat rows, :145-192)

- [ ] **Step 1: Write the failing pure test**

`game-results` tests pure helpers in-file (lib.rs:374+). Add:

```rust
#[test]
fn persistence_allowed_only_at_native_speed() {
    assert!(persistence_allowed(1.0, false));
    assert!(!persistence_allowed(1.5, false));
    assert!(!persistence_allowed(0.5, false));
    assert!(!persistence_allowed(1.0, true)); // practice always blocks
    // float-safety: the u8 config quantizes to /20 steps; anything off 1.0 is intentional
    assert!(!persistence_allowed(1.05, false));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p game-results -j 2 persistence_allowed`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement the gate**

```rust
/// Normal score records require native speed and non-practice play.
/// (Roadmap: "Keep practice, assists, and modified-speed plays out of
/// normal score records.")
pub fn persistence_allowed(play_speed: f32, practice: bool) -> bool {
    !practice && (play_speed - 1.0).abs() < 0.001
}
```

In `save_result_then_despawn`, add `scroll: Res<gameplay_drums::ScrollSettings>` to the params and replace the existing practice-only early-return (:306-309) with:

```rust
if !persistence_allowed(scroll.play_speed, practice.is_some()) {
    despawn_stage::<ResultEntity>(commands, query);
    return; // no ScoreStore entry, no score.ini update
}
```

This single gate covers both sinks (JSON store at :340-341 and `.score.ini` at :345-368) because both live below it in the same system.

- [ ] **Step 4: Make it visible on the results screen**

In `spawn_result`'s `stat_rows` construction (:145-192), when `(scroll.play_speed - 1.0).abs() >= 0.001`, append a row:

```rust
stat_rows.push((format!("Play Speed {:.2}x — score not saved", scroll.play_speed), 0.0));
```

(`spawn_result` needs the same `Res<gameplay_drums::ScrollSettings>` param added. Hidden persistence-skipping would violate the "honest surface" principle.)

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p game-results -j 2 && cargo check -p game-results -j 2`
Expected: PASS / clean.

- [ ] **Step 6: Commit**

```bash
git add crates/game-results
git commit -m "feat(scoring): modified-speed plays skip normal score records, visibly"
```

---

### Task 5: Honest settings row text

**Files:**
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs:153-178`

- [ ] **Step 1: Update the description**

The row desc currently claims "affects both notes and audio" — which was false and is now true, but incomplete. Set it to:

```
Speed for notes and audio together (audio pitch shifts). Speeds other than 1.00x do not save scores.
```

Keep the stepper mechanics (raw byte clamp `0x0A..0x28`, `{:.2}x` format) untouched.

- [ ] **Step 2: Run settings tests**

Run: `cargo test -p gameplay-drums --lib editor::settings_data -j 2`
Expected: PASS (tests exercise value/adjust closures, not desc strings — still run them).

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/editor/settings_data.rs
git commit -m "docs(settings): accurate play speed description"
```

---

### Task 6: End-to-end sanity + manual audio check

- [ ] **Step 1: Full test sweep**

Run: `cargo test -p gameplay-drums -p dtx-timing -p game-results -p dtx-config -j 2`
Expected: PASS, including `fixed_update_schedule_ordering`.

- [ ] **Step 2: Manual check (audio sync is not headlessly assertable)**

Launch `dtxmaniars` (bevy-brp). Set Play Speed 1.50x in Customize → Gameplay. Play a chart with BGM:
- Audio audibly faster and higher-pitched; notes arrive in sync with it (no drift over a full song).
- Drum keysounds also pitch-shifted (channel-wide rate — inherited).
- Results screen shows "Play Speed 1.50x — score not saved"; `scores.json` mtime unchanged, no new `.score.ini` write.
- Set back to 1.00x; play; score saves normally.
- Enter practice mode at Play Speed 1.50x: practice tempo controls (0.5–1.5) behave exactly as before (session overrides play speed).

- [ ] **Step 3: Commit any fixups, then run scoped fmt**

Run: `cargo fmt -p gameplay-drums -p dtx-timing -p game-results -- <changed files>` (scoped — never bare `cargo fmt --all` unless the toolchain-pinning plan has landed).

---

## Failure-handling mapping (roadmap)

- "players cannot create audio/chart desync" → Tasks 2-3 (single rate path; compression deleted).
- "Keep ... modified-speed plays out of normal score records" → Task 4.
- Honest surface → Tasks 4 (visible row) and 5 (accurate desc).

## Interaction notes for the executor

- Practice tempo and play speed never multiply: practice replaces (Task 1). This is deliberate — practice already bypasses scores and owns its own tempo UX (roadmap: do not redesign practice).
- Pitch-preserving time-stretch is explicitly out of scope here; it is the separate `2026-07-11-pitch-preserving-practice-tempo.md` plan. If that plan lands a stretch-capable path later, `target_rate` is the single integration point.
