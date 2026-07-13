# Documentation Truth Inventory

Date: 2026-07-13
Status: In progress until the Cycle 8 final truth gate
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

Pending the Cycle 8 closeout. This section will record checker, command,
formatting, workspace, scope, and manual-device results without declaring the
program complete early.
