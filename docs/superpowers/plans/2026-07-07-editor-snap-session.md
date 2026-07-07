# Editor Anchor Snap + Session Implementation Plan (v2 plan 4 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** (a) osu-style closest-anchor auto-snap while dragging widgets (anchor follows the widget across parent thirds, no-jump offset rewrite, guide lines, per-widget auto/pinned toggle); (b) a dedicated editor session entered from the title screen (F2) that plays the last-played (else random) song on autoplay in a seamless loop with the editor open, Esc returning to the title.

**Architecture:** Snap: `WidgetInstance` gains `anchor_auto: bool` (default true; serialized only when false). A system running right after the drag gesture recomputes the nearest ninth from the widget's visual center within its parent rect; on change it rewrites anchor+origin and recomputes the offset so the resolved position is bit-identical (reuses `offset_for_top_left`). Guide lines at parent thirds render only during a Move gesture. Session: `EditorSession(pub bool)` resource in game-shell (mirrors `PracticeIntent`); title F2 picks a song (config `gameplay.last_played` → validated, else random from `SongDb`) and routes through the normal `SelectedSong` → `SongLoading` flow; in Performance the editor force-opens, the orchestrator's end-of-chart transition is gated off and a `FixedUpdate` watcher seeks back to 0 instead (same mechanism as the practice A/B loop); Esc exits to Title.

**Tech Stack:** Rust, Bevy 0.19, existing seek engine (`SeekToChartTime`), dtx-library `SongDb`.

**Spec:** `docs/superpowers/specs/2026-07-07-layout-editor-v2-design.md` (sections 3, 6 + Input Precedence + Persistence). Reference: `references/osu-lazer/osu.Game/Overlays/SkinEditor/SkinSelectionHandler.cs` (`ApplyClosestAnchorOrigin`), `SkinEditorSceneLibrary.cs` (behavior only).

**Branch:** `feat/editor-snap-session` off `main` (after plan 3 merged).

**Existing context (v1 + plans 1–3):**
- `crates/dtx-layout/src/widgets.rs` — `Anchor9` (+`frac`, `ALL`), `Placement { Natural, Anchored }`, `resolve_top_left`, `offset_for_top_left`.
- `crates/gameplay-drums/src/editor/` — `drag.rs` (`Gesture::{None,Move,Scale}`, `ActiveGesture`, `ensure_anchored`, `EditorGestureSet`), `picking.rs` (`WidgetAabbs`), `selection_box.rs` (`EditorOverlay` cleanup marker, `parent_rect` helper), `panel.rs` (`AnchorCell`, `apply_anchor_cells` rewrites anchor no-jump), `mod.rs` (`EditorOpen`, `PrevAutoplay`, `toggle_editor`, `close_editor_on_exit`).
- `crates/gameplay-drums/src/widget_layout.rs` — `WidgetGeoms`, `transform_point`, `parent_rect_px`.
- `crates/gameplay-drums/src/orchestrator.rs` — the end-of-chart system (contains `past_chart_end` + `request_transition(&mut requests, AppState::StageClear)`) and `DrumsStageCompletion { end_requested, chart_end_ms, .. }`.
- `crates/gameplay-drums/src/stage_end.rs` — `detect_stage_failure` (gauge fail → StageFailed).
- `crates/gameplay-drums/src/seek.rs` — `SeekToChartTime { target_ms, snap, attempt_start_ms }` message; pattern: `crates/gameplay-drums/src/practice/ab_loop.rs::loop_watcher` (FixedUpdate, `.before(crate::seek::apply_seek_system)`, gated `PauseState::Running`).
- `crates/game-shell/src/states.rs` — `PracticeIntent(pub bool)` resource precedent; `lib.rs` `GameShellPlugin` inits it.
- `crates/game-menu/src/title.rs` — `title_input` (Enter → SongSelect, Esc → End), bottom bar with `ESC QUIT` hint.
- `crates/game-menu/src/song_select.rs` — `SelectedSong(pub Option<PathBuf>)`, `ensure_song_db_loaded` (lazy scan via `default_song_dir()` when `SongDb::is_empty`), `dtx_library::{SongDb, SongInfo, default_song_dir}`.
- `crates/game-menu/src/song_loading.rs` — OnEnter(SongLoading) reads `SelectedSong`, eventually `request_transition(AppState::Performance)`.
- `crates/dtx-config/src/lib.rs` — `Config { system, gameplay: GameplayConfig, audio, .. }`, `load(&default_path())`, `save(&path, &cfg)`.
- `crates/gameplay-drums/src/perf_hotkeys.rs` — `PerfHotkeyDraft` reloads config OnEnter(Performance) and flushes on exit — the last_played write must happen BEFORE that reload (OnEnter(SongLoading) is safely earlier).
- rustfmt gotcha: NEVER bare `cargo fmt --all`. 16-plugin tuple limit.

---

## File Structure

- Modify: `crates/dtx-layout/src/widgets.rs` — `anchor_auto` field + `nearest_anchor` pure fn.
- Modify: `crates/dtx-layout/src/scene.rs` — serde for `anchor_auto`.
- Create: `crates/gameplay-drums/src/editor/snap.rs` — auto-snap system + guide lines.
- Modify: `crates/gameplay-drums/src/editor/panel.rs` — "auto" cell on the anchor grid; manual cell pins.
- Modify: `crates/game-shell/src/states.rs` + `lib.rs` — `EditorSession` resource.
- Modify: `crates/game-menu/src/title.rs` — F2 entry + hint.
- Modify: `crates/game-menu/src/song_loading.rs` — persist `last_played`.
- Modify: `crates/dtx-config/src/lib.rs` — `gameplay.last_played`.
- Modify: `crates/gameplay-drums/src/editor/mod.rs` — session force-open, Esc-to-title, toggle gating.
- Create: `crates/gameplay-drums/src/editor/session.rs` — chart-end loop watcher + transition gating helpers.
- Modify: `crates/gameplay-drums/src/orchestrator.rs` + `stage_end.rs` — gate end transitions on session.
- Test: `crates/gameplay-drums/tests/editor_session.rs`.

### Task 0: Branch

- [ ] **Step 0.1:**

```bash
cd /home/lazykern/lab/dtxmaniars && git checkout -b feat/editor-snap-session main
```

### Task 1: dtx-layout — `anchor_auto` + `nearest_anchor`

**Files:**
- Modify: `crates/dtx-layout/src/widgets.rs`, `crates/dtx-layout/src/scene.rs`

- [ ] **Step 1.1: widgets.rs**

`WidgetInstance` gains (after `origin`): `pub anchor_auto: bool,` — and:

```rust
/// Nearest ninth for a fractional position within the parent (thirds rule:
/// <1/3 → start, 1/3..=2/3 → center, >2/3 → end, per axis).
pub fn nearest_anchor(frac_x: f32, frac_y: f32) -> Anchor9 {
    let col = if frac_x < 1.0 / 3.0 {
        0
    } else if frac_x <= 2.0 / 3.0 {
        1
    } else {
        2
    };
    let row = if frac_y < 1.0 / 3.0 {
        0
    } else if frac_y <= 2.0 / 3.0 {
        1
    } else {
        2
    };
    Anchor9::ALL[row * 3 + col]
}
```

Tests:

```rust
#[test]
fn nearest_anchor_nine_regions() {
    assert_eq!(nearest_anchor(0.1, 0.1), Anchor9::TopLeft);
    assert_eq!(nearest_anchor(0.5, 0.1), Anchor9::TopCenter);
    assert_eq!(nearest_anchor(0.9, 0.1), Anchor9::TopRight);
    assert_eq!(nearest_anchor(0.1, 0.5), Anchor9::CenterLeft);
    assert_eq!(nearest_anchor(0.5, 0.5), Anchor9::Center);
    assert_eq!(nearest_anchor(0.9, 0.5), Anchor9::CenterRight);
    assert_eq!(nearest_anchor(0.1, 0.9), Anchor9::BottomLeft);
    assert_eq!(nearest_anchor(0.5, 0.9), Anchor9::BottomCenter);
    assert_eq!(nearest_anchor(0.9, 0.9), Anchor9::BottomRight);
}
```

- [ ] **Step 1.2: scene.rs** — `WidgetEntry` gains:

```rust
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub anchor_auto: bool,
```

with `fn is_true(b: &bool) -> bool { *b }` (module-level; `default_true` already exists). Copy in `to_instance`/`from_instance`; `default_instance` sets `anchor_auto: true`. Fix every `WidgetEntry`/`WidgetInstance` struct literal the compiler flags (scene.rs tests, plan-2 test files).

- [ ] **Step 1.3:** `cargo test -p dtx-layout && cargo build --workspace 2>&1 | tail -3` → PASS/clean (fix flagged literals).

- [ ] **Step 1.4: Commit**

```bash
git add crates/dtx-layout/ crates/gameplay-drums/
git commit -m "feat(dtx-layout): anchor_auto flag + nearest-ninth classifier"
```

### Task 2: editor/snap.rs — auto-snap + guides

**Files:**
- Create: `crates/gameplay-drums/src/editor/snap.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (`pub mod snap;` + plugin)

- [ ] **Step 2.1: Module**

```rust
//! Closest-anchor auto-snap (osu `ApplyClosestAnchorOrigin` behavior): while a
//! widget drag is in progress and the widget has `anchor_auto`, the anchor
//! follows the widget's center across the parent's thirds. Every anchor
//! rewrite recomputes the offset so the resolved position never jumps.

use bevy::prelude::*;
use dtx_layout::{nearest_anchor, Placement, WidgetKind};

use super::drag::{ActiveGesture, Gesture, Selection};
use super::selection_box::EditorOverlay;
use crate::layout::PlayfieldLayout;
use crate::widget_layout::{parent_rect_px, transform_point, WidgetGeoms, WidgetLayouts};

/// Guide line at a parent-space third (spawned once with the overlay).
#[derive(Component)]
pub struct SnapGuide {
    pub vertical: bool,
    /// 1 or 2 (which third).
    pub which: u8,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (apply_anchor_snap, sync_snap_guides)
            .chain()
            .after(super::EditorGestureSet)
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        Update,
        spawn_guides_on_open.run_if(in_state(game_shell::AppState::Performance)),
    );
}

fn spawn_guides_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    existing: Query<Entity, With<SnapGuide>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    for vertical in [true, false] {
        for which in [1u8, 2u8] {
            commands.spawn((
                EditorOverlay,
                SnapGuide { vertical, which },
                Node {
                    position_type: PositionType::Absolute,
                    width: if vertical { Val::Px(1.0) } else { Val::Px(0.0) },
                    height: if vertical { Val::Px(0.0) } else { Val::Px(1.0) },
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                Visibility::Hidden,
                GlobalZIndex(2050),
                Pickable::IGNORE,
            ));
        }
    }
}

/// While dragging with anchor_auto: nearest ninth from the widget's visual
/// center within its parent; on change rewrite anchor+origin and recompute
/// offset (no-jump).
fn apply_anchor_snap(
    gesture: Res<ActiveGesture>,
    selection: Res<Selection>,
    geoms: Res<WidgetGeoms>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut layouts: ResMut<WidgetLayouts>,
) {
    if !matches!(gesture.0, Gesture::Move { .. }) {
        return;
    }
    let Some(kind) = selection.0 else { return };
    if kind == WidgetKind::Playfield {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(g) = geoms.0.get(&kind).copied() else { return };
    let inst_ro = layouts.get(kind).clone();
    if !inst_ro.anchor_auto || inst_ro.placement != Placement::Anchored {
        // Natural widgets convert on gesture start (plan 2); if still Natural
        // here, offset-delta dragging continues un-snapped.
        return;
    }
    let wsize = Vec2::new(window.width(), window.height());
    let sc = wsize / 2.0;
    let vis_min = transform_point(g.unscaled.min, sc, g.applied_translation, g.applied_scale);
    let vis_max = transform_point(g.unscaled.max, sc, g.applied_translation, g.applied_scale);
    let center = (vis_min + vis_max) / 2.0;
    let (px, py, pw, ph) = parent_rect_px(inst_ro.space, wsize, &pfl);
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }
    let frac = ((center - Vec2::new(px, py)) / Vec2::new(pw, ph)).clamp(Vec2::ZERO, Vec2::ONE);
    let want = nearest_anchor(frac.x, frac.y);
    if want == inst_ro.anchor {
        return;
    }
    let Some(inst) = layouts.0.get_mut(&kind) else { return };
    inst.anchor = want;
    inst.origin = want;
    let off_px = dtx_layout::offset_for_top_left(
        want,
        want,
        (g.unscaled.width(), g.unscaled.height()),
        inst.scale,
        (vis_min.x, vis_min.y),
        (px, py, pw, ph),
    );
    inst.offset = (off_px.0 / pfl.scale, off_px.1 / pfl.scale);
}

/// Guides visible only during a Move drag; positioned at the selected
/// widget's parent-space thirds.
fn sync_snap_guides(
    gesture: Res<ActiveGesture>,
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut guides: Query<(&SnapGuide, &mut Node, &mut Visibility)>,
) {
    let dragging = matches!(gesture.0, Gesture::Move { .. });
    let show = dragging
        && selection
            .0
            .map(|k| k != WidgetKind::Playfield && layouts.get(k).anchor_auto)
            .unwrap_or(false);
    if !show {
        for (_, _, mut vis) in &mut guides {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let Some(kind) = selection.0 else { return };
    let Ok(window) = windows.single() else { return };
    let wsize = Vec2::new(window.width(), window.height());
    let (px, py, pw, ph) = parent_rect_px(layouts.get(kind).space, wsize, &pfl);
    for (guide, mut node, mut vis) in &mut guides {
        let t = guide.which as f32 / 3.0;
        if guide.vertical {
            node.left = Val::Px(px + pw * t);
            node.top = Val::Px(py);
            node.height = Val::Px(ph);
            node.width = Val::Px(1.0);
        } else {
            node.left = Val::Px(px);
            node.top = Val::Px(py + ph * t);
            node.width = Val::Px(pw);
            node.height = Val::Px(1.0);
        }
        *vis = Visibility::Visible;
    }
}
```

(`EditorOverlay` must be exported from selection_box.rs — it is `pub`? Make it `pub` if plan 1 left it private. `parent_rect_px`, `transform_point`, `WidgetGeoms` come from plan 2's widget_layout.)

- [ ] **Step 2.2: Register** — `pub mod snap;` in mod.rs, `snap::plugin` into a tuple with room.

- [ ] **Step 2.3: Panel auto-cell + pinning** (`panel.rs`):

In the anchor-grid spawn (plan 2 Step 5.2), append after the 3×3 rows one extra row with an "auto" cell:

```rust
#[derive(Component)]
pub struct AnchorAutoCell;
```

```rust
            grid.spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    AnchorAutoCell,
                    Button,
                    Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
                    BackgroundColor(if inst.anchor_auto {
                        t.accent
                    } else {
                        Color::srgb(0.14, 0.14, 0.18)
                    }),
                    children![(
                        Text::new("auto"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
```

Handler (register with the other panel systems):

```rust
fn handle_anchor_auto_cell(
    cells: Query<&Interaction, (With<AnchorAutoCell>, Changed<Interaction>)>,
    selection: Res<Selection>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    mut cell_bg: Query<&mut BackgroundColor, With<AnchorAutoCell>>,
    theme: Res<dtx_ui::ThemeResource>,
) {
    for interaction in &cells {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(kind) = selection.0 else { continue };
        undo.push(&layouts, &lanes);
        let Some(inst) = layouts.0.get_mut(&kind) else { continue };
        inst.anchor_auto = !inst.anchor_auto;
        for mut bg in &mut cell_bg {
            bg.0 = if inst.anchor_auto {
                theme.0.accent
            } else {
                Color::srgb(0.14, 0.14, 0.18)
            };
        }
    }
}
```

And in `apply_anchor_cells` (manual 3×3 click), after `inst.origin = new_anchor;` add `inst.anchor_auto = false;` (manual choice pins, osu parity).

- [ ] **Step 2.4:** `cargo test -p gameplay-drums editor` → PASS. Add a snap unit test to snap.rs:

```rust
#[cfg(test)]
mod tests {
    use dtx_layout::{nearest_anchor, Anchor9};

    #[test]
    fn snap_rewrite_is_no_jump() {
        // Anchor rewrite + offset_for_top_left keeps resolve_top_left fixed.
        let parent = (0.0, 0.0, 1280.0, 720.0);
        let size = (150.0, 60.0);
        let visual = (900.0, 600.0); // bottom-right-ish → BottomRight anchor
        let frac = (
            (visual.0 + size.0 / 2.0) / 1280.0,
            (visual.1 + size.1 / 2.0) / 720.0,
        );
        let a = nearest_anchor(frac.0, frac.1);
        assert_eq!(a, Anchor9::BottomRight);
        let off = dtx_layout::offset_for_top_left(a, a, size, 1.0, visual, parent);
        let tl = dtx_layout::resolve_top_left(a, a, size, 1.0, off, parent);
        assert!((tl.0 - visual.0).abs() < 0.001 && (tl.1 - visual.1).abs() < 0.001);
    }
}
```

- [ ] **Step 2.5: Commit**

```bash
git add crates/gameplay-drums/src/editor/
git commit -m "feat(editor): closest-anchor auto-snap with guides + auto/pin cell"
```

### Task 3: EditorSession resource + config `last_played`

**Files:**
- Modify: `crates/game-shell/src/states.rs`, `crates/game-shell/src/lib.rs`
- Modify: `crates/dtx-config/src/lib.rs`

- [ ] **Step 3.1: game-shell** — in states.rs next to `PracticeIntent`:

```rust
/// True while the layout-editor session (title → F2) is active: Performance
/// runs on autoplay in a seamless loop with the editor open; Esc exits to
/// Title instead of Results.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorSession(pub bool);
```

Export from lib.rs `pub use states::{...}` list; `GameShellPlugin::build` adds `.init_resource::<states::EditorSession>()`.

- [ ] **Step 3.2: dtx-config** — `GameplayConfig` gains:

```rust
    /// Path of the last song entered in normal play (editor session uses it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<std::path::PathBuf>,
```

Fix the `Default for GameplayConfig` impl (add `last_played: None`). Add a round-trip test next to the existing config tests:

```rust
#[test]
fn last_played_round_trips_and_defaults_none() {
    let mut cfg = Config::default();
    assert_eq!(cfg.gameplay.last_played, None);
    cfg.gameplay.last_played = Some(std::path::PathBuf::from("/tmp/x.dtx"));
    let s = toml::to_string_pretty(&cfg).unwrap();
    let back: Config = toml::from_str(&s).unwrap();
    assert_eq!(back.gameplay.last_played, cfg.gameplay.last_played);
}
```

(Match the existing test style/serde helpers in dtx-config; if `Config` serializing requires the full struct there are existing round-trip tests to crib from.)

- [ ] **Step 3.3:** `cargo test -p dtx-config -p game-shell` → PASS. Commit:

```bash
git add crates/game-shell/ crates/dtx-config/
git commit -m "feat(shell,config): EditorSession resource + gameplay.last_played"
```

### Task 4: Title F2 entry + last_played persistence

**Files:**
- Modify: `crates/game-menu/src/title.rs`
- Modify: `crates/game-menu/src/song_loading.rs`

- [ ] **Step 4.1: title.rs** — add the hint to the bottom bar (between version and ESC QUIT, the bar uses `JustifyContent::SpaceBetween` — a middle child sits centered):

```rust
                    bar.spawn((
                        Text::new("F2 LAYOUT EDITOR"),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
```

Extend `title_input`:

```rust
fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut session: ResMut<game_shell::EditorSession>,
    mut selected: ResMut<crate::song_select::SelectedSong>,
    mut db: ResMut<dtx_library::SongDb>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    } else if keys.just_pressed(KeyCode::F2) {
        match pick_editor_song(&mut db) {
            Some(path) => {
                session.0 = true;
                selected.0 = Some(path);
                request_transition(&mut requests, AppState::SongLoading);
            }
            None => warn!("layout editor: no songs available (empty SongDb)"),
        }
    } else if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::End);
    }
}

/// Song for the editor session: config `last_played` when it still exists,
/// else a random SongDb entry (lazy-scanning the default dir like song
/// select does).
fn pick_editor_song(db: &mut dtx_library::SongDb) -> Option<std::path::PathBuf> {
    let cfg = dtx_config::load(&dtx_config::default_path());
    if let Some(last) = cfg.gameplay.last_played {
        if last.exists() {
            return Some(last);
        }
    }
    if db.is_empty() {
        let dir = dtx_library::default_song_dir();
        if let Err(e) = db.rescan(&dir) {
            warn!("layout editor: song scan failed: {e}");
        }
    }
    if db.is_empty() {
        return None;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    db.get(nanos % db.len()).map(|s| s.path.clone())
}
```

(Check what `song_select.rs` actually calls for the default dir — `default_song_dir()` in that file delegates to `dtx_library::default_song_dir()`; `dtx_config` must be a game-menu dependency — check Cargo.toml, add if missing, matching workspace dependency style. If `SelectedSong`/`AppState::SongLoading` naming differs, mirror how song_select launches a song — find its `request_transition(..., AppState::SongLoading)` call site and copy the exact preparation.)

- [ ] **Step 4.2: song_loading.rs** — persist last_played for NORMAL runs only, on entering SongLoading (before `PerfHotkeyDraft` reloads config at Performance):

```rust
/// Remember the song for the editor session (`gameplay.last_played`).
/// Normal runs only — the editor session must not overwrite it with itself.
fn persist_last_played(
    selected: Res<SelectedSong>,
    session: Res<game_shell::EditorSession>,
) {
    if session.0 {
        return;
    }
    let Some(path) = selected.0.clone() else { return };
    let cfg_path = dtx_config::default_path();
    let mut cfg = dtx_config::load(&cfg_path);
    if cfg.gameplay.last_played.as_ref() == Some(&path) {
        return;
    }
    cfg.gameplay.last_played = Some(path);
    if let Err(e) = dtx_config::save(&cfg_path, &cfg) {
        warn!("failed to persist last_played: {e}");
    }
}
```

Register in song_loading's plugin: `.add_systems(OnEnter(AppState::SongLoading), persist_last_played)` (alongside its existing OnEnter systems).

- [ ] **Step 4.3:** `cargo test -p game-menu 2>&1 | tail -3` → PASS. Commit:

```bash
git add crates/game-menu/
git commit -m "feat(title): F2 layout-editor session entry + last_played persistence"
```

### Task 5: Session behavior in gameplay-drums

**Files:**
- Create: `crates/gameplay-drums/src/editor/session.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`, `crates/gameplay-drums/src/editor/ui.rs`
- Modify: `crates/gameplay-drums/src/orchestrator.rs`, `crates/gameplay-drums/src/stage_end.rs`

- [ ] **Step 5.1: session.rs**

```rust
//! Editor-session runtime: force-open on Performance enter, seamless
//! chart-end loop (seek back to 0 instead of Results), exit-to-title.

use bevy::prelude::*;
use game_shell::{AppState, EditorSession, PauseState};

use crate::orchestrator::DrumsStageCompletion;
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), force_open_for_session)
        .add_systems(
            FixedUpdate,
            session_loop_watcher
                .before(crate::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(session_active),
        );
}

pub fn session_active(session: Res<EditorSession>) -> bool {
    session.0
}

/// Entering Performance in a session: editor opens immediately, autoplay on.
fn force_open_for_session(
    session: Res<EditorSession>,
    mut open: ResMut<super::EditorOpen>,
    mut prev: ResMut<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if !session.0 {
        return;
    }
    prev.0 = autoplay.0;
    autoplay.0 = true;
    open.0 = true;
}

/// Past chart end → seek to 0 (same mechanism as the practice A/B loop);
/// the orchestrator's StageClear transition is gated off during a session.
fn session_loop_watcher(
    clock: Res<GameplayClock>,
    completion: Res<DrumsStageCompletion>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    if !clock.is_ready() || completion.chart_end_ms <= 0 {
        return;
    }
    if clock.current_ms >= completion.chart_end_ms {
        seeks.write(SeekToChartTime {
            target_ms: 0,
            snap: None,
            attempt_start_ms: Some(0),
        });
    }
}
```

(Verify `DrumsStageCompletion` field visibility (`chart_end_ms`) — make `pub` if needed. `GameplayClock`/`current_ms`/`is_ready` usage mirrors ab_loop.rs.)

- [ ] **Step 5.2: Gate the end-of-chart + failure transitions**

In `orchestrator.rs`, the system containing `past_chart_end` (see the `request_transition(&mut requests, AppState::StageClear)` line ~435): add a parameter `session: Res<game_shell::EditorSession>` and, right after the `end_requested` early-return, add:

```rust
    if session.0 {
        return; // editor session loops via session_loop_watcher instead
    }
```

In `stage_end.rs::detect_stage_failure`: same `session: Res<game_shell::EditorSession>` param + early return `if session.0 { return; }` (autoplay never fails, but a mid-song editor toggle plus practice-free gauge could).

- [ ] **Step 5.3: Exit to title + toggle gating**

`ui.rs::close_on_escape` (plan 1 gave it deselect-first) — the close branch becomes session-aware:

```rust
        } else {
            open.0 = false;
            autoplay.0 = prev.0;
            if session.0 {
                session.0 = false;
                request_transition(&mut requests, AppState::Title);
            }
        }
```

(add `mut session: ResMut<game_shell::EditorSession>`, `mut requests: MessageWriter<game_shell::TransitionRequest>`, import `game_shell::{request_transition, AppState}`.)

The sidebar `EditorButton::Close` arm in `ui.rs::handle_buttons` gets the same session branch.

`mod.rs::toggle_editor`: sessions keep the editor open — gate the toggle:

```rust
        .add_systems(
            Update,
            toggle_editor
                .run_if(in_state(AppState::Performance))
                .run_if(|s: Res<game_shell::EditorSession>| !s.0),
        )
```

`mod.rs::close_editor_on_exit` additionally clears the session (covers non-Esc exits):

```rust
    mut session: ResMut<game_shell::EditorSession>,
    // ...in body:
    session.0 = false;
```

- [ ] **Step 5.4: Register** — `pub mod session;` in mod.rs + `session::plugin` in a tuple with room.

- [ ] **Step 5.5:** `cargo test -p gameplay-drums 2>&1 | tail -5` → PASS. Commit:

```bash
git add crates/gameplay-drums/src/
git commit -m "feat(editor): session runtime — force-open, seamless loop, exit to title"
```

### Task 6: Integration tests

**Files:**
- Create: `crates/gameplay-drums/tests/editor_session.rs`

- [ ] **Step 6.1: Tests**

```rust
//! Editor session + snap invariants.

use dtx_layout::{nearest_anchor, Anchor9, Placement, WidgetKind};

#[test]
fn anchor_auto_default_true_and_survives_file_round_trip() {
    let mut map = dtx_layout::SceneSection::default().resolve();
    assert!(map[&WidgetKind::Combo].anchor_auto);
    let c = map.get_mut(&WidgetKind::Combo).unwrap();
    c.placement = Placement::Anchored;
    c.anchor = Anchor9::BottomRight;
    c.origin = Anchor9::BottomRight;
    c.anchor_auto = false;
    c.offset = (5.0, 5.0);
    let section = dtx_layout::SceneSection::from_map(&map);
    let back = section.resolve();
    assert!(!back[&WidgetKind::Combo].anchor_auto);
}

#[test]
fn drag_path_across_thirds_walks_anchors_without_jumps() {
    // Simulate a widget center sweeping left→right at mid height; anchors
    // must walk Left→Center→Right and every rewrite must be position-exact.
    let parent = (0.0, 0.0, 1280.0, 720.0);
    let size = (100.0, 40.0);
    let mut anchor = Anchor9::CenterLeft;
    let mut offset = dtx_layout::offset_for_top_left(
        anchor, anchor, size, 1.0, (50.0, 340.0), parent,
    );
    let mut seen = vec![anchor];
    for x in (50..1150).step_by(50) {
        let visual = (x as f32, 340.0);
        let frac_x = (visual.0 + size.0 / 2.0) / 1280.0;
        let frac_y = (visual.1 + size.1 / 2.0) / 720.0;
        let want = nearest_anchor(frac_x, frac_y);
        if want != anchor {
            // No-jump rewrite: recompute offset at the same visual position.
            offset = dtx_layout::offset_for_top_left(want, want, size, 1.0, visual, parent);
            anchor = want;
            seen.push(anchor);
        }
        let tl = dtx_layout::resolve_top_left(anchor, anchor, size, 1.0, offset, parent);
        assert!((tl.0 - visual.0).abs() < 0.001, "jump at x={x}");
        // (offset is only exact at rewrite points; between them the caller
        // adds drag deltas — emulate that:)
        offset = dtx_layout::offset_for_top_left(
            anchor, anchor, size, 1.0, (visual.0 + 50.0, visual.1), parent,
        );
    }
    assert_eq!(
        seen,
        vec![Anchor9::CenterLeft, Anchor9::Center, Anchor9::CenterRight]
    );
}

#[test]
fn session_resource_defaults_off() {
    assert!(!game_shell::EditorSession::default().0);
}
```

(game_shell must be a dev-dependency of gameplay-drums — it's already a regular dependency.)

- [ ] **Step 6.2:** `cargo test -p gameplay-drums --test editor_session` → 3 PASS.

- [ ] **Step 6.3: Commit**

```bash
git add crates/gameplay-drums/tests/editor_session.rs
git commit -m "test(editor): snap walk + session defaults"
```

### Task 7: Real-binary verification

- [ ] **Step 7.1:** `cargo test --workspace 2>&1 | tail -8` → all PASS.
- [ ] **Step 7.2:** `timeout 40 cargo run 2>&1 | tail -20; echo "exit=$?"` → `exit=124`, no panic/cycle. The FixedUpdate watcher is NEW FixedUpdate wiring — this launch check is mandatory (memory gotcha: hand-wired test apps don't prove the real schedule builds).
- [ ] **Step 7.3:** Report DONE + manual checklist:
  - Title shows `F2 LAYOUT EDITOR`; F2 jumps into a looping autoplay song with the editor open.
  - Song reaching its end restarts seamlessly (no results screen).
  - Esc (nothing selected) returns to the title; next normal play is unaffected.
  - Dragging a widget across screen thirds: anchor dot + red line hop between ninths, widget never jumps; guide lines show during the drag.
  - Anchor grid "auto" cell highlights; clicking a specific cell pins (auto off).
  - Play a song normally, quit; F2 session uses that song.
