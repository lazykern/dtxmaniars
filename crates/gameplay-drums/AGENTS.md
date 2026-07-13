# crates/gameplay-drums

Game-layer owner of the supported drums performance: loading handoff, clocked
input/judgment, sound/system events, score/gauge, HUD, pause, Practice, results
telemetry, and the Customize editor.

## Current mechanics contract

- Twelve bindable channels: HH, SD, BD, HT, LT, FT, CY, HHO, RD, LC, LP, LBD.
  Display lanes may group/reorder channels without changing judgment identity.
- `dtx-input` profiles resolve keyboard/MIDI sources; defaults are the
  reference X/C/Space/V/B/N/M-centered keyboard layout, not numeric keys.
- A fixed 60 Hz schedule orders clock sync, mixer, input, note spawn, judgment,
  and score. `GameplayClock` follows the audio-owned clock, with explicit
  no-BGM behavior only.
- Judgment/grouping, XG score/combo, gauge/damage, scroll math, BGM adjustment,
  hit-sound priority/choke/polyphony, hidden chips, mixer/system events, and
  stage clear/fail are reference-derived mechanics.
- One effective playback rate controls audio, chart time, notes, visuals,
  seeks, and completion. Modified-speed, Practice, and No Fail qualification is
  handed transparently to Results.
- Seeking reconstructs notes, BGM, active sounds, mixer eligibility, BGA state,
  practice attempts, and event cursors.

## Product-owned systems

- `practice/`: A/B loops, whole-chart wrap, tempo/ramp, pre-roll, wait mode,
  attempts/diagnosis, and two-tier HUD.
- `results_analysis`: records normal-play timing events and derives weakest
  lanes/sections for Results handoff.
- `editor/`: Gameplay/Audio/Drums/System/Accessibility/Controls/Lanes/Widgets,
  profile workflows, guided calibration, live layout editing, dirty guards,
  and persistence.
- `pause`, `menu_nav`, `hit_feedback`, HUD/layout, and transitions follow the
  redesigned UX/accessibility contract rather than reference pixel layout.

## Ownership boundary

Pure parsing, scoring formulas, config/layout schemas, and persistence
primitives stay in their Pure crates. Raw input vocabulary stays in
`dtx-input`; audio handles/decoding stay in `dtx-audio`; chart visuals stay in
`dtx-bga`; cross-screen state stays in `game-shell`; result persistence stays
in `game-results`. Systems request transitions rather than writing
`NextState<AppState>`.

## Reference evidence

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfCommonGauge.cs`
- `references/DTXmaniaNX/DTXMania/Score,Song/CChip.cs`

Port mechanics under [ADR-0004](../../docs/decisions/0004-reference-first-mechanics-workflow.md)
and redesign UX under [ADR-0010](../../docs/decisions/0010-port-mechanics-redesign-ux.md).

## Verify

Use the narrow target first:

```sh
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test play_chart
cargo test -p gameplay-drums --test practice_mode
cargo test -p gameplay-drums --test system_events
cargo test -p gameplay-drums --test mixer_events
cargo test -p gameplay-drums --test editor
cargo test -p gameplay-drums --test bindings_lane_pipeline
cargo check -p gameplay-drums
```

Run `cargo test -p gameplay-drums --tests` when the entire crate changed.
Audible sync, MIDI hardware, calibration feel, and visual layout require manual
checks in addition to automated tests.
