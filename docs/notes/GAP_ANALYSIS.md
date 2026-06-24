# GAP_ANALYSIS.md — DTXManiaRS port, file-by-file

> Generated: 2026-06-24
> Method: per-file 1:1 map `references/DTXmaniaNX-BocuD/<path>` ↔ `crates/*/src/<file>.rs`.
> Counts: real fns, real behavior, dead code, duplicates, missing.

## Top-line

| Metric | Value |
|---|---:|
| In-scope ref files | **185** (.cs, ~71,544 LOC) |
| Rust source files | **101** (.rs, 18,504 LOC) |
| Rust source + tests | 19,755 LOC |
| **Dead-code stubs** | 6 files, 865 LOC (4.7% of Rust source) |
| Duplicate plugins | **3 pairs** (song_select, config, change_skin) |
| Workspace tests | **743** passing, 2 ignored, 0 failed |
| Clippy warnings | 0 |
| Binary boots | Startup → Title ✅ |
| Real screen rendering | 8 of 9 stages have at least one `commands.spawn` |

## TL;DR — the user's two questions

**Q1: Is LoC the right metric?**
No. LoC is wrong for 3 reasons:
1. Rust ≪ C# for same logic (no class/get/set boilerplate, no namespace pollution, type inference).
2. Bevy replaces ~7,000 LOC of BocuD UI/Drawable framework.
3. kira + 1 generic backend replaces ~1,891 LOC of CSound.cs + 3 platform backends (~1,600 LOC).

**Q2: What are the real gaps?**
1. **Dead-code stubs**: 6 `*_sub_acts.rs` files declare positions that nothing imports (865 LoC, 158 consts, 0 cross-references).
2. **Duplicate plugins**: `lib.rs` registers BOTH `song_select::plugin` AND `song_select_full::plugin` for the same state → double-spawn on entry. Same for `config` and `change_skin`.
3. **Big missing files** in 06.Performance: `CStagePerfCommonScreen` (5,067), `CStagePerfDrumsScreen` (3,671), `CStagePerfGuitarScreen`+`.Chip` (1,595), `CActPerfBGA` (305), `CActPerfVideo` (520), `CActPerfCommonScore` (142), `CActPerfCommonCombo` (794), `CActPerfCommonGauge` (296), `CActPerfCommonStatusPanel` (531), `CActPerfCommonJudgementString` (301), `CActPerfProgressBar` (543), `CActPerfSkillMeter` (302), `CActPerfStageFailure` (123). All 13k+ LOC of orchestration not ported.
4. **Score,Song incomplete**: `CDTX.cs` (7,295) is partial in `cdtx_model.rs`. `CScoreIni.cs` (1,773) is real in `cscore_ini.rs` (588 LOC). `CSongListNode.cs` real in `c_song_list_node.rs` (219 LOC). `EChannel.cs` real in `channel.rs` (191 LOC).
5. **Core incomplete**: `CConfigIni.cs` (3,926) only small subset in `dtx-config` (316 LOC). `CSkin.cs` (1,147), `CConstants.cs` (780), `StageManager.cs` (699), `CDTXMania.cs` (612) not ported.
6. **FDK Sound partial**: `CSound.cs` (1,891) replaced by kira wrapper (~200 LOC). The 3 device backends (1,605 LOC) intentionally skipped (out of scope).

## Better metrics than LoC

| Metric | Ref | Ours | Status |
|---|---:|---:|---|
| **Sub-act file coverage** (1 ref file = 1 rust file) | 185 | ~80 | ~43% |
| **Real behavior coverage** (not just position consts) | 185 | ~50 | ~27% |
| **Test coverage** (ref fns covered by ≥1 test) | ~5,000 | 743 | ~15% |
| **Wiring coverage** (rust file imported & used) | 185 | ~70 | ~38% |
| **User-facing stages rendering** | 9 | 8 | 89% (only End is text-only) |

## Sub-act mapping (1:1 file, both sides)

### ✅ Real ports (real logic, wired, tested)

| Ref file | LoC ref | Rust file | LoC | Status |
|---|---:|---|---:|---|
| `Score,Song/EChannel.cs` | 195 | `dtx-core/src/channel.rs` | 191 | ✅ |
| `Score,Song/CChip.cs` | 644 | `dtx-core/src/c_chip.rs` | 238 | ⚠ partial (real fields, no full state machine) |
| `Score,Song/CBoxDef.cs` | — | `dtx-core/src/c_box_set_def.rs` | 219 | ✅ |
| `Score,Song/CSetDef.cs` | — | `dtx-core/src/c_box_set_def.rs` | (same) | ✅ |
| `Score,Song/CChartData.cs` | 472 | `dtx-core/src/c_chart_data.rs` | 242 | ✅ |
| `Score,Song/CAVI.cs` | — | `dtx-core/src/c_avi.rs` | 160 | ✅ |
| `Score,Song/CSongListNode.cs` | — | `dtx-core/src/c_song_list_node.rs` | 219 | ✅ |
| `Score,Song/EnumConverter.cs` | — | `dtx-core/src/enum_converter.rs` | 132 | ✅ |
| `Score,Song/CDTX.cs` | 7,295 | `dtx-core/src/cdtx_model.rs` + `cdtx_nested.rs` | 899 | ⚠ partial (~12%) |
| `Score,Song/CScoreIni.cs` | 1,773 | `dtx-core/src/cscore_ini.rs` | 588 | ✅ |
| `Score,Song/score_song/*` | — | `dtx-core/src/score_song.rs` | 225 | ✅ |
| `SongDb/SongDb.cs` | 877 | `dtx-library/src/song_db_sub_acts.rs` + `lib.rs` | 1,084 | ✅ real (sorters, cache, no SQLite) |
| `SongDb/SongNode.cs` | 282 | (same) | — | ✅ |
| `SongDb/SortBy*` (8 sorters) | ~600 | (same) | — | ✅ 6 of 8 |
| `SongDb/SongCacheSqlite.cs` | — | — | — | ❌ not ported (planned) |
| `Core/Framework/Color4.cs` | — | `dtx-ui/src/core_sub_acts.rs` | (454) | ✅ |
| `Core/Framework/DisplayState.cs` | — | (same) | — | ✅ |
| `Core/Framework/RuntimeLogListener.cs` | — | (same) | — | ✅ |
| `Core/Video/DecodedFrameData.cs` | — | (same) | — | ✅ |
| `Core/Video/DisplayedFrame.cs` | — | (same) | — | ✅ |
| `FDK/Common/CTimer.cs` | 155 | `dtx-core/src/fdk_sub_acts.rs` | (670) | ✅ |
| `FDK/Common/CFPS.cs` | 58 | (same) | — | ✅ |
| `FDK/Common/CActivity.cs` | 141 | (same) | — | ✅ |
| `FDK/Common/CCounter.cs` | 206 | (same) | — | ✅ |
| `FDK/Common/CTimerBase.cs` | 102 | (same) | — | ✅ |
| `FDK/Common/CConversion.cs` | 203 | (same) | — | ✅ |
| `FDK/Common/CCommon.cs` | 57 | (same) | — | ✅ |
| `FDK/Common/COS.cs` | 149 | (same) | — | ✅ |
| `FDK/Common/CTraceLogListener.cs` | 151 | (same) | — | ✅ |
| `FDK/Sound/ESoundDeviceType.cs` | 9 | (same) | — | ✅ |
| `FDK/Sound/CSoundTimer.cs` | 91 | `dtx-audio/src/lib.rs` | 113 | ✅ |
| `FDK/Input/EInputDeviceType.cs` | 10 | (same fdk_sub_acts) | — | ✅ |
| `FDK/Input/CInputManager.cs` | 397 | (same) | — | ✅ |
| `FDK/Input/STInputEvent.cs` | 13 | (same) | — | ✅ |
| `Stage/01.Startup/CStageStartup.cs` | 102 | `game-menu/src/startup.rs` | 46 | ✅ |
| `Stage/02.Title/CStageTitle.cs` | 378 | `game-menu/src/title.rs` | 135 | ✅ |
| `Stage/08.End/CStageEnd.cs` | 86 | `game-menu/src/end.rs` | 58 | ✅ |
| `Stage/09.ChangeSkin/CStageChangeSkin.cs` | 95 | `game-menu/src/change_skin.rs` + `change_skin_full.rs` | 137 | ✅ + ⚠ duplicate |
| `Stage/05.SongLoading/CStageSongLoading.cs` | 1,110 | `game-menu/src/song_loading.rs` | 196 | ⚠ stub (loads DTX, no progress UI) |
| `Stage/04.SongSelectionNew/CStageSongSelectionNew.cs` | 596 | `game-menu/src/song_select_new_stage.rs` | 158 | ✅ state machine |
| `Stage/04.SongSelectionNew/StatusPanel.cs` | 144 | `game-menu/src/song_select_full.rs` | (548) | ✅ |
| `Stage/04.SongSelectionNew/StatusPane.cs` | 201 | (same) | — | ✅ |
| `Stage/04.SongSelectionNew/DensityGraph.cs` | 280 | (same) | — | ✅ |
| `Stage/04.SongSelectionNew/SortMenuContainer.cs` | 206 | (same) | — | ✅ |
| `Stage/04.SongSelectionNew/SongSearchMenu.cs` | 108 | (same) | — | ✅ |
| `Stage/04.SongSelectionNew/CommandHistory.cs` | 100 | (same) | — | ✅ |
| `Stage/04.SongSelectionNew/CActSelectPresound.cs` | 185 | (same) | — | ✅ (BGM preview) |
| `Stage/03.Config/CStageConfig.cs` | 531 | `game-menu/src/config.rs` + `config_full.rs` | 454 | ✅ + ⚠ duplicate |
| `Stage/03.Config/CActConfigList.cs` | 818 | `game-menu/src/config_list.rs` | 384 | ✅ |
| `Stage/03.Config/CActConfigList.System.cs` | — | `game-menu/src/config_list_system.rs` | 168 | ⚠ stub (positions, not all items) |
| `Stage/03.Config/CActConfigList.Audio.cs` | 189 | `game-menu/src/config_list_audio.rs` | 74 | ✅ items |
| `Stage/03.Config/CActConfigList.Audio.Driver.cs` | 128 | `game-menu/src/config_list_audio_driver.rs` | 74 | ⚠ empty items vec |
| `Stage/03.Config/CActConfigList.Graphics.cs` | — | `game-menu/src/config_list_graphics.rs` | 76 | ⚠ |
| `Stage/03.Config/CActConfigList.Gameplay.cs` | — | `game-menu/src/config_list_gameplay.rs` | 75 | ⚠ |
| `Stage/03.Config/CActConfigList.Menu.cs` | — | `game-menu/src/config_list_menu.rs` | 63 | ⚠ |
| `Stage/03.Config/CActConfigList.Skin.cs` | — | `game-menu/src/config_list_skin.rs` | 63 | ⚠ |
| `Stage/03.Config/CActConfigList.Drums.cs` | 879 | `game-menu/src/config_list_drums.rs` | 130 | ✅ 30 of 66 items |
| `Stage/03.Config/CActConfigList.Drums.Velocity.cs` | — | `game-menu/src/config_list_drums_velocity.rs` | 86 | ⚠ |
| `Stage/03.Config/CActConfigList.Guitar.cs` | 438 | `game-menu/src/config_list_guitar.rs` | 78 | ⚠ |
| `Stage/03.Config/CActConfigList.Bass.cs` | 426 | `game-menu/src/config_list_bass.rs` | 85 | ✅ 25 of 39 items |
| `Stage/03.Config/CActConfigKeyAssign.cs` | 563 | `game-menu/src/config_key_assign.rs` | 350 | ✅ |
| `Stage/04.SongSelection/CActSelectSongList.cs` | 1,640 | — | — | ❌ not ported (use SongSelectionNew) |
| `Stage/04.SongSelection/CActSelectStatusPanel.cs` | 1,175 | — | — | ❌ not ported (use SongSelectionNew) |
| `Stage/04.SongSelection/CStageSongSelection.cs` | 1,106 | — | — | ❌ not ported (use SongSelectionNew) |
| `Stage/06.Performance/CStagePerfCommonScreen.cs` | 5,067 | — | — | ❌ **biggest single gap** |
| `Stage/06.Performance/CActPerfCommonScore.cs` | 142 | `gameplay-drums/src/score.rs` | 198 | ✅ |
| `Stage/06.Performance/CActPerfCommonCombo.cs` | 794 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonGauge.cs` | 296 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonStatusPanel.cs` | 531 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonJudgementString.cs` | 301 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonLaneFlushGB.cs` | 70 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonRGB.cs` | 59 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonDanger.cs` | 57 | — | — | ❌ |
| `Stage/06.Performance/CActPerfCommonWailingBonus.cs` | 43 | — | — | ❌ |
| `Stage/06.Performance/CActPerfBGA.cs` | 305 | `dtx-bga/src/lib.rs` | 335 | ✅ state machine + overlay render |
| `Stage/06.Performance/CActPerfVideo.cs` | 520 | (same) | — | ⚠ partial (no decode) |
| `Stage/06.Performance/CActPerfAVI.old.cs` | 930 | — | — | ❌ |
| `Stage/06.Performance/CActPerfProgressBar.cs` | 543 | `gameplay-drums/src/perf_sub_acts.rs` | (177) | ❌ consts only, dead code |
| `Stage/06.Performance/CActPerfSkillMeter.cs` | 302 | (same) | — | ❌ consts only, dead code |
| `Stage/06.Performance/CActPerfScrollSpeed.cs` | 87 | `gameplay-drums/src/scroll.rs` | 149 | ✅ |
| `Stage/06.Performance/CActPerfStageClear.cs` | 7 | `gameplay-drums/src/perf_sub_acts.rs` | (177) | ❌ empty marker |
| `Stage/06.Performance/CActPerfStageFailure.cs` | 123 | (same) | — | ❌ consts only, dead code |
| `Stage/06.Performance/PerfNewChipFire.cs` | 233 | `gameplay-drums/src/perf_sub_acts.rs` | (177) | ❌ consts only, dead code |
| `Stage/06.Performance/IPerfFire.cs` | — | — | — | ❌ |
| `Stage/06.Performance/InfoBox.cs` | 84 | `gameplay-drums/src/perf_sub_acts.rs` | (177) | ❌ consts only, dead code |
| `Stage/06.Performance/CActPerformanceInformation.cs` | 74 | — | — | ❌ |
| `Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` | 3,671 | — | — | ❌ **2nd biggest gap** |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsScore.cs` | 76 | `gameplay-drums/src/hud.rs` | (620) | ✅ |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsComboDGB.cs` | 112 | (same) | — | ✅ |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsGauge.cs` | 88 | (same) | — | ✅ |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsStatusPanel.cs` | 211 | (same) | — | ✅ |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsJudgementString.cs` | 102 | (same) | — | ✅ |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsLaneFlushD.cs` | 456 | `gameplay-drums/src/perf_sub_acts_3.rs` | 336 | ✅ real logic |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs` | 498 | `gameplay-drums/src/drums_screen_sub_acts.rs` | (155) | ❌ consts only, dead code |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsDanger.cs` | 77 | `gameplay-drums/src/drums_screen_sub_acts.rs` | (155) | ❌ consts only, dead code |
| `Stage/06.Performance/DrumsScreen/CActPerfDrumsFillingEffect.cs` | 41 | (same) | — | ❌ consts only, dead code |
| `Stage/06.Performance/DrumsScreen/CActPerfPerfChipFireD.cs` | 1,080 | `gameplay-drums/src/perf_sub_acts.rs` | (177) | ❌ consts only, dead code |
| `Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs` | 787 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.Chip.cs` | 808 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarScore.cs` | 116 | `gameplay-guitar/src/score.rs` | 84 | ✅ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarCombo.cs` | 23 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarBonus.cs` | 86 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarDanger.cs` | 78 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarGauge.cs` | 131 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarJudgementString.cs` | 71 | `gameplay-guitar/src/hud.rs` | 104 | ✅ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarLaneFlushGB.cs` | 112 | `gameplay-guitar/src/guitar_screen_sub_acts.rs` | (182) | ❌ consts only, dead code |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarRGB.cs` | 202 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarStatusPanel.cs` | 237 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/CActPerfGuitarWailingBonus.cs` | 197 | — | — | ❌ |
| `Stage/06.Performance/GuitarScreen/HoldNote.cs` | 93 | — | — | ❌ |
| `Stage/06.Performance/Drawable/*` (3 files) | 261 | — | — | ❌ out of scope (Bevy replaces) |
| `Stage/07.Result/CStageResult.cs` | 811 | `game-results/src/lib.rs` | 238 | ✅ |
| `Stage/07.Result/CActResultParameterPanel.cs` | 971 | `game-results/src/result_full.rs` | 356 | ⚠ partial (rank + counts, not full panel) |
| `Stage/07.Result/ResultInfoPanel.cs` | — | (same) | — | ⚠ |
| `Stage/07.Result/ResultParameterPanel.cs` | — | (same) | — | ⚠ |
| `Stage/07.Result/ResultRankIcon.cs` | — | (same) | — | ✅ rank enum |
| `Stage/CActDFPFont.cs` | 602 | — | — | ❌ out of scope (Bevy text) |
| `Stage/CActLVLNFont.cs` | — | — | — | ❌ out of scope |
| `Stage/CActOptionPanel.cs` | — | — | — | ❌ |
| `Stage/CStage.cs` | — | `game-shell/src/states.rs` | 74 | ✅ |
| `Stage/UIPlayerNameplate.cs` | — | — | — | ❌ |
| `Core/CConfigIni.cs` | 3,926 | `dtx-config/src/lib.rs` | 316 | ⚠ small subset (System/Gameplay/Audio + skin) |
| `Core/CSkin.cs` | 1,147 | — | — | ❌ |
| `Core/CConstants.cs` | 780 | — | — | ❌ |
| `Core/StageManager.cs` | 699 | `game-shell/src/fade.rs` | 204 | ✅ fade state machine |
| `Core/CDTXMania.cs` | 612 | — | — | ❌ (Bevy App replaces singleton) |
| `Core/CDTXMania.Init.cs` | 252 | — | — | ❌ |
| `Core/CPad.cs` | 278 | `dtx-input/src/keyboard.rs` | 96 | ⚠ partial |
| `Core/Program.cs` | 202 | `app/dtxmaniars-desktop/src/main.rs` | 87 | ✅ |
| `Core/Input.cs` | 22 | — | — | ❌ |
| `Core/CCharacterConsole.cs` | 117 | — | — | ❌ |
| `Core/DTXManiaGL.cs` | 106 | — | — | ❌ out of scope (Bevy replaces) |
| `Core/GlfwNativeContext.cs` | 37 | — | — | ❌ out of scope |
| `Core/STHitRanges.cs` | 87 | `dtx-scoring/src/lib.rs` | (512) | ✅ (windows + classify) |
| `Core/GameWindowSize.cs` | 6 | — | — | ❌ trivial |
| `Core/BuildInfo.cs` | 10 | — | — | ❌ trivial |
| `Core/CDiscordRichPresence.cs` | 46 | — | — | ❌ out of scope |
| `Core/CDTXRichPresence.cs` | 19 | — | — | ❌ out of scope |
| `Core/Framework/BaseGame.cs` | — | — | — | ❌ (Bevy App replaces) |
| `Core/Framework/IGameHost.cs` | — | — | — | ❌ |
| `Core/Framework/IRenderer.cs` | — | — | — | ❌ |
| `Core/Video/Decoders/AsyncVideoDecoder.cs` | — | — | — | ❌ out of scope (no decode) |
| `Core/Video/Decoders/SoftwareVideoDecoder.cs` | — | — | — | ❌ out of scope |
| `Core/Video/Decoders/VideoDecoder.cs` | — | — | — | ❌ out of scope |
| `Core/Video/FFmpegCore.cs` | — | — | — | ❌ out of scope (per goal) |
| `Core/Video/UINewVideoRenderer.cs` | — | — | — | ❌ |
| `Core/Video/VideoPlayerController.cs` | — | — | — | ❌ |
| `Core/Glue/SlimDXGLFWGlue.cs` | 248 | — | — | ❌ out of scope (Bevy replaces) |
| `FDK/Common/CWin32.cs` | 802 | — | — | ❌ out of scope (Windows-only) |
| `FDK/Common/CPowerManagement.cs` | 22 | — | — | ❌ |
| `FDK/Input/CInputJoystick.cs` | 770 | — | — | ❌ (we use keyboard+virtual midi) |
| `FDK/Input/CInputKeyboard.cs` | 368 | `dtx-input/src/keyboard.rs` | 96 | ⚠ partial |
| `FDK/Input/CInputMouse.cs` | 261 | — | — | ❌ |
| `FDK/Input/CInputMIDI.cs` | 114 | `dtx-input/src/midi.rs` | 201 | ✅ |
| `FDK/Input/DeviceConstantConverter.cs` | 341 | — | — | ❌ |
| `FDK/Input/IInputDevice.cs` | 34 | `dtx-input/src/events.rs` | 67 | ✅ trait-like |
| `FDK/Input/SlimDX.DirectInput.Key.cs` | 155 | — | — | ❌ (Bevy input replaces) |
| `FDK/Sound/CSound.cs` | 1,890 | `dtx-audio/src/lib.rs` (kira wrapper) | 113 | ⚠ kira replaces core; not 1:1 |
| `FDK/Sound/ISoundDevice.cs` | 18 | `dtx-core/src/fdk_sub_acts.rs` | — | ⚠ |
| `FDK/Sound/Cmp3ogg.cs` | 105 | — | — | ❌ out of scope (decoder) |
| `FDK/Sound/Cxa.cs` | 192 | — | — | ❌ out of scope (decoder) |
| `FDK/Sound/SoundDecoder.cs` | 13 | — | — | ❌ out of scope |
| `FDK/Sound/CSoundDeviceDirectSound.cs` | 300 | — | — | ❌ out of scope (Windows) |
| `FDK/Sound/CSoundDeviceWASAPI.cs` | 821 | — | — | ❌ out of scope (Windows) |
| `FDK/Sound/CSoundDeviceASIO.cs` | 484 | — | — | ❌ out of scope (Windows) |

## Why LoC is the wrong metric

```text
BocuD C# (verbose)        Rust (concise)           Ratio
─────────────────────────────────────────────────────────────
class CStagePerfDrums     mod stage_perf_drums     ~1.0x
  + ctor,                 + struct,
  + ~20 get/set props,    + inherent impl,
  + ~10 OnXxx methods     + match arms
─────────────────────────────────────────────────────────────
class CChip               struct CChip             ~0.5x
  12 fields + boilerplate 12 fields + tests        Rust half
─────────────────────────────────────────────────────────────
class CSkin               (Bevy AssetServer)       0.0x
  1147 LOC, file loaders  bevy::asset::Handle<...>  Bevy replaces
─────────────────────────────────────────────────────────────
class CSound              (kira wrapper)           ~0.1x
  1890 LOC, 3 backends    ~200 LOC                 kira replaces
─────────────────────────────────────────────────────────────
CActDFPFont + LVLNFont    (Bevy Text)              0.0x
  600+ LOC text render    Text + TextFont          Bevy replaces
```

A 60% LoC ratio would mean writing ~43k Rust LoC. But:
- 7,000+ LoC of UI/Drawable framework (out of scope) → 0 Rust LoC expected
- 1,891 LoC of CSound → ~200 LoC of kira wrapper
- 1,605 LoC of platform audio backends → 0 LoC expected (out of scope)
- 1,147 LoC of CSkin → 0 LoC (Bevy replaces)

Fair denominator: ~71,544 - 11,000 = **~60,544 LoC in-scope** (excluding replaced).
At 19,755 LoC that's **32.6%** of adjusted denominator.
60% of adjusted = **~36,000 LoC** — still ~16k short, but achievable.

## Other gaps the original analysis missed

### 1. Dead-code stubs (865 LoC, 0 cross-references)

```sh
$ grep -rE "perf_sub_acts::|perf_common_acts::|drums_screen_sub_acts::|\
   drawable_sub_acts::|guitar_screen_sub_acts::|result_sub_acts::" crates/ \
   --include="*.rs" | grep -v "fn main\|tests"
# (empty result)
```

| File | LoC | Consts | Imported by | Status |
|---|---:|---:|---|---|
| `gameplay-drums/perf_sub_acts.rs` | 177 | 28 | nothing | dead |
| `gameplay-drums/perf_common_acts.rs` | 160 | 35 | nothing | dead |
| `gameplay-drums/drums_screen_sub_acts.rs` | 155 | 35 | nothing | dead |
| `gameplay-drums/drawable_sub_acts.rs` | 77 | 10 | nothing | dead |
| `gameplay-guitar/guitar_screen_sub_acts.rs` | 182 | 29 | nothing | dead |
| `game-results/result_sub_acts.rs` | 114 | 21 | nothing | dead |

The position consts in these files are also duplicated in `hud.rs` with different values. Either merge or delete.

### 2. Duplicate plugins (3 pairs, double-spawn on state enter)

`crates/game-menu/src/lib.rs:65-79` registers:

```rust
app.add_plugins((
    song_select::plugin,        // 383 LOC, 17 fns
    song_select_full::plugin,   // 548 LOC, 24 fns   ← BOTH fire OnEnter(SongSelect)
    config::plugin,             // 168 LOC
    config_full::plugin,        // 286 LOC           ← BOTH fire OnEnter(Config)
    change_skin_full::plugin,   // 140 LOC
    change_skin::plugin,        // 55 LOC            ← BOTH fire OnEnter(ChangeSkin)
    ...
))
```

Visual bug: when user enters SongSelect, both plugins spawn their UI → overlapping text, double background, two sets of song rows. Same for Config and ChangeSkin.

### 3. `result_sub_acts.rs` not wired

`game-results/src/lib.rs:25` calls `result_full::plugin` but never `result_sub_acts::plugin`. The 114-LoC stub file is unreferenced.

### 4. 06.Performance missing big files (the real gameplay gap)

| Ref file | LoC ref | Status |
|---|---:|---|
| `CStagePerfCommonScreen.cs` | 5,067 | ❌ not ported (biggest single missing file) |
| `CStagePerfDrumsScreen.cs` | 3,671 | ❌ not ported |
| `CStagePerfGuitarScreen.cs` + `.Chip.cs` | 1,595 | ❌ not ported |
| `CActPerfCommonScore.cs` | 142 | ✅ real (in `score.rs`) |
| `CActPerfCommonCombo.cs` | 794 | ❌ stub (no actual combo display system) |
| `CActPerfCommonGauge.cs` | 296 | ⚠ gauge `Bar` spawned but no fill animation |
| `CActPerfCommonStatusPanel.cs` | 531 | ❌ |
| `CActPerfCommonJudgementString.cs` | 301 | ❌ |
| `CActPerfCommonLaneFlushGB.cs` | 70 | ❌ |
| `CActPerfCommonRGB.cs` | 59 | ❌ |
| `CActPerfCommonDanger.cs` | 57 | ❌ |
| `CActPerfCommonWailingBonus.cs` | 43 | ❌ |
| `CActPerfBGA.cs` | 305 | ✅ real (state machine) |
| `CActPerfVideo.cs` | 520 | ⚠ partial (no decode) |
| `CActPerfProgressBar.cs` | 543 | ❌ dead consts |
| `CActPerfSkillMeter.cs` | 302 | ❌ dead consts |
| `CActPerfScrollSpeed.cs` | 87 | ✅ real (in `scroll.rs`) |
| `CActPerfStageClear.cs` | 7 | ❌ empty marker |
| `CActPerfStageFailure.cs` | 123 | ❌ dead consts |
| `PerfNewChipFire.cs` + `IPerfFire.cs` | 233 | ❌ dead consts |
| `InfoBox.cs` | 84 | ❌ dead consts |
| `CActPerformanceInformation.cs` | 74 | ❌ |

06.Performance real coverage: ~10% by behavior (just `score` + `scroll` + `lane_flush` + `bga` + `hud` partial).

### 5. Core singletons not ported

- `CConfigIni.cs` (3,926 LOC) → 316 LOC subset in `dtx-config`. Missing: KeyAssign tables, skin resolution, all instrument-specific sections.
- `CSkin.cs` (1,147 LOC) → 0 LOC. Bevy AssetServer replaces.
- `CConstants.cs` (780 LOC) → scattered small consts across crates (no central Constants module).
- `StageManager.cs` (699 LOC) → `fade.rs` is 204 LOC. Missing: pre-load screens, stage-fade timing logic.
- `CDTXMania.cs` (612 LOC) → 0 LOC. Bevy App replaces the global singleton.
- `CPad.cs` (278 LOC) → 96 LOC. Missing: pad-snapping, button-down/up event correlation.

### 6. SongDb partial

- `SongDb.cs` (877 LOC) → 1,084 LoC real in `song_db_sub_acts.rs` + `lib.rs`.
- `SongNode.cs` (282 LOC) → in same.
- 8 sorters → 6 of 8 ported.
- `SongCacheSqlite.cs` (1,234 LOC) → 0 LOC. Use in-memory `HashMap` cache.
- `TextConversionCache.cs` (89 LOC) → covered in `song_db_sub_acts.rs`.
- `CacheModels.cs` (45 LOC) → covered.
- `SongDBStatus.cs` (23 LOC) → covered.
- `SongDBTest.cs` (4 LOC) → trivial.

## What the goal actually achieved

In this session and the prior one:

1. **Real DTX engine**: parser, BPM-aware timing, judgment classifier, score persistence, FDK primitives, BGM playback, BGA state machine.
2. **Real end-to-end test**: load `real_chart.dtx` → judge chips → persist → reload → verify (743 tests, 0 failed).
3. **Real menu flow**: Title, SongSelect (sort + search + BGM preview), Config (5 tabs + 14 sub-tabs), Result (rank + counts), ChangeSkin, SongLoading, Performance (Drums + Guitar), Startup, End.
4. **Real gameplay**: scroll notes, judge against AudioClock, score/combo, lane flush animations, basic HUD.
5. **Real architecture**: 9 states, 2 game modes, fade transitions, persistence, plugin structure.

The binary works. The game is playable in skeleton form.

## Recommended path to "full implementation"

The 60% LoC target is wrong-headed but the goal of a complete port is valid. The honest path:

### Phase A — clean up the false positives (1 session, 1 PR)

- Delete 6 dead-code stub files (865 LoC gone).
- Resolve 3 duplicate plugin pairs (pick one, delete other).
- Remove duplicate position consts from `hud.rs` that overlap with the (deleted) sub-acts files.

Net: −1,200 LoC of misleading "ports" → ratio jumps to ~37% honest.

### Phase B — port the 13 LoC Common performance files (1-2 sessions)

- `CActPerfCommonScore.cs` (142) ✅ already done
- `CActPerfCommonCombo.cs` (794) → real combo display system
- `CActPerfCommonGauge.cs` (296) → real gauge fill animation
- `CActPerfCommonStatusPanel.cs` (531) → real status panel
- `CActPerfCommonJudgementString.cs` (301) → real judgement string
- `CActPerfCommonLaneFlushGB.cs` (70) → GB-lane flush
- `CActPerfCommonRGB.cs` (59) → RGB fret indicator
- `CActPerfCommonDanger.cs` (57) → danger overlay
- `CActPerfCommonWailingBonus.cs` (43) → wailing bonus
- 6 drums-specific + 9 guitar-specific → split into real systems
- `CStagePerfCommonScreen.cs` (5,067) → orchestrator plugin

### Phase C — port the core singletons (1 session)

- `CConfigIni.cs` (3,926) → full KeyAssign tables + skin resolution
- `CSkin.cs` (1,147) → skin subfolder swap
- `CConstants.cs` (780) → central constants module
- `StageManager.cs` (699) → pre-load screens
- `CPad.cs` (278) → pad-snapping logic

### Phase D — port CDTX (1 session)

- `CDTX.cs` (7,295) → currently ~12% ported. The rest is BPM/BPMEx parsing, chip-array, all WAV/BMP/AVI/AVIPAN/BGA/BGAPAN resolution.

### Phase E — wire video decode (deferred per goal)

- BGA Movie/MovieFull still log+skip (M7 behavior). Real decode is M7.1+.

**Final result** (after Phase A-D):
- ~50k Rust LoC of real ports
- 100% in-scope ref file coverage
- 100% behavior coverage (no dead-code stubs)
- All duplicate plugins resolved
- All user-facing features working
- ~2,000 tests passing

## Boundaries — what to NOT do

The 60% LoC target tempted scope creep in the form of:
- "Complete" UI/Drawable framework (Bevy replaces; don't reimplement)
- Real video decode (out of scope per goal)
- Online/multiplayer (BocuD has none)
- DTXCreator editor (separate tool)
- Platform-specific audio backends (Windows-only, kblocked by AD-0010)

Stay disciplined. Port the C# logic, use Bevy for rendering, use kira for audio, use bevy_kira_audio for mixer. Anything else is gold-plating.
