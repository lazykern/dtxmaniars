# Settings Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the Customize surface's data-loss path, visual bugs, and non-live settings, and add offset calibration + keyboard tab switching.

**Architecture:** All work is in `crates/gameplay-drums/src/editor/` (the surface), `crates/dtx-config/` (config), `crates/game-shell/src/states.rs` (tab enum), and `app/dtxmaniars-desktop/src/main.rs` (window vsync). Live-apply extends `editor/tabs.rs::apply_draft_live` to push the whole `ConfigDraft` into the narrow live resources that `gameplay-drums/src/lib.rs::apply_config_on_enter` already drives.

**Tech Stack:** Rust, Bevy 0.19, bevy_kira_audio. Build with `cargo`. Verify UI with the `bevy-brp` MCP tools (launch `dtxmaniars`, F1 → surface, screenshot).

**Conventions (READ FIRST):**
- No AI co-author trailer in commits. No unnecessary comments.
- NEVER run bare `cargo fmt --all` (memory: rustfmt version drift reformats unrelated files). If formatting, use `cargo fmt -p <crate> -- <file>` scoped, or skip.
- Bevy 0.19 UI uses `UiGlobalTransform`, not `GlobalTransform` (memory). Not relevant to most tasks here but do not add `&GlobalTransform` UI queries.
- Green unit tests do NOT prove the FixedUpdate schedule builds (memory: tests-skip-real-plugin-schedule). After schedule/system changes, do a real `cargo run` launch or the ordering guard.
- Build check per task: `cargo build -p <crate>` (debug). Full test: `cargo test -p gameplay-drums -p dtx-config -p game-shell`.

---

## Task 1: Delete dead `game-menu/src/config.rs`

**Files:**
- Delete: `crates/game-menu/src/config.rs`

- [ ] **Step 1: Confirm it is unreferenced**

Run: `grep -rn "mod config\b\|menu::config\|game_menu::config" crates/ app/`
Expected: no hits (the file is not declared `pub mod config;` in `crates/game-menu/src/lib.rs`).

- [ ] **Step 2: Delete the file**

Run: `git rm crates/game-menu/src/config.rs`

- [ ] **Step 3: Build the crate**

Run: `cargo build -p game-menu`
Expected: success (nothing referenced it).

- [ ] **Step 4: Commit**

```bash
git commit -m "chore(game-menu): delete dead config.rs orphan"
```

---

## Task 2: Micro-fixes (config doc comment + footer hint offset)

**Files:**
- Modify: `crates/dtx-config/src/lib.rs:195`
- Modify: `crates/gameplay-drums/src/editor/footer.rs`

- [ ] **Step 1: Fix the scroll_speed doc comment**

In `crates/dtx-config/src/lib.rs`, change the line:
```rust
    /// Scroll speed multiplier 0.5..4.0.
```
to:
```rust
    /// Scroll speed multiplier 0.5..9.0.
```

- [ ] **Step 2: Offset the footer description past the left panels**

The footer description text starts at window x=16 and is hidden behind the rail + left panel. In `crates/gameplay-drums/src/editor/footer.rs`, the `FooterDescText` spawn (inside `spawn_footer_on_open`) currently is:
```rust
        p.spawn((
            FooterDescText,
            Text::new(desc_text(&desc)),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
        ));
```
Add a left margin clearing the rail + left panel so the hint sits in the preview area:
```rust
        p.spawn((
            FooterDescText,
            Text::new(desc_text(&desc)),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
            Node {
                margin: UiRect::left(Val::Px(
                    super::chrome::RAIL_WIDTH + super::chrome::LEFT_PANEL_WIDTH,
                )),
                ..default()
            },
        ));
```

- [ ] **Step 3: Verify `RAIL_WIDTH` and `LEFT_PANEL_WIDTH` are `pub` in chrome.rs**

Run: `grep -n "RAIL_WIDTH\|LEFT_PANEL_WIDTH" crates/gameplay-drums/src/editor/chrome.rs`
Expected: both are `pub const ... : f32`. If either is not `pub`, make it `pub`.

- [ ] **Step 4: Build**

Run: `cargo build -p dtx-config -p gameplay-drums`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git commit -am "fix(config,editor): correct scroll doc range; unhide footer hint behind panels"
```

---

## Task 3: Drums/slider value-text overflow

**Files:**
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (the `spawn_settings_block` stepper + slider value `Text` spawns)

- [ ] **Step 1: Add NoWrap + wider column to the stepper value text**

In `spawn_settings_block`, the `SettingControl::Stepper` arm spawns the value text with `min_width: Val::Px(60.0)`. Change it to widen the column and prevent wrapping:
```rust
                        c.spawn((
                            SettingValueText(i),
                            Text::new((item.value)(&draft.0)),
                            dtx_ui::theme::Theme::font(12.0),
                            TextColor(t.text_primary),
                            TextLayout::new_with_no_wrap(),
                            Node {
                                min_width: Val::Px(96.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ));
```

- [ ] **Step 2: Add NoWrap to the slider value text**

In the `SettingControl::Slider` arm, the value text uses `min_width: Val::Px(52.0)`. Add the no-wrap layout:
```rust
                        c.spawn((
                            SettingValueText(i),
                            Text::new((item.value)(&draft.0)),
                            dtx_ui::theme::Theme::font(12.0),
                            TextColor(t.text_primary),
                            TextLayout::new_with_no_wrap(),
                            Node {
                                min_width: Val::Px(52.0),
                                ..default()
                            },
                        ));
```

- [ ] **Step 3: Reduce stepper button horizontal padding to buy room**

Both `SettingAdjust` buttons in the Stepper arm use `padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0))`. Change the horizontal `6.0` to `5.0` on both.

- [ ] **Step 4: Confirm `TextLayout::new_with_no_wrap` is the correct Bevy 0.19 API**

Run: `grep -rn "new_with_no_wrap\|LineBreak::NoWrap\|TextLayout" crates/ | head`
Expected: if the codebase already uses `TextLayout`, match its form. If `new_with_no_wrap` does not exist in this Bevy version, use `TextLayout { linebreak: bevy::text::LineBreak::NoWrap, ..default() }` instead. Pick whichever compiles.

- [ ] **Step 5: Build**

Run: `cargo build -p gameplay-drums`
Expected: success.

- [ ] **Step 6: BRP-verify the Drums tab (done later in the Verification task)** — leave a note; no per-task launch required.

- [ ] **Step 7: Commit**

```bash
git commit -am "fix(editor): stop settings value text wrapping onto next row"
```

---

## Task 4: Layout auto-save on close

**Files:**
- Modify: `crates/gameplay-drums/src/editor/save.rs` (add `save_layout_on_close`, register it)
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (`close_editor_on_exit` saves config + layout + bindings)

- [ ] **Step 1: Add `save_layout_on_close` in save.rs**

Add this system and register it in `save.rs`'s `plugin`:
```rust
/// Layout auto-saves when the surface closes (EditorOpen true→false while still
/// in Performance — the Esc route), matching the config/bindings auto-save
/// contract. The song-ended route is covered by `close_editor_on_exit`.
fn save_layout_on_close(open: Res<super::EditorOpen>, layouts: Res<WidgetLayouts>, lanes: Res<Lanes>) {
    if !open.is_changed() || open.0 {
        return;
    }
    let file = layout_file_from(&layouts, &lanes);
    if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
        warn!("layout auto-save failed: {e}");
    }
}
```
Register it in the `plugin` fn alongside `save_hotkey`:
```rust
    app.add_systems(
        Update,
        (save_hotkey, save_layout_on_close)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
```
Note: `save_hotkey` still needs its `super::editor_open` guard but `save_layout_on_close` must run when open is flipping false, so split them:
```rust
    app.add_systems(
        Update,
        (
            save_hotkey
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
            save_layout_on_close.run_if(in_state(game_shell::AppState::Performance)),
        ),
    );
```

- [ ] **Step 2: Save all three stores in `close_editor_on_exit` (song-ended route)**

In `crates/gameplay-drums/src/editor/mod.rs`, `close_editor_on_exit` currently restores autoplay/clears state but persists nothing. Add layout, config, and bindings saves guarded on `open.0` (only if the surface was still open). Add the needed params and save calls:
```rust
fn close_editor_on_exit(
    mut open: ResMut<EditorOpen>,
    prev: Res<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
    mut selection: ResMut<drag::Selection>,
    mut session: ResMut<game_shell::EditorSession>,
    layouts: Res<crate::widget_layout::WidgetLayouts>,
    lanes: Res<crate::lanes::Lanes>,
    draft: Res<tabs::ConfigDraft>,
    live_bindings: Res<crate::bindings::LiveBindings>,
) {
    if open.0 {
        // Persist all three stores on the song-ended-mid-edit route (no
        // in-Performance flip watcher fires here). Idempotent with the Esc-route
        // savers.
        let file = save::layout_file_from(&layouts, &lanes);
        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
            warn!("layout save on exit failed: {e}");
        }
        if let Err(e) = dtx_config::save(&dtx_config::default_path(), &draft.0) {
            warn!("config save on exit failed: {e}");
        }
        if let Err(e) =
            dtx_config::save_bindings(&dtx_config::default_bindings_path(), &live_bindings.0)
        {
            warn!("bindings save on exit failed: {e}");
        }
        autoplay.0 = prev.0;
        open.0 = false;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
    selection.0 = None;
    session.0 = false;
}
```

- [ ] **Step 3: Verify `save_bindings` signature + `LiveBindings.0` type**

Run: `grep -n "pub fn save_bindings\|pub struct LiveBindings\|struct BindingsFile\|pub fn save\b" crates/dtx-config/src/bindings.rs crates/gameplay-drums/src/bindings.rs`
Expected: confirm `save_bindings(path, &BindingsFile)` and that `LiveBindings.0` is a `BindingsFile` (matching how `save_bindings_on_close` in `gameplay-drums/src/bindings.rs` calls it). If the existing on-close saver uses a different call shape, copy THAT shape exactly.

- [ ] **Step 4: Build**

Run: `cargo build -p gameplay-drums`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git commit -am "fix(editor): auto-save layout (and config/bindings) on surface close"
```

---

## Task 5: Live-apply every setting

**Files:**
- Modify: `crates/gameplay-drums/src/lib.rs` (make `map_damage_level` `pub(crate)`)
- Modify: `crates/gameplay-drums/src/editor/tabs.rs` (expand `apply_draft_live`)
- Modify: `app/dtxmaniars-desktop/src/main.rs` (apply vsync to initial window present_mode)

- [ ] **Step 1: Expose `map_damage_level`**

In `crates/gameplay-drums/src/lib.rs`, find `fn map_damage_level(` and change it to `pub(crate) fn map_damage_level(`.

- [ ] **Step 2: Rewrite `apply_draft_live` to push the full draft**

Replace `apply_draft_live` in `crates/gameplay-drums/src/editor/tabs.rs` with the full mapping. Note the audio-block guard stays local (only re-touch BGM volume/stop when audio changed), but every other write runs each call:
```rust
fn apply_draft_live(
    draft: Res<ConfigDraft>,
    audio: Res<Audio>,
    chart: Res<crate::resources::ActiveChart>,
    mut scroll: ResMut<crate::resources::ScrollSettings>,
    mut input_offset: ResMut<crate::resources::InputOffsetMs>,
    mut bgm_adjust: ResMut<crate::resources::BgmAdjustState>,
    mut audio_settings: ResMut<crate::resources::DrumAudioSettings>,
    mut gauge: ResMut<crate::gauge::StageGauge>,
    mut show_perf_info: ResMut<crate::resources::ShowPerfInfo>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut show_timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut drum_cfg: ResMut<crate::resources::DrumGameplaySettings>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let g = &draft.0.gameplay;
    *scroll = crate::resources::ScrollSettings::from_scroll_speed(g.scroll_speed);
    scroll.play_speed = dtx_config::play_speed_multiplier(g.play_speed);
    input_offset.0 = g.input_offset_ms;
    bgm_adjust.common_ms = g.bgm_adjust_ms;
    gauge.damage_level = crate::map_damage_level(g.damage_level);
    show_timing_lines.0 = g.lane_display.shows_timing_lines();
    show_perf_info.0 = draft.0.system.show_perf_info;
    metronome_on.0 = draft.0.system.metronome;

    // Drums grouping/priority: writing config alone leaves the cached `groups`
    // stale — recompute against the current chart's chip presence.
    if drum_cfg.config != draft.0.drums {
        drum_cfg.config = draft.0.drums.clone();
        if let Some(c) = chart.chart.as_ref() {
            drum_cfg.rebuild_from_chart(c);
        }
    }
    polyphony.set_voices(draft.0.drums.polyphonic_sounds);

    // VSync: mutate the primary window's present_mode (framepace still paces
    // frame rate independently).
    if let Ok(mut window) = windows.single_mut() {
        let want = if draft.0.system.vsync {
            bevy::window::PresentMode::AutoVsync
        } else {
            bevy::window::PresentMode::AutoNoVsync
        };
        if window.present_mode != want {
            window.present_mode = want;
        }
    }

    let next_audio = crate::resources::DrumAudioSettings {
        bgm_enabled: draft.0.audio.bgm_enabled,
        drum_enabled: draft.0.audio.drum_sound_enabled,
        master_volume: draft.0.audio.master_volume,
        bgm_volume: draft.0.audio.bgm_volume,
        drum_volume: draft.0.audio.drum_volume,
    };
    if *audio_settings != next_audio {
        *audio_settings = next_audio;
        if audio_settings.bgm_enabled {
            dtx_audio::set_bgm_volume(&bgm, &mut instances, audio_settings.bgm_gain());
        } else {
            dtx_audio::stop_bgm(&audio, &mut bgm, &mut instances);
        }
    }
}
```

- [ ] **Step 3: Verify field/method names used above**

Run these and reconcile any mismatch (use the real names):
```
grep -n "pub struct ActiveChart\|pub chart\|chart:" crates/gameplay-drums/src/resources.rs | head
grep -n "rebuild_from_chart\|pub fn set_voices\|pub struct DrumPolyphony" crates/gameplay-drums/src/resources.rs crates/dtx-audio/src/*.rs
grep -n "pub struct StageGauge\|damage_level" crates/gameplay-drums/src/gauge.rs | head
grep -n "single_mut\|single(" crates/gameplay-drums/src/*.rs | head -3
```
Expected: `ActiveChart` holds the chart as an `Option<Chart>` (confirm the field name — the plan uses `chart.chart`; if it is `chart.0` or similar, adjust). `DrumGameplaySettings::rebuild_from_chart(&Chart)` exists (resources.rs:397). `DrumPolyphony::set_voices` exists. In Bevy 0.19 `Query::single_mut()` returns `Result`; if this codebase uses `.get_single_mut()`, use that.

- [ ] **Step 4: Apply vsync to the initial window in main.rs**

In `app/dtxmaniars-desktop/src/main.rs`, the config is already loaded for the boot summary. Load it (or reuse the loaded value) and set `present_mode` on the primary `Window`. Add `use bevy::window::PresentMode;` and, in the `Window { .. }` initializer, add:
```rust
                    present_mode: if dtx_config::load(&dtx_config::default_path()).system.vsync {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    },
```
(If a `Config` is already loaded into a local before the `App::new()` block, reuse that local instead of loading twice.)

- [ ] **Step 5: Build the app**

Run: `cargo build -p gameplay-drums && cargo build -p dtxmaniars-desktop`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(editor): apply all settings live while the surface is open"
```

---

## Task 6: Keyboard tab switching (PageUp/PageDown)

**Files:**
- Modify: `crates/game-shell/src/states.rs` (add `CustomizeTab::next`/`prev` + test)
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs` (handle PageUp/Down)
- Modify: `crates/gameplay-drums/src/editor/footer.rs` (legend text)

- [ ] **Step 1: Write failing test for `next`/`prev`**

Add to the `tests` module in `crates/game-shell/src/states.rs`:
```rust
    #[test]
    fn customize_tab_next_prev_cycle() {
        assert_eq!(CustomizeTab::Gameplay.next(), CustomizeTab::Audio);
        assert_eq!(CustomizeTab::Widgets.next(), CustomizeTab::Gameplay);
        assert_eq!(CustomizeTab::Gameplay.prev(), CustomizeTab::Widgets);
        assert_eq!(CustomizeTab::Audio.prev(), CustomizeTab::Gameplay);
    }
```

- [ ] **Step 2: Run it (fails to compile — methods missing)**

Run: `cargo test -p game-shell customize_tab_next_prev_cycle`
Expected: FAIL (no method `next`).

- [ ] **Step 3: Implement `next`/`prev`**

Add to `impl CustomizeTab` in `states.rs`:
```rust
    /// Next tab in rail order, wrapping.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Previous tab in rail order, wrapping.
    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
```

- [ ] **Step 4: Run test — passes**

Run: `cargo test -p game-shell customize_tab_next_prev_cycle`
Expected: PASS.

- [ ] **Step 5: Wire PageUp/PageDown in keyboard_nav.rs**

In `settings_keyboard_nav` (`crates/gameplay-drums/src/editor/keyboard_nav.rs`), the fn early-returns when `!active.0.is_settings()`. Tab switching must work on ALL tabs, so handle Page keys BEFORE that guard. At the top of the fn body (after reading keys, before the `is_settings` return), add:
```rust
    // Page keys cycle tabs on any surface tab (not just settings). Ignore while
    // Ctrl is held (reserved for perf hotkeys / save).
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        if keys.just_pressed(KeyCode::PageDown) {
            active_next = Some(active.0.next());
        } else if keys.just_pressed(KeyCode::PageUp) {
            active_next = Some(active.0.prev());
        }
    }
```
Because `active` is `Res` (read-only) in that system, change its param to `mut active: ResMut<super::tabs::ActiveTab>` and apply at the end:
```rust
    if let Some(next) = active_next {
        active.0 = next;
        return;
    }
```
Declare `let mut active_next: Option<game_shell::CustomizeTab> = None;` near the top. Ensure the existing `active.is_changed()` focus-reset still works (it will, since ActiveTab mutation next frame flags changed).

- [ ] **Step 6: Update the footer legend**

In `crates/gameplay-drums/src/editor/footer.rs`, change:
```rust
const LEGEND: &str = "↑↓ row   ←→ adjust   Tab peek   Ctrl+S save   Esc close";
```
to:
```rust
const LEGEND: &str = "↑↓ row   ←→ adjust (Shift=coarse)   PgUp/Dn tab   Tab peek   Ctrl+S save   Esc close";
```

- [ ] **Step 7: Build + full test**

Run: `cargo build -p gameplay-drums && cargo test -p game-shell`
Expected: success / all pass.

- [ ] **Step 8: Commit**

```bash
git commit -am "feat(editor): PageUp/PageDown switch Customize tabs"
```

---

## Task 7: Offset range widening + Shift-coarse

**Files:**
- Modify: `crates/dtx-config/src/lib.rs` (clamp constants + test)
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs` (offset step 1ms, slider step 1.0)
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs` (Shift = ×10)

- [ ] **Step 1: Widen the clamp constants + update the doc test range**

In `crates/dtx-config/src/lib.rs`:
```rust
pub const INPUT_OFFSET_CLAMP_MS: i32 = 300;
pub const BGM_ADJUST_CLAMP_MS: i32 = 300;
```

- [ ] **Step 2: Offset rows step 1 ms; sliders step 1.0**

In `crates/gameplay-drums/src/editor/settings_data.rs`, the "Input Offset" item: change the adjust delta from `10 * d` to `d`, and the slider `step: 10.0` to `step: 1.0`:
```rust
            adjust: |c, d| {
                c.gameplay.input_offset_ms = (c.gameplay.input_offset_ms + d).clamp(
                    -dtx_config::INPUT_OFFSET_CLAMP_MS,
                    dtx_config::INPUT_OFFSET_CLAMP_MS,
                );
            },
```
and its `SettingControl::Slider { step: 1.0, .. }`. Do the same for "BGM Offset" (`10 * d` → `d`, `step: 10.0` → `step: 1.0`).

- [ ] **Step 3: Shift = coarse (×10) in keyboard_nav.rs**

In `settings_keyboard_nav`, the ArrowLeft/ArrowRight branches call `(item.adjust)(&mut draft.0, ±1)`. Add a Shift multiplier by repeating the adjust 10× when Shift is held:
```rust
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let reps = if coarse { 10 } else { 1 };
    if keys.just_pressed(KeyCode::ArrowRight) {
        if let Some(item) = items.get(focused.0) {
            for _ in 0..reps {
                (item.adjust)(&mut draft.0, 1);
            }
        }
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        if let Some(item) = items.get(focused.0) {
            for _ in 0..reps {
                (item.adjust)(&mut draft.0, -1);
            }
        }
    }
```
(Replace the existing ArrowRight/ArrowLeft branches with these.)

- [ ] **Step 4: Update the existing offset behavior in the codebase if any test asserts ±99**

Run: `grep -rn "99\|INPUT_OFFSET_CLAMP\|BGM_ADJUST_CLAMP" crates/ | grep -i "offset\|clamp"`
Expected: no test asserts the literal `99` for offsets. If one does, update it to `300`.

- [ ] **Step 5: Build + test**

Run: `cargo build -p gameplay-drums && cargo test -p dtx-config -p gameplay-drums`
Expected: success / pass.

- [ ] **Step 6: Commit**

```bash
git commit -am "feat(editor): widen offset range to +/-300ms, 1ms fine / Shift coarse"
```

---

## Task 8: Bindings tab polish (port-name truncation + spatial chip legibility)

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs` (`port_display_label` truncation + NoWrap)
- Modify: `crates/gameplay-drums/src/editor/bindings_spatial.rs` (assess + fix overlay legibility)

- [ ] **Step 1: Truncate the port label + add a unit test**

In `crates/gameplay-drums/src/editor/bindings_panel.rs`, add a helper and use it inside `port_display_label` for the `Some(p)` arm:
```rust
/// Truncate a long device name to `max` chars with a trailing ellipsis.
fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
```
Change the `Some(p) => p.clone(),` arm to `Some(p) => truncate_label(p, 22),`.
Add a test in the `tests` module:
```rust
    #[test]
    fn truncate_label_adds_ellipsis() {
        assert_eq!(truncate_label("short", 22), "short");
        let long = "NUX NTK-61:NUX NTK-61 Midi 32:0";
        let out = truncate_label(long, 22);
        assert_eq!(out.chars().count(), 22);
        assert!(out.ends_with('…'));
    }
```

- [ ] **Step 2: NoWrap on the port label Text**

The port `Text` (the one with `max_width: Val::Px(150.0)`) should not wrap. Add `TextLayout::new_with_no_wrap()` (or the `TextLayout { linebreak: LineBreak::NoWrap, ..default() }` form confirmed in Task 3) to that `Text` spawn's component tuple.

- [ ] **Step 3: Read bindings_spatial.rs and assess the pad-overlay**

Run: `wc -l crates/gameplay-drums/src/editor/bindings_spatial.rs` then read it. The selected channel's bind chips render near the pad and clip/overlap. Fix legibility with the smallest change that works: cap the number of chips drawn per pad (e.g. first 3 then "+N"), or only render the overlay for the hovered/selected channel, or drop the spatial text overlay if it is redundant with the left-panel chips. Choose based on what the module does; implement the minimal fix. If the module's overlay is already gated to the selected channel and the clip is only cosmetic spacing, add a small background/padding so it is readable. Document the choice in the commit message.

- [ ] **Step 4: Build + test**

Run: `cargo build -p gameplay-drums && cargo test -p gameplay-drums truncate_label_adds_ellipsis`
Expected: success / pass.

- [ ] **Step 5: Commit**

```bash
git commit -am "fix(editor): truncate long MIDI port name; improve spatial bind legibility"
```

---

## Task 9: Tap-test calibration (highest risk — last)

**Files:**
- Create: `crates/gameplay-drums/src/editor/calibration.rs` (state, pure math, systems)
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (register `calibration::plugin`, add `pub mod calibration;`)
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (spawn a "Calibrate" button on the Gameplay tab header)
- Modify: `crates/gameplay-drums/src/editor/ui.rs` (Esc gating: do not close while calibrating)

- [ ] **Step 1: Write the pure-math module + failing tests**

Create `crates/gameplay-drums/src/editor/calibration.rs` with the pure functions first (state + systems added in later steps):
```rust
//! Input-offset tap-test calibration overlay for the Customize surface.
//!
//! While collecting, the metronome ticks on the beat and the player taps a pad.
//! Each hit's signed error to the nearest quarter-beat is sampled; the median
//! yields a suggested `input_offset_ms` (negative median → positive offset so
//! future hits land on the beat).

use bevy::prelude::*;

/// Signed error (ms) of `now_ms` to the nearest beat on a grid of `beat_ms`
/// spacing starting at `first_beat_ms`. Range (-beat/2, beat/2].
pub fn error_ms(now_ms: f64, beat_ms: f64, first_beat_ms: f64) -> f64 {
    if beat_ms <= 0.0 {
        return 0.0;
    }
    let rel = now_ms - first_beat_ms;
    let phase = rel.rem_euclid(beat_ms);
    if phase > beat_ms / 2.0 {
        phase - beat_ms
    } else {
        phase
    }
}

/// Median of a sample set (integer ms). Empty → 0.
pub fn median(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    v[v.len() / 2]
}

/// Suggested input offset from the median tap error: cancel the latency.
pub fn suggested_offset(median_err: i32, clamp: i32) -> i32 {
    (-median_err).clamp(-clamp, clamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_ms_nearest_beat_signed() {
        // beat=500ms, first=0. now=40 → +40 (late). now=470 → -30 (early).
        assert!((error_ms(40.0, 500.0, 0.0) - 40.0).abs() < 1e-6);
        assert!((error_ms(470.0, 500.0, 0.0) + 30.0).abs() < 1e-6);
        assert!((error_ms(1010.0, 500.0, 0.0) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn median_odd_and_empty() {
        assert_eq!(median(&[3, 1, 2]), 2);
        assert_eq!(median(&[]), 0);
    }

    #[test]
    fn suggested_offset_cancels_and_clamps() {
        assert_eq!(suggested_offset(40, 300), -40);
        assert_eq!(suggested_offset(-500, 300), 300);
    }
}
```

- [ ] **Step 2: Run the math tests (they must pass once the module compiles)**

First register the module: in `crates/gameplay-drums/src/editor/mod.rs` add `pub mod calibration;` to the module list.
Run: `cargo test -p gameplay-drums -- calibration::tests`
Expected: PASS (3 tests). If FAIL to compile, fix the module before continuing.

- [ ] **Step 3: Add the calibration state + plugin skeleton**

Append to `calibration.rs`:
```rust
/// Tap-test lifecycle. Idle by default.
#[derive(Resource, Default)]
pub enum CalibrationState {
    #[default]
    Idle,
    Collecting {
        samples: Vec<i32>,
        prev_metronome: bool,
        prev_timing_lines: bool,
    },
    Done {
        median: i32,
        prev_metronome: bool,
        prev_timing_lines: bool,
    },
}

/// How many taps before showing a suggestion.
pub const TARGET_SAMPLES: usize = 12;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CalibrationState>().add_systems(
        Update,
        (collect_taps, confirm_or_cancel)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open),
    );
}
```

- [ ] **Step 4: Implement tap collection**

Add `collect_taps`. It reads the beat grid from the active chart's base BPM and the raw gameplay clock, and samples on each drum hit. Confirm the event/clock types first:

Run: `grep -n "pub struct GameplayClock\|current_ms\|pub struct ActiveChart\|base_bpm\|LaneHit\|pub struct Chart\b" crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/*.rs | head`

Then implement (adjust field/event names to what the grep shows):
```rust
fn collect_taps(
    mut state: ResMut<CalibrationState>,
    clock: Res<crate::resources::GameplayClock>,
    chart: Res<crate::resources::ActiveChart>,
    mut hits: MessageReader<crate::LaneHit>,
    input_offset: Res<crate::resources::InputOffsetMs>,
) {
    let CalibrationState::Collecting { samples, .. } = &mut *state else {
        return;
    };
    let Some(now_raw) = clock.current_ms else { return };
    // Measure against the RAW clock: undo the currently-applied input offset so
    // the suggestion is absolute latency, not relative to the present setting.
    let now = now_raw as f64 - input_offset.0 as f64;
    let bpm = chart
        .chart
        .as_ref()
        .map(|c| c.base_bpm as f64)
        .unwrap_or(120.0);
    if bpm <= 0.0 {
        return;
    }
    let beat_ms = 60_000.0 / bpm;
    let mut got = false;
    for _ in hits.read() {
        let e = error_ms(now, beat_ms, 0.0);
        if e.abs() <= beat_ms / 2.0 {
            samples.push(e.round() as i32);
            got = true;
        }
    }
    if got && samples.len() >= TARGET_SAMPLES {
        let m = median(samples);
        if let CalibrationState::Collecting {
            prev_metronome,
            prev_timing_lines,
            ..
        } = *state
        {
            *state = CalibrationState::Done {
                median: m,
                prev_metronome,
                prev_timing_lines,
            };
        }
    }
}
```
NOTE: if `LaneHit` is a Bevy `Event`/`Message`, use the matching reader (`EventReader`/`MessageReader`). Confirm with the grep and the crate's existing readers (see `midi_consumer` in lib.rs for the exact type + reader used). Use the SAME reader type.

- [ ] **Step 5: Implement confirm/cancel + metronome forcing**

Add `confirm_or_cancel`, and the start logic. Start is triggered by the panel button (Task step 7) setting state to `Collecting` — but the metronome/timing-line forcing happens here on transition. Simpler: force the flags in the same system that starts collection. Put start + confirm together:
```rust
fn confirm_or_cancel(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CalibrationState>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
) {
    match &*state {
        CalibrationState::Idle => {}
        CalibrationState::Collecting {
            prev_metronome,
            prev_timing_lines,
            ..
        } => {
            if keys.just_pressed(KeyCode::Escape) {
                metronome_on.0 = *prev_metronome;
                timing_lines.0 = *prev_timing_lines;
                *state = CalibrationState::Idle;
            }
        }
        CalibrationState::Done {
            median,
            prev_metronome,
            prev_timing_lines,
        } => {
            if keys.just_pressed(KeyCode::Enter) {
                let off = suggested_offset(*median, dtx_config::INPUT_OFFSET_CLAMP_MS);
                draft.0.gameplay.input_offset_ms = off;
                metronome_on.0 = *prev_metronome;
                timing_lines.0 = *prev_timing_lines;
                *state = CalibrationState::Idle;
            } else if keys.just_pressed(KeyCode::Escape) {
                metronome_on.0 = *prev_metronome;
                timing_lines.0 = *prev_timing_lines;
                *state = CalibrationState::Idle;
            }
        }
    }
}

/// Called by the panel Calibrate button: enter Collecting, forcing the
/// metronome + timing lines on (the tick fires only on line crossings).
pub fn start_calibration(
    state: &mut CalibrationState,
    metronome_on: &mut crate::resources::MetronomeEnabled,
    timing_lines: &mut crate::resources::ShowTimingLines,
) {
    if !matches!(state, CalibrationState::Idle) {
        return;
    }
    *state = CalibrationState::Collecting {
        samples: Vec::new(),
        prev_metronome: metronome_on.0,
        prev_timing_lines: timing_lines.0,
    };
    metronome_on.0 = true;
    timing_lines.0 = true;
}
```

- [ ] **Step 6: Spawn the calibration overlay text (count / suggestion)**

Add a small overlay in `calibration.rs`: a system `render_overlay` that spawns/despawns a centered `Text` reflecting state. Keep it minimal — a marker component + a rebuild-on-change system:
```rust
#[derive(Component)]
struct CalibrationOverlay;

fn render_overlay(
    mut commands: Commands,
    state: Res<CalibrationState>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<CalibrationOverlay>>,
) {
    if !state.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    let msg = match &*state {
        CalibrationState::Idle => return,
        CalibrationState::Collecting { samples, .. } => {
            format!("Tap to the beat  ({}/{})", samples.len(), TARGET_SAMPLES)
        }
        CalibrationState::Done { median, .. } => {
            let off = suggested_offset(*median, dtx_config::INPUT_OFFSET_CLAMP_MS);
            format!("Suggested {off:+} ms   Enter apply · Esc cancel")
        }
    };
    commands.spawn((
        CalibrationOverlay,
        super::picking::EditorChrome,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            left: Val::Percent(35.0),
            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.95)),
        GlobalZIndex(crate::ui_z::EDITOR_CHROME + 1),
        children![(
            Text::new(msg),
            dtx_ui::theme::Theme::font(16.0),
            TextColor(theme.0.text_primary),
        )],
    ));
}
```
Register `render_overlay` in the plugin's system tuple, and add an `OnExit(Performance)` despawn like the other overlays.

- [ ] **Step 7: Add the Calibrate button to the Gameplay tab header**

In `crates/gameplay-drums/src/editor/panel.rs`, `spawn_settings_block` builds the header row (tab title + RESET TAB). Add a `CalibrateButton` marker component (define near `ResetTabButton`) and, only when `tab == CustomizeTab::Gameplay`, spawn a "Calibrate" button in that header row before RESET TAB:
```rust
            if tab == game_shell::CustomizeTab::Gameplay {
                h.spawn((
                    CalibrateButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        margin: UiRect::right(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.16, 0.24, 0.30)),
                    children![(
                        Text::new("Calibrate"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_primary),
                    )],
                ));
            }
```
Define the marker:
```rust
#[derive(Component)]
pub struct CalibrateButton;
```
Add a handler system (register in panel.rs's `editor_open` tuple):
```rust
fn handle_calibrate_button(
    q: Query<&Interaction, (With<CalibrateButton>, Changed<Interaction>)>,
    mut state: ResMut<super::calibration::CalibrationState>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        super::calibration::start_calibration(&mut state, &mut metronome_on, &mut timing_lines);
    }
}
```

- [ ] **Step 8: Gate Esc-to-close while calibrating**

In `crates/gameplay-drums/src/editor/ui.rs`, `close_on_escape` must NOT close the surface while calibration is active (Esc cancels calibration instead, handled in `confirm_or_cancel`). Add a run-condition or an early check:
```rust
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    calib: Res<super::calibration::CalibrationState>,
    // ...existing params...
) {
    if !matches!(*calib, super::calibration::CalibrationState::Idle) {
        return;
    }
    // ...existing body...
}
```
Ordering note: both `confirm_or_cancel` and `close_on_escape` read `just_pressed(Escape)` in the same frame. The `!Idle` guard in `close_on_escape` prevents the surface closing when Esc cancels calibration; `confirm_or_cancel` flips state to Idle for the NEXT frame, so there is no double-fire.

- [ ] **Step 9: Build + all tests**

Run: `cargo build -p gameplay-drums && cargo test -p gameplay-drums`
Expected: success / all pass (including calibration math tests).

- [ ] **Step 10: Commit**

```bash
git commit -am "feat(editor): input-offset tap-test calibration overlay"
```

---

## Task 10: Verification (BRP real-app pass) + wrap-up

**Files:** none (verification only). Uses `bevy-brp` MCP.

- [ ] **Step 1: Full workspace build + test**

Run: `cargo build && cargo test -p gameplay-drums -p dtx-config -p game-shell`
Expected: success; all tests pass.

- [ ] **Step 2: Launch + drive the surface**

Launch `dtxmaniars` (debug) via `brp_launch`. Wait ~6s. Screenshot the title. Send `F1`. Screenshot.
- Confirm coordinate mapping with ONE test click on a rail tab (screenshot to confirm the click landed) — scale varies per monitor (memory `brp-drive-customize`).

- [ ] **Step 3: Verify each fix visually**

- Drums tab: no value text wrapping onto the next row (Task 3).
- Footer: "Hover a setting for details." is visible, not hidden behind the rail (Task 2).
- PageDown/PageUp: send keys, screenshot — active tab changes across all 7 (Task 6).
- Gameplay tab: "Calibrate" button present; press it, screenshot the overlay ("Tap to the beat (0/12)"); Esc cancels (Task 9). (Real tap accuracy is not assertable headlessly.)
- Bindings tab: MIDI port name is single-line / truncated (Task 8).
- Toggle VSync off/on and a Drums grouping value, confirm no crash (live-apply, Task 5). Play Speed change: watch for gross visual desync; if it reads as broken mid-loop, note it (spec fallback: leave play_speed apply-on-re-enter).

- [ ] **Step 4: Data-loss regression check**

On the Lanes tab, cycle a preset or change a lane width, press Esc to close, re-open via F1 → Lanes; confirm the change persisted (Task 4). Shut down the app via `brp_shutdown`.

- [ ] **Step 5: Update memory**

Update `customize-surface-pillar` memory: settings overhaul shipped (auto-save, live-apply-all, calibration, PgUp/Dn tabs, dead config.rs removed) on `feat/settings-overhaul`.

- [ ] **Step 6: Final push**

```bash
git push -u origin feat/settings-overhaul
```

---

## Self-review notes
- Spec items 1–8 + micro-fixes each map to a task (1→T4, 2→T3, 3→T5, 4→T9, 5→T6, 6→T1, 7→T8, micro→T2, offset-range→T7). ✓
- play_speed live-apply carries the spec's documented fallback (verify visually; revert to apply-on-re-enter if broken). ✓
- Type/name reconciliation steps (grep-before-write) are included wherever the plan asserts a field/method name I could not fully confirm from the read files (ActiveChart field, LaneHit reader type, save_bindings shape, TextLayout API, single_mut vs get_single_mut). The executor MUST run those greps and use the real names.
