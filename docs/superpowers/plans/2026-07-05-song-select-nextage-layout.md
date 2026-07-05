# Song Select — GITADORA NEXT-AGE Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rearrange the song-select screen to the GITADORA NEXT-AGE reference — skill/BPM on the far left, jacket top-center, density graph bottom-left, a two-box difficulty ladder in the center, and full-fidelity wheel rows (thumbnail + skill number + progress bar + title).

**Architecture:** Pure Bevy UI spawn-tree changes in `game-menu` + the shared `dtx-ui` widgets. No new systems for logic; existing update systems (`update_left_cluster`, `render_difficulty_grid`, `wheel_layout_system`, `respawn_wheel_on_change`) keep their marker-driven contracts. New per-row data (thumbnail/skill/achievement) is computed at spawn time from `SongDb` + `score.ini`, exactly like the existing left cluster.

**Tech Stack:** Rust, Bevy 0.19 UI, `bevy_kira_audio`, workspace crates `dtx-ui` / `dtx-library` / `dtx-scoring` / `game-menu`.

**Spec:** `docs/superpowers/specs/2026-07-05-song-select-nextage-layout-design.md`

**Reference screenshot:** GITADORA NEXT-AGE (image supplied in the request).

---

## File Structure

- `crates/game-menu/src/chart_stats.rs` — add two pure formatting helpers (`row_skill_text`, `bar_fill_pct`). Keeps format logic testable and out of the spawn tree.
- `crates/dtx-ui/src/widget/difficulty_grid.rs` — rewrite `spawn_difficulty_grid` slot markup into the two-box (completion | level) form. Markers and format helpers unchanged.
- `crates/game-menu/src/song_select.rs` — wheel-row content (thumbnail/skill/bar/title), plumb `SongDb` + `AssetServer` into `spawn_song_select`/`spawn_wheel_rows`/`respawn_wheel_on_change`, reposition the four clusters, add artist-under-jacket text + its update.

## Conventions (match existing code)

- No `unwrap()` in `crates/*`.
- One commit per task (Conventional Commits, no AI trailers).
- Test commands: `cargo test -p dtx-ui` and `cargo test -p game-menu`.
- Build check: `cargo check -p game-menu`.
- Run: `cargo run -p dtxmaniars-desktop`.

---

### Task 1: Pure row-formatting helpers

**Files:**
- Modify: `crates/game-menu/src/chart_stats.rs`
- Test: `crates/game-menu/src/chart_stats.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` block in `chart_stats.rs`:

```rust
    #[test]
    fn row_skill_text_two_decimals() {
        assert_eq!(row_skill_text(79.17), "79.17");
        assert_eq!(row_skill_text(0.0), "0.00");
        assert_eq!(row_skill_text(2.9), "2.90");
    }

    #[test]
    fn bar_fill_pct_clamps_0_to_100() {
        assert_eq!(bar_fill_pct(64.8), 64.8);
        assert_eq!(bar_fill_pct(0.0), 0.0);
        assert_eq!(bar_fill_pct(100.0), 100.0);
        assert_eq!(bar_fill_pct(-5.0), 0.0);
        assert_eq!(bar_fill_pct(123.4), 100.0);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p game-menu row_skill_text_two_decimals bar_fill_pct_clamps_0_to_100`
Expected: FAIL — `cannot find function row_skill_text` / `bar_fill_pct`.

- [ ] **Step 3: Implement the helpers**

Add near the top of `chart_stats.rs` (after imports, before `skill_points`):

```rust
/// Wheel-row skill number, always two decimals (e.g. "79.17").
pub fn row_skill_text(skill: f32) -> String {
    format!("{skill:.2}")
}

/// Achievement percent clamped to a 0..=100 progress-bar fill.
pub fn bar_fill_pct(achievement: f32) -> f32 {
    achievement.clamp(0.0, 100.0)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p game-menu row_skill_text_two_decimals bar_fill_pct_clamps_0_to_100`
Expected: PASS (2 passed).

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/chart_stats.rs
git commit -m "feat(song-select): add wheel-row skill/progress format helpers"
```

---

### Task 2: Two-box difficulty slot markup

Rewrite each difficulty slot from `Column[colorbar, row[score, level]]` into
`Row[ completion-box , level-box ]`, matching the reference where the dark
"COMPLETION RATE" box sits left of the colored level box.

**Files:**
- Modify: `crates/dtx-ui/src/widget/difficulty_grid.rs` — `spawn_difficulty_grid` body.
- Test: `crates/dtx-ui/src/widget/difficulty_grid.rs` (inline tests).

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `difficulty_grid.rs`:

```rust
    #[test]
    fn grid_spawns_five_of_each_marker() {
        use bevy::prelude::*;
        let mut app = App::new();
        let theme = Theme::default();
        app.world_mut()
            .spawn(Node::default())
            .with_children(|p| spawn_difficulty_grid(p, &theme));
        app.update();
        let count = |app: &mut App, f: &dyn Fn(&World) -> usize| f(app.world());
        let labels = count(&mut app, &|w| {
            w.iter_entities()
                .filter(|e| e.contains::<DifficultySlotLabel>())
                .count()
        });
        let levels = app
            .world()
            .iter_entities()
            .filter(|e| e.contains::<DifficultySlotLevel>())
            .count();
        let scores = app
            .world()
            .iter_entities()
            .filter(|e| e.contains::<DifficultySlotScore>())
            .count();
        let panels = app
            .world()
            .iter_entities()
            .filter(|e| e.contains::<DifficultySlotPanel>())
            .count();
        assert_eq!(labels, GRID_MAX_SLOTS);
        assert_eq!(levels, GRID_MAX_SLOTS);
        assert_eq!(scores, GRID_MAX_SLOTS);
        assert_eq!(panels, GRID_MAX_SLOTS);
    }
```

- [ ] **Step 2: Run test to verify it passes against current code (baseline)**

Run: `cargo test -p dtx-ui grid_spawns_five_of_each_marker`
Expected: PASS. (This is a guard test — it must keep passing after the rewrite; the marker counts are the contract the update systems rely on.)

- [ ] **Step 3: Rewrite the slot markup**

Replace the entire body of `spawn_difficulty_grid` (the `for i in (0..GRID_MAX_SLOTS).rev()` loop) with:

```rust
pub fn spawn_difficulty_grid(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    for i in (0..GRID_MAX_SLOTS).rev() {
        // MASTER on top like GITADORA (highest index first).
        parent
            .spawn((
                DifficultySlotPanel(i),
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.stage_panel_bg),
                BorderColor::all(theme.stage_panel_border),
                BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
            ))
            .with_children(|slot| {
                // Left box: completion rate.
                slot.spawn((
                    Node {
                        width: Val::Px(110.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(theme.stage_panel_bg),
                ))
                .with_children(|box_l| {
                    box_l.spawn((
                        Text::new("COMPLETION RATE"),
                        Theme::font(9.0),
                        TextColor(theme.text_secondary),
                    ));
                    box_l.spawn((
                        DifficultySlotScore(i),
                        Text::new(""),
                        Theme::font(13.0),
                        TextColor(theme.text_primary),
                    ));
                });
                // Right box: colored tier bar on top, big level number below.
                slot.spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|box_r| {
                    box_r.spawn((
                        DifficultySlotLabel(i),
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(theme.difficulty_color(i as u8)),
                        Text::new(""),
                        Theme::font(11.0),
                        TextColor(theme.text_primary),
                    ));
                    box_r
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::FlexEnd,
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            ..default()
                        })
                        .with_children(|num| {
                            num.spawn((
                                DifficultySlotLevel(i),
                                Text::new("--"),
                                Theme::font(28.0),
                                TextColor(theme.text_primary),
                            ));
                        });
                });
            });
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-ui`
Expected: PASS — `grid_spawns_five_of_each_marker`, `level_text_*`, `score_text_*` all green.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui/src/widget/difficulty_grid.rs
git commit -m "feat(dtx-ui): split difficulty slot into completion + level boxes"
```

---

### Task 3: Full-fidelity wheel rows

Give each wheel row a jacket thumbnail, a yellow skill number, a yellow
progress bar (best-achievement fill), and the title. Drop the artist line
from rows (it moves under the big jacket in Task 4).

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`
- Test: `crates/game-menu/src/song_select.rs` (inline tests)

- [ ] **Step 1: Add row-stat helper + failing test**

Add this helper function above `spawn_wheel_rows` in `song_select.rs`:

```rust
/// Representative chart for a folder's wheel row: the highest-dlevel
/// chart present (falls back to the first). Returns the `db.songs`
/// index, or `None` for an empty folder.
fn folder_display_chart(folder: &SongFolderView, db: &SongDb) -> Option<usize> {
    folder
        .chart_indices
        .iter()
        .copied()
        .filter(|idx| db.songs.get(*idx).is_some())
        .max_by_key(|idx| db.songs[*idx].dlevel.unwrap_or(0))
        .or_else(|| folder.chart_indices.first().copied())
}
```

Add to the `mod tests` block. Reuse the existing `make_song(title, artist)`
helper already in this file (it builds a full `SongInfo` with all fields),
then mutate `dlevel` — neither `SongInfo` nor `SongFolderView` derives
`Default`, so construct `SongFolderView` fully (it has `folder`, `title`,
`artist`, `chart_indices`):

```rust
    #[test]
    fn folder_display_chart_picks_highest_dlevel() {
        let mut db = SongDb::default();
        let mut a = make_song("a", "");
        a.dlevel = Some(30);
        let mut b = make_song("b", "");
        b.dlevel = Some(90);
        let mut c = make_song("c", "");
        c.dlevel = Some(50);
        db.songs.push(a);
        db.songs.push(b);
        db.songs.push(c);
        let folder = SongFolderView {
            folder: std::path::PathBuf::from("/x"),
            title: "t".into(),
            artist: "x".into(),
            chart_indices: vec![0, 1, 2],
        };
        assert_eq!(folder_display_chart(&folder, &db), Some(1));
    }

    #[test]
    fn folder_display_chart_empty_is_none() {
        let db = SongDb::default();
        let folder = SongFolderView {
            folder: std::path::PathBuf::from("/x"),
            title: "t".into(),
            artist: "x".into(),
            chart_indices: vec![],
        };
        assert_eq!(folder_display_chart(&folder, &db), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p game-menu folder_display_chart`
Expected: FAIL — `cannot find function folder_display_chart` (or missing builder).

- [ ] **Step 3: Verify helper tests pass**

Run: `cargo test -p game-menu folder_display_chart`
Expected: PASS (2 passed).

- [ ] **Step 4: Add row marker components**

Near the existing `WheelRowTitle` / `WheelRowMeta` marker declarations
(around line 299-303), replace `WheelRowMeta` usage with new markers:

```rust
/// Wheel row title text, tagged for per-frame updates.
#[derive(Component)]
struct WheelRowTitle;
/// Wheel row jacket thumbnail image.
#[derive(Component)]
struct WheelRowJacket;
/// Wheel row skill number text (yellow).
#[derive(Component)]
struct WheelRowSkill;
/// Wheel row progress-bar fill node (width driven at spawn).
#[derive(Component)]
struct WheelRowBar;
```

(Delete the `WheelRowMeta` struct declaration; it is no longer used.)

- [ ] **Step 5: Rewrite `spawn_wheel_rows` signature + row content**

Change the signature to take `db` and `assets`:

```rust
fn spawn_wheel_rows(
    wheel: &mut ChildSpawnerCommands,
    selection_state: &SongSelectSelection,
    db: &SongDb,
    assets: &AssetServer,
    t: &Theme,
) {
```

Keep the empty-list branch as-is. Replace the per-row `.with_children`
body (the block that currently spawns the title/artist column) with:

```rust
            .with_children(|row| {
                // Jacket thumbnail (or tinted placeholder).
                let display = folder_display_chart(folder, db).and_then(|i| db.songs.get(i));
                let jacket_image = display
                    .and_then(|s| s.preimage_path.as_ref())
                    .map(|p| assets.load(p.to_string_lossy().to_string()))
                    .unwrap_or_default();
                row.spawn((
                    WheelRowJacket,
                    Node {
                        width: Val::Px(58.0),
                        height: Val::Px(58.0),
                        ..default()
                    },
                    BackgroundColor(t.stage_panel_border),
                    ImageNode {
                        image: jacket_image,
                        ..default()
                    },
                ));
                // Right column: skill+bar row, then title.
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col| {
                    let (skill, ach) = display
                        .map(|s| {
                            let ini = dtx_scoring::score_ini::score_ini_path(&s.path);
                            let acc = dtx_scoring::score_ini::read_best(&ini)
                                .map(|b| b.accuracy())
                                .unwrap_or(0.0);
                            (crate::chart_stats::skill_points(s.dlevel, acc), acc)
                        })
                        .unwrap_or((0.0, 0.0));
                    // Skill number + progress bar on one line.
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            WheelRowSkill,
                            Text::new(crate::chart_stats::row_skill_text(skill)),
                            Theme::font(15.0),
                            TextColor(t.select_yellow),
                        ));
                        // Progress-bar track.
                        line.spawn((
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(6.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            BorderColor::all(t.stage_panel_border),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                WheelRowBar,
                                Node {
                                    width: Val::Percent(crate::chart_stats::bar_fill_pct(ach)),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(t.select_yellow),
                            ));
                        });
                    });
                    col.spawn((
                        WheelRowTitle,
                        Text::new(folder.title.clone()),
                        Theme::font(18.0),
                        TextColor(t.text_primary),
                    ));
                });
            });
```

- [ ] **Step 6: Plumb `db` + `assets` into the callers**

`spawn_song_select` (line ~452): add params and pass through.

```rust
fn spawn_song_select(
    mut commands: Commands,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    assets: Res<AssetServer>,
    theme: Res<ThemeResource>,
) {
```

At the wheel-rows call site (line ~631):

```rust
                spawn_wheel_rows(wheel, &selection_state, &db, &assets, &t);
```

`respawn_wheel_on_change` (line ~938): add `db: Res<SongDb>` and
`assets: Res<AssetServer>` params, and update the call:

```rust
fn respawn_wheel_on_change(
    mut commands: Commands,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    assets: Res<AssetServer>,
    theme: Res<ThemeResource>,
    wheel: Query<Entity, With<SongWheel>>,
    rows: Query<Entity, With<WheelRow>>,
) {
```

```rust
    commands.entity(wheel_entity).with_children(|w| {
        spawn_wheel_rows(w, &selection_state, &db, &assets, &t);
    });
```

- [ ] **Step 7: Build to verify wiring compiles**

Run: `cargo check -p game-menu`
Expected: no errors. Fix any `WheelRowMeta` leftovers (the system that
queried it, if any) — remove references; the artist no longer lives on rows.

- [ ] **Step 8: Run the full crate test suite**

Run: `cargo test -p game-menu`
Expected: PASS (all existing + new tests).

- [ ] **Step 9: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(song-select): full-fidelity wheel rows with jacket, skill, progress bar"
```

---

### Task 4: Reposition clusters + artist under jacket

Move the four clusters to the reference positions and add the artist line
under the big jacket.

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`

- [ ] **Step 1: Add artist marker + failing compile anchor**

Add a marker near `AlbumArtEntity` (line ~358):

```rust
/// Artist text shown directly under the big jacket.
#[derive(Component)]
struct SelectedArtistText;
```

- [ ] **Step 2: Rewrite the left cluster + jacket + center blocks**

Replace the whole `// ---- left column: art + skill/bpm` block and the
`// ---- center column: density graph + difficulty grid` block (lines
~525-615) with three absolute clusters:

```rust
            // ---- far-left column: skill + bpm, then density graph
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(72.0),
                    width: Val::Px(200.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 30.0, 220.0),
            ))
            .with_children(|left| {
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "SKILL BY SONG", "0.00", true);
                });
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "BPM", "---", false);
                });
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                ))
                .with_children(|p| spawn_density_graph(p, &t));
            });

            // ---- top-center: big jacket + artist
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(240.0),
                    top: Val::Px(72.0),
                    width: Val::Px(360.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 45.0, 220.0),
            ))
            .with_children(|mid| {
                mid.spawn((
                    BigAlbumArt,
                    AlbumArt::default(),
                    AlbumArtEntity,
                    panel(
                        &t,
                        Node {
                            width: Val::Px(360.0),
                            height: Val::Px(270.0),
                            ..default()
                        },
                    ),
                    ImageNode {
                        color: Color::WHITE.with_alpha(0.0),
                        ..default()
                    },
                ));
                mid.spawn((
                    SelectedArtistText,
                    Text::new(""),
                    Theme::font(14.0),
                    TextColor(t.text_secondary),
                ));
            });

            // ---- center-bottom: difficulty ladder
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(240.0),
                    top: Val::Px(372.0),
                    width: Val::Px(360.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 60.0, 220.0),
            ))
            .with_children(|p| spawn_difficulty_grid(p, &t));
```

- [ ] **Step 3: Feed the artist text**

In `update_left_cluster` (line ~818), add a query param and write the
selected folder's artist. Add to the signature:

```rust
    mut artist_text: Query<
        &mut Text,
        (
            With<SelectedArtistText>,
            Without<BadgeValueText>,
            Without<SortChipText>,
        ),
    >,
```

And before the function returns (after the sort-chip loop), add:

```rust
    let artist = selection_state
        .visible
        .get(selection.folder)
        .map(|f| f.artist.clone())
        .unwrap_or_default();
    for mut text in &mut artist_text {
        *text = Text::new(artist.clone());
    }
```

> If the existing `sort_chip` query already excludes `BadgeValueText`,
> mirror that exclusion set so the three `Query<&mut Text, ...>` params
> stay disjoint (Bevy rejects overlapping mutable text queries).

- [ ] **Step 4: Build**

Run: `cargo check -p game-menu`
Expected: no errors. Resolve any query-conflict panic hints by widening
the `Without<...>` filters so all `&mut Text` queries are disjoint.

- [ ] **Step 5: Run tests**

Run: `cargo test -p game-menu`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(song-select): reposition clusters to NEXT-AGE layout, artist under jacket"
```

---

### Task 5: Full build, lint, and visual verification

**Files:** none (verification only).

- [ ] **Step 1: Workspace check + fmt + clippy**

Run:
```bash
cargo fmt --all
cargo check --workspace
cargo clippy -p game-menu -p dtx-ui -- -D warnings
```
Expected: clean. Fix warnings before proceeding.

- [ ] **Step 2: Full test run**

Run: `cargo test -p dtx-ui -p game-menu`
Expected: all PASS.

- [ ] **Step 3: Launch and screenshot via BRP**

Launch the app (`cargo run -p dtxmaniars-desktop` or the BRP launch tool),
navigate to Song Select, and capture a screenshot. Compare against the
reference: skill/BPM far left, jacket top-center with artist below,
density graph bottom-left, two-box difficulty ladder center, full-fidelity
wheel rows on the right.

- [ ] **Step 4: Tune constants if needed**

If clusters overlap or misalign vs the reference, adjust the absolute
`left`/`top`/`width` values in Task 4 and the jacket/thumbnail sizes.
Re-screenshot. Commit any tuning:

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "style(song-select): tune NEXT-AGE cluster positions"
```

- [ ] **Step 5: Final commit (fmt) if fmt changed anything**

```bash
git add -A
git commit -m "style: cargo fmt"
```

---

## Self-Review Notes

- **Spec §1 left cluster** → Task 4 (skill/bpm/density left, jacket top-center, artist below).
- **Spec §2 difficulty ladder** → Task 2 (two-box slot, markers preserved, selection border on outer panel via existing `render_difficulty_grid`).
- **Spec §3 wheel rows** → Task 3 (thumbnail, skill number, progress bar, title; artist dropped).
- **Spec §4 unchanged** → header/search/sort/hint/BGM/input untouched (not modified in any task).
- **Error handling** → missing preimage → placeholder bg (`unwrap_or_default` handle); missing score → `0.00` skill + empty bar + `— no play`; absent slot → dim via existing `present` flag.
- **Marker contract** → `DifficultySlotLabel/Level/Score/Panel` counts asserted in Task 2; `render_difficulty_grid`/`update_left_cluster` unchanged and keep working.
- **Type consistency** → helper names `row_skill_text`, `bar_fill_pct`, `folder_display_chart` used consistently across Tasks 1/3; new markers `WheelRowJacket/Skill/Bar`, `SelectedArtistText` declared before use.
