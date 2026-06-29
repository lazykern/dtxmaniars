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
| `judge.rs` | `judge::plugin` | `LaneHit` + chart → `JudgmentEvent` |
| `score.rs` | `score::plugin` | `JudgmentEvent` → `Score`, `Combo`, `LastJudgment` |
| `miss.rs` | (no-op) | Stub; miss detection lives in `scroll` |
| `hud.rs` | `hud::plugin` | osu-style HUD widgets + playfield layout |

## Test

```sh
cargo test -p gameplay-drums
```

## Reference files

- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs` — lane visual order (HH..RD), 10-lane default
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs` — key/lane mapping inspiration
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs` — lane X coordinates

## v1 scope (M2)

- 12 lanes (HH, SD, BD, HT, LT, FT, CY, HHO, RD, LC, LP, LBD)
- Keys `1-9`, `0`, `-`, `=` default; users rebind via dtx-config (M3+)
- DTXmaniaNX/BocuD timing windows (Perfect/Great/Good/Poor = 34/67/84/117ms)
  and current score points (Perfect=2, Great/Good=1, Poor/Miss=0)
- osu-style HUD with `PlayfieldLayout`, gauge, keyboard viz (ADR-0014)
- BPM-change chips handled by `BpmChangeList`
- Long notes: NOT handled (M6+)

## Deferred

- Skin-aware lane rendering (M5+)
- Audio latency compensation

## Rules

- Pure judgment logic in `judge.rs` — testable without bevy.
- All bevy systems in `*::plugin` fn, registered from `lib::plugin`.
- One `Plugin` per file, sub-modules aggregated in `lib.rs`.
- **Port-first (ADR-0010):** Lane order, X positions, scroll direction,
  hit line Y, hit-sound behavior, combo animation, score HUD position,
  judgment text style — all must match DTXManiaNX-BocuD
  (`Stage/06.Performance/DrumsScreen/*`). Cite file:line in commits.
  Score values (Perfect=2, etc.) come from `dtx-scoring` — verify those
  against the reference too.