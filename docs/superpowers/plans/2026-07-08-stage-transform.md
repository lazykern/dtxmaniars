# Stage Transform (Phase 2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** While the Customize surface is open, offset/scale the whole drums playfield into a sub-rect of the window (osu `ScalingContainer.SetCustomRect` analog) via three presets — **Offset** (settings tabs), **Fit** (kit tabs), **Identity** (peek/closed) — animated, without breaking gameplay geometry, widget drag, or picking.

**Architecture:** Gameplay is 100% Bevy UI-space (single `Camera2d`, `HudRoot`/`WidgetContainer` `Node`s), so a camera or world-root transform moves nothing. Instead we introduce a `StageRect` resource (a window→sub-rect map: `origin` + `size` in physical px) that **replaces every raw `window.width()/height()` read** in the drums render + editor-cursor path. It defaults to the full window (identity), so each consumer is refactored behavior-preservingly first (regression tests prove byte-identical geometry at rect = full window); only then does the editor compute presets and animate the rect. `PlayfieldLayout` finally gains the origin offset that `hud.rs:148` deferred.

**Tech Stack:** Rust, Bevy 0.19. Crate: `gameplay-drums` (all changes are in one crate). Depends on Phase 2a (`ActiveTab`, `EditorOpen`, `EditorChrome`, tab rail) already on branch `feat/customize-surface`.

**Spec:** `docs/superpowers/specs/2026-07-07-customize-surface-design.md` §4.2 (stage transform, three presets) + §4.3 (Fit for kit tabs, Offset for settings).

**Investigation map (file:line anchors, all in `crates/gameplay-drums/src/`):**
- `layout.rs:27-179` `PlayfieldLayout`; `:43-48` `from_window`→`from_size`, `scale = min(w/1280, h/720)`; `:61-63` `ref_strip_left`; `:202-223` `sync_playfield_layout` (rebuild on `WindowResized`).
- `theme.rs` (crate `dtx-ui`) `:7-8` `REF_WIDTH=1280`, `REF_HEIGHT=720`.
- `hud.rs:136-146` `HudRoot` (full-screen `Node`, no transform); `:148-151` TODO deferring the origin offset — this plan.
- `widget_layout.rs:27-70` `WidgetContainer`; `:89-103` `parent_rect_px` (Screen arm = `(0,0,win.x,win.y)`); `:159,203-209` `applied_of` (inverts per-widget transform only); `:212-276` `apply_widget_layout`; `:228-240` `sc = window/2`, `wsize = window`, natural translation `offset * pfl.scale`.
- `editor/picking.rs:84-89` `node_rect`; `:121` `sc = window/2`; `:132-147` widget AABBs; `:167-170` chrome mask; `:190-197` hover.
- `editor/drag.rs:38-46` `apply_drag` (`offset + screen_delta / scale`); `:118-128` press + scale-handle; `:216-219` apply.
- `editor/ui.rs:70-86` left sidebar (`Absolute, left:0, width:220`, `EditorChrome`, `GlobalZIndex(2000)`).
- `editor/panel.rs:88` `PANEL_WIDTH=240`; `:163-179` right panel (`Absolute, right:0, width:PANEL_WIDTH`, `EditorChrome`).
- `orchestrator.rs:88,340` `DrumsStageRoot` (bare marker, not a parent — do NOT use as transform root).

**Critical conventions:**
- NEVER `cargo fmt` / `cargo fmt --all` / `cargo fmt -p`. ONLY `rustfmt --edition 2021 <explicit files you edited>` (all files here are `gameplay-drums`, edition 2021).
- Work from worktree `/home/lazykern/lab/dtxmaniars-customize` (branch `feat/customize-surface`). Run all cargo/git with that cwd. Do NOT touch sibling worktrees.
- Before every `git add`, run `git -C /home/lazykern/lab/dtxmaniars-customize status --short` and confirm ONLY intended files. If unrelated `editor/*.rs` show modified (whitespace drift from another process), do NOT stage them.
- Bevy 0.19: UI nodes use `UiTransform`/`UiGlobalTransform`, NOT `GlobalTransform` (silent no-match trap). State changes flow through `game_shell::request_transition`, not raw `NextState`.
- Green unit tests do NOT prove the plugin schedule builds — the final task runs the full workspace suite incl. the headless schedule-guard test.
- **The refactor is behavior-preserving until Task 5.** Tasks 2-4 MUST keep every existing test green with zero changes to expected values (rect defaults to full window). If an existing test's expected numbers must change in Tasks 2-4, STOP — the refactor is wrong.

---

## Coordinate model (read before any task)

Today every consumer reads `w = window.width()`, `h = window.height()` and:
1. `scale = min(w/1280, h/720)`,
2. places gameplay at `ref_px * scale` (playfield strip horizontally centered on the **1280 ref**, so at `((1280 - strip)/2) * scale` from window-left),
3. widgets/picking use `sc = window/2`, `wsize = window`, screen parent rect `(0,0,w,h)`.

`StageRect { origin: Vec2, size: Vec2 }` (physical px) generalizes this. Every consumer replaces:
- `w,h`  →  `rect.size.x, rect.size.y`
- absolute window-left `0`  →  `rect.origin.x` (and `0` top → `rect.origin.y`)
- `window/2` (as a half-extent for centering)  →  `rect.origin + rect.size/2`
- screen parent rect `(0,0,w,h)`  →  `(rect.origin.x, rect.origin.y, rect.size.x, rect.size.y)`

When `rect = { origin: (0,0), size: window }` this is **identical** to today. The cursor (`window.cursor_position()`) stays raw window-space; because widget AABBs now include `rect.origin`, cursor↔AABB comparisons remain in the same window frame and stay consistent. Chrome (`EditorChrome` nodes + the chrome-mask read at `picking.rs:167`) stays window-space — it is the fixed reference frame.

Presets (Task 5) compute the target rect from the window size, `ActiveTab`, and chrome widths (left 220, right 240):
- **Identity** — `origin=(0,0)`, `size=window`.
- **Offset** (settings tabs) — `scale` preserved (1:1 feel), playfield shifted right into the visible gap: `origin=(LEFT_CHROME, 0)`, `size=window` (right edge may clip under the right panel — acceptable per spec; the subject is the centered playfield).
- **Fit** (kit tabs) — uniform min-fit into the gap between left panel and right panel: `origin=(LEFT_CHROME, TOP_MARGIN)`, `size=(window.x - LEFT_CHROME - RIGHT_CHROME, window.y - 2*TOP_MARGIN)`. The existing `scale = min(size.x/1280, size.y/720)` then shrinks the whole screen to fit, centered on the 1280 ref inside the rect.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/gameplay-drums/src/stage_rect.rs` | Create | `StageRect` + `StageTarget` resources, `full()` ctor, window-sync-to-identity + animation lerp systems, `stage_rect` plugin |
| `crates/gameplay-drums/src/layout.rs` | Modify | `PlayfieldLayout::from_rect(StageRect)` (origin offset); `sync_playfield_layout` reads `StageRect`, rebuilds on its change |
| `crates/gameplay-drums/src/widget_layout.rs` | Modify | `apply_widget_layout` + `parent_rect_px` read `StageRect` instead of raw window |
| `crates/gameplay-drums/src/editor/picking.rs` | Modify | widget AABBs + `sc` include `StageRect` origin/size |
| `crates/gameplay-drums/src/editor/drag.rs` | Modify | `apply_drag` scale + `convert_to_anchored` read `StageRect` |
| `crates/gameplay-drums/src/editor/stage.rs` | Create | preset computation from `ActiveTab`+chrome widths → sets `StageTarget`; hold-Tab peek (Identity + hide `EditorChrome`); editor-gated |
| `crates/gameplay-drums/src/editor/mod.rs` | Modify | register `stage` module + plugin |
| `crates/gameplay-drums/src/lib.rs` (or `orchestrator.rs`) | Modify | register `stage_rect` module + plugin so the resource always exists |

---

### Task 1: `StageRect` + `StageTarget` resources + identity window-sync

**Files:**
- Create: `crates/gameplay-drums/src/stage_rect.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (register module + plugin)

Context: The resource must ALWAYS exist (non-editor gameplay reads it), defaulting to the full window every frame while the surface is closed. Editor code (Task 5) only overrides the target when open. `StageRect` is what layout reads; `StageTarget` is where presets/peek write; a lerp (Task 6) moves `StageRect` toward `StageTarget`. In this task the lerp is a **snap** (we add easing in Task 6) so Task 1 is fully testable in isolation.

- [ ] **Step 1: Write failing tests**

Create `crates/gameplay-drums/src/stage_rect.rs`:

```rust
//! Stage transform: maps the drums playfield into a sub-rect of the window.
//!
//! `StageRect` is the CURRENT rect every drums layout/picking consumer reads
//! instead of the raw window size. `StageTarget` is the desired rect the
//! Customize surface writes (Task 5); a lerp moves `StageRect` toward it
//! (Task 6). When the surface is closed the rect is the full window (identity),
//! so all gameplay geometry is byte-identical to pre-transform behavior.

use bevy::prelude::*;

/// Window sub-rect the drums stage is mapped into (physical px).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StageRect {
    pub origin: Vec2,
    pub size: Vec2,
}

/// Desired stage rect; `StageRect` animates toward this.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StageTarget(pub StageRect);

impl StageRect {
    /// Identity: the whole window.
    pub fn full(window: Vec2) -> Self {
        Self { origin: Vec2::ZERO, size: window }
    }
    /// Center of the rect in window coords (replaces `window/2` half-extent use).
    pub fn center(&self) -> Vec2 {
        self.origin + self.size * 0.5
    }
}

impl Default for StageRect {
    fn default() -> Self {
        // Placeholder until the first `sync_stage_target_to_window`; REF size.
        Self { origin: Vec2::ZERO, size: Vec2::new(1280.0, 720.0) }
    }
}

impl Default for StageTarget {
    fn default() -> Self {
        StageTarget(StageRect::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_is_origin_zero_and_window_size() {
        let r = StageRect::full(Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::ZERO);
        assert_eq!(r.size, Vec2::new(1600.0, 900.0));
    }

    #[test]
    fn center_of_full_is_window_half() {
        let r = StageRect::full(Vec2::new(1600.0, 900.0));
        assert_eq!(r.center(), Vec2::new(800.0, 450.0));
    }

    #[test]
    fn center_of_offset_rect() {
        let r = StageRect { origin: Vec2::new(220.0, 0.0), size: Vec2::new(1000.0, 720.0) };
        assert_eq!(r.center(), Vec2::new(720.0, 360.0));
    }
}
```

- [ ] **Step 2: Run to verify fail**

Add `pub mod stage_rect;` to `crates/gameplay-drums/src/lib.rs` (near the other `pub mod` lines). Run: `cargo test -p gameplay-drums stage_rect`. Expected: FAIL (module/systems not yet wired) then PASS on the three unit tests once it compiles.

- [ ] **Step 3: Add the sync + (snap) animation systems**

Append to `stage_rect.rs`:

```rust
use bevy::window::PrimaryWindow;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<StageRect>()
        .init_resource::<StageTarget>()
        .add_systems(
            Update,
            (
                sync_stage_target_to_window.run_if(surface_closed),
                animate_stage_rect,
            )
                .chain(),
        );
}

/// True when the Customize surface is NOT open (identity should hold).
fn surface_closed(open: Option<Res<crate::editor::EditorOpen>>) -> bool {
    open.map(|o| !o.0).unwrap_or(true)
}

/// While closed, the target is always the full window.
fn sync_stage_target_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut target: ResMut<StageTarget>,
) {
    let Ok(win) = windows.single() else { return };
    let full = StageRect::full(Vec2::new(win.width(), win.height()));
    if target.0 != full {
        target.0 = full;
    }
}

/// Move `StageRect` toward `StageTarget`. Task 1: snap (easing added in Task 6).
fn animate_stage_rect(target: Res<StageTarget>, mut rect: ResMut<StageRect>) {
    if *rect != target.0 {
        *rect = target.0;
    }
}
```

Note: `EditorOpen` is `pub` in `editor/mod.rs` (confirmed Phase 2a). If `crate::editor::EditorOpen` is not the correct path, find it with `rg -n "struct EditorOpen" crates/gameplay-drums/src`. Use `Option<Res<..>>` so the system is robust before the editor plugin adds the resource (it is added by the editor plugin; ordering-independent).

- [ ] **Step 4: Register the plugin**

In `crates/gameplay-drums/src/lib.rs`, add `stage_rect::plugin` to the crate's `add_plugins((...))` tuple (find with `rg -n "add_plugins" crates/gameplay-drums/src/lib.rs`). It must be registered unconditionally (not editor-gated) so the resource always exists.

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums`. Expected: PASS — all existing tests unchanged (nothing reads `StageRect` yet) + the 3 new unit tests + schedule guard still green.

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/stage_rect.rs crates/gameplay-drums/src/lib.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/stage_rect.rs crates/gameplay-drums/src/lib.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): StageRect + StageTarget resources (identity)"
```

---

### Task 2: `PlayfieldLayout` reads `StageRect` (origin offset)

**Files:**
- Modify: `crates/gameplay-drums/src/layout.rs`

Context: `PlayfieldLayout::from_size(w, h, lanes)` (`layout.rs:43-48`) computes `scale = min(w/1280, h/720)` and lays out the strip centered on the 1280 ref at absolute window-left `0`. Add `from_rect(rect, lanes)` that (a) derives `scale` from `rect.size`, (b) offsets every absolute x by `rect.origin.x` and y by `rect.origin.y`. `sync_playfield_layout` (`layout.rs:202-223`) switches to read `StageRect` and rebuild when it changes. **Behavior-preserving:** at `rect = full window`, `from_rect` MUST equal today's `from_size`.

READ `layout.rs` fully first — note every field that is an absolute window-space pixel (e.g. `ref_strip_left`-derived left edges, `col_left`, `backboard_left`, judge-line Y). Each absolute-position field gains `+ rect.origin.{x,y}`; each size/scale field derives from `rect.size`.

- [ ] **Step 1: Write failing regression + offset tests**

Add to the `#[cfg(test)]` module in `layout.rs` (adapt field/method names to the real struct — inspect what `from_size` returns and pick a stable observable like the leftmost column x or judge-line y):

```rust
#[test]
fn from_rect_full_window_equals_from_size() {
    let lanes = /* build the same default lanes `from_size` tests use; copy from an existing test */;
    let win = bevy::math::Vec2::new(1600.0, 900.0);
    let from_size = PlayfieldLayout::from_size(win.x, win.y, &lanes);
    let from_rect = PlayfieldLayout::from_rect(
        crate::stage_rect::StageRect::full(win),
        &lanes,
    );
    assert_eq!(from_rect, from_size, "identity rect must reproduce from_size exactly");
}

#[test]
fn from_rect_offset_shifts_all_x_by_origin() {
    let lanes = /* same default lanes */;
    let win = bevy::math::Vec2::new(1600.0, 900.0);
    let base = PlayfieldLayout::from_rect(crate::stage_rect::StageRect::full(win), &lanes);
    let shifted = PlayfieldLayout::from_rect(
        crate::stage_rect::StageRect { origin: bevy::math::Vec2::new(220.0, 0.0), size: win },
        &lanes,
    );
    // Same scale (size unchanged), every absolute x shifted right by 220.
    assert_eq!(shifted.scale, base.scale);
    // Pick a real absolute-x accessor on PlayfieldLayout (e.g. col_left(0) or backboard_left):
    assert!((shifted.SOME_ABS_X - (base.SOME_ABS_X + 220.0)).abs() < 0.01);
}
```

Requires `PlayfieldLayout: PartialEq` — add `#[derive(PartialEq)]` if absent (it already derives `Debug, Clone` per the investigation; add `PartialEq`). If it holds `f32`s, `PartialEq` is fine for exact-equality of deterministic arithmetic; if any field is non-`PartialEq`, compare the specific observable fields instead of the whole struct.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p gameplay-drums from_rect`. Expected: FAIL — `from_rect` not defined.

- [ ] **Step 3: Implement `from_rect`, express `from_size` via it**

Refactor so `from_size` delegates to `from_rect` (DRY, and guarantees identity equivalence):

```rust
pub fn from_size(width: f32, height: f32, lanes: &Lanes) -> Self {
    Self::from_rect(
        crate::stage_rect::StageRect::full(bevy::math::Vec2::new(width, height)),
        lanes,
    )
}

pub fn from_rect(rect: crate::stage_rect::StageRect, lanes: &Lanes) -> Self {
    let scale = (rect.size.x / REF_WIDTH).min(rect.size.y / REF_HEIGHT);
    // ... existing body, but:
    //   - replace every `scale = (width/REF_WIDTH).min(height/REF_HEIGHT)` with the line above
    //   - add `rect.origin.x` to every absolute window-left x
    //   - add `rect.origin.y` to every absolute window-top y
    // Keep `ref_strip_left()` etc. as REF-space; apply origin when converting to px.
}
```

Match the exact field names/types from the real struct. The `Lanes` type name may differ (`lanes: &crate::resources::Lanes` or similar) — use whatever `from_size` currently takes.

- [ ] **Step 4: Reroute `sync_playfield_layout` to `StageRect`**

Change `sync_playfield_layout` (`layout.rs:202-223`) to take `rect: Res<crate::stage_rect::StageRect>` and build `PlayfieldLayout::from_rect(*rect, &lanes)`. Change its run condition / trigger so it rebuilds when `StageRect` changes OR the window resizes OR lanes change (today it's `WindowResized` + lanes). Add `.run_if(resource_changed::<crate::stage_rect::StageRect>)` OR fold `StageRect` change into the existing trigger — but ALSO keep rebuilding on lane changes. Simplest robust form: run every frame but early-return unless `rect.is_changed() || lanes.is_changed() || <window actually resized>`; or rebuild whenever `StageRect`/`Lanes` changed (since `StageRect` itself already tracks the window via Task 1's sync, `WindowResized` is now subsumed — but keep a window-size guard if the current code reads window directly). Verify the judge line / notes still track by keeping the rebuild responsive to `StageRect`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums`. Expected: PASS — the 2 new tests + **every existing layout test unchanged** (identity equivalence). If any existing test's expected number must change, STOP: `from_rect` is not behavior-preserving at identity — fix the origin/scale wiring.

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/layout.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/layout.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): PlayfieldLayout::from_rect with stage origin offset"
```

---

### Task 3: `widget_layout` parent rect reads `StageRect`

**Files:**
- Modify: `crates/gameplay-drums/src/widget_layout.rs`
- Modify: `crates/gameplay-drums/src/editor/drag.rs` (parent_rect_px call site @180-188)
- Modify: `crates/gameplay-drums/src/editor/snap.rs` (call sites @101-106, 153-154)
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (call sites @699-720, 773-781)

Context: `apply_widget_layout` (`widget_layout.rs:212-276`) uses `sc = window/2` (`:228`), `wsize = window` (`:229`), natural translation `offset * pfl.scale` (`:236-240`); `parent_rect_px(space, window_size, pfl)` Screen arm returns `(0,0,win.x,win.y)` (`:89-103`). All must read `StageRect`. **Behavior-preserving at rect = full window.**

**Cross-file coupling (why this task spans 4 files):** `parent_rect_px` is called from 5 sites — `widget_layout.rs:248`, `drag.rs:188`, `panel.rs:720/781`, `snap.rs:106/154` — each currently computing `wsize = Vec2::new(window.width(), window.height())` and passing it. Only its **Screen arm** needs the stage origin (the **Playfield arm** already uses `pfl.strip_left()`/`lane_top()`, which Task 2 made origin-aware). So: change the signature `parent_rect_px(space, rect: crate::stage_rect::StageRect, pfl)`, Screen arm → `(rect.origin.x, rect.origin.y, rect.size.x, rect.size.y)`, and update all 5 callers to pass a `StageRect` instead of `wsize`. Each caller is (or is called from) a Bevy system — add `rect: Res<crate::stage_rect::StageRect>` and pass `*rect`; where the caller ALSO uses `wsize` for other local math, map that to `rect.size` and (if it needs a screen-space base) `rect.origin`. At identity all 5 are unchanged. Do NOT change `apply_widget_layout`'s other math beyond the `sc`/`wsize`/parent-rect reads. Do NOT touch `applied_of` (`:159,203-209`) — per-widget transform inversion stays.

READ `widget_layout.rs`, `drag.rs`, `snap.rs`, `panel.rs` fully first. For each of the 5 call sites, confirm whether `wsize` is used ONLY for the `parent_rect_px` call (then just swap to `*rect`) or ALSO elsewhere (then thread `rect.size`/`rect.origin` through). Keep the change minimal and behavior-preserving at identity.

- [ ] **Step 1: Write failing regression test**

Widget placement is math over window size + `pfl.scale`. Add a test that a screen-anchored widget's computed px position is unchanged when `StageRect` is the full window, and shifts by `origin` when the rect shifts. If `parent_rect_px` / the placement math is a free function, test it directly; if it's inline in the system, extract a small pure helper `fn screen_parent_rect(rect: StageRect) -> (f32,f32,f32,f32)` and test that:

```rust
#[test]
fn screen_parent_rect_full_window_is_zero_origin() {
    let rect = crate::stage_rect::StageRect::full(bevy::math::Vec2::new(1600.0, 900.0));
    assert_eq!(screen_parent_rect(rect), (0.0, 0.0, 1600.0, 900.0));
}

#[test]
fn screen_parent_rect_offset_uses_origin() {
    let rect = crate::stage_rect::StageRect { origin: bevy::math::Vec2::new(220.0, 10.0), size: bevy::math::Vec2::new(1000.0, 700.0) };
    assert_eq!(screen_parent_rect(rect), (220.0, 10.0, 1000.0, 700.0));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p gameplay-drums screen_parent_rect`. Expected: FAIL — helper not defined.

- [ ] **Step 3: Implement**

Add `rect: Res<crate::stage_rect::StageRect>` to `apply_widget_layout`'s params. Replace:
- `sc = window/2`  →  `sc = rect.center()` (the half-extent center used for NDC-ish placement),
- `wsize = window`  →  `wsize = rect.size`,
- `parent_rect_px` Screen arm `(0,0,win.x,win.y)`  →  `(rect.origin.x, rect.origin.y, rect.size.x, rect.size.y)` (via the new `screen_parent_rect` helper),
- natural translation base: wherever the code adds a window-space origin (if it uses `sc` as center, `rect.center()` already carries origin; verify no separate `0`-origin assumption remains).

Keep `offset * pfl.scale` — `pfl.scale` now already derives from `rect.size` (Task 2), so it is consistent. Do NOT touch `applied_of`.

CAREFUL: if `sc` is used purely as a half-size (extent) rather than a center, split the two uses — extent = `rect.size/2`, center = `rect.center()`. Read the surrounding math and map each `window/2` occurrence to whichever it means. This is the subtlest task; when unsure, add an assert-style regression test for a concrete widget position at identity and confirm it's unchanged.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`. Expected: PASS — new helper tests + all existing widget-layout tests unchanged. If existing widget tests break at identity, the `sc` center/extent split is wrong — fix it.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/widget_layout.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/widget_layout.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): widget layout reads StageRect"
```

---

### Task 4: editor `transform_point` screen-center reads `StageRect`

**Files:**
- Modify: `crates/gameplay-drums/src/editor/picking.rs`
- Modify: `crates/gameplay-drums/src/editor/drag.rs`
- Modify: `crates/gameplay-drums/src/editor/snap.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

Context: `transform_point(p, screen_center, t, s)` (widget_layout.rs:62) takes `screen_center = sc = wsize/2` (window center), used to place widget geometry in screen px for AABBs/snap/drag. Under the stage transform this center becomes `rect.center()` (= `window/2` at identity, so **behavior-preserving**). The `sc = wsize/2` occurs at: `picking.rs:121`, `drag.rs:185`, `snap.rs:105`, `panel.rs:714`, `panel.rs:776`. Task 3 already added `rect: Res<StageRect>` to the drag/snap/panel systems, so those are one-line swaps `let sc = rect.center();`. `picking.rs` needs `rect: Res<crate::stage_rect::StageRect>` added to its hover/AABB system(s), then `sc = rect.center()`.

The cursor stays raw `window.cursor_position()` (picking hover + drag press) — because widget AABBs now use `sc = rect.center()` (origin-inclusive), cursor↔AABB comparisons stay consistent in the same window frame. Chrome mask (`picking.rs:167-170`) + `node_rect` (`:84-89`) stay window-space (chrome is the fixed frame — do NOT change). `apply_drag`'s delta→offset (`drag.rs:38-46`, `screen_delta / scale`) STAYS — `scale` is `pfl.scale`, already rect-derived (Task 2). Leave `snap.rs:186-187` `/1280.0`,`/720.0` (REF-space, unrelated). **Behavior-preserving at identity.**

READ all 4 files fully. Change ONLY the `sc = wsize/2` / `sc = window/2` screen-center lines to `rect.center()` (+ add `Res<StageRect>` to picking's system). Do not touch chrome-mask, raw cursor reads, or the REF-space `/1280`,`/720`.

- [ ] **Step 1: Write failing regression test**

If the AABB build or `convert_to_anchored` has a pure core, extract + test it. Otherwise test `apply_drag`'s delta→offset conversion (already pure at `drag.rs:38-46`) is unchanged at identity `pfl.scale`, and add a picking helper test. Minimum:

```rust
// in picking.rs tests — the widget-AABB center helper at identity equals today.
#[test]
fn widget_aabb_center_full_window_matches_window_half() {
    let rect = crate::stage_rect::StageRect::full(bevy::math::Vec2::new(1600.0, 900.0));
    // whatever helper computes `sc`:
    assert_eq!(stage_half_extent(rect), bevy::math::Vec2::new(800.0, 450.0));
}
```

(Adapt to the real helper you extract; the point is a regression guard that identity == old behavior.)

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p gameplay-drums widget_aabb`. Expected: FAIL — helper not defined.

- [ ] **Step 3: Implement**

- `picking.rs`: add `rect: Res<crate::stage_rect::StageRect>` to the systems that build widget AABBs / hover-test. Replace `sc = window/2` (`:121`) with the stage center/extent (mirror Task 3's center-vs-extent split). Ensure widget AABBs are emitted in the SAME window frame as the raw cursor (add `rect.origin` where the old code implicitly used `0`). Leave `node_rect` (`:84-89`) and the chrome mask (`:167-170`) window-space — chrome does not move.
- `drag.rs`: add `rect: Res<crate::stage_rect::StageRect>` where needed. `apply_drag`'s `screen_delta / scale` stays (scale from `pfl` already rect-derived). `convert_to_anchored` (`:...`) window-size reads → `rect.size` / `rect.origin`. Scale-handle hit-test (`:126-128`) uses the same widget frame — reroute its window math to `rect`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`. Expected: PASS — regression guards + all existing editor tests unchanged.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/picking.rs crates/gameplay-drums/src/editor/drag.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/picking.rs crates/gameplay-drums/src/editor/drag.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): editor picking + drag read StageRect"
```

---

### Task 5: Preset computation (Identity / Offset / Fit)

**Files:**
- Create: `crates/gameplay-drums/src/editor/stage.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (register module + plugin)

Context: With Tasks 1-4 done, setting `StageTarget` to a non-identity rect will (via the Task 1 snap / Task 6 lerp) move the whole playfield + widgets coherently. This task computes the target rect from `ActiveTab` + chrome widths and writes `StageTarget` while the surface is open. Presets per the coordinate model above.

- [ ] **Step 1: Write failing pure-fn tests**

Create `crates/gameplay-drums/src/editor/stage.rs` with a PURE preset fn + tests:

```rust
//! Customize stage-transform presets: map ActiveTab → target StageRect.

use bevy::prelude::*;
use game_shell::CustomizeTab;
use crate::stage_rect::{StageRect, StageTarget};

/// Left sidebar width (editor/ui.rs) and right panel width (editor/panel.rs).
const LEFT_CHROME: f32 = 220.0;
const RIGHT_CHROME: f32 = 240.0;
const TOP_MARGIN: f32 = 24.0;

/// Preset rect for a tab given the window size.
pub fn preset_rect(tab: CustomizeTab, window: Vec2) -> StageRect {
    if tab.is_settings() {
        // Offset: true scale, playfield shifted into the gap right of the rail.
        StageRect { origin: Vec2::new(LEFT_CHROME, 0.0), size: window }
    } else {
        // Fit: shrink whole screen into the gap between both chrome panels.
        StageRect {
            origin: Vec2::new(LEFT_CHROME, TOP_MARGIN),
            size: Vec2::new(
                (window.x - LEFT_CHROME - RIGHT_CHROME).max(1.0),
                (window.y - 2.0 * TOP_MARGIN).max(1.0),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tab_is_offset_true_scale() {
        let r = preset_rect(CustomizeTab::Gameplay, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(220.0, 0.0));
        assert_eq!(r.size, Vec2::new(1600.0, 900.0)); // scale 1 preserved
    }

    #[test]
    fn kit_tab_is_fit_between_chrome() {
        let r = preset_rect(CustomizeTab::Widgets, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(220.0, 24.0));
        assert_eq!(r.size, Vec2::new(1600.0 - 220.0 - 240.0, 900.0 - 48.0));
    }
}
```

- [ ] **Step 2: Register + run to verify fail**

Add `pub mod stage;` to `crates/gameplay-drums/src/editor/mod.rs` and `stage::plugin,` to the editor submodule plugin tuple. Run: `cargo test -p gameplay-drums editor::stage`. Expected: FAIL then PASS on the 2 unit tests once compiled (plugin added in Step 3).

- [ ] **Step 3: Add the target-writing system**

Append to `stage.rs`:

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        set_stage_target.run_if(super::editor_open),
    );
}

/// While the surface is open, drive the target rect from the active tab.
fn set_stage_target(
    active: Res<super::tabs::ActiveTab>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut target: ResMut<StageTarget>,
) {
    let Ok(win) = windows.single() else { return };
    let want = preset_rect(active.0, Vec2::new(win.width(), win.height()));
    if target.0 != want {
        target.0 = want;
    }
}
```

Note: Task 1's `sync_stage_target_to_window` runs `run_if(surface_closed)` and this runs `run_if(editor_open)` — mutually exclusive, no fight over `StageTarget`. On close, Task 1 snaps target back to full window.

- [ ] **Step 4: Run tests + manual reasoning**

Run: `cargo test -p gameplay-drums`. Expected: PASS. At this point (snap animation from Task 1) opening the surface should JUMP the playfield into the preset rect. Full smoothness comes in Task 6.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/stage.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/stage.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): Customize stage presets (Offset/Fit) set StageTarget"
```

---

### Task 6: Animate `StageRect` toward `StageTarget` (ease-out ~450ms)

**Files:**
- Modify: `crates/gameplay-drums/src/stage_rect.rs`

Context: Task 1's `animate_stage_rect` snaps. Replace with a time-based ease-out lerp (~450ms, matching osu's ~450ms transition) so tab switches and open/close glide. Snap immediately when the delta is tiny (avoid asymptotic crawl).

- [ ] **Step 1: Write failing test for the lerp core**

Extract a pure fn and test it:

```rust
/// Exponential ease-out step toward `target` over ~`tau` seconds.
/// `dt` = frame seconds. Returns the new rect.
pub fn ease_rect(current: StageRect, target: StageRect, dt: f32) -> StageRect {
    // Frame-rate independent smoothing: alpha = 1 - exp(-dt / TAU)
    const TAU: f32 = 0.12; // ~ reaches target in ~450ms visually
    let a = 1.0 - (-dt / TAU).exp();
    let lerp = |c: Vec2, t: Vec2| c + (t - c) * a;
    let next = StageRect { origin: lerp(current.origin, target.origin), size: lerp(current.size, target.size) };
    // Snap when close to kill the long tail.
    let close = (next.origin - target.origin).length() < 0.5 && (next.size - target.size).length() < 0.5;
    if close { target } else { next }
}

#[test]
fn ease_moves_toward_target_and_snaps_when_close() {
    let c = StageRect::full(Vec2::new(1000.0, 1000.0));
    let t = StageRect { origin: Vec2::new(220.0, 0.0), size: Vec2::new(1000.0, 1000.0) };
    let mid = ease_rect(c, t, 1.0 / 60.0);
    assert!(mid.origin.x > 0.0 && mid.origin.x < 220.0, "moved partway");
    // A big dt (or many steps) snaps exactly.
    let done = ease_rect(t, t, 1.0 / 60.0);
    assert_eq!(done, t);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p gameplay-drums ease_rect`. Expected: FAIL — not defined.

- [ ] **Step 3: Replace the snap system with the eased one**

```rust
fn animate_stage_rect(
    time: Res<Time>,
    target: Res<StageTarget>,
    mut rect: ResMut<StageRect>,
) {
    if *rect == target.0 {
        return;
    }
    let next = ease_rect(*rect, target.0, time.delta_secs());
    *rect = next;
}
```

`time.delta_secs()` is the Bevy 0.19 accessor (confirm; if it's `delta_seconds()` in this version, use that — `rg -n "delta_secs|delta_seconds" crates` to see which the codebase already uses).

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`. Expected: PASS. Note: `PlayfieldLayout` now rebuilds every frame during the animation (because `StageRect` changes each frame) — that is fine (it already rebuilds on resize); confirm no per-frame allocation surprises by eye, but do not prematurely optimize.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/stage_rect.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/stage_rect.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): ease-out animation for StageRect"
```

---

### Task 7: Hold-Tab peek (Identity + hide chrome)

**Files:**
- Create/modify: `crates/gameplay-drums/src/editor/stage.rs` (peek system)

Context spec §4.2: while the surface is open, holding `Tab` drops to Identity (full-window, no transform) AND hides all `EditorChrome` nodes, giving the exact play view; releasing restores the preset + chrome. Peek overrides the preset target.

- [ ] **Step 1: Add the peek system**

In `stage.rs`, extend `set_stage_target` (or add a higher-priority `peek_stage` system ordered AFTER it) so that when `Tab` is held it forces `StageTarget` to full window and hides chrome:

```rust
fn peek_stage(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    active: Res<super::tabs::ActiveTab>,
    mut target: ResMut<StageTarget>,
    mut chrome: Query<&mut Visibility, With<super::EditorChrome>>,
) {
    let Ok(win) = windows.single() else { return };
    let peeking = keys.pressed(KeyCode::Tab);
    if peeking {
        target.0 = StageRect::full(Vec2::new(win.width(), win.height()));
    } else {
        target.0 = preset_rect(active.0, Vec2::new(win.width(), win.height()));
    }
    let vis = if peeking { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut chrome {
        if *v != vis { *v = vis; }
    }
}
```

Then REMOVE the now-redundant `set_stage_target` (peek_stage supersedes it — it sets the preset when not peeking). Register `peek_stage` in the plugin under `run_if(super::editor_open)`. Confirm `EditorChrome` is `pub` in `editor/mod.rs` (or the module where it's defined — `rg -n "struct EditorChrome" crates/gameplay-drums/src`); widen visibility if needed. Confirm `Tab` is not already consumed by another editor system in a conflicting way (`rg -n "KeyCode::Tab" crates/gameplay-drums/src`); if it is, coordinate (peek should win while held).

- [ ] **Step 2: Test the pure part**

The preset-vs-full choice is already covered by Task 5's `preset_rect` tests; the peek toggle is glue (keyboard + Visibility) verified by the schedule guard + manual smoke. Add no fragile UI test. Run: `cargo test -p gameplay-drums`. Expected: PASS (schedule guard proves `peek_stage` wires in).

- [ ] **Step 3: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): hold-Tab peek (identity + hide chrome)"
```

---

### Task 8: Fit-preset screen-bounds outline

**Files:**
- Modify: `crates/gameplay-drums/src/editor/stage.rs` (or a small `editor/stage_mask.rs`)

Context spec §4.2 Fit: "masked with visible screen-bounds outline" so the user sees the true screen edges while the whole game is shrunk — essential for judging edge-anchored widget placement (WYSIWYG anchors). Implement as a thin bordered UI `Node` positioned at the current `StageRect` (origin+size), shown only on kit tabs (Fit), hidden on settings/peek.

- [ ] **Step 1: Spawn + track an outline node**

Add a system that maintains one `StageOutline`-tagged `Node` (absolute, `left/top = rect.origin`, `width/height = rect.size`, transparent fill, ~2px border in the theme's dim color, `GlobalZIndex` just below chrome). Update its `Node` left/top/width/height each frame from `StageRect`. Show it only when `active.0` is a KIT tab and not peeking; else `Visibility::Hidden`.

```rust
#[derive(Component)]
struct StageOutline;
```

Spawn it lazily (on first surface open) or in the editor UI spawn; update via a system gated `editor_open`. Match the existing outline/border styling used elsewhere in the editor (e.g. selection box in `selection_box.rs`) so it reads as native. Because it is chrome-like (window-space), position it directly from `StageRect` (no self-transform).

- [ ] **Step 2: Build + test**

Run: `cargo test -p gameplay-drums`. Expected: PASS (schedule guard). No unit test for pure-visual node.

- [ ] **Step 3: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): Fit-preset screen-bounds outline"
```

---

### Task 9: Full verification + manual smoke

- [ ] **Step 1: Full workspace tests**

Run: `cargo test --workspace`. Expected: PASS incl. the headless schedule-guard tests (prove the new `stage_rect`/`stage` systems build into the schedule). Every pre-existing test must be unchanged — the refactor was behavior-preserving at identity.

- [ ] **Step 2: Clippy**

Run: `cargo clippy -p gameplay-drums --all-targets 2>&1 | rg -n "stage_rect|editor/stage|layout.rs|widget_layout|picking|drag"`. Expected: no NEW warnings in the touched files.

- [ ] **Step 3: Manual smoke (human)**

Launch `cargo run -p dtxmaniars-desktop`. Verify:
1. **Closed** = identity: normal gameplay, playfield centered, judge line aligned with notes, hits register — no visual change from before this phase.
2. **F1 (settings)** → playfield glides right into the gap beside the rail (Offset), true 1:1 scale. Adjusting settings still works.
3. **F2 / Lanes / Widgets (kit)** → whole screen shrinks (Fit) into the gap between panels, screen-bounds outline visible; **dragging a widget still tracks the cursor exactly** and **picking selects the right widget** (the critical regression check — cursor math under transform).
4. **Hold Tab** (peek) → chrome hides, playfield snaps to full identity view; release → restores preset + chrome.
5. Switch settings↔kit tabs → smooth animated re-fit.
6. **Esc** closes → playfield eases back to identity; gameplay geometry exactly as step 1.

- [ ] **Step 4: Final fixups commit (if any)**

```bash
git -C /home/lazykern/lab/dtxmaniars-customize add -A
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "test: stage transform fixups"
```

---

## Self-review notes

- **Spec §4.2 coverage:** three presets → Task 5 (Offset/Fit) + Task 7 (Identity/peek); animated ~450ms → Task 6; masked screen-bounds outline → Task 8. §4.3 Offset-for-settings / Fit-for-kit → `preset_rect` `is_settings()` branch (Task 5).
- **The load-bearing risk (completeness)** is addressed structurally: `from_size` is re-expressed via `from_rect` (Task 2) so identity equivalence is guaranteed by construction, and every consumer (layout, widget_layout, picking, drag) gets an identity-regression test that MUST stay green through Tasks 2-4. The transform only "turns on" in Task 5. If a raw-window read is missed, its consumer won't follow the stage — caught by the Task 9 manual smoke (drag/pick under Fit).
- **Type consistency:** `StageRect { origin, size }` + `StageTarget(StageRect)` (Task 1) used verbatim in Tasks 2-8; `preset_rect` (Task 5) reused by Task 7; `ease_rect` (Task 6). `StageRect::center()` used in Tasks 3-4 for the `window/2`→center replacement.
- **Cursor stays window-space; chrome stays window-space** — only widget/playfield geometry moves. This keeps `cursor_position()` and the chrome mask (`picking.rs:167`) as the stable frame; AABBs gain `origin` so they stay in that frame.
- **Deferred / out of scope:** guitar stage (drums only, per spec); per-frame layout rebuild during animation is accepted (not optimized); the outline is a plain bordered node, not a true render mask.
- **Green-per-commit:** Tasks 1-4 compile and keep all tests green with the transform inert; Task 5 activates it; 6-8 refine. Each task commits a passing state.
