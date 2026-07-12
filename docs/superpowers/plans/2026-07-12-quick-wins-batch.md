# Quick-Wins Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the highest-value UX gaps from the 2026-07-12 audits: visible gauge, save visibility, wired hit/combo animations, legends, small factual bugs, Esc-twice quit, real search box.

**Architecture:** All changes are UI-side wiring or timing moves inside existing crates (`dtx-ui`, `dtx-layout`, `gameplay-drums`, `game-menu`, `game-results`). No new crates, no mechanics changes (ADR-0010: mechanics stay BocuD-ported). Spec: `docs/superpowers/specs/2026-07-12-quick-wins-batch-design.md`.

**Tech Stack:** Rust, Bevy 0.19 UI (`Node`/`Text`/`UiTransform`), hand-rolled `ScalarTween` (dtx-ui), bevy states.

**Repo rules (binding):** No `unwrap()` in `crates/*`. One commit per logical change. **Never add AI co-author trailers.** Inner loop: `cargo check -p <pkg>` + `cargo test -p <pkg> --lib`. Before final push: `cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings`.

---

### Task 1: Feedback wiring — `judgment_ok` token, popup scale, combo bounce, HitLine

**Files:**
- Modify: `crates/dtx-ui/src/theme.rs` (Theme struct ~line 14-35, Default ~37-61, `judgment_color` ~68-76)
- Modify: `crates/dtx-ui/src/widget/score_detailed.rs` (~line 122, Ok row color)
- Modify: `crates/gameplay-drums/src/hud.rs` (`sync_hud_judgment` ~365-402, `sync_perf_combo` ~527-541, HitLine spawn ~204)
- Test: `crates/dtx-ui/src/theme.rs` (inline mod tests)

- [ ] **Step 1: Write failing theme test**

Add to the `#[cfg(test)]` tests in `theme.rs` (create the mod if absent):

```rust
#[test]
fn judgment_color_maps_ok_and_poor() {
    let t = Theme::default();
    assert_eq!(t.judgment_color("POOR"), t.judgment_ok);
    assert_eq!(t.judgment_color("OK"), t.judgment_ok);
    assert_ne!(t.judgment_ok, t.text_primary);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dtx-ui --lib judgment_color_maps_ok_and_poor`
Expected: FAIL — `no field judgment_ok`.

- [ ] **Step 3: Add the token and mapping**

In `Theme` struct add `pub judgment_ok: Color,` after `judgment_good`. In `Default`:

```rust
judgment_ok: Color::srgb(0.75, 0.45, 0.95), // Ok/Poor purple (was score-panel local)
```

In `judgment_color`, before the `"MISS"` arm:

```rust
"POOR" | "OK" | "PO" => self.judgment_ok,
```

In `score_detailed.rs`, replace the local Ok purple literal (`Color::srgb(0.75, 0.45, 0.95)` at ~line 122) with `theme.judgment_ok` (match surrounding variable naming — the spawn fn takes `t: &Theme`, so `t.judgment_ok`).

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p dtx-ui --lib` — expected PASS (all).

- [ ] **Step 5: Apply popup scale in `sync_hud_judgment`**

`JudgmentPopup::tick` already returns `(alpha, scale)`; hud.rs discards scale (`hud.rs:395-397`). Change the decay loop to also write the scale onto the popup entity's `UiTransform`. Add `&mut bevy::ui::UiTransform` to the query (the popup spawn in `dtx-ui/src/widget/judgment_popup.rs:59-75` must also insert `bevy::ui::UiTransform::default()` into its bundle — add it there):

```rust
let delta = time.delta_secs() * 1000.0;
for (mut popup, _, mut color, mut vis, mut transform) in &mut q {
    let (alpha, scale) = popup.tick(delta);
    color.0 = color.0.with_alpha(alpha);
    transform.scale = Vec2::splat(scale);
    if !popup.is_active() && alpha <= 0.01 {
        *vis = Visibility::Hidden;
    }
}
```

**Note:** mirror the exact `UiTransform` scale-write idiom used by `beat_pulse_system` in `crates/dtx-ui/src/motion.rs` (`BeatPulse` scales a `UiTransform` every frame) — copy its field access verbatim if `scale` is not a plain `Vec2` in this Bevy version.

- [ ] **Step 6: Apply combo bounce in `sync_perf_combo`**

The number entity is spawned in `dtx-ui/src/widget/perf_combo.rs`; ensure the `PerfComboNumber` text entity bundle includes `bevy::ui::UiTransform::default()`. Then in `hud.rs:527-541`:

```rust
fn sync_perf_combo(
    combo: Res<Combo>,
    time: Res<Time>,
    mut q: Query<(&mut ComboDisplay, &mut Text, &mut bevy::ui::UiTransform), With<perf_combo::PerfComboNumber>>,
) {
    if !combo.is_changed() && !time.is_changed() {
        return;
    }
    let delta = time.delta_secs() * 1000.0;
    for (mut display, mut text, mut transform) in &mut q {
        display.set_combo(combo.current);
        display.tick(delta);
        transform.scale = Vec2::splat(display.scale());
        *text = Text::new(format!("{}", display.last_combo));
    }
}
```

(Same `UiTransform` idiom note as Step 5.)

- [ ] **Step 7: HitLine 3px at spawn**

`hud.rs:204`: change `height: Val::Px(4.0 * layout.scale)` to `height: Val::Px(3.0 * layout.scale)` (sync at `hud.rs:295` already uses 3.0).

- [ ] **Step 8: Check + test**

Run: `cargo check -p dtx-ui -p gameplay-drums && cargo test -p dtx-ui --lib && cargo test -p gameplay-drums --lib`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/dtx-ui crates/gameplay-drums
git commit -m "feat(hud): wire combo bounce + popup scale, add judgment_ok token, fix hit-line height"
```

---

### Task 2: Loading screen difficulty chip uses real tier color

**Files:**
- Modify: `crates/game-menu/src/song_loading.rs` (~line 419 `t.difficulty_color(2)`, and the spawn fn's metadata block above it)

- [ ] **Step 1: Find the tier index**

The spawn fn already has `difficulty` (label string) and `dlevel` in scope (song_loading.rs ~380-433). Locate where they are derived (top of the spawn/system, from the selected chart) — the same source exposes the selected difficulty slot index. If only the label exists, derive the index where the label is derived, using the same slot data (`Selection.difficulty` / the chart's difficulty slot), and thread it into the UI block as `difficulty_index: u8`.

- [ ] **Step 2: Use it**

Replace `BackgroundColor(t.difficulty_color(2))` (song_loading.rs:419) with `BackgroundColor(t.difficulty_color(difficulty_index))`.

- [ ] **Step 3: Check**

Run: `cargo check -p game-menu && cargo test -p game-menu --lib`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu
git commit -m "fix(loading): difficulty chip uses selected tier color, not hardcoded EXTREME"
```

---

### Task 3: Import toast outcome colors

**Files:**
- Modify: `crates/game-menu/src/import_ui.rs` (ImportToast ~47-51, poll_imports ~127-172, spawn_toast_node ~210-231, update_toast ~239-256)
- Test: inline tests in `import_ui.rs` (existing `mod tests` at ~258)

- [ ] **Step 1: Write failing tone test**

```rust
#[test]
fn toast_tone_by_outcome() {
    assert_eq!(ToastTone::Success.is_error(), false);
    let t = dtx_ui::theme::Theme::default();
    assert_eq!(ToastTone::Success.color(&t), t.clear_green);
    assert_eq!(ToastTone::Warn.color(&t), t.select_yellow);
    assert_eq!(ToastTone::Error.color(&t), t.judgment_miss);
}
```

Run: `cargo test -p game-menu --lib toast_tone_by_outcome` — expected FAIL (no `ToastTone`).

- [ ] **Step 2: Implement tone + per-line children**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastTone {
    Success,
    Warn,
    Error,
}

impl ToastTone {
    fn color(self, t: &Theme) -> Color {
        match self {
            ToastTone::Success => t.clear_green,
            ToastTone::Warn => t.select_yellow,
            ToastTone::Error => t.judgment_miss,
        }
    }
    #[cfg(test)]
    fn is_error(self) -> bool {
        self == ToastTone::Error
    }
}
```

Change `ImportToast { lines: Vec<String>, .. }` to `lines: Vec<(String, ToastTone)>`. In `poll_imports` assign tones:
- `Ok(..)` → `Success`
- `AlreadyImported` → `Warn`
- `UnsupportedFormat` / `NoCharts` / `UnsafePath` / `Io` → `Error`

Change `spawn_toast_node`: the `ToastNode` becomes a column container (`flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0)`) — remove `Text`/`TextColor` from it, keep bg/z/visibility. Change `update_toast` to rebuild children when lines changed:

```rust
fn update_toast(
    mut commands: Commands,
    mut toast: ResMut<ImportToast>,
    theme: Res<ThemeResource>,
    time: Res<Time>,
    mut nodes: Query<(Entity, &mut Visibility), With<ToastNode>>,
) {
    let expired = time.elapsed_secs_f64() > toast.expires;
    if expired && !toast.lines.is_empty() {
        toast.lines.clear();
    }
    if !toast.is_changed() {
        return;
    }
    let t = theme.0;
    for (entity, mut visibility) in &mut nodes {
        commands.entity(entity).despawn_related::<Children>();
        if toast.lines.is_empty() {
            *visibility = Visibility::Hidden;
        } else {
            *visibility = Visibility::Visible;
            commands.entity(entity).with_children(|col| {
                for (line, tone) in &toast.lines {
                    col.spawn((Text::new(line.clone()), Theme::font(16.0), TextColor(tone.color(&t))));
                }
            });
        }
    }
}
```

(If `despawn_related::<Children>()` is not the current Bevy API in this repo, use the child-despawn idiom already used by `practice/toast.rs`, which rebuilds its toast column each change — copy that.)

- [ ] **Step 3: Test + check**

Run: `cargo test -p game-menu --lib && cargo check -p game-menu`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu
git commit -m "feat(import): color-code import toast outcomes (success/duplicate/error)"
```

---

### Task 4: Album art — single driver via crossfade

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (`update_album_art_image` ~1787-1816)
- Read first: `crates/dtx-ui/src/widget/album_art.rs` (PreviewSwapEvent handler ~59-112)

- [ ] **Step 1: Read `album_art.rs` fully** and identify the `PreviewSwapEvent` payload and what its handler drives (image handle + fade alphas). The bug: `update_album_art_image` writes `ImageNode`/`BackgroundColor` directly on selection change, racing the crossfade.

- [ ] **Step 2: Route through the event**

Rewrite `update_album_art_image` to only emit the swap event (keeping its `Selection.is_changed()` gate and preimage-path resolution) and let `album_art.rs` own image + placeholder alpha. If the crossfade handler does not currently handle the "no art → placeholder at 0.18 alpha" case, add that to `album_art.rs` (fade to placeholder instead of to a new image) rather than keeping a second writer. Preserve exact current end-state alphas: image 1.0/bg 0.0 with art; image 0.0/bg 0.18 without.

- [ ] **Step 3: Check + existing tests**

Run: `cargo check -p game-menu -p dtx-ui && cargo test -p game-menu --lib && cargo test -p dtx-ui --lib`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu crates/dtx-ui
git commit -m "fix(song-select): album art driven only by crossfade, drop racing direct write"
```

---

### Task 5: Controls — segment-scoped reset + stale capture comment

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs` (`reset_bindings` ~680-683, reset button UI ~307-370, `handle_bindings_reset` ~689+, test `reset_bindings_restores_all_defaults` ~1057-1067)
- Modify: `crates/gameplay-drums/src/editor/capture_modal.rs` (stale comment ~161-163)

- [ ] **Step 1: Write failing segment-reset tests** (replace `reset_bindings_restores_all_defaults`)

`segment_matches(segment, source)` already exists (bindings_panel.rs:199-204). New tests:

```rust
#[test]
fn keyboard_reset_keeps_midi_map_and_device() {
    let mut live = LiveBindings(dtx_input::InputBindings::default());
    let mut rev = BindingsRev(0);
    live.0.midi.velocity_threshold = 64;
    // Remove every keyboard source from one channel to create drift.
    for sources in live.0.map.values_mut() {
        sources.retain(|s| !matches!(s, dtx_input::BindSource::Key(_)));
    }
    reset_segment(&mut live, &mut rev, ControlsSegment::Keyboard);
    let defaults = dtx_input::InputBindings::default();
    // Keyboard sources restored…
    for (ch, def_sources) in &defaults.map {
        for s in def_sources.iter().filter(|s| matches!(s, dtx_input::BindSource::Key(_))) {
            assert!(live.0.map.get(ch).is_some_and(|v| v.contains(s)), "{ch:?} missing {s:?}");
        }
    }
    // …device fields untouched.
    assert_eq!(live.0.midi.velocity_threshold, 64);
}

#[test]
fn midi_reset_keeps_keyboard_map_and_resets_device() {
    let mut live = LiveBindings(dtx_input::InputBindings::default());
    let mut rev = BindingsRev(0);
    live.0.midi.velocity_threshold = 64;
    reset_segment(&mut live, &mut rev, ControlsSegment::Midi);
    assert_eq!(live.0.midi.velocity_threshold, dtx_input::InputBindings::default().midi.velocity_threshold);
    // Keyboard defaults still present (they were untouched).
    let defaults = dtx_input::InputBindings::default();
    for (ch, def_sources) in &defaults.map {
        for s in def_sources.iter().filter(|s| matches!(s, dtx_input::BindSource::Key(_))) {
            assert!(live.0.map.get(ch).is_some_and(|v| v.contains(s)));
        }
    }
}
```

Run: `cargo test -p gameplay-drums --lib segment_reset` — expected FAIL (`reset_segment` undefined). (Adjust `MidiDeviceConfig` field assertions to actual field names when compiling.)

- [ ] **Step 2: Implement `reset_segment`** (replaces `reset_bindings`)

```rust
/// Reset only the active segment: that segment's sources return to defaults,
/// the other segment's sources are untouched. MIDI reset also restores the
/// device fields (port, velocity threshold).
fn reset_segment(live: &mut LiveBindings, rev: &mut BindingsRev, segment: ControlsSegment) {
    let defaults = dtx_input::InputBindings::default();
    let channels: std::collections::HashSet<_> =
        live.0.map.keys().chain(defaults.map.keys()).copied().collect();
    let mut map = std::collections::HashMap::new();
    for ch in channels {
        let mut sources: Vec<_> = live
            .0
            .map
            .get(&ch)
            .into_iter()
            .flatten()
            .filter(|s| !segment_matches(segment, s))
            .cloned()
            .collect();
        sources.extend(
            defaults
                .map
                .get(&ch)
                .into_iter()
                .flatten()
                .filter(|s| segment_matches(segment, s))
                .cloned(),
        );
        if !sources.is_empty() {
            map.insert(ch, sources);
        }
    }
    live.0.map = map;
    if segment == ControlsSegment::Midi {
        live.0.midi = defaults.midi;
    }
    rev.0 = rev.0.wrapping_add(1);
}
```

Update `handle_bindings_reset` to read the active segment (the same state the `SegmentBtn`/`handle_segment_btn` toggle writes — a `ControlsSegment` in `ControlsFocus` or its own resource; follow `segment_rows` callers) and call `reset_segment(..., active_segment)`.

- [ ] **Step 3: Relabel the button per segment**

Where the reset button label is spawned/updated (~bindings_panel.rs:307-370), render `"Reset keyboard"` / `"Reset MIDI"` from the active segment, and make the confirm row text enumerate scope: `"reset keyboard bindings to defaults?"` / `"reset MIDI bindings, port and threshold to defaults?"`. The label must update when the segment toggles (do it in the same system that re-renders segment-dependent UI).

- [ ] **Step 4: Fix stale comment**

`capture_modal.rs:161-163` claims below-threshold hits "never reach the capture machine". Replace with:

```rust
// Below-threshold hits still reach capture (velocity > 0 is the only gate;
// the threshold blocks gameplay dispatch, not learning). Shown muted so the
// player can diagnose soft pads while still being able to bind them.
```

- [ ] **Step 5: Editor save-failure surfacing (spec §2.2)**

Files: `crates/gameplay-drums/src/editor/footer.rs` (~24-127), `crates/gameplay-drums/src/editor/tabs.rs` (~72-74), `crates/gameplay-drums/src/editor/save.rs` (~59, 74), `crates/gameplay-drums/src/editor/mod.rs` (~194-196).

New resource + writer:

```rust
/// Transient save-failure banner shown in the footer's description slot.
#[derive(Resource, Default)]
pub struct EditorSaveError {
    pub message: Option<String>,
    pub until_secs: f64,
}

impl EditorSaveError {
    pub fn set(&mut self, now: f64, message: impl Into<String>) {
        self.message = Some(message.into());
        self.until_secs = now + 4.0;
    }
}
```

- Register `init_resource::<EditorSaveError>()` in the editor plugin.
- Every failing save path that today only logs — settings draft (`tabs.rs:72-74`, `mod.rs:194-196`), widget layout (`save.rs:59, 74`) — additionally calls `err.set(time.elapsed_secs_f64(), format!("save failed: {e}"))` (add `time: Res<Time>` + `ResMut<EditorSaveError>` params; keep the existing `warn!`/`error!`).
- In the footer render system (`footer.rs`, where the hover-desc text is written): when `err.message.is_some() && time.elapsed_secs_f64() < err.until_secs`, show the message in `chrome::ERR` red instead of the hover desc; clear `err.message` after expiry and restore the normal desc color. Capture-armed overrides (footer.rs:106-127) keep priority over the error line.

Note: caught-on-close settings failures despawn the editor with the state — the results/dirty-dialog paths already cover profile failures; this banner primarily serves Ctrl+S and in-editor writes. That asymmetry is acceptable for this batch.

- [ ] **Step 6: Test + check**

Run: `cargo test -p gameplay-drums --lib && cargo check -p gameplay-drums`
Expected: PASS (including the two new tests; old full-reset test removed).

- [ ] **Step 7: Commit**

```bash
git add crates/gameplay-drums
git commit -m "fix(controls): segment-scoped reset, save-failure footer banner, stale comment"
```

---

### Task 6: Result save on entry + save-status line

**Files:**
- Modify: `crates/game-results/src/lib.rs` (plugin ~43-50, spawn_result ~116-263, save_result_then_despawn ~297-377, tests)

- [ ] **Step 1: Restructure systems**

- New resource:

```rust
/// Outcome of the on-entry persistence attempt, shown as the last stat row.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
enum SaveStatus {
    #[default]
    Practice, // nothing to save
    Saved,
    Failed,
}
```

- Split `save_result_then_despawn` into `save_result` (persistence only, sets `SaveStatus`) and `despawn_result` (despawn only). Registration:

```rust
app.init_resource::<SaveStatus>()
    .add_systems(OnEnter(AppState::Result), (save_result, spawn_result).chain())
    .add_systems(OnExit(AppState::Result), despawn_result)
```

`save_result` body = current persistence code with: practice guard sets `SaveStatus::Practice` and returns; `store.add(entry)` stays; set `*status = if store.save().is_ok() { SaveStatus::Saved } else { SaveStatus::Failed }` (keep the `warn!`); score.ini write failure also downgrades to `Failed` (keep its `warn!`). `despawn_result` = `despawn_stage::<ResultEntity>(commands, query)` only.

- [ ] **Step 2: Status row in `spawn_result`**

`spawn_result` gains `status: Res<SaveStatus>`. After the `"ESC / ENTER → Song Select"` row (delay `STAGGER_MS * 14.0`), append one more colored row (skip entirely for `Practice`). Colored rows need a color other than white, so spawn it directly (not via the white `stat_rows` vec):

```rust
let (label, color) = match *status {
    SaveStatus::Saved => ("saved ✓", t.clear_green),
    SaveStatus::Failed => ("save failed — score kept this session only", t.judgment_miss),
    SaveStatus::Practice => ("", Color::NONE),
};
if !label.is_empty() {
    let row = commands
        .spawn((
            StatRow { reveal_at_ms: STAGGER_MS * 15.0 },
            Text::new(label),
            Theme::label_font(),
            TextColor(color.with_alpha(0.0)),
        ))
        .id();
    commands.entity(inner).add_child(row);
}
```

(`animate_staggered_reveal` only touches alpha, so the color survives the reveal.)

- [ ] **Step 3: Write the ordering test**

Add a Bevy `App` test proving persistence happens on entry (pattern: build a minimal `App` with the plugin's resources; if the crate has no existing App-test harness for states, instead unit-test the extracted pure part):

Minimum acceptable test — extract the entry construction already covered, plus:

```rust
#[test]
fn save_status_defaults_to_practice() {
    assert_eq!(SaveStatus::default(), SaveStatus::Practice);
}
```

and manually verify entry-save via the BRP smoke drive in Task 11. If a states-driven App test is feasible in this crate (check existing tests in `game-shell` for a pattern), prefer: insert resources → `OnEnter(Result)` → assert `store` has one entry before any exit.

- [ ] **Step 4: Test + check**

Run: `cargo test -p game-results && cargo check -p game-results`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/game-results
git commit -m "feat(results): persist on entry and show saved/failed status line"
```

---

### Task 7: Gauge widget

**Files:**
- Modify: `crates/dtx-layout/src/widgets.rs` (WidgetKind ~79-121, ALL array)
- Modify: `crates/dtx-layout/src/scene.rs` (`default_instance` ~17-38)
- Modify: `crates/dtx-ui/src/widget/gauge_bar.rs` (spawn fn + danger coloring support)
- Modify: `crates/gameplay-drums/src/hud.rs` (spawn + sync registration)
- Test: `crates/dtx-layout/src/widgets.rs`, `crates/dtx-ui/src/widget/gauge_bar.rs` inline tests

- [ ] **Step 1: Failing kind test**

In `dtx-layout/src/widgets.rs` tests:

```rust
#[test]
fn gauge_kind_serializes_kebab_and_is_listed() {
    assert!(WidgetKind::ALL.contains(&WidgetKind::Gauge));
    let s = toml::to_string(&std::collections::BTreeMap::from([("k", WidgetKind::Gauge)])).unwrap();
    assert_eq!(s.trim(), r#"k = "gauge""#);
}
```

Run: `cargo test -p dtx-layout --lib gauge_kind` — expected FAIL.

- [ ] **Step 2: Add the kind**

- `WidgetKind` gains `Gauge` variant; `ALL` becomes `[WidgetKind; 11]` with `WidgetKind::Gauge` appended; `display_name` returns `"Gauge"`.
- `scene.rs::default_instance`: add `WidgetKind::Gauge` to the `(true, false)` visibility arm (visible in play, hidden in practice — practice pins the gauge full).

Run: `cargo test -p dtx-layout --lib` — expected PASS. Then `cargo check --workspace` — fix every non-exhaustive `match` on `WidgetKind` the compiler reports (editor widget list picks it up automatically if it iterates `ALL`; the Widgets inspector needs no special case — Gauge is a normal non-Playfield widget).

- [ ] **Step 3: Gauge spawn fn in dtx-ui**

Add to `gauge_bar.rs` (existing `GaugeBarWidget`/`GaugeFill` reused; track spans the strip top):

```rust
#[derive(Component)]
pub struct GaugeThresholdTick;

/// Horizontal stage gauge across the top of the playfield strip.
/// `ref_x/ref_w` are ref-px strip bounds; `s` is layout scale.
pub fn spawn_stage_gauge(
    commands: &mut Commands,
    parent: Entity,
    theme: &crate::theme::Theme,
    s: f32,
    ref_x: f32,
    ref_w: f32,
) -> Entity {
    let track = commands
        .spawn((
            GaugeBarWidget {
                track_width: ref_w * s,
                ..Default::default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * s),
                top: Val::Px(64.0 * s),
                width: Val::Px(ref_w * s),
                height: Val::Px(10.0 * s),
                ..default()
            },
            BackgroundColor(theme.gauge_track),
        ))
        .id();
    let fill = commands
        .spawn((
            GaugeFill,
            Node {
                width: Val::Px(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme.gauge_fill),
        ))
        .id();
    commands.entity(track).add_child(fill);
    let tick = commands
        .spawn((
            GaugeThresholdTick,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(-2.0 * s),
                width: Val::Px(2.0 * s),
                height: Val::Px(14.0 * s),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        ))
        .id();
    commands.entity(track).add_child(tick);
    commands.entity(parent).add_child(track);
    track
}
```

Position note: `top: 64.0` ref-px sits just under the 60 ref-px frame-chrome speaker bar; the threshold tick marks displayed 0% (`StageGauge` fails at −0.1 internal, which displays as 0 via `pct()`), so place the tick at `left: 0` — its role is "empty = dead" clarity. Executor: verify against `frame_chrome.rs` actual bar height and adjust `top` so the gauge doesn't overlap chrome.

- [ ] **Step 4: Wire in hud.rs**

In `spawn_hud` after the FrameChrome block (~hud.rs:211-219):

```rust
let c_gauge = spawn_widget_container(&mut commands, root, WidgetKind::Gauge);
dtx_ui::widget::gauge_bar::spawn_stage_gauge(
    &mut commands,
    c_gauge,
    &t,
    s,
    layout.ref_strip_left(),
    layout.ref_strip_width(),
);
```

New sync system registered in the hud plugin's Update set (with the other `sync_*`):

```rust
fn sync_stage_gauge(
    gauge: Res<crate::gauge::StageGauge>,
    time: Res<Time>,
    mut bars: Query<&mut dtx_ui::widget::gauge_bar::GaugeBarWidget>,
    mut fills: Query<
        (&mut Node, &mut BackgroundColor),
        With<dtx_ui::widget::gauge_bar::GaugeFill>,
    >,
) {
    let delta = time.delta_secs() * 1000.0;
    for mut bar in &mut bars {
        bar.set_pct(gauge.pct());
        bar.tick(delta);
        for (mut node, mut color) in &mut fills {
            node.width = Val::Px(bar.fill_width());
            color.0 = crate::gauge::gauge_fill_color(gauge.value, gauge.failed);
        }
    }
}
```

(`gauge_fill_color` already encodes full-green / mid-yellow / danger-orange / failed-red — no new color logic.)

- [ ] **Step 5: Failing + passing widget test**

In `gauge_bar.rs` tests:

```rust
#[test]
fn stage_gauge_track_width_follows_ref_width() {
    let g = GaugeBarWidget { track_width: 558.0, pct: 50.0, ..Default::default() };
    assert!((g.fill_width() - 279.0).abs() < 0.1);
}
```

Run: `cargo test -p dtx-ui --lib && cargo test -p dtx-layout --lib && cargo check -p gameplay-drums`
Expected: PASS.

- [ ] **Step 6: Editor exposure sanity**

Grep the editor for exhaustive `WidgetKind` matches: `grep -rn "WidgetKind::" crates/gameplay-drums/src/editor/ crates/gameplay-drums/src/widget_layout.rs`. Anywhere kinds are matched exhaustively (natural-size table, drag rules, inspector), give Gauge sane entries (natural size ≈ strip width × 10 ref-px; draggable like other widgets; normal inspector). Compiler errors from Step 2 are the primary guide.

- [ ] **Step 7: Commit**

```bash
git add crates/dtx-layout crates/dtx-ui crates/gameplay-drums
git commit -m "feat(hud): render stage gauge above playfield as customizable widget"
```

---

### Task 8: Legends — practice quick keys, title pad hint, loading cancel hint

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/mini_strip.rs` (or the quick-tier spawn site in `practice/hud/mod.rs`)
- Modify: `crates/game-menu/src/title.rs` (spawn_title ~18-112)
- Modify: `crates/game-menu/src/song_loading.rs` (status column ~463-468)

- [ ] **Step 1: Practice quick-key legend**

Where the quick tier spawns (mini strip / status chip site in `practice/hud/`), add a bottom-center legend row above the 10px strip, always visible in practice quick tier, `GlobalZIndex(ui_z::PRACTICE)`:

```rust
// Keyboard legend for the quick tier — the bindings live in actions.rs and
// were previously discoverable only by reading the source.
parent
    .spawn((Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(16.0),
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        ..default()
    },))
    .with_children(|row| {
        dtx_ui::widget::nav_legend::spawn_nav_legend(
            row,
            &t,
            &[
                ("[ ]", "loop A/B"),
                ("Bksp", "clear"),
                ("-/=", "tempo"),
                ("R", "restart"),
                ("T", "ramp"),
                ("Tab", "menu"),
            ],
        );
    });
```

(`spawn_nav_legend` signature per `game-results/src/lib.rs:260`: `(&mut ChildSpawnerCommands, &Theme, &[(&str, &str)])` — match the call idiom at that site. It must despawn with the quick-tier HUD and must NOT show inside the full HUD — parent it to the quick-tier root entity.)

- [ ] **Step 2: Title pad legend**

In `spawn_title`, add `midi: Option<Res<game_shell::MidiConnected>>` param; after the PRESS ENTER chip:

```rust
if midi.is_some_and(|m| m.0) {
    root.spawn(Node {
        margin: UiRect::top(Val::Px(8.0)),
        ..default()
    })
    .with_children(|row| {
        dtx_ui::widget::nav_legend::spawn_nav_legend(row, &t, &[("BD", "start")]);
    });
}
```

**Caveat:** title spawns once on enter; MIDI connecting later won't retro-add the legend. Acceptable for this batch (song select already has a live-updating legend; copy that update-system pattern only if it is trivial — otherwise ship enter-time gating).

- [ ] **Step 3: Loading cancel hint**

In the loading hero card's text column (song_loading.rs ~463-468, next to `LoadingStatusText`), add:

```rust
col.spawn((
    Text::new("Esc — cancel"),
    Theme::font(12.0),
    TextColor(t.text_secondary),
));
```

- [ ] **Step 4: Check + commit**

Run: `cargo check -p gameplay-drums -p game-menu && cargo test -p gameplay-drums --lib && cargo test -p game-menu --lib`
Expected: PASS.

```bash
git add crates/gameplay-drums crates/game-menu
git commit -m "feat(ux): practice quick-key legend, title pad hint, loading cancel hint"
```

---

### Task 9: Esc-twice quit on title

**Files:**
- Modify: `crates/game-menu/src/title.rs` (title_input ~114-151, footer ESC QUIT text ~105-109)
- Test: inline `mod tests` in title.rs

- [ ] **Step 1: Failing state-machine test**

```rust
#[test]
fn quit_arm_fires_only_within_window() {
    let mut arm = QuitArm::default();
    assert!(!arm.press(0.0));   // first press arms
    assert!(arm.press(1.0));    // second press within 2s quits
    let mut arm2 = QuitArm::default();
    assert!(!arm2.press(0.0));
    assert!(!arm2.press(3.0));  // expired → re-arms instead of quitting
    assert!(arm2.press(3.5));
}
```

Run: `cargo test -p game-menu --lib quit_arm` — expected FAIL.

- [ ] **Step 2: Implement**

```rust
/// Esc-twice quit guard: first press arms a 2s window, second quits.
#[derive(Resource, Default)]
pub struct QuitArm {
    armed_at: Option<f64>,
}

const QUIT_ARM_SECS: f64 = 2.0;

impl QuitArm {
    /// Returns true when this press should quit.
    fn press(&mut self, now: f64) -> bool {
        match self.armed_at {
            Some(t) if now - t <= QUIT_ARM_SECS => true,
            _ => {
                self.armed_at = Some(now);
                false
            }
        }
    }
    fn disarm(&mut self) {
        self.armed_at = None;
    }
    fn is_armed(&self, now: f64) -> bool {
        self.armed_at.is_some_and(|t| now - t <= QUIT_ARM_SECS)
    }
}
```

Register `init_resource::<QuitArm>()` in the title plugin; reset on `OnEnter(AppState::Title)`. In `title_input` (needs `time: Res<Time>`, `arm: ResMut<QuitArm>`, and a query for the footer text — tag the `ESC QUIT` footer text with a `QuitHintText` component):

- `Escape` branch: `if arm.press(time.elapsed_secs_f64()) { request_transition(&mut requests, AppState::End); }`
- Every *other* handled input (Enter/pad/F1/F2) calls `arm.disarm()`.
- A small system (or the same one) updates the hint: armed → `"PRESS ESC AGAIN TO QUIT"` with `t.select_yellow`; not armed → `"ESC QUIT"` with `t.text_secondary`. Window expiry also reverts the text (`is_armed` checked per frame).

- [ ] **Step 3: Test + check + commit**

Run: `cargo test -p game-menu --lib && cargo check -p game-menu` — expected PASS.

```bash
git add crates/game-menu
git commit -m "feat(title): esc-twice quit guard with inline hint"
```

---

### Task 10: Search input box + Esc-clears-search

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (SearchText spawn ~786-791, `search_input` ~1745-1785, `song_select_kb_emit` ~1628-1655)
- Test: inline tests in song_select.rs

- [ ] **Step 1: Failing Esc-rule test**

The keyboard emitter decides Back vs clear. Extract the decision as a pure fn and test it:

```rust
/// Esc on song select: a non-empty search clears first; only an empty
/// search backs out to the title.
fn esc_clears_search_first(query: &str) -> bool {
    !query.is_empty()
}

#[test]
fn esc_clears_before_backing_out() {
    assert!(esc_clears_search_first("abc"));
    assert!(!esc_clears_search_first(""));
}
```

Run: `cargo test -p game-menu --lib esc_clears` — expected FAIL (fn undefined).

- [ ] **Step 2: Wire the rule**

`song_select_kb_emit` gains `mut selection_state: ResMut<SongSelectSelection>` and, in the `Escape` arm:

```rust
} else if keys.just_pressed(KeyCode::Escape) {
    if esc_clears_search_first(&selection_state.search_query) {
        selection_state.search_query.clear();
        selection_state.dirty = true;
        return;
    }
    NavVerb::Back
}
```

(Also refresh the search display — see Step 3's single render fn.)

- [ ] **Step 3: Real input-box UI**

Replace the bare `SearchText` spawn (~786-791) with a bordered field:

```rust
chips
    .spawn((
        SearchBox,
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            min_width: Val::Px(200.0),
            ..default()
        },
        BackgroundColor(t.stage_panel_bg),
        BorderColor::all(t.stage_panel_border),
    ))
    .with_children(|field| {
        field.spawn((Text::new("⌕"), Theme::font(13.0), TextColor(t.text_secondary)));
        field.spawn((
            SearchText,
            Text::new("type to search…"),
            Theme::font(13.0),
            TextColor(t.text_secondary),
        ));
    });
```

Add `#[derive(Component)] struct SearchBox;`. Single render helper used by `search_input`, the Esc-clear path, and screen-enter reset:

```rust
fn render_search(
    query: &str,
    theme: &Theme,
    text_q: &mut Query<(&mut Text, &mut TextColor), With<SearchText>>,
    box_q: &mut Query<&mut BorderColor, With<SearchBox>>,
) {
    let active = !query.is_empty();
    for (mut text, mut color) in text_q {
        *text = Text::new(if active {
            format!("{query}█")
        } else {
            "type to search…".to_string()
        });
        color.0 = if active { theme.text_primary } else { theme.text_secondary };
    }
    for mut border in box_q {
        *border = BorderColor::all(if active { theme.accent } else { theme.stage_panel_border });
    }
}
```

Update `search_input` to call `render_search` instead of its inline `"search: {q}"` write (drop that format — the field itself is now the affordance). Make sure the screen-enter query reset also routes through `render_search` so re-entering shows the placeholder.

- [ ] **Step 4: Test + check + commit**

Run: `cargo test -p game-menu --lib && cargo check -p game-menu` — expected PASS.

```bash
git add crates/game-menu
git commit -m "feat(song-select): real search input box; esc clears search before backing out"
```

---

### Task 11: Workspace gates + runtime verification + push

**Files:** none (verification only)

- [ ] **Step 1: Workspace gates**

Run: `cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. Fix anything (commit fixes with the task they belong to via `git commit --fixup` style or a `chore:` commit).

- [ ] **Step 2: Package tests for every changed crate**

Run: `cargo test -p dtx-ui -p dtx-layout -p game-menu -p game-results && cargo test -p gameplay-drums --lib`
Expected: PASS.

- [ ] **Step 3: Runtime smoke (BRP drive per memory note)**

Launch `cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking` and verify visually (screenshot each):
1. Title: pad legend (if MIDI connected), Esc once → amber hint, Esc again → quits. Relaunch.
2. Song select: search box visible; type → accent border + caret; Esc clears; Esc again → title.
3. Start a song: gauge visible above playfield, moves on hits/misses, turns orange/red near empty; combo visibly bounces; judgment popup grows as it fades; POOR popup is purple.
4. Loading: chip matches selected tier color; `Esc — cancel` hint shown.
5. Results: `saved ✓` line appears; new entry in history after returning.
6. Practice: quick-key legend visible at bottom; absent in full HUD.
7. Customize → Widgets: Gauge listed, selectable, movable, Reset Widget works; Controls: reset button reads `Reset keyboard`/`Reset MIDI` and only resets the visible segment.

- [ ] **Step 4: Push**

```bash
git push origin main
```

(No co-author trailers anywhere — repo rule.)
