# Widget Registry + HUD Layout Persistence (Layout Pillar, Plan 2 of 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Externalize gameplay HUD widget placement into a serializable `[scene.gameplay]` layout with a widget registry, per-widget offset/anchor/scale/visibility/z, applied at runtime via per-widget container nodes — default layout byte-identical to today.

**Architecture:** `dtx-layout` gains the widget data model (`WidgetKind`, `Anchor9`, `WidgetInstance`, `SceneSection`), pure anchor-resolution math, and a `scene` section in `LayoutFile`. `gameplay-drums` wraps each HUD widget's children in a per-widget **container** `Node` (absolute at ref-origin 0,0, full-size) tagged with `WidgetContainer{kind}`; a `WidgetLayouts` resource (code defaults ⊕ file) drives an apply system that sets each container's `left/top` (offset·scale), `ZIndex`, and visibility. Because absolute children position relative to their container's box, moving the container translates the whole widget as one unit; at the default offset (0,0) every widget lands exactly where it does today.

**Tech Stack:** Rust, Bevy 0.19, serde + toml. Parent spec: `docs/superpowers/specs/2026-07-07-layout-editor-design.md`. Builds on plan 1 (dtx-layout crate, layout.toml already exists with `[lanes]`).

**Design refinement vs spec:** The spec described "wrap each widget in a single root node; children root-relative (subtract design origin)." This plan uses the equivalent **container-at-origin** technique instead: the container sits at ref (0,0) full-size, children keep their existing absolute ref coords unchanged, and the container's `left/top` is the widget's offset. This delivers the same "move as a unit" behaviour with **zero changes to any widget's internal coordinates** (only `hud.rs` wiring changes), which guarantees parity and avoids touching 10 widget files. Anchor/origin are fully modeled + resolved (pure, tested) for the editor (plan 3); in plan 2 runtime, anchor governs resize/snapping semantics but with the existing uniform scale the applied position reduces to `default + offset` — documented at the apply system.

**Project gotchas:**
- NEVER `cargo fmt --all`/`-p` (reformats ~26 files). Only `rustfmt <new-file>`; then `git status` + `git checkout --` strays.
- Green tests don't prove the real FixedUpdate schedule builds. This plan adds only Update/OnEnter systems — no FixedUpdate edges. If you add any `.before/.after` in FixedUpdate, extend `crates/gameplay-drums/tests/fixed_update_schedule_ordering.rs`.
- `PlayfieldLayout` is no longer `Copy` (holds a Vec) — pass by ref.

---

## File Structure

```
crates/dtx-layout/
  src/widgets.rs      NEW — WidgetKind, Anchor9, WidgetInstance, anchor math, defaults
  src/scene.rs        NEW — SceneSection (serde [scene.gameplay]) + resolution
  src/file.rs         MODIFY — LayoutFile gains `scene: SceneSection`
  src/lib.rs          MODIFY — module decls + re-exports

crates/gameplay-drums/
  src/widget_layout.rs  NEW — WidgetContainer component, WidgetLayouts resource,
                        apply system (offset/z/visibility), startup load
  src/hud.rs            MODIFY — wrap each spawn_* under a container; register plugin
  src/lib.rs            MODIFY — add widget_layout plugin/module
  tests/widget_layout.rs  NEW — integration: default parity, offset shift, visibility
```

---

### Task 1: Anchor9 + WidgetKind + anchor math (dtx-layout)

**Files:** Create `crates/dtx-layout/src/widgets.rs`; modify `crates/dtx-layout/src/lib.rs`.

- [ ] **Step 1: Write `widgets.rs` with tests-first, then impl. Full file:**

```rust
//! HUD widget placement model (display/arrangement axis for widgets).
//!
//! Anchor/origin use a 3×3 grid; `resolve_offset` computes the ref-px top-left
//! of a widget given its anchor, origin, natural size, and offset within a
//! parent rect. Pure — no bevy.

use serde::{Deserialize, Serialize};

/// 9-point anchor/origin grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor9 {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor9 {
    /// Fractional position within a unit rect: (0,0)=TopLeft .. (1,1)=BottomRight.
    pub fn frac(self) -> (f32, f32) {
        let x = match self {
            Anchor9::TopLeft | Anchor9::CenterLeft | Anchor9::BottomLeft => 0.0,
            Anchor9::TopCenter | Anchor9::Center | Anchor9::BottomCenter => 0.5,
            Anchor9::TopRight | Anchor9::CenterRight | Anchor9::BottomRight => 1.0,
        };
        let y = match self {
            Anchor9::TopLeft | Anchor9::TopCenter | Anchor9::TopRight => 0.0,
            Anchor9::CenterLeft | Anchor9::Center | Anchor9::CenterRight => 0.5,
            Anchor9::BottomLeft | Anchor9::BottomCenter | Anchor9::BottomRight => 1.0,
        };
        (x, y)
    }

    pub const ALL: [Anchor9; 9] = [
        Anchor9::TopLeft,
        Anchor9::TopCenter,
        Anchor9::TopRight,
        Anchor9::CenterLeft,
        Anchor9::Center,
        Anchor9::CenterRight,
        Anchor9::BottomLeft,
        Anchor9::BottomCenter,
        Anchor9::BottomRight,
    ];
}

/// Which anchor space the widget lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorSpace {
    /// Anchored to the full screen ref rect (1280×720).
    Screen,
    /// Anchored to the playfield strip rect (dynamic; resolved by the consumer).
    Playfield,
}

impl Default for AnchorSpace {
    fn default() -> Self {
        Self::Screen
    }
}

/// The gameplay HUD widgets that can be arranged. Serialized kebab-case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetKind {
    ScorePanel,
    Combo,
    JudgmentPopup,
    PhraseMeter,
    SongProgress,
    NowPlaying,
    LiveGraph,
    SpeedReadout,
    FrameChrome,
    PracticeTransport,
    Playfield,
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 11] = [
        WidgetKind::ScorePanel,
        WidgetKind::Combo,
        WidgetKind::JudgmentPopup,
        WidgetKind::PhraseMeter,
        WidgetKind::SongProgress,
        WidgetKind::NowPlaying,
        WidgetKind::LiveGraph,
        WidgetKind::SpeedReadout,
        WidgetKind::FrameChrome,
        WidgetKind::PracticeTransport,
        WidgetKind::Playfield,
    ];

    /// Human-readable name for the editor sidebar.
    pub fn display_name(self) -> &'static str {
        match self {
            WidgetKind::ScorePanel => "Score Panel",
            WidgetKind::Combo => "Combo",
            WidgetKind::JudgmentPopup => "Judgment Popup",
            WidgetKind::PhraseMeter => "Phrase Meter",
            WidgetKind::SongProgress => "Song Progress",
            WidgetKind::NowPlaying => "Now Playing",
            WidgetKind::LiveGraph => "Live Graph",
            WidgetKind::SpeedReadout => "Speed Readout",
            WidgetKind::FrameChrome => "Frame Chrome",
            WidgetKind::PracticeTransport => "Practice Transport",
            WidgetKind::Playfield => "Playfield",
        }
    }
}

pub const MIN_WIDGET_SCALE: f32 = 0.25;
pub const MAX_WIDGET_SCALE: f32 = 3.0;

/// A placed widget instance (one per kind in v1).
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetInstance {
    pub kind: WidgetKind,
    pub space: AnchorSpace,
    pub anchor: Anchor9,
    pub origin: Anchor9,
    /// Ref-px offset from the anchored/origin-aligned base position.
    pub offset: (f32, f32),
    /// Uniform scale, clamped [MIN_WIDGET_SCALE, MAX_WIDGET_SCALE].
    pub scale: f32,
    pub z: i32,
    pub visible_play: bool,
    pub visible_practice: bool,
}

/// Resolve the ref-px top-left of a widget of natural size `size` placed at
/// `offset` with `anchor`/`origin` inside a parent rect `parent` (x, y, w, h).
///
/// anchor point A = parent.origin + anchor.frac * parent.size
/// origin point O within the widget = origin.frac * (size * scale)
/// top-left = A + offset - O
pub fn resolve_top_left(
    anchor: Anchor9,
    origin: Anchor9,
    size: (f32, f32),
    scale: f32,
    offset: (f32, f32),
    parent: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (px, py, pw, ph) = parent;
    let (af_x, af_y) = anchor.frac();
    let (of_x, of_y) = origin.frac();
    let ax = px + af_x * pw;
    let ay = py + af_y * ph;
    let ox = of_x * size.0 * scale;
    let oy = of_y * size.1 * scale;
    (ax + offset.0 - ox, ay + offset.1 - oy)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32, f32, f32) = (0.0, 0.0, 1280.0, 720.0);

    #[test]
    fn frac_corners() {
        assert_eq!(Anchor9::TopLeft.frac(), (0.0, 0.0));
        assert_eq!(Anchor9::Center.frac(), (0.5, 0.5));
        assert_eq!(Anchor9::BottomRight.frac(), (1.0, 1.0));
    }

    #[test]
    fn top_left_anchor_origin_is_pure_offset() {
        let tl = resolve_top_left(
            Anchor9::TopLeft,
            Anchor9::TopLeft,
            (100.0, 40.0),
            1.0,
            (16.0, 78.0),
            SCREEN,
        );
        assert_eq!(tl, (16.0, 78.0));
    }

    #[test]
    fn center_center_zero_offset_centers_widget() {
        let (l, t) = resolve_top_left(
            Anchor9::Center,
            Anchor9::Center,
            (200.0, 100.0),
            1.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (640.0 - 100.0)).abs() < 0.01);
        assert!((t - (360.0 - 50.0)).abs() < 0.01);
    }

    #[test]
    fn bottom_right_anchor_origin_pins_to_corner() {
        let (l, t) = resolve_top_left(
            Anchor9::BottomRight,
            Anchor9::BottomRight,
            (120.0, 30.0),
            1.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (1280.0 - 120.0)).abs() < 0.01);
        assert!((t - (720.0 - 30.0)).abs() < 0.01);
    }

    #[test]
    fn scale_grows_from_origin() {
        // origin bottom-right, scale 2×: top-left moves left/up by extra size.
        let (l, t) = resolve_top_left(
            Anchor9::TopLeft,
            Anchor9::BottomRight,
            (100.0, 50.0),
            2.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (0.0 - 200.0)).abs() < 0.01);
        assert!((t - (0.0 - 100.0)).abs() < 0.01);
    }

    #[test]
    fn all_nine_anchors_have_distinct_points() {
        let mut seen = std::collections::HashSet::new();
        for a in Anchor9::ALL {
            let (fx, fy) = a.frac();
            assert!(seen.insert(((fx * 2.0) as i32, (fy * 2.0) as i32)));
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn widget_kind_serde_kebab() {
        let s = toml::to_string(&std::collections::BTreeMap::from([(
            "k",
            WidgetKind::ScorePanel,
        )]))
        .unwrap();
        assert_eq!(s.trim(), r#"k = "score-panel""#);
    }
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p dtx-layout widgets`
Expected: FAIL (module not declared).

- [ ] **Step 3: Add to `lib.rs`**

Add `pub mod widgets;` and re-export:
```rust
pub use widgets::{
    resolve_top_left, Anchor9, AnchorSpace, WidgetInstance, WidgetKind, MAX_WIDGET_SCALE,
    MIN_WIDGET_SCALE,
};
```

- [ ] **Step 4: Run**

Run: `cargo test -p dtx-layout`
Expected: PASS (23 prior + 7 new = 30).

- [ ] **Step 5: Commit**

```bash
rustfmt crates/dtx-layout/src/widgets.rs
git status
git add crates/dtx-layout
git commit -m "feat(dtx-layout): widget placement model + anchor-resolution math"
```

---

### Task 2: Default widget instances + `[scene.gameplay]` section (dtx-layout)

**Files:** Create `crates/dtx-layout/src/scene.rs`; modify `crates/dtx-layout/src/lib.rs`, `crates/dtx-layout/src/file.rs`.

The default `WidgetInstance` for every kind is anchor=TopLeft, origin=TopLeft, offset=(0,0), scale=1.0, z per below, visible per below. Because the gameplay HUD's container-at-origin technique (plan 2 gameplay side) makes offset=(0,0) reproduce today's layout, defaults are uniform and simple. Practice-only widgets default `visible_play=false`.

- [ ] **Step 1: Write `scene.rs` (tests-first then impl). Full file:**

```rust
//! `[scene.gameplay]` layout.toml section for HUD widget placement.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::widgets::{
    Anchor9, AnchorSpace, WidgetInstance, WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE,
};

/// Default instance for a kind (offset 0 ⇒ today's on-screen position via the
/// container-at-origin technique). z spreads widgets; practice widgets hidden in play.
pub fn default_instance(kind: WidgetKind) -> WidgetInstance {
    let (z, vis_play, vis_practice) = match kind {
        WidgetKind::Playfield => (0, true, true),
        WidgetKind::FrameChrome => (1, true, true),
        WidgetKind::PracticeTransport => (90, false, true),
        _ => (10, true, true),
    };
    WidgetInstance {
        kind,
        space: AnchorSpace::Screen,
        anchor: Anchor9::TopLeft,
        origin: Anchor9::TopLeft,
        offset: (0.0, 0.0),
        scale: 1.0,
        z,
        visible_play: vis_play,
        visible_practice: vis_practice,
    }
}

/// One serialized widget entry ([[scene.gameplay.widgets]]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetEntry {
    pub kind: WidgetKind,
    #[serde(default = "default_space")]
    pub space: AnchorSpace,
    #[serde(default = "default_anchor")]
    pub anchor: Anchor9,
    #[serde(default = "default_anchor")]
    pub origin: Anchor9,
    #[serde(default)]
    pub offset: [f32; 2],
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub z: i32,
    #[serde(default = "default_true")]
    pub visible_play: bool,
    #[serde(default = "default_true")]
    pub visible_practice: bool,
}

fn default_space() -> AnchorSpace {
    AnchorSpace::Screen
}
fn default_anchor() -> Anchor9 {
    Anchor9::TopLeft
}
fn default_scale() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}

impl WidgetEntry {
    fn to_instance(&self) -> WidgetInstance {
        WidgetInstance {
            kind: self.kind,
            space: self.space,
            anchor: self.anchor,
            origin: self.origin,
            offset: (self.offset[0], self.offset[1]),
            scale: self.scale.clamp(MIN_WIDGET_SCALE, MAX_WIDGET_SCALE),
            z: self.z,
            visible_play: self.visible_play,
            visible_practice: self.visible_practice,
        }
    }

    fn from_instance(i: &WidgetInstance) -> Self {
        Self {
            kind: i.kind,
            space: i.space,
            anchor: i.anchor,
            origin: i.origin,
            offset: [i.offset.0, i.offset.1],
            scale: i.scale,
            z: i.z,
            visible_play: i.visible_play,
            visible_practice: i.visible_practice,
        }
    }
}

/// `[scene.gameplay]` section: a list of widget entries (v1: ≤1 per kind).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<WidgetEntry>,
}

impl SceneSection {
    /// Full resolved map: every `WidgetKind` present, file entries overriding
    /// code defaults. Unknown/duplicate kinds: first wins, extras warned+dropped.
    pub fn resolve(&self) -> HashMap<WidgetKind, WidgetInstance> {
        let mut map: HashMap<WidgetKind, WidgetInstance> = WidgetKind::ALL
            .into_iter()
            .map(|k| (k, default_instance(k)))
            .collect();
        let mut seen = std::collections::HashSet::new();
        for entry in &self.widgets {
            if !seen.insert(entry.kind) {
                eprintln!(
                    "dtx-layout: duplicate widget {:?} in [scene.gameplay], extra dropped",
                    entry.kind
                );
                continue;
            }
            map.insert(entry.kind, entry.to_instance());
        }
        map
    }

    /// Build a section from a resolved map, writing only entries that differ
    /// from the code default (keeps the file minimal).
    pub fn from_map(map: &HashMap<WidgetKind, WidgetInstance>) -> Self {
        let mut widgets: Vec<WidgetEntry> = WidgetKind::ALL
            .into_iter()
            .filter_map(|k| {
                let inst = map.get(&k)?;
                if *inst != default_instance(k) {
                    Some(WidgetEntry::from_instance(inst))
                } else {
                    None
                }
            })
            .collect();
        widgets.sort_by_key(|w| format!("{:?}", w.kind));
        Self { widgets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scene_resolves_all_kinds_to_defaults() {
        let map = SceneSection::default().resolve();
        assert_eq!(map.len(), WidgetKind::ALL.len());
        for k in WidgetKind::ALL {
            assert_eq!(map[&k], default_instance(k));
        }
    }

    #[test]
    fn practice_transport_hidden_in_play_by_default() {
        let d = default_instance(WidgetKind::PracticeTransport);
        assert!(!d.visible_play);
        assert!(d.visible_practice);
    }

    #[test]
    fn file_entry_overrides_default() {
        let section = SceneSection {
            widgets: vec![WidgetEntry {
                kind: WidgetKind::Combo,
                space: AnchorSpace::Screen,
                anchor: Anchor9::TopLeft,
                origin: Anchor9::TopLeft,
                offset: [40.0, -20.0],
                scale: 1.5,
                z: 12,
                visible_play: true,
                visible_practice: true,
            }],
        };
        let map = section.resolve();
        assert_eq!(map[&WidgetKind::Combo].offset, (40.0, -20.0));
        assert_eq!(map[&WidgetKind::Combo].scale, 1.5);
        // Untouched widgets stay default.
        assert_eq!(map[&WidgetKind::ScorePanel], default_instance(WidgetKind::ScorePanel));
    }

    #[test]
    fn scale_clamped_on_resolve() {
        let section = SceneSection {
            widgets: vec![WidgetEntry {
                kind: WidgetKind::Combo,
                space: AnchorSpace::Screen,
                anchor: Anchor9::TopLeft,
                origin: Anchor9::TopLeft,
                offset: [0.0, 0.0],
                scale: 99.0,
                z: 0,
                visible_play: true,
                visible_practice: true,
            }],
        };
        assert_eq!(section.resolve()[&WidgetKind::Combo].scale, MAX_WIDGET_SCALE);
    }

    #[test]
    fn duplicate_kind_first_wins() {
        let mk = |offx: f32| WidgetEntry {
            kind: WidgetKind::Combo,
            space: AnchorSpace::Screen,
            anchor: Anchor9::TopLeft,
            origin: Anchor9::TopLeft,
            offset: [offx, 0.0],
            scale: 1.0,
            z: 0,
            visible_play: true,
            visible_practice: true,
        };
        let section = SceneSection {
            widgets: vec![mk(10.0), mk(99.0)],
        };
        assert_eq!(section.resolve()[&WidgetKind::Combo].offset, (10.0, 0.0));
    }

    #[test]
    fn from_map_only_writes_non_default_entries() {
        let mut map = SceneSection::default().resolve();
        map.get_mut(&WidgetKind::Combo).unwrap().offset = (5.0, 5.0);
        let section = SceneSection::from_map(&map);
        assert_eq!(section.widgets.len(), 1);
        assert_eq!(section.widgets[0].kind, WidgetKind::Combo);
    }

    #[test]
    fn scene_round_trips() {
        let mut map = SceneSection::default().resolve();
        map.get_mut(&WidgetKind::NowPlaying).unwrap().offset = (12.0, 34.0);
        map.get_mut(&WidgetKind::NowPlaying).unwrap().anchor = Anchor9::TopRight;
        let section = SceneSection::from_map(&map);
        let back = section.resolve();
        assert_eq!(back[&WidgetKind::NowPlaying].offset, (12.0, 34.0));
        assert_eq!(back[&WidgetKind::NowPlaying].anchor, Anchor9::TopRight);
    }
}
```

- [ ] **Step 2: Verify failure** — `cargo test -p dtx-layout scene` → FAIL (module missing).

- [ ] **Step 3: Wire into `LayoutFile` (`file.rs`).** Add field:
```rust
pub struct LayoutFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub lanes: LanesSection,
    #[serde(default)]
    pub scene: SceneSection,
}
```
Update `Default for LayoutFile` to add `scene: SceneSection::default()`. Add `use crate::scene::SceneSection;` at the top of file.rs. (Existing `[lanes]` tests still pass because `scene` defaults.)

- [ ] **Step 4: `lib.rs`** — add `pub mod scene;` and re-export:
```rust
pub use scene::{default_instance, SceneSection, WidgetEntry};
```

- [ ] **Step 5: Run** — `cargo test -p dtx-layout` → PASS (30 + 7 = 37). Verify existing `layout_file_round_trip` still passes (scene defaults, `skip_serializing_if` keeps it absent).

- [ ] **Step 6: Commit**
```bash
rustfmt crates/dtx-layout/src/scene.rs
git status
git add crates/dtx-layout
git commit -m "feat(dtx-layout): [scene.gameplay] widget section + defaults + resolution"
```

---

### Task 3: `WidgetContainer` + `WidgetLayouts` resource + apply system (gameplay-drums)

**Files:** Create `crates/gameplay-drums/src/widget_layout.rs`; modify `crates/gameplay-drums/src/lib.rs`.

This task builds the runtime pieces WITHOUT yet wrapping the widgets (Task 4 wires hud.rs). It's independently testable.

- [ ] **Step 1: Write `widget_layout.rs`:**

```rust
//! Runtime HUD widget placement: per-widget container nodes driven by a
//! `WidgetLayouts` resource (code defaults ⊕ layout.toml `[scene.gameplay]`).
//!
//! Each HUD widget's children are parented to a `WidgetContainer` node placed
//! absolutely at ref-origin (0,0), full-size. The container's `left/top` is the
//! widget's resolved offset·scale, so moving it translates the whole widget as
//! one unit; at the default offset (0,0) every widget lands where it did before
//! this system existed (parity). Anchor/origin are modeled for the editor
//! (plan 3); with the current uniform scale the applied position reduces to
//! `offset` (screen-space) — see `apply_widget_layout`.

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_layout::{WidgetInstance, WidgetKind};
use game_shell::AppState;

use crate::layout::PlayfieldLayout;

/// Marks a per-widget container node (parent of one widget's children).
#[derive(Component, Debug, Clone, Copy)]
pub struct WidgetContainer(pub WidgetKind);

/// Resolved placement for every widget kind (defaults ⊕ file).
#[derive(Resource, Debug, Clone)]
pub struct WidgetLayouts(pub HashMap<WidgetKind, WidgetInstance>);

impl Default for WidgetLayouts {
    fn default() -> Self {
        Self(dtx_layout::SceneSection::default().resolve())
    }
}

impl WidgetLayouts {
    pub fn get(&self, kind: WidgetKind) -> &WidgetInstance {
        // resolve() always contains every kind, so this is safe; fall back to a
        // default if a caller ever constructs a partial map.
        self.0.get(&kind).expect("WidgetLayouts missing a kind")
    }
}

/// Whether a widget is visible in the current mode (practice vs play).
pub fn widget_visible(inst: &WidgetInstance, practice: bool) -> bool {
    if practice {
        inst.visible_practice
    } else {
        inst.visible_play
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WidgetLayouts>()
        .add_systems(Startup, load_widget_layouts)
        .add_systems(
            Update,
            apply_widget_layout
                .run_if(in_state(AppState::Performance))
                .run_if(resource_changed::<WidgetLayouts>.or_else(resource_changed::<PlayfieldLayout>)),
        );
}

/// Load `[scene.gameplay]` from layout.toml at startup (defaults on absence).
fn load_widget_layouts(mut layouts: ResMut<WidgetLayouts>) {
    let file = dtx_layout::load(&dtx_layout::default_path());
    layouts.0 = file.scene.resolve();
}

/// Position + z-order + visibility for every widget container. Runs on layout
/// or arrangement change. Position = offset·scale (screen-space, uniform scale);
/// full anchor-aware resolution is a plan-3 concern where variable scale matters.
fn apply_widget_layout(
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    mut containers: Query<(&WidgetContainer, &mut Node, Option<&mut ZIndex>, &mut Visibility)>,
) {
    let is_practice = practice.is_some();
    let scale = pfl.scale;
    for (container, mut node, z, mut vis) in &mut containers {
        let inst = layouts.get(container.0);
        node.left = Val::Px(inst.offset.0 * scale);
        node.top = Val::Px(inst.offset.1 * scale);
        // z-index is optional: the practice transport uses GlobalZIndex instead
        // and simply isn't repositioned in the local stacking context.
        if let Some(mut z) = z {
            *z = ZIndex(inst.z);
        }
        *vis = if widget_visible(inst, is_practice) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::default_instance;

    #[test]
    fn default_layouts_cover_all_kinds() {
        let l = WidgetLayouts::default();
        for k in WidgetKind::ALL {
            assert_eq!(*l.get(k), default_instance(k));
        }
    }

    #[test]
    fn visibility_respects_mode() {
        let transport = default_instance(WidgetKind::PracticeTransport);
        assert!(!widget_visible(&transport, false)); // play: hidden
        assert!(widget_visible(&transport, true)); // practice: shown
        let combo = default_instance(WidgetKind::Combo);
        assert!(widget_visible(&combo, false));
        assert!(widget_visible(&combo, true));
    }
}
```

Notes for the implementer: verify `crate::practice::PracticeSession` is the correct path (it was added in the practice-mode work; grep `pub struct PracticeSession`). Verify `ZIndex` is a tuple struct `ZIndex(i32)` in this Bevy version (grep usages, e.g. `GlobalZIndex(900)` appears in practice/ui.rs — check whether widgets need `ZIndex` or `GlobalZIndex`; use `ZIndex` for local stacking within HudRoot). If `resource_changed::<T>.or_else(...)` isn't available, use `.or_else(resource_changed::<PlayfieldLayout>)` combinator form that compiles in this Bevy (mirror the pattern already used in `layout.rs` `in_state(...).or_else(in_state(...))`).

- [ ] **Step 2: `lib.rs`** — add `pub mod widget_layout;` and add `widget_layout::plugin` to the plugin set (wherever sub-plugins like `layout::plugin`, `keyboard_viz` are added — grep for `.add_plugins` or the `fn plugin` body that calls `hud::plugin`). Add it next to `hud::plugin`.

- [ ] **Step 3: Run** — `cargo test -p gameplay-drums widget_layout` → 2 PASS. `cargo build -p gameplay-drums` clean.

- [ ] **Step 4: Commit**
```bash
rustfmt crates/gameplay-drums/src/widget_layout.rs
git status
git add crates/gameplay-drums
git commit -m "feat(gameplay-drums): WidgetContainer + WidgetLayouts resource + apply system"
```

---

### Task 4: Wrap HUD widgets in containers (gameplay-drums hud.rs)

**Files:** Modify `crates/gameplay-drums/src/hud.rs`.

Read `hud.rs` `spawn_hud` fully first. Currently each widget spawns under `root` (the HudRoot entity). We insert one container per widget kind between `root` and the widget's children, so `apply_widget_layout` can move/hide each.

- [ ] **Step 1: Add a container helper** near the top of hud.rs (after the marker structs):

```rust
use dtx_layout::WidgetKind;
use crate::widget_layout::WidgetContainer;

/// Spawn a per-widget container under `root` (absolute, ref-origin 0,0,
/// full-size) that `apply_widget_layout` positions. Returns the container
/// entity to parent the widget's children to.
fn spawn_widget_container(commands: &mut Commands, root: Entity, kind: WidgetKind) -> Entity {
    let container = commands
        .spawn((
            WidgetContainer(kind),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ZIndex(0),
            Visibility::Inherited,
        ))
        .id();
    commands.entity(root).add_child(container);
    container
}
```

- [ ] **Step 2: Route each widget's spawn through a container.** For each `spawn_*` call in `spawn_hud`, replace the `root` parent argument with a fresh container of the matching kind. Concretely:

```rust
    let c_frame = spawn_widget_container(&mut commands, root, WidgetKind::FrameChrome);
    frame_chrome::spawn_frame_chrome(
        &mut commands,
        c_frame,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_left() + layout.ref_strip_width(),
    );

    let c_score = spawn_widget_container(&mut commands, root, WidgetKind::ScorePanel);
    score_detailed::spawn_score_detailed_panel(&mut commands, c_score, &t, s);

    let c_phrase = spawn_widget_container(&mut commands, root, WidgetKind::PhraseMeter);
    phrase_meter::spawn_phrase_meter(&mut commands, c_phrase, &t, s, layout.ref_phrase_x());

    let c_prog = spawn_widget_container(&mut commands, root, WidgetKind::SongProgress);
    song_progress::spawn_song_progress(
        &mut commands,
        c_prog,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_width(),
    );

    let c_speed = spawn_widget_container(&mut commands, root, WidgetKind::SpeedReadout);
    playfield_speed::spawn_playfield_speed(&mut commands, c_speed, &t, s, 24.0, 470.0);

    let c_now = spawn_widget_container(&mut commands, root, WidgetKind::NowPlaying);
    now_playing::spawn_now_playing(&mut commands, c_now, &t, s, layout.ref_hud_right_x());

    let c_combo = spawn_widget_container(&mut commands, root, WidgetKind::Combo);
    let combo_ref_x = layout.ref_strip_left() + layout.ref_strip_width() / 2.0 - 180.0;
    perf_combo::spawn_perf_combo(&mut commands, c_combo, &t, s, combo_ref_x, 150.0);

    let c_graph = spawn_widget_container(&mut commands, root, WidgetKind::LiveGraph);
    // ...preserve the exact existing live_graph::spawn_live_graph(...) args, but pass c_graph instead of root...

    let c_popup = spawn_widget_container(&mut commands, root, WidgetKind::JudgmentPopup);
    judgment_popup::spawn_judgment_popup(&mut commands, c_popup, &t);
```

IMPORTANT: match the CURRENT exact argument lists of each spawn fn (numbers, extra params) — only change the parent-entity argument from `root` to the container. Read the current call sites and preserve everything else. The `keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &lanes, &t)` call and the `PlayfieldBackboard`/`HitLine` inline spawns stay parented to `root` for now (they belong to the Playfield widget — handled in Task 5); do NOT wrap them in this task.

- [ ] **Step 3: Verify parity.** `cargo build -p gameplay-drums` clean. `cargo test -p gameplay-drums` PASS. Because every container spawns at (0,0) full-size and children keep absolute coords, the HUD is visually identical (apply_widget_layout at default offset 0 keeps left/top 0).

- [ ] **Step 4: Commit**
```bash
git add crates/gameplay-drums/src/hud.rs
git commit -m "feat(gameplay-drums): route HUD widgets through per-kind containers"
```

---

### Task 5: Playfield as a widget + practice-transport container + integration tests

**Files:** Modify `crates/gameplay-drums/src/hud.rs`, `crates/gameplay-drums/src/practice/ui.rs`; create `crates/gameplay-drums/tests/widget_layout.rs`.

- [ ] **Step 1: Tag the Playfield widget group.** The playfield = backboard + hit line + key caps + notes. For v1, moving the playfield as a widget is out of scope (spec allows deferring; notes/caps derive from `PlayfieldLayout` which has no offset input yet). So the `Playfield` widget kind is **visibility/registry-only** in v1: register a container but keep backboard/hitline parented to `root` (they always show). Add a code comment in hud.rs noting playfield offset/scale is deferred to a later plan (it needs `PlayfieldLayout` to take an origin offset). Keep `WidgetKind::Playfield` in the registry so the editor lists it (drag disabled for it in plan 3). **No functional change here beyond the comment** — this step exists to document the deferral explicitly.

- [ ] **Step 2: Practice transport — registry-only in v1 (do NOT wrap).** The transport (`TransportRoot` in `crates/gameplay-drums/src/practice/ui.rs`) is a **bottom-anchored** bar (`bottom:0`, `GlobalZIndex(900)`) that is already conditionally spawned only when `PracticeSession` exists (so it's naturally play-hidden / practice-shown). Wrapping it in a `WidgetContainer` would let `apply_widget_layout` write `top: Px(0)`, which conflicts with its `bottom:0` anchoring and yanks it to the top of the screen. Bottom/right-anchored widget movement is a plan-3 concern (the editor needs anchor-aware application). So in v1: **do NOT add `WidgetContainer` to `TransportRoot`.** `WidgetKind::PracticeTransport` stays in the registry purely so the editor can list it; its actual container wrapping + anchor-aware offset lands in plan 3. Add a one-line comment at the `TransportRoot` spawn noting this deferral. No code change to ui.rs beyond that comment.

- [ ] **Step 3: Integration tests** — `crates/gameplay-drums/tests/widget_layout.rs`:

```rust
//! Integration: widget layout resource drives container transform + visibility.

use bevy::prelude::*;
use dtx_layout::{default_instance, WidgetKind};
use gameplay_drums::widget_layout::{widget_visible, WidgetLayouts};

#[test]
fn default_layouts_have_zero_offset_for_parity() {
    let l = WidgetLayouts::default();
    for k in WidgetKind::ALL {
        assert_eq!(l.get(k).offset, (0.0, 0.0), "{k:?} default offset must be 0 (parity)");
    }
}

#[test]
fn practice_transport_hidden_in_play_shown_in_practice() {
    let l = WidgetLayouts::default();
    let t = l.get(WidgetKind::PracticeTransport);
    assert!(!widget_visible(t, false));
    assert!(widget_visible(t, true));
}

#[test]
fn custom_offset_flows_through_resolve() {
    let section = dtx_layout::SceneSection {
        widgets: vec![dtx_layout::WidgetEntry {
            kind: WidgetKind::Combo,
            space: dtx_layout::AnchorSpace::Screen,
            anchor: dtx_layout::Anchor9::TopLeft,
            origin: dtx_layout::Anchor9::TopLeft,
            offset: [50.0, -30.0],
            scale: 1.0,
            z: 10,
            visible_play: true,
            visible_practice: true,
        }],
    };
    let map = section.resolve();
    let layouts = WidgetLayouts(map);
    assert_eq!(layouts.get(WidgetKind::Combo).offset, (50.0, -30.0));
    // others default
    assert_eq!(*layouts.get(WidgetKind::ScorePanel), default_instance(WidgetKind::ScorePanel));
}
```

Ensure `widget_layout` module and its items are `pub` (`pub mod widget_layout;`, `pub struct WidgetLayouts`, `pub fn widget_visible`, `pub fn get`). Make `WidgetLayouts.0` accessible or add a `pub fn new(map)` — the test uses `WidgetLayouts(map)`, so the tuple field must be `pub` (it is).

- [ ] **Step 4: Verify** — `cargo test -p gameplay-drums --test widget_layout` → 3 PASS. `cargo test --workspace` → all PASS.

- [ ] **Step 5: Commit**
```bash
rustfmt crates/gameplay-drums/tests/widget_layout.rs
git status
git add crates/gameplay-drums
git commit -m "feat(gameplay-drums): playfield widget registry entry + practice-transport container + tests"
```

---

### Task 6: Startup load wiring check + save API + docs

**Files:** Modify `crates/dtx-layout/src/lib.rs` (save helper already exists from plan 1 — verify it round-trips scene); modify `docs/superpowers/specs/2026-07-07-layout-editor-design.md`.

- [ ] **Step 1: Add a dtx-layout round-trip test covering lanes+scene together** in `crates/dtx-layout/src/file.rs` tests:

```rust
    #[test]
    fn layout_file_round_trips_lanes_and_scene() {
        let mut scene = crate::scene::SceneSection::default().resolve();
        scene.get_mut(&crate::WidgetKind::Combo).unwrap().offset = (7.0, 8.0);
        let file = LayoutFile {
            version: LATEST_VERSION,
            lanes: LanesSection::from_arrangement(&crate::presets::nx_type_b()),
            scene: crate::scene::SceneSection::from_map(&scene),
        };
        let s = toml::to_string_pretty(&file).unwrap();
        let back: LayoutFile = toml::from_str(&s).unwrap();
        assert_eq!(back, file);
        assert_eq!(back.scene.resolve()[&crate::WidgetKind::Combo].offset, (7.0, 8.0));
    }
```

- [ ] **Step 2: Run** — `cargo test -p dtx-layout` → PASS.

- [ ] **Step 3: Spec status note.** In the design spec, under `Progress:`, change to note plan 2 delivered:
```markdown
Progress: plans 1 (lane arrangement) and 2 (widget registry + [scene.gameplay]
placement, container-at-origin technique) implemented. Plan 3 (editor overlay)
pending.
```

- [ ] **Step 4: Full verification** — `cargo test --workspace` → all PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/dtx-layout docs/superpowers/specs/2026-07-07-layout-editor-design.md
git commit -m "feat(dtx-layout): full layout.toml lanes+scene round-trip; mark plan 2 delivered"
```

---

## Manual verification (post-merge, needs display)

- Default run: HUD pixel-identical to before (all containers at offset 0).
- Hand-edit `layout.toml` `[[scene.gameplay.widgets]]` with `kind = "combo"`, `offset = [80.0, 40.0]` → combo shifts down-right by 80×40 ref-px, nothing else moves.
- `visible_play = false` on `now-playing` → now-playing hidden in normal play, still there structurally.
- Practice mode: transport strip still shows (unchanged — registry-only in v1).

## Out of scope (plan 3)

- Editor overlay, mouse drag, sidebar, anchor snapping UI, lane-drag UX, hit-group dropdowns, save-from-editor.
- Playfield offset/scale as a movable widget (needs `PlayfieldLayout` origin offset — deferred).
- Anchor-aware runtime repositioning (uniform scale makes offset sufficient in v1; the resolve math is implemented + tested for the editor to use).
