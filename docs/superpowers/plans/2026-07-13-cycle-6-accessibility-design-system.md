# Cycle 6 Accessibility and Design-System Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add independent readable-text, reduced-motion, reduced-flash, and background-motion controls; make No Fail explicit and non-qualifying; and migrate player-critical UI to shared accessible primitives.

**Architecture:** `dtx-config` owns version-tolerant persisted choices, while `dtx-ui` derives one runtime `AccessibilityPolicy` and exposes semantic typography, color, motion, component, and reference-layout APIs. Gameplay and screen crates consume that policy without reading configuration themselves. No Fail is normalized at the config boundary and copied into the completed-run context before Results decides persistence.

**Tech Stack:** Rust 1.95+, Bevy 0.19, Serde/TOML, `dtx-config`, `dtx-ui`, `dtx-bga`, `game-shell`, `game-menu`, `gameplay-drums`, and `game-results`.

## Global Constraints

- Defaults reproduce current behavior: Standard text, full motion and flashes, and moving backgrounds enabled.
- Text scaling affects player-facing text, never notes, timing windows, lane widths, density geometry, or scoring.
- Reduced motion keeps timing-bearing motion and uses a 120 ms OutQuint opacity screen fade.
- Reduced flashes must preserve a visible outline, marker, or low-contrast 120 ms state change.
- Background Motion off suppresses decorative parallax, movies, BGAPAN, and AVIPAN motion while retaining static BGA state.
- No Fail is an assisted modifier and must not update history, PB, rank records, compatible score.ini data, or player skill.
- Color is never the only signal for focus, selection, errors, destructive actions, failure, or modifier state.
- Distant-kit gestures and control grammar remain owned by the system-bind design and are not changed here.
- Preserve the 1280×720 reference coordinate system and support 1920×1080 and 2560×1080.
- Do not modify CI/CD files.

---

## File structure

- Create `crates/dtx-ui/src/accessibility.rs`: runtime policy and effect decisions.
- Create `crates/dtx-ui/src/typography.rs`: semantic text and spacing roles.
- Create `crates/dtx-ui/src/widget/action_button.rs`: shared action state and activation reducer.
- Create `crates/dtx-ui/src/widget/modal_dialog.rs`: focus-trapped modal model and renderer helpers.
- Create `crates/dtx-ui/src/widget/notification.rs`: bounded typed notification queue and presentation.
- Create `crates/dtx-ui/src/reference_layout.rs`: safe-area, compact-layout, and widget recovery helpers.
- Modify `crates/dtx-config/src/lib.rs`: accessibility persistence and canonical Fail Mode migration.
- Modify `crates/dtx-ui/src/lib.rs`, `theme.rs`, `motion.rs`, `parallax.rs`, `transition.rs`, and `widget/mod.rs`: register and apply shared policies.
- Modify `crates/game-shell/src/states.rs` and `lib.rs`: Accessibility Customize tab and assisted-run context.
- Modify `crates/gameplay-drums/src/editor/settings_data.rs`, `tabs.rs`, `ui.rs`, `panel.rs`, and `profile_dialog_ui.rs`: independent controls and shared components.
- Modify `crates/gameplay-drums/src/stage_end.rs`, `orchestrator.rs`, `hud.rs`, `hit_feedback.rs`, `keyboard_viz.rs`, `pause.rs`, `widget_layout.rs`, and practice HUD/toast modules: No Fail, reduced effects, layout safety, and migrated UI.
- Modify `crates/dtx-bga/src/lib.rs`: effective static/movie/motion policy.
- Modify `crates/game-menu/src/title.rs`, `song_select.rs`, `song_loading.rs`, and `import_ui.rs`: semantic typography/components and reduced motion.
- Modify `crates/game-results/src/lib.rs`, `input.rs`, and `ui.rs`: No Fail persistence exclusion and accessible result UI.
- Modify `app/dtxmaniars-desktop/src/main.rs`: load the runtime policy once at startup.

### Task 1: Persist accessibility choices and derive one runtime policy

**Files:**
- Modify: `crates/dtx-config/src/lib.rs`
- Modify: `crates/dtx-ui/Cargo.toml`
- Create: `crates/dtx-ui/src/accessibility.rs`
- Modify: `crates/dtx-ui/src/lib.rs`
- Modify: `app/dtxmaniars-desktop/src/main.rs`

**Interfaces:**
- Consumes: `dtx_config::load`, the existing desktop startup, and Bevy resources.
- Produces: `TextScale`, `AccessibilityConfig`, `ConfigLoadReport`, `StartupConfigWarning`, `AccessibilityPolicy`, `MotionDecision`, and `FlashDecision`.

- [ ] **Step 1: Write failing config and policy tests**

```rust
#[test]
fn old_config_defaults_to_current_accessibility_behavior() {
    let cfg: Config = toml::from_str("").unwrap();
    assert_eq!(cfg.accessibility.text_scale, TextScale::Standard);
    assert!(!cfg.accessibility.reduce_motion);
    assert!(!cfg.accessibility.reduce_flashes);
    assert!(cfg.accessibility.background_motion);
}

#[test]
fn policy_maps_independent_controls_without_a_preset() {
    let cfg = AccessibilityConfig {
        text_scale: TextScale::XLarge,
        reduce_motion: true,
        reduce_flashes: false,
        background_motion: false,
    };
    let policy = AccessibilityPolicy::from(&cfg);
    assert_eq!(policy.text_multiplier(), 1.5);
    assert_eq!(policy.screen_transition_ms(), 120);
    assert_eq!(policy.flash_decision(), FlashDecision::Full);
    assert!(!policy.background_motion());
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-config -p dtx-ui accessibility`

Expected: FAIL because the config section and policy types do not exist.

- [ ] **Step 3: Implement persisted values and policy conversion**

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextScale {
    #[default]
    Standard,
    Large,
    XLarge,
}

impl TextScale {
    pub const fn multiplier(self) -> f32 {
        match self { Self::Standard => 1.0, Self::Large => 1.25, Self::XLarge => 1.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilityConfig {
    pub text_scale: TextScale,
    pub reduce_motion: bool,
    pub reduce_flashes: bool,
    pub background_motion: bool,
}

pub struct ConfigLoadReport {
    pub config: Config,
    pub warning: Option<String>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct StartupConfigWarning(pub Option<String>);
```

Add `dtx-config = { workspace = true }` to `dtx-ui` so the policy conversion
depends only on a Pure-layer crate.

Add `#[serde(default)] pub accessibility: AccessibilityConfig` to `Config`. Define `AccessibilityPolicy` as a Bevy `Resource` copied from this config, with `screen_transition_ms()` returning 120 or 300 and independent motion/flash/background accessors. Register it in `dtx_ui::plugin`, and insert the loaded value during desktop startup before the first screen is spawned.

Add `load_with_report(path) -> ConfigLoadReport`, where the report contains the
recovered config and an optional readable parse warning. Keep `load(path)` as a
compatibility wrapper returning only the config. Store a pending startup
warning resource; Task 5 drains it into `NotificationQueue` after the shared
notification primitive is registered.

- [ ] **Step 4: Verify round-trip/default behavior and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-config -p dtx-ui accessibility`

Expected: PASS, including a TOML round-trip at all three scales and a malformed-enum load falling back through the existing whole-config recovery policy.

```bash
git add crates/dtx-config/src/lib.rs crates/dtx-ui/Cargo.toml crates/dtx-ui/src/accessibility.rs crates/dtx-ui/src/lib.rs app/dtxmaniars-desktop/src/main.rs Cargo.lock
git commit -m "feat: persist independent accessibility controls"
```

### Task 2: Expose the controls in Customize with live preview and discard

**Files:**
- Modify: `crates/game-shell/src/states.rs`
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs`
- Modify: `crates/gameplay-drums/src/editor/ui.rs`
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs`
- Modify: `crates/gameplay-drums/src/editor/chrome.rs`
- Modify: `crates/gameplay-drums/src/editor/close_dialog.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`

**Interfaces:**
- Consumes: `AccessibilityConfig`, `AccessibilityPolicy`, `ConfigDraft`, and `CustomizeTab` navigation.
- Produces: `CustomizeTab::Accessibility`, four independently focusable rows, `SavedConfigDraft`, and save/discard policy synchronization.

- [ ] **Step 1: Write failing tab and settings-table tests**

```rust
#[test]
fn accessibility_tab_is_a_settings_tab() {
    assert!(CustomizeTab::Accessibility.is_settings());
    assert_eq!(CustomizeTab::SETTINGS.len(), 5);
}

#[test]
fn accessibility_rows_are_independent() {
    let items = settings_items(CustomizeTab::Accessibility);
    assert_eq!(items.iter().map(|item| item.label).collect::<Vec<_>>(),
        ["Text Scale", "Reduce Motion", "Reduce Flashes", "Background Motion"]);
    let mut cfg = dtx_config::Config::default();
    (items[1].adjust)(&mut cfg, 1);
    assert!(cfg.accessibility.reduce_motion);
    assert!(!cfg.accessibility.reduce_flashes);
    assert!(cfg.accessibility.background_motion);
}

#[test]
fn discard_restores_saved_config_and_policy() {
    let saved = SavedConfigDraft(dtx_config::Config::default());
    let mut draft = ConfigDraft(saved.0.clone());
    draft.0.accessibility.text_scale = TextScale::XLarge;
    discard_config_draft(&saved, &mut draft);
    assert_eq!(draft.0, saved.0);
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-shell -p gameplay-drums accessibility_tab`

Expected: FAIL because the tab and rows are absent.

- [ ] **Step 3: Add the tab, rows, and live runtime update**

Add Accessibility between System and Controls. Text Scale cycles Standard → Large → XLarge; each boolean row toggles only its own field. Keep `ConfigDraft` as the working copy and add `SavedConfigDraft` as the last loaded/successfully saved snapshot. Save writes the working copy and updates the snapshot only on success; Discard copies the snapshot into the working copy. Closing a dirty settings draft uses the existing close-decision surface until Task 5 migrates it to `ModalDialog`. Extend `apply_draft_live` with:

```rust
let next_policy = dtx_ui::AccessibilityPolicy::from(&draft.0.accessibility);
if *accessibility_policy != next_policy {
    *accessibility_policy = next_policy;
}
```

Because policy is derived from the working draft, Save and Discard immediately produce the matching runtime value. Keyboard, pad, and pointer row activation continue through the existing `SettingItem.adjust` path. A failed save retains the dirty working copy and current preview while surfacing the existing footer error.

- [ ] **Step 4: Verify navigation, save, discard, and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p game-shell -p gameplay-drums editor`

Expected: PASS; opening, live preview, save, and discard tests observe identical config/policy values.

```bash
git add crates/game-shell/src/states.rs crates/gameplay-drums/src/editor/settings_data.rs crates/gameplay-drums/src/editor/tabs.rs crates/gameplay-drums/src/editor/ui.rs crates/gameplay-drums/src/editor/keyboard_nav.rs crates/gameplay-drums/src/editor/chrome.rs crates/gameplay-drums/src/editor/close_dialog.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat: add accessibility customize controls"
```

### Task 3: Normalize and qualify No Fail runs

**Files:**
- Modify: `crates/dtx-config/src/lib.rs`
- Modify: `crates/game-shell/src/states.rs`
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs`
- Modify: `crates/gameplay-drums/src/resources.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/stage_end.rs`
- Modify: `crates/gameplay-drums/src/orchestrator.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`
- Modify: `crates/game-results/src/lib.rs`
- Modify: `crates/game-results/src/ui.rs`

**Interfaces:**
- Consumes: `GameplayConfig`, `DamageLevel`, `CompletedRunContext`, and the Results save gate.
- Produces: `FailMode`, `NoFailEnabled`, `RunModifiers { no_fail }`, `SaveStatus::NoFail`, and the persistent `NO FAIL` badge.

- [ ] **Step 1: Write failing migration and score-exclusion tests**

```rust
#[test]
fn legacy_damage_none_migrates_to_canonical_no_fail() {
    let mut cfg = Config::default();
    cfg.gameplay.damage_level = DamageLevel::None;
    cfg.normalize_legacy();
    assert_eq!(cfg.gameplay.fail_mode(), FailMode::NoFail);
    assert_eq!(cfg.gameplay.damage_level, DamageLevel::Small);
    assert!(!cfg.gameplay.stage_failed_enabled);
}

#[test]
fn no_fail_result_never_mutates_score_store() {
    let run = CompletedRunContext::normal(1.0, RunModifiers { no_fail: true });
    let mut world = result_world_with_run(run);
    world.run_system_once(save_result).unwrap();
    assert_eq!(world.resource::<SaveStatus>(), &SaveStatus::NoFail);
    assert!(world.resource::<ScoreStoreResource>().is_empty());
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-config -p game-shell -p game-results no_fail`

Expected: FAIL because canonical migration and run modifiers are absent.

- [ ] **Step 3: Implement canonical Fail Mode and result qualification**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailMode { Standard, NoFail }

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoFailEnabled(pub bool);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunModifiers { pub no_fail: bool }

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CompletedRunContext {
    pub kind: RunKind,
    pub playback_rate: f64,
    pub modifiers: RunModifiers,
}
```

`dtx_config::load` and `load_with_report` call `normalize_legacy`: `DamageLevel::None` becomes Small plus `stage_failed_enabled = false`. Saving always emits a non-None damage severity. Customize replaces Damage Level's None option with separate Fail Mode and Small/Normal/High rows; the severity row reports disabled while No Fail is active. `apply_config_on_enter` and live draft preview update `NoFailEnabled`; `detect_stage_failure` checks it. Performance HUD renders a text-plus-shield `NO FAIL` badge. Both clear and failure completion copy it into `CompletedRunContext`. Results checks practice, modified speed, then No Fail before constructing any score entry or writing score.ini, and renders `NO FAIL` plus `Not saved: No Fail enabled`.

- [ ] **Step 4: Verify legacy/current configs, both completion paths, persistence exclusion, and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-config -p game-shell -p gameplay-drums -p game-results no_fail`

Expected: PASS; No Fail cannot write either native history or score.ini.

```bash
git add crates/dtx-config/src/lib.rs crates/game-shell/src/states.rs crates/gameplay-drums/src/editor/settings_data.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/tabs.rs crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/stage_end.rs crates/gameplay-drums/src/orchestrator.rs crates/gameplay-drums/src/hud.rs crates/game-results/src/lib.rs crates/game-results/src/ui.rs
git commit -m "feat: make no fail explicit and non-qualifying"
```

### Task 4: Establish semantic typography, color, and spacing

**Files:**
- Create: `crates/dtx-ui/src/typography.rs`
- Modify: `crates/dtx-ui/src/theme.rs`
- Modify: `crates/dtx-ui/src/lib.rs`

**Interfaces:**
- Consumes: `AccessibilityPolicy` and the existing `Theme`.
- Produces: `TypographyRole`, `Typography`, `SpacingRole`, `InteractionTone`, and non-color `StateMarker`.

- [ ] **Step 1: Write failing token tests**

```rust
#[test]
fn semantic_text_scales_and_never_drops_below_minimum() {
    let typography = Typography::default();
    assert_eq!(typography.px(TypographyRole::Body, TextScale::Large), 20.0);
    assert!(typography.px(TypographyRole::Hint, TextScale::Standard) >= 14.0);
    assert_eq!(typography.px(TypographyRole::Hud, TextScale::XLarge), 48.0);
}

#[test]
fn interaction_tones_always_have_shape_or_text_markers() {
    for tone in InteractionTone::ALL {
        assert!(!tone.marker().label().is_empty());
    }
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui typography`

Expected: FAIL because semantic roles are absent.

- [ ] **Step 3: Implement roles and extend Theme**

```rust
pub enum TypographyRole { Display, Title, Heading, Body, Label, Hint, Hud }
pub enum SpacingRole { Xs, Sm, Md, Lg, Xl }
pub enum InteractionTone { Focus, Selected, Error, Destructive, Success, Disabled }

impl Typography {
    pub fn font(self, role: TypographyRole, policy: AccessibilityPolicy) -> TextFont {
        Theme::font(self.px(role, policy.text_scale()))
    }
}
```

Give each interaction tone a theme color and a stable marker (`>`, check, `!`, destructive label, confirmation marker, or unavailable marker). Retain lane/difficulty colors as domain tokens. Deprecate raw player-facing constants only after all Cycle 6 call sites migrate.

- [ ] **Step 4: Verify token coverage and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui`

Expected: PASS at 1.00, 1.25, and 1.50 scales.

```bash
git add crates/dtx-ui/src/typography.rs crates/dtx-ui/src/theme.rs crates/dtx-ui/src/lib.rs
git commit -m "feat: add semantic accessible ui tokens"
```

### Task 5: Add shared action, dialog, and notification primitives

**Files:**
- Create: `crates/dtx-ui/src/widget/action_button.rs`
- Create: `crates/dtx-ui/src/widget/modal_dialog.rs`
- Create: `crates/dtx-ui/src/widget/notification.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs`
- Modify: `crates/dtx-ui/src/lib.rs`
- Modify: `crates/game-menu/src/import_ui.rs`
- Modify: `app/dtxmaniars-desktop/src/main.rs`
- Modify: `crates/gameplay-drums/src/practice/toast.rs`
- Modify: `crates/gameplay-drums/src/editor/profile_dialog_ui.rs`
- Modify: `crates/gameplay-drums/src/editor/capture_modal.rs`
- Modify: `crates/gameplay-drums/src/pause.rs`

**Interfaces:**
- Consumes: semantic tokens, `AccessibilityPolicy`, and existing navigation actions.
- Produces: `ActionButton`, `ActionButtonState`, `ModalDialog`, `DialogAction`, `NotificationQueue`, `NotificationTone`, and `Notification`.

- [ ] **Step 1: Write failing reducer/queue tests**

```rust
#[test]
fn all_activation_sources_emit_the_same_action() {
    let action = DialogAction::Confirm;
    for source in [ActivationSource::Keyboard, ActivationSource::Pad, ActivationSource::Pointer] {
        assert_eq!(reduce_activation(source, action, false), Some(action));
    }
    assert_eq!(reduce_activation(ActivationSource::Keyboard, action, true), None);
}

#[test]
fn notifications_are_bounded_and_errors_live_long_enough() {
    let mut queue = NotificationQueue::with_capacity(4);
    for n in 0..5 { queue.push(Notification::info(n.to_string())); }
    assert_eq!(queue.len(), 4);
    assert!(Notification::error("save failed").lifetime_ms() >= 5_000);
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui`

Expected: FAIL because the primitives are absent.

- [ ] **Step 3: Implement primitives and migrate existing duplicate surfaces**

`ActionButton` carries action, visual state, tone, and marker. `ModalDialog` traps focus inside its action list, never defaults focus to a destructive action, and maps cancel explicitly. `NotificationQueue` stores at most four visible entries, ages them deterministically, uses opacity-only fades under reduced motion, and keeps errors until their readable minimum expires. Drain the startup `ConfigLoadReport` warning after plugin initialization. Replace import status, practice toast rendering, editor profile/capture dialogs, and pause actions while retaining their domain reducers. Failed config/layout/profile writes enqueue Error without clearing the dirty draft.

- [ ] **Step 4: Verify keyboard/pad/pointer parity and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui -p game-menu -p gameplay-drums`

Expected: PASS; save/import failures remain visible and dirty drafts remain intact.

```bash
git add crates/dtx-ui/src/widget/action_button.rs crates/dtx-ui/src/widget/modal_dialog.rs crates/dtx-ui/src/widget/notification.rs crates/dtx-ui/src/widget/mod.rs crates/dtx-ui/src/lib.rs app/dtxmaniars-desktop/src/main.rs crates/game-menu/src/import_ui.rs crates/gameplay-drums/src/practice/toast.rs crates/gameplay-drums/src/editor/profile_dialog_ui.rs crates/gameplay-drums/src/editor/capture_modal.rs crates/gameplay-drums/src/pause.rs
git commit -m "feat: consolidate accessible ui primitives"
```

### Task 6: Apply reduced motion, flashes, and background motion

**Files:**
- Modify: `crates/dtx-ui/src/motion.rs`
- Modify: `crates/dtx-ui/src/parallax.rs`
- Modify: `crates/dtx-ui/src/transition.rs`
- Modify: `crates/dtx-ui/src/widget/rolling_counter.rs`
- Modify: `crates/dtx-ui/src/widget/lane_flush.rs`
- Modify: `crates/dtx-ui/src/widget/judgment_popup.rs`
- Modify: `crates/gameplay-drums/src/hit_feedback.rs`
- Modify: `crates/gameplay-drums/src/keyboard_viz.rs`
- Modify: `crates/gameplay-drums/src/drums_perf.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`
- Modify: `crates/dtx-bga/src/lib.rs`
- Modify: `crates/game-menu/src/song_select.rs`

**Interfaces:**
- Consumes: `AccessibilityPolicy`, current tween/flash states, `SystemConfig`, and `BgaSettings`.
- Produces: policy-driven effect transforms and `BgaSettings { images_enabled, movies_enabled, motion_enabled, image_alpha, movie_alpha }`.

- [ ] **Step 1: Write failing effect-decision tests**

```rust
#[test]
fn reduced_effects_keep_feedback_but_remove_oscillation() {
    let policy = policy(true, true, false);
    assert_eq!(entrance_effect(policy), EntranceEffect::OpacityOnly { duration_ms: 120 });
    assert_eq!(hit_effect(policy), HitEffect::StableOutline { duration_ms: 120 });
    assert_eq!(danger_effect(policy), DangerEffect::ConstantBorder);
}

#[test]
fn background_motion_off_keeps_static_images_only() {
    let settings = BgaSettings::from_configs(&SystemConfig::default(), &accessibility(false));
    assert!(settings.images_enabled);
    assert!(!settings.movies_enabled);
    assert!(!settings.motion_enabled);
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui -p dtx-bga`

Expected: FAIL because effects do not consume the policy.

- [ ] **Step 3: Centralize policy transformations**

Make entrance translations/parallax/beat pulses/rolling overshoot resolve through `MotionDecision`; timing-bearing scroll, progress, gauge, and practice playheads bypass it. Screen fades read 120 or 300 ms from policy and keep OutQuint easing. Lane/key-cap/judgment/danger effects resolve through `FlashDecision`, with a stable marker for 120 ms under reduction. Build BGA settings from both system and accessibility config; when background motion is off, stop movies and animated geometry but rebuild the latest static image on seek.

- [ ] **Step 4: Verify state-preserving reduced effects and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui -p dtx-bga -p gameplay-drums reduced`

Expected: PASS; every affected gameplay event still produces a visible state change.

```bash
git add crates/dtx-ui/src/motion.rs crates/dtx-ui/src/parallax.rs crates/dtx-ui/src/transition.rs crates/dtx-ui/src/widget/rolling_counter.rs crates/dtx-ui/src/widget/lane_flush.rs crates/dtx-ui/src/widget/judgment_popup.rs crates/gameplay-drums/src/hit_feedback.rs crates/gameplay-drums/src/keyboard_viz.rs crates/gameplay-drums/src/drums_perf.rs crates/gameplay-drums/src/hud.rs crates/dtx-bga/src/lib.rs crates/game-menu/src/song_select.rs
git commit -m "feat: honor reduced effects and background motion"
```

### Task 7: Migrate player-critical typography and enforce safe layouts

**Files:**
- Create: `crates/dtx-ui/src/reference_layout.rs`
- Modify: `crates/dtx-ui/src/lib.rs`
- Modify: `crates/game-menu/src/title.rs`
- Modify: `crates/game-menu/src/song_select.rs`
- Modify: `crates/game-menu/src/song_loading.rs`
- Modify: `crates/game-results/src/ui.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`
- Modify: `crates/gameplay-drums/src/pause.rs`
- Modify: `crates/gameplay-drums/src/stage_end.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mini_strip.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/wait_prompt.rs`
- Modify: `crates/gameplay-drums/src/editor/chrome.rs`
- Modify: `crates/gameplay-drums/src/editor/panel.rs`
- Modify: `crates/gameplay-drums/src/widget_layout.rs`
- Test: `crates/gameplay-drums/tests/practice_hud.rs`
- Test: `crates/gameplay-drums/tests/widget_layout.rs`

**Interfaces:**
- Consumes: `Typography`, `AccessibilityPolicy`, reference-space geometry, and persisted widget positions.
- Produces: `SafeArea`, `FitDecision`, `repair_runtime_rect`, and semantic text on all player-critical screens.

- [ ] **Step 1: Write failing pure layout tests**

```rust
#[test]
fn xlarge_text_chooses_compact_layout_before_shrinking_below_minimum() {
    let fit = fit_overlay(Size::new(420.0, 180.0), SafeArea::for_viewport(1280.0, 720.0), 1.5);
    assert_eq!(fit, FitDecision::CompactScrollable);
}

#[test]
fn offscreen_widget_is_repaired_for_runtime_without_rewriting_persistence() {
    let saved = Rect::new(1400.0, 900.0, 200.0, 80.0);
    let repaired = repair_runtime_rect(saved, SafeArea::reference_720p(), Size::new(24.0, 24.0));
    assert_ne!(repaired, saved);
    assert!(SafeArea::reference_720p().contains_focus_handle(repaired));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui -p gameplay-drums`

Expected: FAIL because layout helpers are absent.

- [ ] **Step 3: Implement reference-layout helpers and migrate screens**

Apply semantic roles and shared `ActionButton` states to Title, Song Select, Loading, Pause, stage banners, HUD labels, Results, practice controls, notifications, and Customize dialogs. Editor microcopy uses the roles; dense canvases may retain Standard text only when the focused description and error text still use the selected scale. Keep notes and mechanics geometry unchanged. Convert fixed screen-pixel practice/editor rails to reference-space safe-area constraints. Wrap at word boundaries; use scrolling or the declared compact layout for overflow. Runtime widget repair clamps enough of the focus handle and critical text into view, but the saved position changes only after the player moves and saves it. Missing icon glyphs fall back to the token's text marker.

- [ ] **Step 4: Verify all supported viewports/scales and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-ui -p game-menu -p game-results -p gameplay-drums`

Expected: PASS for 1280×720, 1920×1080, 2560×1080, and XL text fixtures.

```bash
git add crates/dtx-ui/src/reference_layout.rs crates/dtx-ui/src/lib.rs crates/game-menu/src/title.rs crates/game-menu/src/song_select.rs crates/game-menu/src/song_loading.rs crates/game-results/src/ui.rs crates/gameplay-drums/src/hud.rs crates/gameplay-drums/src/pause.rs crates/gameplay-drums/src/stage_end.rs crates/gameplay-drums/src/practice/hud/full_hud.rs crates/gameplay-drums/src/practice/hud/mini_strip.rs crates/gameplay-drums/src/practice/hud/wait_prompt.rs crates/gameplay-drums/src/editor/chrome.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/widget_layout.rs crates/gameplay-drums/tests/practice_hud.rs crates/gameplay-drums/tests/widget_layout.rs
git commit -m "feat: scale critical text and recover layouts"
```

### Task 8: Verify Cycle 6 and record completion

**Files:**
- Modify: `docs/notes/2026-07-13-game-improvement-program.md`

- [ ] **Step 1: Run focused package gates**

Run: `cargo fmt --all -- --check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-config -p dtx-ui -p dtx-bga -p game-shell -p game-menu -p gameplay-drums -p game-results && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy -p dtx-config -p dtx-ui -p dtx-bga -p game-shell -p game-menu -p gameplay-drums -p game-results --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 2: Perform the required visual/manual matrix**

Run the desktop app at 1280×720, 1920×1080, and 2560×1080. Check Standard and XL at desk distance and 2.5–3.5 m; repeat with Reduce Motion, Reduce Flashes, and Background Motion independently toggled. Confirm grayscale focus/selection/error meaning, keyboard/MIDI/pointer activation, No Fail badge/save explanation, and safe access to every only action.

Expected: Every independent setting is live, persisted, reversible, and retains required gameplay state.

- [ ] **Step 3: Run workspace gates**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo check --workspace && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy --workspace --all-targets -- -D warnings && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test --workspace --lib`

Expected: PASS.

- [ ] **Step 4: Update the ledger and commit**

Record the verified controls, No Fail exclusion, shared primitives, reduced-effects behavior, and viewport matrix under Cycle 6.

```bash
git add docs/notes/2026-07-13-game-improvement-program.md
git commit -m "docs: record accessibility cycle completion"
```

## Plan self-review

The eight tasks cover every Cycle 6 design section: persisted independent settings, live preview/discard, text roles, reduced motion/flashes/backgrounds, canonical No Fail migration and score exclusion, semantic color-plus-shape cues, shared actions/dialogs/notifications, reference-space safety, error retention, automated coverage, and the manual distance/viewport matrix. `AccessibilityPolicy`, typography roles, run modifiers, and layout helpers are introduced before consumers. No gameplay timing, lane routing, scoring formula, distant-kit grammar, reference content, or CI/CD file is changed.
