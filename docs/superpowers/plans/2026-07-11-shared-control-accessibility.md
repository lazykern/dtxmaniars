# Shared Control Accessibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix accessibility at the shared-constructor level (keyboard focus, visible focus state, semantic name/value, larger targets on slider/stepper/toggle), plus high-contrast theme, reduced flashes with background dimming, scalable HUD text, and No Fail.

**Architecture:** The shared widgets (`dtx-ui/src/widget/controls.rs`) are mouse-only with 14-16 px targets, used solely by the drums editor panel — updating the constructors fixes every call site at once (roadmap's explicit instruction). Focus reuses the proven `Outline`-ring pattern from `editor/keyboard_nav.rs`. Theme work rides the single `ThemeResource`: a high-contrast variant plus a text-scale factor inside `Theme::font()` so no call site changes. Flash reduction gates the three flash drivers and finally consumes the long-dead `bg_alpha` config. No Fail gates the single failure trigger and keeps modified plays out of normal score records (consistent with the play-speed plan's `persistence_allowed`).

**Tech Stack:** existing crates only.

**Source basis (verified 2026-07-11):**
- Controls: `crates/dtx-ui/src/widget/controls.rs` (349 lines). `spawn_slider` (:79-107, track 110×**14 px**), `spawn_stepper` (:110-167, 12 px label, 6×1 px button padding), `spawn_toggle` (:170-193, **30×16 px**, 12 px knob, ON color hardcoded `srgb(0,0.831,0.667)` at :321-325 bypassing theme). Systems mouse-only (`drive_sliders` :216-254, `drive_steppers` :256-270, `drive_toggles` :272-280). Pure helpers `slider_value_at`/`stepper_next` tested :335-348. `ControlsPlugin` registered dtx-ui lib.rs:104.
- Call sites (ALL in `crates/gameplay-drums/src/editor/panel.rs`): steppers :447/:461/:487, sliders :475/:696/:830, toggles :501/:505; each gets a `PanelField` marker; change detection :1012-1013.
- Focus precedent: `editor/keyboard_nav.rs` — `FocusedRow`, `NavLevel`, `update_focus_rings` :76-94 (`Outline { width: 3px, color: FOCUS_RING }`), pure-mirror test style :285-536. Nav bus: `game_shell::nav::NavVerb/NavAction` (nav.rs:10-46).
- Theme: `crates/dtx-ui/src/theme.rs` — `Theme` struct :15-35, defaults :37-61, `ThemeResource` :65, `font(size)` :95-101 (fixed px), `lane_colors()` :131-143. No variant, no scale.
- Flashes: `lane_flush.rs` (`tick_lane_flushes` :76-96), `keyboard_viz.rs` `flash_key_caps_on_hit` :116, `hit_feedback.rs`. BGA dim hook: `SystemConfig::{bg_alpha, movie_alpha}` (`dtx-config/src/lib.rs:64-92`) — parsed, persisted, ZERO consumers.
- No Fail: single failure trigger `detect_stage_failure` (`stage_end.rs:64-86`, request at :80; already skips practice :71 and editor :74). `StageGauge` (`gameplay-drums/src/gauge.rs:59-67`); `DamageLevel::None` already zeroes miss drain (:50-57). `ScoreEntry` has no modifier field; persist gate at `game-results/src/lib.rs:306-309` (play-speed plan generalizes it to `persistence_allowed`).
- Settings rows: `settings_data.rs` tables; live-apply `editor/tabs.rs:69-129`.

**Ordering note:** Task 6 (No Fail) touches the same persist gate as the play-speed-contract plan. If that plan hasn't landed, implement `persistence_allowed` here first with the same shape; if it has, extend it.

---

### Task 1: Bigger targets + semantic name/value on shared constructors

**Files:**
- Modify: `crates/dtx-ui/src/widget/controls.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (all 8 call sites)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn control_sizes_meet_minimum_targets() {
    // Xbox Accessibility Guideline 103-adjacent: interactive targets get
    // meaningfully bigger than the old 14-16 px strips.
    assert!(SLIDER_HEIGHT >= 22.0);
    assert!(TOGGLE_WIDTH >= 44.0 && TOGGLE_HEIGHT >= 24.0);
    assert!(STEPPER_BTN_MIN_WIDTH >= 24.0 && STEPPER_BTN_MIN_HEIGHT >= 24.0);
}

#[test]
fn control_name_carries_semantics() {
    let n = ControlName::new("Master Volume");
    assert_eq!(n.0, "Master Volume");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-ui -j 2 control_`
Expected: FAIL — constants/`ControlName` missing.

- [ ] **Step 3: Implement**

- Promote sizes to named constants and enlarge: `SLIDER_HEIGHT = 24.0` (track can render a thinner inner bar — hit area is the Node), `TOGGLE_WIDTH = 44.0`, `TOGGLE_HEIGHT = 24.0`, knob 18 px, `STEPPER_BTN_MIN_WIDTH/HEIGHT = 24.0` (via `min_width`/`min_height` on the button Node + padding `UiRect::axes(Val::Px(8.), Val::Px(4.))`), stepper label font 12 → 14.
- Semantic name/value:

```rust
/// Machine-readable control name — the semantic identity of the widget
/// (what a screen-reader/BRP inspector would announce with the value).
#[derive(Component, Debug, Clone)]
pub struct ControlName(pub String);
impl ControlName {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
}
```

- Constructors gain the name: `spawn_slider(p, theme, name: ControlName, spec, value)`, same for stepper/toggle; insert `ControlName` on the interactive entity. Value is already machine-readable (`ControlValue`/`ControlBool`).
- Toggle ON color: replace the hardcoded `srgb(0.0,0.831,0.667)` (:321-325) with `theme.accent` — requires the paint system to read `Res<ThemeResource>`; while there, give the OFF state a shape cue (knob left + track outline) so on/off isn't color-only.

Update all 8 call sites in `panel.rs` with real names (`ControlName::new("Offset X")`, `"Offset Y"`, `"Z Order"`, `"Widget Scale"`, `"Visible in Play"`, `"Visible in Practice"`, plus the two settings-draft sliders — use their row labels).

- [ ] **Step 4: Run tests + build + visual sanity**

Run: `cargo test -p dtx-ui -j 2 && cargo check -p gameplay-drums -j 2`
Expected: PASS / clean. Manual (bevy-brp): editor panel renders without overflow — the panel rows must accommodate the taller widgets; adjust the row heights in `panel.rs` if clipped (screenshot before/after).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui crates/gameplay-drums
git commit -m "feat(a11y): larger control targets, semantic names, themed toggle"
```

---

### Task 2: Keyboard focus + visible focus ring on shared controls

**Files:**
- Modify: `crates/dtx-ui/src/widget/controls.rs`

- [ ] **Step 1: Pure focus model, test-first (keyboard_nav.rs mirror style)**

```rust
/// Focus order = spawn order. Tab/Shift+Tab moves; arrows adjust the focused
/// control; Space/Enter toggles.
#[derive(Resource, Default, Debug)]
pub struct ControlFocus(pub Option<usize>); // index into ordered focusable list

pub fn next_focus(current: Option<usize>, count: usize, backwards: bool) -> Option<usize> {
    if count == 0 { return None; }
    Some(match (current, backwards) {
        (None, false) => 0,
        (None, true) => count - 1,
        (Some(i), false) => (i + 1) % count,
        (Some(i), true) => (i + count - 1) % count,
    })
}

#[cfg(test)]
mod focus_tests {
    use super::*;
    #[test]
    fn tab_cycles_and_wraps() {
        assert_eq!(next_focus(None, 3, false), Some(0));
        assert_eq!(next_focus(Some(2), 3, false), Some(0));
        assert_eq!(next_focus(Some(0), 3, true), Some(2));
        assert_eq!(next_focus(None, 0, false), None);
    }
}
```

- [ ] **Step 2: Systems**

Add to `ControlsPlugin`:

- `Focusable` marker component inserted by all three constructors.
- `focus_hotkeys` system: Tab / Shift+Tab move `ControlFocus` over the ordered `Query<Entity, With<Focusable>>` (order by `Entity` iteration is unstable — attach `FocusOrder(u32)` counter component at spawn via a `Local<u32>`-style counter in a resource, or sort by a `FocusOrder` inserted from an atomic counter; simplest: `#[derive(Resource, Default)] struct FocusCounter(u32)` incremented in each constructor via a deferred insert — constructors take `&mut ChildSpawnerCommands` and can't reach resources, so instead insert `FocusOrder` from a startup-ish system that stamps unstamped `Focusable`s in `GlobalTransform`/UI order... KEEP IT SIMPLE: stamp in a system `stamp_focus_order` that assigns ascending ids to newly added `Focusable` entities (`Added<Focusable>` + `Local<u32>`); spawn order within a frame follows query iteration of `Added` which matches archetype insertion — verify manually with BRP; imperfect order is acceptable v1, visible ring makes it learnable).
- `focus_adjust` system: when focused entity has `Slider`+`ControlValue` → Left/Right arrows step by `(max-min)/20` (Shift = ×10 via existing `stepper_next`-style math); `Stepper` → arrows step by `spec.step`; `Toggle` → Space/Enter flips `ControlBool`.
- `paint_focus_ring` system: `Outline { width: Val::Px(3.0), offset: Val::Px(1.0), color: theme.accent }` on the focused entity, removed from others (mirror `update_focus_rings`, keyboard_nav.rs:76-94).
- Scope guard: these systems run only when any `Focusable` exists (the editor panel); they must not fight `keyboard_nav.rs`'s Tab-free scheme — keyboard_nav uses arrows/Enter on settings tabs, controls live on KIT tabs. Check `keyboard_nav.rs` gating (`editor_open` + tab kind) and gate `focus_hotkeys` to the KIT tabs correspondingly (grep how panel.rs knows the active tab; reuse that condition).

- [ ] **Step 3: Run tests + schedule build**

Run: `cargo test -p dtx-ui -j 2 && cargo check -p gameplay-drums -j 2`
Expected: PASS / clean.

- [ ] **Step 4: Manual check (bevy-brp)**

Editor → Widgets tab: Tab walks the panel controls with a visible accent ring; arrows nudge the focused stepper/slider; Space flips toggles; mouse still works.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui crates/gameplay-drums
git commit -m "feat(a11y): keyboard focus, activation, and visible focus ring on shared controls"
```

---

### Task 3: High-contrast theme + HUD text scale

**Files:**
- Modify: `crates/dtx-ui/src/theme.rs`
- Modify: `crates/dtx-config/src/lib.rs` (`SystemConfig`)
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs` + `editor/tabs.rs` (rows + live-apply)

- [ ] **Step 1: Theme variant + scale, test-first**

```rust
#[test]
fn high_contrast_pushes_text_to_full_alpha() {
    let t = Theme::high_contrast();
    assert_eq!(t.text_secondary.alpha(), 1.0);
    assert_ne!(t.panel_bg, Theme::default().panel_bg);
}

#[test]
fn font_scale_multiplies_px() {
    let mut t = Theme::default();
    t.text_scale = 1.5;
    // body_font is 16 px at scale 1.0
    // assert via whatever font() exposes — if FontSize is opaque, assert on a
    // new helper `scaled(size)` used by font():
    assert_eq!(t.scaled(16.0), 24.0);
}
```

Implement:
- `pub text_scale: f32` on `Theme` (default 1.0); `fn scaled(&self, px: f32) -> f32 { px * self.text_scale }`; `font()/title_font()/body_font()/hud_font()/label_font()` route through `scaled` — zero call-site changes, HUD text scales globally.
- `pub fn high_contrast() -> Theme`: full-alpha `text_secondary` (`Color::WHITE`), darker opaque `panel_bg`/`stage_panel_bg` (e.g. `srgb(0.05,0.05,0.07)` at alpha 1.0), brighter `gauge_track`, keep judgment/lane hues (they carry meaning) but raise their lightness floor. Assert distinctness tests still pass (theme.rs:146-193).

- [ ] **Step 2: Config + rows + live-apply**

- `SystemConfig`: add `high_contrast: bool` (default false) and `hud_text_scale: f32` (default 1.0, clamp 1.0..=1.5 in the row's adjust) with serde defaults + round-trip test (mirror existing config tests).
- `settings_data.rs` System tab: rows "High Contrast" (toggle) and "Text Size" (stepper 1.0..1.5 step 0.1, format `{:.1}x`).
- `tabs.rs::apply_draft_live`: on change rebuild `ThemeResource`:

```rust
let mut theme = if draft.0.system.high_contrast { Theme::high_contrast() } else { Theme::default() };
theme.text_scale = draft.0.system.hud_text_scale.clamp(1.0, 1.5);
theme_res.0 = theme;
```

Also apply at boot wherever `ThemeResource` is first inserted (grep `ThemeResource` init site — likely app/main or dtx-ui plugin) using the loaded config.
CAVEAT (known repo truth): most text entities read the theme at spawn — a live theme change affects newly spawned screens, not text already on screen. Acceptable v1: note in the row desc "applies on next screen". Do NOT build a retroactive restyler.

- [ ] **Step 3: Run tests**

Run: `cargo test -p dtx-ui -p dtx-config -p gameplay-drums -j 2`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-ui crates/dtx-config crates/gameplay-drums
git commit -m "feat(a11y): high-contrast theme variant and global HUD text scale"
```

---

### Task 4: Reduced flashes + background dimming (finally consume bg_alpha)

**Files:**
- Modify: `crates/dtx-config/src/lib.rs` (one field)
- Modify: `crates/dtx-ui/src/widget/lane_flush.rs`, `crates/gameplay-drums/src/keyboard_viz.rs`, `crates/gameplay-drums/src/hit_feedback.rs`
- Modify: `crates/dtx-bga/src/lib.rs` (dim multiply)
- Modify: `settings_data.rs` / `tabs.rs` (rows + live-apply)

- [ ] **Step 1: Config + resource**

- Add `reduce_flashes: bool` (default false) to `SystemConfig` (serde default + round-trip test).
- New resource in gameplay-drums (or dtx-ui — put it in dtx-ui so `lane_flush.rs` can read it without a dep cycle): `#[derive(Resource, Default)] pub struct ReduceFlashes(pub bool);` registered in dtx-ui's plugin; set from config at Performance enter + live-apply.

- [ ] **Step 2: Gate the three flash drivers**

- `lane_flush.rs::tick_lane_flushes` (:76-96): when `ReduceFlashes(true)`, clamp the flash alpha to ≤0.15 (a whisper, not a strobe) — don't remove entirely (it carries hit feedback); write the pure clamp as `fn flash_alpha(base: f32, reduced: bool) -> f32` with a unit test.
- `keyboard_viz.rs::flash_key_caps_on_hit` (:116) + `decay_key_cap_flashes`: same clamp via the shared fn.
- `hit_feedback.rs`: same treatment on its flash color alpha.

- [ ] **Step 3: Background dim — wire `bg_alpha`**

In dtx-bga's image spawn (post-BGA-image plan) or the current placeholder path: multiply the overlay's `ImageNode.color` (or `BackgroundColor`) by `bg_alpha as f32 / 255.0` from config. Thread the value the same way `ActiveChartRes` travels (song_loading has `Res<...Config...>` access — grep how config reaches `poll_chart_parse`; store `pub dim: f32` on `ActiveChartRes`). Settings row "Background Brightness" (slider 0..255 step 5, shown as %) in the System tab. Delete `bga_enabled`/`movie_enabled`from consideration (still dead, still hidden — out of scope).

- [ ] **Step 4: Rows + tests + manual**

Rows: "Reduce Flashes" toggle, "Background Brightness" slider. Run: `cargo test -p dtx-ui -p dtx-config -p gameplay-drums -p dtx-bga -j 2` → PASS. Manual: play with Reduce Flashes on — keycaps/lanes barely pulse; background slider visibly dims BGA live.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui crates/dtx-config crates/gameplay-drums crates/dtx-bga
git commit -m "feat(a11y): reduce-flashes option and background dimming via bg_alpha"
```

---

### Task 5: Color-plus-shape audit fix

**Files:**
- Modify: `crates/gameplay-drums/src/gauge.rs` HUD counterpart (grep the gauge bar widget — `dtx-ui/src/widget/gauge_bar.rs`)

- [ ] **Step 1: Audit**

Judgments are text ("PERFECT") — already shape-safe. Stepper adjust-mode glyph swap exists (keyboard_nav.rs:115-133). The one confirmed color-only signal: the gauge's danger state (value < 0.3 renders by color alone). Verify: `grep -n 'danger\|0.3' crates/dtx-ui/src/widget/gauge_bar.rs crates/gameplay-drums/src/gauge.rs`.

- [ ] **Step 2: Add a shape cue**

In the gauge bar widget: when in danger, show a `!` glyph (or "LOW") label at the bar's end in addition to the color shift. Pure threshold fn + test:

```rust
pub fn danger_label(value: f32) -> Option<&'static str> {
    (value < 0.3).then_some("LOW")
}
```

- [ ] **Step 3: Test + commit**

Run: `cargo test -p dtx-ui -j 2` → PASS.

```bash
git add crates/dtx-ui crates/gameplay-drums
git commit -m "feat(a11y): shape cue on gauge danger state"
```

---

### Task 6: No Fail

**Files:**
- Modify: `crates/dtx-config/src/lib.rs` (`GameplayConfig::no_fail: bool`, serde default)
- Modify: `crates/gameplay-drums/src/stage_end.rs` (`detect_stage_failure` :64-86)
- Modify: `crates/game-results/src/lib.rs` (persist gate + visible row)
- Modify: `settings_data.rs` (row)

- [ ] **Step 1: Tests first**

- dtx-config round-trip for the new field (mirror existing).
- game-results: extend `persistence_allowed` (from the play-speed plan; create it here if that plan hasn't landed):

```rust
pub fn persistence_allowed(play_speed: f32, practice: bool, no_fail: bool) -> bool {
    !practice && !no_fail && (play_speed - 1.0).abs() < 0.001
}

#[test]
fn no_fail_blocks_persistence() {
    assert!(!persistence_allowed(1.0, false, true));
    assert!(persistence_allowed(1.0, false, false));
}
```

- stage_end pure guard:

```rust
pub fn failure_fires(gauge_failed: bool, practice: bool, editor: bool, no_fail: bool) -> bool {
    gauge_failed && !practice && !editor && !no_fail
}
```

with a truth-table test.

- [ ] **Step 2: Wire**

- New resource `NoFailEnabled(pub bool)` in gameplay-drums, set from config at Performance enter + live-apply (mirror `ShowPerfInfo` plumbing, lib.rs:239-258).
- `detect_stage_failure`: route its condition through `failure_fires(...)` — gauge keeps draining and displaying (honest state; the run just doesn't end).
- Persist gate: pass `no_fail` into `persistence_allowed`; when blocked by no_fail append the visible row `("No Fail — score not saved", 0.0)` in `spawn_result` (same pattern as the play-speed row).
- Settings row (Gameplay tab): "No Fail" toggle, desc "Failing the gauge no longer ends the song. Runs do not save scores."

- [ ] **Step 3: Run + manual**

Run: `cargo test -p gameplay-drums -p game-results -p dtx-config -j 2` → PASS.
Manual: enable No Fail, tank the gauge — song continues to the end, results show the no-save row, `scores.json` untouched.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums crates/game-results crates/dtx-config
git commit -m "feat(a11y): no-fail mode, gated from normal score records"
```

---

## Verification (whole plan)

1. `cargo test -p dtx-ui -p dtx-config -p gameplay-drums -p game-results -p dtx-bga -j 2` green + schedule guard.
2. Manual keyboard-only session: open editor, Tab through controls with visible ring, adjust everything without the mouse.
3. High contrast + 1.5x text: HUD readable at distance; screenshots archived.
4. Reduce Flashes on: no full-brightness strobes during dense play; background dim slider works live.
5. No Fail run reaches results after gauge exhaustion; nothing persisted; row says so.

## Deliberately out of scope

- Screen-reader output (no bevy a11y tree wired in this codebase; `ControlName` is the seed for later `AccessibilityNode` work).
- Retroactive live-restyle of already-spawned text (Task 3 caveat).
- Colorblind-specific palettes (high-contrast variant first; palette variants ride the same `ThemeResource` mechanism later).
