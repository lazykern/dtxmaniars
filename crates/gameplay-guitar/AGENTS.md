# crates/gameplay-guitar

Game-layer crate. Guitar mode vertical slice (M6b).

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs` (29KB)
- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs` (guitar channels 0x21-0x27 / 0x93-0x9A / 0xA2-0xAA)
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CActPerfGuitarLaneFlushGB.cs`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/GuitarScreen/CActPerfGuitarScore.cs`

## Lane layout (M6b minimum)

5-lane standard (BocuD CStagePerfGuitarScreen.cs:99-105 positions [107,146,185,224,264]):

```
Lane 0: R  (red,   low E)
Lane 1: G  (green, A)
Lane 2: B  (blue,  D)
Lane 3: Y  (yellow, G)
Lane 4: P  (pick/pink, B)
```

Open (0x20) and chord channels (0x23, 0x25-0x27, etc.) are NOT mapped in M6b —
they require multi-lane judgment (chord = hit multiple lanes at once) which is
M6.1+. For M6b only the single-note channels trigger the corresponding lane.

## EChannel mapping (single notes only)

DTXManiaNX guitar single-note channels:
- 0x24 → R lane
- 0x22 → G lane
- 0x21 → B lane
- 0x93 → Y lane (M6b: ship after EChannel.rs extension)
- 0xA3 → P lane (M6b: ship after EChannel.rs extension)

For M6b we ship the LANE MAP + input + scoring plumbing. The chord→multi-lane
judge is deferred to M6.1.

## Layer

Game. Depends on `dtx-core`, `dtx-scoring`, `dtx-timing`, `game-shell`.
MIRRORS `gameplay-drums/` structure so that M6.1 can extract shared bits.

## M6b scope (deliverables)

- `lane_map.rs` — LaneMap with 5 lanes, default keys A/S/D/F/G
- `events.rs` — LaneHit, JudgmentEvent, NoteMissed (mirror drums)
- `resources.rs` — ActiveChart, Score, Combo, GameStartMs, JudgmentCounts
- `input.rs` — keyboard → LaneHit system
- `hud.rs` — minimal text: score + combo
- `lib.rs` — plugin assembly
- `EGameMode` enum in `game-shell/src/states.rs`
- `game-shell::performance` branches on `EGameMode` to add the right plugin
- `game-menu::song_select` shows mode, F2 to toggle

## OUT OF SCOPE (deferred)

- Hold notes (CStagePerfGuitarScreen `HoldNote` class) — M6.1
- Open notes / chord judgment — M6.1
- Wailing / RGB effects — M6.1
- Bass mode — M6.2