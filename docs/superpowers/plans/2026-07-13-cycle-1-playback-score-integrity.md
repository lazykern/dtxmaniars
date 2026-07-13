# Cycle 1 Playback and Score Integrity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Play Speed advance one authoritative chart clock and prevent modified-speed runs from entering ordinary score history.

**Architecture:** Replace the split `ScrollSettings::play_speed`/`AudioRate` model with one `EffectivePlaybackRate` resource. Chart target times remain unscaled; the clock and every gameplay audio instance consume the rate. Before leaving Performance, gameplay writes a cross-stage `CompletedRunContext`, and Results uses that immutable snapshot as its persistence gate.

**Tech Stack:** Rust 1.95, Bevy 0.19 states/resources/systems, bevy_kira_audio 0.26, Cargo tests.

## Global Constraints

- Complete Cycle 0 before this plan.
- All gameplay coordinates remain chart milliseconds.
- Practice tempo overrides normal Play Speed; the two rates never multiply.
- Pitch changes with speed; pitch-preserving time stretch is out of scope.
- Modified-speed results display normally but write neither the native store nor `score.ini`.
- Do not change CI/CD configuration or workflows.

---

## File map

- Create: `crates/gameplay-drums/src/playback_rate.rs` — rate selection and Kira application
- Modify: `crates/gameplay-drums/src/resources.rs` — `EffectivePlaybackRate`, rate source, scroll-only settings
- Modify: `crates/gameplay-drums/src/lib.rs` — plugin wiring, config application, clock delta
- Modify: `crates/gameplay-drums/src/judge.rs` — remove target-time compression
- Modify: `crates/gameplay-drums/src/scroll.rs` — use unscaled chart targets
- Modify: `crates/gameplay-drums/src/practice/rate.rs` — write practice tempo through the shared rate path
- Modify: `crates/gameplay-drums/src/editor/tabs.rs` — keep live Play Speed edits synchronized
- Modify: `crates/gameplay-drums/src/perf_hotkeys.rs` — scroll-only constructor
- Modify: `crates/gameplay-drums/src/orchestrator.rs` — snapshot rate on clear
- Modify: `crates/gameplay-drums/src/stage_end.rs` — snapshot rate on failure
- Modify: `crates/gameplay-drums/tests/end_to_end_stage.rs` — completed-run integration coverage
- Modify: `crates/gameplay-drums/tests/practice_mode.rs` — override, pause, and seek coverage
- Modify: `crates/game-shell/src/states.rs` — cross-stage run context
- Modify: `crates/game-shell/src/lib.rs` — resource initialization/export
- Modify: `crates/game-results/src/lib.rs` — persistence qualification
- Modify: `crates/game-results/src/ui.rs` — modified-speed explanation

### Task 1: Model one effective playback rate

**Files:**

- Create: `crates/gameplay-drums/src/playback_rate.rs`
- Modify: `crates/gameplay-drums/src/resources.rs:220-270`
- Modify: `crates/gameplay-drums/src/lib.rs:90-125`
- Test: `crates/gameplay-drums/src/resources.rs`

**Interfaces:**

- Consumes: `dtx_config::play_speed_multiplier(u8) -> f32`
- Produces: `PlaybackRateSource`, `EffectivePlaybackRate::{native, normal, practice, scaled_delta_secs}`
- Produces: `playback_rate::initial_playback_rate(play_speed, practice)`
- Produces: `playback_rate::apply_playback_rate(...)`

- [ ] **Step 1: Write failing resource tests**

Add to `resources.rs` tests:

```rust
#[test]
fn effective_rate_defaults_to_native() {
    let rate = EffectivePlaybackRate::default();
    assert_eq!(rate.source, PlaybackRateSource::Native);
    assert!((rate.value - 1.0).abs() < f64::EPSILON);
}

#[test]
fn effective_rate_scales_only_wall_delta() {
    let slow = EffectivePlaybackRate::normal(0.5);
    let fast = EffectivePlaybackRate::normal(2.0);
    assert!((slow.scaled_delta_secs(0.016) - 0.008).abs() < 1e-12);
    assert!((fast.scaled_delta_secs(0.016) - 0.032).abs() < 1e-12);
}

#[test]
fn practice_rate_has_explicit_source() {
    assert_eq!(
        EffectivePlaybackRate::practice(0.75).source,
        PlaybackRateSource::PracticeTempo
    );
}
```

- [ ] **Step 2: Run the tests and confirm the missing types fail**

Run:

```bash
cargo test -p gameplay-drums --lib resources::tests::effective_rate -- --nocapture
```

Expected: compile failure because `EffectivePlaybackRate` and `PlaybackRateSource` do not exist.

- [ ] **Step 3: Replace `AudioRate` and make scroll settings visual-only**

In `resources.rs`, replace the `ScrollSettings` and `AudioRate` definitions with:

```rust
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScrollSettings {
    pub pixels_per_ms: f32,
}

impl ScrollSettings {
    pub const NX_BASE_PIXELS_PER_MS: f32 = 0.17875;

    pub fn from_scroll_speed(multiplier: f32) -> Self {
        Self {
            pixels_per_ms: Self::NX_BASE_PIXELS_PER_MS * multiplier.max(0.1),
        }
    }
}

impl Default for ScrollSettings {
    fn default() -> Self {
        Self::from_scroll_speed(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackRateSource {
    Native,
    NormalPlaySetting,
    PracticeTempo,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct EffectivePlaybackRate {
    pub value: f64,
    pub source: PlaybackRateSource,
}

impl EffectivePlaybackRate {
    pub const fn native() -> Self {
        Self { value: 1.0, source: PlaybackRateSource::Native }
    }

    pub fn normal(value: f64) -> Self {
        let value = value.max(f64::EPSILON);
        if (value - 1.0).abs() < 1e-9 {
            Self::native()
        } else {
            Self { value, source: PlaybackRateSource::NormalPlaySetting }
        }
    }

    pub fn practice(value: f64) -> Self {
        Self { value: value.max(f64::EPSILON), source: PlaybackRateSource::PracticeTempo }
    }

    pub fn scaled_delta_secs(self, wall_delta_secs: f64) -> f64 {
        wall_delta_secs * self.value
    }
}

impl Default for EffectivePlaybackRate {
    fn default() -> Self {
        Self::native()
    }
}
```

Register `EffectivePlaybackRate` instead of `AudioRate` in `gameplay-drums::plugin`, declare `mod playback_rate;`, and delete the old `rate_default_is_native` test/import.

- [ ] **Step 4: Add the shared audio application module**

Create `playback_rate.rs` with these interfaces and systems:

```rust
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::{AppState, PracticeIntent};

use crate::orchestrator::DrumsEnterSet;
use crate::resources::EffectivePlaybackRate;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        configure_playback_rate.before(DrumsEnterSet),
    )
    .add_systems(OnExit(AppState::Performance), reset_playback_rate);
}

pub(crate) fn apply_playback_rate(
    next: EffectivePlaybackRate,
    rate: &mut EffectivePlaybackRate,
    audio: &Audio,
    bgm: &dtx_audio::BgmHandle,
    instances: &mut Assets<AudioInstance>,
) {
    *rate = next;
    audio.set_playback_rate(next.value);
    if let Some(handle) = &bgm.instance
        && let Some(mut instance) = instances.get_mut(handle)
    {
        instance.set_playback_rate(next.value, AudioTween::default());
    }
}

pub(crate) fn initial_playback_rate(
    configured_play_speed: f64,
    practice: bool,
) -> EffectivePlaybackRate {
    if practice {
        EffectivePlaybackRate::practice(1.0)
    } else {
        EffectivePlaybackRate::normal(configured_play_speed)
    }
}

fn configure_playback_rate(
    intent: Res<PracticeIntent>,
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let cfg = dtx_config::load(&dtx_config::default_path());
    let next = initial_playback_rate(
        f64::from(dtx_config::play_speed_multiplier(cfg.gameplay.play_speed)),
        intent.0,
    );
    apply_playback_rate(next, &mut rate, &audio, &bgm, &mut instances);
}

fn reset_playback_rate(
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    apply_playback_rate(
        EffectivePlaybackRate::native(),
        &mut rate,
        &audio,
        &bgm,
        &mut instances,
    );
}
```

Register `playback_rate::plugin(app)` before the orchestrator plugin is registered.
Add unit tests in the module proving `initial_playback_rate(0.75, false)` is
`NormalPlaySetting` at 0.75 and `initial_playback_rate(0.75, true)` is
`PracticeTempo` at 1.0.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p gameplay-drums --lib resources::tests::effective_rate -- --nocapture
```

Expected: all three new tests pass.

- [ ] **Step 6: Commit the rate model**

```bash
git add crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/playback_rate.rs crates/gameplay-drums/src/lib.rs
git commit -m "refactor: introduce effective playback rate"
```

### Task 2: Remove target-time compression and unify practice/editor rate changes

**Files:**

- Modify: `crates/gameplay-drums/src/judge.rs:215-245`
- Modify: `crates/gameplay-drums/src/scroll.rs:115-135`
- Modify: `crates/gameplay-drums/src/lib.rs:330-405`
- Modify: `crates/gameplay-drums/src/practice/rate.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs:80-125`
- Modify: `crates/gameplay-drums/src/perf_hotkeys.rs:165-185`
- Test: `crates/gameplay-drums/src/resources.rs`

**Interfaces:**

- Consumes: `EffectivePlaybackRate` from Task 1
- Produces: unscaled `chip_target_ms` as the only judgment/scroll target function
- Produces: practice/editor changes routed through `apply_playback_rate`

- [ ] **Step 1: Add the accelerated-clock regression test**

Add to the `resources.rs` test module:

```rust
#[test]
fn two_x_rate_reaches_one_second_of_chart_in_half_a_second() {
    let rate = EffectivePlaybackRate::normal(2.0);
    let mut clock = GameplayClock::default();
    clock.start_wall_clock();
    for _ in 0..30 {
        clock.tick(rate.scaled_delta_secs(1.0 / 60.0), None);
    }
    assert!((clock.current_ms - 1_000).abs() <= 1, "got {}", clock.current_ms);
}

#[test]
fn measured_chart_position_is_not_scaled_twice() {
    let rate = EffectivePlaybackRate::normal(2.0);
    let mut clock = GameplayClock::default();
    clock.start_audio_required();
    clock.tick(rate.scaled_delta_secs(1.0 / 60.0), Some(500));
    assert_eq!(clock.current_ms, 500);
}
```

In `judge.rs`, add a regression using one BPM/bar-aware chip target and rates
0.5, 1.0, and 2.0. Assert `chip_target_ms` stays identical while the implied
wall duration `target_ms as f64 / rate.value` changes; the chart target function
must accept no playback-rate argument.

- [ ] **Step 2: Run the regression test**

Run:

```bash
cargo test -p gameplay-drums --lib resources::tests::two_x_rate -- --nocapture
```

Expected: pass once Task 1 exists; this locks the intended clock contract before call sites change.

- [ ] **Step 3: Remove chart-time scaling**

Delete `chip_target_ms_with_speed` from `judge.rs`. In `scroll.rs`, replace its call with:

```rust
let target_ms = crate::judge::chip_target_ms(chip, base_bpm, timing);
```

Remove `play_speed` from `ScrollSettings`, its constructor, comments, imports, and every assignment. Use:

```rust
*scroll = ScrollSettings::from_scroll_speed(g.scroll_speed);
```

in `editor/tabs.rs`, and:

```rust
*scroll = ScrollSettings::from_scroll_speed(draft.cfg.gameplay.scroll_speed);
```

in `perf_hotkeys.rs`.

- [ ] **Step 4: Advance the clock through `EffectivePlaybackRate`**

Change `sync_gameplay_clock` to consume `Res<EffectivePlaybackRate>` and tick with:

```rust
gameplay_clock.tick(rate.scaled_delta_secs(time.delta_secs_f64()), chart_ms);
```

Remove `play_speed_multiplier` from `apply_config_on_enter`; that system now configures only visual scroll velocity and the other existing runtime settings.

- [ ] **Step 5: Route practice and live editor changes through the shared path**

Replace `practice/rate.rs`'s direct `AudioRate` logic with:

```rust
fn apply_practice_rate(
    session: Res<PracticeSession>,
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut applied: Local<f64>,
) {
    let target = f64::from(session.effective_tempo());
    if (*applied - target).abs() < 1e-9 {
        return;
    }
    *applied = target;
    crate::playback_rate::apply_playback_rate(
        EffectivePlaybackRate::practice(target),
        &mut rate,
        &audio,
        &bgm,
        &mut instances,
    );
}
```

Remove its OnExit reset because `playback_rate.rs` owns reset. In `editor/tabs.rs`, add `ResMut<EffectivePlaybackRate>` and call the same helper with `EffectivePlaybackRate::normal(...)` whenever the draft Play Speed changes, using the existing `Audio`, `BgmHandle`, and `Assets<AudioInstance>` parameters.

- [ ] **Step 6: Run gameplay tests and compile all call sites**

The module-level `initial_playback_rate` test covers practice overriding the
normal setting. Extend `tests/practice_mode.rs` with two system-level cases: set
`EffectivePlaybackRate::practice(0.75)`, transition PauseState Running → Paused
→ Running, and assert the value/source never reset; then issue
`SeekToChartTime { target_ms: 9_000, snap: None, attempt_start_ms: None }` at
0.75x and assert `GameplayClock.current_ms == 9_000` and the queued fallback
BGM offset is exactly `9.0` seconds, not divided or multiplied by the rate.
Add a restart case that seeks to chart time 0 at 0.75x and asserts both the
effective rate remains 0.75 and the queued BGM offset is 0 seconds.

Run:

```bash
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test practice_mode
cargo test -p gameplay-drums --test play_chart
cargo check --workspace
```

Expected: all commands exit 0 and no reference to the legacy rate paths remains.

Confirm with:

```bash
rg -n "AudioRate|chip_target_ms_with_speed|scroll\.play_speed|ScrollSettings::new" crates
```

Expected: no matches.

- [ ] **Step 7: Commit unified timing**

```bash
git add crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/scroll.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/practice/rate.rs crates/gameplay-drums/src/editor/tabs.rs crates/gameplay-drums/src/perf_hotkeys.rs crates/gameplay-drums/src/resources.rs
git commit -m "fix: synchronize play speed with chart audio"
```

### Task 3: Snapshot completed-run qualification across stages

**Files:**

- Modify: `crates/game-shell/src/states.rs:90-115`
- Modify: `crates/game-shell/src/lib.rs:15-45`
- Modify: `crates/gameplay-drums/src/orchestrator.rs:395-455`
- Modify: `crates/gameplay-drums/src/stage_end.rs:60-95`
- Modify: `crates/gameplay-drums/tests/end_to_end_stage.rs`

**Interfaces:**

- Consumes: `EffectivePlaybackRate`
- Produces: `game_shell::CompletedRunContext { kind, playback_rate }`
- Produces: `RunKind::{Practice, Normal}`

- [ ] **Step 1: Add failing cross-stage context tests**

Add to `game-shell/src/states.rs` tests:

```rust
#[test]
fn completed_run_defaults_to_safe_non_saving_practice() {
    let run = CompletedRunContext::default();
    assert_eq!(run.kind, RunKind::Practice);
    assert!((run.playback_rate - 1.0).abs() < f64::EPSILON);
}

#[test]
fn normal_run_records_its_rate() {
    let run = CompletedRunContext::normal(0.75);
    assert_eq!(run.kind, RunKind::Normal);
    assert!((run.playback_rate - 0.75).abs() < f64::EPSILON);
}
```

- [ ] **Step 2: Run and observe missing types**

Run:

```bash
cargo test -p game-shell completed_run -- --nocapture
```

Expected: compile failure because the context types do not exist.

- [ ] **Step 3: Implement and register the context**

Add to `states.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunKind {
    #[default]
    Practice,
    Normal,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CompletedRunContext {
    pub kind: RunKind,
    pub playback_rate: f64,
}

impl CompletedRunContext {
    pub fn normal(playback_rate: f64) -> Self {
        Self { kind: RunKind::Normal, playback_rate }
    }
}

impl Default for CompletedRunContext {
    fn default() -> Self {
        Self { kind: RunKind::Practice, playback_rate: 1.0 }
    }
}
```

Export both types from `game-shell/src/lib.rs` and initialize `CompletedRunContext` in `GameShellPlugin`.

- [ ] **Step 4: Snapshot both clear and failure paths**

Add `Res<EffectivePlaybackRate>` and `ResMut<CompletedRunContext>` to `detect_end_of_stage` and `detect_stage_failure`. Immediately before each `request_transition(... StageClear/StageFailed)` call, write:

```rust
*completed_run = game_shell::CompletedRunContext::normal(rate.value);
```

Practice and editor paths are already gated before those writes, so only a real normal run can produce a normal completed context.

- [ ] **Step 5: Extend the end-to-end test app and assertions**

Initialize `EffectivePlaybackRate` and `CompletedRunContext` in `build_app`. Add:

```rust
#[test]
fn completed_run_snapshots_modified_rate_before_clear() {
    let mut app = build_app();
    app.world_mut().resource_mut::<ActiveChart>().chart = chart_with_measures(2);
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::normal(0.75);
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Performance);
    app.update();
    let end_ms = app.world().resource::<DrumsStageCompletion>().chart_end_ms;
    let mut clock = app.world_mut().resource_mut::<GameplayClock>();
    clock.start();
    clock.sync(Some(end_ms));
    app.update();
    let run = app.world().resource::<game_shell::CompletedRunContext>();
    assert_eq!(run.kind, game_shell::RunKind::Normal);
    assert!((run.playback_rate - 0.75).abs() < f64::EPSILON);
}
```

Add the analogous gauge-failure test in `stage_end.rs`: initialize a modified
rate, mark the gauge failed, run `detect_stage_failure`, and assert the same
normal/rate snapshot is written before the StageFailed request.

- [ ] **Step 6: Run cross-stage tests**

Run:

```bash
cargo test -p game-shell completed_run -- --nocapture
cargo test -p gameplay-drums --test end_to_end_stage -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 7: Commit the completed-run snapshot**

```bash
git add crates/game-shell/src/states.rs crates/game-shell/src/lib.rs crates/gameplay-drums/src/orchestrator.rs crates/gameplay-drums/src/stage_end.rs crates/gameplay-drums/tests/end_to_end_stage.rs
git commit -m "feat: snapshot completed run qualification"
```

### Task 4: Block modified score persistence and explain it

**Files:**

- Modify: `crates/game-results/src/lib.rs:20-230`
- Modify: `crates/game-results/src/ui.rs:330-410`
- Test: `crates/game-results/src/lib.rs`
- Test: `crates/game-results/src/ui.rs`

**Interfaces:**

- Consumes: `CompletedRunContext` and `RunKind` from Task 3
- Produces: `SaveStatus::ModifiedSpeed { rate: f64 }`
- Preserves: native 1.0x persistence exactly once

- [ ] **Step 1: Add failing persistence tests**

Refactor the existing result-world setup into a helper that inserts `CompletedRunContext::normal(rate)`. Add:

```rust
#[test]
fn save_result_skips_modified_speed_native_and_score_ini_writes() {
    use bevy::ecs::system::RunSystemOnce;

    let dir = std::env::temp_dir().join(format!("dtxmaniars-modified-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let chart_path = dir.join("chart.dtx");
    std::fs::write(&chart_path, b"#TITLE: Modified\n#00113: 01\n").unwrap();
    let mut world = result_world(Some(chart_path.clone()), 0.75);
    world.run_system_once(save_result).expect("save_result runs");

    assert!(world.resource::<ScoreStoreResource>().entries.is_empty());
    assert_eq!(
        *world.resource::<SaveStatus>(),
        SaveStatus::ModifiedSpeed { rate: 0.75 }
    );
    assert!(!dtx_scoring::score_ini::score_ini_path(&chart_path).exists());
    std::fs::remove_dir_all(dir).unwrap();
}
```

- [ ] **Step 2: Run the new test and observe the failure**

Run:

```bash
cargo test -p game-results save_result_skips_modified -- --nocapture
```

Expected: compile failure because `ModifiedSpeed` does not exist, or assertion failure because a store entry is inserted.

- [ ] **Step 3: Gate persistence before constructing any score record**

Change `SaveStatus` to derive `PartialEq` rather than `Eq` and add:

```rust
ModifiedSpeed { rate: f64 },
```

Replace the `PracticeSession` save gate with `Res<CompletedRunContext>` and start `save_result` with:

```rust
if run.kind == game_shell::RunKind::Practice {
    *status = SaveStatus::Practice;
    return;
}
if (run.playback_rate - 1.0).abs() >= 1e-9 {
    *status = SaveStatus::ModifiedSpeed { rate: run.playback_rate };
    return;
}
```

The early return must occur before `native_score_entry`, `store.add`, `store.save`, and `write_result`.

- [ ] **Step 4: Render the modified-speed status**

Add the UI match arm:

```rust
SaveStatus::ModifiedSpeed { rate } => {
    right.spawn(reveal_text(
        &format!("{rate:.2}× play speed — result not saved as a normal record"),
        Theme::font(14.0),
        t.text_secondary,
        SLOT_SAVE,
    ));
}
```

Add a UI test that inserts `SaveStatus::ModifiedSpeed { rate: 0.75 }`, runs `spawn_result`, and asserts the exact status string is present.

- [ ] **Step 5: Verify both modified and native behavior**

Run:

```bash
cargo test -p game-results --lib -- --nocapture
```

Expected: the modified-speed test proves zero store entries and no `score.ini`; the existing native-speed test proves one entry and `SaveStatus::Saved`; UI tests pass.

- [ ] **Step 6: Commit score integrity**

```bash
git add crates/game-results/src/lib.rs crates/game-results/src/ui.rs
git commit -m "fix: keep modified-speed runs out of normal scores"
```

### Task 5: Run the Cycle 1 regression gate

**Files:**

- Test: all Cycle 1 crates and workspace

**Interfaces:**

- Consumes: Tasks 1-4
- Produces: verified Cycle 1 deliverable

- [ ] **Step 1: Run focused and workspace verification**

Run:

```bash
cargo test -p game-shell
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --tests
cargo test -p game-results
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: every command exits 0.

- [ ] **Step 2: Inspect the final diff and prohibited legacy paths**

Run:

```bash
git diff --check
rg -n "AudioRate|chip_target_ms_with_speed|scroll\.play_speed|ScrollSettings::new" crates
git status --short
```

Expected: no diff errors, no legacy-rate matches, and no uncommitted Cycle 1 files.
