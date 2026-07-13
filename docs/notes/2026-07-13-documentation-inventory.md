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

Pending Task 7 classification. Every retained match will name the public
contract it protects; behaviorless assertions will be replaced or removed.

## Final evidence

Pending the Cycle 8 closeout. This section will record checker, command,
formatting, workspace, scope, and manual-device results without declaring the
program complete early.
