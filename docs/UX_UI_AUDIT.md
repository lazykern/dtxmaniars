# UX/UI Audit: DTXManiaRS vs DTXManiaNX-BocuD vs osu-lazer

> Full reference inventory + gap analysis for the visual/UX layer.
> Date: 2026-06-28 (revised after full reference research).
> **v1 target (ADR-0010):** BocuD visual/UX parity. **M6+ target:** osu-lazer fluidity.

Reference clones (local, read-only per ADR-0004):
- `references/DTXmaniaNX-BocuD/` (730 MB)
- `references/osu-lazer/` (89 MB — game only)
- `references/osu-framework/` (engine — cloned 2026-06-28)

---

## 0. Executive Summary

| Dimension | DTXManiaNX-BocuD | osu-lazer + framework | DTXManiaRS (today) |
|---|---|---|---|
| **Framework** | Custom C# scene graph + ImGui inspector | `osu.Framework` Drawable tree | Bevy UI (flexbox) + ECS |
| **Screen model** | `CStage` + `EStage`/`EPhase` enums | `Screen` + `ScreenStack` push/pop | `AppState` + OnEnter/OnExit |
| **UI primitives** | `UIDrawable` tree, matrix transforms | `Drawable` → `CompositeDrawable` | `Node` + `Text` |
| **Animations** | `CCounter` + **BocuD `Animation/` clips** + `GitaDoraTransition` | `Transform` queue + easing | State machines only; **no visual tweens wired** |
| **Stage transitions** | **Two systems:** (1) GitaDora panel wipe ~0.66s, (2) StageManager snapshot fade **1500ms** (load→perf only) | Screen `FadeIn`/`FadeOut` 100–3000ms | **None** (`fade.rs` removed; ADR-0011 unimplemented) |
| **Time interpolation** | Frame-locked | `InterpolatingFramedClock` (50ms half-life) | Raw `AudioClock.current_ms` |
| **Skin system** | **Dual:** `CSkin` (System/ assets+sounds) + `UI/Skin/` (JSON hierarchy) | Skin JSON + textures | `dtx-ui/skin.rs` scaffold only |
| **Image rendering** | `UIImage`, GIF, MP4 video | `Sprite` + textures | **None** (no sprite pipeline) |
| **Text** | Glyph atlas + Skia backend | `SpriteText` / rolling counters | Bevy `Text`, default font |
| **UI LoC (approx.)** | Stage ~83k + UI ~6k + Core skin ~1.1k | Framework Graphics ~15k; game Screens/HUD ~8k | **~4.6k** UI-adjacent Rust |
| **Polish** | GITADORA-era + BocuD renderer refresh | Industry-leading | Functional text UI; mechanics > visuals |

**TL;DR:** BocuD is the v1 UX baseline (ADR-0010). osu-lazer is the M6+ fluidity
destination. RS has screen flow + keyboard nav + score/combo logic, but lacks
sprites, transitions, animation wiring, and most BocuD sub-act visuals.

---

## 1. DTXManiaNX-BocuD — full UX inventory

### 1.1 Global stage model

Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs` (146 LoC)

**`CStage.EStage`** (10 values): `DoNothing_0`, `Startup_1`, `Title_2`, `Config_3`,
`SongSelection_4`, `SongLoading_5`, `Performance_6`, `Result_7`, `End_8`,
`ChangeSkin_9`.

**`CStage.EPhase`** (22+): shared `Common_DefaultState`, `Common_FadeIn`,
`Common_FadeOut`, `Common_EndStatus`; plus per-stage phases (`NOWLOADING_*`,
`PERFORMANCE_STAGE_FAILED`, `PERFORMANCE_STAGE_CLEAR`, …).

Lifecycle per stage:
```csharp
class CStage : CActivity {
    internal UIGroup ui;
    public abstract void InitializeBaseUI();
    public abstract void InitializeDefaultUI();
    // LoadUI = Base + Default + SkinManager.ApplySkin
    public virtual void OnActivate();
    public virtual void OnDeactivate();
    public override int OnUpdateAndDraw();
}
```

**StageManager** (`DTXMania/Core/StageManager.cs`):
- Wires **new** song select (`CStageSongSelectionNew`), not legacy `04.SongSelection/`.
- **Snapshot fade** (load → performance only): `FadeDurationMs = 1500f` (L29);
  captures `GameRenderTarget.ReadPixels()` → draws snapshot with linear alpha decay
  (L637–699). Hides perf first-frame spike.

---

### 1.2 Per-stage catalog

#### 01.Startup — `CStageStartup.cs` (103 LoC)
- No CAct sub-acts. Auto-advances after init.
- Phases: `EStage.Startup_1`.

#### 02.Title — `CStageTitle.cs` (379 LoC)
- No CAct sub-acts. Version text + background image/video.
- Keys: `Escape` exit; `Up`/`Down` menu; `ActionDecide()` confirm.
- **GitaDoraTransition:** `Close()` on menu decide; gates `Common_FadeOut` on `isAnimating`.

#### 03.Config — `CStageConfig.cs` (532 LoC)

| CAct | File | LoC | Purpose |
|---|---|---:|---|
| `CActConfigKeyAssign` | `CActConfigKeyAssign.cs` | 564 | Pad/key assignment flow |
| `CActConfigList` | `CActConfigList.cs` + 11 partials | 818 + 3083 | Option list shell + tabs |

**Config tabs (partials):** System, Graphics, Audio, Audio.Driver, Gameplay,
Drums, Drums.Velocity, Guitar, Bass, Skin, Menu.

Keys: `Up`/`Down`; `ActionDecide`/`ActionCancel`. Pad: HH/HT up, SD/LT down.
**GitaDora:** `Open(2)` on activate; `Close()`+`Open()` on exit.

#### 04.SongSelection (legacy) — NOT in StageManager

| CAct | File | LoC | Purpose |
|---|---|---:|---|
| `CActScrollBar` | `CActScrollBar.cs` | 66 | Scrollbar |
| `CActSelectArtistComment` | `CActSelectArtistComment.cs` | 254 | Artist comment |
| `CActSelectInformation` | `CActSelectInformation.cs` | 135 | Song info |
| `CActSelectPerfHistoryPanel` | `CActSelectPerfHistoryPanel.cs` | 140 | Best scores |
| `CActSelectPopupMenu` | `CActSelectPopupMenu.cs` | 473 | Sort popup |
| `CActSelectPreimagePanel` | `CActSelectPreimagePanel.cs` | 540 | Album art preview |
| `CActSelectQuickConfig` | `CActSelectQuickConfig.cs` | 703 | Inline AUTO/config |
| `CActSelectSongList` | `CActSelectSongList.cs` | 1641 | Virtualized song list |
| `CActSelectStatusPanel` | `CActSelectStatusPanel.cs` | 1176 | Status/skill panel |
| `CActSortSongs` | `CActSortSongs.cs` | 104 | Sort logic |

`CStageSongSelection.cs` — 1107 LoC. Out of scope per ADR-0010 (use SongSelectionNew).

#### 04.SongSelectionNew (active) — `CStageSongSelectionNew.cs` (597 LoC)

| Component | File | LoC | Purpose |
|---|---|---:|---|
| `CActSelectPresound` | `CActSelectPresound.cs` | 185 | BGM preview on cursor |
| `SongSelectionContainer` | `SongSelectionContainer.cs` | ~600 | Main layout + input routing |
| `SongSelectionElement` | `SongSelectionElement.cs` | ~350 | Single song row |
| `DensityGraph` | `DensityGraph.cs` | ~350 | 8-bar note density |
| `SortMenuContainer` | `SortMenuContainer.cs` | ~230 | Sort mode ring UI |
| `SortMenuElement` | `SortMenuElement.cs` | ~40 | Sort menu item |
| `SongSearchMenu` | `SongSearchMenu.cs` | ~100 | Text search overlay |
| `StatusPanel` | `StatusPanel.cs` | ~130 | Current sort + count |
| `StatusPane` | `StatusPane.cs` | ~240 | Song metadata panes |
| `CommandHistory` | `CommandHistory.cs` | ~80 | Undo/redo for navigation |

Keys (via container): `Up`/`Down` scroll; `R` random; `ActionDecide` select;
`ActionCancel` back; `Tab` sort cycle; `F1` config; `F2` instrument; `F5` rescan;
type-to-search filter.

**GitaDora:** `Open(2)` after thumbnail cache (`ELoadPhase.ReadyToOpen`).

#### 05.SongLoading — `CStageSongLoading.cs` (1111 LoC)
- Phases: `NOWLOADING_DTX_FILE_READING` → WAV → BMP → BGM ready.
- `Escape` abort → song select. **GitaDora:** `Close(2, …)` on abort.

#### 06.Performance

| Stage class | File | LoC |
|---|---|---:|
| `CStagePerfCommonScreen` | `CStagePerfCommonScreen.cs` | 5068 |
| `CStagePerfDrumsScreen` | `DrumsScreen/CStagePerfDrumsScreen.cs` | 3672 |
| `CStagePerfGuitarScreen` | `GuitarScreen/CStagePerfGuitarScreen.cs` | 788 |

**Common CAct* (shared drums/guitar):**

| CAct | LoC | Purpose |
|---|---:|---|
| `CActPerfBGA` | 306 | BGA chip rendering |
| `CActPerfVideo` | 521 | Background MP4 |
| `CActPerfCommonCombo` | 795 | Combo counter base |
| `CActPerfCommonGauge` | 297 | Gauge base |
| `CActPerfCommonJudgementString` | 301 | Judgment popup base |
| `CActPerfCommonLaneFlushGB` | 71 | Guitar lane flash base |
| `CActPerfCommonScore` | 143 | Score counter |
| `CActPerfCommonStatusPanel` | 532 | BPM/skill HUD base |
| `CActPerfCommonDanger` | 58 | DANGER overlay base |
| `CActPerfCommonWailingBonus` | 44 | Wailing bonus stub |
| `CActPerfProgressBar` | 543 | Chart position bar |
| `CActPerfScrollSpeed` | 88 | Scroll speed indicator |
| `CActPerfSkillMeter` | 303 | Skill meter graph |
| `CActPerfStageFailure` | 124 | Fail overlay |
| `CActPerfStageClear` | 8 | Stage clear stub |
| `CActPerformanceInformation` | 75 | Debug overlay |
| `CActPerfAVI.old` | 931 | Legacy AVI (superseded) |

**Drums CAct*:**

| CAct | LoC | Purpose |
|---|---:|---|
| `CActPerfDrumsPad` | 499 | Lane pad rendering |
| `CActPerfDrumsLaneFlushD` | 457 | 10-lane hit flash |
| `CActPerfPerfChipFireD` | 1081 | Chip fire / note strike |
| `CActPerfDrumsComboDGB` | 113 | Combo + PG/GR |
| `CActPerfDrumsGauge` | 89 | Gauge |
| `CActPerfDrumsJudgementString` | 103 | Judgment labels |
| `CActPerfDrumsScore` | 77 | Score |
| `CActPerfDrumsStatusPanel` | 212 | Status HUD |
| `CActPerfDrumsDanger` | 78 | DANGER overlay |
| `CActPerfDrumsFillingEffect` | 42 | Fill-in effect |

**Guitar CAct*:**

| CAct | LoC | Purpose |
|---|---:|---|
| `CActPerfGuitarLaneFlushGB` | 113 | Lane flash |
| `CActPerfGuitarRGB` | 203 | RGB overlay |
| `CActPerfGuitarCombo/Score/Gauge/StatusPanel/JudgementString/Danger/Bonus/WailingBonus` | 24–238 each | Guitar HUD slices |

Performance keys: `Escape` interrupt; arrow keys timing adjust; `PageUp`/`PageDown`
scroll; numpad; `F5`/`F6`; Shift/Alt/Ctrl modifiers.
**GitaDora:** `Close()` on exit; gates on `!isAnimating`.

#### 07.Result — `CStageResult.cs` (812 LoC)

| CAct | LoC | Purpose |
|---|---:|---|
| `CActResultParameterPanel` | 971 | Animated stats panel |

Keys: `Escape`/`ActionDecide` exit (after anim); `F12` save PNG.
**GitaDora:** `Close()` on exit.

#### 08.End — `CStageEnd.cs` (87 LoC)
- 1000ms counter + game-end sound. No sub-acts.

#### 09.ChangeSkin — `CStageChangeSkin.cs` (96 LoC)
- Reloads `CSkin` sounds; returns to song select.

#### Stage root shared

| File | LoC | Purpose |
|---|---:|---|
| `CActDFPFont.cs` | 603 | DFP bitmap font |
| `CActLVLNFont.cs` | 121 | Level font |
| `CActOptionPanel.cs` | 86 | Option panel helper |
| `UIPlayerNameplate.cs` | ~120 | Player nameplate |

---

### 1.3 UIDrawable scene graph

Reference: `DTXMania/UI/Drawable/UIDrawable.cs`

```csharp
abstract class UIDrawable {
    [Themable] Vector3 position, scale, rotation;
    [Themable] Vector2 anchor, size;
    [Themable] bool isVisible;
    Matrix4x4 GetFullTransformMatrix();  // walks parents
    public abstract void Draw(Matrix4x4 parentMatrix);  // render-only, no Update
}
```

Concrete types: `UIImage`, `UIText`, `UIBasicButton`, `UIGroup`, `UITexture`,
`UISelectList`, `HorizontallyScrollingText`, `GitaDoraTransition`.

OpenGL backend: `UI/OpenGL/OpenGlRenderer.cs`, `OpenGlTexture.cs`, Skia text.

---

### 1.4 BocuD Animation system (NEW — not in upstream DTXMania)

Reference: `DTXMania/UI/Drawable/Animation/` (~2.1k LoC)

| File | Purpose |
|---|---|
| `AnimationClip.cs` | Serializable clip (tracks, duration, loop); hosts `Animator` |
| `AnimationTrack.cs` | Property path + keyframes; evaluates onto drawable tree |
| `Keyframe.cs` | Value + `Easing` (Step, Linear, Quad/Cubic in/out) |
| `Interpolator.cs` | Type registry for lerp (float, Vector2/3/4, Color4, Quaternion) |
| `PropertyAccessor.cs` | Compiled getters/setters for animated properties |
| `AnimationClipIO.cs` | JSON save/load |
| `AnimationClipEditor*.cs` | ImGui timeline editor (dev tooling) |

`Animator` on `UIGroup`: ticks clips each frame, writes properties before draw.
**Port implication:** v1 can port BocuD clip semantics via `bevy_tweening` (djeedai; ADR-0007)
rather than reimplementing ImGui editor.

---

### 1.5 GitaDoraTransition

Reference: `DTXMania/UI/Drawable/GitaDoraTransition.cs` (198 LoC)

| Aspect | Detail |
|---|---|
| Visual | Full-screen GITADORA wipe: two black bars rotate/separate; logo slides/fades |
| Size | 1280×720; bars 3000×1000 |
| Duration | Progress-driven, **not fixed ms**: `animationSpeed = 3.4f`; Close ~0.66s; Open similar |
| Bar easing | Cubic polynomial on progress `t` |
| Logo easing | Quintic ease-out on remapped progress |
| API | `Open(delayFrames, onComplete)`, `Close(…)`, `isAnimating`, `isClosed` |

**Used on:** Title, Config, SongSelect (enter), SongLoading (abort), Performance
(exit), Result (exit). Registered in `CDTXMania.Init.cs` as persistent child.

**Separate from** StageManager 1500ms snapshot fade (load→perf only).

---

### 1.6 Dual skin model

#### Legacy: `DTXMania/Core/CSkin.cs` (1148 LoC)

- Resolves paths under `System/{skin}/Graphics/` and `Sounds/`.
- `CSkin.Path(relative)`, `ReloadSkin()`, `SkinConfig.ini` (fonts, judge frames).
- Valid skin = has `Graphics/1_background.jpg`.
- System sounds via `ESystemSound` enum + `CSystemSound.tPlay()`.

**Default background paths:**
- `Graphics/1_background.jpg` — Startup
- `Graphics/2_background.png` + optional `2_background.mp4` — Title
- `Graphics/3_background.jpg` — SongSelect
- `Graphics/4_background.jpg` — SongLoading
- `Graphics/5_background.jpg` — Performance (per instrument)
- `Graphics/6_background.jpg` — Result
- `Graphics/7_background.jpg` — End

Plus hundreds of gameplay sprites: lane flush, judgment labels, gauge frames,
combo/score digits, rank icons.

#### Modern: `DTXMania/UI/Skin/` (~692 LoC)

| File | Purpose |
|---|---|
| `SkinManager.cs` | Scan `{exe}/Skins/`; create/change skins |
| `SkinDescriptor.cs` | `skin.json` metadata; per-stage JSON paths |
| `SkinHierarchySerializer.cs` | Serialize `UIGroup` tree → JSON |
| `SkinHierarchyMerger.cs` | Apply JSON onto live stage hierarchy |
| `SkinResources.cs` | Copy Image/Font assets into skin folder |

**Note:** `FDK/Skin/` does **not** exist in BocuD. Prior audit cite was wrong.

---

### 1.7 BocuD UX strengths (preserve in v1)

1. 9-screen + ChangeSkin flow with phase machine
2. Sub-act composition — clean separation
3. Keyboard-first navigation (power-user friendly)
4. Persistent cross-screen state (SelectedSong, SortMode, …)
5. GitaDora panel transitions between major screens
6. Dual transition: GitaDora (most screens) + snapshot fade (load→perf)
7. BGM preview on song select (`CActSelectPresound`)
8. Result persistence (`CScoreIni`)

### 1.8 BocuD weaknesses (M6+ improvement targets)

1. Pixel-clamped positions — stutter at slow scroll
2. Single-threaded render
3. No UI virtualization in legacy song select (New uses container)
4. 228k+ LoC UI — over-engineered vs output quality
5. ImGui inspector/dev tooling mixed with shipping UI

---

## 2. osu-framework — engine UX primitives

Reference: `references/osu-framework/` (cloned locally)

### 2.1 Scene graph hierarchy

```
Transformable → Drawable (2857 LoC) → CompositeDrawable (2013 LoC) → Container (547 LoC)
```

| Class | File | Purpose |
|---|---|---|
| `Drawable` | `Graphics/Drawable.cs` | Base node: layout, input, render, `InvalidationID` (L1767) |
| `CompositeDrawable` | `Containers/CompositeDrawable.cs` | Child propagation, async load, masking |
| `Container<T>` | `Containers/Container.cs` | Typed child collection |
| `Transformable` | `Transforms/Transformable.cs` | Transform queue, clock, `UpdateTransforms` |
| `TransformableExtensions` | `Graphics/TransformableExtensions.cs` | `FadeIn`/`FadeOut`/`MoveTo`/`ScaleTo` (L287+) |
| `Transform` | `Transforms/Transform.cs` | Single property animation |
| `TransformSequence` | `Transforms/TransformSequence.cs` | Chained transforms + `Then()` |
| `Bindable<T>` | `Bindables/Bindable.cs` | Reactive observable values |

Default `FadeIn(duration=0)` = instant unless caller passes ms.

### 2.2 Screen stack

| Class | File | Purpose |
|---|---|---|
| `Screen` | `Screens/Screen.cs` | Lifecycle hooks: `OnEntering`, `OnExiting`, `OnSuspending`, `OnResuming` |
| `ScreenStack` | `Screens/ScreenStack.cs` | Push/pop, async load, suspend/resume orchestration |
| `IScreen` | `Screens/IScreen.cs` | Contract + extension helpers |

Push flow: parent `OnSuspending` → load child → child `OnEntering`.
Exit: child `OnExiting` (return true = block) → parent `OnResuming`.

### 2.3 Timing / fluidity

| Class | File | Purpose |
|---|---|---|
| `InterpolatingFramedClock` | `Timing/InterpolatingFramedClock.cs` | Smooths clock drift; 33ms allowable error, 50ms half-life |
| `FramedClock` | `Timing/FramedClock.cs` | Per-frame snapshot from source |
| `IAdjustableClock` | `Timing/IAdjustableClock.cs` | Start/stop/seek/rate |

**Why it matters:** audio clock is sample-accurate; render frames jitter. Interpolating
clock damps visual time so chips don't jump on frame spikes.

### 2.4 Easing

`DefaultEasingFunction.cs`, `CubicBezierEasingFunction.cs`, `Spring.cs`.
Enum: `OutQuint`, `InOut`, `OutElasticQuarter`, etc.

---

## 3. osu-lazer — game-layer UX inventory

Reference: `references/osu-lazer/` (game repo; uses framework as NuGet dep)

### 3.1 Screen flow

```
Startup/Loader → MainMenu → SoloSongSelect → PlayerLoader (1800ms hold) → Player → SoloResultsScreen
```

| Folder | Files | Role |
|---|---:|---|
| `Screens/Menu/` | 26 | Main menu, intro, logo, button wheel |
| `Screens/Select/` | 61 | Carousel, wedges, filters, footer buttons |
| `Screens/Play/` | 50 | Player, loader, HUD host, overlays |
| `Screens/Ranking/` | 16 | Results (not named "Results/") |

### 3.2 Transition timings (file:line verified)

| Context | ms | Source |
|---|---:|---|
| Song select fade | 300 | `SongSelect.cs:79`, `:675`, `:695` |
| Main menu in/out | 300 / 400 | `MainMenu.cs:52–54` |
| HUD show/hide | 300 | `HUDOverlay.cs:37` (OutQuint) |
| PlayerLoader hold | **1800** + disclaimers×500 | `PlayerLoader.cs:54` |
| Player enter scale | 750 + fade 250 | `Player.cs:1122–1125` |
| Player exit | 250 | `Player.cs:1295` |
| Results enter | 250 | `ResultsScreen.cs:402–405` |
| Main menu exit (confirmed) | 3000 | `MainMenu.cs:444` |

### 3.3 Play HUD (`Screens/Play/HUD/` — 64 files, ~7660 LoC)

Skinnable via `HUDOverlay` + `SkinnableContainer`. Two visual skins: **Default** and **Argon**.

| Category | Key components |
|---|---|
| Counters | `ComboCounter`, `GameplayScoreCounter`, `GameplayAccuracyCounter`, `UnstableRateCounter`, `PerformancePointsCounter` |
| Health | `HealthDisplay`, `ArgonHealthDisplay` (+ bar/background parts) |
| Progress | `SongProgress`, `SongProgressBar`, `SongProgressGraph` |
| Hit error | `BarHitErrorMeter`, `ColourHitErrorMeter` |
| Keys | `KeyCounterDisplay`, `KeyCounter`, triggers (keyboard/mouse/action) |
| Judgement | `JudgementCounter`, `JudgementCounterDisplay` |
| Multi | `MatchScoreDisplay`, `DrawableGameplayLeaderboard`, `SpectatorList` |
| Misc | `ModDisplay`, `FailingLayer`, `HoldForMenuButton`, `BPMCounter`, `ReplayOverlay` |

`RollingCounter<T>` lives in `osu.Game/Graphics/UserInterface/RollingCounter.cs` (181 LoC)
— **not** in osu-framework. Proportional roll duration + `OutQuad` easing.

### 3.4 Song select UX (osu patterns — M6+ reference only)

- Carousel + left/right wedges (metadata, leaderboard)
- FilterControl + query parser
- Footer: mods, random, options
- Live BGM + background reveal animations
- Logo scale/fade choreography (240ms in, 120ms out)

### 3.5 Overlays (global, above screens)

Settings, Dialog, Notifications, ModSelect, MusicController, Chat, Dashboard,
BeatmapListing, Profile, Rankings, SkinEditor, Volume, Login, FirstRunSetup, …

87 reusable widgets in `Graphics/UserInterface/`: `OsuButton`, `SearchTextBox`,
`ProgressBar`, `BarGraph`, `LoadingLayer`, tab controls, dropdowns, etc.

---

## 4. DTXManiaRS — current state (2026-06-28)

### 4.1 UI file inventory (~4645 LoC UI-adjacent)

| File | LoC | Status |
|---|---:|---|
| `game-menu/src/song_select.rs` | 844 | Partial — list, density bars (stub data), sort ring, search overlay, status panel |
| `game-menu/src/config.rs` | 389 | Stub — ConfigTab enum, layout, hardcoded items |
| `game-menu/src/config_key_assign.rs` | 350 | Stub — UI exists, capture logic partial |
| `game-menu/src/song_loading.rs` | 196 | Functional — text + progress |
| `game-menu/src/title.rs` | 89 | Functional — text prompt |
| `game-menu/src/startup.rs` | 46 | Stub — auto-advance, no logo |
| `gameplay-drums/src/hud.rs` | 314 | Minimal — 9-lane strip, hit line, text score/combo/gauge |
| `gameplay-guitar/src/hud.rs` | 268 | Minimal — RGBYP lanes + text HUD |
| `game-results/src/lib.rs` | 224 | Text-only result wall; `result_full` deleted |
| `dtx-ui/src/perf_common.rs` | 321 | Layout constants + lane flush **state** |
| `dtx-ui/src/skin.rs` | 289 | Skin registry scaffold |
| `dtx-ui/src/core_sub_acts.rs` | 454 | VideoState, Color4, log types |
| `dtx-ui/src/lib.rs` | 252 | Constants; `SCREEN_FADE_MS = 1500` defined but **fade not wired** |
| `dtx-bga/src/lib.rs` | 335 | BGA state machine, placeholder overlays |
| `game-shell/src/*` | ~136 | States, performance ESC→Result; **no fade system** |

Also: `gameplay-drums/drums_perf.rs`, `orchestrator.rs`, `scroll.rs` (note transforms,
no sprites); `gameplay-guitar/guitar_perf.rs`, `orchestrator.rs`.

### 4.2 What works (mechanics + minimal UI)

- ✅ AppState flow: Startup → Title → SongSelect → SongLoading → Performance → Result
- ✅ Keyboard nav (arrows, Enter, Esc, Tab, F-keys)
- ✅ SongDb scan + BGM preview on select
- ✅ Score/combo/gauge/judgment accumulation
- ✅ Result rank computation + ScoreStore JSON persistence
- ✅ Lane flush / danger / RGB **state machines** (no sprite render)
- ✅ Note scroll **transform logic** (`scroll.rs`) — invisible without sprites
- ✅ Config tabs structure; song select density/sort/search **UI spawned** (stub data)

### 4.3 Truly missing (visual layer)

| Gap | BocuD reference | RS status |
|---|---|---|
| GitaDoraTransition | `GitaDoraTransition.cs` | ❌ not started |
| StageManager snapshot fade | `StageManager.cs:29` (1500ms) | ❌ removed (`fade.rs` gone); ADR-0011 unimplemented |
| Sprite/image pipeline | `UIImage`, `#BMPxx` | ❌ none |
| Skin render hookup | `CSkin.cs` + `UI/Skin/` | ❌ registry only |
| Lane flush visuals | `CActPerfDrumsLaneFlushD.cs` | ❌ state only |
| Judgment label flash | `CActPerfDrumsJudgementString.cs` | ❌ `LastJudgment` unused in hud |
| Chip fire / note sprites | `CActPerfPerfChipFireD.cs` | ❌ none |
| Rank icons | `ResultRankIcon.cs` | ❌ text rank only |
| Background images per screen | `CSkin` Graphics/1–7 | ❌ solid colors |
| BGA/video render | `CActPerfBGA`, `CActPerfVideo` | ❌ placeholder state |
| ChangeSkin / End screens | `CStageChangeSkin`, `CStageEnd` | ❌ enum only, no plugins |
| Animation wiring | BocuD `Animation/` or CCounter | ❌ `bevy_tweening` pinned in workspace, not yet used in code |
| Interpolated visual clock | — (M6+: osu) | ❌ raw AudioClock |

### 4.4 Stub vs missing distinction

These exist as **UI nodes or state** but lack BocuD fidelity:

- Density graph — 8 bars spawned; `update_density_graph` no-op (no real chip histogram)
- Song search — overlay + filter logic; not BocuD modal behavior
- Sort menu — ring UI; sort modes partial
- Config — tab shell; 250+ CConfigIni fields unported
- Drums HUD — 9 lanes in RS vs BocuD 10-lane flush D

---

## 5. Gap analysis — port-first (v1) vs M6+ (osu)

### 5.1 Priority 0 — v1 BocuD parity (ADR-0010)

| Gap | Effort | BocuD reference | RS target |
|---|---|---|---|
| GitaDoraTransition port | 3d | `GitaDoraTransition.cs` | `game-shell/src/gita_dora.rs` |
| Restore 1500ms fade | 1d | `StageManager.cs:637–699` | `game-shell/src/fade.rs` (ADR-0011 black overlay OK v1) |
| Sprite pipeline | 2d | `UIImage`, `#BMPxx` | `dtx-assets` + Bevy `Image`/`Sprite` |
| CSkin path resolver | 2d | `CSkin.cs` | extend `dtx-ui/skin.rs` |
| Drums lane flush render | 1d | `CActPerfDrumsLaneFlushD.cs:457` | `gameplay-drums` |
| Judgment label flash | 1d | `CActPerfDrumsJudgementString.cs` | `gameplay-drums` |
| Note chip sprites | 3d | `CActPerfDrumsPad.cs`, `CActPerfPerfChipFireD.cs` | `gameplay-drums` |
| Per-screen backgrounds | 1d | `CSkin` Graphics/1–7 | per-screen OnEnter |
| Result rank icons | 1d | `ResultRankIcon.cs` | `game-results` |
| Song select visual parity | 5d | `04.SongSelectionNew/*` | `game-menu` (supersedes ADR-0012) |

### 5.2 Priority 1 — v1 polish (still BocuD-sourced)

| Gap | BocuD reference |
|---|---|
| Density graph real data | `DensityGraph.cs` |
| BGA layer render | `CActPerfBGA.cs` |
| Progress bar | `CActPerfProgressBar.cs` |
| Config key assign capture | `CActConfigKeyAssign.cs` |
| ChangeSkin thumbnail grid | `CStageChangeSkin.cs` |
| End screen | `CStageEnd.cs` |
| Skin JSON hierarchy | `SkinHierarchyMerger.cs` |
| BocuD Animation clips → bevy_tweening | `Animation/AnimationClip.cs` (ADR-0007) |

### 5.3 Priority 2 — M6+ osu fluidity (blocked until port baseline)

| Gap | osu reference |
|---|---|
| InterpolatedAudioClock | `InterpolatingFramedClock.cs` |
| Rolling score/combo counters | `RollingCounter.cs`, `ComboCounter.cs` |
| Transform-based HUD tweens | `TransformableExtensions.cs` |
| Hit error meters | `BarHitErrorMeter.cs` |
| Hero song on title | `MainMenu.cs` |
| 300ms OutQuint screen fades | `SongSelect.cs`, `MainMenu.cs` |
| Parallax backgrounds | `BackgroundScreenStack` |
| Particles/trails/combo burst | various HUD |

**Do not mix P0 and P2** — ADR-0010 requires BocuD values for v1 fades (1500ms linear
snapshot), lane order, judgment windows, HUD positions.

---

## 6. Architecture recommendations

### 6.1 Dependency graph (unchanged intent)

```
dtx-core, dtx-timing, dtx-config, dtx-scoring, dtx-assets, dtx-bga, dtx-input
    └── dtx-ui (widgets, skin, tween wrappers)
        └── game-shell (transitions: GitaDora + fade)
            └── game-menu, game-results, gameplay-drums, gameplay-guitar
```

### 6.2 Transition model (v1 = two BocuD systems)

1. **GitaDoraTransition** — panel wipe between Title/Config/Select/Loading/Perf/Result
2. **Snapshot fade** — 1500ms linear alpha on load→Performance (black overlay OK per ADR-0011)

Do **not** use osu 200–300ms Push/Pop for v1.

### 6.3 Animation strategy

- v1: port BocuD `CCounter` timings + key `AnimationClip` durations via `bevy_tweening` (djeedai; ADR-0007)
- M6+: add `InterpolatedAudioClock` for scroll smoothness

### 6.4 Skin strategy

- v1: `CSkin.Path()` resolution + default `System/Default/Graphics/` assets
- v1.1: `UI/Skin/` JSON hierarchy merger for layout overrides

---

## 7. Implementation phases

### Phase J — BocuD foundations (1 week)
- Restore fade overlay (1500ms, ADR-0011)
- Sprite load from `#BMPxx`
- GitaDoraTransition MVP (bar wipe + logo)
- ~~Wire `bevy_tween` (uncomment Cargo.toml dep)~~ — **done 2026-06-28** as git pin `bevy_tweening` rev `5e3d0c9` (BEVY_UX_UI.md §6)

### Phase K — Performance visuals (1.5 weeks)
- Lane flush sprites
- Judgment label flash
- Note chip rendering (scroll + fire)
- Gauge/status panel sprites
- Background image per perf stage

### Phase L — Menu/result parity (1.5 weeks)
- Song select: real density data, album art, sort/search fidelity
- Result rank icons + animated parameter panel
- Config visual tabs + key assign capture
- ChangeSkin + End minimal screens

### Phase M — Skin + BGA (1 week)
- CSkin asset loader
- BGA image sequence
- Skin JSON hierarchy (optional v1.1)

### Phase N — M6+ osu fluidity (2+ weeks, post-baseline)
- InterpolatedAudioClock
- Rolling counters
- Hit error meters
- osu-style micro-fades (requires ADR superseding 0010)

---

## 8. Success criteria

### v1 (BocuD parity)

| Criterion | Target |
|---|---|
| Screen flow | All 9 stages + ChangeSkin navigable |
| Transitions | GitaDora on major screens; 1500ms load→perf fade |
| Performance HUD | All drums sub-acts render (pad, flush, judgment, gauge, combo, score) |
| Song select | Density graph, sort, search, status panel match BocuD layout |
| Skins | Default CSkin graphics load |
| Compare | Side-by-side screenshot vs BocuD binary |

### M6+ (osu fluidity)

| Criterion | Target |
|---|---|
| Scroll smoothness | InterpolatedAudioClock, zero visible stutter |
| HUD animation | Rolling counters, tweened gauge |
| Frame rate | Stable 60fps+, 144fps with vsync |

---

## 9. References (verified paths)

### DTXManiaNX-BocuD
- `DTXMania/Stage/CStage.cs` — stage base
- `DTXMania/Core/StageManager.cs` — snapshot fade
- `DTXMania/Core/CSkin.cs` — legacy skin (NOT `FDK/Skin/CSkin.cs`)
- `DTXMania/UI/Drawable/UIDrawable.cs` — scene graph
- `DTXMania/UI/Drawable/GitaDoraTransition.cs` — panel wipe
- `DTXMania/UI/Drawable/Animation/` — BocuD animation clips
- `DTXMania/UI/Skin/` — JSON skin hierarchy
- `DTXMania/Stage/04.SongSelectionNew/` — active song select (11 files)
- `DTXMania/Stage/06.Performance/` — perf common + drums + guitar
- `DTXMania/Stage/07.Result/` — result screen

### osu-framework (local clone)
- `osu.Framework/Graphics/Drawable.cs`
- `osu.Framework/Graphics/TransformableExtensions.cs`
- `osu.Framework/Graphics/Transforms/Transform*.cs`
- `osu.Framework/Screens/Screen.cs`, `ScreenStack.cs`
- `osu.Framework/Timing/InterpolatingFramedClock.cs`
- `osu.Framework/Bindables/Bindable.cs`

### osu-lazer
- `osu.Game/Screens/Menu/MainMenu.cs`
- `osu.Game/Screens/Select/SongSelect.cs`
- `osu.Game/Screens/Play/Player.cs`, `PlayerLoader.cs`, `HUDOverlay.cs`
- `osu.Game/Screens/Play/HUD/` — 64 HUD components
- `osu.Game/Screens/Ranking/ResultsScreen.cs`
- `osu.Game/Graphics/UserInterface/RollingCounter.cs`

### Project ADRs
- ADR-0010 — port-first BocuD baseline
- ADR-0011 — fade black-overlay approximation (1500ms)
- ADR-0012 — song-select simplification (temporary; supersede in Phase L)
- ADR-0013 — result text-only (temporary; supersede in Phase L)
- ADR-0007 — bevy_tweening, no JSON loader v1

---

## 10. Open questions

1. **GitaDora vs snapshot vs black overlay** — ship all three for v1?
2. ~~BocuD Animation JSON vs bevy_tween-only~~ — **resolved 2026-06-28**: standardized on `bevy_tweening` (djeedai, git rev `5e3d0c9`); JSON loader stays deferred to M5+ skin system (ADR-0007).
3. **9 vs 10 drum lanes** — RS hud uses 9; BocuD flush D uses 10 (LC+LP)?
4. **ChangeSkin + End** — v1 required or defer?
5. **Legacy `04.SongSelection/`** — permanently out of scope?
6. **ROADMAP M9–M13 "Done"** — revert claims or re-implement stripped UI?
7. **When does "ADR-0010 relaxed" end** — code comments vs ADR text diverge.
8. **ImGui inspector** — port dev tooling or Bevy egui only (ADR-0006)?
9. **Video backgrounds** — `bevy_movie` vs ffmpeg subprocess vs defer?
10. **Skin format v1** — CSkin paths only, or also JSON hierarchy?

---

## 11. Doc / code drift register

| Doc/code | Claims | Reality (2026-06-28) |
|---|---|---|
| `docs/ROADMAP.md` M9–M13 | "Done", full HUD/song select/result/config | UI stripped; `result_full` deleted |
| `game-shell/AGENTS.md` | 1500ms fade in `fade.rs` | `fade.rs` does not exist |
| `game-shell/performance.rs:7` | "fade UI removed" | Conflicts ADR-0011 |
| `dtx-ui/lib.rs` | `SCREEN_FADE_MS = 1500` | Constant defined, not wired |
| `Cargo.toml` | bevy_tweening | **Pinned 2026-06-28** to git rev `5e3d0c9` (workspace.dependencies); `dtx-ui` declares dep |
| Prior audit §1.9 | "no animation library" | Wrong — BocuD has `Animation/` |
| Prior audit §8 | `FDK/Skin/CSkin.cs` | Wrong — use `Core/CSkin.cs` |
| Prior audit §2 | osu.Framework paths | Unverifiable until 2026-06-28 clone |

---

## 12. Research completion checklist

| Area | Status |
|---|---|
| BocuD all 10 stages + sub-acts | ✅ cataloged |
| BocuD UI/Drawable/Animation/Skin | ✅ cataloged |
| BocuD GitaDoraTransition + StageManager fade | ✅ cataloged |
| BocuD legacy song select | ✅ noted out-of-scope |
| osu-framework engine primitives | ✅ cloned + verified |
| osu-lazer screens/HUD/overlays | ✅ cataloged |
| osu transition timings (file:line) | ✅ cited |
| DTXManiaRS current inventory | ✅ cross-checked |
| Port-first vs M6+ gap split | ✅ separated |
| Open questions + drift register | ✅ listed |
| **Bevy 0.19 UI/API** | ✅ see `docs/BEVY_UX_UI.md` |
| **`bevy_tweening` (djeedai) pin** | ✅ **done 2026-06-28**: git rev `5e3d0c9`; matrix in BEVY_UX_UI.md §6 |
| **Bevy fade/GitaDora/sprite recipes** | ✅ in `BEVY_UX_UI.md` §5–§6 |

**Remaining for implementation (not research):** screenshot baseline vs BocuD
binary; per-sub-act pixel position audit with file:line cites; default skin asset
inventory from `System/Default/Graphics/`; ~~pin `bevy_tweening` 0.19 crate version~~ — **done 2026-06-28** (git rev `5e3d0c9`);
spike `#BMPxx` → `ImageNode` + fade overlay on state change.

---

## 13. Bevy implementation stack

Full Bevy UX/UI guide: **`docs/BEVY_UX_UI.md`**

**Design & implementation plan:** **`docs/UX_UI_DESIGN.md`**

Covers:
- `Node` / `Text` / `ImageNode` / `Sprite` split
- `Camera2d` requirement, `GlobalZIndex` overlays
- OnEnter/OnExit + despawn patterns
- 1500ms fade overlay recipe (ADR-0011)
- GitaDora port approach
- ~~Animation crate decision (`bevy_tweening` vs `bevy_tween` vs hand-rolled)~~ — **resolved 2026-06-28**: `bevy_tweening` (djeedai), git rev `5e3d0c9`; hand-rolled in `dtx-ui::tween::ScalarTween` for v1
- Asset/skin loading, virtual list, dev egui
- BocuD → Bevy task map for Phase J
