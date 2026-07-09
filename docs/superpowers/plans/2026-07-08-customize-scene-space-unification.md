# Customize Scene-Space Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the coordinate split-brain the single-HudRoot-transform refactor left behind (picking/snap/measure still compute in shrunk-StageRect space while placement moved to full-window "scene" space), clamp widget drags so nothing escapes the miniature, and consolidate scattered editor state.

**Architecture:** One rule — all widget math happens in **scene space** (the full-window coordinates HudRoot children lay out in, before the stage `UiTransform` shrinks them). The window-space cursor converts to scene space at exactly one boundary (`window_to_scene`, the inverse of `stage_xform`). Editor overlays (selection box, snap guides, anchor viz) reparent under `HudRoot` so they ride the same transform, keeping their `GlobalZIndex` so they stack above the preview scrim. Bevy cannot clip a `UiTransform`-scaled subtree (`update_clipping` adds only the transform's translation, not scale — verified in bevy_ui 0.19 `src/update.rs`), so escape prevention is a drag-delta clamp instead of masking.

**Tech Stack:** Rust, Bevy 0.19 (bevy_ui `UiTransform`/`UiGlobalTransform`/`GlobalZIndex`), existing crates `gameplay-drums`, `dtx-layout`.

**Worktree:** `/home/lazykern/lab/dtxmaniars-customize`, branch `feat/customize-surface`. All paths below relative to that root.

**Build/test commands** (never bare `cargo fmt`):
- Test one crate: `cargo test -p gameplay-drums`
- Test all: `cargo test --workspace`
- Format ONLY files you touched: `rustfmt --edition 2024 <files>`
- Stage ONLY files named in the task (a format daemon churns unrelated files — leave that unstaged).

**Background facts the executor needs:**
- `HudRoot` (`crates/gameplay-drums/src/hud.rs`) is a 100%×100% UI node, parent of the playfield and every HUD widget container. `stage_rect::apply_stage_transform` writes a uniform `UiTransform` (scale `s`, translation `t`, pivot = node center = window center) that shrinks the whole scene into `StageRect`.
- "Scene space" = the coordinates children of HudRoot lay out in = full-window logical px. When the Customize surface is closed the stage transform is identity, so scene space == window space.
- `UiGlobalTransform` on a node **includes** inherited `UiTransform`s (so measurements taken from it are window-space visual coords). `GlobalZIndex` changes stacking only, never transforms — a HudRoot child with `GlobalZIndex` still rides the stage transform.
- Transform convention (widget_layout.rs): a container `UiTransform` (translation T, uniform scale s) maps unscaled point p to `S + s·(p − S) + T` where S = window center.

---

### Task 1: `window_to_scene` + transform composition helpers

**Files:**
- Modify: `crates/gameplay-drums/src/stage_rect.rs`
- Modify: `crates/gameplay-drums/src/widget_layout.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` module in `crates/gameplay-drums/src/stage_rect.rs`:

```rust
    #[test]
    fn window_to_scene_is_identity_when_full() {
        let w = Vec2::new(1745.0, 1090.0);
        let p = Vec2::new(300.0, 700.0);
        let q = window_to_scene(p, StageRect::full(w), w);
        assert!((q - p).length() < 1e-3);
    }

    #[test]
    fn window_to_scene_inverts_stage_xform() {
        let w = Vec2::new(1745.0, 1090.0);
        let rect = StageRect {
            origin: Vec2::new(496.0, 24.0),
            size: Vec2::new(1233.0, 1042.0),
        };
        let (s, t) = stage_xform(rect, w);
        let c = w * 0.5;
        // Forward: scene point p renders at c + s(p−c) + t.
        let p = Vec2::new(200.0, 900.0);
        let rendered = c + s * (p - c) + t;
        let back = window_to_scene(rendered, rect, w);
        assert!((back - p).length() < 1e-3);
    }
```

Append to the `tests` module in `crates/gameplay-drums/src/widget_layout.rs`:

```rust
    #[test]
    fn compose_about_center_matches_nested_transforms() {
        let sc = Vec2::new(872.0, 545.0);
        let (t_in, s_in) = (Vec2::new(40.0, -20.0), 1.5);
        let (t_out, s_out) = (Vec2::new(-120.0, 60.0), 0.6);
        let p = Vec2::new(300.0, 700.0);
        let nested = transform_point(transform_point(p, sc, t_in, s_in), sc, t_out, s_out);
        let (t_c, s_c) = compose_about_center(t_out, s_out, t_in, s_in);
        let composed = transform_point(p, sc, t_c, s_c);
        assert!((nested - composed).length() < 1e-3);
    }
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p gameplay-drums window_to_scene compose_about_center`
Expected: compile FAIL — `window_to_scene` / `compose_about_center` not found.

- [ ] **Step 3: Implement**

In `crates/gameplay-drums/src/stage_rect.rs`, directly below `stage_xform`:

```rust
/// Inverse of `stage_xform`: map a window-space point (e.g. the cursor) into
/// scene space — the full-window coordinates `HudRoot` children lay out in
/// before the stage transform shrinks them. Identity while the surface is
/// closed (rect == full window).
pub fn window_to_scene(pos: Vec2, rect: StageRect, window: Vec2) -> Vec2 {
    let (s, t) = stage_xform(rect, window);
    let c = window * 0.5;
    c + (pos - t - c) / s
}
```

In `crates/gameplay-drums/src/widget_layout.rs`, directly below `transform_point`:

```rust
/// Compose two scale-about-the-same-center transforms: `outer(inner(p))`.
/// Both pivot on the screen center (Bevy `UiTransform` node-center convention
/// for full-screen nodes), so composition is linear in (T, s).
pub fn compose_about_center(
    t_outer: Vec2,
    s_outer: f32,
    t_inner: Vec2,
    s_inner: f32,
) -> (Vec2, f32) {
    (s_outer * t_inner + t_outer, s_outer * s_inner)
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p gameplay-drums window_to_scene compose_about_center`
Expected: 3 PASS.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/stage_rect.rs crates/gameplay-drums/src/widget_layout.rs
git add crates/gameplay-drums/src/stage_rect.rs crates/gameplay-drums/src/widget_layout.rs
git commit -m "feat(customize): window_to_scene inverse + transform composition helpers"
```

---

### Task 2: `measure_widget_geoms` strips the stage transform

`WidgetGeoms.unscaled` is currently polluted on shrunk tabs: the measured `UiGlobalTransform` includes the HudRoot stage transform, but the inversion only strips the container's own transform.

**Files:**
- Modify: `crates/gameplay-drums/src/widget_layout.rs:159-216` (`measure_widget_geoms`)

- [ ] **Step 1: Modify the system**

Replace the `measure_widget_geoms` signature and body:

```rust
/// Measure every widget container's visual content rect and invert the applied
/// transforms to keep `WidgetGeoms` in unscaled SCENE space. The measured
/// `UiGlobalTransform` includes BOTH the container's own `UiTransform` and the
/// inherited HudRoot stage transform, so the inversion must strip their
/// composition; `applied_*` still stores only the container's own transform
/// (consumers reconstruct scene-space visual rects with it). Runs every frame
/// in Performance (cheap: ~10 widgets, shallow trees).
fn measure_widget_geoms(
    mut geoms: ResMut<WidgetGeoms>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    roots: Query<&UiTransform, With<crate::hud::HudRoot>>,
    containers: Query<(Entity, &WidgetContainer, &UiTransform), Without<crate::hud::HudRoot>>,
    children_q: Query<&Children>,
    nodes: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let sc = Vec2::new(window.width() / 2.0, window.height() / 2.0);
    let (stage_t, stage_s) = roots
        .single()
        .map(|tf| applied_of(tf))
        .unwrap_or((Vec2::ZERO, 1.0));
    for (entity, container, ui_tf) in &containers {
        let kind = container.0;
        if kind == WidgetKind::Playfield {
            continue;
        }
        let (t, s) = applied_of(ui_tf);
        let (t_comp, s_comp) = compose_about_center(stage_t, stage_s, t, s);
        let mut union: Option<Rect> = None;
        let mut stack: Vec<Entity> = children_q
            .get(entity)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        while let Some(e) = stack.pop() {
            // UI nodes carry `UiGlobalTransform` (not `GlobalTransform`); its
            // translation is the node center in physical px and already
            // includes the container's UiTransform AND the HudRoot stage
            // transform. Rendered size is the layout size times the composed
            // scale, so form the VISUAL rect here and let `untransform_rect`
            // (with the composed transform) recover unscaled scene space.
            if let Ok((cn, gt)) = nodes.get(e) {
                if cn.size().x > 0.0 && cn.size().y > 0.0 {
                    let inv = cn.inverse_scale_factor();
                    let center = gt.translation * inv;
                    let size = cn.size() * inv * s_comp;
                    let r = Rect::from_center_size(center, size);
                    union = Some(union.map_or(r, |u| u.union(r)));
                }
            }
            if let Ok(c) = children_q.get(e) {
                stack.extend(c.iter());
            }
        }
        if let Some(measured) = union.filter(|r| r.width() >= 1.0 && r.height() >= 1.0) {
            let unscaled = untransform_rect(measured, sc, t_comp, s_comp);
            geoms.0.insert(
                kind,
                WidgetGeom {
                    unscaled,
                    applied_translation: t,
                    applied_scale: s,
                },
            );
        } else if let Some(g) = geoms.0.get_mut(&kind) {
            // Keep last-known unscaled rect; just refresh the applied transform.
            g.applied_translation = t;
            g.applied_scale = s;
        }
    }
}
```

Notes for the executor:
- The `Without<crate::hud::HudRoot>` filter on `containers` is required for query disjointness with `roots` (both access `&UiTransform`).
- `roots.single()` returns `Result` in Bevy 0.19 — the `.map().unwrap_or()` above handles the no-HudRoot case (identity).
- The union rect check `width() >= 1.0` can now reject shrunk-but-real content when `stage_s` is small; that is fine — measurement while shrunk refreshes `applied_*` and keeps the last unscaled rect, and the important measurements happen at identity too.

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: all PASS (system not unit-tested directly; math covered by Task 1's composition test + existing `transform_math_round_trips`).

- [ ] **Step 3: Format + commit**

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/widget_layout.rs
git add crates/gameplay-drums/src/widget_layout.rs
git commit -m "fix(customize): measure widget geoms through the composed stage transform"
```

---

### Task 3: picking in scene space

**Files:**
- Modify: `crates/gameplay-drums/src/editor/picking.rs`

- [ ] **Step 1: `collect_widget_aabbs` uses the window center**

Replace the `rect: Res<crate::stage_rect::StageRect>` parameter with a window query, and `let sc = rect.center();` with the window center. The signature becomes:

```rust
fn collect_widget_aabbs(
    mut aabbs: ResMut<WidgetAabbs>,
    mut hidden: ResMut<CanvasHidden>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    pfl: Res<PlayfieldLayout>,
    windows: Query<&Window>,
) {
    let Ok(window) = windows.single() else { return };
    let sc = Vec2::new(window.width() / 2.0, window.height() / 2.0);
```

Rest of the body unchanged (the `transform_point` calls now produce scene-space rects; the Playfield AABB from `PlayfieldLayout` is already scene-space). Update the doc comment on `WidgetAabbs`:

```rust
/// SCENE-space (full-window logical px, pre stage-transform) AABB per widget,
/// rebuilt each frame while the editor is open. Entries persist across frames
/// (last non-empty wins) so widgets that render nothing this instant (e.g.
/// judgment popup between hits) stay grabbable.
```

- [ ] **Step 2: `update_hover` converts the cursor**

Add `rect: Res<crate::stage_rect::StageRect>` to `update_hover`'s parameters and convert before hit-testing:

```rust
    let Some(pos) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    let pos = crate::stage_rect::window_to_scene(
        pos,
        *rect,
        Vec2::new(window.width(), window.height()),
    );
```

`update_cursor_over_chrome` stays window-space — chrome is top-level UI, not under HudRoot.

- [ ] **Step 3: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (picking unit tests are pure and space-agnostic).

- [ ] **Step 4: Format + commit**

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/picking.rs
git add crates/gameplay-drums/src/editor/picking.rs
git commit -m "fix(customize): hit-test widgets in scene space via cursor inverse transform"
```

---

### Task 4: drag in scene space

**Files:**
- Modify: `crates/gameplay-drums/src/editor/drag.rs`
- Modify: `crates/gameplay-drums/src/editor/selection_box.rs` (make `HANDLE_SIZE` pub)

- [ ] **Step 1: Export `HANDLE_SIZE`**

In `selection_box.rs` change `const HANDLE_SIZE: f32 = 10.0;` to `pub const HANDLE_SIZE: f32 = 10.0;`.

- [ ] **Step 2: `begin_gesture` — scene cursor + AABB-derived handle hit-test**

In `begin_gesture`, right after `let Some(pos) = window.cursor_position() else { ... };` insert:

```rust
    let win_size = Vec2::new(window.width(), window.height());
    let pos = crate::stage_rect::window_to_scene(pos, *rect, win_size);
```

Replace the whole scale-handle block (the `for (_, cn, gt) in &handles` loop and the `handles` query parameter) with AABB-corner hit-testing — the selection box will live under HudRoot (Task 6), so its nodes' `UiGlobalTransform` is stage-transformed and `node_rect` would need un-transforming; deriving handle rects from the scene-space AABB is simpler and exact. Delete the `handles: Query<...>` parameter entirely and replace the block with:

```rust
    // 1. Scale handles first (they can overhang neighboring widgets). Handle
    // rects are derived from the selected widget's scene-space AABB corners
    // (the visual handles are children of the selection box and sit exactly
    // on those corners).
    if let Some(kind) = selection.0 {
        if kind != dtx_layout::WidgetKind::Playfield {
            if let Some(aabb) = aabbs.0.get(&kind) {
                let grab = super::selection_box::HANDLE_SIZE + 6.0;
                let corners = [
                    aabb.min,
                    Vec2::new(aabb.max.x, aabb.min.y),
                    Vec2::new(aabb.min.x, aabb.max.y),
                    aabb.max,
                ];
                if corners
                    .iter()
                    .any(|c| Rect::from_center_size(*c, Vec2::splat(grab)).contains(pos))
                {
                    let start_center = aabb.center();
                    let start_dist = (pos - start_center).length().max(1.0);
                    let start_scale = layouts.get(kind).scale;
                    undo.push(&layouts, &lanes);
                    convert_to_anchored(&mut layouts, &geoms, &pfl, win_size, kind);
                    gesture.0 = Gesture::Scale {
                        start_dist,
                        start_scale,
                        start_center,
                    };
                    return;
                }
            }
        }
    }
```

In the widget-pick branch below it, update the `convert_to_anchored` call the same way (`win_size` instead of `*rect`).

- [ ] **Step 3: `convert_to_anchored` — full-window parent + window center**

Replace the function:

```rust
/// Convert a widget Natural→Anchored at gesture start, capturing its current
/// scene-space visual top-left (from the geom pushed through its applied
/// transform — NOT `WidgetAabbs`, whose rects are inflated to MIN_GRAB for tiny
/// widgets) so the widget doesn't jump. All math in scene space: parent is the
/// full window, matching `apply_widget_layout`.
fn convert_to_anchored(
    layouts: &mut WidgetLayouts,
    geoms: &crate::widget_layout::WidgetGeoms,
    pfl: &crate::layout::PlayfieldLayout,
    window: Vec2,
    kind: WidgetKind,
) {
    if let Some(g) = geoms.0.get(&kind).copied() {
        if let Some(inst) = layouts.0.get_mut(&kind) {
            let full = crate::stage_rect::StageRect::full(window);
            let sc = full.center();
            let visual_min = crate::widget_layout::transform_point(
                g.unscaled.min,
                sc,
                g.applied_translation,
                g.applied_scale,
            );
            let parent = crate::widget_layout::parent_rect_px(inst.space, full, pfl);
            ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
        }
    }
}
```

- [ ] **Step 4: `update_gesture` — scene cursor, drop the stage_s hack**

Replace the cursor/scale section of `update_gesture` (from `let Some(pos) = ...` through `let drag_scale = ...`) with:

```rust
    let Some(pos) = window.cursor_position() else {
        return;
    };
    // Cursor converts to scene space at the boundary, so gesture deltas and
    // distances are scene-px; the only remaining unit change is scene-px →
    // ref-px, i.e. `pfl.scale`.
    let pos = crate::stage_rect::window_to_scene(
        pos,
        *rect,
        Vec2::new(window.width(), window.height()),
    );
    let drag_scale = pfl.scale;
```

`Gesture::Move { last_cursor }` and `Gesture::Scale { start_center, .. }` now hold scene-space points; `begin_gesture` already stores them converted (Step 2). Update the `Gesture` doc comment: `/// Active mouse gesture (cursor points in scene space). Scale carries drag-start reference data.`

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: PASS (drag unit tests are pure).

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/drag.rs crates/gameplay-drums/src/editor/selection_box.rs
git add crates/gameplay-drums/src/editor/drag.rs crates/gameplay-drums/src/editor/selection_box.rs
git commit -m "fix(customize): drag gestures operate in scene space"
```

---

### Task 5: snap + guides in scene space

**Files:**
- Modify: `crates/gameplay-drums/src/editor/snap.rs`

- [ ] **Step 1: `apply_anchor_snap` full-window parent**

Replace the `rect: Res<crate::stage_rect::StageRect>` parameter with `windows: Query<&Window>`, and inside, replace

```rust
    let sc = rect.center();
```
```rust
    let (px, py, pw, ph) = parent_rect_px(inst_ro.space, *rect, &pfl);
```

with

```rust
    let Ok(window) = windows.single() else { return };
    let full = crate::stage_rect::StageRect::full(Vec2::new(window.width(), window.height()));
    let sc = full.center();
```
```rust
    let (px, py, pw, ph) = parent_rect_px(inst_ro.space, full, &pfl);
```

- [ ] **Step 2: `sync_snap_guides` full-window parent**

Same substitution: replace the `rect` parameter with `windows: Query<&Window>` and

```rust
    let (px, py, pw, ph) = parent_rect_px(layouts.get(kind).space, *rect, &pfl);
```

with

```rust
    let Ok(window) = windows.single() else { return };
    let full = crate::stage_rect::StageRect::full(Vec2::new(window.width(), window.height()));
    let (px, py, pw, ph) = parent_rect_px(layouts.get(kind).space, full, &pfl);
```

(Guide nodes render these scene coords correctly once reparented under HudRoot — Task 6.)

- [ ] **Step 3: Run tests, format, commit**

Run: `cargo test -p gameplay-drums` — expected PASS.

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/snap.rs
git add crates/gameplay-drums/src/editor/snap.rs
git commit -m "fix(customize): anchor snap and guides compute against the full-window parent"
```

---

### Task 6: reparent editor overlays under HudRoot

Selection box, hover outline, anchor viz, and snap guides position from scene-space rects now, so they must ride the stage transform. They KEEP their `GlobalZIndex` — `GlobalZIndex` affects stacking only, transforms still inherit, and staying global keeps them above the `PreviewScrim` (GlobalZIndex 1500). Also fix the bindings overlay, which currently uses local `ZIndex` and gets dimmed by the scrim.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/selection_box.rs` (`spawn_overlay_on_open`)
- Modify: `crates/gameplay-drums/src/editor/snap.rs` (`spawn_guides_on_open`)
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs` (`ZIndex` → `GlobalZIndex`)

- [ ] **Step 1: selection_box — parent everything to HudRoot**

In `spawn_overlay_on_open`, add the query parameter

```rust
    roots: Query<Entity, With<crate::hud::HudRoot>>,
```

then after the early-outs (`if !open.0 { return; }`) add:

```rust
    let Ok(root) = roots.single() else {
        return;
    };
```

Change every top-level `commands.spawn((...))` in this function to capture ids and attach them:

```rust
    let hover = commands
        .spawn((
            EditorOverlay,
            HoverOutlineRoot,
            /* ... existing components unchanged ... */
        ))
        .id();
    let selection = commands
        .spawn((/* SelectionBoxRoot bundle unchanged */))
        .with_children(|p| { /* handles + name tag unchanged */ })
        .id();
    let line = commands.spawn((/* AnchorLine bundle unchanged */)).id();
    let a_dot = commands.spawn((/* AnchorDot bundle unchanged */)).id();
    let o_dot = commands.spawn((/* OriginDot bundle unchanged */)).id();
    commands
        .entity(root)
        .add_children(&[hover, selection, line, a_dot, o_dot]);
```

Keep every `GlobalZIndex(...)` exactly as-is. Update the module doc comment: overlays are HudRoot children (they ride the stage transform; scene coords render 1:1) and keep `GlobalZIndex` to stack above the preview scrim.

- [ ] **Step 2: snap guides — same reparent**

In `spawn_guides_on_open` add the same `roots` query + `let Ok(root) = roots.single() else { return; };`, capture each guide's id and attach:

```rust
    let mut ids = Vec::with_capacity(4);
    for vertical in [true, false] {
        for which in [1u8, 2u8] {
            ids.push(
                commands
                    .spawn((/* existing guide bundle unchanged, keep GlobalZIndex(2050) */))
                    .id(),
            );
        }
    }
    commands.entity(root).add_children(&ids);
```

- [ ] **Step 3: bindings overlay — back to GlobalZIndex**

In `bindings_spatial.rs`'s `spawn_overlay_on_open`, change both `ZIndex(OUTLINE_Z)` components to `GlobalZIndex(OUTLINE_Z)` (entities stay HudRoot children). Update the `OUTLINE_Z` doc comment: global so the selected-lane accent stacks above the preview scrim (1500) and the stage outline (1900), under chrome (2000).

- [ ] **Step 4: Run tests, format, commit**

Run: `cargo test -p gameplay-drums` — expected PASS.

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/selection_box.rs crates/gameplay-drums/src/editor/snap.rs crates/gameplay-drums/src/editor/bindings_spatial.rs
git add crates/gameplay-drums/src/editor/selection_box.rs crates/gameplay-drums/src/editor/snap.rs crates/gameplay-drums/src/editor/bindings_spatial.rs
git commit -m "fix(customize): editor overlays ride the HudRoot stage transform, stack above the scrim"
```

---

### Task 7: clamp drag + nudge inside the window

Scene space makes this trivial: the miniature's bounds ARE the window rect in scene coordinates. Clamp the **delta**, not the position — a widget that is somehow already out of bounds can always move back in, never further out.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/drag.rs`

- [ ] **Step 1: Write failing tests**

Append to `drag.rs` tests:

```rust
    #[test]
    fn clamp_delta_free_when_inside() {
        let aabb = Rect::new(100.0, 100.0, 200.0, 150.0);
        let w = Vec2::new(1280.0, 720.0);
        assert_eq!(
            clamp_delta(aabb, Vec2::new(10.0, -20.0), w),
            Vec2::new(10.0, -20.0)
        );
    }

    #[test]
    fn clamp_delta_stops_at_edges() {
        let aabb = Rect::new(100.0, 100.0, 200.0, 150.0);
        let w = Vec2::new(1280.0, 720.0);
        // Trying to move 200 left only allows 100 (aabb.min.x).
        assert_eq!(
            clamp_delta(aabb, Vec2::new(-200.0, 0.0), w).x,
            -100.0
        );
        // Trying to move 2000 right only allows window − aabb.max.x = 1080.
        assert_eq!(clamp_delta(aabb, Vec2::new(2000.0, 0.0), w).x, 1080.0);
    }

    #[test]
    fn clamp_delta_out_of_bounds_can_only_return() {
        // AABB hangs off the left edge: further left is blocked, right is open.
        let aabb = Rect::new(-50.0, 100.0, 50.0, 150.0);
        let w = Vec2::new(1280.0, 720.0);
        assert_eq!(clamp_delta(aabb, Vec2::new(-30.0, 0.0), w).x, 0.0);
        assert!(clamp_delta(aabb, Vec2::new(30.0, 0.0), w).x > 0.0);
    }
```

- [ ] **Step 2: Run tests, verify fail**

Run: `cargo test -p gameplay-drums clamp_delta`
Expected: compile FAIL — `clamp_delta` not found.

- [ ] **Step 3: Implement**

Add below `apply_drag` in `drag.rs`:

```rust
/// Clamp a scene-px move delta so the widget's AABB stays inside the window
/// (the miniature's true screen bounds — Bevy can't clip a transformed
/// subtree, so escapes are prevented at the gesture instead). Clamps the
/// delta, not the position: an out-of-bounds widget can move back in but
/// never further out.
pub fn clamp_delta(aabb: Rect, delta: Vec2, window: Vec2) -> Vec2 {
    let lo = -aabb.min;
    let hi = window - aabb.max;
    Vec2::new(
        delta.x.clamp(lo.x.min(0.0), hi.x.max(0.0)),
        delta.y.clamp(lo.y.min(0.0), hi.y.max(0.0)),
    )
}
```

- [ ] **Step 4: Wire into `update_gesture` (Move) and `nudge_selected_widget`**

`update_gesture`: add parameter `aabbs: Res<super::picking::WidgetAabbs>`, and in the `Gesture::Move` arm replace the delta application with:

```rust
        Gesture::Move { last_cursor } => {
            let mut delta = pos - last_cursor;
            if let Some(aabb) = aabbs.0.get(&kind) {
                delta = clamp_delta(
                    *aabb,
                    delta,
                    Vec2::new(window.width(), window.height()),
                );
            }
            if delta != Vec2::ZERO {
                if let Some(inst) = layouts.0.get_mut(&kind) {
                    inst.offset = apply_drag(inst.offset, delta, drag_scale);
                }
            }
            gesture.0 = Gesture::Move { last_cursor: pos };
        }
```

`nudge_selected_widget`: add parameters `aabbs: Res<super::picking::WidgetAabbs>`, `pfl: Res<crate::layout::PlayfieldLayout>`, `windows: Query<&Window>`, and replace the final application with:

```rust
    if d != (0.0, 0.0) {
        let mut scene_d = Vec2::new(d.0, d.1) * pfl.scale;
        if let (Ok(window), Some(aabb)) = (windows.single(), aabbs.0.get(&kind)) {
            scene_d = clamp_delta(
                *aabb,
                scene_d,
                Vec2::new(window.width(), window.height()),
            );
        }
        let scale = pfl.scale.max(f32::EPSILON);
        if let Some(inst) = layouts.0.get_mut(&kind) {
            inst.offset.0 += scene_d.x / scale;
            inst.offset.1 += scene_d.y / scale;
        }
    }
```

- [ ] **Step 5: Run tests, format, commit**

Run: `cargo test -p gameplay-drums` — expected PASS including 3 new.

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/drag.rs
git add crates/gameplay-drums/src/editor/drag.rs
git commit -m "feat(customize): clamp widget drag/nudge inside the screen bounds"
```

---

### Task 8: `PreviewState` resource (kill the scattered Tab checks)

Four systems independently re-read `keys.pressed(KeyCode::Tab)` + tab + open. Compute once, read everywhere.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (resource + update system, registered before consumers)
- Modify: `crates/gameplay-drums/src/editor/stage.rs` (`peek_stage`, `sync_stage_outline`, `sync_preview_scrim`)
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs` (`sync_bind_overlay`)
- Modify: `crates/gameplay-drums/src/widget_layout.rs` (`apply_widget_layout`, `hide_practice_hud_on_preview` gates)

- [ ] **Step 1: Define + update the resource**

In `editor/mod.rs` add:

```rust
/// Single source of truth for the Customize preview's frame state. Computed
/// once per frame (before every consumer); systems read this instead of
/// re-deriving open/peek/tab/inspector themselves.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PreviewState {
    pub open: bool,
    /// Tab held: full play view peek (chrome + overlays hidden, identity rect).
    pub peeking: bool,
    pub tab: game_shell::CustomizeTab,
    /// Widgets tab with a live selection → right inspector reserves space.
    pub has_inspector: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            open: false,
            peeking: false,
            tab: game_shell::CustomizeTab::default(),
            has_inspector: false,
        }
    }
}

fn update_preview_state(
    open: Res<EditorOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<tabs::ActiveTab>,
    selection: Res<drag::Selection>,
    mut state: ResMut<PreviewState>,
) {
    let next = PreviewState {
        open: open.0,
        peeking: open.0 && keys.pressed(KeyCode::Tab),
        tab: active.0,
        has_inspector: active.0 == game_shell::CustomizeTab::Widgets && selection.0.is_some(),
    };
    if *state != next {
        *state = next;
    }
}
```

Register `init_resource::<PreviewState>()` and add `update_preview_state` to `Update` ordered `.before(...)` the editor system sets (or simply `.in_set(EditorPickSet)` predecessor — match the existing set layout in `editor/mod.rs`; the requirement is: runs before `stage::plugin`'s, `bindings_spatial`'s, and `widget_layout`'s consumers each frame).

If `game_shell::CustomizeTab` has no `Default`, use the first settings tab (check `game_shell` — whatever `ActiveTab::default()` uses; mirror it).

- [ ] **Step 2: Convert consumers**

Each consumer drops its own `keys`/`active`/`selection` reads where they exist purely for this logic:

- `stage.rs peek_stage`: replace `keys` + `active` + `selection` params with `state: Res<PreviewState>`; `let peeking = state.peeking;` `let has_inspector = state.has_inspector;` `preset_rect(state.tab, ...)`.
- `stage.rs sync_stage_outline`: replace `keys` + `active` with `state: Res<PreviewState>`; `let show = !state.peeking && !state.tab.is_settings();`.
- `stage.rs sync_preview_scrim`: replace `keys` with `state: Res<PreviewState>`; `let show = !state.peeking;`.
- `bindings_spatial.rs sync_bind_overlay`: replace `keys` + `active` with `state: Res<PreviewState>`; `let peeking = state.peeking;` `let on_bindings = state.tab == game_shell::CustomizeTab::Bindings;`.
- `widget_layout.rs apply_widget_layout`: replace `open` + `active` with `state: Res<crate::editor::PreviewState>`; `let hide_for_preview = state.open && state.tab != game_shell::CustomizeTab::Widgets && container.0 != WidgetKind::Playfield;`. In the `run_if`, replace `resource_changed::<crate::editor::tabs::ActiveTab>.or_else(resource_changed::<crate::editor::EditorOpen>)` with `resource_changed::<crate::editor::PreviewState>` (the `if *state != next` guard in Step 1 makes this precise change detection).
- `widget_layout.rs hide_practice_hud_on_preview`: same replacement (params + `run_if`).

- [ ] **Step 3: Run tests, format, commit**

Run: `cargo test --workspace` — expected all PASS (this touches the schedule; use the FixedUpdate ordering-guard test awareness: gameplay-drums schedule must still build — run the ordering guard test if present).

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/stage.rs crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/widget_layout.rs
git add crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/stage.rs crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/widget_layout.rs
git commit -m "refactor(customize): single PreviewState resource replaces scattered peek/tab checks"
```

---

### Task 9: z-index registry + shared chrome widths

**Files:**
- Create: `crates/gameplay-drums/src/ui_z.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (add `pub mod ui_z;`)
- Modify: every file found by the greps below
- Modify: `crates/gameplay-drums/src/editor/stage.rs`, `crates/gameplay-drums/src/editor/ui.rs`, `crates/gameplay-drums/src/editor/panel.rs` (chrome widths)

- [ ] **Step 1: Create the registry**

`crates/gameplay-drums/src/ui_z.rs`:

```rust
//! Global z-index registry for gameplay-drums UI stacking. `GlobalZIndex`
//! creates one global stacking order across the whole UI; every layer that
//! participates is named here so collisions are greppable instead of folklore.

/// Combo counter (above the playfield, below every overlay).
pub const COMBO: i32 = 20;
/// Practice HUD layers.
pub const PRACTICE: i32 = 900;
/// Pause overlay.
pub const PAUSE: i32 = 1000;
/// Stage-end results overlay.
pub const STAGE_END: i32 = 1100;
/// Customize: full-window dim scrim (above all HUD, below editor layers).
pub const PREVIEW_SCRIM: i32 = 1500;
/// Customize: miniature bounds outline.
pub const STAGE_OUTLINE: i32 = 1900;
/// Customize: bindings selected-lane overlay.
pub const BIND_OVERLAY: i32 = 1910;
/// Customize: chrome (rail, panels, inspector).
pub const EDITOR_CHROME: i32 = 2000;
/// Customize: snap guide lines.
pub const SNAP_GUIDES: i32 = 2050;
/// Customize: hover outline.
pub const HOVER_OUTLINE: i32 = 2100;
/// Customize: anchor line/dots.
pub const ANCHOR_VIZ: i32 = 2150;
/// Customize: selection box (topmost editor visual).
pub const SELECTION_BOX: i32 = 2200;
```

- [ ] **Step 2: Replace magic numbers**

Run: `grep -rn "GlobalZIndex(" crates/gameplay-drums/src/ | grep -v ui_z`
For each hit, replace the literal with the matching `crate::ui_z::` constant per the table above (the values must not change — this is a pure rename). Where a file defines its own `const ..._Z` (stage.rs `OUTLINE_Z`/`SCRIM_Z`, bindings_spatial.rs `OUTLINE_Z`), delete the local const and use the registry. If a grep hit has a value NOT in the table (other than 20/900/1000/1100/1500/1900/1910/2000/2050/2100/2150/2200), add a named constant for it rather than leaving a literal.

- [ ] **Step 3: Chrome widths — single definition**

Run: `grep -rn "132.0\|348.0\|236.0" crates/gameplay-drums/src/editor/`
`stage.rs` currently mirrors panel/rail widths by comment-contract. Make `stage.rs`'s constants `pub`:

```rust
/// Tabs-only rail width. SINGLE SOURCE — editor/ui.rs sizes the rail from this.
pub const RAIL_WIDTH: f32 = 132.0;
/// Left content panel width. SINGLE SOURCE — editor/panel.rs sizes from this.
pub const LEFT_PANEL_WIDTH: f32 = 348.0;
/// Right inspector panel width. SINGLE SOURCE — editor/panel.rs sizes from this.
pub const INSPECTOR_WIDTH: f32 = 236.0;
```

and change the literal widths in `editor/ui.rs` / `editor/panel.rs` node definitions to `super::stage::RAIL_WIDTH` etc. (grep confirms exact sites).

- [ ] **Step 4: Run tests, format, commit**

Run: `cargo test --workspace` — expected all PASS.

```bash
rustfmt --edition 2024 crates/gameplay-drums/src/ui_z.rs <every file touched in steps 2-3>
git add crates/gameplay-drums/src/ui_z.rs crates/gameplay-drums/src/lib.rs <every file touched>
git commit -m "refactor(customize): named z-index registry + single-source chrome widths"
```

---

### Task 10: BRP verification + docs

**Files:**
- Modify: `docs/superpowers/customize-visual-punchlist.md`

- [ ] **Step 1: Build + launch**

```bash
cargo build -p dtxmaniars
```
Launch via BRP MCP: `brp_launch` target `dtxmaniars`, path `/home/lazykern/lab/dtxmaniars-customize`, profile debug. Navigate: Enter (title) → Enter (song) → wait ~5s → `Ctrl+Shift+E` (ControlLeft+ShiftLeft+KeyE). Mouse coords are LOGICAL = physical/1.65 (window 2879×1800 physical → ~1745×1090 logical).

- [ ] **Step 2: Verify checklist (screenshot each)**

1. Normal play (before opening surface): full brightness, no overlays — identity preserved.
2. Widgets tab: miniature framed + dimmed; **click a widget in the shrunk miniature** → selection box appears glued to it INSIDE the miniature (scene-space picking + reparented overlay).
3. Drag the widget: box + widget track the cursor 1:1; snap guides appear at thirds INSIDE the miniature; anchor dot moves on snap.
4. Drag hard toward an edge: widget stops at the miniature's frame (clamp) — cannot escape.
5. Arrow-key nudge at the edge: also stops.
6. Bindings tab: select a channel → lane outline + source label glued to the lane, NOT dimmed by the scrim (GlobalZIndex fix).
7. Hold Tab: full-brightness play view, all overlays/scrim/outline hidden; release: restored.
8. Settings tab: clean shifted lanes+notes, no outline, no widget bleed.
9. Close surface (Ctrl+Shift+E): normal play byte-identical, nothing leaks.
10. Dim re-eval with user: with the clamp + framed miniature, ask whether the 0.72 scrim alpha should stay/lighten/go (one constant: `ui_z`-adjacent `BackgroundColor` in `stage.rs spawn_outline_on_open`).

- [ ] **Step 3: Update punchlist + commit**

Update `docs/superpowers/customize-visual-punchlist.md`: add a section "Scene-space unification (post-P2)" recording the split-brain fix, the clamp, the Bevy clipping limitation (translation-only clip rects), and the overlay reparent.

```bash
git add docs/superpowers/customize-visual-punchlist.md
git commit -m "docs(customize): record scene-space unification + clamp + clipping limitation"
```

---

## Self-review notes

- **Spec coverage:** split-brain (T1-T5), overlays (T6), escape/clamp (T7), scattered state (T8), z folklore + width drift (T9), verification + dim decision (T10). Closest-anchor snap needed NO task — already implemented in `editor/snap.rs` with osu semantics.
- **Ordering constraint:** T2 and T3 must both land before BRP-verifying shrunk-tab picking (T2 alone leaves `collect_widget_aabbs` using the wrong center). Tests stay green throughout; behavior is only fully correct after T5.
- **Known risk:** T8 touches system scheduling — green unit tests don't prove the schedule builds; run the app once (or the ordering-guard test) after T8.
- **Deliberately out of scope:** real masking/clipping of the miniature (Bevy limitation), flat lane labels (user chose domes), slider styling (P4 cosmetic).
