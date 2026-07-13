# Guided Calibration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace song-dependent offset taps with a reliable synthetic calibration sequence that reports confidence and cannot apply weak evidence.

**Architecture:** Put robust sample analysis in `editor/calibration.rs` as pure types and functions. Keep runtime state in the same module: it schedules synthetic clicks from `Instant`, consumes `InputHit`, and renders the report. Existing `ConfigDraft` remains the sole apply/persist handoff; BGM adjustment is never mutated.

**Tech Stack:** Rust, Bevy 0.19 messages/resources/UI, bevy_kira_audio static sound sources, existing `gameplay-drums` test harness.

## Global Constraints

- Preserve `gameplay.input_offset_ms` sign semantics and its `±300 ms` config clamp.
- Use `InputHit.captured_at`, not `LaneHit`, for calibration samples.
- Keyboard and MIDI must reach calibration through the same event reader.
- A low-confidence report must not mutate `ConfigDraft`.
- Restore autoplay, metronome, and timing-line resources on apply, cancel, or Performance exit.
- Do not edit `references/` or CI/CD files.

---

### Task 1: Pure calibration report

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`
- Test: `crates/gameplay-drums/src/editor/calibration.rs`

**Consumes:** signed tap errors in milliseconds and scheduler observations.

**Produces:** `CalibrationReport::from_errors(errors: &[i32], scheduler_delays: &[i32]) -> CalibrationReport`, `Confidence`, and `can_apply()`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn report_uses_median_and_rejects_distant_outlier() {
    let report = CalibrationReport::from_errors(&[39, 40, 41, 40, 400], &[2, 3]);
    assert_eq!(report.proposed_offset_ms, 40);
    assert_eq!(report.accepted_samples, 4);
    assert_eq!(report.rejected_samples, 1);
}

#[test]
fn unstable_or_sparse_evidence_cannot_apply() {
    assert!(!CalibrationReport::from_errors(&[10; 11], &[0]).can_apply());
    assert!(!CalibrationReport::from_errors(&[10; 12], &[35]).can_apply());
}
```

- [ ] **Step 2: Run the tests to verify red**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums report_`

Expected: compilation failure because `CalibrationReport` does not exist.

- [ ] **Step 3: Implement the smallest pure report**

```rust
pub const TARGET_ACCEPTED_SAMPLES: usize = 12;
pub const OUTLIER_DISTANCE_MS: i32 = 100;
pub const MAX_MAD_MS: i32 = 20;
pub const MAX_SCHEDULER_DELAY_MS: i32 = 34;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence { High, Low }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationReport {
    pub proposed_offset_ms: i32,
    pub accepted_samples: usize,
    pub rejected_samples: usize,
    pub spread_mad_ms: i32,
    pub max_scheduler_delay_ms: i32,
    pub confidence: Confidence,
}

impl CalibrationReport {
    pub fn from_errors(errors: &[i32], scheduler_delays: &[i32]) -> Self {
        let center = median(errors);
        let accepted: Vec<_> = errors.iter().copied()
            .filter(|error| (error - center).abs() <= OUTLIER_DISTANCE_MS)
            .collect();
        let proposal = median(&accepted);
        let deviations: Vec<_> = accepted.iter().map(|value| (value - proposal).abs()).collect();
        let spread = median(&deviations);
        let max_delay = scheduler_delays.iter().copied().max().unwrap_or(0).max(0);
        let rejected = errors.len().saturating_sub(accepted.len());
        let confidence = if accepted.len() >= TARGET_ACCEPTED_SAMPLES
            && rejected * 4 <= errors.len()
            && spread <= MAX_MAD_MS
            && max_delay <= MAX_SCHEDULER_DELAY_MS { Confidence::High } else { Confidence::Low };
        Self { proposed_offset_ms: proposal, accepted_samples: accepted.len(), rejected_samples: rejected,
            spread_mad_ms: spread, max_scheduler_delay_ms: max_delay, confidence }
    }
    pub fn can_apply(&self) -> bool { self.confidence == Confidence::High }
}
```

- [ ] **Step 4: Run the focused tests to verify green**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums report_`

Expected: both report tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat: assess calibration sample confidence"
```

### Task 2: Synthetic sequence and shared input collection

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`
- Test: `crates/gameplay-drums/src/editor/calibration.rs`

**Consumes:** `InputHit { audio_ms, captured_at, .. }`, Bevy `Audio`, and a synthesized static click source.

**Produces:** a `Collecting` state with 120 BPM scheduled beats, scheduler-delay observations, and accepted raw errors.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn synthetic_schedule_has_a_lead_in_and_half_second_beats() {
    let schedule = CalibrationSchedule::new(Instant::now());
    assert_eq!(schedule.beat_interval(), Duration::from_millis(500));
    assert!(schedule.first_beat_at() > schedule.started_at());
}

#[test]
fn tap_error_uses_the_physical_input_timestamp() {
    let started = Instant::now();
    let schedule = CalibrationSchedule::new(started);
    assert_eq!(schedule.error_ms(schedule.first_beat_at() + Duration::from_millis(37)), 37);
}
```

- [ ] **Step 2: Run the tests to verify red**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums synthetic_schedule tap_error_uses`

Expected: compilation failure because `CalibrationSchedule` does not exist.

- [ ] **Step 3: Implement the schedule and collection path**

```rust
pub struct CalibrationSchedule { started_at: Instant, first_beat_at: Instant }
impl CalibrationSchedule {
    pub fn new(started_at: Instant) -> Self {
        Self { started_at, first_beat_at: started_at + Duration::from_secs(1) }
    }
    pub fn beat_interval(&self) -> Duration { Duration::from_millis(500) }
    pub fn error_ms(&self, tap: Instant) -> i32 {
        let interval = self.beat_interval().as_millis() as i128;
        let elapsed = tap.saturating_duration_since(self.first_beat_at()).as_millis() as i128;
        let nearest = ((elapsed + interval / 2).div_euclid(interval)) * interval;
        (elapsed - nearest) as i32
    }
}
```

Create the synthetic click `AudioSource` once, fire due clicks with `play_sfx_handle`, record `now - scheduled` in milliseconds, and change `collect_taps` to read `InputHit`. Ignore input before the first beat and stop after the fixed sequence. Do not read `ActiveChart`, `LaneHit`, or chart BPM.

- [ ] **Step 4: Run focused tests to verify green**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums calibration`

Expected: new schedule tests and existing calibration tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat: drive calibration from synthetic timed clicks"
```

### Task 3: Safe apply, recovery, and player-facing report

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`
- Test: `crates/gameplay-drums/src/editor/calibration.rs`

**Consumes:** `CalibrationReport`, `ConfigDraft`, runtime toggle snapshot, optional `MidiConnected` resource.

**Produces:** high-confidence-only apply, restoration on every terminal route, and explanatory overlay copy.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn weak_report_does_not_replace_manual_offset() {
    let mut value = 17;
    apply_report(&mut value, &CalibrationReport::from_errors(&[7; 11], &[0]));
    assert_eq!(value, 17);
}

#[test]
fn strong_report_replaces_manual_offset_within_config_clamp() {
    let mut value = 17;
    apply_report(&mut value, &CalibrationReport::from_errors(&[450; 12], &[0]));
    assert_eq!(value, 300);
}
```

- [ ] **Step 2: Run the tests to verify red**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums report_`

Expected: compilation failure because `apply_report` does not exist.

- [ ] **Step 3: Implement terminal handling and overlay copy**

```rust
pub fn apply_report(current: &mut i32, report: &CalibrationReport) {
    if report.can_apply() {
        *current = suggested_offset(report.proposed_offset_ms, dtx_config::INPUT_OFFSET_CLAMP_MS);
    }
}
```

On Enter, call `apply_report`; then restore the snapshot. On Esc, Performance exit, and MIDI disconnection, restore the same snapshot. The overlay must show offset, accepted/rejected counts, spread, scheduler observation, confidence, and the separate BGM-adjust explanation. It must say that low confidence is not applied and retain the manual current value.

- [ ] **Step 4: Run package tests and static checks**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy -p gameplay-drums --all-targets -- -D warnings`

Expected: all pass with no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs crates/gameplay-drums/src/editor/panel.rs
git commit -m "feat: safely apply guided calibration results"
```

### Task 4: Final verification

**Files:** no source changes expected.

- [ ] **Step 1: Format and run the affected-package suite**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo fmt --all -- --check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo check -p gameplay-drums && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums`

Expected: all commands exit 0.

- [ ] **Step 2: Commit the verified documentation state if it changed**

```bash
git add docs/superpowers/specs/2026-07-13-guided-calibration-design.md docs/superpowers/plans/2026-07-13-guided-calibration.md
git commit -m "docs: plan guided calibration"
```

## Self-review

- Synthetic timing, input-event ownership, confidence, outlier rejection, audio and visual scheduling observations, manual/BGM separation, cancellation, state restoration, and MIDI-disconnect behavior all have a named implementation task.
- The plan contains no deferred implementation placeholders; source snippets name the actual interfaces and commands.
- Task interfaces flow from report → schedule/collection → apply/render, with `ConfigDraft` remaining the only persistence handoff.
