# Practice Mode Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the paused practice rail with a mandatory Setup/Settings workflow, non-judged live preview, chart-specific saved presets, separate pause semantics, and a Progress view built on completed loop attempts.

**Architecture:** Practice remains inside the loaded Performance stage. `game-shell` carries a dependency-neutral request seed and origin, `dtx-config` owns the versioned preset registry, and `gameplay-drums` owns draft, flow, preview, runtime mechanics, and UI. Setup/Editing reuse the real playfield and chart clock while explicit run conditions prevent judgment, scoring, misses, attempts, wait halts, and ramp evaluation.

**Tech Stack:** Rust 1.95+, Bevy 0.19, bevy_kira_audio 0.26, TOML/Serde, dtx-persistence atomic writes, dtx-config, game-shell, game-menu, game-results, gameplay-drums, and dtx-ui.

## Global Constraints

- Preserve DTXManiaNX-derived timing, judgment, scoring, lane, scroll, audio-clock, and chart semantics under ADR-0010.
- Every practice request opens Setup before a practice attempt starts.
- Preview starts stopped and cannot emit judgments, misses, score, combo, gauge changes, attempt data, lane diagnosis, wait halts, or ramp decisions.
- `Pause -> Resume` restores the exact frozen position. `Practice Settings -> Continue Practice` starts a fresh attempt from pre-roll.
- Saved presets use `CONFIG_DIR/practice-presets.toml`, schema version 1, and atomic replacement through dtx-persistence.
- Presets are keyed by canonical chart hash plus selected difficulty index. The source path is display/recovery metadata only.
- Trainer mode is exactly one of Off, Wait, or Ramp. Preview never runs trainer behavior.
- Only complete eligible loop attempts enter Progress or ramp evaluation.
- Standard, Large, and Extra Large text must remain usable. Selection and critical state cannot depend on color alone.
- Use existing 300 ms OutQuint application transitions. Setup-local motion must obey Reduce Motion.
- No `unwrap()` in `crates/*`. Use `thiserror` in Pure/library crates.
- Do not edit `references/` or add AI co-author trailers.
- Keep Bevy dynamic linking dev-only. Do not change linker, rustflags, toolchain, or shared-target cleanup.

---

## File structure

- Create `crates/dtx-config/src/practice.rs`: pure preset schema, validation, versioned load/save, last-used snapshot, and atomic persistence.
- Modify `crates/dtx-config/src/lib.rs` and `Cargo.toml`: export the practice registry and depend on dtx-persistence.
- Modify `crates/game-shell/src/states.rs` and `lib.rs`: practice request origin and constructors.
- Modify `crates/game-menu/src/song_select.rs` and `crates/game-results/src/input.rs`: create origin-aware practice requests.
- Create `crates/gameplay-drums/src/practice/draft.rs`: gameplay-neutral draft conversion and validation against `ChipTimeline`.
- Create `crates/gameplay-drums/src/practice/flow.rs`: Setup/Running/Editing state, preview state, edit snapshot, attempt eligibility, and flow reducers.
- Create `crates/gameplay-drums/src/practice/presets.rs`: Bevy resource and typed preset commands/results around dtx-config.
- Create `crates/gameplay-drums/src/practice/preview.rs`: preview transport, clock/audio ownership, loop wrapping, and frozen-position restoration.
- Replace `crates/gameplay-drums/src/practice/hud/full_hud.rs` with focused `setup.rs`, `setup_controls.rs`, and `progress.rs` modules. Keep `timeline_ui.rs` for gesture math.
- Modify `crates/gameplay-drums/src/practice/session.rs`, `mod.rs`, `actions.rs`, `stats.rs`, `ramp.rs`, `wait.rs`, and `ab_loop.rs`: trainer enum, eligibility, flow entry/commit, and existing mechanic integration.
- Modify `crates/gameplay-drums/src/lib.rs`, `judge.rs`, `scroll.rs`, `score.rs`, `gauge.rs`, `hit_sound.rs`, `menu_nav.rs`, and `pause.rs`: clock/gameplay gates, preview-safe note cleanup, menu context, and separate pause actions.
- Modify `crates/gameplay-drums/tests/practice_mode.rs` and `practice_hud.rs`: lifecycle, preview, UI, input, persistence, and pause coverage.
- Modify `docs/data-and-persistence.md`, `docs/player-guide.md`, `docs/roadmap.md`, and the practice sections of current behavior/user-story notes.

### Task 1: Versioned practice preset registry

**Files:**
- Create: `crates/dtx-config/src/practice.rs`
- Modify: `crates/dtx-config/src/lib.rs`
- Modify: `crates/dtx-config/Cargo.toml`
- Test: `crates/dtx-config/src/practice.rs`

**Interfaces:**
- Consumes: `dtx_persistence::replace_bytes`, Serde, TOML, and the config-directory path convention from `dtx_config::default_path()`.
- Produces: `PracticeChartKey`, `PracticePresetConfig`, `PracticeTrainerPreset`, `PracticePreset`, `PracticePresetRegistry`, `PracticePresetStartup`, `PracticePresetError`, `practice_presets_path()`, `load_practice_presets()`, and `save_practice_presets()`.

- [ ] **Step 1: Add failing schema and path tests**

~~~rust
#[test]
fn practice_key_separates_difficulties() {
    let basic = PracticeChartKey::new("dtx1:abc", 0);
    let extreme = PracticeChartKey::new("dtx1:abc", 2);
    assert_ne!(basic, extreme);
}

#[test]
fn preset_registry_round_trips_every_field() {
    let key = PracticeChartKey::new("dtx1:abc", 2);
    let config = PracticePresetConfig {
        loop_start_ms: Some(43_200),
        loop_end_ms: Some(51_400),
        snap: PracticeSnapPreset::Bar,
        tempo: 0.8,
        preroll: PracticePrerollPreset::OneBar,
        count_in: true,
        trainer: PracticeTrainerPreset::Ramp(RampPreset {
            start_tempo: 0.7,
            target_tempo: 1.0,
            step: 0.05,
            threshold_pct: 90.0,
            required_successes: 1,
        }),
    };
    let mut registry = PracticePresetRegistry::default();
    let id = registry
        .create(key.clone(), Some("Chorus"), None, config.clone())
        .expect("valid preset");
    let raw = toml::to_string_pretty(&registry).expect("serialize");
    let decoded: PracticePresetRegistry = toml::from_str(&raw).expect("parse");
    assert_eq!(decoded.preset(id).expect("saved").config, config);
    assert_eq!(decoded.presets_for(&key).count(), 1);
}

#[test]
fn corrupt_or_newer_file_is_preserved_as_read_only() {
    let path = test_path("newer");
    std::fs::write(&path, "version = 99\n").expect("fixture");
    let before = std::fs::read(&path).expect("bytes");
    assert!(matches!(
        load_practice_presets(&path),
        PracticePresetStartup::ReadOnly { .. }
    ));
    assert_eq!(std::fs::read(&path).expect("preserved"), before);
}
~~~

- [ ] **Step 2: Run the tests and confirm the missing module failure**

Run: `cargo test -p dtx-config practice`

Expected: FAIL because `dtx_config::practice` and its types do not exist.

- [ ] **Step 3: Implement the registry and transactional mutations**

Create the following public model in `practice.rs`:

~~~rust
pub const PRACTICE_PRESET_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PracticeChartKey {
    pub canonical_chart_hash: String,
    pub difficulty: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticeSnapPreset { Bar, Beat, HalfBeat }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticePrerollPreset { OneBar, TwoSeconds, Off }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RampPreset {
    pub start_tempo: f32,
    pub target_tempo: f32,
    pub step: f32,
    pub threshold_pct: f32,
    pub required_successes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticeTrainerPreset { Off, Wait, Ramp(RampPreset) }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePresetConfig {
    pub loop_start_ms: Option<i64>,
    pub loop_end_ms: Option<i64>,
    pub snap: PracticeSnapPreset,
    pub tempo: f32,
    pub preroll: PracticePrerollPreset,
    pub count_in: bool,
    pub trainer: PracticeTrainerPreset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePreset {
    pub id: u64,
    pub chart: PracticeChartKey,
    pub name: Option<String>,
    pub source_path_hint: Option<PathBuf>,
    pub config: PracticePresetConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastUsedPractice {
    pub chart: PracticeChartKey,
    pub source_path_hint: Option<PathBuf>,
    pub config: PracticePresetConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePresetRegistry {
    pub version: u32,
    pub next_id: u64,
    #[serde(default)] pub presets: Vec<PracticePreset>,
    #[serde(default)] pub last_used: Vec<LastUsedPractice>,
}
~~~

Implement `create`, `update`, `delete`, `preset`, `presets_for`, `last_used`, and `set_last_used`. Validate trimmed optional names with a 48-character limit, reject control characters, and reject case-insensitive duplicate player names within one `PracticeChartKey`. Validate finite tempo/ramp values, positive bounded loops when set, and paired `loop_start_ms`/`loop_end_ms` options.

`load_practice_presets` returns `Ready(default)` for a missing file, `Ready(parsed)` for version 1, and `ReadOnly { registry: default(), error }` for read/parse/version/validation errors. `save_practice_presets` serializes a validated clone and calls `replace_bytes`; callers replace their in-memory registry only after success. Derive `practice-presets.toml` by replacing the filename from `default_path()`.

Export the module/types from `lib.rs`. Add `dtx-persistence = { workspace = true }` to `dtx-config`.

- [ ] **Step 4: Verify registry behavior**

Run: `cargo fmt --all && cargo test -p dtx-config`

Expected: PASS, including atomic round-trip, missing file, corrupt file, newer version, duplicate name, invalid bounds, update/delete, and Last Used tests.

- [ ] **Step 5: Commit**

~~~bash
git add crates/dtx-config/Cargo.toml crates/dtx-config/src/lib.rs crates/dtx-config/src/practice.rs Cargo.lock
git commit -m "feat(config): persist practice presets"
~~~

### Task 2: Origin-aware practice requests

**Files:**
- Modify: `crates/game-shell/src/states.rs`
- Modify: `crates/game-shell/src/lib.rs`
- Modify: `crates/game-menu/src/song_select.rs`
- Modify: `crates/game-results/src/lib.rs`
- Modify: `crates/game-results/src/input.rs`
- Test: `crates/game-shell/src/states.rs`
- Test: `crates/game-results/src/input.rs`

**Interfaces:**
- Consumes: existing `PracticeRecommendation`, `PracticeIntent`, SongLoading transition, and Results `CompletedRunContext`.
- Produces: `PracticeOrigin`, `PracticeSeed`, `PracticeRequest`, `PracticeIntent::{manual,recommended,request}`, and a one-shot `ResultReturnState` that prevents duplicate result persistence on Setup cancellation.

- [ ] **Step 1: Write failing origin tests**

~~~rust
#[test]
fn manual_request_remembers_song_select_origin() {
    let intent = PracticeIntent::manual(PracticeOrigin::SongSelect);
    assert_eq!(intent.request().expect("request").origin, PracticeOrigin::SongSelect);
    assert!(matches!(intent.request().expect("request").seed, PracticeSeed::Manual));
}

#[test]
fn result_practice_action_keeps_results_origin() {
    let intent = practice_intent_for_result(Some(recommendation()));
    assert_eq!(intent.request().expect("request").origin, PracticeOrigin::Results);
    assert!(matches!(intent.request().expect("request").seed, PracticeSeed::Recommended(_)));
}

#[test]
fn returning_from_setup_skips_result_processing_once() {
    let mut state = ResultReturnState { available: true, skip_processing_once: true };
    assert!(!should_process_result(&state));
    finish_result_entry(&mut state);
    assert!(should_process_result(&state));
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p game-shell -p game-results origin`

Expected: FAIL because origin-aware request types are absent.

- [ ] **Step 3: Replace the intent payload without adding gameplay dependencies**

Use the following shell API:

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeOrigin { SongSelect, Results, NormalPause }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PracticeSeed { Manual, Recommended(PracticeRecommendation) }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeRequest {
    pub origin: PracticeOrigin,
    pub seed: PracticeSeed,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResultReturnState {
    pub available: bool,
    pub skip_processing_once: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub enum PracticeIntent {
    #[default] None,
    Request(PracticeRequest),
}
~~~

Add `manual(origin)`, `recommended(origin, recommendation)`, `is_requested`, `request`, and `recommendation` helpers. Export all new types from `game-shell::lib` and initialize `ResultReturnState` in `GameShellPlugin`.

Song Select creates `PracticeIntent::manual(PracticeOrigin::SongSelect)`. Results creates either manual or recommended intent with `PracticeOrigin::Results`. Retry keeps the existing intent. Normal play continues to set `None`.

On a fresh Result entry, snapshot analysis and save as today, then set `ResultReturnState.available = true`. If `skip_processing_once` is true, preserve existing Result resources, skip analysis/save, clear only that flag, and respawn the Result UI. Song Select entry clears both fields. This makes Results-origin Setup cancellation idempotent.

- [ ] **Step 4: Verify all request consumers**

Run: `cargo fmt --all && cargo test -p game-shell -p game-menu -p game-results -p gameplay-drums`

Expected: PASS after mechanically updating existing pattern matches in `gameplay-drums::practice` and playback-rate tests to use `intent.recommendation()`.

- [ ] **Step 5: Commit**

~~~bash
git add crates/game-shell/src/states.rs crates/game-shell/src/lib.rs crates/game-menu/src/song_select.rs crates/game-results/src/lib.rs crates/game-results/src/input.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/playback_rate.rs
git commit -m "refactor(practice): track request origin"
~~~

### Task 3: Practice draft, trainer mode, and flow reducers

**Files:**
- Create: `crates/gameplay-drums/src/practice/draft.rs`
- Create: `crates/gameplay-drums/src/practice/flow.rs`
- Modify: `crates/gameplay-drums/src/practice/session.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs`
- Modify: `crates/gameplay-drums/src/practice/ramp.rs`
- Modify: `crates/gameplay-drums/src/practice/wait.rs`
- Test: the new modules and `session.rs`

**Interfaces:**
- Consumes: `PracticeSession`, `ChipTimeline`, `PracticeRequest`, config preset enums, and existing ramp/wait state.
- Produces: `PracticeDraft`, `PracticeDraftSource`, `PracticeTrainerMode`, `PracticeFlow`, `PracticePhase`, `PreviewState`, `PracticeEditSnapshot`, `ValidatedDraft`, and run conditions `practice_running`, `practice_surface_open`, `gameplay_input_active`, and `chart_clock_active`.

- [ ] **Step 1: Write failing reducer and conversion tests**

~~~rust
#[test]
fn wait_and_ramp_are_one_mode() {
    let mut draft = PracticeDraft::default();
    draft.set_trainer_mode(PracticeTrainerMode::Wait);
    assert_eq!(draft.trainer_mode(), PracticeTrainerMode::Wait);
    draft.set_trainer_mode(PracticeTrainerMode::Ramp);
    assert_eq!(draft.trainer_mode(), PracticeTrainerMode::Ramp);
}

#[test]
fn invalid_bounds_fall_back_to_whole_song() {
    let mut draft = PracticeDraft::default();
    draft.loop_region = Some(LoopRegion { start_ms: 4_000, end_ms: 4_000 });
    let validated = draft.validate(&timeline()).expect("recoverable draft");
    assert_eq!(validated.draft.loop_region, None);
    assert!(validated.warning.is_some());
}

#[test]
fn opening_settings_marks_current_pass_ineligible() {
    let mut session = PracticeSession::default();
    let flow = PracticeFlow::running();
    let (flow, snapshot) = flow.open_settings(2_500, &mut session);
    assert_eq!(flow.phase, PracticePhase::Editing);
    assert!(!session.current_attempt_eligible);
    assert_eq!(snapshot.chart_ms, 2_500);
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums practice::`

Expected: FAIL because draft/flow/trainer-mode types are absent.

- [ ] **Step 3: Implement pure state and conversions**

Add:

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeTrainerMode { Off, Wait, Ramp }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticePhase { Setup, Running, Editing }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewState { Stopped, Playing }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeDraftSource {
    WholeSong,
    LastUsed,
    Recommended,
    Saved(u64),
    Custom,
}

#[derive(Resource, Debug, Clone)]
pub struct PracticeDraft {
    pub source: PracticeDraftSource,
    pub loop_region: Option<LoopRegion>,
    pub user_tempo: f32,
    pub snap: SnapDivisor,
    pub preroll: PrerollSetting,
    pub count_in: bool,
    pub trainer: PracticeTrainerDraft,
}

#[derive(Resource, Debug, Clone)]
pub struct PracticeFlow {
    pub phase: PracticePhase,
    pub preview: PreviewState,
    pub origin: PracticeOrigin,
    pub edit_snapshot: Option<PracticeEditSnapshot>,
}
~~~

`PracticeTrainerDraft` stores mode plus `RampConfig`. `PracticeTrainer` stores `mode`, `ramp_config`, and dynamic `ramp`. Replace direct reads/writes of `wait_enabled` with `mode == Wait` and helper methods. Arming Ramp sets mode Ramp; enabling Wait sets mode Wait and disarms Ramp; disabling the active trainer sets Off.

Add `current_attempt_eligible: bool` to `PracticeSession`, default true. `roll_attempt` records only eligible attempts, then resets eligibility to true for the next loop. Ramp consumes only the finalized eligible record. Manual loop/tempo changes and opening Settings set eligibility false.

Draft conversion maps all fields to/from `PracticeSession` and `PracticePresetConfig`. Validation clamps tempo/ramp values, clamps bounds to `timeline.end_ms`, normalizes reversed endpoints, and falls back to whole-song for a zero-length result while returning a warning string.

- [ ] **Step 4: Verify mechanics and reducer compatibility**

Run: `cargo fmt --all && cargo test -p gameplay-drums --lib && cargo test -p gameplay-drums --test practice_mode`

Expected: PASS. Existing wait/ramp tests use `PracticeTrainerMode`; incomplete attempts do not record or promote Ramp.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/practice/draft.rs crates/gameplay-drums/src/practice/flow.rs crates/gameplay-drums/src/practice/session.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/practice/ramp.rs crates/gameplay-drums/src/practice/wait.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "refactor(practice): add setup flow and drafts"
~~~

### Task 4: Mandatory Setup lifecycle and gameplay gates

**Files:**
- Modify: `crates/gameplay-drums/src/practice/mod.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/judge.rs`
- Modify: `crates/gameplay-drums/src/scroll.rs`
- Modify: `crates/gameplay-drums/src/score.rs`
- Modify: `crates/gameplay-drums/src/gauge.rs`
- Modify: `crates/gameplay-drums/src/hit_sound.rs`
- Modify: `crates/gameplay-drums/src/practice/stats.rs`
- Modify: `crates/gameplay-drums/src/practice/ab_loop.rs`
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

**Interfaces:**
- Consumes: Task 3 flow run conditions and existing fixed-update ordering.
- Produces: `enter_practice_setup`, `start_or_continue_practice`, `cancel_initial_setup`, and a pipeline where preview/setup cannot create gameplay output.

- [ ] **Step 1: Add failing lifecycle and gate tests**

~~~rust
#[test]
fn every_practice_intent_enters_setup_before_running() {
    let mut app = build_app();
    app.insert_resource(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    assert_eq!(app.world().resource::<PracticeFlow>().phase, PracticePhase::Setup);
    assert_eq!(app.world().resource::<PracticeFlow>().preview, PreviewState::Stopped);
}

#[test]
fn setup_drops_judgment_and_miss_output() {
    let mut app = setup_phase_app();
    app.world_mut().write_message(LaneHit { lane: 1, velocity: 100 });
    app.update();
    assert!(app.world().resource::<Messages<JudgmentEvent>>().is_empty());
    assert!(app.world().resource::<Messages<NoteMissed>>().is_empty());
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert!(app.world().resource::<PracticeSession>().attempt_history.is_empty());
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums --test practice_mode setup_`

Expected: FAIL because practice still starts in the running rail model.

- [ ] **Step 3: Wire Setup and explicit run conditions**

On Performance entry, materialize `PracticeSession`, `PracticeDraft`, and `PracticeFlow { phase: Setup, preview: Stopped, origin, edit_snapshot: None }` for any request. Seed Recommended into the draft but do not seek or start an attempt. Normal play inserts none of these resources.

Use these conditions:

~~~rust
pub fn gameplay_input_active(flow: Option<Res<PracticeFlow>>) -> bool {
    flow.is_none_or(|flow| flow.phase == PracticePhase::Running)
}

pub fn chart_clock_active(flow: Option<Res<PracticeFlow>>) -> bool {
    flow.is_none_or(|flow| {
        flow.phase == PracticePhase::Running || flow.preview == PreviewState::Playing
    })
}
~~~

Add `gameplay_input_active` to judge, missed-note emission, score, gauge, hit-sound, practice stats, wait watcher, ramp, and loop-attempt systems. Keep visual note spawn/scroll active under `chart_clock_active`. Split missed-note cleanup so Preview despawns passed visuals without touching `JudgedChips` or emitting `NoteMissed`.

Change the root chart-clock chain and pending-BGM start gate in `lib.rs` to include `chart_clock_active`. Preserve PauseState and wait-flow gates for Running.

`start_or_continue_practice` validates and commits the draft, records Last Used through a typed command, stops preview, clears the edit snapshot, marks the new attempt eligible, emits `SeekToChartTime` with `preroll_target`, and sets phase Running. Put initial setup materialization in a reusable `begin_practice_setup(request, session, draft, flow)` helper so Task 8 can enter Setup from an already loaded normal run. Initial cancel emits a transition based on origin; NormalPause cancellation goes to Song Select. Results cancellation requires `ResultReturnState.available`; it sets `skip_processing_once` before requesting Result, otherwise it reports the fallback and requests Song Select.

- [ ] **Step 4: Verify lifecycle and core mechanics**

Run: `cargo fmt --all && cargo test -p gameplay-drums --lib && cargo test -p gameplay-drums --test practice_mode && cargo test -p gameplay-drums --test play_chart`

Expected: PASS. Normal play remains judged; Setup does not; Start/Continue begins an eligible attempt from pre-roll.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/scroll.rs crates/gameplay-drums/src/score.rs crates/gameplay-drums/src/gauge.rs crates/gameplay-drums/src/hit_sound.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/practice/stats.rs crates/gameplay-drums/src/practice/ab_loop.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(practice): gate runs behind setup"
~~~

### Task 5: Non-judged preview transport

**Files:**
- Create: `crates/gameplay-drums/src/practice/preview.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs`
- Modify: `crates/gameplay-drums/src/practice/actions.rs`
- Modify: `crates/gameplay-drums/src/pause.rs`
- Modify: `crates/gameplay-drums/src/seek.rs`
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

**Interfaces:**
- Consumes: `PracticeFlow`, `PracticeDraft`, `GameplayClock`, `SeekToChartTime`, chart-audio pause/resume helpers, and active loop calculation.
- Produces: `PreviewAction::{Play,Pause,Seek,PrevBar,NextBar}`, `PreviewController`, `open_practice_settings`, `cancel_practice_settings`, and preview loop wrapping.

- [ ] **Step 1: Add failing preview tests**

~~~rust
#[test]
fn preview_starts_stopped_and_loops_without_attempts() {
    let mut app = setup_phase_app();
    send_preview_action(&mut app, PreviewAction::Play);
    advance_clock_past_loop_end(&mut app);
    assert_eq!(app.world().resource::<PracticeFlow>().preview, PreviewState::Playing);
    assert_eq!(last_seek(&app), Some(loop_start(&app)));
    assert!(app.world().resource::<PracticeSession>().attempt_history.is_empty());
}

#[test]
fn cancel_editing_restores_frozen_position() {
    let mut app = running_practice_app_at(5_000);
    open_settings(&mut app);
    send_preview_action(&mut app, PreviewAction::Seek(20_000));
    cancel_settings(&mut app);
    assert_eq!(last_seek(&app), Some(5_000));
    assert_eq!(next_pause(&app), PauseState::Paused);
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums --test practice_mode preview_`

Expected: FAIL because preview transport and edit restoration do not exist.

- [ ] **Step 3: Implement preview ownership**

Add:

~~~rust
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    Play,
    Pause,
    Seek(i64),
    PrevBar,
    NextBar,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PreviewController {
    pub restore_ms: Option<i64>,
}
~~~

Opening Setup pauses chart audio after Performance initialization. Opening Settings stores the current chart milliseconds and committed session snapshot, marks the current pass ineligible, pauses audio, sets Editing, and creates a draft from the session.

Play seeks to the editing cursor or loop start, applies the draft start tempo (Ramp start, otherwise user tempo), resumes chart audio, and sets Preview Playing. Pause freezes audio and leaves the cursor at the current playhead. Preview seek emits `SeekToChartTime` with `attempt_start_ms: None`. Preview wrap seeks to A or zero and never emits `PracticeLoopCompleted`.

Cancel Editing pauses preview, restores the committed session configuration, keeps its current attempt ineligible, seeks to `restore_ms`, sets Preview Stopped, sets PauseState Paused, and opens the practice pause overlay. Start/Continue uses Task 4 commit behavior instead.

- [ ] **Step 4: Verify preview and seek reconstruction**

Run: `cargo fmt --all && cargo test -p gameplay-drums --test practice_mode && cargo test -p gameplay-drums --test system_events && cargo test -p gameplay-drums --test mixer_events`

Expected: PASS. Preview wraps and restores BGM/system/mixer/BGA positions through the existing seek engine without gameplay output.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/practice/preview.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/practice/actions.rs crates/gameplay-drums/src/pause.rs crates/gameplay-drums/src/seek.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(practice): add non-judged preview"
~~~

### Task 6: Setup/Settings shell and responsive layout

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/setup.rs`
- Create: `crates/gameplay-drums/src/practice/hud/progress.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mod.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/timeline_ui.rs`
- Delete after replacement: `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- Modify: `crates/gameplay-drums/tests/practice_hud.rs`

**Interfaces:**
- Consumes: `PracticeFlow`, `PracticeDraft`, `PracticeSession`, `ChipTimeline`, `PlayfieldLayout`, `AccessibilityPolicy`, Theme, semantic typography, density graph, and timeline gesture math.
- Produces: `PracticeSetupRoot`, `PracticeTab::{Setup,Progress,Preview}`, `PracticeLayoutMode::{Split,Tabbed}`, setup shell entities, preview label, primary action, and a full-width transport timeline.

- [ ] **Step 1: Add failing layout and shell tests**

~~~rust
#[test]
fn reference_layout_is_split_and_xlarge_narrow_layout_is_tabbed() {
    assert_eq!(practice_layout_mode(1280.0, 720.0, 1.0), PracticeLayoutMode::Split);
    assert_eq!(practice_layout_mode(900.0, 720.0, 1.5), PracticeLayoutMode::Tabbed);
}

#[test]
fn setup_shell_labels_preview_as_not_judged() {
    let mut app = setup_hud_app();
    app.update();
    assert!(texts(&mut app).iter().any(|text| text == "PREVIEW: INPUT IS NOT JUDGED"));
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 1);
    assert_eq!(count::<FullHudRoot>(&mut app), 0);
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums --test practice_hud setup_`

Expected: FAIL because the old full rail still owns the surface.

- [ ] **Step 3: Build the shared shell with stable regions**

Define:

~~~rust
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PracticeTab { #[default] Setup, Progress, Preview }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeLayoutMode { Split, Tabbed }

pub fn practice_layout_mode(width: f32, height: f32, text_multiplier: f32) -> PracticeLayoutMode {
    let scale = (width / 1280.0).min(height / 720.0);
    let settings_need = 400.0 * scale * text_multiplier;
    let preview_need = 520.0 * scale;
    if width >= settings_need + preview_need { PracticeLayoutMode::Split } else { PracticeLayoutMode::Tabbed }
}
~~~

Spawn one full-screen `PracticeSetupRoot` during Setup/Editing. In Split mode, use a left settings pane, right live playfield region, and bottom timeline. The playfield remains the existing gameplay scene; the right region reserves space rather than creating notes. In Tabbed mode, show Setup/Progress or Preview as full-width content and keep the transport timeline visible.

Use semantic text roles and visible `StateMarker` prefixes for selected tabs/rows. Add the persistent preview contract label. Pin `Start Practice` or `Continue Practice` to the bottom of the settings pane. Reuse timeline gesture reducers and density data, but bind markers to `PracticeDraft` rather than committed `PracticeSession`.

Delete the old rail spawn/input/refresh implementation only after equivalent timeline behavior has moved. Keep mini strip/chip/wait prompt for Running.

- [ ] **Step 4: Verify shell behavior and overflow**

Run: `cargo fmt --all && cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --lib practice::hud`

Expected: PASS at reference, 1080p, narrow, and all text-scale cases. No old rail entity or `PracticePauseSurface::Rail` assertion remains.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/practice/hud crates/gameplay-drums/tests/practice_hud.rs
git commit -m "feat(practice): build setup and progress shell"
~~~

### Task 7: Setup controls, presets, Progress, and typed UI actions

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/setup_controls.rs`
- Create: `crates/gameplay-drums/src/practice/presets.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/setup.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/progress.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/timeline_ui.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs`
- Modify: `crates/gameplay-drums/src/practice/toast.rs`
- Modify: `crates/gameplay-drums/tests/practice_hud.rs`

**Interfaces:**
- Consumes: Task 1 registry, Task 3 draft conversion, Task 5 preview actions, Task 6 shell, attempt history, and lane diagnosis.
- Produces: `SetupItem`, `SetupSelection`, `PracticeUiAction`, `PresetCommand`, `PresetResult`, `PracticePresetStore`, automatic labels, and Progress text/models.

- [ ] **Step 1: Add failing control and persistence-system tests**

~~~rust
#[test]
fn selecting_saved_source_populates_draft_without_starting_preview() {
    let mut app = setup_hud_app_with_preset();
    send_ui_action(&mut app, PracticeUiAction::SelectSource(PracticeDraftSource::Saved(7)));
    app.update();
    assert_eq!(app.world().resource::<PracticeDraft>().user_tempo, 0.8);
    assert_eq!(app.world().resource::<PracticeFlow>().preview, PreviewState::Stopped);
}

#[test]
fn failed_save_keeps_draft_and_reports_retry() {
    let mut store = read_only_store();
    let draft = configured_draft();
    let result = apply_preset_command(&mut store, PresetCommand::SaveNew { name: None, draft: draft.clone() });
    assert!(matches!(result, PresetResult::Failed { .. }));
    assert_eq!(draft, configured_draft());
}

#[test]
fn progress_omits_ineligible_partial_attempt() {
    let mut session = PracticeSession::default();
    session.current_attempt_eligible = false;
    session.current_attempt.counts.perfect = 10;
    assert!(!progress_rows(&session, 20_000).iter().any(|row| row.contains("10")));
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums --test practice_hud`

Expected: FAIL because controls and store resources are absent.

- [ ] **Step 3: Implement source, transport, trainer, and save reducers**

Use a single ordered selection enum:

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupItem {
    Source,
    LoopStart,
    LoopEnd,
    Tempo,
    Snap,
    Preroll,
    CountIn,
    TrainerMode,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
    RampPasses,
    SaveAsNew,
    UpdateSaved,
    DeleteSaved,
    StartOrContinue,
}
~~~

Hide Ramp detail rows unless mode is Ramp. Hide Update/Delete unless source is Saved. Keyboard/pad Up/Down changes visible selection, Left/Right adjusts values, Confirm activates, and Back follows the flow rules. Mouse row/adjust buttons call the same reducers.

Load `PracticePresetStore` once at plugin startup. Preserve `ReadOnly` load errors and publish a global warning. For Save/Update/Delete/Last Used, clone the registry, apply the mutation, write the clone atomically, then replace the resource only on success. Keep the draft unchanged on error and push a retry-capable error notification.

Compute automatic labels from current timeline bars and `format_chart_time`. Source order is Whole Song, Last Used when present, Recommended when present, saved presets sorted by optional name/automatic label, then Custom.

Progress renders only completed `attempt_history` records for the current span plus `lane_diag`. Use accuracy for Off/Ramp and flow for Wait records. Keep at most the existing bounded 20 attempts.

- [ ] **Step 4: Verify controls, persistence, and Progress**

Run: `cargo fmt --all && cargo test -p dtx-config && cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --test practice_mode`

Expected: PASS. Saved selection fills every draft field, explicit writes persist, Last Used updates only on Start/Continue, and Progress ignores partial attempts.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/practice/presets.rs crates/gameplay-drums/src/practice/hud/setup.rs crates/gameplay-drums/src/practice/hud/setup_controls.rs crates/gameplay-drums/src/practice/hud/progress.rs crates/gameplay-drums/src/practice/hud/timeline_ui.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/practice/toast.rs crates/gameplay-drums/tests/practice_hud.rs
git commit -m "feat(practice): wire setup controls and presets"
~~~

### Task 8: Separate normal and practice pause actions

**Files:**
- Modify: `crates/gameplay-drums/src/pause.rs`
- Modify: `crates/gameplay-drums/src/practice/actions.rs`
- Modify: `crates/gameplay-drums/src/practice/flow.rs`
- Modify: `crates/gameplay-drums/src/perf_hotkeys.rs`
- Test: `crates/gameplay-drums/src/pause.rs`
- Test: `crates/gameplay-drums/tests/practice_mode.rs`

**Interfaces:**
- Consumes: flow open/cancel/continue operations, `PracticeRecommendation`, current playhead, bar timeline, and existing Quick Settings values.
- Produces: final pause item arrays, `Practice This Section`, compact Quick Settings, exact Resume, Restart Loop, Settings, and Exit behavior.

- [ ] **Step 1: Add failing menu/action tests**

~~~rust
#[test]
fn pause_items_match_context_contracts() {
    assert_eq!(pause_items(PauseContext::Normal), &[
        PauseItemKind::Resume,
        PauseItemKind::RestartSong,
        PauseItemKind::PracticeThisSection,
        PauseItemKind::QuickSettings,
        PauseItemKind::ReturnToSongSelect,
    ]);
    assert_eq!(pause_items(PauseContext::Practice), &[
        PauseItemKind::Resume,
        PauseItemKind::RestartLoop,
        PauseItemKind::PracticeSettings,
        PauseItemKind::ExitToSongSelect,
    ]);
}

#[test]
fn practice_this_section_builds_bar_aligned_normal_pause_request() {
    let request = practice_request_at(&timeline(), 5_100);
    assert_eq!(request.origin, PracticeOrigin::NormalPause);
    let PracticeSeed::Recommended(section) = request.seed else { panic!("recommended") };
    assert!(section.loop_start_ms <= 5_100 && section.loop_end_ms > 5_100);
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums pause`

Expected: FAIL because the current menus expose three rows and the rail discriminator.

- [ ] **Step 3: Implement pause contexts and actions**

Remove `PracticePauseSurface` and all rail branches. Esc and the bound system Pause input always open the overlay while Running. Practice Settings calls `open_practice_settings`, resumes `PauseState::Running` so Editing can own its clock/audio policy, and leaves preview stopped.

Normal `Practice This Section` creates a recommendation spanning one prior bar through one following bar, sets `PracticeIntent::recommended(PracticeOrigin::NormalPause, recommendation)`, marks the normal run abandoned, and enters Setup in the loaded Performance stage. Cancelling goes to Song Select.

Quick Settings is an inline pause subview with Scroll Speed, Lane Visibility, BGM Volume, and Input Offset plus Back. Changes use the existing config/runtime resources. Full input profiles and Customize remain outside Pause.

Resume unpauses exact position. Restart Song uses SongLoading. Restart Loop emits the existing pre-roll seek and marks a fresh attempt eligible. Exit to Song Select clears practice resources through existing stage exit cleanup.

- [ ] **Step 4: Verify pause and expert shortcuts**

Run: `cargo fmt --all && cargo test -p gameplay-drums pause && cargo test -p gameplay-drums --test practice_mode`

Expected: PASS. Esc never opens Settings, Tab opens Settings only from Practice Running, and existing A/B/tempo/restart shortcuts still work while marking the current pass ineligible where required.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/pause.rs crates/gameplay-drums/src/practice/actions.rs crates/gameplay-drums/src/practice/flow.rs crates/gameplay-drums/src/perf_hotkeys.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(pause): separate practice settings"
~~~

### Task 9: Pad navigation, accessibility, motion, and old-rail cleanup

**Files:**
- Modify: `crates/gameplay-drums/src/menu_nav.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/setup.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/setup_controls.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/progress.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mod.rs`
- Modify: `crates/gameplay-drums/tests/practice_hud.rs`
- Modify: `crates/gameplay-drums/tests/practice_mode.rs`

**Interfaces:**
- Consumes: existing `PadNavHit -> NavAction`, AccessibilityPolicy, semantic typography, StateMarker, and OutQuint tween helpers.
- Produces: `NavContext::PracticeSetup`, complete keyboard/mouse/pad parity, reduced-motion setup transitions, and no remaining rail symbols.

- [ ] **Step 1: Add failing navigation and accessibility tests**

~~~rust
#[test]
fn pads_navigate_setup_and_preview_without_gameplay_hits() {
    assert_eq!(
        active_context(&AppState::Performance, &PauseState::Running, false, false, false, Some(PracticePhase::Setup)),
        Some(NavContext::PracticeSetup),
    );
}

#[test]
fn selected_controls_have_text_or_shape_markers() {
    let mut app = setup_hud_app();
    app.update();
    assert!(selected_markers(&mut app).iter().all(|label| !label.is_empty()));
}

#[test]
fn old_rail_symbols_are_gone() {
    let source = include_str!("../src/practice/hud/mod.rs");
    assert!(!source.contains("full_hud"));
    assert!(!source.contains("PracticePauseSurface"));
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p gameplay-drums --test practice_hud`

Expected: FAIL until menu context and cleanup are complete.

- [ ] **Step 3: Finish input ownership and presentation states**

Extend `active_context` with an optional practice phase. Setup and Editing return `PracticeSetup`, so real MIDI pads emit NavAction and the judge path remains gated. Running returns no menu context. Keep the 500 ms entry grace and 80 ms debounce.

Keyboard arrows/Enter/Space and mouse interactions call the same `PracticeUiAction` reducer as NavAction. SD/Back cancels Setup or returns Editing to Pause. The visible legend changes between Setup, Progress, Preview, Pause, and Running.

Add semantic fonts to every player-facing label. Prefix focused rows with `StateMarker::Focus`, active tabs with `StateMarker::Selected`, destructive delete with `StateMarker::Destructive`, failed saves with `StateMarker::Error`, and unavailable controls with `StateMarker::Disabled`.

Use a short OutQuint panel transition and tab crossfade when motion is allowed. Under Reduce Motion, spawn at the final position and switch tab visibility without translation. Do not animate playfield geometry.

Delete old rail resources, components, tests, comments, and z-index names. Keep the Running mini strip/chip and update their legend to show `Esc Pause` and `Tab Settings`.

- [ ] **Step 4: Verify all input and HUD paths**

Run: `cargo fmt --all && cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --test practice_mode && cargo test -p gameplay-drums --test bindings_lane_pipeline`

Expected: PASS for keyboard, mouse, and synthetic pad paths; no rail symbol remains; reduced motion and all text scales render the correct structural mode.

- [ ] **Step 5: Commit**

~~~bash
git add crates/gameplay-drums/src/menu_nav.rs crates/gameplay-drums/src/practice/hud crates/gameplay-drums/tests/practice_hud.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(practice): finish accessible setup input"
~~~

### Task 10: Player documentation and full verification

**Files:**
- Modify: `docs/data-and-persistence.md`
- Modify: `docs/player-guide.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/notes/2026-07-11-player-manual-current-behavior.md`
- Modify: `docs/notes/2026-07-11-player-user-stories.md`

**Interfaces:**
- Consumes: completed runtime behavior from Tasks 1-9.
- Produces: current player-facing instructions, persistence/backup coverage, roadmap status, and final gate evidence.

- [ ] **Step 1: Update documentation from verified runtime behavior**

Document:

~~~text
Practice always opens Setup. Preview starts stopped and is not judged.
Esc opens Pause during a run; Tab opens Practice Settings.
Pause Resume continues exactly; Continue Practice restarts from pre-roll.
Saved loops live in CONFIG_DIR/practice-presets.toml and require explicit Save/Update.
Trainer modes are Off, Wait, and Ramp. Only completed loop attempts enter Progress.
~~~

Add `practice-presets.toml` to the data table and backup list. Remove descriptions of Esc opening the full rail, double-Enter exit, `PracticePauseSurface::Rail`, and the 17-row rail.

- [ ] **Step 2: Run focused package tests**

Run:

~~~bash
cargo test -p dtx-config
cargo test -p game-shell
cargo test -p game-menu
cargo test -p game-results
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --tests
~~~

Expected: all commands exit 0 with no failed tests.

- [ ] **Step 3: Run workspace gates**

Run:

~~~bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
~~~

Expected: all commands exit 0 with no formatting diff, compile error, or warning.

- [ ] **Step 4: Perform manual runtime checks**

Run: `cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking`

Verify at 1280x720 and 1920x1080:

1. Song Select Practice and Results Practice both open stopped Setup.
2. Preview audio, notes, BGA, and timeline move together and loop without judgments.
3. Start uses pre-roll/count-in and records only complete attempts.
4. Esc Pause resumes exactly; Tab Settings continues from pre-roll.
5. Save, reload, update, and delete a loop for one difficulty without exposing it to another.
6. Standard/Large/Extra Large text and Reduce Motion preserve usable layout.
7. Keyboard, mouse, and a connected kit can navigate, preview, start, restart, and exit.

- [ ] **Step 5: Commit documentation**

~~~bash
git add docs/data-and-persistence.md docs/player-guide.md docs/roadmap.md docs/notes/2026-07-11-player-manual-current-behavior.md docs/notes/2026-07-11-player-user-stories.md
git commit -m "docs(practice): document setup workflow"
~~~

- [ ] **Step 6: Record final evidence**

Run:

~~~bash
git status --short
git log --oneline --decorate -12
~~~

Expected: clean status and one logical commit per task. Do not merge until every focused test and workspace gate above has fresh successful output.
