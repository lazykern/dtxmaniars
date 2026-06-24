# GAP_ANALYSIS.md — DTXManiaRS port, file-by-file

> Generated: 2026-06-24 (post-Phase-D session)
> Method: per-file 1:1 map `references/DTXmaniaNX-BocuD/<path>` ↔ `crates/*/src/<file>.rs`.
> Counts: real fns, real behavior, dead code, duplicates, missing.

## Top-line (current state)

| Metric | Value |
|---|---:|
| In-scope ref files | **185** (.cs, ~71,544 LOC) |
| Rust source files | **104** (.rs, 21,456 LOC) |
| Rust source + tests | 22,887 LOC |
| **Dead-code stubs** | **0** (all 6 deleted in Phase A) |
| Duplicate plugins | **0** (3 pairs merged in Phase A) |
| Workspace tests | **826** passing, 2 ignored, 0 failed |
| Clippy warnings | 0 |
| Binary boots | Startup → Title ✅ |
| End-to-end orchestrator tests | 7 in `gameplay-drums/tests/end_to_end_stage.rs` |

## Session progress (6 commits)

| Commit | Phase | LoC | Tests |
|---|---|---:|---:|
| `4f54f55` | A: cleanup dead stubs + merge duplicates | -1,368 | 689 |
| `765bdcb` | B: 18 sub-acts + 2 orchestrators | +1,895 | 756 |
| `fde13c3` | C-partial: CPad + CConstants | +714 | 783 |
| `8b3341a` | C-complete: CSkin + CKeyAssign | +817 | 802 |
| `a7d193a` | D-1: CDTX registries + asset parsers | +538 | 819 |
| `7681239` | D-2: end-to-end orchestrator tests | +187 | 826 |

**Net session:** +2,783 source LoC, +83 tests.

## What changed since the original GAP_ANALYSIS

### 06.Performance sub-acts (Phase B)
**Status: 18 of ~28 sub-acts now have real Rust counterparts.**

- `dtx-ui/src/perf_common.rs` (272 LoC) — base types shared by drums+guitar:
  - `LaneFlushGB` (10-lane flush state, CActPerfCommonLaneFlushGB.cs:70)
  - `RgbState` (10-pressed-state array, CActPerfCommonRGB.cs:59)
  - `DangerState` + `DangerInstrument` (3-instrument danger, CActPerfCommonDanger.cs:57)
  - `WailingBonusState` (bonus trigger, CActPerfCommonWailingBonus.cs:43)
- `gameplay-drums/src/drums_perf.rs` (498 LoC) — drums-specific:
  - `DrumsLane` enum + `PadPosition` + `PadRect` (10-lane pad table verbatim from CActPerfDrumsPad.cs:498)
  - `DrumsPadState` (press/release + position lookup)
  - `DrumsDangerState` (ct_move + ct_opacity, CActPerfDrumsDanger.cs:27-28)
  - `DrumsFillingEffect` (marker, CActPerfDrumsFillingEffect.cs:41)
- `gameplay-guitar/src/guitar_perf.rs` (612 LoC) — guitar/bass:
  - 11 guitar sub-acts (Score, Combo, Gauge, StatusPanel, JudgementString, LaneFlushGB, RGB, Danger, WailingBonus, Bonus, HoldNote)
- `gameplay-drums/src/orchestrator.rs` (300 LoC) + `gameplay-guitar/src/orchestrator.rs` (200 LoC) — port of CStagePerfDrumsScreen.cs:3671 + CStagePerfGuitarScreen.cs:787+.Chip.cs:808
  - `*StageRoot` Component + `*StageCompletion` Resource
  - `on_enter_performance` / `on_exit_performance` / `detect_end_of_stage` systems
  - `chart_end_ms` computation (BPM-aware approximation)

### Core singletons (Phase C)
**Status: 4 of 5 singletons ported. CConfigIni individual fields partial.**

- `dtx-input/src/pad.rs` (310 LoC) — CPad.cs:278 fully ported (DetectedDevice, InstrumentPart, Pad, KeyAssign, PadDispatcher)
- `dtx-core/src/constants.rs` (310 LoC) — CConstants.cs:780 fully ported (LaneType, RdPosition, DarkMode, DamageLevel, RandomMode, InstrumentPart, Judgement, Lane, CYGroup/FTGroup/HHGroup/BDGroup, PlaybackPriority)
- `dtx-ui/src/skin.rs` (280 LoC) — CSkin.cs:1147 partially ported (ESystemSound + SkinResolver; texture loading is Bevy AssetServer)
- `dtx-config/src/key_assign.rs` (521 LoC) — CConfigIni.CKeyAssign.cs:14-435 fully ported (KeyAssignPart, KeyAssignPad, STKeyAssign, KeyAssignTable with BocuD default key map)
- ⚠️ `dtx-config/src/lib.rs` — System/Gameplay/Audio/Skin sections (316 LoC). ~3,500 LoC of individual config bool/int/STDGBVALUE fields remain unported.
- ⚠️ `game-shell/src/fade.rs` (204 LoC) — StageManager.cs:699 partial (fade state machine only).

### CDTX.cs (Phase D)
**Status: extended from 1,189 LoC to ~2,200 LoC of Rust. ~30% of CDTX.cs LoC ported by file count; structural coverage ~70% (rendering methods are Bevy/kira territory).**

- `crates/dtx-core/src/cdtx_model.rs` (463 LoC) — `CDTX` struct extended with:
  - `wav_registry`, `bmp_registry`, `avi_registry`, `bga_registry` (BocuD listWAV/listBMP/listAVI/listBGA)
  - `bpm_array: [f32; 256]` (BocuD nBPM 0x00..0xFF)
  - `has_chips: HasChips` (BocuD STHASCHIPS)
  - `raw_lines: Vec<String>` (BocuD listDTXManiaFormat)
  - `WavEntry` / `BmpEntry` / `AviEntry` / `BgaEntry` / `HasChips` structs
- `crates/dtx-core/src/assets.rs` (459 LoC) — `#WAVxx` / `#BMPxx` / `#AVIxx` / `#BGAxx` / `#BGAPANxx` / `#BPMxx` directive parsers + `DtxAssets` aggregate
- ⚠️ CBMP / CBMPTEX / CBMPbase / CWAV / CAVIPAN / CBGA / CBGAPAN actual rendering logic — Bevy AssetServer + kira audio territory
- ⚠️ STLANEINT per-instrument struct — partial (HasChips covers the high-level booleans)
- ⚠️ listBPM / listBarLength / listWAVbarrel — partial

### End-to-end integration tests
- `crates/gameplay-drums/tests/end_to_end_stage.rs` (200 LoC, 7 tests):
  - end_to_end_enter_performance_captures_end_ms
  - end_to_end_detect_end_triggers_result_transition
  - end_to_end_detect_no_transition_when_audio_before_end
  - end_to_end_detect_no_transition_when_audio_clock_none
  - end_to_end_end_requested_flag_prevents_duplicate
  - end_to_end_on_exit_clears_completion_state
  - end_to_end_empty_chart_no_transition

## Honest assessment vs goal objective

The goal's hard "100%" requirements are not met:

1. **"100% sub-act coverage"** — 18 of ~28 sub-acts have real Rust types with positions/state; many are position-consts-only without actual rendering systems.
2. **"All 9 stages render with real behavior"** — 8 stages spawn Bevy UI (commands.spawn verified); End is text-only. Performance sub-acts are position-consts; rendering systems are partial.
3. **"All 185 in-scope ref files have a Rust counterpart with real logic"** — ~60% file coverage by LoC; ~70% structural coverage; rendering class files (CBMP, CWAV, etc.) are Bevy/kira territory so the LoC port is not 1:1.
4. **"Binary playable end-to-end"** — Binary boots Startup→Title. State machine works in tests. Live play-through (select song → load → render → judge → result) has not been manually verified.

## LoC metric discussion (carried from prior analysis)

The original 60% LoC target is unachievable for the in-scope code:
- Bevy replaces ~7,000 LoC of UI/Drawable framework
- kira replaces ~1,891 LoC of CSound.cs + 1,605 LoC of platform backends
- Adjusted denominator: ~60k LoC in-scope

Current ratio: 21,456 / 60,000 = **35.8%** (was 17.8% pre-session).

## What's still missing (multi-session effort)

### High impact
- **Render systems for 06.Performance sub-acts** — turn position consts into actual `commands.spawn` systems (currently most are in const tables; rendering is in `hud.rs` for a subset)
- **CConfigIni individual fields** — 250+ bool/int/STDGBVALUE fields, mechanical but voluminous (~3,500 LoC)
- **CStagePerfCommonScreen.cs:5067 orchestrator** — currently logic is duplicated across drums+guitar orchestrators; could be extracted into a shared module
- **Live play-through verification** — test that SongSelect→SongLoading→Performance→Result actually works with real chart + BGM

### Medium impact
- **04.SongSelection/ old variant** (5,624 LoC) — out of scope per ADR-0010 (we use SongSelectionNew)
- **CBMP / CBMPTEX / CWAV rendering** — Bevy AssetServer + kira audio territory
- **DTXMania/Core/CDTXMania.cs:612 singleton** — Bevy App replaces

### Low impact
- **FDK backends** (CSoundDeviceWASAPI/DirectSound/ASIO) — out of scope (Windows-only, kblocked by ADR-0010)
- **DTXCreator** (8,600+ LoC) — separate editor tool, out of scope
- **Online / multiplayer** — BocuD has none
- **CUpdater** — auto-updater, out of scope

## Boundaries (still in force from ADR-0010)

- ✅ Bevy replaces UI/Drawable framework
- ✅ kira replaces CSound.cs + 3 platform backends
- ✅ Real video decode is out of scope (per goal)
- ✅ Port-first: every sub-act's positions verbatim from reference
- ❌ No "modernization" without ADR override

## Quality gates

- `cargo test --workspace`: 826 pass, 0 fail, 2 ignored
- `cargo clippy --workspace --all-targets`: 0 issues
- `cargo fmt --check`: clean
- Binary: boots Startup→Title
- End-to-end orchestrator: 7 tests covering state machine
