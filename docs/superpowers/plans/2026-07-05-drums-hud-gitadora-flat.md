# Drums HUD — GITADORA Flat Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reskin the drums performance HUD into a GITADORA flat-lane layout (centered strip, symmetric panels, hollow secondary chips, GITADORA plates, live accuracy graph) matching the redesigned menus.

**Architecture:** The mechanics core already exists — `lane_geometry.rs` (10 columns, `column_of`, `chip_color`), `layout.rs` (column geometry), note spawning on columns (`scroll.rs`), receptor/burst feedback (`playfield_viz.rs`), pad row (`keyboard_viz.rs`), and the widgets (`score_detailed`, `now_playing`, `phrase_meter`, `perf_combo`, `song_progress`, `playfield_speed`, `frame_chrome`). This plan changes the *visual layer only*: recenter the strip, make merged secondaries render hollow, restyle panels into GITADORA plates, move the combo to the strip center, add a new `live_graph` widget, and add side pillars. The 12-lane input/judge/score model is untouched.

**Tech Stack:** Bevy 0.19 (`Node`, `BackgroundColor`, `Outline`, `BorderColor`/`BorderRadius`, `MessageReader`), ref-resolution scaling via `PlayfieldLayout::scale` + `HudRefRect`, hand-rolled `ComboDisplay` tween.

**Spec:** `docs/superpowers/specs/2026-07-05-drums-hud-gitadora-flat-design.md`

**Verified facts (do not re-derive):**
- `lane_geometry::COLUMNS[col]` has `.ref_x`/`.ref_w`/`.label`/`.color` at NX absolute coords: strip = x 295..853 (width 558) at 1280×720. `STRIP_REF_LEFT=295.0`, `STRIP_REF_WIDTH=558.0`.
- `column_of(EChannel) -> Option<usize>` folds HHO→HH(1), LBD→BD(5). `chip_color(channel) -> Color`, `column_color(col)`, `lane_fill_color(col)` exist.
- `PlayfieldLayout` (scale, width, height) methods: `col_left(col)`, `col_width(col)`, `strip_left()`, `strip_width()`, `judge_y()`, `lane_top()`, `lane_height()`, `note_width(col)`, `note_height()`, `phrase_x()`, `ref_hud_right()`, `combo_left()`, `combo_top()`, `px(ref)`, `measure_label_left()`, `speed_label_left()`, `progress_bar_*()`. `REF_WIDTH=1280.0`, `REF_HEIGHT=720.0` in `dtx_ui::theme`.
- `Theme` tokens: `stage_panel_bg` (#0d0d0dee), `stage_panel_border` (#444), `select_yellow` (#ffcc00), `clear_green` (#00cc88), `panel_bg`, `accent` (cyan), `text_primary`, `text_secondary`, `judgment_perfect/great/good/miss`. `Theme::font(px)`, `scaled_font(scale, ref_px)`.
- `HudRefRect::new(left, top, width, height)` + `.apply(scale, &mut node)`; `hud.rs::apply_hud_ref_layout` rescales any entity carrying `HudRefRect` (with several `Without<...>` exclusions) on layout change.
- Note spawn: `scroll.rs::spawn_notes_system` spawns `Node` + `BackgroundColor(chip_color(chip.channel))` per chip; `reposition_notes_on_layout_change` re-anchors on resize.
- `resources::JudgmentCounts` fields: `perfect, great, good, ok, miss: u32`, `.total()`. `GameplayClock.current_ms: i64`. `derived::ChartDerived.phrase.last_chip_ms: i64`.
- `hud.rs::accuracy_pct` weights Perfect=100/Great=80/Good=60/Ok=40 → 0..100 achievement value.
- `dtx_scoring::Rank` boundaries (from lib.rs tests): S≥95, A 85–95, B 70–85, C 50–70.
- Bevy 0.19: renamed message events `MessageReader`/`MessageWriter`/`add_message`. `despawn` despawns children through the `Children` relationship. `BorderColor::all(color)`, `BorderRadius`, `Node { border: UiRect, .. }`.

**Conventions for every task:**
- Run tests from repo root: `cargo test -p gameplay-drums` / `cargo test -p dtx-ui`.
- Commit after each green step. Message style: `feat(hud): ...`, `feat(ui): ...`, `refactor(hud): ...`. **No AI co-author lines.**
- Do not add narrating comments; match surrounding comment density.
- Reference resolution stays 1280×720; use ref px in `HudRefRect` / `PlayfieldLayout::px`.

---

### Task 1: Center the lane strip

Move the strip from NX left-anchored (x 295..853) to centered (x 361..919). `lane_geometry` stays NX-authentic; only `layout.rs` places it on screen.

**Files:**
- Modify: `crates/gameplay-drums/src/layout.rs`

- [ ] **Step 1: Write failing tests** — replace the body of `col_left_matches_ref_at_default_scale` and `strip_width_matches_ref` in the `tests` module of `crates/gameplay-drums/src/layout.rs`, and add a centering test:

```rust
    #[test]
    fn strip_centered_at_default_scale() {
        let layout = PlayfieldLayout::default(); // scale 1.0 at 1280x720
        let expected_left = (REF_WIDTH - STRIP_REF_WIDTH) / 2.0; // 361.0
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
        assert!((layout.col_left(0) - expected_left).abs() < 0.01);
        let last = COLUMN_COUNT - 1;
        assert!(
            (layout.col_left(last) + layout.col_width(last)
                - (expected_left + STRIP_REF_WIDTH))
                .abs()
                < 0.5,
            "strip right edge should be centered"
        );
    }
```

Also delete the now-obsolete `col_left_matches_ref_at_default_scale` test (it asserted the old left-anchored 295 value). Add `use dtx_ui::theme::REF_WIDTH;` to the test module imports if not present (it already imports `STRIP_REF_LEFT, STRIP_REF_WIDTH, COLUMN_COUNT`).

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p gameplay-drums --lib layout::tests`
Expected: FAIL — `strip_centered_at_default_scale` fails (`strip_left()` still returns 295).

- [ ] **Step 3: Implement centered geometry** — in `crates/gameplay-drums/src/layout.rs`, add a centered-left constant and a column-x helper near the top (after the `use` block, before `PlayfieldLayout`):

```rust
use dtx_ui::theme::REF_WIDTH;

/// Centered strip left edge at ref resolution (redesign: symmetric panels).
pub const STRIP_REF_CENTERED_LEFT: f32 = (REF_WIDTH - STRIP_REF_WIDTH) / 2.0;

/// A column's left edge in ref px, translated from NX absolute into the
/// centered strip (columns keep their NX proportional widths + gaps).
#[inline]
pub fn col_ref_x(col: usize) -> f32 {
    STRIP_REF_CENTERED_LEFT + (COLUMNS[col].ref_x - STRIP_REF_LEFT)
}
```

Then change the placement methods to use the centered origin:

```rust
    pub fn col_left(&self, col: usize) -> f32 {
        col_ref_x(col) * self.scale
    }

    pub fn strip_left(&self) -> f32 {
        STRIP_REF_CENTERED_LEFT * self.scale
    }
```

Update the free helpers `ref_lane_left`, `ref_phrase_x`, and `ref_hud_right_x` to the centered origin:

```rust
#[inline]
pub fn ref_lane_left() -> f32 {
    STRIP_REF_CENTERED_LEFT
}

#[inline]
pub fn ref_phrase_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH + 8.0
}

#[inline]
pub fn ref_hud_right_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH + 24.0
}
```

And `speed_label_left`:

```rust
    pub fn speed_label_left(&self) -> f32 {
        (STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH - 96.0) * self.scale
    }
```

`col_width`, `strip_width`, `note_width`, `progress_bar_*` (which delegate to `strip_left`/`strip_width`) need no change.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p gameplay-drums --lib layout::tests`
Expected: PASS (all layout tests including `columns_monotonic`, `strip_centered_at_default_scale`).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/layout.rs
git commit -m "feat(hud): center lane strip for symmetric GITADORA panels"
```

---

### Task 2: Hollow secondary chips (HHO / LBD)

Open hi-hat and left bass render as a colored outline with transparent fill instead of a solid brighter/darker bar.

**Files:**
- Modify: `crates/gameplay-drums/src/lane_geometry.rs`
- Modify: `crates/gameplay-drums/src/scroll.rs`

- [ ] **Step 1: Write failing test** — append to the `tests` module of `crates/gameplay-drums/src/lane_geometry.rs`:

```rust
    #[test]
    fn only_open_hh_and_left_bass_are_hollow() {
        assert!(is_hollow(EChannel::HiHatOpen));
        assert!(is_hollow(EChannel::LeftBassDrum));
        assert!(!is_hollow(EChannel::HiHatClose));
        assert!(!is_hollow(EChannel::BassDrum));
        assert!(!is_hollow(EChannel::Snare));
    }
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p gameplay-drums --lib lane_geometry::tests::only_open_hh_and_left_bass_are_hollow`
Expected: FAIL — `is_hollow` not defined.

- [ ] **Step 3: Implement `is_hollow`** — add to `crates/gameplay-drums/src/lane_geometry.rs` (after `chip_color`):

```rust
/// Merged-secondary chips render as an outline (transparent fill) so they read
/// distinct from the filled primary sharing their column: HHO vs HH, LBD vs BD.
pub fn is_hollow(channel: EChannel) -> bool {
    matches!(channel, EChannel::HiHatOpen | EChannel::LeftBassDrum)
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p gameplay-drums --lib lane_geometry::tests`
Expected: PASS.

- [ ] **Step 5: Render hollow chips in spawn** — in `crates/gameplay-drums/src/scroll.rs`, update the import and the spawn block. Change the import line:

```rust
use crate::lane_geometry::{chip_color, column_of, is_hollow};
```

Replace the `commands.spawn((...))` note bundle inside `spawn_notes_system` (currently ending `BackgroundColor(chip_color(chip.channel)),`) with a hollow-aware version:

```rust
        let color = chip_color(chip.channel);
        let mut note_cmd = commands.spawn((
            Note {
                chip_id: idx,
                lane,
                target_ms,
            },
            NoteVisual,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(layout.note_width(col)),
                height: Val::Px(layout.note_height()),
                border: if is_hollow(chip.channel) {
                    UiRect::all(Val::Px(2.0 * layout.scale))
                } else {
                    UiRect::ZERO
                },
                ..default()
            },
        ));
        if is_hollow(chip.channel) {
            note_cmd.insert((
                BackgroundColor(Color::NONE),
                BorderColor::all(color),
            ));
        } else {
            note_cmd.insert(BackgroundColor(color));
        }
        let note_entity = note_cmd.id();
        commands.entity(hud).add_child(note_entity);
```

- [ ] **Step 6: Verify build + existing tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (compiles; scroll tests unaffected).

- [ ] **Step 7: Commit**

```bash
git add crates/gameplay-drums/src/lane_geometry.rs crates/gameplay-drums/src/scroll.rs
git commit -m "feat(hud): render open-hihat and left-bass chips as hollow outlines"
```

---

### Task 3: Add achievement-percent helper to `JudgmentCounts`

The live graph (Task 6) needs the same 0..100 achievement value the accuracy readout uses. Promote the private formula to a method so both share it.

**Files:**
- Modify: `crates/gameplay-drums/src/resources.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`

- [ ] **Step 1: Write failing test** — append to the `tests` module of `crates/gameplay-drums/src/resources.rs` (create the module if absent — check the file end; if there is no `#[cfg(test)] mod tests`, add one):

```rust
    #[test]
    fn achievement_pct_empty_is_full() {
        assert!((JudgmentCounts::default().achievement_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn achievement_pct_all_perfect_is_100() {
        let c = JudgmentCounts { perfect: 10, ..Default::default() };
        assert!((c.achievement_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn achievement_pct_all_good_is_60() {
        let c = JudgmentCounts { good: 4, ..Default::default() };
        assert!((c.achievement_pct() - 60.0).abs() < 0.01);
    }
```

If the test module already exists, add just the three tests. Ensure `JudgmentCounts` fields used here (`perfect`, `good`) match the real struct; adjust `..Default::default()` construction if the struct derives `Default` (it does — it is `init_resource`d).

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p gameplay-drums --lib resources::tests::achievement_pct_empty_is_full`
Expected: FAIL — `achievement_pct` not defined.

- [ ] **Step 3: Implement the method** — in `crates/gameplay-drums/src/resources.rs`, add an `impl JudgmentCounts` block (or extend the existing one) near the `JudgmentCounts` definition:

```rust
impl JudgmentCounts {
    /// GITADORA-style achievement value in 0..100 (Perfect=100, Great=80,
    /// Good=60, Ok=40, Miss=0), weighted over total judged chips.
    pub fn achievement_pct(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        let weighted = self.perfect as f32 * 100.0
            + self.great as f32 * 80.0
            + self.good as f32 * 60.0
            + self.ok as f32 * 40.0;
        weighted / total as f32
    }
}
```

If an `impl JudgmentCounts` already defines `total()`, add `achievement_pct` inside it instead of adding a second block.

- [ ] **Step 4: Point `hud.rs` at the shared method** — in `crates/gameplay-drums/src/hud.rs`, delete the private `fn accuracy_pct(counts: &JudgmentCounts) -> f32 { ... }` and its call site. In `sync_accuracy`, change `accuracy_pct(&counts)` to `counts.achievement_pct()`. Update the test `accuracy_default_full` to:

```rust
    #[test]
    fn accuracy_default_full() {
        assert!((JudgmentCounts::default().achievement_pct() - 100.0).abs() < 0.01);
    }
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p gameplay-drums`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/hud.rs
git commit -m "refactor(hud): share achievement-percent formula on JudgmentCounts"
```

---

### Task 4: Live accuracy graph widget

New right-column widget: 128-slot accuracy history rendered as vertical cyan bars behind SS/S/A/B threshold lines.

**Files:**
- Create: `crates/dtx-ui/src/widget/live_graph.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`

- [ ] **Step 1: Write failing test** — create `crates/dtx-ui/src/widget/live_graph.rs` with just the pure logic + tests first:

```rust
//! Live accuracy graph — 128 vertical bars vs rank threshold lines.

use crate::theme::Theme;
use crate::widget::hud_ref::HudRefRect;
use bevy::prelude::*;

pub const GRAPH_SLOTS: usize = 128;

/// Rank threshold lines drawn across the graph (percent, high→low).
pub const RANK_THRESHOLDS: [(f32, &str); 3] = [(95.0, "S"), (85.0, "A"), (70.0, "B")];

#[derive(Component)]
pub struct LiveGraphRoot;

#[derive(Component)]
pub struct LiveGraphBar {
    pub slot: usize,
}

/// Slot index for a song position (`pos_ms` of `total_ms`), clamped to 0..127.
pub fn slot_for_pos(pos_ms: i64, total_ms: i64) -> usize {
    if total_ms <= 0 {
        return 0;
    }
    let frac = (pos_ms as f64 / total_ms as f64).clamp(0.0, 1.0);
    ((frac * GRAPH_SLOTS as f64) as usize).min(GRAPH_SLOTS - 1)
}

/// Bar height in ref px for an accuracy percent over a graph of `bar_area_h`.
pub fn bar_height(accuracy_pct: f32, bar_area_h: f32) -> f32 {
    (accuracy_pct.clamp(0.0, 100.0) / 100.0) * bar_area_h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_zero_at_start() {
        assert_eq!(slot_for_pos(0, 10_000), 0);
    }

    #[test]
    fn slot_last_at_end() {
        assert_eq!(slot_for_pos(10_000, 10_000), GRAPH_SLOTS - 1);
    }

    #[test]
    fn slot_mid() {
        assert_eq!(slot_for_pos(5_000, 10_000), GRAPH_SLOTS / 2);
    }

    #[test]
    fn slot_guards_zero_total() {
        assert_eq!(slot_for_pos(1_000, 0), 0);
    }

    #[test]
    fn bar_full_at_100() {
        assert!((bar_height(100.0, 200.0) - 200.0).abs() < 0.01);
    }

    #[test]
    fn bar_half_at_50() {
        assert!((bar_height(50.0, 200.0) - 100.0).abs() < 0.01);
    }

    #[test]
    fn thresholds_match_rank_boundaries() {
        assert_eq!(RANK_THRESHOLDS[0], (95.0, "S"));
        assert_eq!(RANK_THRESHOLDS[1], (85.0, "A"));
        assert_eq!(RANK_THRESHOLDS[2], (70.0, "B"));
    }
}
```

- [ ] **Step 2: Register the module** — add to `crates/dtx-ui/src/widget/mod.rs` in alphabetical position (between `lane_flush` and `now_playing`):

```rust
pub mod live_graph;
```

- [ ] **Step 3: Run tests to verify pass**

Run: `cargo test -p dtx-ui --lib widget::live_graph`
Expected: PASS (7 tests).

- [ ] **Step 4: Add the spawn function** — append to `crates/dtx-ui/src/widget/live_graph.rs` (before `#[cfg(test)]`):

```rust
/// Spawn the graph panel: background plate, threshold lines with labels, and
/// `GRAPH_SLOTS` zero-height bars anchored to the panel bottom.
pub fn spawn_live_graph(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    ref_x: f32,
    ref_y: f32,
    ref_w: f32,
    ref_h: f32,
) {
    let bar_area_h = ref_h - 4.0;
    let bar_w = ref_w / GRAPH_SLOTS as f32;
    let bg = theme.stage_panel_bg;
    let bar_color = theme.accent;
    let line_color = Color::srgba(1.0, 0.85, 0.1, 0.4);

    commands.entity(parent).with_children(|p| {
        p.spawn((
            LiveGraphRoot,
            HudRefRect::new(ref_x, ref_y, ref_w, ref_h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(ref_y * scale),
                width: Val::Px(ref_w * scale),
                height: Val::Px(ref_h * scale),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(theme.stage_panel_border),
        ));

        for (pct, label) in RANK_THRESHOLDS {
            let line_y = ref_y + (1.0 - pct / 100.0) * bar_area_h;
            p.spawn((
                HudRefRect::new(ref_x, line_y, ref_w, 1.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(ref_x * scale),
                    top: Val::Px(line_y * scale),
                    width: Val::Px(ref_w * scale),
                    height: Val::Px(1.0 * scale),
                    ..default()
                },
                BackgroundColor(line_color),
            ));
            p.spawn((
                HudRefRect::new(ref_x + ref_w - 14.0, line_y - 6.0, 14.0, 12.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((ref_x + ref_w - 14.0) * scale),
                    top: Val::Px((line_y - 6.0) * scale),
                    width: Val::Px(14.0 * scale),
                    height: Val::Px(12.0 * scale),
                    ..default()
                },
                Text::new(label),
                Theme::font(10.0 * scale),
                TextColor(theme.text_secondary),
            ));
        }

        for slot in 0..GRAPH_SLOTS {
            let bx = ref_x + slot as f32 * bar_w;
            p.spawn((
                LiveGraphBar { slot },
                HudRefRect::new(bx, ref_y + ref_h, bar_w, 0.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(bx * scale),
                    top: Val::Px((ref_y + ref_h) * scale),
                    width: Val::Px(bar_w.max(1.0) * scale),
                    height: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(bar_color),
            ));
        }
    });
}
```

- [ ] **Step 5: Verify build**

Run: `cargo test -p dtx-ui --lib widget::live_graph`
Expected: PASS (compiles, 7 tests still green).

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-ui/src/widget/live_graph.rs crates/dtx-ui/src/widget/mod.rs
git commit -m "feat(ui): live accuracy graph widget"
```

---

### Task 5: Wire the live graph into the drums HUD

Add an `AccuracyHistory` resource, sample it each frame the clock advances, and drive the bar heights.

**Files:**
- Modify: `crates/gameplay-drums/src/resources.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`

- [ ] **Step 1: Write failing test** — append to the `tests` module of `crates/gameplay-drums/src/resources.rs`:

```rust
    #[test]
    fn accuracy_history_defaults_empty() {
        let h = AccuracyHistory::default();
        assert_eq!(h.samples.len(), 128);
        assert!(h.samples.iter().all(|s| s.is_none()));
    }

    #[test]
    fn accuracy_history_records_slot() {
        let mut h = AccuracyHistory::default();
        h.record(3, 88.0);
        assert_eq!(h.samples[3], Some(88.0));
    }
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p gameplay-drums --lib resources::tests::accuracy_history_defaults_empty`
Expected: FAIL — `AccuracyHistory` not defined.

- [ ] **Step 3: Implement the resource** — add to `crates/gameplay-drums/src/resources.rs`:

```rust
/// Per-slot accuracy history for the live graph (128 song-position buckets).
#[derive(Resource, Debug, Clone)]
pub struct AccuracyHistory {
    pub samples: [Option<f32>; 128],
}

impl Default for AccuracyHistory {
    fn default() -> Self {
        Self { samples: [None; 128] }
    }
}

impl AccuracyHistory {
    pub fn record(&mut self, slot: usize, accuracy_pct: f32) {
        if let Some(s) = self.samples.get_mut(slot) {
            *s = Some(accuracy_pct);
        }
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p gameplay-drums --lib resources::tests`
Expected: PASS.

- [ ] **Step 5: Register the resource** — in `crates/gameplay-drums/src/lib.rs`, add to the `plugin` builder chain next to the other `init_resource` calls:

```rust
    .init_resource::<resources::AccuracyHistory>()
```

- [ ] **Step 6: Spawn + sync in hud.rs** — in `crates/gameplay-drums/src/hud.rs`:

Add to the widget imports:

```rust
        now_playing, perf_combo, phrase_meter, playfield_speed, score_detailed, song_progress,
        live_graph,
```

Add to the `resources` import list: `AccuracyHistory`.

In `spawn_hud`, after the `now_playing::spawn_now_playing(...)` call, spawn the graph in the right column (placed under the phrase meter — coords finalized in Task 8; use these for now):

```rust
    live_graph::spawn_live_graph(
        &mut commands,
        root,
        &t,
        s,
        ref_hud_right_x() + 60.0, // right of the phrase meter
        300.0,
        140.0,
        300.0,
    );
```

Register two systems in the `Update` set (add to the tuple in `plugin`):

```rust
            sample_accuracy_history,
            sync_live_graph,
```

Add the systems at the end of the file (before `#[cfg(test)]`). `LiveGraphBar` entities are spawned with `HudRefRect.top` = the panel bottom (`ref_y + ref_h`); bars grow **upward** from that fixed baseline, so raise `node.top` by the bar height:

```rust
fn sample_accuracy_history(
    counts: Res<JudgmentCounts>,
    clock: Res<GameplayClock>,
    derived: Res<ChartDerived>,
    mut history: ResMut<AccuracyHistory>,
) {
    if !clock.is_changed() {
        return;
    }
    let total = derived.phrase.last_chip_ms.max(1);
    let slot = live_graph::slot_for_pos(clock.current_ms, total);
    history.record(slot, counts.achievement_pct());
}

fn sync_live_graph(
    history: Res<AccuracyHistory>,
    layout: Res<PlayfieldLayout>,
    mut q: Query<(&live_graph::LiveGraphBar, &HudRefRect, &mut Node)>,
) {
    if !history.is_changed() && !layout.is_changed() {
        return;
    }
    let bar_area_h = 300.0 - 4.0;
    for (bar, rect, mut node) in &mut q {
        let Some(acc) = history.samples.get(bar.slot).copied().flatten() else {
            node.height = Val::Px(0.0);
            continue;
        };
        let h = live_graph::bar_height(acc, bar_area_h);
        node.top = Val::Px((rect.top - h) * layout.scale);
        node.height = Val::Px(h * layout.scale);
    }
}
```

Because `LiveGraphBar` entities carry `HudRefRect` but have their `top`/`height` driven here, exclude them from the generic `apply_hud_ref_layout` rescaler: add `Without<live_graph::LiveGraphBar>` to that query's filter tuple in `apply_hud_ref_layout`.

Reset the history on entering a performance — in `spawn_hud`, take `mut history: ResMut<AccuracyHistory>` and set `*history = AccuracyHistory::default();` at the top (after the `EGameMode::Drums` guard).

- [ ] **Step 7: Verify build + tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (compiles, all green).

- [ ] **Step 8: Commit**

```bash
git add crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/hud.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(hud): sample and render live accuracy graph"
```

---

### Task 6: GITADORA left-panel restyle (SCORE DETAILED)

Remove the debug white outline box; use a GITADORA plate (dark panel bg + border) with a yellow left accent on the SCORE caption. Pure styling — verified by build + manual run.

**Files:**
- Modify: `crates/dtx-ui/src/widget/score_detailed.rs`

- [ ] **Step 1: Restyle the stats box** — in `spawn_score_detailed_panel`, replace the `StatsBoxBorder` bundle's `BackgroundColor` + `Outline` with the plate look:

```rust
            BackgroundColor(theme.stage_panel_bg),
            BorderColor::all(theme.stage_panel_border),
```

and add `border: UiRect::all(Val::Px(1.0)),` to that node's `Node { .. }`. Remove the `Outline { .. }` block entirely.

- [ ] **Step 2: Add a yellow accent bar on SCORE** — after the `ScoreNumberText` spawn, add a thin vertical accent rect to the left of the SCORE caption:

```rust
        let accent = HudRefRect::new(panel_x, 8.0, 3.0, 58.0);
        p.spawn((
            accent,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(accent.left * scale),
                top: Val::Px(accent.top * scale),
                width: Val::Px(accent.width * scale),
                height: Val::Px(accent.height * scale),
                ..default()
            },
            BackgroundColor(theme.select_yellow),
        ));
```

Shift the `ScoreCaptionText` and `ScoreNumberText` `left` from `panel_x + 8.0` to `panel_x + 14.0` so they clear the accent bar.

- [ ] **Step 3: Verify build + existing test**

Run: `cargo test -p dtx-ui --lib widget::score_detailed`
Expected: PASS (the `stats_box_in_left_panel` test is arithmetic-only, unaffected).

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui/src/widget/score_detailed.rs
git commit -m "feat(ui): GITADORA plate styling for score-detailed panel"
```

---

### Task 7: Pad-row restyle (dark fill + colored rim, rounded top)

Turn the tinted key-cap row into GITADORA pad glyphs: dark fill, 2px column-colored border, rounded top, brighten on hit.

**Files:**
- Modify: `crates/gameplay-drums/src/keyboard_viz.rs`

- [ ] **Step 1: Restyle spawn** — in `spawn_key_caps`, replace the `KeyCap` bundle so each pad has a dark fill, colored border, and rounded top. Change the `Node { .. }` to add border + the bundle to add `BorderColor` + `BorderRadius`:

```rust
    for col in 0..COLUMN_COUNT {
        let rim = column_color(col);
        commands.entity(parent).with_children(|row| {
            row.spawn((
                KeyCap { col: col as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col) + 2.0),
                    top: Val::Px(layout.key_viz_top()),
                    width: Val::Px(layout.col_width(col) - 4.0),
                    height: Val::Px(cap_h),
                    border: UiRect::all(Val::Px(2.0 * layout.scale)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.11, 0.13)),
                BorderColor::all(rim),
                BorderRadius {
                    top_left: Val::Px(cap_h * 0.4),
                    top_right: Val::Px(cap_h * 0.4),
                    bottom_left: Val::Px(4.0 * layout.scale),
                    bottom_right: Val::Px(4.0 * layout.scale),
                },
                children![(
                    Text::new(COLUMNS[col].label),
                    Theme::font(13.0 * layout.scale),
                    TextColor(theme.text_primary),
                )],
            ));
        });
    }
```

- [ ] **Step 2: Update the hit-flash resting color** — the flash systems (`flash_key_caps_on_hit`, `decay_key_cap_flashes`) currently rest at `column_color(col).with_alpha(0.18)`. Change the resting color in `decay_key_cap_flashes` to the new dark fill so decay returns there:

```rust
    for (cap, mut bg) in &mut caps {
        let rest = Color::srgb(0.11, 0.11, 0.13);
        if bg.0 != rest {
            let a = (bg.0.alpha() - dt * 4.0).max(1.0);
            if bg.0.alpha() <= 0.99 || a <= 1.0 {
                bg.0 = rest;
            }
        }
    }
```

Simplify: since the border now carries color identity, the fill just brightens white-ish on hit and decays back to dark. Replace `flash_key_caps_on_hit`'s writes (`bg.0 = accent.with_alpha(0.45)` / `0.55`) with a bright fill:

```rust
                bg.0 = Color::srgb(0.30, 0.30, 0.34); // press
```
and
```rust
                bg.0 = column_color(col).with_alpha(0.85); // judged hit
```

Keep `decay_key_cap_flashes` lerping back to `Color::srgb(0.11, 0.11, 0.13)`:

```rust
pub fn decay_key_cap_flashes(
    _theme: Res<ThemeResource>,
    time: Res<Time>,
    mut caps: Query<(&KeyCap, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    let rest = Color::srgb(0.11, 0.11, 0.13);
    for (_cap, mut bg) in &mut caps {
        if bg.0 == rest {
            continue;
        }
        let cur = bg.0.to_srgba();
        let target = rest.to_srgba();
        let lerp = |a: f32, b: f32| a + (b - a) * (dt * 6.0).min(1.0);
        let next = Color::srgba(
            lerp(cur.red, target.red),
            lerp(cur.green, target.green),
            lerp(cur.blue, target.blue),
            1.0,
        );
        bg.0 = if (next.to_srgba().red - target.red).abs() < 0.01 {
            rest
        } else {
            next
        };
    }
}
```

- [ ] **Step 3: Verify build + tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (compiles).

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/keyboard_viz.rs
git commit -m "feat(hud): GITADORA pad glyphs with colored rims"
```

---

### Task 8: Combo to strip center + right-column layout + side pillars

Reposition the combo into the strip center, place song card / phrase meter / live graph in the right column at centered-strip coords, and add dark side pillars around the strip.

**Files:**
- Modify: `crates/gameplay-drums/src/hud.rs`
- Modify: `crates/dtx-ui/src/widget/frame_chrome.rs`

- [ ] **Step 1: Move the combo to strip center** — in `crates/gameplay-drums/src/hud.rs` `spawn_hud`, replace the `perf_combo::spawn_perf_combo(...)` call. The combo number is 360 ref wide; center it on the strip. Add to the top-of-file imports:

```rust
use crate::layout::STRIP_REF_CENTERED_LEFT;
use crate::lane_geometry::STRIP_REF_WIDTH;
```

Then replace the call with:

```rust
    let combo_ref_x = STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH / 2.0 - 180.0;
    perf_combo::spawn_perf_combo(&mut commands, root, &t, s, combo_ref_x, 150.0);
```

(360-wide number centered → left = center − 180. `ref_y=150` sits it in the strip's upper third above the notes' hit line.)

- [ ] **Step 2: Place the right column** — update the right-side spawns in `spawn_hud` to the centered layout. `ref_hud_right_x()` already returns the centered strip's right edge + 24 (≈ 943). Set:

```rust
    let hud_right = ref_hud_right_x();
    now_playing::spawn_now_playing(&mut commands, root, &t, s, hud_right);
    phrase_meter::spawn_phrase_meter(&mut commands, root, &t, s, ref_phrase_x());
    live_graph::spawn_live_graph(
        &mut commands,
        root,
        &t,
        s,
        hud_right + 40.0,
        300.0,
        REF_WIDTH - (hud_right + 40.0) - 12.0,
        300.0,
    );
```

Add `use dtx_ui::theme::REF_WIDTH;` to the `hud.rs` imports. Remove the earlier Task-5 `spawn_live_graph` call (this replaces it) so the graph is spawned exactly once.

Note `now_playing` uses `panel_w = 400.0` internally — at `hud_right ≈ 943` that overflows 1280. Pass a card that fits: change the `now_playing` panel width. In `crates/dtx-ui/src/widget/now_playing.rs`, change `let panel_w = 400.0;` to `let panel_w = 320.0;`. Rebuild both crates.

- [ ] **Step 3: Add side pillars to frame_chrome** — replace `crates/dtx-ui/src/widget/frame_chrome.rs` `spawn_frame_chrome` body to add two dark vertical pillars flanking the strip. It needs the strip bounds, so extend the signature:

```rust
pub fn spawn_frame_chrome(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    strip_left_ref: f32,
    strip_right_ref: f32,
) {
    let pillar_w = 10.0;
    let pillar_color = Color::srgb(0.08, 0.08, 0.10);
    let edge = theme.stage_panel_border;
    for (x, _side) in [
        (strip_left_ref - pillar_w - 2.0, 0u8),
        (strip_right_ref + 2.0, 1u8),
    ] {
        commands.entity(parent).with_children(|p| {
            p.spawn((
                HudRefRect::new(x, 0.0, pillar_w, REF_HEIGHT),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x * scale),
                    top: Val::Px(0.0),
                    width: Val::Px(pillar_w * scale),
                    height: Val::Percent(100.0),
                    border: UiRect::horizontal(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(pillar_color),
                BorderColor::all(edge),
            ));
        });
    }
}
```

Add `use crate::theme::REF_HEIGHT;` to `frame_chrome.rs`. Update the caller in `hud.rs`:

```rust
    frame_chrome::spawn_frame_chrome(
        &mut commands,
        root,
        &t,
        s,
        STRIP_REF_CENTERED_LEFT,
        STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH,
    );
```

- [ ] **Step 4: Verify build + tests**

Run: `cargo test -p gameplay-drums && cargo test -p dtx-ui`
Expected: PASS (both crates compile; existing widget tests green).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/hud.rs crates/dtx-ui/src/widget/frame_chrome.rs crates/dtx-ui/src/widget/now_playing.rs
git commit -m "feat(hud): center combo, right-column layout, side pillars"
```

---

### Task 9: Full workspace build, tests, and visual verification

**Files:** none (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo build`
Expected: builds clean, no warnings introduced by this work.

- [ ] **Step 2: Full workspace tests**

Run: `cargo test`
Expected: all green.

- [ ] **Step 3: Clippy**

Run: `cargo clippy -p gameplay-drums -p dtx-ui`
Expected: no new warnings.

- [ ] **Step 4: Visual verification (manual)** — launch the app, start a drums chart, and confirm against the spec:
  - Strip is centered; left panel (SCORE/detailed/completion/skill) and right panel (time/song card/phrase meter/live graph) are symmetric.
  - Column order LC HH LP SD HT BD LT FT CY RD, left→right.
  - Open hi-hat and left-bass chips render as hollow outlines; all others solid.
  - Combo number sits big in the strip center and pops on increment.
  - Yellow hit line glows; pad row shows dark pads with colored rims that brighten on hit.
  - Live graph fills left→right as you play, with S/A/B threshold lines.
  - Phrase meter shows unlabeled density blocks with a moving cursor.
  - No debug keybind text, no white stats border.

- [ ] **Step 5: Commit any tuning** — if manual verification requires coordinate/color tweaks, make them and commit:

```bash
git add -A
git commit -m "fix(hud): visual tuning after playtest"
```

---

## Self-Review Notes

- **Spec coverage:** Center strip (T1), hollow HHO/LBD (T2), achievement% share (T3), live graph widget + wiring (T4–T5), GITADORA plates (T6), pad glyphs (T7), combo center + right column + pillars (T8), verification (T9). Phrase meter is already unlabeled density blocks — no task needed (spec §Right column point 3 satisfied by existing `phrase_meter.rs`). Ambient dimmed background: the HUD root already paints solid black behind the strip; the dim-ambient-background menu commit (`1c7984e`) covers the menu; a gameplay ambient bg is out of scope for this pass (strip stays on black per current `spawn_hud`).
- **Type consistency:** `slot_for_pos`/`bar_height`/`GRAPH_SLOTS`/`RANK_THRESHOLDS`/`LiveGraphBar` used identically in T4 and T5. `achievement_pct` defined T3, consumed T5. `STRIP_REF_CENTERED_LEFT` defined T1, consumed T8. `is_hollow` defined T2, consumed T2.
- **Known risk:** T5's `sync_live_graph` grows bars upward using the spawned `HudRefRect.top` (panel bottom) as the fixed baseline. If bars render downward, the sign on `rect.top - h` is the single knob to flip — flagged for T9 manual check.
