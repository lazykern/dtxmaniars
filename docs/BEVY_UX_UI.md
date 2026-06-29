# Bevy UX/UI — implementation reference for DTXManiaRS

> Companion to `docs/UX_UI_AUDIT.md`. Covers **how** we build UX/UI in Bevy 0.19.
> Date: 2026-06-28. Docs via `npx ctx7@latest docs /websites/rs_bevy "…"`.

---

## 0. Stack summary

| Layer | Choice | ADR / note |
|---|---|---|
| Game UI | **native `bevy_ui`** | ADR-0006 |
| Dev tools | **`bevy_egui`** (future) | ADR-0006, `dev-tools` crate |
| Gameplay notes/sprites | **`Sprite` + `Transform`** (world space) | Not flexbox — scroll lane |
| Menu/HUD chrome | **`Node` + `Text` + `ImageNode`** (UI space) | Flexbox layout |
| Camera | **`Camera2d`** required for UI | `main.rs:spawn_ui_camera` |
| Animation v1 | **`bevy_tweening`** (preferred) or hand-rolled alpha | ADR-0007; see §6 |
| Frame pacing | **`bevy_framepace`** (planned) | `BEVY_PATTERNS.md` |
| Audio clock for scroll | **`dtx-timing::AudioClock`** | ADR-0002 — never `Time` for judge/scroll |

**Bevy version:** 0.19 (Rust 1.95+). Workspace uses `default-features = false` on
`bevy`; desktop enables `DefaultPlugins`.

---

## 1. UI vs 2D world — when to use what

```
┌─────────────────────────────────────────────────────────┐
│  UI layer (bevy_ui)          Z: GlobalZIndex overlays   │
│  - Menus, config, result text, fade overlay, GitaDora     │
│  - HUD labels, gauge chrome (if pixel-fixed like BocuD) │
├─────────────────────────────────────────────────────────┤
│  World layer (Sprite/Mesh2d)  Transform + Camera2d      │
│  - Scrolling note chips, lane flush sprites, BGA        │
│  - Judgment popups that move with lane Y                │
└─────────────────────────────────────────────────────────┘
```

| BocuD concept | Bevy mapping |
|---|---|
| `UIDrawable` at fixed HUD coords | `Node { position_type: Absolute, left/top: Px(...) }` + `Text`/`ImageNode` |
| `UIImage` on performance field | `Sprite::from_image(handle)` + `Transform` |
| Full-screen fade / GitaDora bars | Top-level `Node` 100%×100% + `BackgroundColor` alpha tween |
| Skin texture | `AssetServer::load` → `Handle<Image>` → `ImageNode` or `Sprite` |
| Album art / rank icon | `ImageNode` in UI tree |
| Chip scroll | `Transform.translation.y` driven by `AudioClock` ms (see `scroll.rs`) |

**Rule:** BocuD positions cited as `(x, y)` pixels map to `Val::Px(x)`, `Val::Px(y)`
on `PositionType::Absolute` nodes (`dtx-ui::absolute_label`).

---

## 2. Bevy 0.19 UI building blocks

### 2.1 Core components

| Component | Purpose | Project usage |
|---|---|---|
| `Node` | Flexbox/grid layout, size, margin, padding | All screens |
| `Text` | String content (Bevy 0.19: component, not bundle) | Labels, HUD numbers |
| `TextFont` | `font`, `font_size` (`FontSize::Px`) | `dtx-ui::default_text_font` |
| `TextColor` | Foreground color | Per-stage via `stage_label_color` |
| `BackgroundColor` | Fill color for UI node | Panels, fade overlay |
| `BorderColor` | Border stroke | Buttons (future) |
| `ImageNode` | UI image (replaces old `UiImage`) | Skins, rank icons, backgrounds |
| `Button` | Interactive button marker | Future config items |
| `ZIndex` / `GlobalZIndex` | Draw order within/across trees | Fade overlay on top |
| `UiTargetCamera` | Bind UI root to specific camera | Multi-camera (future) |

### 2.2 Layout helpers

```rust
use bevy::prelude::*;

Node {
    width: percent(100),
    height: percent(100),
    display: Display::Flex,
    flex_direction: FlexDirection::Column,
    align_items: AlignItems::Center,
    justify_content: JustifyContent::Center,
    position_type: PositionType::Absolute,  // HUD overlays
    left: Val::Px(430.0),
    top: Val::Px(720.0),
    overflow: Overflow::scroll_y(),         // song list (future)
    ..default()
}
```

Val units: `Px(f32)`, `Percent(f32)`, `Vw`, `Vh`, `Auto`.

### 2.3 ImageNode (UI images)

```rust
// Static image
commands.spawn((
    ImageNode::new(asset_server.load("skins/Default/Graphics/rank_s.png")),
    Node { width: px(128), height: px(128), ..default() },
));

// Texture atlas (sprite sheet — lane flush frames, combo digits)
ImageNode::from_atlas_image(
    texture_handle,
    TextureAtlas { index: frame, layout: atlas_layout_handle },
);
```

Reference: Bevy docs `ImageNode`, `TextureAtlasLayout::from_grid`.

### 2.4 Sprite (gameplay world)

```rust
commands.spawn((
    Sprite::from_image(asset_server.load(path)),
    Transform::from_xyz(lane_x, chip_y, 0.0),
    NoteVisual { chip_id },
));
```

Use for scrolling chips and lane effects. UI `Node` does not follow world
`Transform` — don't mix for moving notes.

### 2.5 Camera

```rust
commands.spawn(Camera2d);  // required once at startup
```

Without `Camera2d`, UI entities spawn but window stays blank
(`app/dtxmaniars-desktop/src/main.rs:48–54`).

For gameplay: same `Camera2d` renders both UI and sprites by default. Use
`GlobalZIndex` to stack fade overlay above everything.

---

## 3. Interaction (Bevy 0.19)

Bevy 0.19 uses **observers** for pointer events (not legacy `Interaction` only):

```rust
commands.spawn((
    Button,
    Node { /* ... */ },
    BackgroundColor(NORMAL),
))
.observe(|mut click: On<Pointer<Click>>, mut focus: ResMut<InputFocus>| {
    focus.0 = Some(click.entity);
    click.propagate(false);
});
```

| Event | Use |
|---|---|
| `Pointer<Click>` | Menu buttons |
| `Pointer<Over>` / `Pointer<Out>` | Hover highlight (M6+) |
| `InputFocus` + `TabIndex` | Keyboard tab navigation |

**DTXManiaRS v1:** keyboard-first (matches BocuD). Mouse = M6+ per audit.

---

## 4. Screen lifecycle (project pattern)

### 4.1 States

```rust
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState { #[default] Startup, Title, /* ... */ }
```

Registered in `game-shell`: `app.init_state::<AppState>()`.

### 4.2 Spawn / despawn

**Current pattern** (marker + OnExit):

```rust
#[derive(Component)]
pub struct TitleEntity;

app.add_systems(OnEnter(AppState::Title), spawn_title)
   .add_systems(OnExit(AppState::Title), despawn_stage::<TitleEntity>);
```

**Recommended upgrade:** `StateScoped(AppState::Title)` on root entity (Bevy 0.14+)
for automatic cleanup — see `docs/BEVY_PATTERNS.md`.

### 4.3 Transition flow

```
NextState.set(AppState::SongLoading)
  → OnExit(SongSelect): despawn song select UI, stop BGM
  → OnEnter(SongLoading): spawn loading UI, start parse
```

Never mutate `State` directly — use `ResMut<NextState<AppState>>`.

---

## 5. Transitions — BocuD → Bevy recipes

### 5.1 1500ms fade overlay (ADR-0011)

BocuD: snapshot texture, linear alpha 1→0 over 1500ms.
Bevy v1: fullscreen black `Node` + `BackgroundColor` alpha lerp.

```rust
#[derive(Resource)]
struct FadeOverlay {
    entity: Entity,
    alpha: f32,
    elapsed_ms: f32,
}

const FADE_MS: f32 = 1500.0; // dtx-ui::SCREEN_FADE_MS

fn tick_fade(time: Res<Time>, mut fade: ResMut<FadeOverlay>, mut colors: Query<&mut BackgroundColor>) {
    fade.elapsed_ms += time.delta_secs() * 1000.0;
    fade.alpha = (1.0 - fade.elapsed_ms / FADE_MS).clamp(0.0, 1.0);
    if let Ok(mut bg) = colors.get_mut(fade.entity) {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, fade.alpha);
    }
}
```

Spawn overlay with `GlobalZIndex(9999)` on enter; despawn when alpha hits 0.
Use only for **SongLoading → Performance** per BocuD StageManager.

### 5.2 GitaDoraTransition (~0.66s)

BocuD: two rotating black bars + logo, progress-driven easing.

Bevy approach:
- Two `Node` or `Sprite` quads, animate `Transform.rotation` + `translation` via tween
- Or pre-bake keyframes from BocuD polynomial (see `GitaDoraTransition.cs`)
- Gate `NextState` on `is_animating` flag (matches BocuD Title/Config/Result)

Screens needing GitaDora: Title, Config, SongSelect enter, SongLoading abort,
Performance exit, Result exit.

### 5.3 What NOT to do in v1

- osu 300ms `FadeIn`/`FadeOut` on every screen (ADR-0010 violation)
- `ScreenStack` push/pop slide (osu pattern, not BocuD)

---

## 6. Animation crates — decision matrix

**Note (2026-06-28):** We standardized on **`bevy_tweening`** (djeedai) for
animations. `bevy_tween` (multirious) is **not** used — it's Bevy 0.18 only.
ADR-0007 still says `bevy_tween` by name but the intent ("Rust tweens, no JSON
loader v1") stands; the crate has been replaced upstream. See ADR-0007 status note.

| Crate | Bevy 0.19 | API style | Status (2026-06-28) |
|---|---|---|---|
| **`bevy_tweening`** (djeedai) | ✅ PR #170 merged (rev `5e3d0c9`); no crates.io 0.16 yet | `Tween::new(Lens, …)`, `TextColorLens`, `EaseFunction` | **Pinned in workspace.dependencies**, swap rev → version when 0.16 ships |
| **`bevy_tween`** (multirious) | ❌ 0.12 = Bevy 0.18 only | `commands.animation().insert(tween(…))` | **Not used** |
| **Hand-rolled** (`dtx-ui::tween::ScalarTween`) | Always works | System mutating `BackgroundColor`, `Node.width`, `Transform` | OK for fade + gauge v1; will be phased out as tweens land |

### 6.1 bevy_tweening setup (when 0.19-ready)

```rust
use bevy_tweening::{Tween, TweeningPlugin, EaseFunction, Lens};

App::new()
    .add_plugins(TweeningPlugin)
    // ...

// Fade TextColor
commands.spawn((
    Text::new("PERFECT"),
    TextColor(Color::WHITE),
    Tween::new(
        TextColorLens { start: Color::WHITE, end: Color::NONE },
        Duration::from_millis(300),
        EaseFunction::QuadraticOut,
    ),
));
```

Features to enable: `bevy_ui`, `bevy_text`, `bevy_sprite` for respective lenses.

### 6.2 Hand-rolled lane flush (v1 fallback)

Match BocuD frame counter — no crate needed:

```rust
if flush.frames_remaining > 0 {
    flush.frames_remaining -= 1;
    alpha = flush.frames_remaining as f32 / MAX_FRAMES as f32;
}
// apply to ImageNode modulate or BackgroundColor
```

Already have state in `dtx-ui/perf_common.rs::LaneFlushGB`.

### 6.3 BocuD Animation JSON (M5+ skin)

BocuD `AnimationClipIO.cs` loads JSON keyframes. ADR-0007 defers loader to M5+.
Until then: hardcode equivalent durations/easing in Rust tweens.

---

## 7. Assets & skins

### 7.1 Loading helpers (`dtx-ui/src/lib.rs`)

```rust
load_font_handle(&asset_server, path)    // → Handle<Font>
load_texture_handle(&asset_server, path) // → Handle<Image>
load_audio_handle(&asset_server, path)   // → Handle<AudioSource>
```

### 7.2 Preload pattern (BEVY_PATTERNS)

```rust
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct SkinAssets {
    #[dependency] pub lane_flush: Handle<Image>,
    #[dependency] pub judgment_perfect: Handle<Image>,
    // ...
}
```

Load in `OnEnter(SongLoading)` or `OnEnter(Performance)`; block until
`AssetServer` reports loaded (or use `bevy_asset_loader` pattern from ARCHITECTURE).

### 7.3 CSkin path resolution

Map BocuD `CSkin.Path("Graphics/lane_flush.png")` →
`assets/skins/{active}/Graphics/lane_flush.png` via `dtx-ui/skin.rs::SkinResolver`.

Validate skin: `Graphics/1_background.jpg` exists (BocuD `CSkin.bIsValid`).

---

## 8. Text & fonts

| Constant | Value | BocuD ref |
|---|---|---|
| `DEFAULT_FONT_PATH` | `fonts/FiraMono-subset.ttf` | placeholder; BocuD uses texgyreadventor |
| `DEFAULT_LABEL_PT` | 18 | CStageTitle |
| `DEFAULT_HUD_PT` | 36 | CActPerfDrumsScore |
| `pt_to_px` | ×1.333 | UIFonts.cs |

Stage colors: `dtx-ui::stage_label_color(state)`.

Future: load TTF from skin folder per `SkinConfig.ini`.

---

## 9. Crate feature matrix

| Crate | bevy features enabled |
|---|---|
| `dtx-ui` | `bevy_asset`, `bevy_text`, `bevy_audio`, `bevy_ui`, `bevy_image` |
| `dtx-bga` | `bevy_asset`, `bevy_render`, `bevy_image`, `bevy_ui` |
| `dtx-input` | `serialize`, `keyboard` |
| `game-menu`, `game-shell`, `game-results`, `gameplay-*` | `default` / full |
| `app/dtxmaniars-desktop` | `DefaultPlugins` |

When adding `bevy_tweening`: enable `bevy_ui`, `bevy_text`, `bevy_sprite` features
on that crate only; keep `dtx-ui` lean.

---

## 10. Patterns already in codebase

| Pattern | File | Notes |
|---|---|---|
| Absolute HUD layout | `gameplay-drums/hud.rs` | Flex column + absolute gauge |
| Gauge width from state | `hud.rs:refresh_gauge_bar` | Mutate `Node.width` — proto animation |
| Song select regions | `game-menu/song_select.rs` | DensityGraphComp, SortMenu, Search overlay |
| Note transform scroll | `gameplay-drums/scroll.rs` | Transform Y, no sprite yet |
| Lane flush state | `dtx-ui/perf_common.rs` | Needs sprite hookup |
| UI camera boot | `main.rs:spawn_ui_camera` | Critical |
| Stage despawn | `game-shell::despawn_stage` | Marker component pattern |

---

## 11. Virtual song list (future)

BocuD legacy list = 1641 LoC virtualized. Bevy options:

1. **Manual windowing:** compute `visible_range` from scroll offset; spawn/despawn
   row `Node` entities (see audit §5.8).
2. **`Node.overflow = scroll_y()`** + clip child rows (simpler, less scalable).
3. Third-party `bevy_virtual_list` — evaluate when SongDb >500 entries.

---

## 12. Dev tools UI (ADR-0006)

`crates/dev-tools` — stub. Planned `bevy_egui` for:
- FPS / frame time graph
- AudioClock vs visual ms overlay
- Entity inspector for HUD nodes
- Log viewer

Gate: `#[cfg(debug_assertions)]` in desktop `main.rs`. Never in release.

---

## 13. Planned dependencies (workspace)

From root `Cargo.toml` (commented):

```toml
# bevy_framepace  = "..."    # INPUT_LATENCY_MS = 16 target
# bevy_tweening   = ...      # pinned in workspace.dependencies (rev 5e3d0c9)
# bevy_egui       = "..."    # dev-tools only
```

**Action before Phase J:**
1. ~~Watch `bevy_tweening` 0.19 release or pin git rev from PR #170~~ — **done 2026-06-28** (PR #170 merged at rev `5e3d0c9`, pinned in `Cargo.toml` `[workspace.dependencies]`, pulled by `dtx-ui`).
2. ~~Update ADR-0007 / BEVY_PATTERNS if we standardize on `bevy_tweening` name~~ — **done 2026-06-28** (this file + `BEVY_PATTERNS.md` + `AGENTS.md` + `UX_UI_AUDIT.md` updated; ADR-0007 status note added).
3. Hand-rolled tweens (`dtx-ui::tween::ScalarTween`) stay until first real tween wired in M5+.

---

## 14. BocuD UX → Bevy implementation map

| Phase J task | Bevy API | BocuD ref |
|---|---|---|
| Fade overlay | `Node` + `BackgroundColor` alpha | StageManager.cs:29 |
| GitaDora wipe | `Sprite`/`Node` + rotation tween | GitaDoraTransition.cs |
| Background JPG | `ImageNode` fullscreen child | CSkin Graphics/N_background.jpg |
| Lane flush | `ImageNode` or `Sprite` + alpha decay | CActPerfDrumsLaneFlushD.cs |
| Judgment popup | `Text` + `TextColor` fade + position | CActPerfDrumsJudgementString.cs |
| Note chip | `Sprite` + `Transform.y` from clock | CActPerfDrumsPad.cs, scroll.rs |
| Rank icon | `ImageNode` | ResultRankIcon.cs |
| Combo digits | `Text` or atlas `ImageNode` | CActPerfDrumsComboDGB.cs |
| Density bars | `Node` children height from data | DensityGraph.cs |
| Config list | `Node` column + `Button` observers | CActConfigList.cs |

---

## 15. Verification checklist

Before claiming UX/UI "ready to implement":

- [x] Bevy 0.19 UI API (`Node`, `Text`, `ImageNode`, observers)
- [x] UI vs Sprite split documented
- [x] Camera2d requirement documented
- [x] State OnEnter/OnExit pattern documented
- [x] Fade overlay recipe (1500ms ADR-0011)
- [x] GitaDora Bevy approach documented
- [x] Animation crate matrix + 0.19 blocker noted
- [x] Asset/skin loading path documented
- [x] Project code patterns indexed
- [ ] Pin exact `bevy_tweening` version for Bevy 0.19 (blocked on upstream)
- [ ] Spike: `ImageNode` + `#BMPxx` load from fixture chart
- [ ] Spike: fade overlay end-to-end on state change

---

## 16. References

- `docs/BEVY_PATTERNS.md` — project rules (fade ms, plugins, events)
- `docs/UX_UI_AUDIT.md` — what to build (BocuD/osu inventory)
- ADR-0006, ADR-0007, ADR-0011
- Bevy docs: `/websites/rs_bevy` via ctx7
- `bevy_tweening` (djeedai) docs: `/djeedai/bevy_tweening` — git rev `5e3d0c9` (0.16 not yet on crates.io)
- `bevy_tween` (multirious): NOT USED — Bevy 0.18 only as of 0.12.0
