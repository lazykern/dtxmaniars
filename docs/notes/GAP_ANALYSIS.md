# GAP_ANALYSIS.md — Honest assessment of DTXManiaRS port completeness

> Generated: 2026-06-24
> Method: per-file LOC, fn count, const count, Bevy-rendering usage, and
> name-match against `references/DTXmaniaNX-BocuD/`. Not marketing.

## Top-line numbers

| Metric | Value |
|---|---:|
| BocuD source | 103,864 LOC across 327 files |
| Rust port | 18,495 LOC across 113 files |
| **Port ratio** | **17.8%** |
| Rust files with ≥3 real pub fns | 38 files, 10,882 LOC |
| Rust files with 0 fns (pure consts) | **6 files, 871 LOC** (the "stubs") |
| BocuD files with no Rust counterpart | 211 files, 43,406 LOC (41.8%) |
| Tests passing | 707 |
| Clippy warnings | 0 |

## What's REAL (verified, has tests, does the work)

The engine layer. ~6,300 Rust LOC, ~205 fns, ~280 tests:

- `dtx-core/parser.rs` — real DTX text parser
- `dtx-core/cdtx_model.rs` — CChip, CachedBpmChange, `compute_playback_time` with BPM changes
- `dtx-core/cscore_ini.rs` — Skill/ClearState, INI parse/save, high score logic
- `dtx-core/c_chip.rs` — 12-field CChip with state machine
- `dtx-core/c_box_set_def.rs` — CBoxDef, CSetDef with lookup + fraction + time_ms
- `dtx-core/cdtx_nested.rs` — Point/Size/Rectangle, CAVIPAN/CBGA/CBGAPAN, CBMP/CBMPTEX, CBPM, CWAV, STRESULT
- `dtx-core/c_chart_data.rs`, `c_avi.rs`, `c_song_list_node.rs`, `enum_converter.rs`, `score_song.rs`
- `dtx-core/fdk_sub_acts.rs` — CTimer, CFPS, CActivity, ESoundDeviceType, CInputManager
- `dtx-library/song_db_sub_acts.rs` — DBState, SongNode tree, 6 sorters, cache
- `dtx-ui/core_sub_acts.rs` — VideoState, Color4, DisplayState, LogLevel
- `dtx-scoring/lib.rs` — `classify` (16/32/64/128ms windows), `Rank::from_perfect_pct`
- `dtx-timing/lib.rs` — `chip_time_ms_with_bpm_changes`
- `dtx-scoring/tests/end_to_end_score.rs` — **real** end-to-end: load → judge → persist → reload

## What's STUB (declarations, no behavior)

6 files, 871 LOC, all `pub const` and trait signatures:

| File | Lines | Stubs |
|---|---:|---|
| `gameplay-guitar/guitar_screen_sub_acts.rs` | 183 | `LANE_COUNT=5, LANE_W=80, SCORE_X=40, COMBO_X=1245, ...` |
| `gameplay-drums/perf_sub_acts.rs` | 178 | `POS_DRUMS=(855,15), WIDTH=20, HEIGHT=540, SLICES=10, ...` |
| `gameplay-drums/perf_common_acts.rs` | 161 | `SCORE_X, COMBO_X, COMBO_BOMB_X, GAUGE_X_DRUMS, ...` |
| `gameplay-drums/drums_screen_sub_acts.rs` | 156 | `DEFAULT_FROM_OUTSIDE, BOMB_X, BOMB_Y, ...` |
| `game-results/result_sub_acts.rs` | 115 | `CHAR_POSITIONS=11, TEXTURE_COUNT=3, ROW_COUNT=7, ...` |
| `gameplay-drums/drawable_sub_acts.rs` | 78 | `JUDGMENT_KINDS=5, DISPLAY_MS=600, PARTICLE_COUNT=16, ...` |

Total stubs: 181 consts. Zero draw fns. These declare *where* things would be drawn if
they were drawn. They are not "ports" in the sense of working code.

## What's MISSING (BocuD files, no Rust counterpart)

211 files, 43,406 LOC (41.8% of BocuD).

### Truly out of scope (ADR-0010)
- `DTXMania/UI/` — 7,000+ LOC of UI/Drawable framework (Bevy replaces)
- `DTXMania/Core/OpenGL/` — 655 LOC (Bevy replaces)
- `DTXCreator/` — 8,600+ LOC (separate editor tool)
- `FDK/Sound/CSoundDeviceWASAPI.cs` — Windows-only
- `FDK/Common/CWin32.cs` — Windows-only
- `CubeTest/`, `Updater/`, online, video decode (FFmpegCore.cs)

**Subtotal out of scope: ~17,000 LOC**

### In scope but not ported (real gap)
- `FDK/Sound/CSound.cs` (1,891) — audio engine (we have kira but integration incomplete)
- `DTXMania/Stage/04.SongSelection/` (5,624) — old song select stage
- `DTXMania/Core/` (4,413) — `CConstants, CSkin, StageManager, CDTXMania` singletons
- `DTXMania/Stage/04.SongSelectionNew/` (2,779) — **we have 11 stub files for these 11 source files; the real port is "consts only"**
- `DTXMania/Stage/05.SongLoading/` (1,111) — loading stage
- `DTXMania/SongDb/SongDb.cs` (878) — the actual SongDb class (we have a partial)
- `DTXMania/Stage/04.SongSelectionNew/CActSelectPresound.cs` (185) — BGM preview

**Subtotal in-scope-not-ported: ~17,000 LOC**

## The rendering gap (what the user actually sees)

| Capability | Status |
|---|---|
| Bevy `Camera2d/Camera3d` | **0 files** |
| Bevy `SpriteBundle/Sprite` | **0 files** |
| Bevy `TextBundle/TextFont` imports | 15 files |
| `commands.spawn` actual usage | 15 files (most are setup helpers) |
| `crates/game-shell/src/title.rs` | **Missing — does not exist** |
| Chip rendering systems | 0 |
| HUD draw systems | 0 |
| BGA rendering | 0 (Bevy uses) |

**The binary opens a window and logs. It does not render anything. The user cannot see the title, navigate the song select, watch chips fall, or see the result.**

## Layer reality check

| Layer | Real? | Notes |
|---|:---:|---|
| Engine (parse, time, judge, persist) | ✓ | Solid, ~6.3k LOC, ~280 tests |
| Library (SongDb) | ✓ partial | 1,086 Rust vs 2,513 BocuD; real sorters/cache, no scan |
| UI framework (dtx-ui) | ✓ | Stubs the OUT-OF-SCOPE UI/Drawable layer; does the in-scope math |
| Config (dtx-config) | ✓ | Wired into binary, real RON load/save |
| BGA | ✗ | Stub layer, no actual video decode (ADR-skip), no rendering |
| Input (dtx-input) | ✗ | Events defined, no consumer wiring |
| Game menu (game-menu) | ✗ | 79 consts, 47 fns — positions + signatures, no actual UI |
| Game shell (game-shell) | ✗ | No title.rs; no state transition systems |
| Performance (gameplay-*) | ✗ | 181 consts, 32 fns — declared, not assembled |
| Result (game-results) | ✗ | 45 consts, 3 fns — declared, not assembled |

## The 60% port target — is it the right metric?

The goal defined "≥60% port ratio" as a verification contract. Calculation:
- BocuD: 103,537 LOC
- 60%: 62,122 Rust LOC
- Current: 18,495 Rust LOC
- Gap: 43,627 Rust LOC

**The 60% target is unreachable in this session, this week, this month.** Reasons:

1. **Out-of-scope work** (~17k BocuD LOC): UI/Drawable, OpenGL, DTXCreator, Win32, CubeTest, Updater. These are not part of the port per ADR-0010. They inflate the denominator.

2. **Substantively replaced** (~3,933 BocuD LOC): `FDK/Sound/CSound.cs` — we use `bevy_kira_audio`, not a 1:1 port of CSound. The kira binding is ~800 LOC of Rust for a 1,891-LOC C# file. The ratio is the wrong metric.

3. **Real-but-tiny ports** (~6,300 Rust LOC of working engine): the engine layer is real and tested but is a small fraction of the total because BocuD has a lot of UI glue.

4. **The "60%" definition** assumed a 1:1 line-for-line translation. With Bevy replacing 17k LOC of UI + kira replacing 1,891 LOC of audio, a fair port ratio would be against an *adjusted denominator* of ~70k LOC (excluding out-of-scope), giving a current ratio of 26% — and a 60%-of-adjusted-denominator target of ~42k Rust LOC, still ~24k short of the work needed.

## What the goal ACTUALLY achieved

In this session and the prior one, the work produced:

1. **A working DTX engine** (parser, timing, judgment, persistence, FDK primitives) — 707 tests, 0 clippy warnings, fmt clean.
2. **A real end-to-end test** that exercises the engine: load real_chart.dtx, judge chips, persist to score.ini, reload, verify.
3. **A binary that boots** with dtx-config wiring, score store, and config log.
4. **The architectural framework** for 9 stages (Startup/Title/Config/SongSelect/SongLoading/Performance/Result/ChangeSkin/End) and 2 game modes (Drums/Guitar) — declared and wired, but no actual screens render.

The work is real. The 60% LOC target is the wrong ruler for what was built.

## Recommended path forward

Three options, ranked by user effort:

### (a) Honest completion — accept engine scope
- Update goal verification contract: 60% port ratio replaced with "engine layer complete + binary boots"
- Document the game-layer gap as M7+ work
- Mark complete with PORT_GAP.md as the truthful artifact

### (b) Add rendering pipeline (5–8k Rust LOC, 1–2 sessions)
- Add `commands.spawn` camera in main.rs
- Real Title screen: 1 file, ~200 LOC
- Wire SongSelect row select → existing engine
- Render performance chips using existing judge.rs
- Render result using existing CScoreIni
- This makes the game **playable** even if it doesn't hit 60% LOC

### (c) Lower verification target to engine scope
- Re-target 60% → 25% (current), regenerate verification contract
- Mark complete
- Add M7+ roadmap for the game-layer rendering work

The 60% LOC target is not achievable for the in-scope code without a multi-month effort. The engine is real, the game is a skeleton, and the user should decide which matters.
