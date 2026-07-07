# Editor Lane Panel Implementation Plan (v2 plan 3 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Selecting the Playfield shows a lane management panel: preset cycle, per-lane reorder (▲▼), width slider, chip-click split (ungroup a channel into its own lane), and ✕ merge (remap a lane's channels onto a neighbor) — display axis only, no judgment changes.

**Architecture:** Pure lane-transform functions on `LaneArrangement` live in dtx-layout (`lane_edit.rs`, no bevy) — reorder/set-width/split/merge, every mutation flipping `preset` to `Custom` and preserving the invariant that all 12 drum channels stay mapped to an existing lane. The editor's settings panel (plan 2 `panel.rs`) grows a Playfield branch that renders lane rows and routes button/slider events through those functions into the `Lanes` resource — the existing reactive chain (`PlayfieldLayout` → chips/keycaps respawn) and undo/save cover the rest for free. Panel rebuilds only on *structural* change (row count / chip sets / preset), not on width drags, so slider entities survive their own gestures.

**Tech Stack:** Rust, Bevy 0.19, dtx-ui controls from plan 2 (`Slider`, `ControlValue`, spawn helpers).

**Spec:** `docs/superpowers/specs/2026-07-07-layout-editor-v2-design.md` (section 5 lane block). Explicitly OUT: hit-group (judgment) UI — `DrumsConfig`/`drum_groups.rs` untouched.

**Branch:** `feat/editor-lane-panel` off `main` (after plan 2 merged).

**Existing context:**
- `crates/dtx-layout/src/lanes.rs` — `DisplayLane { id, label, width, color, primary }`, `LaneArrangement { preset, lanes, map }`, `lane_index_of`, `is_secondary`, `channel_short_name`/`channel_from_short`, `default_lane_width`, `DRUM_CHANNELS` (12), `MIN/MAX_LANE_WIDTH` (24/160). Lane ids are ALWAYS channel short names.
- `crates/dtx-layout/src/presets.rs` — `LanePreset { Classic, NxTypeB, NxTypeD, Custom }`, `arrangement_for(preset)`.
- `crates/dtx-layout/src/file.rs` — `LanesSection::{resolve, from_arrangement}`; resolver repairs unmapped channels (`lanes[0]` fallback) — the edit ops must never rely on that repair, they maintain the invariant themselves.
- `crates/gameplay-drums/src/lanes.rs` — `Lanes(pub dtx_layout::LaneArrangement)` resource.
- `crates/gameplay-drums/src/editor/panel.rs` (plan 2) — `rebuild_panel` early-returns on `WidgetKind::Playfield` with a `// plan 3 adds the lane panel here` comment; `PanelRoot`, `EditorChrome`, `row(...)` helper, undo pattern.
- `crates/gameplay-drums/src/editor/save.rs` — `next_lane_preset` (cycles named presets).
- `crates/gameplay-drums/src/editor/ui.rs` — left sidebar (NextPreset button was removed in plan 2).
- `sync_playfield_layout` (layout.rs) + chip/keycap respawn systems already react to `Lanes` changes (v1 plan 1) — no render work needed here.
- rustfmt gotcha: NEVER bare `cargo fmt --all`. 16-plugin tuple limit.

---

## File Structure

- Create: `crates/dtx-layout/src/lane_edit.rs` — pure transforms + tests.
- Modify: `crates/dtx-layout/src/lib.rs` — `pub mod lane_edit;` + re-exports.
- Modify: `crates/gameplay-drums/src/editor/panel.rs` — Playfield branch (lane rows) + handlers.
- Test: `crates/gameplay-drums/tests/editor_lanes.rs`.

### Task 0: Branch

- [ ] **Step 0.1:**

```bash
cd /home/lazykern/lab/dtxmaniars && git checkout -b feat/editor-lane-panel main
```

### Task 1: dtx-layout lane_edit.rs — pure transforms

**Files:**
- Create: `crates/dtx-layout/src/lane_edit.rs`
- Modify: `crates/dtx-layout/src/lib.rs`

- [ ] **Step 1.1: Write the module**

```rust
//! Editor-facing lane transforms. Pure (no bevy). Every mutation flips the
//! arrangement to `LanePreset::Custom` and maintains the invariant: all 12
//! drum channels map to a lane id present in `lanes`, and `lanes` is non-empty.

use dtx_core::EChannel;

use crate::lanes::{
    channel_short_name, default_lane_width, DisplayLane, LaneArrangement, MAX_LANE_WIDTH,
    MIN_LANE_WIDTH,
};
use crate::presets::LanePreset;

/// Swap lane `index` with its neighbor in `dir` (-1 left, +1 right).
/// Returns false (no-op) when the move would leave the strip.
pub fn reorder_lane(arr: &mut LaneArrangement, index: usize, dir: i32) -> bool {
    let Some(target) = index.checked_add_signed(dir as isize) else {
        return false;
    };
    if index >= arr.lanes.len() || target >= arr.lanes.len() {
        return false;
    }
    arr.lanes.swap(index, target);
    arr.preset = LanePreset::Custom;
    true
}

/// Clamp + set lane width (ref px).
pub fn set_lane_width(arr: &mut LaneArrangement, index: usize, width: f32) -> bool {
    let Some(lane) = arr.lanes.get_mut(index) else {
        return false;
    };
    let clamped = width.clamp(MIN_LANE_WIDTH, MAX_LANE_WIDTH);
    if (lane.width - clamped).abs() < f32::EPSILON {
        return false;
    }
    lane.width = clamped;
    arr.preset = LanePreset::Custom;
    true
}

/// Split a secondary channel out of its host lane into its own new lane,
/// inserted directly after the host. No-op when `ch` already has its own lane
/// (is primary) or isn't a drum channel.
pub fn split_channel(arr: &mut LaneArrangement, ch: EChannel) -> bool {
    let Some(name) = channel_short_name(ch) else {
        return false;
    };
    let Some(host) = arr.lane_index_of(ch) else {
        return false;
    };
    if arr.lanes[host].primary == ch {
        return false;
    }
    // Degenerate state guard: a lane with this id already exists but the
    // channel points elsewhere — just remap.
    if arr.lanes.iter().any(|l| l.id == name) {
        arr.map.insert(ch, name.to_string());
        arr.preset = LanePreset::Custom;
        return true;
    }
    arr.lanes.insert(
        host + 1,
        DisplayLane {
            id: name.to_string(),
            label: name.to_string(),
            width: default_lane_width(ch),
            color: None,
            primary: ch,
        },
    );
    arr.map.insert(ch, name.to_string());
    arr.preset = LanePreset::Custom;
    true
}

/// Remove lane `index`, remapping every channel it hosted onto the left
/// neighbor (right neighbor when leftmost). No-op on the last remaining lane.
pub fn merge_lane(arr: &mut LaneArrangement, index: usize) -> bool {
    if arr.lanes.len() <= 1 || index >= arr.lanes.len() {
        return false;
    }
    let target_idx = if index > 0 { index - 1 } else { index + 1 };
    let target_id = arr.lanes[target_idx].id.clone();
    let removed = arr.lanes.remove(index);
    for id in arr.map.values_mut() {
        if *id == removed.id {
            *id = target_id.clone();
        }
    }
    arr.preset = LanePreset::Custom;
    true
}

/// Channels mapped to lane `index`, primary first, rest in DRUM_CHANNELS order.
pub fn lane_chips(arr: &LaneArrangement, index: usize) -> Vec<EChannel> {
    let Some(lane) = arr.lanes.get(index) else {
        return Vec::new();
    };
    let mut chips: Vec<EChannel> = crate::lanes::DRUM_CHANNELS
        .into_iter()
        .filter(|ch| arr.map.get(ch) == Some(&lane.id))
        .collect();
    chips.sort_by_key(|ch| (*ch != lane.primary,));
    chips
}

/// Structural signature: changes when rows / chip sets / preset change (used
/// by the editor to know when to rebuild the panel vs just refresh values).
pub fn structure_signature(arr: &LaneArrangement) -> String {
    use std::fmt::Write;
    let mut s = format!("{:?}|", arr.preset);
    for (i, lane) in arr.lanes.iter().enumerate() {
        let _ = write!(s, "{};", lane.id);
        for ch in lane_chips(arr, i) {
            let _ = write!(s, "{},", channel_short_name(ch).unwrap_or("?"));
        }
        s.push('|');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::DRUM_CHANNELS;
    use crate::presets::classic;

    fn assert_invariant(arr: &LaneArrangement) {
        assert!(!arr.lanes.is_empty());
        for ch in DRUM_CHANNELS {
            let idx = arr.lane_index_of(ch);
            assert!(idx.is_some(), "{ch:?} unmapped");
        }
    }

    #[test]
    fn reorder_swaps_and_flips_custom() {
        let mut arr = classic();
        let first = arr.lanes[0].id.clone();
        let second = arr.lanes[1].id.clone();
        assert!(reorder_lane(&mut arr, 0, 1));
        assert_eq!(arr.lanes[0].id, second);
        assert_eq!(arr.lanes[1].id, first);
        assert_eq!(arr.preset, LanePreset::Custom);
        assert_invariant(&arr);
    }

    #[test]
    fn reorder_off_the_edge_is_noop() {
        let mut arr = classic();
        assert!(!reorder_lane(&mut arr, 0, -1));
        let last = arr.lanes.len() - 1;
        assert!(!reorder_lane(&mut arr, last, 1));
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn width_clamps() {
        let mut arr = classic();
        assert!(set_lane_width(&mut arr, 0, 999.0));
        assert_eq!(arr.lanes[0].width, MAX_LANE_WIDTH);
        assert!(set_lane_width(&mut arr, 0, 1.0));
        assert_eq!(arr.lanes[0].width, MIN_LANE_WIDTH);
    }

    #[test]
    fn split_hho_out_of_hh_lane() {
        let mut arr = classic();
        let before = arr.lanes.len();
        assert!(arr.is_secondary(EChannel::HiHatOpen));
        assert!(split_channel(&mut arr, EChannel::HiHatOpen));
        assert_eq!(arr.lanes.len(), before + 1);
        assert!(!arr.is_secondary(EChannel::HiHatOpen));
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let hho = arr.lane_index_of(EChannel::HiHatOpen).unwrap();
        assert_eq!(hho, hh + 1, "split lane inserted right after host");
        assert_invariant(&arr);
    }

    #[test]
    fn split_primary_is_noop() {
        let mut arr = classic();
        assert!(!split_channel(&mut arr, EChannel::Snare));
        assert_eq!(arr.preset, LanePreset::Classic);
    }

    #[test]
    fn merge_rd_moves_chips_to_left_neighbor() {
        let mut arr = classic();
        let rd_idx = arr.lane_index_of(EChannel::RideCymbal).unwrap();
        let left_id = arr.lanes[rd_idx - 1].id.clone();
        let before = arr.lanes.len();
        assert!(merge_lane(&mut arr, rd_idx));
        assert_eq!(arr.lanes.len(), before - 1);
        assert_eq!(arr.map[&EChannel::RideCymbal], left_id);
        assert_invariant(&arr);
    }

    #[test]
    fn merge_leftmost_uses_right_neighbor() {
        let mut arr = classic();
        let right_id = arr.lanes[1].id.clone();
        let first_primary = arr.lanes[0].primary;
        assert!(merge_lane(&mut arr, 0));
        assert_eq!(arr.map[&first_primary], right_id);
        assert_invariant(&arr);
    }

    #[test]
    fn merge_last_lane_refused() {
        let mut arr = classic();
        while arr.lanes.len() > 1 {
            assert!(merge_lane(&mut arr, 0));
        }
        assert!(!merge_lane(&mut arr, 0));
        assert_invariant(&arr);
    }

    #[test]
    fn chips_list_primary_first() {
        let arr = classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let chips = lane_chips(&arr, hh);
        assert_eq!(chips[0], arr.lanes[hh].primary);
        assert!(chips.contains(&EChannel::HiHatOpen));
    }

    #[test]
    fn signature_changes_on_structure_not_width() {
        let mut arr = classic();
        let sig0 = structure_signature(&arr);
        set_lane_width(&mut arr, 0, 100.0);
        // Width change flips preset → signature changes once (Custom), then
        // further width edits keep it stable.
        let sig1 = structure_signature(&arr);
        assert_ne!(sig0, sig1);
        set_lane_width(&mut arr, 0, 120.0);
        assert_eq!(sig1, structure_signature(&arr));
        split_channel(&mut arr, EChannel::HiHatOpen);
        assert_ne!(sig1, structure_signature(&arr));
    }

    #[test]
    fn edited_arrangement_round_trips_through_file() {
        let mut arr = classic();
        split_channel(&mut arr, EChannel::HiHatOpen);
        reorder_lane(&mut arr, 0, 1);
        merge_lane(&mut arr, arr.lanes.len() - 1);
        set_lane_width(&mut arr, 2, 88.0);
        let section = crate::LanesSection::from_arrangement(&arr);
        assert_eq!(section.resolve(), arr);
    }
}
```

- [ ] **Step 1.2: lib.rs** — add `pub mod lane_edit;` and re-export:

```rust
pub use lane_edit::{
    lane_chips, merge_lane, reorder_lane, set_lane_width, split_channel, structure_signature,
};
```

- [ ] **Step 1.3: Run + commit**

Run: `cargo test -p dtx-layout lane_edit` → 11 PASS.

```bash
git add crates/dtx-layout/
git commit -m "feat(dtx-layout): lane edit transforms (reorder/width/split/merge)"
```

### Task 2: panel.rs — Playfield lane block UI

**Files:**
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

- [ ] **Step 2.1: Marker components + preset label helper**

```rust
/// Lane panel controls (Playfield selected).
#[derive(Component, Debug, Clone, Copy)]
pub struct LaneReorderBtn {
    pub index: usize,
    pub dir: i32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LaneMergeBtn(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct ChipSplitBtn(pub dtx_core::EChannel);

#[derive(Component, Debug, Clone, Copy)]
pub struct LaneWidthSlider(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct PresetCycleBtn(pub i32);

#[derive(Component)]
pub struct PresetLabel;

fn preset_name(p: dtx_layout::LanePreset) -> &'static str {
    match p {
        dtx_layout::LanePreset::Classic => "classic",
        dtx_layout::LanePreset::NxTypeB => "nx type-b",
        dtx_layout::LanePreset::NxTypeD => "nx type-d",
        dtx_layout::LanePreset::Custom => "custom",
    }
}
```

(`dtx_core` may not be a direct dependency of gameplay-drums — check `crates/gameplay-drums/Cargo.toml`; it is (chart channels). Import as used elsewhere in the crate.)

- [ ] **Step 2.2: Replace the Playfield early-return in `rebuild_panel` with the lane block**

Replace:

```rust
    if kind == WidgetKind::Playfield {
        return; // plan 3 adds the lane panel here
    }
```

with a branch that spawns the lane panel instead of the generic block (the generic rows stay for every other kind — restructure `rebuild_panel`'s `with_children` closure into `if kind == WidgetKind::Playfield { spawn_lane_block(p, &t, &lanes); } else { /* existing generic rows */ }`; `rebuild_panel` gains `lanes: Res<Lanes>`):

```rust
fn spawn_lane_block(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, lanes: &Lanes) {
    p.spawn((
        Text::new("Lanes"),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(t.text_primary),
    ));

    // Preset row: ◄ name ►
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            PresetCycleBtn(-1),
            Button,
            Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(Text::new("<"), dtx_ui::theme::Theme::font(12.0), TextColor(t.text_primary))],
        ));
        r.spawn((
            PresetLabel,
            Text::new(preset_name(lanes.0.preset).to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
            Node { min_width: Val::Px(70.0), ..default() },
        ));
        r.spawn((
            PresetCycleBtn(1),
            Button,
            Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(Text::new(">"), dtx_ui::theme::Theme::font(12.0), TextColor(t.text_primary))],
        ));
    });

    // One row per lane: [▲][▼] ID (chips…) width-slider [✕]
    let last = lanes.0.lanes.len().saturating_sub(1);
    for (i, lane) in lanes.0.lanes.iter().enumerate() {
        let chips = dtx_layout::lane_chips(&lanes.0, i);
        let can_merge = lanes.0.lanes.len() > 1;
        let width = lane.width;
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::vertical(Val::Px(2.0)),
            ..default()
        })
        .with_children(|lane_col| {
            lane_col
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|r| {
                    for (dir, sym, enabled) in
                        [(-1, "^", i > 0), (1, "v", i < last)]
                    {
                        if enabled {
                            r.spawn((
                                LaneReorderBtn { index: i, dir },
                                Button,
                                Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..default() },
                                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                                children![(Text::new(sym), dtx_ui::theme::Theme::font(11.0), TextColor(t.text_primary))],
                            ));
                        } else {
                            r.spawn((
                                Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..default() },
                                children![(Text::new(sym), dtx_ui::theme::Theme::font(11.0), TextColor(t.text_secondary))],
                            ));
                        }
                    }
                    r.spawn((
                        Text::new(lane.id.clone()),
                        dtx_ui::theme::Theme::font(12.0),
                        TextColor(t.text_primary),
                        Node { min_width: Val::Px(34.0), ..default() },
                    ));
                    // Chips: primary shown flat; secondaries are split buttons.
                    for ch in &chips {
                        let name = dtx_layout::channel_short_name(*ch).unwrap_or("?");
                        if *ch == lane.primary {
                            r.spawn((
                                Text::new(name),
                                dtx_ui::theme::Theme::font(10.0),
                                TextColor(t.text_secondary),
                            ));
                        } else {
                            r.spawn((
                                ChipSplitBtn(*ch),
                                Button,
                                Node { padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)), ..default() },
                                BackgroundColor(Color::srgb(0.18, 0.22, 0.28)),
                                children![(
                                    Text::new(format!("{name} x")),
                                    dtx_ui::theme::Theme::font(10.0),
                                    TextColor(t.text_primary),
                                )],
                            ));
                        }
                    }
                    if can_merge {
                        r.spawn((
                            LaneMergeBtn(i),
                            Button,
                            Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..default() },
                            BackgroundColor(Color::srgb(0.3, 0.14, 0.14)),
                            children![(Text::new("x"), dtx_ui::theme::Theme::font(11.0), TextColor(t.text_primary))],
                        ));
                    }
                });
            lane_col
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|r| {
                    let e = controls::spawn_slider(
                        r,
                        t,
                        Slider { min: dtx_layout::MIN_LANE_WIDTH, max: dtx_layout::MAX_LANE_WIDTH },
                        width,
                    );
                    r.commands_mut().entity(e).insert(LaneWidthSlider(i));
                });
        });
    }
}
```

(Verify `MIN_LANE_WIDTH`/`MAX_LANE_WIDTH` are re-exported from `dtx_layout`; add to lib.rs re-exports if not.)

- [ ] **Step 2.3: Structural rebuild trigger**

`rebuild_panel` currently runs on `resource_changed::<Selection>` / `EditorOpen`. Lane edits change `Lanes` (structure) — the panel must rebuild then too, but NOT on width-slider drags. Change the run condition to always run and gate inside with a signature check:

```rust
fn rebuild_panel(
    /* existing params */
    lanes: Res<Lanes>,
    mut last_sig: Local<Option<(Option<WidgetKind>, bool, String)>>,
) {
    let sig = (
        selection.0,
        open.0,
        dtx_layout::structure_signature(&lanes.0),
    );
    if last_sig.as_ref() == Some(&sig) {
        return;
    }
    *last_sig = Some(sig);
    // ... existing despawn + rebuild body ...
}
```

and register it plainly (`.run_if(in_state(...))` only, drop the `resource_changed` combinators — the Local guard debounces). The rebuild body itself is unchanged apart from the Playfield branch.

- [ ] **Step 2.4: Build**

Run: `cargo build -p gameplay-drums 2>&1 | tail -5` → clean (handlers come next; unused-marker warnings acceptable until Task 3).

- [ ] **Step 2.5: Commit**

```bash
git add crates/gameplay-drums/src/editor/panel.rs
git commit -m "feat(editor): lane panel UI (preset, reorder, chips, width, merge)"
```

### Task 3: Handlers

**Files:**
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

- [ ] **Step 3.1: Button handler (structural ops + preset)**

```rust
/// Preset cycle order for the ◄ ► buttons (named presets only; any manual
/// edit lands on Custom via the transforms).
const PRESET_ORDER: [dtx_layout::LanePreset; 3] = [
    dtx_layout::LanePreset::Classic,
    dtx_layout::LanePreset::NxTypeB,
    dtx_layout::LanePreset::NxTypeD,
];

fn handle_lane_buttons(
    reorders: Query<(&LaneReorderBtn, &Interaction), Changed<Interaction>>,
    merges: Query<(&LaneMergeBtn, &Interaction), Changed<Interaction>>,
    splits: Query<(&ChipSplitBtn, &Interaction), Changed<Interaction>>,
    presets: Query<(&PresetCycleBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    let mut mutate: Option<Box<dyn FnOnce(&mut dtx_layout::LaneArrangement) -> bool>> = None;
    for (btn, i) in &reorders {
        if *i == Interaction::Pressed {
            let (index, dir) = (btn.index, btn.dir);
            mutate = Some(Box::new(move |arr| dtx_layout::reorder_lane(arr, index, dir)));
        }
    }
    for (btn, i) in &merges {
        if *i == Interaction::Pressed {
            let index = btn.0;
            mutate = Some(Box::new(move |arr| dtx_layout::merge_lane(arr, index)));
        }
    }
    for (btn, i) in &splits {
        if *i == Interaction::Pressed {
            let ch = btn.0;
            mutate = Some(Box::new(move |arr| dtx_layout::split_channel(arr, ch)));
        }
    }
    for (btn, i) in &presets {
        if *i == Interaction::Pressed {
            let dir = btn.0;
            mutate = Some(Box::new(move |arr| {
                let cur = PRESET_ORDER.iter().position(|p| *p == arr.preset);
                let next = match cur {
                    Some(idx) => {
                        let n = PRESET_ORDER.len() as i32;
                        PRESET_ORDER[((idx as i32 + dir).rem_euclid(n)) as usize]
                    }
                    // From Custom: either direction lands on Classic.
                    None => dtx_layout::LanePreset::Classic,
                };
                *arr = dtx_layout::arrangement_for(next);
                true
            }));
        }
    }
    if let Some(f) = mutate {
        // Snapshot BEFORE mutating; drop the snapshot if the op was a no-op.
        let before = super::undo::Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        };
        if f(&mut lanes.0) {
            undo.push_snapshot(before);
        }
    }
}
```

`UndoStack::push_snapshot` doesn't exist yet — add to `crates/gameplay-drums/src/editor/undo.rs`:

```rust
    /// Push a pre-built snapshot (callers that must snapshot before a
    /// conditional mutation).
    pub fn push_snapshot(&mut self, snap: Snapshot) {
        self.past.push(snap);
        if self.past.len() > MAX_HISTORY {
            self.past.remove(0);
        }
        self.future.clear();
    }
```

and refactor the existing `push(&layouts, &lanes)` to delegate: `self.push_snapshot(Snapshot { layouts: layouts.clone(), lanes: lanes.clone() })` (check the existing body first — keep behavior identical).

- [ ] **Step 3.2: Width slider apply + refresh**

```rust
/// Width slider → Lanes. One undo snapshot per mouse-hold.
fn apply_lane_width_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    sliders: Query<(&LaneWidthSlider, &ControlValue), Changed<ControlValue>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
    mut snapped_this_hold: Local<bool>,
) {
    if !buttons.pressed(MouseButton::Left) {
        *snapped_this_hold = false;
    }
    let mut pending: Vec<(usize, f32)> = Vec::new();
    for (slider, value) in &sliders {
        let idx = slider.0;
        let differs = lanes
            .0
            .lanes
            .get(idx)
            .map(|l| (l.width - value.0).abs() > 0.01)
            .unwrap_or(false);
        if differs {
            pending.push((idx, value.0));
        }
    }
    if pending.is_empty() {
        return;
    }
    if !*snapped_this_hold {
        undo.push(&layouts, &lanes);
        *snapped_this_hold = true;
    }
    for (idx, w) in pending {
        dtx_layout::set_lane_width(&mut lanes.0, idx, w);
    }
}

/// External Lanes changes (undo, preset) → refresh slider values + preset
/// label. Equality-guarded to terminate the write-back loop.
fn refresh_lane_panel_values(
    lanes: Res<Lanes>,
    mut sliders: Query<(&LaneWidthSlider, &mut ControlValue)>,
    mut preset_label: Query<&mut Text, With<PresetLabel>>,
) {
    if !lanes.is_changed() {
        return;
    }
    for (slider, mut value) in &mut sliders {
        if let Some(lane) = lanes.0.lanes.get(slider.0) {
            if (value.0 - lane.width).abs() > 0.01 {
                value.0 = lane.width;
            }
        }
    }
    if let Ok(mut text) = preset_label.single_mut() {
        let want = preset_name(lanes.0.preset);
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}
```

- [ ] **Step 3.3: Register the three systems** in `panel::plugin`'s editor-open group:

```rust
            (
                apply_panel_controls,
                apply_anchor_cells,
                handle_reset,
                refresh_panel_values,
                handle_lane_buttons,
                apply_lane_width_sliders,
                refresh_lane_panel_values,
            )
                .run_if(super::editor_open),
```

- [ ] **Step 3.4: Tests + commit**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5` → PASS.

```bash
git add crates/gameplay-drums/src/editor/
git commit -m "feat(editor): lane panel handlers (reorder/split/merge/width/preset)"
```

### Task 4: Integration tests

**Files:**
- Create: `crates/gameplay-drums/tests/editor_lanes.rs`

- [ ] **Step 4.1: Tests**

```rust
//! Lane panel transforms end-to-end through the Lanes resource + save path.

use dtx_core::EChannel;
use dtx_layout::{
    lane_chips, merge_lane, reorder_lane, set_lane_width, split_channel, structure_signature,
    LanePreset,
};
use gameplay_drums::editor::save::layout_file_from;
use gameplay_drums::lanes::Lanes;
use gameplay_drums::widget_layout::WidgetLayouts;

#[test]
fn split_then_save_then_reload_preserves_arrangement() {
    let mut lanes = Lanes::default();
    assert!(split_channel(&mut lanes.0, EChannel::HiHatOpen));
    assert!(reorder_lane(&mut lanes.0, 0, 1));
    assert!(set_lane_width(&mut lanes.0, 3, 90.0));
    let file = layout_file_from(&WidgetLayouts::default(), &lanes);
    let resolved = file.lanes.resolve();
    assert_eq!(resolved, lanes.0);
    assert_eq!(resolved.preset, LanePreset::Custom);
}

#[test]
fn merge_then_split_round_trips_channel_home() {
    let mut lanes = Lanes::default();
    let rd = lanes.0.lane_index_of(EChannel::RideCymbal).unwrap();
    assert!(merge_lane(&mut lanes.0, rd));
    // RD now a secondary chip on CY lane.
    assert!(lanes.0.is_secondary(EChannel::RideCymbal));
    let cy = lanes.0.lane_index_of(EChannel::Cymbal).unwrap();
    assert!(lane_chips(&lanes.0, cy).contains(&EChannel::RideCymbal));
    // Split it back out.
    assert!(split_channel(&mut lanes.0, EChannel::RideCymbal));
    assert!(!lanes.0.is_secondary(EChannel::RideCymbal));
}

#[test]
fn playfield_layout_tracks_lane_edits() {
    use gameplay_drums::layout::PlayfieldLayout;
    let mut lanes = Lanes::default();
    let before = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    split_channel(&mut lanes.0, EChannel::HiHatOpen);
    let after = PlayfieldLayout::from_size(1280.0, 720.0, &lanes);
    assert_eq!(after.col_count(), before.col_count() + 1);
    assert!(after.strip_width() > before.strip_width());
}

#[test]
fn signature_stable_across_width_drag() {
    let mut lanes = Lanes::default();
    set_lane_width(&mut lanes.0, 0, 60.0);
    let sig = structure_signature(&lanes.0);
    set_lane_width(&mut lanes.0, 0, 61.0);
    set_lane_width(&mut lanes.0, 0, 62.0);
    assert_eq!(sig, structure_signature(&lanes.0));
}
```

- [ ] **Step 4.2: Run**

Run: `cargo test -p gameplay-drums --test editor_lanes` → 4 PASS.

- [ ] **Step 4.3: Commit**

```bash
git add crates/gameplay-drums/tests/editor_lanes.rs
git commit -m "test(editor): lane panel transform integration tests"
```

### Task 5: Real-binary verification

- [ ] **Step 5.1:** `cargo test --workspace 2>&1 | tail -8` → all PASS.
- [ ] **Step 5.2:** `timeout 40 cargo run 2>&1 | tail -20; echo "exit=$?"` → `exit=124`, no panic/cycle.
- [ ] **Step 5.3:** Report DONE + manual checklist:
  - Select Playfield (click the lane strip) → lane panel replaces the generic block.
  - `^`/`v` reorder lanes; chips move with their lane; playfield re-centers.
  - Click `HHO x` chip on the HH lane → HHO gets its own lane next to HH (hollow chips become solid).
  - Click `x` on the RD lane → RD chips draw hollow on the CY lane.
  - Width slider fattens/thins a lane live.
  - `<`/`>` preset cycle: classic → nx type-b → nx type-d; any manual edit flips the label to "custom".
  - Ctrl+Z undoes each lane op; Ctrl+S then restart → arrangement persists.
