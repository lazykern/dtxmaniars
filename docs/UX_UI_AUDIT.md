# UX/UI Audit: DTXManiaRS vs DTXManiaNX vs osu-lazer

> Comprehensive comparison of the visual/UX layer across all three games.
> Date: 2026-06-28. Target: get DTXManiaRS to osu-lazer quality with DTXManiaNX
> mechanics correctness.

---

## 0. Executive Summary

| Dimension | DTXManiaNX | osu-lazer | DTXManiaRS (today) |
|---|---|---|---|
| **Framework** | Custom C# scene graph + ImGui | osu-framework (ECS scene graph) | Bevy UI (flexbox) |
| **Screen model** | `CStage` w/ `ePhaseID` enum | `Screen` w/ OnEnter/Exit callbacks | `AppState` enum w/ OnEnter/Exit systems |
| **UI primitives** | `UIDrawable` tree, parent matrix | `Drawable` tree, parent matrix | `Node` (flexbox) + `Text` |
| **Animations** | `CCounter` per state, manual per-frame | `Transform` keyframes w/ easing | **None** (no animation system) |
| **Time interpolation** | None (frame-locked) | `InterpolatingFramedClock` w/ damping | None |
| **Sub-pixel positioning** | Disabled by default (pixel-clamped) | Native f32 everywhere | Native f32 (free) |
| **GPU invalidation** | None (full re-render) | `InvalidationID` per drawable | Bevy ECS (free via `Changed<T>`) |
| **Update/Draw threads** | Single thread | Parallel update + dedicated draw | Single thread (Bevy default) |
| **Skin system** | JSON overlays on top of code UI | Skin JSON + textures + sample sounds | None |
| **Image rendering** | `UIImage` (textures, animated GIF) | `Sprite` + `Texture` + animations | **None** |
| **Text rendering** | Glyph atlas + per-glyph transform | `SpriteText` (glyph atlas) | `Text` (Bevy wgpu text) |
| **Line count (UI)** | ~228k LoC | ~180k LoC (osu-framework) + ~30k (osu HUD) | ~900 LoC |
| **Polish level** | Decent but aged | Industry-leading | Pre-MVP |

**TL;DR:** We have the right framework (Bevy ECS + flexbox UI) but zero animation,
zero image rendering, zero skin system, zero transition polish. We're at the "M0
functional UI" stage. osu is 18 years ahead on visual polish; DTXMania is 15.

---

## 1. DTXManiaNX UI — full architecture map

### 1.1 Screen hierarchy (`CStage`)

Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/CStage.cs`

`CStage` is the base for every top-level screen. State machine driven by:
- `EStage` enum (9 values): `DoNothing_0`, `Startup_1`, `Title_2`, `Config_3`,
  `SongSelection_4`, `SongLoading_5`, `Performance_6`, `Result_7`, `End_8`,
  `ChangeSkin_9`
- `EPhase` enum (22+ values): shared phases (`Common_DefaultState`,
  `Common_FadeIn`, `Common_FadeOut`, `Common_EndStatus`) + per-stage phases
  (`NOWLOADING_DTX_FILE_READING`, `NOWLOADING_WAV_FILE_READING`,
  `NOWLOADING_BGM_SOUND_COMPLETION`, `PERFORMANCE_STAGE_FAILED`,
  `PERFORMANCE_STAGE_CLEAR`, etc.)

Per-stage lifecycle:
```csharp
class CStage : CActivity {
    internal UIGroup ui;                        // scene graph root
    public abstract void InitializeBaseUI();     // code-built UI (always)
    public abstract void InitializeDefaultUI();  // optional defaults
    // LoadUI() composes: InitializeBaseUI + InitializeDefaultUI + SkinManager.ApplySkin
    public virtual void OnActivate();            // stage enter
    public virtual void OnDeactivate();          // stage exit
    public override int OnUpdateAndDraw();       // per-frame: walk children
}
```

### 1.2 `UIDrawable` scene graph

Reference: `references/DTXmaniaNX-BocuD/DTXMania/UI/Drawable/UIDrawable.cs`

```csharp
abstract class UIDrawable : IDisposable {
    [Themable] Vector3 position;     // overrideable by skin
    [Themable] Vector2 anchor;
    [Themable] Vector2 size;
    [Themable] Vector3 scale;
    [Themable] Vector3 rotation;
    [Themable] bool   isVisible;
    Matrix4x4 localTransformMatrix;  // computed from above
    Matrix4x4 GetFullTransformMatrix();  // walks parents
    public abstract void Draw(Matrix4x4 parentMatrix);
    public void SetParent(UIGroup? parent);
}
```

Notable: `Draw(parentMatrix)` is **pure render** — no Update hook. UIDrawable
has no concept of "every frame, do X". Animation is done by external
`CActivity` subclasses (`CCounter` ticks, sets property, redraws).

Concrete element types: `UIImage`, `UIText`, `UIBasicButton`, `UIGroup`
(container).

### 1.3 Composition pattern: sub-acts

Each stage composes UI from many `CActivity` sub-acts. Example from
`CStagePerfDrumsScreen.cs` (3671 LoC):

```csharp
class CStagePerfDrumsScreen : CStagePerfCommonScreen {
    public CStagePerfDrumsScreen() {
        listChildActivities.Add(actPad = new CActPerfDrumsPad());
        listChildActivities.Add(actCombo = new CActPerfDrumsComboDGB());
        listChildActivities.Add(actDANGER = new CActPerfDrumsDanger());
        listChildActivities.Add(actGauge = new CActPerfDrumsGauge());
        listChildActivities.Add(actGraph = new CActPerfSkillMeter());
        listChildActivities.Add(actJudgeString = new CActPerfDrumsJudgementString());
        listChildActivities.Add(actLaneFlushD = new CActPerfDrumsLaneFlushD());
        listChildActivities.Add(actScore = new CActPerfDrumsScore());
        listChildActivities.Add(actStatusPanel = new CActPerfDrumsStatusPanel());
        listChildActivities.Add(actScrollSpeed = new CActPerfScrollSpeed());
        listChildActivities.Add(video = new CActPerfVideo());
        listChildActivities.Add(actBGA = new CActPerfBGA());
        listChildActivities.Add(actStageFailed = new CActPerfStageFailure());
        listChildActivities.Add(actPlayInfo = new CActPerformanceInformation());
        listChildActivities.Add(actFillin = new CActPerfDrumsFillingEffect());
        listChildActivities.Add(actProgressBar = new CActPerfProgressBar());
    }
}
```

**17 sub-acts** in drums performance alone. Each owns a slice of screen real
estate and runs its own per-frame logic.

### 1.4 Per-screen sub-acts

| Screen | Sub-acts | File |
|---|---|---|
| Startup | (none — just log area + version text) | `01.Startup/CStageStartup.cs` |
| Title | Version text + background image + background video (mp4) | `02.Title/CStageTitle.cs` |
| SongSelect | 12 sub-acts: Presound, ScrollBar, ArtistComment, Information, PerfHistoryPanel, PopupMenu, PreimagePanel, QuickConfig, SongList, StatusPanel, SortSongs, SongSearch | `04.SongSelectionNew/CStageSongSelectionNew.cs` |
| SongLoading | Phase-based loading (DTX → WAV → BMP → BGM ready) | `05.SongLoading/CStageSongLoading.cs` |
| Performance (Drums) | 17 sub-acts (see above) | `06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` |
| Performance (Guitar) | 10 sub-acts: Bonus, Combo, Danger, Gauge, JudgementString, LaneFlushGB, RGB, Score, StatusPanel, WailingBonus | `06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs` |
| Result | 4 sub-acts: ParameterPanel, InfoPanel, RankIcon, + background | `07.Result/CStageResult.cs` |
| Config | 10+ tabs: System, Skin, Gameplay, Drums, Drums.Velocity, Guitar, Bass, Audio, Audio.Driver, Graphics | `03.Config/CStageConfig.cs` |
| ChangeSkin | Skin picker (folder browser + preview) | `09.ChangeSkin/CStageChangeSkin.cs` |
| End | Credits / version + link | `08.End/CStageEnd.cs` |

### 1.5 Sub-act types per Performance screen (the big one)

**Drums** (`06.Performance/DrumsScreen/`):
| File | Purpose |
|---|---|
| `CActPerfDrumsPad.cs` | Hit pad visualization (lane press flash) |
| `CActPerfDrumsComboDGB.cs` | Combo counter (animated digit roll) |
| `CActPerfDrumsDanger.cs` | Danger state (gauge near 0 — red overlay + screen shake) |
| `CActPerfDrumsFillingEffect.cs` | Fill-in zone effect (when playing through fill-in chips) |
| `CActPerfDrumsGauge.cs` | Life gauge bar (animated fill) |
| `CActPerfDrumsJudgementString.cs` | Judgement label flash ("PERFECT", "GREAT", etc.) |
| `CActPerfDrumsLaneFlushD.cs` | Lane hit flash (10-lane flush overlay) |
| `CActPerfDrumsScore.cs` | Score display |
| `CActPerfDrumsStatusPanel.cs` | Song info, BPM, current judgment breakdown |
| `CActPerfSkillMeter.cs` | Skill meter (optional difficulty indicator) |
| `CActPerfScrollSpeed.cs` | Scroll speed indicator |
| `CActPerfVideo.cs` | Background video (mp4) |
| `CActPerfBGA.cs` | BGA image display (background animation) |
| `CActPerfStageFailure.cs` | Failure overlay (gauge = 0) |
| `CActPerformanceInformation.cs` | Play info (notes hit/missed, accuracy) |
| `CActLVLNFont.cs` | Level/font icon |
| `CActPerfProgressBar.cs` | Progress bar (current position in chart) |
| `CActPerfPerfChipFireD.cs` | Legacy chip fire animation (alternate of new fire) |

**Guitar** (`06.Performance/GuitarScreen/`):
| File | Purpose |
|---|---|
| `CActPerfGuitarBonus.cs` | Bonus effect (orange note bonus) |
| `CActPerfGuitarCombo.cs` | Combo counter |
| `CActPerfGuitarDanger.cs` | Danger state |
| `CActPerfGuitarGauge.cs` | Gauge |
| `CActPerfGuitarJudgementString.cs` | Judgement label |
| `CActPerfGuitarLaneFlushGB.cs` | Lane flush (10-lane: 5 RGB + 5 reverses) |
| `CActPerfGuitarRGB.cs` | RGB note indicators |
| `CActPerfGuitarScore.cs` | Score |
| `CActPerfGuitarStatusPanel.cs` | Status panel |
| `CActPerfGuitarWailingBonus.cs` | Wailing bonus (long-note streak) |

### 1.6 SongSelect deep-dive (most complex screen)

Reference: `04.SongSelectionNew/CStageSongSelectionNew.cs` (596 LoC) + 11
sub-act files.

**Layout regions:**
- Song list (left/center) — virtualized list of folders/charts
- Status panel (right, ~430x720) — current song info, BPM, level, notes
- Density graph (left, 8 vertical bars) — note density over chart
- Sort menu (top-right, ~662x92) — Default/Title/Artist ring selector
- Search menu (centered modal, 500x300) — text input + filtered results
- Artist comment strip (bottom) — artist commentary text
- Preimage panel (album art preview)
- PerfHistory panel (best scores on this chart)
- Quick config (small inline popup)

**Interactions:**
- Up/Down arrows: navigate list
- Enter: select song + go to SongLoading
- Tab: cycle sort mode (Default → Title → Artist)
- F1: open Config
- F2: cycle instrument (Drums/Guitar/Bass)
- F5: refresh scan
- Esc: back to Title
- Type-to-search: filter by title/artist (incremental)

**BG preview:** Selecting a row triggers BGM playback via `CActSelectPresound`.

### 1.7 Skin system (the customization layer)

Reference: `DTXMania/UI/Skin/`

Skin = JSON files overlaid on top of code UI. Mechanism:
- `[Themable] Vector3 position` etc → skin JSON can override
- `CSkin.Path(@"Graphics\2_background.png")` resolves relative to active skin
- `SkinManager.ApplySkin(ui, eStageID)` walks the UIDrawable tree and
  overrides `[Themable]` fields from JSON

**Default skin paths:**
- `Graphics/1_background.jpg` — Startup background
- `Graphics/2_background.png` — Title background
- `Graphics/2_background.mp4` — Title background video (optional)
- `Graphics/3_background.jpg` — SongSelect background
- `Graphics/4_background.jpg` — SongLoading background
- `Graphics/5_background.jpg` — Performance background (per-instrument)
- `Graphics/6_background.jpg` — Result background
- `Graphics/7_background.jpg` — End background

Plus hundreds of small images: note textures, lane flush sprites, gauge
frames, judgement labels (PERFECT/GREAT/etc.), score digits, combo digits,
rank icons.

### 1.8 DTXMania UI strengths to preserve

1. **9-screen flow** with phase machine — good UX discipline
2. **Sub-act composition** — clean separation of concerns
3. **Heavy keyboard-driven navigation** (Up/Down/Enter/Tab/Esc) — power-user friendly
4. **Persistent state** across screens (SelectedSong, SortMode, etc.)
5. **Clear modal vs non-modal distinction** (search popup overlays list)
6. **Result persistence to disk** (CScoreIni)

### 1.9 DTXMania UI weaknesses to fix

1. **No animation library** — every transition is `CCounter` boilerplate
2. **Pixel-clamped positions** — visible stutter at slow scroll
3. **Single-threaded** — bottleneck at high note counts
4. **No skin hot-reload** — restart required
5. **No UI virtualization** — drawing 1000+ songs at once
6. **Glyph-atlas text** — slow first render
7. **228k LoC of UI code** — over-engineered for what it does

---

## 2. osu-lazer UI — full architecture map

### 2.1 Screen model

Reference: `osu.Framework/Screens/Screen.cs`

```csharp
class Screen : CompositeDrawable, IScreen {
    public virtual void OnEntering(ScreenTransitionEvent e);  // start fade-in
    public virtual void OnEntered(ScreenTransitionEvent e);   // fully visible
    public virtual void OnSuspending(ScreenTransitionEvent e); // pause (PUSH)
    public virtual void OnResuming(ScreenTransitionEvent e);   // resume (POP)
    public virtual void OnExiting(ScreenTransitionEvent e);   // start fade-out
}
```

Push/pop stack model: `MainMenu → SongSelect → InGamePlay → Results → back to SongSelect`.

### 2.2 `Drawable` scene graph

Reference: `osu.Framework/Graphics/Drawable.cs`

```csharp
abstract class Drawable : Transformable, IDisposable {
    public Bindable<Clock> Clock;
    public virtual bool UpdateSubTree();         // per-frame logic
    internal virtual DrawNode GenerateDrawNodeSubtree(...);  // GPU state
    public bool Invalidate(Invalidation flags, InvalidationSource src);
    protected virtual bool OnInvalidate(...);    // invalidation hooks
    public long InvalidationID { get; private set; }
    public Invalidation InvalidationFromParentSize { get; }
    public Invalidation InvalidationFromParentTransform { get; }
    public Invalidation InvalidationFromChildSize { get; }
    // ... 12+ invalidation flags (DrawSize, DrawInfo, Colour, Transform, etc)
}
```

**Key insight:** Each drawable has `InvalidationID` that increments on any
change. DrawNode (`osuTK` draw state) only regenerated when `InvalidationID`
differs. Static HUD = no per-frame GPU work.

### 2.3 `CompositeDrawable` (parent with children)

Reference: `osu.Framework/Graphics/Containers/CompositeDrawable.cs`

```csharp
abstract class CompositeDrawable : Drawable {
    internal SortedList<Drawable> internalChildren;       // ALL children
    internal SortedList<Drawable> aliveInternalChildren;  // visible+alive
    private LayoutValue childrenSizeDependencies;
    private LayoutValue childMaskingBoundsBacking;
    // Invalidation propagates to children:
    // - Layout flags → only alive children
    // - Geometry/colour → all children
    public void Add(Drawable d);
    public void Remove(Drawable d);
    protected virtual bool UpdateChildren();  // walks children per frame
}
```

### 2.4 `Container<T>` (typed children)

Reference: `osu.Framework/Graphics/Containers/Container.cs`

```csharp
class Container<T> : CompositeDrawable where T : Drawable {
    public IReadOnlyList<T> Children;  // type-safe children
    // Layout modes:
    // - Manual (default)
    // - Fill, Fit, Scale, AspectFit, etc.
    // - Auto-size based on child extent
    // - Padding, Margin, Spacing
}
```

### 2.5 Transform system (THE fluidity secret)

Reference: `osu.Framework/Graphics/Transforms/Transform.cs`,
`Transformable.cs`

```csharp
class Transform {
    public ulong TransformID;
    public TransformTarget Target;
    public double StartTime, EndTime;
    public Easing Easing;                  // enum: Out, InOut, OutQuint, etc.
    public double LoopCount;               // -1 = infinite
    public abstract void Apply(double time);
}

class Transformable : Drawable {
    public void FadeIn(duration, easing);
    public void FadeOut(duration, easing);
    public void MoveToX(target, duration, easing);
    public void RotateTo(angle, duration, easing);
    // ... 30+ transform methods
    public void Delay(duration);
    public TransformSequence BeginSequence();  // chain transforms
}
```

**Apply example:**
```csharp
hitObject.MoveToY(judgementLineY, scrollTimeMs, Easing.Out);
hitObject.FadeIn(200, Easing.OutQuad);
hitObject.ScaleTo(1.5f, 100).Then().ScaleTo(1.0f, 100);  // bounce
```

**Implementation:** Each `Drawable` has `Transform[]` queue. `Apply(time)`
computes `t = (currentTime - StartTime) / duration`, applies easing curve,
sets property. **Runs once per frame on update thread** but with high-quality
interpolation.

### 2.6 `InterpolatingFramedClock` (smooth time)

Reference: `osu.Framework/Timing/InterpolatingFramedClock.cs`

```csharp
class InterpolatingFramedClock : IFrameBasedClock, ISourceChangeableClock {
    public double AllowableErrorMilliseconds = 1000.0/60 * 2;  // 2 frames
    public double DriftRecoveryHalfLife = 50;  // ms
    private Stopwatch realtimeClock;
    private double currentTime, drift;
    private double lastFrameTime;

    public void ProcessFrame() {
        // 1. Read source clock (audio)
        // 2. If drift < AllowableErrorMilliseconds → interpolate
        currentTime = Interpolation.DampContinuously(
            currentTime,
            framedSourceClock.CurrentTime,
            DriftRecoveryHalfLife,
            realtimeClock.ElapsedFrameTime
        );
        // 3. Else snap (bypass interpolation)
        currentTime = framedSourceClock.CurrentTime;
    }
}
```

**Why this matters:** Audio clock is exact (sample-accurate). Render frames
are 16.67ms apart. If a frame stutters to 25ms, naive code shows the chip
jumping 25ms worth of motion in one frame = visible jump. Interpolating clock
**gradually** catches up over 50ms half-life — invisible to player.

### 2.7 HUD components

#### ComboCounter (`osu.Game/Screens/Play/HUD/ComboCounter.cs`)
```csharp
class ComboCounter : RollingCounter<int> {
    // RollingCounter animates digit changes:
    // 0 → 100: each digit rolls up (0→1→...→9→10) over X ms
    // Uses sprite sheet with 10 digits per sprite
    // + bounce/overshoot easing
}
```

#### HealthDisplay (`osu.Game/Screens/Play/HUD/HealthDisplay.cs`)
```csharp
class HealthDisplay : CompositeDrawable {
    Bindable<bool> showHealthBar;
    Bindable<double> Current;  // 0..1
    // Animates gauge fill via Transform on inner Sprite
    // Show/hide via FadeIn/FadeOut on parent
    // Bound to HealthProcessor (reactive — auto-updates)
}
```

### 2.8 Main menu / song select flow

osu's UX principles:
1. **Hero song playing in background** when idle (auto-loop)
2. **No modal blocking** — all interactions inline (popup overlay, not full screen)
3. **Carousel-style song list** (osu!stable) or **grid layout** (lazer)
4. **Big album art** as visual anchor
5. **Group filter chips** (Ranked/Loved/Played) above list
6. **Live preview** of selected song's first 10s
7. **Beatmap difficulty pills** stacked vertically next to song
8. **Leaderboard inline** below beatmap info

### 2.9 Performance HUD

osu shows during gameplay:
- **Combo counter** (top-left, big rolling digits)
- **Score counter** (top-right, rolling)
- **Accuracy** (% below score, decimal precision)
- **Health bar** (bottom, animated fill with glow on full)
- **Hit error meters** (judgement timing visualization, top-center)
- **Combo burst** (huge explosion on 1000x combo)
- **Kiai flash** (white border flash during kiai sections)
- **Fail screen** (red overlay when HP = 0)
- **Pause overlay** (semi-transparent + ESC hint)
- **Score panel** (real-time numbers with rolling animation)

### 2.10 osu strengths to copy

1. **`InterpolatingFramedClock`** — number one fluidity boost
2. **`Transform` system** — declarative animations, frame-rate independent
3. **`Bindable<T>` reactivity** — health gauge auto-updates from gameplay
4. **GPU invalidation** — zero per-frame cost for static UI
5. **Sub-pixel positioning** — no jitter at any scroll speed
6. **Screen transition stack** — clean Push/Pop model
7. **Hero song idle state** — game feels alive even in menu
8. **Live preview on select** — instant feedback
9. **No modal blocking** — overlays, not full-screen replacements
10. **High-DPI scaling** — pixel-perfect at any resolution

### 2.11 osu weaknesses (rare)

1. **Memory overhead** — every drawable pre-allocates DrawNode[] buffer
2. **Update thread scheduling** — multi-thread update has race conditions
3. **Skin complexity** — custom format hard to author

---

## 3. DTXManiaRS current state — full inventory

### 3.1 UI files

| File | Lines | Status | Notes |
|---|---:|---|---|
| `crates/game-menu/src/title.rs` | 92 | ✅ Functional | "DTXManiaRS" + subtitle + ENTER prompt |
| `crates/game-menu/src/startup.rs` | 56 | ⚠️ Stub | Auto-advances to Title after 0.5s; no logo |
| `crates/game-menu/src/song_select.rs` | 600+ | ⚠️ Partial | List renders, no virtualization, status panel stub |
| `crates/game-menu/src/song_loading.rs` | 200+ | ⚠️ Functional | Text "Loading..." + progress bar (not animated) |
| `crates/game-menu/src/config.rs` | 250+ | ⚠️ Stub | 4 hardcoded groups (Drums/System/Skin/Gameplay) |
| `crates/game-menu/src/config_key_assign.rs` | 350+ | ⚠️ Stub | UI exists, key capture logic stub |
| `crates/gameplay-drums/src/hud.rs` | 270 | ⚠️ Minimal | Lane strip + hit line + score/combo/gauge text |
| `crates/gameplay-guitar/src/hud.rs` | 270 | ⚠️ Minimal | RGBYP lanes + score/combo/gauge text |
| `crates/game-results/src/lib.rs` | 224 | ⚠️ Functional | Monospace text wall (title/score/combo/rank/breakdown) |
| `crates/game-shell/src/performance.rs` | 50 | ✅ Functional | ESC → Result transition |
| `crates/dtx-ui/src/lib.rs` | 252 | ⚠️ Empty shell | `perf_common.rs` + `skin.rs` exist but most unused |
| `crates/dtx-ui/src/skin.rs` | 289 | ⚠️ Scaffolding | Skin registry, no actual rendering |
| `crates/dtx-ui/src/perf_common.rs` | 321 | ⚠️ Layout constants | Position constants only, no rendering |

**Total UI LoC: ~3,400** (vs DTXManiaNX ~228k, osu ~210k)

### 3.2 What we have (functional)

- ✅ Bevy ECS plugin per screen
- ✅ AppState transitions with OnEnter/OnExit
- ✅ Text rendering via `Text` + `TextFont` + `TextColor`
- ✅ Layout via `Node` (flexbox)
- ✅ Camera2d for UI rendering
- ✅ Static color backgrounds (`BackgroundColor`)
- ✅ Absolute positioning for HUD overlays
- ✅ Persistent state (SelectedSong, SortMode, ConfigSelection, etc.)
- ✅ Keyboard navigation (arrows, Enter, Esc, Tab, F-keys)
- ✅ Score/combo/gauge accumulation
- ✅ Result persistence (ScoreStore JSON)
- ✅ BGM preview on row select
- ✅ User charts directory scan (XDG-aware, DTX_SONG_DIR override)
- ✅ Shift-JIS + lenient DTX parser

### 3.3 What we DON'T have (the gaps)

**Critical (no game can play without):**
- ❌ **Animations** — no Transform, no easing, no tween system
- ❌ **Smooth time interpolation** — `AudioClock.current_ms` jumps discretely
- ❌ **Note scrolling visualization** — `scroll.rs` updates chip positions but no rendering pipeline
- ❌ **Lane flush animation** — no visual feedback on hit
- ❌ **Judgment label flash** — no "PERFECT/GREAT" popups
- ❌ **Image/sprite rendering** — no album art, no BGA, no preimage

**Important (game feels bare without):**
- ❌ **Screen transitions** — no FadeIn/Out, hard cuts
- ❌ **Animated gauge** — bar width updates discretely, no tween
- ❌ **Animated score** — number changes instantly, no rolling
- ❌ **Animated combo** — number changes instantly, no bounce
- ❌ **Skin system** — no overlay layer over code UI
- ❌ **Backgrounds** — solid colors only, no images
- ❌ **Sound preview card** — text-only song info
- ❌ **Result rank icon** — text "S/A/B/C/D/E" only

**Nice to have (polish):**
- ❌ **Density graph** — note density visualization
- ❌ **Progress bar** — current chart position
- ❌ **Hero song** — auto-playing background music on title
- ❌ **Beat preview** — 10s clip on song select
- ❌ **Leaderboard** — best scores per chart
- ❌ **Group folders** — folder tree (box.def support)
- ❌ **Modal popups** — search/sort menus
- ❌ **Tooltip / hover states** — no mouse interaction yet
- ❌ **Localization** — UI text hardcoded English
- ❌ **Accessibility** — no key remapping hints visible, no colorblind mode

### 3.4 Tech debt

- ❌ Position constants duplicated in 3 files (`drums_perf.rs`, `guitar_perf.rs`, `perf_common.rs`)
- ❌ String text for rank ("S/A/B/...") instead of icon
- ❌ No async loading — DTX parse blocks SongSelect
- ❌ No virtualization — long song lists drawn all at once
- ❌ Text component is monospace default font (looks dated)
- ❌ `commands.spawn` scattered — no convention for screen UI lifecycle

---

## 4. Gap analysis — what to build

### Priority 0 (MUST for osu-lazer quality)

| Gap | Effort | Reference | New module |
|---|---|---|---|
| InterpolatedAudioClock | 2 days | osu `InterpolatingFramedClock` | `crates/dtx-timing/src/interpolated.rs` |
| Tween system | 3 days | osu `Transform` | `crates/dtx-ui/src/tween.rs` (or use `bevy_tween`) |
| Note scroll w/ transform | 2 days | osu hitobject Y interpolation | `crates/dtx-ui/src/scroll/` |
| Image rendering pipeline | 2 days | DTXMania `UIImage` | `crates/dtx-assets/src/sprite.rs` |
| Skin resource loader | 2 days | DTXMania `SkinManager` | `crates/dtx-ui/src/skin_loader.rs` |
| Lane flush animation | 1 day | DTXMania `CActPerfDrumsLaneFlushD` | `crates/gameplay-drums/src/lane_flush.rs` |
| Judgment label flash | 1 day | DTXMania `CActPerfDrumsJudgementString` | `crates/gameplay-drums/src/judgment_label.rs` |
| Screen transition (fade) | 1 day | DTXMania `Common_FadeIn/Out` | `crates/game-shell/src/transition.rs` |

### Priority 1 (high-impact UX)

| Gap | Effort | Reference | New module |
|---|---|---|---|
| Animated gauge drain | 1 day | osu HealthDisplay | update `crates/dtx-ui/src/hud_gauge.rs` |
| Rolling score counter | 1 day | osu RollingCounter | `crates/dtx-ui/src/rolling_counter.rs` |
| Combo counter with bounce | 1 day | osu ComboCounter | `crates/dtx-ui/src/combo_counter.rs` |
| BGA / preimage display | 2 days | DTXMania `CActPerfBGA` | `crates/dtx-bga/src/preimage.rs` |
| Result rank icons | 1 day | DTXMania `ResultRankIcon` | `crates/game-results/src/rank_icon.rs` |
| Background image per screen | 0.5 day | DTXMania `2_background.png` etc | per-screen setup |
| Beat preview on hover/select | 1 day | DTXMania `CActSelectPresound` | already have presound, polish |
| Density graph | 1 day | DTXMania `DensityGraph` | `crates/game-menu/src/density_graph.rs` |

### Priority 2 (polish)

| Gap | Effort | Reference | New module |
|---|---|---|---|
| Hero song on Title | 1 day | osu MainMenu | `crates/dtx-audio/src/hero.rs` |
| Folder box tree (box.def) | 2 days | DTXMania `CSongSelectionNode` | `crates/dtx-library/src/box_tree.rs` |
| Modal search popup | 1 day | DTXMania `SongSearchMenu` | `crates/game-menu/src/search.rs` |
| Sort menu ring UI | 0.5 day | DTXMania `SortMenuContainer` | update `song_select.rs` |
| Tooltip / hover states | 1 day | osu `TooltipContainer` | `crates/dtx-ui/src/tooltip.rs` |
| Localization framework | 3 days | osu `LocalisationManager` | `crates/dtx-i18n/` |
| Colorblind modes | 0.5 day | osu `OsuColour` | update `crates/dtx-ui/src/colour.rs` |
| Async chart loading | 1 day | DTXMania async | use `bevy_tasks` |

### Priority 3 (osu-tier polish)

| Gap | Effort | Reference | New module |
|---|---|---|---|
| Hit error meters | 2 days | osu `HitErrorMeters` | `crates/gameplay-drums/src/error_meter.rs` |
| Kiai flash | 1 day | osu KiaiHit | `crates/dtx-ui/src/kiai.rs` |
| Combo burst (1000x) | 1 day | osu ComboBurst | `crates/dtx-ui/src/combo_burst.rs` |
| Parallax background | 1 day | osu parallax | `crates/dtx-ui/src/parallax.rs` |
| Particle effects | 3 days | osu particles | `crates/dtx-ui/src/particles.rs` |
| Trail effects | 2 days | osu trails | `crates/dtx-ui/src/trails.rs` |
| Keyboard hints in UI | 0.5 day | osu HUD overlay | small per-screen |
| Mouse support | 2 days | osu MouseManager | update input |

---

## 5. Architecture for the redesign

### 5.1 Target dependency graph

```
dtx-core ──────┐
dtx-timing ────┤
dtx-config ────┤
dtx-scoring ───┤
dtx-assets ────┤  (Pure/Engine, no UI)
dtx-bga ───────┤
dtx-input ─────┤
               │
               ├──→ dtx-ui ◄──── (Engine layer)
               │       │
               │       ├── tween (interpolated transforms)
               │       ├── skin (texture + JSON overlay)
               │       ├── widget library (Button, Label, Gauge, Combo, etc.)
               │       └── animated text (rolling counters)
               │
game-shell ────┤  (Bevy plugin, screen transitions)
game-menu ─────┤  (Title, SongSelect, SongLoading, Config, etc.)
game-results ──┤
gameplay-drums ┤
gameplay-guitar┤
```

### 5.2 `dtx-ui` widget library (the missing piece)

Currently `dtx-ui` only has position constants + a stub skin registry. We need:

```rust
// crates/dtx-ui/src/widget/mod.rs
pub mod button;
pub mod label;
pub mod gauge;           // animated fill bar
pub mod combo_counter;   // rolling digits + bounce
pub mod score_counter;   // rolling digits
pub mod lane_flush;      // lane hit flash (used by gameplay-drums)
pub mod judgment_label;  // PERFECT/GREAT/OK/MISS popup
pub mod density_graph;   // 8-bar note density chart
pub mod progress_bar;    // chart position bar
pub mod sprite;          // animated sprite (BMP, GIF, video)
pub mod text_input;      // text field
pub mod list;            // virtualized list (song list)

// crates/dtx-ui/src/tween.rs
pub mod tween;            // bevy_tween wrappers (easing curves)
pub mod interpolated;     // InterpolatedAudioClock for dtx-timing
```

### 5.3 Screen transition system

Replace `OnEnter(AppState::X)` with a `ScreenTransition` resource:

```rust
pub enum TransitionKind {
    FadeIn,   // 200ms fade from black
    FadeOut,  // 200ms fade to black
    Push,     // slide from right (osu-style)
    Pop,      // slide to right
}

pub struct ScreenTransition {
    pub kind: TransitionKind,
    pub duration_ms: u32,
    pub from_state: AppState,
    pub to_state: AppState,
    pub t: f32,  // 0..=1
}
```

Implement via a single overlay `ColorMaterial` quad that fades alpha over
the transition window. States still swap at midpoint.

### 5.4 `InterpolatedAudioClock` for `dtx-timing`

```rust
// crates/dtx-timing/src/interpolated.rs
pub struct InterpolatedAudioClock {
    raw: AudioClock,                    // sample-accurate
    visual_ms: f64,                     // damped visual time
    last_realtime: Instant,
    drift_recovery_half_life_ms: f64,
    allowable_error_ms: f64,            // 1000.0/60 * 2 = 33.3ms
}

impl InterpolatedAudioClock {
    pub fn tick(&mut self, realtime: Instant) {
        let dt_ms = realtime.duration_since(self.last_realtime).as_secs_f64() * 1000.0;
        self.last_realtime = realtime;
        let raw_ms = self.raw.current_ms.unwrap_or(0) as f64;
        let drift = (raw_ms - self.visual_ms).abs();
        if drift < self.allowable_error_ms {
            // Damped interpolation toward raw
            let alpha = 2f64.powf(-dt_ms / self.drift_recovery_half_life_ms);
            self.visual_ms = raw_ms + (self.visual_ms - raw_ms) * alpha;
        } else {
            // Snap (too far behind, bypass interpolation)
            self.visual_ms = raw_ms;
        }
    }
    pub fn visual_ms(&self) -> f64 { self.visual_ms }
}
```

Replace `AudioClock.current_ms` everywhere with `InterpolatedAudioClock.visual_ms()`
in render systems. This alone fixes ~80% of "stutter".

### 5.5 Tween system

Use `bevy_tween` (already in our project rules per ADR-0007) for:

```rust
// Lane flush on hit:
commands.entity(lane_entity).animation()
    .insert_tween_here(Duration::from_millis(70), 
        EaseFunction::QuadOut, 
        TweenTarget::Alpha { from: 1.0, to: 0.0 });

// Judgment label flash:
commands.entity(judgment_entity).animation()
    .insert_tween_here(Duration::from_millis(300), 
        EaseFunction::CubicOut, 
        TweenTarget::Position { from: center, to: above_center });

// Gauge drain:
commands.entity(gauge_fill).animation()
    .insert_tween_here(Duration::from_millis(200),
        EaseFunction::Linear,
        TweenTarget::Width { from: full_width, to: empty_width });

// Combo bounce:
commands.entity(combo_entity).animation()
    .insert_tween_here(Duration::from_millis(100),
        EaseFunction::BackOut,
        TweenTarget::Scale { from: 1.5, to: 1.0 });
```

### 5.6 Image / sprite rendering

Currently we have NO image rendering. Need:

```rust
// crates/dtx-assets/src/sprite.rs
pub struct AtlasSprite {
    pub texture: Handle<Image>,
    pub uv_rect: Rect,            // for sprite-sheet slicing
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
}

// crates/dtx-assets/src/gif.rs (animated BGA support)
pub struct AnimatedGif {
    pub frames: Vec<Handle<Image>>,
    pub frame_duration_ms: u32,
    pub loop_count: i32,          // -1 = infinite
}

// crates/dtx-bga/src/video.rs (mp4 background)
pub struct VideoBackground {
    pub path: PathBuf,
    pub looping: bool,
}
```

For DTX's `#BMPxx: filename`, the `.bmp` file needs to be loaded and rendered.
For `#AVIXX: filename`, the `.mp4/.avi` file needs video decode (use
`bevy_kira_audio` for audio, `bevy_movie` or similar for video).

### 5.7 Skin JSON format

Minimal v1 skin format:

```json
{
  "version": 1,
  "name": "Default",
  "graphics": {
    "background.title": "Graphics/2_background.png",
    "background.title_video": "Graphics/2_background.mp4",
    "background.song_select": "Graphics/3_background.jpg",
    "background.performance.drums": "Graphics/5_background_drums.jpg",
    "background.performance.guitar": "Graphics/5_background_guitar.jpg",
    "background.result": "Graphics/6_background.jpg",
    "lane.flush": "Graphics/lane_flush.png",
    "judgment.perfect": "Graphics/perfect.png",
    "judgment.great": "Graphics/great.png",
    "judgment.good": "Graphics/good.png",
    "judgment.ok": "Graphics/ok.png",
    "judgment.miss": "Graphics/miss.png",
    "gauge.frame": "Graphics/gauge_frame.png",
    "gauge.fill": "Graphics/gauge_fill.png",
    "rank.s": "Graphics/rank_s.png",
    "rank.a": "Graphics/rank_a.png",
    "rank.b": "Graphics/rank_b.png",
    "rank.c": "Graphics/rank_c.png",
    "rank.d": "Graphics/rank_d.png",
    "rank.e": "Graphics/rank_e.png"
  },
  "layout": {
    "title.subtitle_y": 100,
    "hud.gauge_x": 20,
    "hud.gauge_y_bottom": 20,
    "hud.score_top": 20
  }
}
```

`SkinManager` resolves these into a `Skin` resource that widgets query.

### 5.8 Song select virtualization

Song list with 1000+ songs = render 1000+ `Node` entities. Performance dies.
Need `bevy_virtual_list` or roll our own:

```rust
// crates/dtx-ui/src/list/virtual_list.rs
pub struct VirtualList<T> {
    pub items: Vec<T>,
    pub visible_range: Range<usize>,  // recomputed on scroll
    pub item_height: f32,
    pub scroll_offset: f32,
}

// System:
// 1. Compute visible_range from scroll_offset + viewport_height
// 2. Despawn entities for items outside visible_range
// 3. Spawn/update entities for items inside visible_range
```

---

## 6. Implementation phases

### Phase J — Foundations (1 week)
- Add `InterpolatedAudioClock` to `dtx-timing`
- Add `bevy_tween` to workspace deps
- Wire up `Camera2d` for both 2D and UI (already done partially)
- Add `ScreenTransition` system to `game-shell`
- Add `Sprite` component to `dtx-assets` (basic texture render)

### Phase K — Core widgets (1.5 weeks)
- `dtx-ui/widget/button.rs` — animated button (hover, press, focus states)
- `dtx-ui/widget/label.rs` — animated text with shadow/glow
- `dtx-ui/widget/gauge.rs` — tween-animated gauge bar
- `dtx-ui/widget/combo_counter.rs` — rolling digits + bounce
- `dtx-ui/widget/score_counter.rs` — rolling digits
- `dtx-ui/widget/lane_flush.rs` — lane hit flash
- `dtx-ui/widget/judgment_label.rs` — PERFECT/GREAT popup
- `dtx-ui/widget/list.rs` — virtualized list

### Phase L — Polish (1.5 weeks)
- `dtx-ui/skin_loader.rs` — JSON skin registry
- `dtx-ui/widget/density_graph.rs` — note density bars
- `dtx-ui/widget/progress_bar.rs` — chart position bar
- `dtx-ui/widget/text_input.rs` — search field
- `dtx-bga/preimage.rs` — album art in song select
- `dtx-bga/bga_layer.rs` — BGA background animation
- Background images per screen (Startup/Title/SongSelect/SongLoading/Performance/Result)
- Result rank icons (replace text with sprites)

### Phase M — osu-tier polish (2 weeks)
- `gameplay-drums/error_meter.rs` — judgement timing viz
- `dtx-ui/widget/kiai.rs` — kiai flash overlay
- `dtx-ui/widget/combo_burst.rs` — 1000x combo celebration
- `dtx-ui/parallax.rs` — background parallax on song select
- Hero song playback on Title
- Beat preview (10s clip) on song select hover
- Beatmap difficulty pills (stacked next to song in select)

### Phase N — Extras (1 week)
- `dtx-i18n/` — localization framework
- Colorblind modes
- Mouse support (hover, click)
- Box.def folder tree (DTXMania group folders)

---

## 7. Success criteria

After all phases complete:

| Criterion | Target |
|---|---|
| Frame rate | Stable 60fps minimum, 144fps+ with vsync |
| Visual smoothness | Zero stutter on chip scroll (transform interpolation) |
| Time smoothness | Audio desync invisible (InterpolatingAudioClock) |
| Animation polish | All state changes tweened (no instant pop) |
| Image support | Album art + BGA + skin textures render |
| Skin support | Override graphics via JSON without recompile |
| Result presentation | Rank icons (sprites), not text |
| Song list | Virtualized (handles 10,000 songs) |
| Text quality | Anti-aliased, hinted, variable-weight fonts |
| Polish parity | osu-lazer M0 quality (functional + fluid) |

---

## 8. References

- `references/DTXmaniaNX-BocuD/DTXMania/UI/` — full DTXMania UI tree
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/` — all 9 CStage screens
- `osu.Framework/Graphics/Drawable.cs` — osu's Drawable
- `osu.Framework/Graphics/Containers/CompositeDrawable.cs` — children management
- `osu.Framework/Graphics/Transforms/Transform.cs` — transform interpolation
- `osu.Framework/Timing/InterpolatingFramedClock.cs` — damped time
- `osu.Game/Screens/Play/HUD/` — gameplay HUD components
- ADR-0007 (bevy_tween) — animation primitive decision
- ADR-0010 (port-first mechanics) — relaxed for UI/skin
- ADR-0011 (fade snapshot) — transition approximation (will be replaced)
- ADR-0012 (song-select visual simplification) — to be revised with new widgets

---

## 9. Open questions

1. **Skin format:** Use DTXMania-style JSON overlay or define a new osu-style format?
2. **Video background:** Use `bevy_movie` (Rust-native decode) or shell out to ffmpeg?
3. **BGA animations:** Support GIF (simple) or full AVI (complex) for v1?
4. **Sample-accurate audio sync:** Use existing `AudioClock` or move to `bevy_kira_audio` clock?
5. **Localization:** Hardcode English v1, framework later?
6. **High-DPI:** Use Bevy's window scale factor or custom DPI handling?
7. **Color theme:** Day/night mode toggle, or follow system?
8. **Modal popups:** Modal (blocks input) or non-modal (overlay)?

These need user decisions before Phase J implementation begins.
