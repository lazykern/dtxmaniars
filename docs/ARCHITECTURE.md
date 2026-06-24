# Architecture

## Layers

```
┌─────────────────────────────────────────────────┐
│ Bin     app/dtxmaniars-desktop, tools/dtx-cli   │
├─────────────────────────────────────────────────┤
│ Game    dtx-ui, gameplay-drums, game-shell,     │
│         game-menu, game-results, dev-tools      │
├─────────────────────────────────────────────────┤
│ Engine  dtx-timing, dtx-audio, dtx-input,       │
│         dtx-assets, dtx-library                 │
├─────────────────────────────────────────────────┤
│ Pure    dtx-core, dtx-scoring, dtx-config       │
│         (no bevy; testable w/o GPU)             │
└─────────────────────────────────────────────────┘
```

**Rules:** Pure → no bevy. Engine → may depend on Pure. Game → may depend on Pure + Engine. Bin → wires everything together.

## Crate map

| Crate | Layer | Status | Purpose |
|---|---|---|---|
| `dtx-core` | Pure | M0 | DTX parser, Chart, Chip, EChannel types |
| `dtx-scoring` | Pure | M0+ | Timing windows, judgment types (Perfect/Good/Miss), score rules |
| `dtx-config` | Pure | M0+ | Settings schema (RON), load/save |
| `dtx-timing` | Engine | M1 | Audio-clock authoritative; never judge on `Time::delta()` alone |
| `dtx-audio` | Engine | M1 | `bevy_kira_audio` wrapper; hit-sound manager; BGM stream |
| `dtx-input` | Engine | M3 | Keyboard / MIDI / pad mapping → `LaneHit` events |
| `dtx-assets` | Engine | M3 | Skin manifest loader; handle collections |
| `dtx-library` | Engine | M4 | Async chart scan, metadata DB, presound preview |
| `dtx-ui` | Game | M3+ | Reusable widgets, screen fade constants, animation helpers |
| `gameplay-drums` | Game | M2 | Scroll, judge, score; drums vertical slice |
| `game-shell` | Game | M3 | AppState, loading phases, transitions |
| `game-menu` | Game | M4 | Song select, settings menu |
| `game-results` | Game | M5 | Result screen |
| `dev-tools` | Game | M3+ | Egui inspector, FPS, log viewer (dev-only) |
| `xtask` | Bin | M0+ | matklad-style automation: asset import, release pack |
| `dtxmaniars-desktop` | Bin | M0+ | Binary entrypoint (bevy DefaultPlugins) |
| `dtx-cli` | Bin | M0+ | `validate <file.dtx>`, future tools |

## Data flow (M2 drums vertical slice)

```
.dtx file ─► dtx-core::Chart ─► gameplay-drums
                                   │
                                   ├► dtx-audio ◄─ BGM stream
                                   ├► dtx-input ◄─ keyboard
                                   └► dtx-timing ◄─ audio clock
                                            │
                                            ▼
                                       judgment
                                            │
                                            ▼
                                       dtx-scoring
                                            │
                                            ▼
                                       score events
                                            │
                                            ▼
                                       bevy_ui render
```

## System ordering (drums gameplay)

```
PreUpdate    input poll → LaneHit events
Update       scroll position update → lane state
             judge LaneHit against chart (using audio clock)
             update score
             update combo, gauge
             update visuals (animations, FX)
             update HUD (text)
PostUpdate   despawn expired entities
```

## State machine (AppState)

```
Boot ─► Title ─► SongSelect ─► Loading ─► Playing ─► Result ─► (SongSelect | Title)
                                                       ▲
                                                       └── (Exit → End)
```

Each transition uses 300ms `OutQuint` fade. Loading screen holds ≥1800ms minimum.