# Results Analysis and Recommended Practice Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Explain normal-play timing weaknesses on Results and enter the existing practice transport with the weakest section already looped.

**Architecture:** gameplay-drums records bounded, ephemeral normal-play events and derives a report using the existing chart timeline. game-results snapshots it before saving, compares it with prior native history, and renders a compact summary plus a details surface. game-shell owns the typed practice request so menu and results can enter practice without a dependency inversion.

**Tech Stack:** Rust, Bevy 0.19, gameplay-drums, game-results, game-shell, dtx-scoring, and dtx-ui.

## Global Constraints

- Do not change DTXManiaNX timing windows, scoring, lane mapping, or score.ini.
- Raw telemetry is in-memory only and capped at 8,192 records.
- Record only non-practice gameplay events; practice and modified-speed runs are ineligible for normal PB comparison.
- Negative error is early; positive error is late.
- Recommendation loops are bar-aligned with one-bar pre-roll and 1.0 initial tempo.
- game-shell request types contain only primitives/enums and do not depend on gameplay-drums.
- Keep Results compact by default; detailed diagnostics are explicitly opened.
- Do not modify CI/CD files.

---

## File structure

- Create crates/gameplay-drums/src/results_analysis.rs: stream, pure report, record systems, and tests.
- Modify crates/gameplay-drums/src/lib.rs: register analysis resource/plugin.
- Modify crates/gameplay-drums/src/practice/mod.rs: materialize recommendations in PracticeSession.
- Modify crates/gameplay-drums/src/playback_rate.rs: use typed intent.
- Modify crates/game-shell/src/states.rs and lib.rs: typed intent/recommendation API.
- Modify crates/game-menu/src/song_select.rs: select normal/manual intent.
- Modify crates/game-results/src/lib.rs: snapshot report and pre-save PB comparison.
- Modify crates/game-results/src/input.rs and ui.rs: details and recommended handoff.

### Task 1: Typed practice request

**Files:**
- Modify: crates/game-shell/src/states.rs:96-102
- Modify: crates/game-shell/src/lib.rs:16-19
- Modify: crates/game-menu/src/song_select.rs:1763-1820
- Modify: crates/gameplay-drums/src/practice/mod.rs:79-98
- Modify: crates/gameplay-drums/src/playback_rate.rs:43-60

**Consumes:** existing SongLoading transition and PracticeSession.

**Produces:** PracticeIntent with None, Manual, and Recommended(PracticeRecommendation), plus is_requested() and recommendation().

- [ ] **Step 1: Write failing tests**

~~~rust
#[test]
fn recommendation_is_a_practice_request() {
    let intent = PracticeIntent::Recommended(
        PracticeRecommendation::weak_section(1_000, 5_000, 3),
    );
    assert!(intent.is_requested());
    assert_eq!(intent.recommendation().unwrap().loop_start_ms, 1_000);
}

#[test]
fn recommended_intent_seeds_existing_transport() {
    let session = session_from_intent(&PracticeIntent::Recommended(
        PracticeRecommendation::weak_section(1_000, 5_000, 3),
    ));
    assert_eq!(session.transport.loop_region.unwrap().end_ms, 5_000);
    assert_eq!(session.transport.preroll, PrerollSetting::OneBar);
    assert_eq!(session.transport.user_tempo, 1.0);
}
~~~

- [ ] **Step 2: Verify the tests fail**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-shell -p gameplay-drums recommendation_is_a_practice_request

Expected: FAIL because the request API has not been defined.

- [ ] **Step 3: Implement the minimal request API**

~~~rust
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub enum PracticeIntent {
    #[default]
    None,
    Manual,
    Recommended(PracticeRecommendation),
}

impl PracticeIntent {
    pub fn is_requested(self) -> bool { !matches!(self, Self::None) }
    pub fn recommendation(self) -> Option<PracticeRecommendation> {
        match self { Self::Recommended(value) => Some(value), _ => None }
    }
}
~~~

Define primitive PracticeRecommendation, PracticePreRoll::OneBar, and PracticeReason::WeakSection { lane: Option<u8>, section_start_ms: i64 } in game-shell. Make a focused session_from_intent helper: None makes no session, Manual returns defaults, and a valid recommendation sets loop, one-bar pre-roll, and clamped initial tempo. Replace every boolean .0 read/write: menu Confirm becomes None, menu Practice becomes Manual, and Retry leaves the intent untouched.

- [ ] **Step 4: Verify and commit**

Run: cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-shell -p gameplay-drums

Expected: PASS.

~~~bash
git add crates/game-shell/src/states.rs crates/game-shell/src/lib.rs crates/game-menu/src/song_select.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/src/playback_rate.rs
git commit -m "refactor: structure practice requests"
~~~

### Task 2: Bounded normal-play telemetry and pure report

**Files:**
- Create: crates/gameplay-drums/src/results_analysis.rs
- Modify: crates/gameplay-drums/src/lib.rs:26-210

**Consumes:** JudgmentEvent, NoteMissed, ChipTimeline, and absence of PracticeSession.

**Produces:** NormalPlayEventStream, RecordedJudgment, PerformanceAnalysis, and analyze_normal_play.

- [ ] **Step 1: Write failing tests**

~~~rust
#[test]
fn report_has_median_mad_lane_and_bar_loop() {
    let report = analyze_normal_play(&events, &[0, 2_000, 4_000, 6_000, 8_000]);
    assert_eq!(report.bias_ms, Some(-20));
    assert_eq!(report.spread_ms, Some(5));
    assert_eq!(report.weakest_lane.unwrap().lane, 3);
    assert_eq!(report.weakest_section.unwrap().loop_start_ms, 2_000);
    assert_eq!(report.weakest_section.unwrap().loop_end_ms, 8_000);
}

#[test]
fn stream_is_bounded() {
    let mut stream = NormalPlayEventStream::default();
    for event in (0..8_193).map(event_at) { stream.push(event); }
    assert_eq!(stream.events.len(), 8_192);
    assert!(stream.truncated);
}
~~~

- [ ] **Step 2: Verify failure**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums results_analysis::tests

Expected: FAIL because the module is absent.

- [ ] **Step 3: Implement stream and analysis**

~~~rust
pub const MAX_NORMAL_PLAY_EVENTS: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordedJudgment {
    pub lane: LaneId,
    pub kind: JudgmentKind,
    pub delta_ms: i64,
    pub chip_idx: usize,
    pub chart_ms: i64,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct NormalPlayEventStream {
    pub events: Vec<RecordedJudgment>,
    pub truncated: bool,
}
~~~

Reset on Performance entry. In FixedUpdate, independently consume both judgment messages after DrumsSets::Score. With no practice session, append hits using timeline.judge_ms_by_idx; append scroll misses as JudgmentKind::Miss, delta zero, and the missed chip timeline time. Ignore malformed indices. Implement sorted lower-middle medians and MAD over hits only. Score lanes/bars using Perfect 0.0, Great 0.2, Good 0.4, Poor 0.7, Miss 1.0; require three events and resolve ties by lower lane id/earlier bar. Build the loop from one prior bar through the following bar, clamped to valid boundaries.

- [ ] **Step 4: Verify and commit**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums

Expected: PASS, including normal/practice recorder tests.

~~~bash
git add crates/gameplay-drums/src/results_analysis.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat: analyze normal play timing events"
~~~

### Task 3: Snapshot compatible PB analysis before saving

**Files:**
- Modify: crates/game-results/src/lib.rs:20-235
- Test: crates/game-results/src/lib.rs:240-360

**Consumes:** NormalPlayEventStream, PerformanceAnalysis, CompletedRunContext, ScoreStoreResource.

**Produces:** ResultAnalysis with report, pb_delta, and recommendation.

- [ ] **Step 1: Write failing tests**

~~~rust
#[test]
fn snapshot_uses_prior_best_before_current_score_is_saved() {
    let snapshot = snapshot_result_analysis(&normal_run, 900, &chart, &store, &events, &timeline);
    assert_eq!(snapshot.pb_delta, Some(100));
}

#[test]
fn practice_and_modified_runs_have_no_pb_or_recommendation() {
    assert_eq!(snapshot_result_analysis(&practice_run, 900, &chart, &store, &events, &timeline).pb_delta, None);
    assert!(snapshot_result_analysis(&slow_run, 900, &chart, &store, &events, &timeline).recommendation.is_none());
}
~~~

- [ ] **Step 2: Verify failure**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-results snapshot_

Expected: FAIL because ResultAnalysis is absent.

- [ ] **Step 3: Implement the snapshot**

Register a default ResultAnalysis resource. Chain snapshot_result_analysis_system before save_result on Result entry. Calculate canonical identity before store mutation. For native, 1.0-rate normal runs, compare current score against store.best_for_chart(canonical_hash). Convert only a valid weakest section into PracticeRecommendation. For practice or modified speed, keep an optional display report but set PB delta and recommendation to none. Leave the score store schema and score.ini path unchanged.

- [ ] **Step 4: Verify and commit**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-results

Expected: PASS; persistence tests retain prior behavior.

~~~bash
git add crates/game-results/src/lib.rs
git commit -m "feat: compare results with prior best"
~~~

### Task 4: Details surface and recommended handoff

**Files:**
- Modify: crates/game-results/src/input.rs:1-380
- Modify: crates/game-results/src/ui.rs:1-840

**Consumes:** ResultAnalysis, typed PracticeIntent, and existing result input/reveal state.

**Produces:** compact PB/weakness rows, a Details panel, and a recommendation-aware Practice action.

- [ ] **Step 1: Write failing UI/input tests**

~~~rust
#[test]
fn practice_label_names_recommended_section() {
    assert_eq!(practice_label(Some(&recommendation)), "Practice weakest section");
    assert_eq!(practice_label(None), "Practice");
}

#[test]
fn practice_action_uses_recommendation_or_manual_fallback() {
    apply_practice(&mut intent, Some(recommendation));
    assert!(matches!(*intent, PracticeIntent::Recommended(_)));
    apply_practice(&mut intent, None);
    assert_eq!(*intent, PracticeIntent::Manual);
}
~~~

- [ ] **Step 2: Verify failure**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-results practice_label

Expected: FAIL because the label/action is absent.

- [ ] **Step 3: Implement the focused UI**

After save status, conditionally render +N vs PB or N below PB and a brief weakness summary. Add ResultDetailsOpen(pub bool); Tab toggles a panel that shows bias, MAD, truncation, judgment distribution, lane ranking, and the selected section range. First input while reveal is running remains consumed. Keep Continue/Retry mappings unchanged. Practice copies the recommendation to typed intent or falls back to Manual; label it Practice weakest section only when one exists.

- [ ] **Step 4: Verify and commit**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-results -p gameplay-drums

Expected: PASS.

~~~bash
git add crates/game-results/src/input.rs crates/game-results/src/ui.rs
git commit -m "feat: hand off weakest result section to practice"
~~~

### Task 5: Verify and record completion

**Files:**
- Modify: docs/notes/2026-07-13-game-improvement-program.md

- [ ] **Step 1: Run package gates**

Run: cargo fmt --all -- --check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-shell -p gameplay-drums -p game-results && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy -p game-shell -p gameplay-drums -p game-results --all-targets -- -D warnings

Expected: PASS.

- [ ] **Step 2: Run workspace gates**

Run: CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo check --workspace && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy --workspace --all-targets -- -D warnings && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test --workspace --lib

Expected: PASS.

- [ ] **Step 3: Audit, update ledger, and commit**

Run: git diff --check && git status --short && git log --oneline -8

Expected: no whitespace errors. Mark Cycle 4 implemented with verified outcomes, then commit:

~~~bash
git add docs/notes/2026-07-13-game-improvement-program.md
git commit -m "docs: record results analysis completion"
~~~

## Plan self-review

The five tasks cover bounded collection, all approved metrics, practice exclusion, compact/details UI, comparable PB deltas, typed handoff, manual fallback, tests, and local gates. Types are introduced before consumers, and the plan makes no score-store or CI/CD change.
