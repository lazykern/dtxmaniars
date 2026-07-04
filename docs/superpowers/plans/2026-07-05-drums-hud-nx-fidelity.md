# Drums HUD DTXManiaNX Fidelity — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the drums performance HUD faithful to DTXManiaNX — correct 10-column lane order with variable widths + per-column color, legible notes, full hit feedback, and a panel layout that uses the whole screen.

**Architecture:** Decouple the *visual column* (10, NX geometry) from the *lane/channel* (12, unchanged input/judge/score model). A new `lane_geometry.rs` module is the single source of truth for column x/width/color and the `EChannel → column` map (HHO→HH, LBD→BD). All render systems switch from `lane_left(lane)`/`lane_width()` to `col_left(col)`/`col_width(col)`; note/feedback color comes from `chip_color(channel)`.

**Tech Stack:** Rust, Bevy UI (Node/BackgroundColor absolute layout), existing `gameplay-drums` crate.

**Reference:** `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs` (pad base x), `CActPerfDrumsLaneFlushD.cs` (lane widths). Ref resolution 1280×720 == `REF_WIDTH`/`REF_HEIGHT`, so NX px used directly.

**Spec:** `docs/superpowers/specs/2026-07-05-drums-hud-nx-fidelity-design.md`

---

## File Structure

- **Create** `crates/gameplay-drums/src/lane_geometry.rs` — column table (label, ref_x, ref_w, color), `column_of(channel)`, `chip_color(channel)`, `COLUMN_COUNT`. Pure data + lookups.
- **Modify** `crates/gameplay-drums/src/lib.rs` — register `mod lane_geometry;`.
- **Modify** `crates/gameplay-drums/src/layout.rs` — replace uniform `ref_lane_w`/`lane_left`/`lane_width` with `col_left`/`col_width`/`strip_left`/`strip_width`; bump `note_height`; update strip-anchor constants; fix tests.
- **Modify** `crates/gameplay-drums/src/scroll.rs` — delete `lane_color`; notes map `channel→column`, use column geometry + `chip_color`.
- **Modify** `crates/gameplay-drums/src/hud.rs` — backboard/lane columns/hit line iterate columns; `LaneColumn{lane}`→`{col}`; reposition panel spawns.
- **Modify** `crates/gameplay-drums/src/playfield_viz.rs` — receptors + hit bursts keyed on column; flash maps `hit.lane→channel→column`.
- **Modify** `crates/gameplay-drums/src/keyboard_viz.rs` — pad glyphs per column, drop the `\n{key_label}` debug text.
- **Modify** `crates/gameplay-drums/src/beat_lines.rs` — beat lines span `strip_left..strip_width` (rename calls only).
- **Modify** widget spawns as needed for reposition: `crates/dtx-ui/src/widget/{now_playing,perf_combo,song_progress,playfield_speed}.rs` are called with new ref coords from `hud.rs` — no signature change expected; verify.

**Note on tests:** `lane_geometry` and `layout` geometry are pure functions — real unit tests (TDD). The render systems place Bevy UI nodes; their correctness is visual, not unit-assertable, so those tasks pair exact code with a `cargo build` + run-and-look verification step. Do not fabricate pixel-position unit tests for nodes.

---

### Task 1: `lane_geometry` module — column table + channel mapping

**Files:**
- Create: `crates/gameplay-drums/src/lane_geometry.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (add `mod lane_geometry;`)

- [ ] **Step 1: Write the module with failing-compile tests**

Create `crates/gameplay-drums/src/lane_geometry.rs`:

```rust
//! Visual lane columns (DTXManiaNX geometry).
//!
//! The input/judge/score model uses 12 `EChannel`/`LaneId` lanes (see `lane_map`).
//! The *render* layer collapses those into 10 on-screen columns matching NX:
//! order LC HH LP SD HT BD LT FT CY RD, with open hi-hat drawn on the HH column
//! and left bass on the BD column. Geometry derived from `CActPerfDrumsPad.cs`
//! pad bases + `CActPerfDrumsLaneFlushD.cs` flush rects at 1280x720 (EType.A/RCRD).

use bevy::prelude::Color;
use dtx_core::EChannel;

pub const COLUMN_COUNT: usize = 10;

pub struct Column {
    pub label: &'static str,
    pub ref_x: f32,
    pub ref_w: f32,
    /// Base chip color (linear-ish sRGB tuple).
    pub color: (f32, f32, f32),
}

/// Columns ordered left→right at 1280x720. Contiguous, strip = x 295..853 (w 558).
pub const COLUMNS: [Column; COLUMN_COUNT] = [
    Column { label: "LC", ref_x: 295.0, ref_w: 72.0, color: (0.80, 0.27, 0.80) }, // purple
    Column { label: "HH", ref_x: 367.0, ref_w: 49.0, color: (0.20, 0.73, 0.93) }, // cyan
    Column { label: "LP", ref_x: 416.0, ref_w: 51.0, color: (1.00, 0.40, 0.67) }, // pink
    Column { label: "SD", ref_x: 467.0, ref_w: 57.0, color: (1.00, 0.87, 0.20) }, // yellow
    Column { label: "HT", ref_x: 524.0, ref_w: 49.0, color: (1.00, 0.33, 0.33) }, // red
    Column { label: "BD", ref_x: 573.0, ref_w: 69.0, color: (1.00, 0.53, 0.20) }, // orange
    Column { label: "LT", ref_x: 642.0, ref_w: 49.0, color: (0.33, 0.87, 0.33) }, // green
    Column { label: "FT", ref_x: 691.0, ref_w: 54.0, color: (0.20, 0.53, 1.00) }, // blue
    Column { label: "CY", ref_x: 745.0, ref_w: 70.0, color: (0.87, 0.40, 1.00) }, // violet
    Column { label: "RD", ref_x: 815.0, ref_w: 38.0, color: (0.40, 0.87, 0.80) }, // teal
];

/// Strip left edge / total width at ref resolution.
pub const STRIP_REF_LEFT: f32 = 295.0;
pub const STRIP_REF_WIDTH: f32 = 558.0;

/// EChannel → visual column index. HHO→HH col, LBD→BD col. None if not a drum chip.
pub fn column_of(channel: EChannel) -> Option<usize> {
    Some(match channel {
        EChannel::LeftCymbal => 0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 1,
        EChannel::LeftPedal => 2,
        EChannel::Snare => 3,
        EChannel::HighTom => 4,
        EChannel::BassDrum | EChannel::LeftBassDrum => 5,
        EChannel::LowTom => 6,
        EChannel::FloorTom => 7,
        EChannel::Cymbal => 8,
        EChannel::RideCymbal => 9,
        _ => return None,
    })
}

/// Chip color for a channel: column base, with a distinct variant for the merged
/// secondary chips (HHO reads brighter than HH; LBD reads darker than BD).
pub fn chip_color(channel: EChannel) -> Color {
    let Some(col) = column_of(channel) else {
        return Color::WHITE;
    };
    let (r, g, b) = COLUMNS[col].color;
    match channel {
        EChannel::HiHatOpen => Color::srgb((r + 0.25).min(1.0), (g + 0.15).min(1.0), 1.0),
        EChannel::LeftBassDrum => Color::srgb(r * 0.6, g * 0.6, b * 0.6),
        _ => Color::srgb(r, g, b),
    }
}

/// Column base color as a Bevy `Color`.
pub fn column_color(col: usize) -> Color {
    let (r, g, b) = COLUMNS.get(col).map(|c| c.color).unwrap_or((1.0, 1.0, 1.0));
    Color::srgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_columns_ordered_and_contiguous() {
        assert_eq!(COLUMNS.len(), COLUMN_COUNT);
        for w in COLUMNS.windows(2) {
            assert!(w[1].ref_x >= w[0].ref_x, "columns must be left→right");
            assert!(
                (w[0].ref_x + w[0].ref_w - w[1].ref_x).abs() < 1.0,
                "columns should be contiguous: {} end {} vs {} start {}",
                w[0].label, w[0].ref_x + w[0].ref_w, w[1].label, w[1].ref_x
            );
        }
    }

    #[test]
    fn strip_bounds_match_constants() {
        assert_eq!(COLUMNS[0].ref_x, STRIP_REF_LEFT);
        let last = &COLUMNS[COLUMN_COUNT - 1];
        assert!((last.ref_x + last.ref_w - (STRIP_REF_LEFT + STRIP_REF_WIDTH)).abs() < 1.0);
    }

    #[test]
    fn hho_maps_to_hh_column_bd_lbd_shared() {
        assert_eq!(column_of(EChannel::HiHatOpen), column_of(EChannel::HiHatClose));
        assert_eq!(column_of(EChannel::LeftBassDrum), column_of(EChannel::BassDrum));
        assert_eq!(COLUMNS[column_of(EChannel::HiHatClose).unwrap()].label, "HH");
        assert_eq!(COLUMNS[column_of(EChannel::BassDrum).unwrap()].label, "BD");
    }

    #[test]
    fn canonical_order_left_to_right() {
        let labels: Vec<_> = COLUMNS.iter().map(|c| c.label).collect();
        assert_eq!(labels, ["LC","HH","LP","SD","HT","BD","LT","FT","CY","RD"]);
    }

    #[test]
    fn secondary_chips_distinct_from_primary() {
        assert_ne!(chip_color(EChannel::HiHatOpen), chip_color(EChannel::HiHatClose));
        assert_ne!(chip_color(EChannel::LeftBassDrum), chip_color(EChannel::BassDrum));
    }

    #[test]
    fn non_drum_channel_has_no_column() {
        assert_eq!(column_of(EChannel::BGM), None);
        assert_eq!(column_of(EChannel::BarLine), None);
    }
}
```

Add to `crates/gameplay-drums/src/lib.rs` alongside the other `mod` lines (find the `mod lane_map;` / `mod layout;` block and add):

```rust
mod lane_geometry;
```

- [ ] **Step 2: Run tests — verify they pass**

Run: `cargo test -p gameplay-drums lane_geometry`
Expected: PASS (6 tests). If `EChannel` variant names differ (e.g. `LeftBassDrum` spelled otherwise), fix to match `dtx_core::EChannel` — check with `cargo build -p gameplay-drums` and read the error.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/lane_geometry.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(drums): lane_geometry module — 10 NX columns + channel mapping"
```

---

### Task 2: `layout.rs` — column-driven geometry

**Files:**
- Modify: `crates/gameplay-drums/src/layout.rs`

- [ ] **Step 1: Update tests first (they define the new API)**

In `crates/gameplay-drums/src/layout.rs`, replace the whole `#[cfg(test)] mod tests { ... }` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_geometry::{COLUMN_COUNT, STRIP_REF_LEFT, STRIP_REF_WIDTH};

    #[test]
    fn judge_below_lane_top() {
        let layout = PlayfieldLayout::default();
        assert!(layout.judge_y() > layout.lane_top());
    }

    #[test]
    fn lane_height_spans_to_judge() {
        let layout = PlayfieldLayout::default();
        assert!(
            (layout.lane_top() + layout.lane_height() - layout.judge_y()).abs() < 1.0,
            "lane bottom should align with judge line"
        );
    }

    #[test]
    fn col_left_matches_ref_at_default_scale() {
        let layout = PlayfieldLayout::default(); // scale 1.0 at 1280x720
        assert!((layout.col_left(0) - STRIP_REF_LEFT).abs() < 0.01);
        let last = COLUMN_COUNT - 1;
        assert!(
            (layout.col_left(last) + layout.col_width(last)
                - (STRIP_REF_LEFT + STRIP_REF_WIDTH)).abs() < 0.5
        );
    }

    #[test]
    fn columns_monotonic() {
        let layout = PlayfieldLayout::default();
        for c in 1..COLUMN_COUNT {
            assert!(layout.col_left(c) > layout.col_left(c - 1));
        }
    }

    #[test]
    fn strip_width_matches_ref() {
        let layout = PlayfieldLayout::default();
        assert!((layout.strip_width() - STRIP_REF_WIDTH).abs() < 0.5);
    }
}
```

- [ ] **Step 2: Run tests — verify they FAIL to compile**

Run: `cargo test -p gameplay-drums --lib layout`
Expected: FAIL — `col_left`, `col_width`, `strip_width` not found.

- [ ] **Step 3: Implement column geometry**

In `layout.rs`, change the imports at the top:

```rust
use crate::lane_geometry::{COLUMNS, COLUMN_COUNT, STRIP_REF_LEFT, STRIP_REF_WIDTH};
```

(Remove the `use crate::lane_map::LANE_COUNT;` line — no longer used here.)

Replace the geometry methods. Remove `ref_lane_w`, `lane_left`, `lane_width`, `lane_strip_width`, `lane_strip_left`, and the `note_width` body's dependence on `lane_width`. Add:

```rust
    pub fn col_left(&self, col: usize) -> f32 {
        COLUMNS[col].ref_x * self.scale
    }

    pub fn col_width(&self, col: usize) -> f32 {
        COLUMNS[col].ref_w * self.scale
    }

    pub fn strip_left(&self) -> f32 {
        STRIP_REF_LEFT * self.scale
    }

    pub fn strip_width(&self) -> f32 {
        STRIP_REF_WIDTH * self.scale
    }
```

Update `note_width` to take a column:

```rust
    pub fn note_width(&self, col: usize) -> f32 {
        (self.col_width(col) - 4.0 * self.scale).max(2.0 * self.scale)
    }
```

Update `note_height` (bigger, NX chip proportion):

```rust
    pub fn note_height(&self) -> f32 {
        14.0 * self.scale
    }
```

Update the backboard/measure/progress helpers that referenced `lane_strip_*`:

```rust
    pub fn measure_label_left(&self) -> f32 {
        self.strip_left() + self.strip_width() + 8.0 * self.scale
    }

    pub fn backboard_left(&self) -> f32 {
        self.strip_left() - REF_BACKBOARD_PAD * self.scale
    }

    pub fn backboard_width(&self) -> f32 {
        self.strip_width() + REF_BACKBOARD_PAD * self.scale * 2.0
    }

    pub fn progress_bar_left(&self) -> f32 {
        self.strip_left()
    }

    pub fn progress_bar_width(&self) -> f32 {
        self.strip_width()
    }
```

Update the module-level anchor helpers so the right-hand panel sits just right of the NX strip (strip right edge = 853):

```rust
/// Phrase meter sits just right of the lane strip.
#[inline]
pub fn ref_phrase_x() -> f32 {
    STRIP_REF_LEFT + STRIP_REF_WIDTH + 8.0
}

/// Right HUD column (song info, combo, gauge) anchor.
#[inline]
pub fn ref_hud_right_x() -> f32 {
    STRIP_REF_LEFT + STRIP_REF_WIDTH + 24.0
}
```

(Add `use crate::lane_geometry::{STRIP_REF_LEFT, STRIP_REF_WIDTH};` at module scope if these free functions are outside the impl — they are; keep the import at file top so both impl and free fns see it. Remove now-unused `ref_lane_left` if nothing references it; if `hud.rs` still calls it, keep it returning `STRIP_REF_LEFT`.) Keep a compatibility shim so `hud.rs` compiles before Task 4:

```rust
#[inline]
pub fn ref_lane_left() -> f32 {
    STRIP_REF_LEFT
}
```

- [ ] **Step 4: Run tests — verify they pass**

Run: `cargo test -p gameplay-drums --lib layout`
Expected: PASS. `cargo build -p gameplay-drums` will still FAIL (callers use old `lane_left`) — that is expected and fixed in Tasks 3–6. Do not commit yet if the crate does not build; instead proceed to Task 3 and commit after the crate builds again. If you prefer a green commit here, temporarily keep old method names as `#[deprecated]` shims delegating to column 0 — but simplest is to push through Tasks 3–6 then commit. Choose the push-through path.

---

### Task 3: `scroll.rs` — notes on columns + chip color

**Files:**
- Modify: `crates/gameplay-drums/src/scroll.rs`

- [ ] **Step 1: Delete `lane_color`, retarget note spawn**

In `scroll.rs`, delete the entire `pub fn lane_color(lane: u8) -> Color { ... }` (lines ~71–88).

Add import near the top imports:

```rust
use crate::lane_geometry::{chip_color, column_of};
```

In `spawn_notes_system`, the loop computes `lane` from `lane_of(chip.channel)`. Replace the geometry lines. Find:

```rust
        let top = top_for_note(target_ms, now, judge_y, px_per_ms);
        let left = layout.lane_left(lane as usize) + 2.0;
```

Replace with (map channel→column; skip if no column):

```rust
        let Some(col) = column_of(chip.channel) else {
            continue;
        };
        let top = top_for_note(target_ms, now, judge_y, px_per_ms);
        let left = layout.col_left(col) + 2.0;
```

Then in the spawned `Node { ... }` change the width and the color:

```rust
                    width: Val::Px(layout.note_width(col)),
                    height: Val::Px(layout.note_height()),
```
```rust
                BackgroundColor(chip_color(chip.channel)),
```

(The `lane` binding from `lane_of` is still used for the `Note { lane, .. }` component — keep it for judgement/scroll. Only *geometry+color* switch to column.)

- [ ] **Step 2: Fix `scroll_notes_system` width if it re-sets width**

Search `scroll.rs` for other `layout.note_width()` / `layout.lane_left(` calls (the scroll/relayout system). For each, derive the column from the `Note`'s lane: `let col = column_of(crate::lane_map::lane_channel(note.lane).unwrap()).unwrap_or(0);` then use `layout.col_left(col)` / `layout.note_width(col)`. If `scroll_notes_system` only updates `top`, no change needed.

- [ ] **Step 3: Build**

Run: `cargo build -p gameplay-drums`
Expected: still FAILs on `hud.rs`/`playfield_viz.rs`/`keyboard_viz.rs` (Tasks 4–6). `scroll.rs` errors should be gone — scroll to confirm no `scroll.rs:*` errors remain.

---

### Task 4: `hud.rs` — backboard, lane columns, hit line, panel anchors

**Files:**
- Modify: `crates/gameplay-drums/src/hud.rs`

- [ ] **Step 1: Lane columns iterate visual columns**

In `hud.rs`, the `LaneColumn` component: change its field from `lane` to `col`. Find its definition (search `struct LaneColumn`) and change `pub lane: usize` → `pub col: usize`.

Replace the lane-column spawn loop (currently `for lane in 0..LANE_COUNT`):

```rust
        for col in 0..lane_geometry::COLUMN_COUNT {
            let tint = lane_geometry::column_color(col).with_alpha(0.10);
            root.spawn((
                LaneColumn { col },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col)),
                    top: Val::Px(layout.lane_top()),
                    width: Val::Px(layout.col_width(col) - 2.0),
                    height: Val::Px(layout.lane_height()),
                    ..default()
                },
                BackgroundColor(tint),
            ));
        }
```

Add `use crate::lane_geometry;` to the imports. The `use ... LANE_COUNT` re-export (`pub use crate::lane_map::LANE_COUNT;`) can stay (other modules/tests use it) but is no longer used by this loop.

Update the hit line spawn to use strip helpers:

```rust
                left: Val::Px(layout.strip_left()),
                top: Val::Px(layout.judge_y()),
                width: Val::Px(layout.strip_width()),
```

- [ ] **Step 2: Fix `apply_lane_column_layout`**

```rust
fn apply_lane_column_layout(
    layout: Res<PlayfieldLayout>,
    mut lanes: Query<(&LaneColumn, &mut Node)>,
) {
    for (col, mut node) in &mut lanes {
        node.left = Val::Px(layout.col_left(col.col));
        node.top = Val::Px(layout.lane_top());
        node.width = Val::Px(layout.col_width(col.col) - 2.0);
        node.height = Val::Px(layout.lane_height());
    }
}
```

- [ ] **Step 3: Fix `apply_hit_line_layout`**

```rust
fn apply_hit_line_layout(
    layout: Res<PlayfieldLayout>,
    mut hit_line: Query<&mut Node, With<HitLine>>,
) {
    for mut node in &mut hit_line {
        node.left = Val::Px(layout.strip_left());
        node.top = Val::Px(layout.judge_y());
        node.width = Val::Px(layout.strip_width());
        node.height = Val::Px(3.0 * layout.scale);
    }
}
```

- [ ] **Step 4: Reposition panel spawns (kill dead space, fix combo clip)**

In the spawn function, update the widget spawn calls (around lines 162–180). Replace with:

```rust
        song_progress::spawn_song_progress(
            &mut commands,
            root,
            &t,
            s,
            lane_geometry::STRIP_REF_LEFT,
            lane_geometry::STRIP_REF_WIDTH,
        );
        playfield_speed::spawn_playfield_speed(
            &mut commands,
            root,
            &t,
            s,
            lane_geometry::STRIP_REF_LEFT + lane_geometry::STRIP_REF_WIDTH - 96.0,
        );
        let hud_right = ref_hud_right_x();
        now_playing::spawn_now_playing(&mut commands, root, &t, s, hud_right);
        // Combo below the song-info card (was clipping at y=72). Card ≈ y 20..118.
        perf_combo::spawn_perf_combo(&mut commands, root, &t, s, hud_right, 150.0);
```

(If `spawn_playfield_speed`/`spawn_song_progress` are outside the `with_children` closure in the current code, keep them where they are — only change the argument values. Match the existing call structure; change only the coordinate args and the combo Y `REF_COMBO_Y`→`150.0`.)

- [ ] **Step 5: Build**

Run: `cargo build -p gameplay-drums`
Expected: remaining errors only in `playfield_viz.rs` / `keyboard_viz.rs`. If `REF_COMBO_Y` / `REF_LANE_FIELD_W` imports become unused, remove them from the `use` list to clear warnings.

---

### Task 5: `playfield_viz.rs` — receptors + bursts on columns

**Files:**
- Modify: `crates/gameplay-drums/src/playfield_viz.rs`

- [ ] **Step 1: Receptor keyed on column**

Change `LaneReceptor { pub lane: u8 }` → `LaneReceptor { pub col: u8 }`.

Replace the import `use crate::scroll::lane_color;` with:

```rust
use crate::lane_geometry::{column_color, column_of, COLUMN_COUNT};
use crate::lane_map::lane_channel;
```

Rewrite `spawn_lane_receptors` to loop columns:

```rust
pub fn spawn_lane_receptors(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    theme: &dtx_ui::theme::Theme,
) {
    for col in 0..COLUMN_COUNT {
        commands.entity(parent).with_children(|root| {
            root.spawn((
                LaneReceptor { col: col as u8 },
                ReceptorFlash {
                    timer: Timer::from_seconds(0.0, TimerMode::Once),
                    strength: 0.0,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col) + 2.0),
                    top: Val::Px(layout.judge_y() - 12.0 * layout.scale),
                    width: Val::Px(layout.col_width(col) - 4.0),
                    height: Val::Px(24.0 * layout.scale),
                    ..default()
                },
                BackgroundColor(theme.panel_bg),
            ));
        });
    }
}
```

- [ ] **Step 2: Map hit lane → column at flash site**

In `flash_receptors_on_hit`, incoming events carry `.lane` (LaneId). Add a helper closure and match on column. Replace the two loop bodies:

```rust
    let lane_to_col = |lane: u8| -> Option<usize> {
        lane_channel(lane).and_then(column_of)
    };
    for hit in lane_hits.read() {
        let Some(col) = lane_to_col(hit.lane) else { continue };
        for (receptor, mut flash) in &mut receptors {
            if receptor.col as usize == col {
                flash.timer = Timer::from_seconds(RECEPTOR_FLASH_SECS, TimerMode::Once);
                flash.strength = 0.7;
            }
        }
        spawn_hit_burst(&mut commands, hud, &layout, col, 0.7);
    }
    for ev in events.read() {
        let strength = match ev.kind {
            JudgmentKind::Perfect => 1.0,
            JudgmentKind::Great => 0.9,
            JudgmentKind::Good => 0.75,
            JudgmentKind::Poor => 0.55,
            JudgmentKind::Miss => 0.0,
        };
        if strength <= 0.0 { continue; }
        let Some(col) = lane_to_col(ev.lane) else { continue };
        for (receptor, mut flash) in &mut receptors {
            if receptor.col as usize == col {
                flash.timer = Timer::from_seconds(RECEPTOR_FLASH_SECS, TimerMode::Once);
                flash.strength = strength;
            }
        }
        spawn_hit_burst(&mut commands, hud, &layout, col, strength);
    }
```

- [ ] **Step 3: Burst + flash use column geometry/color**

Rewrite `spawn_hit_burst` signature to take `col: usize`:

```rust
fn spawn_hit_burst(
    commands: &mut Commands,
    hud: Entity,
    layout: &PlayfieldLayout,
    col: usize,
    strength: f32,
) {
    let burst = commands
        .spawn((
            HitBurst {
                timer: Timer::from_seconds(HIT_BURST_SECS, TimerMode::Once),
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.col_left(col) + 4.0),
                top: Val::Px(layout.judge_y() - layout.note_height()),
                width: Val::Px(layout.note_width(col)),
                height: Val::Px(layout.note_height() * 1.6),
                ..default()
            },
            BackgroundColor(column_color(col).with_alpha(0.85 * strength)),
        ))
        .id();
    commands.entity(hud).add_child(burst);
}
```

In `tick_receptor_flashes`, replace `let base = lane_color(receptor.lane);` with `let base = column_color(receptor.col as usize);`.

In `apply_receptor_layout`, replace `lane`/`lane_left`/`lane_width` with column:

```rust
    for (receptor, mut node) in &mut receptors {
        let col = receptor.col as usize;
        node.left = Val::Px(layout.col_left(col) + 2.0);
        node.top = Val::Px(layout.judge_y() - 12.0 * layout.scale);
        node.width = Val::Px(layout.col_width(col) - 4.0);
        node.height = Val::Px(24.0 * layout.scale);
    }
```

- [ ] **Step 4: Build**

Run: `cargo build -p gameplay-drums`
Expected: remaining errors only in `keyboard_viz.rs`.

---

### Task 6: `keyboard_viz.rs` — pad glyphs per column, drop keybind text

**Files:**
- Modify: `crates/gameplay-drums/src/keyboard_viz.rs`

- [ ] **Step 1: Pad glyph per column, label only**

Change `KeyCap { pub lane: u8 }` → `KeyCap { pub col: u8 }`.

Add imports:

```rust
use crate::lane_geometry::{column_color, column_of, COLUMNS, COLUMN_COUNT};
use crate::lane_map::lane_channel;
```

Rewrite `spawn_key_caps` (drop the `lane_map`/`key_display` dependency for the label; keep the `lane_map` param for signature compat or remove it — see Step 3):

```rust
pub fn spawn_key_caps(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    theme: &dtx_ui::theme::Theme,
) {
    let cap_h = layout.key_cap_height();
    for col in 0..COLUMN_COUNT {
        let tint = column_color(col).with_alpha(0.18);
        commands.entity(parent).with_children(|row| {
            row.spawn((
                KeyCap { col: col as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col) + 2.0),
                    top: Val::Px(layout.key_viz_top()),
                    width: Val::Px(layout.col_width(col) - 4.0),
                    height: Val::Px(cap_h),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(tint),
                children![(
                    Text::new(COLUMNS[col].label),
                    Theme::font(13.0 * layout.scale),
                    TextColor(theme.text_primary),
                )],
            ));
        });
    }
}
```

Delete the now-unused `key_display` fn.

- [ ] **Step 2: Fix layout + flash systems for column**

`apply_key_cap_layout`:

```rust
fn apply_key_cap_layout(layout: Res<PlayfieldLayout>, mut caps: Query<(&KeyCap, &mut Node)>) {
    for (cap, mut node) in &mut caps {
        let col = cap.col as usize;
        node.left = Val::Px(layout.col_left(col) + 2.0);
        node.top = Val::Px(layout.key_viz_top());
        node.width = Val::Px(layout.col_width(col) - 4.0);
        node.height = Val::Px(layout.key_cap_height());
    }
}
```

`flash_key_caps_on_hit` — map `hit.lane`/`ev.lane` → column:

```rust
fn flash_key_caps_on_hit(
    mut lane_hits: MessageReader<LaneHit>,
    mut events: MessageReader<JudgmentEvent>,
    theme: Res<ThemeResource>,
    mut caps: Query<(&KeyCap, &mut BackgroundColor)>,
) {
    let accent = theme.0.accent;
    let to_col = |lane: u8| lane_channel(lane).and_then(column_of);
    for hit in lane_hits.read() {
        let Some(col) = to_col(hit.lane) else { continue };
        for (cap, mut bg) in &mut caps {
            if cap.col as usize == col {
                bg.0 = accent.with_alpha(0.45);
            }
        }
    }
    for ev in events.read() {
        if ev.kind == dtx_scoring::JudgmentKind::Miss { continue; }
        let Some(col) = to_col(ev.lane) else { continue };
        for (cap, mut bg) in &mut caps {
            if cap.col as usize == col {
                bg.0 = accent.with_alpha(0.55);
            }
        }
    }
}
```

`decay_key_cap_flashes` uses a hard-coded `base` color; change `base` to `Color::srgba(0.05, 0.05, 0.07, 0.6)` so it settles to a dim neutral (the per-column tint is applied at spawn; decay just needs a stable low state). Leave the rest.

- [ ] **Step 3: Update the caller signature in `hud.rs`**

`spawn_key_caps` dropped the `lane_map` param. In `hud.rs` change:

```rust
        keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &t);
```

If `lane_map` is now unused in `spawn_hud`, remove its `Res<LaneMap>` param and the `use` if the compiler flags it.

- [ ] **Step 4: Build the whole crate**

Run: `cargo build -p gameplay-drums`
Expected: PASS (0 errors). Fix any residual unused-import warnings.

- [ ] **Step 5: Commit Tasks 2–6 together (first green build)**

```bash
git add crates/gameplay-drums/src/layout.rs crates/gameplay-drums/src/scroll.rs \
        crates/gameplay-drums/src/hud.rs crates/gameplay-drums/src/playfield_viz.rs \
        crates/gameplay-drums/src/keyboard_viz.rs crates/gameplay-drums/src/beat_lines.rs
git commit -m "feat(drums): render 10 NX columns — variable widths, per-column color, column feedback"
```

---

### Task 7: `beat_lines.rs` + judgment popup center + stale tests

**Files:**
- Modify: `crates/gameplay-drums/src/beat_lines.rs`
- Modify: `crates/gameplay-drums/src/hud.rs` (judgment popup center) or the popup widget
- Modify: `crates/gameplay-drums/src/lane_map.rs` (stale test expectations)

- [ ] **Step 1: beat_lines span the strip**

In `beat_lines.rs` around line 145/147, replace:

```rust
                    left: Val::Px(layout.strip_left()),
```
```rust
                    width: Val::Px(layout.strip_width()),
```

(Was `lane_strip_left()` / `lane_strip_width()`.)

- [ ] **Step 2: Judgment popup centered on strip**

Find where `judgment_popup` positions the popup text (search `spawn_judgment_popup` / the popup's `left`/`Node`). If it centers on `ref_lane_left()+REF_LANE_FIELD_W/2` or a fixed x, set it to the strip center. In `hud.rs` where `spawn_judgment_popup(&mut commands, root, &t)` is called, if the popup reads center from layout it already follows `strip_left`+`strip_width`; otherwise pass/compute `layout.strip_left() + layout.strip_width() * 0.5`. Verify by reading `judgment_popup.rs`; adjust the horizontal anchor to `STRIP_REF_LEFT + STRIP_REF_WIDTH * 0.5 = 574.0` at ref.

- [ ] **Step 3: Fix stale `lane_map.rs` tests**

The old test `lane_order_matches_bocud` asserts the *input* lane order — that model is unchanged (12 lanes), so it still holds for `LANE_ORDER`. No change needed there. But confirm `LANE_ORDER` still compiles and `default_labels_match_lane_order` passes. If any test referenced deleted geometry, update it. Run:

Run: `cargo test -p gameplay-drums`
Expected: PASS (all, including new `lane_geometry` + `layout` tests).

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/beat_lines.rs crates/gameplay-drums/src/hud.rs \
        crates/gameplay-drums/src/lane_map.rs
git commit -m "feat(drums): beat lines + judgment popup follow NX strip"
```

---

### Task 8: Full build, workspace tests, visual verification

**Files:** none (verification)

- [ ] **Step 1: Workspace build + test**

Run: `cargo build` then `cargo test -p gameplay-drums`
Expected: build PASS, tests PASS.

- [ ] **Step 2: Run the app and verify against the reference**

Use the project run skill / binary to launch a chart. Verify:
- Column order left→right: **LC HH LP SD HT BD LT FT CY RD** (variable widths, RD narrowest, LC/CY widest).
- Notes fill their column width, ~14px tall, per-column color; HHO chips land on the HH column with a brighter tint; LBD on BD darker.
- On hit: pad glyph brightens + column flash + upward burst + judgement popup near strip center; combo pops.
- Pad row shows column labels only (no `Digit0`/`Minus` keybind text), no overlap with SPEED.
- Right region shows song-info card with COMBO below it (no clip); left region shows SCORE + stats; minimal black dead space.

- [ ] **Step 3: Screenshot check + commit any tuning**

If colors/spacing need tuning, adjust `COLUMNS[*].color` in `lane_geometry.rs` and re-run. Commit final tuning:

```bash
git add -A
git commit -m "polish(drums): color/spacing tuning to match NX reference"
```

---

## Self-Review Notes

- **Spec coverage:** lane order + 10 columns (T1,T2,T4), HHO/LBD merge (T1,T3), variable widths (T1,T2), per-column color (T1,T3), bigger notes (T2,T3), pad glyphs+flash+burst+popup+combo (T5,T6,T7), panel reposition + kill debug row + fix clip + dead space (T4,T6). All covered.
- **Type consistency:** `col`/`column_of`/`col_left`/`col_width`/`chip_color`/`column_color`/`COLUMN_COUNT`/`COLUMNS`/`STRIP_REF_LEFT`/`STRIP_REF_WIDTH` used consistently across tasks. `LaneReceptor.col`, `KeyCap.col`, `LaneColumn.col` all renamed from `lane`.
- **Risk:** exact `EChannel` variant spelling (`LeftBassDrum`, `HiHatOpen`, `LeftPedal`, `LeftCymbal`) must match `dtx_core` — Task 1 Step 2 catches mismatches at compile. `spawn_playfield_speed`/`spawn_song_progress`/`now_playing`/`perf_combo` signatures assumed unchanged; if a widget hard-codes width from `REF_LANE_FIELD_W`, adjust at call site only.
