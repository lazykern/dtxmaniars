# crates/gameplay-drums — agent scope

**Layer:** Game (bevy + visuals).
**Milestone:** M2.
**Status:** Active.

## Purpose

Vertical slice: load a DTX chart → scroll notes → judge keyboard hits → show score.

## Sub-plugins (one fn per file)

| File | Plugin | Purpose |
|---|---|---|
| `lane_map.rs` | (resource only) | Keyboard → LaneId mapping; default `1-9` |
| `input.rs` | `input::plugin` | KeyDown → `LaneHit` event |
| `scroll.rs` | `scroll::plugin` | Spawn + move `Note` entities + despawn missed |
| `judge.rs` | `judge::plugin` | `LaneHit` + chart → `JudgmentEvent` (nearest-unhit chip, input offset) |
| `score.rs` | `score::plugin` | `JudgmentEvent` → `Score`, `Combo`, `LastJudgment` (XG scoring) |
| `gauge.rs` | `gauge::plugin` | Life gauge (NX deltas, damage-level miss scaling) |
| `miss.rs` | (no-op) | Stub; miss detection lives in `scroll` |
| `hud.rs` | `hud::plugin` | osu-style HUD widgets + playfield layout |
| `interp.rs` | `interp::plugin` | `RenderClock` sub-frame interpolation for smooth scroll |
| `pause.rs` | `pause::plugin` | Esc pause overlay (resume/retry/quit); freezes BGM+clock |
| `perf_hotkeys.rs` | `perf_hotkeys::plugin` | In-song ↑/↓ scroll, ←/→ input adjust, Shift+↑/↓ BGM adjust |
| `stage_end.rs` | `stage_end::plugin` | StageClear / StageFailed banners → Result |
| `sound_bank.rs` | (helpers) | Tiered WAV preload (immediate note WAVs vs deferred BGM/SE) |

## Test

```sh
cargo test -p gameplay-drums
```

## Reference files

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs` — lane visual order (HH..RD), 10-lane default
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs` — key/lane mapping inspiration
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs` — lane X coordinates

## Mechanics (corrected to DTXManiaNX-BocuD)

- 12 lanes (HH, SD, BD, HT, LT, FT, CY, HHO, RD, LC, LP, LBD)
- Keys `1-9`, `0`, `-`, `=` default; users rebind via dtx-config (M3+)
- Timing windows Perfect/Great/Good/Poor = 34/67/84/117ms (`dtx-scoring`)
- **Scoring:** DTXManiaNX XG (`dtx_scoring::xg_score`) —
  `base = (1e6 − 500·bonus) / (1275 + 50·(maxCombo − 50))`, combo ramp 1..50
  then flat ×50, end bonus FC +15k / EXC +30k
  (`CStagePerfCommonScreen.cs:1606-1658`).
- **Combo:** Poor resets combo to 0 (`CStagePerfCommonScreen.cs:1521-1522`).
- **Gauge:** starts 2/3, fails at −0.1, XG deltas `+0.005/+0.001/0/0/−0.017`,
  Miss scaled by damage level (`CActPerfCommonGauge.cs:37-154`, `gauge.rs`).
- **Scroll:** NX `(scrollIdx+1)·0.17875` px/ms at 720px ref, scaled by layout
  (`CChip.cs:568-578`, `resources.rs ScrollSettings`).
- **Judgment routing:** nearest-unhit-chip with lane groups (`drum_groups.rs`).
- **Input offset:** `gameplay.input_offset_ms` shifts the judgement clock
  (`InputOffsetMs`, applied in `judge.rs`).
- **In-song hotkeys:** ↑/↓ scroll, ←/→ input adjust (Ctrl fine), Shift+↑/↓
  per-song BGM adjust, **F11** perf debug overlay (`perf_hotkeys.rs`,
  `CStagePerfCommonScreen.cs:2266-2437`).
- Smoothness: split clock + `RenderClock` sub-frame interpolation (from dtxpt),
  `bevy_framepace` in the desktop binary.
- BPM-change chips handled by `BpmChangeList`.
- Long notes: NOT handled (M6+).

## Config → gameplay

`apply_config_on_enter` (in `lib.rs`) re-reads `dtx-config` on each Performance
entry so Config-screen edits (scroll speed, master volume, damage level, input
offset) apply without an app restart.

## Deferred

- Skin-aware lane rendering (M5+)
- Guitar/bass gameplay, BGA video decode, online

## Rules

- Pure judgment logic in `judge.rs` — testable without bevy.
- All bevy systems in `*::plugin` fn, registered from `lib::plugin`.
- One `Plugin` per file, sub-modules aggregated in `lib.rs`.
- **Port-first (ADR-0010):** Lane order, X positions, scroll direction,
  hit line Y, hit-sound behavior, combo animation, score HUD position,
  judgment text style — all must match DTXManiaNX-BocuD
  (`Stage/06.Performance/DrumsScreen/*`). Cite file:line in commits.
  Scoring/gauge/scroll formulas come from `dtx-scoring` + `gauge.rs` —
  verify those against the reference too.
- **Smoothness/UX is dtxpt-derived engine plumbing** (split clock,
  interpolation, framepace, pause/stage-end overlays, tiered preload) — these
  have no C# analog and follow ADR-0014, not BocuD pixel layouts.