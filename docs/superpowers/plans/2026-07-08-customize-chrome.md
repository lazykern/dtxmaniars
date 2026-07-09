# Customize Chrome Restructure (Phase 4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Reshape the Customize surface chrome to match the approved prototype: a **tabs-only left rail**, a **left content panel** (per-tab: settings rows / bindings / lane list / widget list) with a **RESET TAB** button, a **right inspector** (Widgets-only, selected-widget knobs), a **topbar** (`CUSTOMIZE ▸ song · BPM` + entry hints + autoplay/loops chip), and a **footer** (hover description + key legend). Settings rows gain section groups + real sliders/steppers. `Bindings` moves to the KIT group.

**Architecture:** The rail (`ui.rs` `EditorUiRoot`) is stripped to tab buttons only. `panel.rs`'s single `PanelRoot` (today dual-positioned by `is_settings`) splits into two roots: `LeftContentRoot` (rebuilds on **tab change**, renders every tab's content) and `RightInspectorRoot` (rebuilds on **selection change**, renders only on Widgets+selection). The existing widget-knobs block becomes the inspector; the widget `Select` list migrates from the rail into the left panel. Topbar + footer are new full-width chrome nodes. All are `EditorChrome`-tagged (hidden during peek) and window-space (fixed frame; the stage shrinks between them via the 2b `StageRect` Fit preset).

**Tech Stack:** Rust, Bevy 0.19. Crates: `gameplay-drums` (editor chrome), `game-shell` (`CustomizeTab::Bindings`), `dtx-ui` (reuse `controls::` slider/stepper/toggle). Depends on Phase 2a/2b + the `7e38f2c` smoke fixes on `feat/customize-surface`.

**Spec/source of truth:** the prototype screenshots + `docs/superpowers/specs/2026-07-07-customize-surface-design.md` §4.1 (composition), §4.4 (chrome behaviors — footer/reset-tab/modified-dots/keyboard-nav; **search DROPPED**).

**Investigation anchors (all `crates/gameplay-drums/src/editor/` unless noted):**
- `ui.rs`: `EditorUiRoot`+`EditorChrome` root (`:55-113`, `left:0,width:220`); rail groups + `spawn_tab_button` (`:90-99,146-171`); widget `Select` list gated Widgets (`:100-105`); action buttons ResetAll/Undo/Redo/Save/Close (`:106-111`); `EditorButton` enum (`:18-26`); `handle_buttons` (`:186-251`); `plugin` (`:28-39`); `ui_needs_respawn` (`:50`).
- `panel.rs`: `PANEL_WIDTH=240` (`:88`), `RAIL_WIDTH=220` (`:92`); root dual-position `is_settings?left:right` (`:167-191`); rebuild trigger `resource_changed::<Selection|EditorOpen|Lanes|ActiveTab>` (`:101-106`) + debounce sig (`:144`); branches: `spawn_settings_block` (`:193`,`:549-628` flat `◂value▸`, desc never rendered), `spawn_lane_block` (`:197`,`:367-547`), widget-knobs inspector-content (`:205-364`: anchor grid `:217-281`, offset/scale/z, toggles, reset); `AnchorCell`/`apply_anchor_cells` (`:751-811`).
- `dtx-ui/src/widget/controls.rs`: `spawn_slider` (`:79`, draggable, `drive_sliders`), `spawn_stepper` (`:110`), `spawn_toggle` (`:153`); `ControlValue`/`ControlBool` changed-watched; `ControlsPlugin`.
- `resources.rs:32` `ActiveChart` → `.chart.metadata.title`/`.bpm` (pattern hud.rs:507); `autoplay.rs:22` `AutoplayEnabled`; `editor/session.rs` `EditorSession` (loops).
- `dtx-ui/src/theme.rs:15-35` tokens: `panel_bg, accent, text_primary/secondary, stage_panel_border(#444), select_yellow(#ffcc00), judgment_perfect(gold)`. NOTE editor uses hardcoded `srgb(0.14,0.14,0.18)` literals — route new UI through theme tokens where practical, but do not mass-refactor existing literals.
- `editor/settings_data.rs`: `SettingItem { label, value, adjust, desc }` — `desc` exists, unrendered.
- `game-shell/src/states.rs`: `CustomizeTab` enum, `ALL[6]`/`SETTINGS[4]`/`KIT[2]`, `label`, `is_settings`, partition test.

**Critical conventions:**
- NEVER `cargo fmt`/`--all`/`-p`. ONLY `rustfmt --edition 2021 <files you edited>` (all gameplay-drums/game-shell = 2021).
- Format-on-save DAEMON reorders imports (harmless). Before editing: `git -C <wt> checkout -- <file>` to start clean; before `git add`, `git status --short`, stage ONLY intended files.
- Worktree `/home/lazykern/lab/dtxmaniars-customize` (branch `feat/customize-surface`). HEAD `7e38f2c`.
- Bevy 0.19: `UiTransform`/`UiGlobalTransform`; `MessageWriter`; `windows.single()`→Result.
- New chrome nodes MUST be `EditorChrome`-tagged (peek hides them) + `GlobalZIndex(2000)` + window-space (do not read `StageRect` for chrome position — chrome is the fixed frame).
- Final task runs `cargo test --workspace` (schedule guard) + clippy.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `game-shell/src/states.rs` | Modify | `CustomizeTab::Bindings` in **KIT** group; `ALL`→7, `KIT`→3, `label`; partition test |
| `editor/panel.rs` | Modify | Split `PanelRoot`→`LeftContentRoot`(tab-change) + `RightInspectorRoot`(selection-change); widget list branch; widget-knobs→inspector; RESET TAB button; modified dots |
| `editor/ui.rs` | Modify | Rail → tabs only (strip widget list + action buttons); shrink to 132px; keep Undo/Redo/Save/ResetAll via hotkeys |
| `editor/hotkeys.rs` | Create | Ctrl+Z/Y (undo/redo), Ctrl+S (save layout), Ctrl+R? — the actions removed from the rail, now keyboard-only (Close=Esc already exists) |
| `editor/topbar.rs` | Create | Full-width topbar: `CUSTOMIZE ▸ title · BPM` + `F1@Gameplay F2@Widgets` + `AUTOPLAY · CHART LOOPS` chip |
| `editor/footer.rs` | Create | Full-width footer: hovered-row description (left) + key legend (right); `HoveredDesc` resource |
| `editor/settings_data.rs` | Modify | Add `group: &'static str` + `control: SettingControl{Slider{min,max,step}|Stepper}` to `SettingItem`; tag each row |
| `editor/mod.rs` | Modify | Register `hotkeys`/`topbar`/`footer` modules |
| `editor/keyboard_nav.rs` | Create | ↑↓ row focus, ←→ adjust focused settings row (tail task) |

---

### Task 1: `CustomizeTab::Bindings` in the KIT group

**Files:** Modify `game-shell/src/states.rs`.

Context: The prototype rail shows KIT = {Bindings, Lanes, Widgets}. `Bindings` uses the Fit stage preset (kit) so its spatial lane display shows the whole shrunk playfield. `is_settings()` stays FALSE for Bindings. The panel-left placement (from the `7e38f2c` fix) currently uses `is_settings()` to dock left — Bindings' content panel must ALSO dock left; handle that in Task 2 (the panel split makes ALL content dock left anyway, so this resolves itself).

- [ ] **Step 1: Update partition test**

```rust
#[test]
fn bindings_is_a_kit_tab() {
    assert!(!CustomizeTab::Bindings.is_settings());
    assert!(CustomizeTab::KIT.contains(&CustomizeTab::Bindings));
}
```
The existing `customize_tab_groups_partition_all_variants` passes automatically once Bindings is in `ALL` + `KIT`.

- [ ] **Step 2:** Run `cargo test -p game-shell bindings_is_a_kit_tab` — FAIL.

- [ ] **Step 3: Add variant**
- Enum: add `Bindings` before `Lanes`.
- `ALL: [_; 7]` — insert `Bindings` before `Lanes` (rail order: …System, Bindings, Lanes, Widgets).
- `KIT: [_; 3]` — `[Bindings, Lanes, Widgets]`.
- `label()`: `Bindings => "Bindings"`.
- `is_settings()` unchanged.

- [ ] **Step 4:** `cargo test -p game-shell` — PASS.
- [ ] **Step 5: Commit**
```bash
rustfmt --edition 2021 crates/game-shell/src/states.rs
git -C /home/lazykern/lab/dtxmaniars-customize add crates/game-shell/src/states.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(game-shell): CustomizeTab::Bindings in KIT group"
```

---

### Task 2: Split panel into LeftContentRoot + RightInspectorRoot

**Files:** Modify `editor/panel.rs`.

Context (**the crux, highest risk**): Today one `PanelRoot` rebuilds on `(selection, open, lanes, active)` and is positioned left (settings) or right (kit). Split into:
- **`LeftContentRoot`** — `left: Px(RAIL_WIDTH)` (right of the shrunk rail; RAIL_WIDTH updates to 132 in Task 3, keep the const reference), width ~348px, rebuilds on **tab change** (`ActiveTab`) + content-data change (Lanes for lane tab). Renders EVERY tab's content: settings block / lane block / **widget list** (migrated from rail) / (bindings block later). Includes the tab title + RESET TAB button at top.
- **`RightInspectorRoot`** — `right: 0`, width ~236px, rebuilds on **selection change**, renders ONLY when `ActiveTab == Widgets` AND a widget is selected: the existing widget-knobs block (anchor grid + offset/scale/z + toggles + reset). Hidden/despawned otherwise.

This kills the flicker where selecting a widget respawns the whole left list.

READ `panel.rs` fully first. Plan the split carefully.

- [ ] **Step 1: Two root markers + two rebuild systems**

Replace `PanelRoot` with `LeftContentRoot` + `RightInspectorRoot` (both `#[derive(Component)]`). Split `rebuild_panel` into:
- `rebuild_left_content` — trigger `resource_changed::<ActiveTab>.or(resource_changed::<EditorOpen>).or(resource_changed::<Lanes>)` (NOT `Selection`). Despawn old `LeftContentRoot`, spawn new at `left: Px(RAIL_WIDTH)`. Body: tab title + RESET TAB (Task 6 adds the button; here just the title), then branch on `active.0`: settings→`spawn_settings_block`; lanes→`spawn_lane_block`; widgets→**`spawn_widget_list`** (new — migrate the `Select(WidgetKind)` loop from ui.rs); bindings→placeholder (Phase 3a fills it) or empty.
- `rebuild_right_inspector` — trigger `resource_changed::<Selection>.or(resource_changed::<ActiveTab>).or(resource_changed::<EditorOpen>)`. Despawn old `RightInspectorRoot`. Spawn ONLY when `active.0 == Widgets && selection.0.is_some()` (and selected widget isn't Playfield → for Playfield keep today's lane-in-left behavior, or show nothing in inspector). Body: the existing widget-knobs block (anchor grid + offset/scale/z + toggles + reset), now parented under `RightInspectorRoot` at `right:0`.

Keep two separate `Local` debounce signatures. Update params: both systems need what they read (`active`, `open`, `selection`, `lanes`, `draft`, theme). `spawn_settings_block`/`spawn_lane_block` reused as-is (just re-parented).

- [ ] **Step 2: `spawn_widget_list`**

Extract the widget `Select(WidgetKind)` loop (currently ui.rs:100-105) into `spawn_widget_list(p, theme, selection)` that spawns one `EditorButton::Select(kind)` per `WidgetKind::ALL` with the selected one highlighted. Reuse the existing `Select` handling in `handle_buttons` (ui.rs) — the `EditorButton::Select` component + `handle_buttons` stay; only the SPAWN moves to the left panel. (ui.rs Task 3 removes the rail's copy.)

- [ ] **Step 3: Build + test**

`cargo test -p gameplay-drums` — PASS (schedule guard proves both new systems wire in). Existing panel tests: adapt any that reference `PanelRoot` by name.

- [ ] **Step 4: Commit**
```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): split panel into left-content + right-inspector roots"
```

---

### Task 3: Rail → tabs only (+ action hotkeys)

**Files:** Modify `editor/ui.rs`; Create `editor/hotkeys.rs`; Modify `editor/mod.rs`.

Context: Strip the rail to SETTINGS/KIT tab groups only. Remove the widget `Select` list (now in the left panel, Task 2) and the 5 action buttons. The removed actions need keyboard access: Undo=Ctrl+Z, Redo=Ctrl+Y, Save(layout)=Ctrl+S, Reset-All→becomes per-tab RESET TAB (Task 6); Close=Esc already exists (`close_on_escape`). Shrink rail width 220→132.

- [ ] **Step 1: `hotkeys.rs`** — port the Undo/Redo/Save action logic from `handle_buttons` into keyboard handlers:
```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, editor_action_hotkeys
        .run_if(in_state(game_shell::AppState::Performance))
        .run_if(super::editor_open));
}
fn editor_action_hotkeys(keys: Res<ButtonInput<KeyCode>>, /* UndoStack, Lanes, save deps — mirror handle_buttons */) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyZ) { /* undo */ }
    if ctrl && keys.just_pressed(KeyCode::KeyY) { /* redo */ }
    if ctrl && keys.just_pressed(KeyCode::KeyS) { /* save layout */ }
}
```
Copy the exact undo/redo/save bodies from `handle_buttons` (ui.rs:186-251). Ensure these don't conflict with the perf-hotkey Ctrl+arrows (different keys, fine) or the settings-tab typing.

- [ ] **Step 2: Strip `ui.rs`** — in `spawn_ui_on_open`: remove the widget-list block (`:100-105`) and the action-button block (`:106-111`) and their `"- widgets -"`/`"- actions -"` labels. Keep: `CUSTOMIZE` label (or move to topbar — keep a short rail header or drop), SETTINGS group + tabs, KIT group + tabs. Reduce root `width: Px(132.0)`. Trim `EditorButton` enum to just `Select` (still used by the migrated widget list) — keep `handle_buttons` for `Select` only; delete the ResetAll/Save/Undo/Redo/Close arms (logic moved to hotkeys + RESET TAB). Keep `close_on_escape`.

- [ ] **Step 3: Register hotkeys** in `mod.rs`. Update `RAIL_WIDTH` const (panel.rs + stage.rs both hardcode 220) → 132 everywhere (grep `220.0` / `RAIL_WIDTH`); update `stage.rs` `preset_rect` chrome math + `panel.rs` `LeftContentRoot` left offset accordingly (SETTINGS_LEFT_CHROME = 132 + panel width).

- [ ] **Step 4:** `cargo test -p gameplay-drums` — PASS.
- [ ] **Step 5: Commit**
```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/ui.rs crates/gameplay-drums/src/editor/hotkeys.rs crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/ui.rs crates/gameplay-drums/src/editor/hotkeys.rs crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/stage.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): rail is tabs-only; actions move to hotkeys"
```

---

### Task 4: Topbar

**Files:** Create `editor/topbar.rs`; Modify `editor/mod.rs`.

Context: Full-width top chrome bar: left = `CUSTOMIZE ▸ <song title> · BPM <n>`; right = `F1 @ Gameplay   F2 @ Widgets` + an `AUTOPLAY · CHART LOOPS` chip. Window-space, `EditorChrome`, spawned on surface open, despawned on close. Read `ActiveChart` for title/bpm.

- [ ] **Step 1:** `spawn_topbar_on_open` (run_if `ui_needs_respawn`-equivalent or just `resource_changed::<EditorOpen>`): absolute `top:0,left:0,width:100%,height:~40px`, dark bg, `EditorChrome`, `GlobalZIndex(2000)`. Children: `TopbarTitle` text (updated by a system reading `ActiveChart` changed → `CUSTOMIZE ▸ {title} · BPM {bpm:.0}`), and the static hint + chip on the right. Despawn on close / `OnExit(Performance)`.
- [ ] **Step 2:** `update_topbar_title` system (reads `Res<ActiveChart>`, writes the `TopbarTitle` text) gated `editor_open`.
- [ ] **Step 3:** register in `mod.rs`; `cargo test -p gameplay-drums` — PASS.
- [ ] **Step 4: Commit** `feat(gameplay-drums): Customize topbar (song · BPM · entry hints)`.

---

### Task 5: Footer (hover description + key legend)

**Files:** Create `editor/footer.rs`; Modify `editor/panel.rs` (emit hover desc), `editor/mod.rs`.

Context spec §4.4: full-width bottom bar; left = 1-2 line description of the hovered/focused row (from `SettingItem.desc`, currently unrendered); right = key legend `↑↓ row  ←→ adjust  Tab peek  Ctrl+S save  Esc close`.

- [ ] **Step 1:** `#[derive(Resource, Default)] struct HoveredDesc(pub String);`. `spawn_footer_on_open`: absolute `bottom:0,width:100%,height:~28px`, `EditorChrome`. Left text `FooterDesc` (updated from `HoveredDesc`), right static legend.
- [ ] **Step 2:** In `panel.rs`, when spawning settings rows, add an `Interaction`-carrying node per row tagged with its `desc`; a system `update_hovered_desc` sets `HoveredDesc.0` from the hovered row's desc. (Reuse `SettingRow` — add the desc string to it or a parallel component.) Update `FooterDesc` text from `HoveredDesc` changed.
- [ ] **Step 3:** register; `cargo test -p gameplay-drums` — PASS.
- [ ] **Step 4: Commit** `feat(gameplay-drums): Customize footer (hover desc + key legend)`.

---

### Task 6: RESET TAB button + settings groups + sliders

**Files:** Modify `editor/settings_data.rs`, `editor/panel.rs`.

Context: (a) RESET TAB button at the top-right of the left content panel — resets the active tab's values to default (settings tab → reset that tab's `ConfigDraft` fields; kit → existing reset). (b) Settings rows get section-group labels (FEEL/RULES per prototype) and use real sliders (continuous: scroll/offset/BGM/volumes) vs steppers (discrete: play-speed/damage/lane-display).

- [ ] **Step 1: Extend `SettingItem`** with `group: &'static str` and `control: SettingControl` where `enum SettingControl { Slider { min: f32, max: f32, step: f32 }, Stepper }`. Tag each row in the SYSTEM/GAMEPLAY/AUDIO/DRUMS tables (group names + control kind per the prototype: FEEL={scroll,input offset,BGM offset}, then play speed=Stepper, RULES={damage, lane display}=Stepper; audio volumes=Slider; etc.). Update the `all_tabs_have_rows` test if needed.
- [ ] **Step 2: Rebuild `spawn_settings_block`** to emit group-label rows and use `dtx_ui::widget::controls::spawn_slider`/`spawn_stepper` per `item.control`, wired to the existing `SettingAdjust`/value-refresh (or bridge slider `ControlValue` changes to `item.adjust`). Keep live-apply working (Fix 3 already applies ConfigDraft live).
- [ ] **Step 3: RESET TAB** — a `ResetTabButton` at the top of the left panel; handler resets the active tab: for settings, set each row's config field to `Config::default()`'s value (call `item.adjust` toward default, or re-load default for those fields); for kit tabs, call the existing reset. Confirm-on-click optional (spec says confirm — a simple second-press or skip for 3a; note if skipped).
- [ ] **Step 4:** `cargo test -p gameplay-drums` — PASS. **Step 5: Commit** `feat(gameplay-drums): settings groups + sliders + RESET TAB`.

---

### Task 7: Modified indicators (amber dot)

**Files:** Modify `editor/panel.rs`.

Context spec §4.4: amber dot on any settings row whose value ≠ default. Compare `(item.value)(&draft.0)` vs `(item.value)(&Config::default())`; if different, render a small `select_yellow` dot before the label.

- [ ] **Step 1:** in `spawn_settings_block`, per row compute `modified = (item.value)(&draft.0) != (item.value)(&Config::default())`; spawn a 6px dot node (theme `select_yellow`) when modified. **Step 2:** test/build PASS. **Step 3: Commit** `feat(gameplay-drums): modified-value dots on settings rows`.

---

### Task 8: Keyboard navigation

**Files:** Create `editor/keyboard_nav.rs`; Modify `editor/mod.rs`.

Context spec §4.4: ↑↓ moves the focused settings row; ←→ adjusts it. Only on settings tabs (don't clash with widget-drag arrow-nudge on kit tabs or perf-hotkey arrows — those are gated `editor_closed`, so safe while surface open, but be explicit).

- [ ] **Step 1:** `FocusedRow(usize)` resource; system (gated `editor_open` + `active.0.is_settings()`): ↑↓ moves focus (clamp to row count), ←→ calls `settings_items(active.0)[focused].adjust(&mut draft.0, ±1)`. Highlight the focused row. **Step 2:** build/test PASS. **Step 3: Commit** `feat(gameplay-drums): keyboard nav for settings rows`.

---

### Task 9: Full verification + manual smoke

- [ ] **Step 1:** `cargo test --workspace` — PASS (schedule guard).
- [ ] **Step 2:** `cargo clippy -p gameplay-drums --all-targets` — no new warnings in touched files.
- [ ] **Step 3: Manual smoke** (`cargo run -p dtxmaniars-desktop`): rail = tabs only (SETTINGS/KIT, Bindings under KIT); F1 → left content panel with grouped settings rows + sliders + RESET TAB, topbar shows `CUSTOMIZE ▸ song · BPM`, footer shows hover desc + legend; scroll-speed slider changes gameplay live; Widgets tab → widget list in left panel, selecting a widget opens the right inspector (anchor grid etc.) WITHOUT respawning the left list; ↑↓/←→ nav works; modified rows show amber dot; hold-Tab peek hides all chrome.
- [ ] **Step 4:** final fixups commit.

---

## Self-review notes

- **Prototype coverage:** rail=tabs → Task 3; left content panel (all tabs) → Task 2; right inspector (Widgets) → Task 2; topbar → Task 4; footer → Task 5; groups+sliders+RESET TAB → Task 6; dots → Task 7; keyboard nav → Task 8; Bindings in KIT → Task 1. Search = dropped (not built).
- **Biggest risk (Task 2):** the two-root split with distinct rebuild triggers — get the trigger sets right (left excludes `Selection`; right includes it) or you reintroduce the whole-panel-respawn-on-select flicker. The schedule guard + manual smoke (select a widget, watch the left list NOT flicker) verify it.
- **Repurposed:** widget-knobs block → inspector; `AnchorCell`/`apply_anchor_cells` unchanged; `controls::` slider/stepper/toggle for settings rows; `handle_buttons` Select arm survives (spawn relocated). **New:** topbar, footer, hotkeys, keyboard_nav, two-root split.
- **Deferred to Phase 3a:** the Bindings tab CONTENT (device box + channel list) mounts into the left-content `Bindings` branch this plan leaves as a placeholder.
- **Green-per-commit:** each task compiles + tests pass; chrome appears progressively.
```
