# Documentation Truth Inventory

Date: 2026-07-13
Status: Complete — Cycle 8 final truth gate passed
Scope: Non-reference repository files; `references/` is read-only

## Baseline

The pre-repair command was:

```sh
rg -l 'DTXmaniaNX[-]BocuD' --glob '!references/**'
```

It found exactly 81 files. The obsolete directory token was replaced with the
live `references/DTXmaniaNX/` root only after checking the intended target in
the vendored tree. Two occurrences are intentional checker evidence: the Cycle
8 design's historical explanation and the negative stale-root fixture.

### Root and crate handbooks

- `AGENTS.md`
- `crates/dtx-assets/AGENTS.md`
- `crates/dtx-audio/AGENTS.md`
- `crates/dtx-bga/AGENTS.md`
- `crates/dtx-core/AGENTS.md`
- `crates/dtx-input/AGENTS.md`
- `crates/dtx-library/AGENTS.md`
- `crates/dtx-timing/AGENTS.md`
- `crates/game-results/AGENTS.md`
- `crates/game-shell/AGENTS.md`
- `crates/gameplay-drums/AGENTS.md`
- `crates/gameplay-guitar/AGENTS.md`
- `docs/decisions/README.md`

### Source comments and tests

- `crates/dtx-assets/src/lib.rs`
- `crates/dtx-audio/src/lib.rs`
- `crates/dtx-bga/src/lib.rs`
- `crates/dtx-bga/src/video.rs`
- `crates/dtx-config/src/drums.rs`
- `crates/dtx-config/src/lib.rs`
- `crates/dtx-core/src/assets.rs`
- `crates/dtx-core/src/base36.rs`
- `crates/dtx-core/src/bga.rs`
- `crates/dtx-core/src/c_avi.rs`
- `crates/dtx-core/src/c_box_set_def.rs`
- `crates/dtx-core/src/c_chart_data.rs`
- `crates/dtx-core/src/c_chip.rs`
- `crates/dtx-core/src/c_song_list_node.rs`
- `crates/dtx-core/src/cdtx_config.rs`
- `crates/dtx-core/src/cdtx_model.rs`
- `crates/dtx-core/src/cdtx_nested.rs`
- `crates/dtx-core/src/channel.rs`
- `crates/dtx-core/src/chip_classify.rs`
- `crates/dtx-core/src/chip_transform.rs`
- `crates/dtx-core/src/constants.rs`
- `crates/dtx-core/src/cscore_ini.rs`
- `crates/dtx-core/src/enum_converter.rs`
- `crates/dtx-core/src/fdk_sub_acts.rs`
- `crates/dtx-core/src/parser.rs`
- `crates/dtx-core/src/random_mode.rs`
- `crates/dtx-core/src/score_song.rs`
- `crates/dtx-core/src/timing.rs`
- `crates/dtx-core/src/trigger_pipeline.rs`
- `crates/dtx-input/src/pad.rs`
- `crates/dtx-library/src/lib.rs`
- `crates/dtx-library/src/song_db_sub_acts.rs`
- `crates/dtx-scoring/src/gauge.rs`
- `crates/dtx-scoring/src/hit_ranges.rs`
- `crates/dtx-scoring/src/lib.rs`
- `crates/dtx-scoring/src/score_ini.rs`
- `crates/dtx-scoring/src/xg_score.rs`
- `crates/dtx-scoring/tests/end_to_end_score.rs`
- `crates/dtx-timing/src/lib.rs`
- `crates/dtx-timing/tests/bpm_segment.rs`
- `crates/dtx-ui/src/core_sub_acts.rs`
- `crates/dtx-ui/src/perf_common.rs`
- `crates/game-menu/src/song_loading.rs`
- `crates/game-menu/src/song_select.rs`
- `crates/game-shell/src/performance.rs`
- `crates/game-shell/src/states.rs`
- `crates/gameplay-drums/src/autoplay.rs`
- `crates/gameplay-drums/src/damage_level.rs`
- `crates/gameplay-drums/src/gauge.rs`
- `crates/gameplay-drums/src/hud.rs`
- `crates/gameplay-drums/src/lib.rs`
- `crates/gameplay-drums/src/orchestrator.rs`
- `crates/gameplay-drums/src/perf_common.rs`
- `crates/gameplay-drums/src/perf_hotkeys.rs`
- `crates/gameplay-drums/src/phrase.rs`
- `crates/gameplay-drums/src/resources.rs`
- `crates/gameplay-drums/tests/end_to_end_stage.rs`
- `crates/gameplay-guitar/src/guitar_perf.rs`
- `crates/gameplay-guitar/src/hud.rs`
- `crates/gameplay-guitar/src/lib.rs`
- `crates/gameplay-guitar/src/orchestrator.rs`

### Historical notes, plans, and specs

- `docs/notes/2026-07-12-player-uxui-design-review.md`
- `docs/notes/2026-07-13-game-improvement-program.md`
- `docs/superpowers/plans/2026-07-11-chart-visuals.md`
- `docs/superpowers/plans/2026-07-12-atomic-multi-target-bindings.md`
- `docs/superpowers/specs/2026-07-11-chart-visuals-design.md`
- `docs/superpowers/specs/2026-07-13-documentation-truth-repair-design.md`

### Tooling fixture

- `tools/docs-check/tests/fixtures/stale-root/README.md`

## Repair dispositions

- Source comments, tests, plans, specs, handbooks, and current notes now use
  `references/DTXmaniaNX/`.
- The 2026-07-12 UX review has a dated correction; its original audit claim is
  retained as historical context without presenting it as current state.
- The Cycle 8 documentation design retains its obsolete-root wording as the
  historical problem statement and is allowlisted by `docs-check`.
- The stale-root fixture retains the obsolete token so the checker proves the
  failure contract; repository-mode checking excludes negative fixtures.
- Moved targets were repaired to their actual locations: config under
  `DTXMania/Core/Config`, common performance files directly under
  `Stage/06.Performance`, input under `FDK/Input`, `CPerformanceEntry` inside
  `CScoreIni.cs`, `CChip` under `Score,Song`, and instrument parts in
  `CConstants.cs`.

## Test-quality suspect inventory

The exact suspect scan is:

```sh
rg -n --pcre2 'assert_eq!\(([^,;]+),\s*\1\s*\)|assert!\([^)]*\|\|[^)]*\)|#\[ignore|placeholder' crates app tools --glob '*.rs'
```

Behaviorless findings and dispositions:

- `crates/dtx-core/tests/comprehensive.rs`: replaced the always-true
  `wav_cache` disjunction with an observable BGM chip → measure-keyed
  `"7.wav"` cache mapping.
- `crates/dtx-core/tests/parser_edge_cases.rs`: simplified the redundant
  `is_empty() || len() <= 1` assertion to the actual `len() <= 1` contract.
- `crates/dtx-scoring/tests/comprehensive.rs`: removed self-equality assertions
  for `JudgmentKind::Perfect` and `Rank::S`; boundary classification remains.
- `crates/dtx-scoring/tests/edge_cases.rs`: removed `Rank::S == Rank::S`; the
  same test still protects variant distinction and Hash de-duplication.
- `crates/dtx-ui/src/widget/pad_chips.rs`: replaced `assert_eq!(5, 5)` and the
  no-op flash function with a pure `PadFlashState` reducer. Tests protect an
  exact 120 ms lifetime and reduced-flash `StableOutline` presentation.

Retained matches after repair are behavior-bearing terminology, not tests:

- `crates/dtx-bga/src/lib.rs` describes the colored placeholders that real
  image layers replaced; it is historical implementation context.
- `crates/gameplay-drums/src/editor/bindings_panel.rs` names the actual
  no-device placeholder label shown by the MIDI port row.
- `crates/dtx-ui/src/widget/album_art.rs` owns the public no-art placeholder
  state (`placeholder_alpha`, `with_placeholder_alpha`) and tests its crossfade
  behavior.
- `crates/game-menu/src/song_select.rs` consumes that album-art placeholder for
  songs without `#PREIMAGE` media.

There are no ignored Rust tests and no behaviorless assertion remains in the
repeat scan. `rank_clone_copy` is retained as a compile-surface contract for
the public `Rank: Copy` API; unlike the removed self-comparison it compares the
original binding with a value copied through assignment.

## Final evidence

Recorded 2026-07-13 from the `feat/cycles-6-8` worktree with the shared Cargo
target at `/home/lazykern/lab/dtxmaniars/target`.

### Documentation and documented commands

- `cargo test -p docs-check`: 5 passed, 0 failed, including invalid,
  malformed, and reversed reference-line ranges.
- `cargo run -p docs-check`: 393 repository files checked, 0 failures.
- `cargo run -p dtx-cli -- validate crates/dtx-core/tests/fixtures/minimal.dtx`:
  validated the fixture and reported its metadata and two chips.
- Every safe package/check/test command named by the eleven refreshed crate
  handbooks passed. This included the core compatibility matrix, MP3 decode,
  BGA pan/swap/movie integration, MIDI feature compilation, archive import,
  timing compatibility, Result/Shell/Guitar targets, all focused Drums targets,
  and `cargo test -p gameplay-drums --tests`.
- `cargo check -p dtxmaniars-desktop --features bevy/dynamic_linking` passed
  after the previously uncached optional `bevy_dylib` package was fetched.
- The README install command passed with `--root
  /tmp/dtxmaniars-doc-install`; the installed `dtxmaniars` binary is
  executable.
- `cargo build --release -p dtxmaniars-desktop` passed on the final source.

### Test quality and workspace gates

- The repeated suspect scan contains no ignored tests or behaviorless
  assertion. Remaining `placeholder` matches are the classified functional
  no-art/no-device states listed above.
- `cargo test -p dtx-core --test comprehensive`,
  `cargo test -p dtx-scoring --tests`, and `cargo test -p dtx-ui` passed after
  the test-quality repairs.
- `cargo fmt --all -- --check` passed.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace --lib` passed every library test binary with zero
  failures or ignored tests.
- `git diff --check` passed.

### Independent pre-merge review remediation

The independent review found three semantic gaps and two guarantees that were
present but insufficiently demonstrated. Before merge, the branch was repaired
and reverified so hidden drum chips update empty-hit sound templates, FillIn
start/end state rebuilds across seeks, startup config recovery is presented by
global UI, and reference citations reject zero, reversed, and past-EOF line
ranges. The unused primary-BGM bootstrap path was removed so the only live
scheduler is mixer-gated, and decoded BGA crop rectangles now clamp authored
edges exactly at both zero and media bounds. Focused regression tests cover all
five original review areas. Follow-up remediation also ensures hidden-template WAV
slots participate in the immediate preload/diagnostic tier, notification text
uses the active accessibility scale plus explicit INFO/OK/WARNING/ERROR
markers, the production FixedUpdate ordering exposes MixerAdd before primary
BGM eligibility is observed, and malformed citation-range syntax fails closed.

### Cross-cycle acceptance and scope

Focused and workspace evidence covers one-rate chart/audio/visual/seek/end
behavior; non-qualifying modified/Practice/No Fail results; case-insensitive
DTX/GDA/G2D discovery and deterministic conditionals; media diagnostics/XA
recovery; confidence-gated calibration; normal-play analysis and recommended
Practice loops; composable library discovery; and non-color, scalable,
reduced-effect accessibility behavior. The maintained guides and ADR map pass
the executable documentation checker.

`git diff --name-only main...HEAD` contained 194 program files at final
closeout. A mechanical
path audit found zero CI/CD paths and zero `references/` paths. The main
checkout also reported a clean `git status --short references`. No CI/CD or
vendored reference change is part of this program.

### Manual release-device checks

Commands requiring a window, audible output, or physical hardware were not
misreported as automated evidence. The following remain release-device QA:

- enumerate and play a real MIDI kit, including disconnect/rescan and velocity
  threshold behavior;
- confirm audible output, device/output latency, and guided-calibration feel;
- inspect movie/image presentation, reduced effects, ultrawide layout, and
  2.5–3.5 m drum-kit readability on target displays.

The default and MIDI-feature desktop builds, dynamic-link development build,
release build, installer, synthetic calibration reducers, decoder fixtures,
and geometry/accessibility tests passed mechanically. The manual checks are
environment-dependent validation, not unfinished product implementation.
