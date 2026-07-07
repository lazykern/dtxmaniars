# Editor Settings Panel Implementation Plan (v2 plan 2 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A right-side per-widget settings panel (anchor 3×3 grid, offset steppers, scale slider, z stepper, visibility toggles, per-row resets) backed by reusable dtx-ui form controls — and make anchor + scale actually functional at render time.

**Architecture:** Two-layer change. (1) Placement engine: `WidgetInstance` gains `placement: Natural | Anchored`. `Natural` keeps v1's exact semantics (container translated by `offset·pfl.scale` from the widget's hard-coded natural position; scale locked to 1) so v1 files and untouched widgets stay byte-identical. `Anchored` resolves absolutely via `resolve_top_left` and applies uniform scale — rendered through the container's `UiTransform` (translation + scale about screen center, with a computed compensation so the content lands exactly at the resolved top-left). The first editor gesture/knob on a widget converts it Natural→Anchored with a no-jump capture. (2) Controls + panel: dtx-ui gains `Slider`/`Stepper`/`Toggle`/`AnchorGrid` controls (marker components + a `ControlsPlugin` driving them); `editor/panel.rs` composes them per selected widget and maps `Changed<ControlValue>` back into `WidgetLayouts`.

**Tech Stack:** Rust, Bevy 0.19. `UiTransform { translation: Val2, scale: Vec2, rotation: Rot2 }` — a render/hit transform that does NOT affect layout (`ComputedNode` is unaffected; children keep their natural layout, so measurements are stable). UI-node `GlobalTransform` DOES include `UiTransform`, so measured rects are visual rects; the known applied `(translation, scale)` is inverted to recover unscaled geometry.

**Spec:** `docs/superpowers/specs/2026-07-07-layout-editor-v2-design.md` (sections 4, 5 generic block + Persistence). Reference: `references/osu-lazer/osu.Game/Overlays/SkinEditor/SkinSettingsToolbox.cs` (behavior only).

**Branch:** `feat/editor-settings-panel` off `main` (after plan 1 `feat/editor-canvas-selection` is merged; this plan builds on `WidgetAabbs`, `Selection`, `ActiveGesture`, `EditorChrome`).

**Existing context (v1 + plan 1):**
- `crates/dtx-layout/src/widgets.rs` — `Anchor9::frac()`, `AnchorSpace`, `WidgetInstance { kind, space, anchor, origin, offset: (f32,f32) /*ref-px*/, scale, z, visible_play, visible_practice }`, `resolve_top_left(anchor, origin, size, scale, offset, parent)` (pure, unit-agnostic).
- `crates/dtx-layout/src/scene.rs` — `default_instance(kind)`, `WidgetEntry` (serde form), `SceneSection::{resolve, from_map}` (from_map skips default-equal entries).
- `crates/gameplay-drums/src/widget_layout.rs` — `WidgetContainer(WidgetKind)` full-screen nodes; `apply_widget_layout` sets `node.left/top = offset·pfl.scale` (v1 delta semantics, anchors/scale unused).
- `crates/gameplay-drums/src/hud.rs` — `spawn_widget_container(commands, root, kind)` helper.
- `crates/gameplay-drums/src/editor/` — plan 1 modules: `picking.rs` (`WidgetAabbs`, `EditorChrome`, `node_rect`, `EditorPickSet`), `drag.rs` (`Gesture`, `ActiveGesture`, `begin_gesture`, `update_gesture`), `selection_box.rs`, `ui.rs` (left sidebar), `undo.rs` (`UndoStack`), `save.rs` (`reset_widget`, `reset_all_widgets`, `next_lane_preset`, `layout_file_from`).
- Theme: `dtx_ui::ThemeResource` / `dtx_ui::theme::Theme` (`accent`, `text_primary`, `text_secondary`, `panel_bg`, `Theme::font(size)`).
- rustfmt gotcha: NEVER bare `cargo fmt --all`.
- 16-plugin tuple limit on `.add_plugins((...))`.

---

## File Structure

- Modify: `crates/dtx-layout/src/widgets.rs` — `Placement` enum + `WidgetInstance.placement` + `offset_for_top_left` inverse resolver.
- Modify: `crates/dtx-layout/src/scene.rs` — serde plumbing for `placement`.
- Modify: `crates/gameplay-drums/src/widget_layout.rs` — `WidgetGeoms` measurement + `UiTransform`-based `apply_widget_layout`.
- Modify: `crates/gameplay-drums/src/hud.rs` — containers spawn with `UiTransform::default()`.
- Modify: `crates/gameplay-drums/src/editor/picking.rs` — derive `WidgetAabbs` from `WidgetGeoms` (drop duplicate traversal).
- Modify: `crates/gameplay-drums/src/editor/drag.rs` — `ensure_anchored` conversion at gesture start.
- Create: `crates/dtx-ui/src/widget/controls.rs` — Slider/Stepper/Toggle/AnchorGrid + `ControlsPlugin`.
- Create: `crates/gameplay-drums/src/editor/panel.rs` — right settings panel.
- Modify: `crates/gameplay-drums/src/editor/ui.rs` — slim left sidebar (remove Reset Widget + Next Lane Preset buttons; panel owns per-widget actions).
- Test: inline unit tests + `crates/gameplay-drums/tests/editor_panel.rs`.

### Task 0: Branch

- [ ] **Step 0.1:**

```bash
cd /home/lazykern/lab/dtxmaniars && git checkout -b feat/editor-settings-panel main
```

### Task 1: dtx-layout — `Placement` + inverse resolver

**Files:**
- Modify: `crates/dtx-layout/src/widgets.rs`
- Modify: `crates/dtx-layout/src/scene.rs`

- [ ] **Step 1.1: Add `Placement` and the field (widgets.rs)**

```rust
/// How the widget's position is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// v1 semantics: widget stays at its code-natural position, translated by
    /// `offset` (ref-px). Anchor/origin/scale are inert (scale renders as 1).
    #[default]
    Natural,
    /// Absolute: `resolve_top_left(anchor, origin, size, scale, offset·s, parent)`.
    Anchored,
}
```

Add to `WidgetInstance` (after `space`): `pub placement: Placement,`.

- [ ] **Step 1.2: Add the inverse resolver + tests (widgets.rs)**

```rust
/// Inverse of `resolve_top_left`: the offset that places the widget's top-left
/// at `top_left` given everything else. Same unit convention as resolve.
pub fn offset_for_top_left(
    anchor: Anchor9,
    origin: Anchor9,
    size: (f32, f32),
    scale: f32,
    top_left: (f32, f32),
    parent: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (px, py, pw, ph) = parent;
    let (af_x, af_y) = anchor.frac();
    let (of_x, of_y) = origin.frac();
    (
        top_left.0 - (px + af_x * pw) + of_x * size.0 * scale,
        top_left.1 - (py + af_y * ph) + of_y * size.1 * scale,
    )
}
```

Tests (same `mod tests`):

```rust
#[test]
fn offset_for_top_left_round_trips_resolve() {
    let parent = (100.0, 50.0, 800.0, 600.0);
    for anchor in Anchor9::ALL {
        for origin in Anchor9::ALL {
            let offset = offset_for_top_left(anchor, origin, (120.0, 40.0), 1.5, (300.0, 200.0), parent);
            let tl = resolve_top_left(anchor, origin, (120.0, 40.0), 1.5, offset, parent);
            assert!((tl.0 - 300.0).abs() < 0.001 && (tl.1 - 200.0).abs() < 0.001,
                "{anchor:?}/{origin:?}");
        }
    }
}

#[test]
fn placement_default_is_natural() {
    assert_eq!(Placement::default(), Placement::Natural);
}
```

- [ ] **Step 1.3: scene.rs serde plumbing** — `WidgetEntry` gains:

```rust
    #[serde(default)]
    pub placement: Placement,
```

(import `Placement` in the `use crate::widgets::{...}` list); `to_instance`/`from_instance` copy it; `default_instance` sets `placement: Placement::Natural,`. Update every struct-literal `WidgetEntry { ... }` in scene.rs tests with `placement: Placement::Natural,`.

- [ ] **Step 1.4: Fix all `WidgetInstance` literals across the workspace**

Run: `cargo build --workspace 2>&1 | grep -E '^error' | head` — add `placement: Placement::Natural` (or `dtx_layout::Placement::Natural`) to every literal the compiler flags (expected: scene.rs `default_instance`, possibly tests).

- [ ] **Step 1.5: Tests + commit**

Run: `cargo test -p dtx-layout` → PASS (incl. the two new ones + all existing round-trips: a default-placement entry serializes without a `placement` key because of `#[serde(default)]`? No — serde `default` affects deserialize only. ALSO add `#[serde(skip_serializing_if = "placement_is_natural")]` with `fn placement_is_natural(p: &Placement) -> bool { *p == Placement::Natural }` so v1-shaped files stay minimal and round-trip tests keep passing).

```bash
git add crates/dtx-layout/
git commit -m "feat(dtx-layout): Placement model + inverse anchor resolver"
```

### Task 2: widget_layout.rs — geometry measurement + UiTransform apply

**Files:**
- Modify: `crates/gameplay-drums/src/widget_layout.rs`
- Modify: `crates/gameplay-drums/src/hud.rs` (containers get `UiTransform::default()`)

- [ ] **Step 2.1: Add `WidgetGeoms` + pure transform math + tests**

```rust
use bevy::ui::UiTransform;

/// Per-widget content geometry in UNSCALED logical px (children's natural
/// layout, before the container's UiTransform). `applied` is the transform we
/// set last frame, used to invert visual measurements back to unscaled space.
#[derive(Debug, Clone, Copy)]
pub struct WidgetGeom {
    pub unscaled: Rect,
    pub applied_translation: Vec2,
    pub applied_scale: f32,
}

#[derive(Resource, Debug, Default)]
pub struct WidgetGeoms(pub std::collections::HashMap<WidgetKind, WidgetGeom>);

/// A UiTransform (translation T, uniform scale s) maps an unscaled point p to
/// S + s·(p − S) + T, where S = screen center (full-screen container's center).
pub fn transform_point(p: Vec2, screen_center: Vec2, t: Vec2, s: f32) -> Vec2 {
    screen_center + s * (p - screen_center) + t
}

/// Inverse of `transform_point` for a whole rect (recover unscaled geometry
/// from a visual measurement under a known applied transform).
pub fn untransform_rect(measured: Rect, screen_center: Vec2, t: Vec2, s: f32) -> Rect {
    let inv = |m: Vec2| screen_center + (m - t - screen_center) / s.max(f32::EPSILON);
    Rect::from_corners(inv(measured.min), inv(measured.max))
}

/// Translation that puts the unscaled content top-left `u_min` at visual
/// position `desired` under scale `s` about `screen_center`.
pub fn translation_for(desired: Vec2, u_min: Vec2, screen_center: Vec2, s: f32) -> Vec2 {
    desired - screen_center - s * (u_min - screen_center)
}
```

Unit tests:

```rust
#[test]
fn transform_math_round_trips() {
    let sc = Vec2::new(640.0, 360.0);
    let t = Vec2::new(37.0, -12.0);
    let s = 1.7;
    let r = Rect::new(100.0, 50.0, 300.0, 120.0);
    let vis = Rect::from_corners(
        transform_point(r.min, sc, t, s),
        transform_point(r.max, sc, t, s),
    );
    let back = untransform_rect(vis, sc, t, s);
    assert!((back.min - r.min).length() < 0.001);
    assert!((back.max - r.max).length() < 0.001);
}

#[test]
fn translation_for_places_content() {
    let sc = Vec2::new(640.0, 360.0);
    let u_min = Vec2::new(200.0, 100.0);
    let desired = Vec2::new(50.0, 400.0);
    let s = 2.0;
    let t = translation_for(desired, u_min, sc, s);
    assert!((transform_point(u_min, sc, t, s) - desired).length() < 0.001);
}

#[test]
fn identity_transform_at_defaults() {
    let sc = Vec2::new(640.0, 360.0);
    let u_min = Vec2::new(123.0, 45.0);
    // Natural placement, offset 0 → desired == natural top-left → T == 0.
    let t = translation_for(u_min, u_min, sc, 1.0);
    assert!(t.length() < 0.001);
}
```

- [ ] **Step 2.2: Measurement system (replaces plan 1's traversal as the single source)**

```rust
/// Measure every widget container's visual content rect and invert the applied
/// transform to keep `WidgetGeoms` in unscaled space. Runs every frame in
/// Performance (cheap: ~10 widgets, shallow trees).
fn measure_widget_geoms(
    mut geoms: ResMut<WidgetGeoms>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    containers: Query<(Entity, &WidgetContainer, &UiTransform)>,
    children_q: Query<&Children>,
    nodes: Query<(&ComputedNode, &GlobalTransform)>,
) {
    let Ok(window) = windows.single() else { return };
    let sc = Vec2::new(window.width() / 2.0, window.height() / 2.0);
    for (entity, container, ui_tf) in &containers {
        let kind = container.0;
        if kind == WidgetKind::Playfield {
            continue;
        }
        let mut union: Option<Rect> = None;
        let mut stack: Vec<Entity> = children_q
            .get(entity)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        while let Some(e) = stack.pop() {
            if let Ok((cn, gt)) = nodes.get(e) {
                if cn.size().x > 0.0 && cn.size().y > 0.0 {
                    let inv = cn.inverse_scale_factor();
                    let center = gt.translation().truncate() * inv;
                    let size = cn.size() * inv;
                    let r = Rect::from_center_size(center, size);
                    union = Some(union.map_or(r, |u| u.union(r)));
                }
            }
            if let Ok(c) = children_q.get(e) {
                stack.extend(c.iter());
            }
        }
        let (t, s) = applied_of(ui_tf);
        if let Some(measured) = union.filter(|r| r.width() >= 1.0 && r.height() >= 1.0) {
            let unscaled = untransform_rect(measured, sc, t, s);
            geoms.0.insert(
                kind,
                WidgetGeom { unscaled, applied_translation: t, applied_scale: s },
            );
        } else if let Some(g) = geoms.0.get_mut(&kind) {
            // Keep last-known unscaled rect; just refresh the applied transform.
            g.applied_translation = t;
            g.applied_scale = s;
        }
    }
}

/// Extract (translation px, uniform scale) from a container's UiTransform.
fn applied_of(tf: &UiTransform) -> (Vec2, f32) {
    let t = match (tf.translation.x, tf.translation.y) {
        (Val::Px(x), Val::Px(y)) => Vec2::new(x, y),
        _ => Vec2::ZERO,
    };
    (t, tf.scale.x.max(f32::EPSILON))
}
```

(If `Val2`'s fields aren't `x`/`y`, check `~/.cargo/registry/src/*/bevy_ui-0.19.0/src/ui_transform.rs` and adjust — do not guess.)

- [ ] **Step 2.3: Rewrite `apply_widget_layout`**

```rust
/// Parent rect (logical px) for a widget's anchor space.
pub fn parent_rect_px(space: AnchorSpace, window_size: Vec2, pfl: &PlayfieldLayout) -> (f32, f32, f32, f32) {
    match space {
        AnchorSpace::Screen => (0.0, 0.0, window_size.x, window_size.y),
        AnchorSpace::Playfield => (
            pfl.strip_left(),
            pfl.lane_top(),
            pfl.strip_width(),
            pfl.lane_height(),
        ),
    }
}

fn apply_widget_layout(
    layouts: Res<WidgetLayouts>,
    geoms: Res<WidgetGeoms>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut containers: Query<(
        &WidgetContainer,
        &mut UiTransform,
        Option<&mut ZIndex>,
        &mut Visibility,
    )>,
) {
    let Ok(window) = windows.single() else { return };
    let wsize = Vec2::new(window.width(), window.height());
    let sc = wsize / 2.0;
    let is_practice = practice.is_some();
    for (container, mut tf, z, mut vis) in &mut containers {
        let inst = layouts.get(container.0);
        match inst.placement {
            dtx_layout::Placement::Natural => {
                // v1 semantics: pure ref-px delta, scale inert.
                tf.translation = Val2::new(
                    Val::Px(inst.offset.0 * pfl.scale),
                    Val::Px(inst.offset.1 * pfl.scale),
                );
                tf.scale = Vec2::ONE;
            }
            dtx_layout::Placement::Anchored => {
                let Some(geom) = geoms.0.get(&container.0) else {
                    // Not measured yet (first frames): leave last transform.
                    continue;
                };
                let size = (geom.unscaled.width(), geom.unscaled.height());
                let parent = parent_rect_px(inst.space, wsize, &pfl);
                let desired = dtx_layout::resolve_top_left(
                    inst.anchor,
                    inst.origin,
                    size,
                    inst.scale,
                    (inst.offset.0 * pfl.scale, inst.offset.1 * pfl.scale),
                    parent,
                );
                let t = translation_for(
                    Vec2::new(desired.0, desired.1),
                    geom.unscaled.min,
                    sc,
                    inst.scale,
                );
                tf.translation = Val2::new(Val::Px(t.x), Val::Px(t.y));
                tf.scale = Vec2::splat(inst.scale);
            }
        }
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
```

Plugin changes in `widget_layout::plugin`:

```rust
        .init_resource::<WidgetGeoms>()
        .add_systems(
            Update,
            (
                measure_widget_geoms,
                apply_widget_layout.run_if(
                    resource_changed::<WidgetLayouts>
                        .or_else(resource_changed::<PlayfieldLayout>)
                        .or_else(any_anchored_widget),
                ),
            )
                .chain()
                .run_if(in_state(AppState::Performance)),
        )
```

with:

```rust
/// Anchored widgets need a per-frame apply (their resolved position depends on
/// measured geometry, which can change as content re-lays-out). Natural-only
/// scenes keep the v1 change-detection behavior.
fn any_anchored_widget(layouts: Res<WidgetLayouts>) -> bool {
    layouts
        .0
        .values()
        .any(|i| i.placement == dtx_layout::Placement::Anchored)
}
```

- [ ] **Step 2.4: Containers spawn with `UiTransform::default()`** — in `hud.rs::spawn_widget_container`, add `bevy::ui::UiTransform::default(),` to the container bundle. Remove the now-dead `node.left/top` writes if `apply_widget_layout` was the only writer (containers stay `left: Val::Px(0.0), top: Val::Px(0.0)` at spawn — keep those spawn values).

- [ ] **Step 2.5: Parity check + tests**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5`
Expected: PASS. The end-to-end stage test (`tests/end_to_end_stage.rs`) and hud tests must not regress — Natural placement with offset 0 must produce identity transform.

- [ ] **Step 2.6: Commit**

```bash
git add crates/gameplay-drums/src/widget_layout.rs crates/gameplay-drums/src/hud.rs
git commit -m "feat(gameplay-drums): UiTransform placement engine (Natural/Anchored)"
```

### Task 3: picking.rs derives from WidgetGeoms; drag.rs converts to Anchored

**Files:**
- Modify: `crates/gameplay-drums/src/editor/picking.rs`
- Modify: `crates/gameplay-drums/src/editor/drag.rs`

- [ ] **Step 3.1: Replace `collect_widget_aabbs`' traversal with a derive**

```rust
/// Visual AABB = unscaled geom pushed through the applied transform. The
/// traversal lives in widget_layout::measure_widget_geoms (always-on); the
/// editor just derives hit rects from it.
fn collect_widget_aabbs(
    mut aabbs: ResMut<WidgetAabbs>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(window) = windows.single() else { return };
    let sc = Vec2::new(window.width() / 2.0, window.height() / 2.0);
    let is_practice = practice.is_some();
    for kind in dtx_layout::WidgetKind::ALL {
        if kind == dtx_layout::WidgetKind::Playfield {
            continue;
        }
        if !widget_visible(layouts.get(kind), is_practice) {
            aabbs.0.remove(&kind);
            continue;
        }
        let Some(g) = geoms.0.get(&kind) else { continue };
        let vis = Rect::from_corners(
            crate::widget_layout::transform_point(
                g.unscaled.min, sc, g.applied_translation, g.applied_scale),
            crate::widget_layout::transform_point(
                g.unscaled.max, sc, g.applied_translation, g.applied_scale),
        );
        let vis = Rect::from_center_size(vis.center(), vis.size().max(Vec2::splat(MIN_GRAB)));
        aabbs.0.insert(kind, vis);
    }
    aabbs.0.insert(
        dtx_layout::WidgetKind::Playfield,
        Rect::new(
            pfl.backboard_left(),
            pfl.backboard_top(),
            pfl.backboard_left() + pfl.backboard_width(),
            pfl.backboard_top() + pfl.backboard_height(),
        ),
    );
}
```

Delete the unused `node_rect` callers if any remain in picking (keep `node_rect` itself — chrome masking uses it).

- [ ] **Step 3.2: drag.rs — Natural→Anchored no-jump conversion at gesture start**

Add helper:

```rust
/// First edit converts a Natural widget to Anchored, capturing its current
/// visual position so nothing jumps. Keeps existing anchor/origin values.
pub fn ensure_anchored(
    inst: &mut dtx_layout::WidgetInstance,
    visual_top_left: Vec2,
    unscaled_size: Vec2,
    parent: (f32, f32, f32, f32),
    pfl_scale: f32,
) {
    if inst.placement == dtx_layout::Placement::Anchored {
        return;
    }
    inst.placement = dtx_layout::Placement::Anchored;
    inst.scale = 1.0;
    let off_px = dtx_layout::offset_for_top_left(
        inst.anchor,
        inst.origin,
        (unscaled_size.x, unscaled_size.y),
        1.0,
        (visual_top_left.x, visual_top_left.y),
        parent,
    );
    inst.offset = (off_px.0 / pfl_scale.max(f32::EPSILON), off_px.1 / pfl_scale.max(f32::EPSILON));
}
```

In `begin_gesture`, right after `undo.push(...)` for BOTH the Scale and Move branches, convert (needs `geoms: Res<crate::widget_layout::WidgetGeoms>`, `windows` already present, `pfl: Res<crate::layout::PlayfieldLayout>` added, and `layouts` back to `ResMut<WidgetLayouts>`). IMPORTANT: the visual top-left comes from the geom pushed through its applied transform — NOT from `WidgetAabbs` (those rects are inflated to `MIN_GRAB` for tiny widgets, which would corrupt the captured offset):

```rust
    if let Some(g) = geoms.0.get(&kind).copied() {
        if let Some(inst) = layouts.0.get_mut(&kind) {
            let wsize = Vec2::new(window.width(), window.height());
            let sc = wsize / 2.0;
            let visual_min = crate::widget_layout::transform_point(
                g.unscaled.min, sc, g.applied_translation, g.applied_scale);
            let parent = crate::widget_layout::parent_rect_px(inst.space, wsize, &pfl);
            ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
        }
    }
```

Unit test for the no-jump invariant (drag.rs `mod tests`):

```rust
#[test]
fn ensure_anchored_preserves_position() {
    let mut inst = dtx_layout::default_instance(dtx_layout::WidgetKind::Combo);
    let parent = (0.0, 0.0, 1280.0, 720.0);
    let visual = Vec2::new(831.0, 72.0);
    let size = Vec2::new(140.0, 60.0);
    ensure_anchored(&mut inst, visual, size, parent, 1.0);
    assert_eq!(inst.placement, dtx_layout::Placement::Anchored);
    let tl = dtx_layout::resolve_top_left(
        inst.anchor, inst.origin, (size.x, size.y), inst.scale,
        (inst.offset.0, inst.offset.1), parent,
    );
    assert!((tl.0 - visual.x).abs() < 0.001 && (tl.1 - visual.y).abs() < 0.001);
}
```

(Ensure `dtx-layout`'s `default_instance` is re-exported: `crates/dtx-layout/src/lib.rs` already re-exports scene/widget items — verify `default_instance`, `offset_for_top_left`, `Placement` are in the `pub use` lists; add if missing.)

- [ ] **Step 3.3: Tests + commit**

Run: `cargo test -p gameplay-drums editor -- --nocapture` → PASS.

```bash
git add crates/gameplay-drums/src/editor/ crates/dtx-layout/src/lib.rs
git commit -m "feat(editor): derive hit rects from geoms; no-jump Anchored conversion"
```

### Task 4: dtx-ui controls

**Files:**
- Create: `crates/dtx-ui/src/widget/controls.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs` (add `pub mod controls;`)
- Modify: `crates/dtx-ui/src/lib.rs` (register `controls::ControlsPlugin` if a central UiPlugin exists; otherwise consumers add it — check `lib.rs`: if there's a `DtxUiPlugin` aggregating systems, add there; else export the plugin for gameplay-drums to add)

- [ ] **Step 4.1: Write the controls module**

```rust
//! Minimal form controls for editor/settings UIs: Slider, Stepper, Toggle,
//! AnchorGrid. Pattern: caller spawns via the helpers, tags rows with its own
//! marker, then watches Changed<ControlValue>/<ControlBool>/<AnchorChoice>.
//! `ControlsPlugin` drives interaction → value updates + visuals.

use bevy::prelude::*;

use crate::theme::Theme;

/// Continuous value carried by Slider and Stepper entities.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ControlValue(pub f32);

/// Boolean carried by Toggle entities.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ControlBool(pub bool);

#[derive(Component, Debug, Clone, Copy)]
pub struct Slider {
    pub min: f32,
    pub max: f32,
}

/// Slider child: filled track portion.
#[derive(Component)]
pub struct SliderFill;

#[derive(Component, Debug, Clone, Copy)]
pub struct Stepper {
    pub step: f32,
    pub min: f32,
    pub max: f32,
    /// Decimal places shown on the label.
    pub decimals: usize,
}

/// Stepper child button: -1 or +1.
#[derive(Component, Debug, Clone, Copy)]
pub struct StepperBtn(pub i8);

/// Stepper child: the numeric label.
#[derive(Component)]
pub struct StepperLabel;

#[derive(Component, Debug, Clone, Copy)]
pub struct Toggle;

/// Toggle child: the knob square.
#[derive(Component)]
pub struct ToggleKnob;

/// Currently dragged slider (one at a time).
#[derive(Resource, Debug, Default)]
pub struct ActiveSlider(pub Option<Entity>);

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveSlider>().add_systems(
            Update,
            (drive_sliders, drive_steppers, drive_toggles, paint_slider_fill,
             paint_stepper_labels, paint_toggles),
        );
    }
}

pub const SLIDER_WIDTH: f32 = 110.0;
pub const SLIDER_HEIGHT: f32 = 14.0;

/// Spawn a slider (track + fill). Returns the slider entity (carries
/// `Slider` + `ControlValue` + `Button` for Interaction).
pub fn spawn_slider(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    spec: Slider,
    value: f32,
) -> Entity {
    p.spawn((
        spec,
        ControlValue(value),
        Button,
        Node {
            width: Val::Px(SLIDER_WIDTH),
            height: Val::Px(SLIDER_HEIGHT),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        children![(
            SliderFill,
            Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme.accent),
        )],
    ))
    .id()
}

/// Spawn a stepper row: `[-] value [+]`. Returns the stepper entity.
pub fn spawn_stepper(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    spec: Stepper,
    value: f32,
) -> Entity {
    p.spawn((
        spec,
        ControlValue(value),
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        },
        children![
            (
                StepperBtn(-1),
                Button,
                Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                children![(Text::new("-"), Theme::font(12.0), TextColor(theme.text_primary))],
            ),
            (
                StepperLabel,
                Text::new(""),
                Theme::font(12.0),
                TextColor(theme.text_primary),
                Node { min_width: Val::Px(44.0), ..default() },
            ),
            (
                StepperBtn(1),
                Button,
                Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                children![(Text::new("+"), Theme::font(12.0), TextColor(theme.text_primary))],
            ),
        ],
    ))
    .id()
}

/// Spawn a toggle. Returns the toggle entity.
pub fn spawn_toggle(p: &mut ChildSpawnerCommands, theme: &Theme, value: bool) -> Entity {
    p.spawn((
        Toggle,
        ControlBool(value),
        Button,
        Node {
            width: Val::Px(30.0),
            height: Val::Px(16.0),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        children![(
            ToggleKnob,
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(theme.accent),
        )],
    ))
    .id()
}

/// Pure: slider value from a cursor x within the track rect.
pub fn slider_value_at(min: f32, max: f32, track_left: f32, track_width: f32, cursor_x: f32) -> f32 {
    if track_width <= f32::EPSILON {
        return min;
    }
    let frac = ((cursor_x - track_left) / track_width).clamp(0.0, 1.0);
    min + frac * (max - min)
}

/// Pure: stepper arithmetic (shift = ×10 step).
pub fn stepper_next(value: f32, dir: i8, step: f32, big: bool, min: f32, max: f32) -> f32 {
    let s = if big { step * 10.0 } else { step };
    (value + s * dir as f32).clamp(min, max)
}

fn drive_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: ResMut<ActiveSlider>,
    mut sliders: Query<(Entity, &Slider, &mut ControlValue, &Interaction, &ComputedNode, &GlobalTransform)>,
) {
    if !buttons.pressed(MouseButton::Left) {
        active.0 = None;
    } else if buttons.just_pressed(MouseButton::Left) {
        for (e, _, _, interaction, _, _) in &sliders {
            if *interaction == Interaction::Pressed {
                active.0 = Some(e);
                break;
            }
        }
    }
    let Some(active_e) = active.0 else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    if let Ok((_, spec, mut value, _, cn, gt)) = sliders.get_mut(active_e) {
        let inv = cn.inverse_scale_factor();
        let center = gt.translation().truncate() * inv;
        let size = cn.size() * inv;
        let left = center.x - size.x / 2.0;
        let next = slider_value_at(spec.min, spec.max, left, size.x, cursor.x);
        if (next - value.0).abs() > f32::EPSILON {
            value.0 = next;
        }
    }
}

fn drive_steppers(
    keys: Res<ButtonInput<KeyCode>>,
    btns: Query<(&StepperBtn, &Interaction, &ChildOf), Changed<Interaction>>,
    mut steppers: Query<(&Stepper, &mut ControlValue)>,
) {
    let big = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for (btn, interaction, child_of) in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok((spec, mut value)) = steppers.get_mut(child_of.parent()) {
            value.0 = stepper_next(value.0, btn.0, spec.step, big, spec.min, spec.max);
        }
    }
}

fn drive_toggles(
    mut toggles: Query<(&Interaction, &mut ControlBool), (With<Toggle>, Changed<Interaction>)>,
) {
    for (interaction, mut v) in &mut toggles {
        if *interaction == Interaction::Pressed {
            v.0 = !v.0;
        }
    }
}

fn paint_slider_fill(
    sliders: Query<(&Slider, &ControlValue, &Children), Changed<ControlValue>>,
    mut fills: Query<&mut Node, With<SliderFill>>,
) {
    for (spec, value, children) in &sliders {
        let frac = ((value.0 - spec.min) / (spec.max - spec.min)).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(frac * 100.0);
            }
        }
    }
}

fn paint_stepper_labels(
    steppers: Query<(&Stepper, &ControlValue, &Children), Changed<ControlValue>>,
    mut labels: Query<&mut Text, With<StepperLabel>>,
) {
    for (spec, value, children) in &steppers {
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child) {
                text.0 = format!("{:.*}", spec.decimals, value.0);
            }
        }
    }
}

fn paint_toggles(
    toggles: Query<(&ControlBool, &Children), (With<Toggle>, Changed<ControlBool>)>,
    mut knobs: Query<(&mut Node, &mut BackgroundColor), With<ToggleKnob>>,
) {
    for (v, children) in &toggles {
        for child in children.iter() {
            if let Ok((mut node, mut bg)) = knobs.get_mut(child) {
                node.margin = if v.0 {
                    UiRect::left(Val::Px(14.0))
                } else {
                    UiRect::left(Val::Px(0.0))
                };
                bg.0 = if v.0 {
                    Color::srgb(0.0, 0.831, 0.667)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.3)
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_value_maps_cursor_to_range() {
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 100.0), 0.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 300.0), 10.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 200.0), 5.0);
        assert_eq!(slider_value_at(0.0, 10.0, 100.0, 200.0, 0.0), 0.0); // clamped
    }

    #[test]
    fn stepper_steps_and_clamps() {
        assert_eq!(stepper_next(5.0, 1, 1.0, false, 0.0, 10.0), 6.0);
        assert_eq!(stepper_next(5.0, 1, 1.0, true, 0.0, 10.0), 10.0); // 5+10 clamped
        assert_eq!(stepper_next(0.5, -1, 1.0, false, 0.0, 10.0), 0.0);
    }
}
```

Note: labels paint on `Changed<ControlValue>` only — the spawn helper must trigger an initial paint. Simplest: after spawning, the caller doesn't need to do anything because a fresh `ControlValue` component IS "changed" on its first frame. Verify with the integration test in Task 6.

- [ ] **Step 4.2: Register plugin** — check `crates/dtx-ui/src/lib.rs` for an aggregate plugin; if none, `gameplay-drums/src/lib.rs` adds `dtx_ui::widget::controls::ControlsPlugin` next to its other plugin registrations (watch the 16-tuple limit).

- [ ] **Step 4.3: Tests + commit**

Run: `cargo test -p dtx-ui controls` → PASS.

```bash
git add crates/dtx-ui/ crates/gameplay-drums/src/lib.rs
git commit -m "feat(dtx-ui): slider/stepper/toggle form controls"
```

### Task 5: editor/panel.rs — the settings panel

**Files:**
- Create: `crates/gameplay-drums/src/editor/panel.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (`pub mod panel;` + plugin registration)
- Modify: `crates/gameplay-drums/src/editor/ui.rs` (remove `ResetWidget` + `NextPreset` buttons + their match arms; lane preset moves to plan 3's lane panel, reset lives in this panel)

- [ ] **Step 5.1: Panel scaffold + rebuild-on-selection**

```rust
//! Right settings panel: per-widget knobs for the selected widget. Rebuilt
//! whenever the selection changes; control changes write straight into
//! `WidgetLayouts` (single mutation path — undo/save cover it).

use bevy::prelude::*;
use dtx_layout::{Anchor9, WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE};
use dtx_ui::widget::controls::{
    self, ControlBool, ControlValue, Slider, Stepper,
};

use super::drag::Selection;
use super::picking::EditorChrome;
use super::EditorOpen;
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Component)]
pub struct PanelRoot;

/// Which widget field a control edits.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum PanelField {
    OffsetX,
    OffsetY,
    Scale,
    Z,
    VisiblePlay,
    VisiblePractice,
}

/// 3×3 anchor grid cell.
#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorCell(pub Anchor9);

/// Reset-this-widget button.
#[derive(Component)]
pub struct PanelResetWidget;

pub const PANEL_WIDTH: f32 = 240.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            rebuild_panel.run_if(
                resource_changed::<Selection>.or_else(resource_changed::<super::EditorOpen>),
            ),
            (apply_panel_controls, apply_anchor_cells, handle_reset, refresh_panel_values)
                .run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_panel);
}

fn despawn_panel(mut commands: Commands, q: Query<Entity, With<PanelRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
```

- [ ] **Step 5.2: `rebuild_panel` — spawn rows for the selected widget**

```rust
fn rebuild_panel(
    mut commands: Commands,
    open: Res<EditorOpen>,
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<PanelRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let Some(kind) = selection.0 else { return };
    if kind == WidgetKind::Playfield {
        return; // plan 3 adds the lane panel here
    }
    let t = theme.0;
    let inst = layouts.get(kind).clone();
    let root = commands
        .spawn((
            PanelRoot,
            EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.92)),
            GlobalZIndex(2000),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new(format!("Settings ({})", kind.display_name())),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_primary),
        ));

        // Anchor 3×3 grid.
        p.spawn((
            Text::new("anchor"),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for row in 0..3 {
                grid.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|r| {
                    for col in 0..3 {
                        let a = Anchor9::ALL[row * 3 + col];
                        let selected = inst.anchor == a;
                        r.spawn((
                            AnchorCell(a),
                            Button,
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                ..default()
                            },
                            BackgroundColor(if selected {
                                t.accent
                            } else {
                                Color::srgb(0.14, 0.14, 0.18)
                            }),
                        ));
                    }
                });
            }
        });

        // Offset / scale / z rows.
        row(p, &t, "offset x", |p| {
            let e = controls::spawn_stepper(
                p, &t,
                Stepper { step: 1.0, min: -2000.0, max: 2000.0, decimals: 0 },
                inst.offset.0,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetX);
        });
        row(p, &t, "offset y", |p| {
            let e = controls::spawn_stepper(
                p, &t,
                Stepper { step: 1.0, min: -2000.0, max: 2000.0, decimals: 0 },
                inst.offset.1,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetY);
        });
        row(p, &t, "scale", |p| {
            let e = controls::spawn_slider(
                p, &t,
                Slider { min: MIN_WIDGET_SCALE, max: MAX_WIDGET_SCALE },
                inst.scale,
            );
            p.commands_mut().entity(e).insert(PanelField::Scale);
        });
        row(p, &t, "z", |p| {
            let e = controls::spawn_stepper(
                p, &t,
                Stepper { step: 1.0, min: -100.0, max: 100.0, decimals: 0 },
                inst.z as f32,
            );
            p.commands_mut().entity(e).insert(PanelField::Z);
        });
        row(p, &t, "show in play", |p| {
            let e = controls::spawn_toggle(p, &t, inst.visible_play);
            p.commands_mut().entity(e).insert(PanelField::VisiblePlay);
        });
        row(p, &t, "show in practice", |p| {
            let e = controls::spawn_toggle(p, &t, inst.visible_practice);
            p.commands_mut().entity(e).insert(PanelField::VisiblePractice);
        });

        p.spawn((
            PanelResetWidget,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(
                Text::new("Reset Widget"),
                dtx_ui::theme::Theme::font(12.0),
                TextColor(t.text_primary),
            )],
        ));
    });
}

fn row(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    label: &str,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            Text::new(label.to_string()),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        content(r);
    });
}
```

(If `ChildSpawnerCommands::commands_mut()` doesn't exist under that name, look for the method returning `Commands` on the child builder in bevy 0.19 — e.g. `commands()`; fix per compiler.)

- [ ] **Step 5.3: Control → `WidgetLayouts` mapping (+ undo, + Anchored conversion)**

```rust
/// One undo snapshot per discrete panel change; slider drags snapshot on the
/// first change of a mouse-hold (tracked via Local).
fn apply_panel_controls(
    selection: Res<Selection>,
    buttons: Res<ButtonInput<MouseButton>>,
    values: Query<(&PanelField, Option<&ControlValue>, Option<&ControlBool>),
        Or<(Changed<ControlValue>, Changed<ControlBool>)>>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut snapped_this_hold: Local<bool>,
) {
    let Some(kind) = selection.0 else { return };
    if values.is_empty() {
        if !buttons.pressed(MouseButton::Left) {
            *snapped_this_hold = false;
        }
        return;
    }
    // Panel rebuilds mark every fresh control Changed with values equal to the
    // instance — pushing undo then would flood the stack with no-op snapshots.
    // Only snapshot (and write) when an incoming value actually differs.
    let dirty = {
        let inst = layouts.get(kind).clone();
        values.iter().any(|(field, val, boolean)| match (field, val, boolean) {
            (PanelField::OffsetX, Some(v), _) => (v.0 - inst.offset.0).abs() > 0.0005,
            (PanelField::OffsetY, Some(v), _) => (v.0 - inst.offset.1).abs() > 0.0005,
            (PanelField::Scale, Some(v), _) => (v.0 - inst.scale).abs() > 0.0005,
            (PanelField::Z, Some(v), _) => v.0 as i32 != inst.z,
            (PanelField::VisiblePlay, _, Some(b)) => b.0 != inst.visible_play,
            (PanelField::VisiblePractice, _, Some(b)) => b.0 != inst.visible_practice,
            _ => false,
        })
    };
    if !dirty {
        if !buttons.pressed(MouseButton::Left) {
            *snapped_this_hold = false;
        }
        return;
    }
    if !*snapped_this_hold {
        undo.push(&layouts, &lanes);
        *snapped_this_hold = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        *snapped_this_hold = false;
    }

    // Geometry-dependent conversion context.
    let Ok(window) = windows.single() else { return };
    let wsize = Vec2::new(window.width(), window.height());

    for (field, val, boolean) in &values {
        let Some(inst) = layouts.0.get_mut(&kind) else { continue };
        // Position/scale edits require Anchored; visibility/z don't.
        let needs_anchor = matches!(
            field,
            PanelField::OffsetX | PanelField::OffsetY | PanelField::Scale
        );
        if needs_anchor {
            if let Some(g) = geoms.0.get(&kind).copied() {
                let sc = wsize / 2.0;
                let visual_min = crate::widget_layout::transform_point(
                    g.unscaled.min, sc, g.applied_translation, g.applied_scale);
                let parent = crate::widget_layout::parent_rect_px(inst.space, wsize, &pfl);
                super::drag::ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
            }
        }
        match (field, val, boolean) {
            (PanelField::OffsetX, Some(v), _) => inst.offset.0 = v.0,
            (PanelField::OffsetY, Some(v), _) => inst.offset.1 = v.0,
            (PanelField::Scale, Some(v), _) => inst.scale = super::drag::clamp_scale(v.0),
            (PanelField::Z, Some(v), _) => inst.z = v.0 as i32,
            (PanelField::VisiblePlay, _, Some(b)) => inst.visible_play = b.0,
            (PanelField::VisiblePractice, _, Some(b)) => inst.visible_practice = b.0,
            _ => {}
        }
    }
}

/// Anchor grid clicks: rewrite anchor+origin, keep the widget's visual
/// position (no-jump — recompute offset via offset_for_top_left).
fn apply_anchor_cells(
    selection: Res<Selection>,
    cells: Query<(&AnchorCell, &Interaction), Changed<Interaction>>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cell_bg: Query<(&AnchorCell, &mut BackgroundColor)>,
    theme: Res<dtx_ui::ThemeResource>,
) {
    let Some(kind) = selection.0 else { return };
    let mut clicked: Option<Anchor9> = None;
    for (cell, interaction) in &cells {
        if *interaction == Interaction::Pressed {
            clicked = Some(cell.0);
        }
    }
    let Some(new_anchor) = clicked else { return };
    let Ok(window) = windows.single() else { return };
    let Some(g) = geoms.0.get(&kind).copied() else { return };
    undo.push(&layouts, &lanes);
    let Some(inst) = layouts.0.get_mut(&kind) else { return };
    let wsize = Vec2::new(window.width(), window.height());
    let sc = wsize / 2.0;
    let visual_min = crate::widget_layout::transform_point(
        g.unscaled.min, sc, g.applied_translation, g.applied_scale);
    let parent = crate::widget_layout::parent_rect_px(inst.space, wsize, &pfl);
    super::drag::ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
    inst.anchor = new_anchor;
    inst.origin = new_anchor;
    let off_px = dtx_layout::offset_for_top_left(
        inst.anchor,
        inst.origin,
        (g.unscaled.width(), g.unscaled.height()),
        inst.scale,
        (visual_min.x, visual_min.y),
        parent,
    );
    inst.offset = (off_px.0 / pfl.scale, off_px.1 / pfl.scale);
    for (cell, mut bg) in &mut cell_bg {
        bg.0 = if cell.0 == new_anchor {
            theme.0.accent
        } else {
            Color::srgb(0.14, 0.14, 0.18)
        };
    }
}

fn handle_reset(
    resets: Query<&Interaction, (With<PanelResetWidget>, Changed<Interaction>)>,
    selection: Res<Selection>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    for interaction in &resets {
        if *interaction == Interaction::Pressed {
            if let Some(kind) = selection.0 {
                undo.push(&layouts, &lanes);
                super::save::reset_widget(&mut layouts, kind);
            }
        }
    }
}

/// External mutations (undo/redo, canvas drag, reset) → push values back into
/// the visible controls so the panel never shows stale numbers. Guarded to
/// avoid write-back loops: only touch a control whose value actually differs.
fn refresh_panel_values(
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    mut values: Query<(&PanelField, Option<&mut ControlValue>, Option<&mut ControlBool>)>,
) {
    if !layouts.is_changed() {
        return;
    }
    let Some(kind) = selection.0 else { return };
    let Some(inst) = layouts.0.get(&kind) else { return };
    for (field, val, boolean) in &mut values {
        let want = match field {
            PanelField::OffsetX => Some(inst.offset.0),
            PanelField::OffsetY => Some(inst.offset.1),
            PanelField::Scale => Some(inst.scale),
            PanelField::Z => Some(inst.z as f32),
            _ => None,
        };
        if let (Some(w), Some(mut v)) = (want, val) {
            if (v.0 - w).abs() > 0.0005 {
                v.0 = w;
            }
        }
        let want_b = match field {
            PanelField::VisiblePlay => Some(inst.visible_play),
            PanelField::VisiblePractice => Some(inst.visible_practice),
            _ => None,
        };
        if let (Some(w), Some(mut b)) = (want_b, boolean) {
            if b.0 != w {
                b.0 = w;
            }
        }
    }
}
```

Feedback-loop note: `refresh_panel_values` writing `ControlValue` marks it Changed → `apply_panel_controls` would re-apply the same value next frame — harmless (writes identical value; `Changed<WidgetLayouts>`? `layouts.0.get_mut` marks changed → one extra refresh cycle that then converges because values are equal and guarded by `abs() > 0.0005`). The equality guards on BOTH sides are what terminate the loop — do not remove them.

- [ ] **Step 5.4: Register + slim the sidebar**

`mod.rs`: `pub mod panel;`, add `panel::plugin` to a plugin tuple with room.

`ui.rs`: delete `EditorButton::ResetWidget` and `EditorButton::NextPreset` variants, their spawn lines, and their match arms (lane preset returns in plan 3's lane panel; reset now lives in the settings panel). Keep `ResetAll`.

- [ ] **Step 5.5: Build + tests + commit**

Run: `cargo test -p gameplay-drums 2>&1 | tail -5` → PASS.

```bash
git add crates/gameplay-drums/src/editor/
git commit -m "feat(editor): right settings panel with anchor grid + knobs"
```

### Task 6: Integration tests

**Files:**
- Create: `crates/gameplay-drums/tests/editor_panel.rs`

- [ ] **Step 6.1: Tests**

```rust
//! Settings panel: placement engine invariants + control math.

use bevy::prelude::*;
use dtx_layout::{default_instance, Anchor9, Placement, WidgetKind};
use gameplay_drums::editor::drag::ensure_anchored;
use gameplay_drums::widget_layout::{transform_point, translation_for, untransform_rect};

#[test]
fn natural_default_is_identity() {
    let inst = default_instance(WidgetKind::Combo);
    assert_eq!(inst.placement, Placement::Natural);
    assert_eq!(inst.offset, (0.0, 0.0));
    assert_eq!(inst.scale, 1.0);
}

#[test]
fn anchored_conversion_then_anchor_rewrite_never_moves_widget() {
    let parent = (0.0, 0.0, 1280.0, 720.0);
    let visual = Vec2::new(900.0, 500.0);
    let size = Vec2::new(150.0, 80.0);
    let mut inst = default_instance(WidgetKind::Combo);
    ensure_anchored(&mut inst, visual, size, parent, 1.0);
    // Simulate the panel's anchor-cell rewrite to BottomRight.
    inst.anchor = Anchor9::BottomRight;
    inst.origin = Anchor9::BottomRight;
    let off = dtx_layout::offset_for_top_left(
        inst.anchor, inst.origin, (size.x, size.y), inst.scale,
        (visual.x, visual.y), parent,
    );
    inst.offset = (off.0, off.1);
    let tl = dtx_layout::resolve_top_left(
        inst.anchor, inst.origin, (size.x, size.y), inst.scale,
        (inst.offset.0, inst.offset.1), parent,
    );
    assert!((tl.0 - visual.x).abs() < 0.001 && (tl.1 - visual.y).abs() < 0.001);
}

#[test]
fn scale_about_screen_center_with_compensation_keeps_top_left() {
    let sc = Vec2::new(640.0, 360.0);
    let unscaled_min = Vec2::new(300.0, 200.0);
    let desired = Vec2::new(300.0, 200.0); // hold position while scaling 2×
    let t = translation_for(desired, unscaled_min, sc, 2.0);
    let vis = transform_point(unscaled_min, sc, t, 2.0);
    assert!((vis - desired).length() < 0.001);
    // And the inversion recovers the unscaled rect.
    let r = Rect::from_corners(unscaled_min, unscaled_min + Vec2::new(100.0, 40.0));
    let vis_rect = Rect::from_corners(
        transform_point(r.min, sc, t, 2.0),
        transform_point(r.max, sc, t, 2.0),
    );
    let back = untransform_rect(vis_rect, sc, t, 2.0);
    assert!((back.min - r.min).length() < 0.001 && (back.max - r.max).length() < 0.001);
}

#[test]
fn layout_file_with_placement_round_trips() {
    let mut map = dtx_layout::SceneSection::default().resolve();
    let combo = map.get_mut(&WidgetKind::Combo).unwrap();
    combo.placement = Placement::Anchored;
    combo.anchor = Anchor9::Center;
    combo.origin = Anchor9::Center;
    combo.offset = (10.0, -5.0);
    combo.scale = 1.5;
    let section = dtx_layout::SceneSection::from_map(&map);
    let back = section.resolve();
    assert_eq!(back[&WidgetKind::Combo].placement, Placement::Anchored);
    assert_eq!(back[&WidgetKind::Combo].anchor, Anchor9::Center);
    assert_eq!(back[&WidgetKind::Combo].scale, 1.5);
}

#[test]
fn v1_file_without_placement_parses_natural() {
    let toml_str = r#"
version = 1
[[scene.gameplay.widgets]]
kind = "combo"
offset = [40.0, -20.0]
"#;
    let file = dtx_layout::parse_with_migrations(toml_str);
    let map = file.scene.resolve();
    assert_eq!(map[&WidgetKind::Combo].placement, Placement::Natural);
    assert_eq!(map[&WidgetKind::Combo].offset, (40.0, -20.0));
}
```

(Check how `SceneSection` nests: if the TOML table is `[[scene.gameplay.widgets]]` vs `[[scene.widgets]]`, mirror what `LayoutFile`/`SceneSection` serde derives actually produce — look at `crates/dtx-layout/src/file.rs` + an existing round-trip test, and fix the literal accordingly.)

- [ ] **Step 6.2: Run**

Run: `cargo test -p gameplay-drums --test editor_panel && cargo test -p dtx-layout` → PASS.

- [ ] **Step 6.3: Commit**

```bash
git add crates/gameplay-drums/tests/editor_panel.rs
git commit -m "test(editor): placement engine + panel invariants"
```

### Task 7: Real-binary verification

- [ ] **Step 7.1:** `cargo test --workspace 2>&1 | tail -8` → all PASS.
- [ ] **Step 7.2:** `timeout 40 cargo run 2>&1 | tail -20; echo "exit=$?"` → `exit=124`, no panic, no schedule-cycle error.
- [ ] **Step 7.3:** Report DONE + manual checklist:
  - Select combo → right panel appears with its values.
  - Offset steppers move the widget live; Shift+click = ±10.
  - Scale slider scales the widget in place (no jump to a corner).
  - Anchor grid click: widget does NOT move; anchor dot/line (plan 1 viz) jumps to the new anchor.
  - Toggles hide/show in play/practice preview; Reset Widget restores defaults.
  - Undo (Ctrl+Z) reverts panel edits and the panel numbers follow.
  - Untouched widgets after save/reload sit pixel-identical to v1.
