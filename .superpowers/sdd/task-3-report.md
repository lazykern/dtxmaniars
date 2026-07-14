# Task 3 Report: Practice draft, trainer mode, and flow reducers

## RED evidence

- `cargo test -p gameplay-drums practice::` exited 101 before production changes. The compiler reported the expected missing `PracticeDraft`, `PracticeTrainerMode`, `PracticeFlow`, phase/preview types, run conditions, trainer helpers, and `current_attempt_eligible` field.
- `cargo test -p gameplay-drums practice::draft::tests::invalid_recommendation_falls_back_to_whole_song_source` then failed with `left: Recommended`, `right: WholeSong`, pinning the request-boundary recovery before its fix.

## GREEN evidence

- `cargo test -p gameplay-drums --tests`: passed all 591 library tests and every gameplay-drums integration target.
- `cargo test -p gameplay-drums --lib`: 591 passed, 0 failed.
- `cargo test -p gameplay-drums --test practice_mode`: 22 passed, 0 failed.
- `cargo test -p gameplay-drums --test practice_hud`: 14 passed, 0 failed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`: passed.
- `git diff --check`: passed before the report was added and is repeated in the final audit.

## Behavior delivered

- Added pure draft validation and conversions for practice requests, committed sessions, and persisted preset configuration, including loop-bound recovery, numeric clamping, and invalid-recommendation fallback.
- Added Setup/Running/Editing flow state, stopped/playing preview state, edit snapshots, and the four Task 3 run conditions without wiring Task 4 runtime gates.
- Replaced independent Wait/Ramp flags with one `PracticeTrainerMode` and helper mutations that preserve exclusivity.
- Added attempt eligibility. Ineligible or empty attempts do not enter history or reach Ramp evaluation; rolling the loop starts the next eligible attempt.
- Manual loop, tempo, restart, scrub, trainer, and Settings interruptions invalidate the current attempt. Existing Wait/Ramp/HUD call sites were mechanically migrated to the enum-backed helpers.

## Files

- Added `crates/gameplay-drums/src/practice/draft.rs`.
- Added `crates/gameplay-drums/src/practice/flow.rs`.
- Updated practice session, ramp, wait, module exports, actions, stats, and existing HUD call sites/tests.
- Updated the judgment wait-defer read to use the enum-backed helper.
- Updated `crates/gameplay-drums/tests/practice_mode.rs` with the ineligible-loop recovery integration test.

## Concerns

- `RTK.md` is referenced by the supplied root instructions but is absent from the repository and worktree.
- Task 4 lifecycle/gate wiring and Task 5 preview transport remain intentionally unimplemented.

## Fix cycle: normalize invalid-loop sources and cover the flow matrix

### RED evidence

- `cargo test -p gameplay-drums --lib out_of_chart_ -- --nocapture` exited 101 with all three new regressions failing at the source assertion: `Saved(17)`, `Custom`, and `Recommended` were retained instead of `WholeSong` after their out-of-chart loops clamped to zero length.
- Root cause: the zero-length validation branch cleared `loop_region` and emitted the fallback warning but did not normalize `PracticeDraftSource`, leaving provenance inconsistent with the validated whole-song draft.

### GREEN evidence

- The recovery branch now sets `source = PracticeDraftSource::WholeSong` alongside `loop_region = None`; no other validation or runtime behavior changed.
- `cargo test -p gameplay-drums --lib out_of_chart_ -- --nocapture`: 3 passed, 0 failed.
- `cargo test -p gameplay-drums --lib practice::flow::tests:: -- --nocapture`: 5 passed, 0 failed. Coverage includes no-flow semantics, every `PracticePhase` x `PreviewState` combination, all four run conditions, all three origins, defaults, and frozen edit snapshots.
- `cargo test -p gameplay-drums --lib`: 597 passed, 0 failed.
- `cargo test -p gameplay-drums --test practice_mode`: 22 passed, 0 failed.
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- Task 4 runtime gates remain unwired.
