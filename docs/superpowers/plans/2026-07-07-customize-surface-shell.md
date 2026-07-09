# Customize Surface Shell (Phase 2a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the F2 layout editor into a tabbed **Customize** surface that also hosts the settings (Gameplay/Audio/Drums/System), delete the standalone `AppState::Config` screen, and enter the surface from F1 (settings) or F2 (widgets).

**Architecture:** The existing editor overlay (`EditorOpen` + left sidebar `ui.rs` + right `panel.rs` block-swap) becomes the surface chrome. A new `ActiveTab` resource selects which right-panel block renders: settings-row blocks (new) for Gameplay/Audio/Drums/System, or the existing lane/widget blocks for Lanes/Widgets. Settings edit a `ConfigDraft` copied from `config.toml` on open and saved on close — identical persistence semantics to today's config screen, just relocated. Entry funnels through the existing `EditorSession(true)` → SongLoading → Performance path (used by F2 today); a `PendingCustomizeTab` carries the desired initial tab across the load.

**Tech Stack:** Rust, Bevy 0.19. Crates: `game-shell` (state + cross-crate resources), `gameplay-drums` (the surface), `game-menu` (F1/F2 entry, config screen being deleted), `dtx-config`.

**Spec:** `docs/superpowers/specs/2026-07-07-customize-surface-design.md` §4 (surface architecture) + §4.5 (entries/deletion). Depends on Phase 1 (input bindings backend) already on this branch.

**Explicitly DEFERRED (not this plan):**
- **Stage-transform presets** (Offset/Fit/Identity shrink + peek) — invasive `PlayfieldLayout`/`drag.rs` rework; its own plan. In 2a, settings tabs render over the full-window live game (panel floats over it, osu-settings style); Lanes/Widgets tabs keep today's exact behavior.
- **Live-apply of settings** — 2a preserves today's semantics: `ConfigDraft` edits persist on close and take effect on next song entry, exactly as the current config screen does. Live preview is a follow-up.
- **Bindings tab** — Phase 3 (needs the capture-flow UX + input un-gating).
- **Search box, per-row modified dots, footer description bar, reset-tab** — AAA polish, follow-up. (The `desc` field is ported now so the follow-up is cheap.)

**Critical conventions:**
- NEVER run `cargo fmt` / `cargo fmt --all` / `cargo fmt -p <crate>` — local rustfmt version drift reformats unrelated files and pollutes commits (this bit us in Phase 1). ONLY `rustfmt --edition <ed> <explicit files you edited>`. `dtx-config`/`gameplay-drums`/`game-shell` are edition 2021; `game-menu` is edition **2024** (uses let-chains) — format its files with `--edition 2024`.
- Before every `git add -A`, run `git -C <worktree> status --short` and confirm ONLY intended files appear. If unrelated files show modified, STOP.
- Work from the worktree at `/home/lazykern/lab/dtxmaniars-input-bindings` (branch `feat/input-bindings-backend`). Run all cargo/git with that cwd.
- Bevy 0.19: state changes flow through `game_shell::request_transition(&mut writer, AppState::X)` (a fade director), NOT raw `NextState`. UI nodes use `UiTransform`/`UiGlobalTransform`, not `GlobalTransform`.
- Green unit tests do NOT prove the plugin schedule builds — the final task runs the full workspace suite which includes the headless schedule guard tests.
- Per-task: `cargo test -p <crate>`; final: `cargo test --workspace`.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/game-shell/src/states.rs` | Modify | Add `CustomizeTab` enum + `PendingCustomizeTab(Option<CustomizeTab>)` resource (cross-crate, like `EditorSession`) |
| `crates/game-shell/src/lib.rs` | Modify | init `PendingCustomizeTab`; re-export `CustomizeTab` |
| `crates/gameplay-drums/src/editor/settings_data.rs` | Create | `SettingItem` table type + the 4 ported settings tables (System/Gameplay/Audio/Drums) + label/cycle helpers |
| `crates/gameplay-drums/src/editor/tabs.rs` | Create | `ActiveTab` resource, `ConfigDraft` resource, open/close load-save, `PendingCustomizeTab` consumption, tab-group metadata |
| `crates/gameplay-drums/src/editor/ui.rs` | Modify | Left sidebar → tab rail (SETTINGS/KIT groups) + widget list (Widgets tab only) + action buttons; tab-click sets `ActiveTab` |
| `crates/gameplay-drums/src/editor/panel.rs` | Modify | Rebuild keyed on `ActiveTab`; new settings-row block; Lanes/Widgets route to existing blocks; settings rows adjust `ConfigDraft` |
| `crates/gameplay-drums/src/editor/mod.rs` | Modify | Register `tabs`/`settings_data` modules; init resources |
| `crates/game-menu/src/title.rs` | Modify | F1 → set `PendingCustomizeTab(Gameplay)` + session + SongLoading; add "F1 SETTINGS" hint |
| `crates/game-menu/src/song_select.rs` | Modify | F1 → customize-session (was `→Config`); update hint |
| `crates/game-menu/src/config.rs` | Delete | Standalone config screen (tables ported) |
| `crates/game-menu/src/lib.rs` | Modify | Drop `mod config;` + its plugin registration |
| `crates/game-shell/src/states.rs` | Modify | Delete `AppState::Config` variant |

---

### Task 1: `CustomizeTab` + `PendingCustomizeTab` in game-shell

**Files:**
- Modify: `crates/game-shell/src/states.rs`
- Modify: `crates/game-shell/src/lib.rs`

Context: `EditorSession(pub bool)` already lives in `states.rs` (states.rs:108-109) specifically so `game-menu` can set cross-crate coordination flags that `gameplay-drums` reads. `CustomizeTab` + `PendingCustomizeTab` follow that exact precedent. `AppState` is defined at states.rs:10-35.

- [ ] **Step 1: Write failing tests**

Append to the `#[cfg(test)] mod tests` in `states.rs` (create if absent):

```rust
#[test]
fn customize_tab_groups_partition_all_variants() {
    let settings = CustomizeTab::SETTINGS;
    let kit = CustomizeTab::KIT;
    assert_eq!(settings.len() + kit.len(), CustomizeTab::ALL.len());
    for t in CustomizeTab::ALL {
        assert!(settings.contains(&t) ^ kit.contains(&t), "{t:?} must be in exactly one group");
    }
}

#[test]
fn pending_customize_tab_defaults_none() {
    assert_eq!(PendingCustomizeTab::default().0, None);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p game-shell customize`
Expected: FAIL — `CustomizeTab` not found.

- [ ] **Step 3: Implement**

Add to `states.rs` (near `EditorSession`):

```rust
/// Which Customize-surface tab is active. SETTINGS group edits `config.toml`;
/// KIT group edits the layout (lanes/widgets). Bindings tab lands in Phase 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomizeTab {
    Gameplay,
    Audio,
    Drums,
    System,
    Lanes,
    Widgets,
}

impl CustomizeTab {
    /// All tabs in rail order.
    pub const ALL: [CustomizeTab; 6] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
        CustomizeTab::Lanes,
        CustomizeTab::Widgets,
    ];
    /// Settings group (edits config.toml).
    pub const SETTINGS: [CustomizeTab; 4] = [
        CustomizeTab::Gameplay,
        CustomizeTab::Audio,
        CustomizeTab::Drums,
        CustomizeTab::System,
    ];
    /// Kit group (edits layout.toml).
    pub const KIT: [CustomizeTab; 2] = [CustomizeTab::Lanes, CustomizeTab::Widgets];

    /// Short rail label.
    pub fn label(self) -> &'static str {
        match self {
            CustomizeTab::Gameplay => "Gameplay",
            CustomizeTab::Audio => "Audio",
            CustomizeTab::Drums => "Drums",
            CustomizeTab::System => "System",
            CustomizeTab::Lanes => "Lanes",
            CustomizeTab::Widgets => "Widgets",
        }
    }

    /// True if this tab edits `config.toml` (vs the layout).
    pub fn is_settings(self) -> bool {
        Self::SETTINGS.contains(&self)
    }
}

/// Initial Customize tab to open, set by the entry key (F1/F2) before the
/// SongLoading→Performance transition and consumed when the surface opens.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PendingCustomizeTab(pub Option<CustomizeTab>);
```

(Note: `SETTINGS`/`KIT` are arrays; the test calls `.len()`/`.contains()` on them, which array refs support. `contains` needs `&CustomizeTab` args — the test passes `&t`, and `CustomizeTab: PartialEq` derived above satisfies it.)

- [ ] **Step 4: Register the resource + re-export**

In `crates/game-shell/src/lib.rs`, wherever states/resources are registered (find with `rg -n "EditorSession|init_resource|pub use" crates/game-shell/src/lib.rs`): add `.init_resource::<states::PendingCustomizeTab>()` alongside the other resource inits, and add `CustomizeTab, PendingCustomizeTab` to the `pub use states::{...}` re-export list.

- [ ] **Step 5: Run tests**

Run: `cargo test -p game-shell`
Expected: PASS.

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/game-shell/src/states.rs crates/game-shell/src/lib.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/game-shell
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(game-shell): CustomizeTab enum + PendingCustomizeTab resource"
```

---

### Task 2: Port settings tables into gameplay-drums

**Files:**
- Create: `crates/gameplay-drums/src/editor/settings_data.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (add `pub mod settings_data;`)

Context: The settings row tables currently live in `crates/game-menu/src/config.rs:96-423` as `ConfigItem` (fn-pointer table) + `LazyLock<Vec<ConfigItem>>` statics + label/cycle helpers. `config.rs` will be deleted in Task 8, so the tables move here first. `gameplay-drums` already depends on `dtx-config`. This task is a faithful copy — keep the exact `value`/`adjust`/`desc` closures so behavior is identical.

- [ ] **Step 1: Read the source tables**

Read `crates/game-menu/src/config.rs:96-423` in full — the `ConfigItem` struct, the four `LazyLock` statics (`SYSTEM_ITEMS`, `GAMEPLAY_ITEMS`, `AUDIO_ITEMS`, `DRUMS_ITEMS`), and every helper they call (`bool_label`, `lane_display_label`, `cycle_cy`, `cycle_hh`, `cycle_ft`, `cycle_bd`, `HspSlot`, `cycle_hsp`, and any label fns). You will copy all of them.

- [ ] **Step 2: Write the failing test**

Create `crates/gameplay-drums/src/editor/settings_data.rs` with (tests first, then port in step 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tabs_have_rows() {
        for tab in game_shell::CustomizeTab::SETTINGS {
            assert!(!settings_items(tab).is_empty(), "{tab:?} has no rows");
        }
    }

    #[test]
    fn scroll_speed_adjust_changes_value() {
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::Gameplay);
        let scroll = items.iter().find(|i| i.label == "Scroll Speed").unwrap();
        let before = (scroll.value)(&cfg);
        (scroll.adjust)(&mut cfg, 1);
        let after = (scroll.value)(&cfg);
        assert_ne!(before, after);
    }

    #[test]
    fn vsync_toggle_round_trips() {
        let mut cfg = dtx_config::Config::default();
        let items = settings_items(game_shell::CustomizeTab::System);
        let vsync = items.iter().find(|i| i.label == "VSync").unwrap();
        let start = (vsync.value)(&cfg);
        (vsync.adjust)(&mut cfg, 1);
        (vsync.adjust)(&mut cfg, 1);
        assert_eq!(start, (vsync.value)(&cfg));
    }
}
```

- [ ] **Step 3: Run to verify fail**

First add `pub mod settings_data;` to `crates/gameplay-drums/src/editor/mod.rs` (near the other `mod`/`pub mod` lines). Then:
Run: `cargo test -p gameplay-drums settings_data`
Expected: FAIL — `settings_items`/`SettingItem` not found.

- [ ] **Step 4: Implement**

At the top of `settings_data.rs` write the module (copy the four tables + all helpers verbatim from config.rs, renaming `ConfigItem`→`SettingItem`). Then add the dispatch function:

```rust
//! Settings row tables for the Customize surface (System/Gameplay/Audio/Drums).
//!
//! Ported verbatim from the former `game-menu::config` screen — same
//! `value`/`adjust`/`desc` semantics, now rendered as Customize tabs.

use std::sync::LazyLock;

use game_shell::CustomizeTab;

/// One editable setting row. `value` reads the current value as a display
/// string; `adjust` mutates `Config` with `dir = ±1` (←/→); `desc` is the
/// one-line explanation.
#[derive(Clone, Copy)]
pub struct SettingItem {
    pub label: &'static str,
    pub value: fn(&dtx_config::Config) -> String,
    pub adjust: fn(&mut dtx_config::Config, i32),
    pub desc: &'static str,
}

// <<< paste SYSTEM_ITEMS, GAMEPLAY_ITEMS, AUDIO_ITEMS, DRUMS_ITEMS here,
//     with `ConfigItem` renamed to `SettingItem`, plus every helper fn
//     they reference (bool_label, lane_display_label, cycle_cy/hh/ft/bd,
//     HspSlot, cycle_hsp, etc.) copied verbatim. >>>

/// Rows for a settings tab. Non-settings tabs (Lanes/Widgets) return `&[]`.
pub fn settings_items(tab: CustomizeTab) -> &'static [SettingItem] {
    match tab {
        CustomizeTab::System => &SYSTEM_ITEMS,
        CustomizeTab::Gameplay => &GAMEPLAY_ITEMS,
        CustomizeTab::Audio => &AUDIO_ITEMS,
        CustomizeTab::Drums => &DRUMS_ITEMS,
        CustomizeTab::Lanes | CustomizeTab::Widgets => &[],
    }
}
```

Note: if any helper is `pub(crate)` or referenced elsewhere in config.rs beyond the tables, only copy what the four tables need. Verify with the compiler.

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums settings_data`
Expected: PASS.

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/settings_data.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/gameplay-drums/src/editor/settings_data.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(gameplay-drums): port settings row tables into editor::settings_data"
```

---

### Task 3: `ActiveTab` + `ConfigDraft` + open/close lifecycle

**Files:**
- Create: `crates/gameplay-drums/src/editor/tabs.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs` (register module + plugin)

Context: The surface opens via `EditorOpen` becoming true (set in `mod.rs::toggle_editor` for Ctrl+Shift+E and `session.rs::force_open_for_session` for F1/F2 sessions). We add: `ActiveTab` (which tab shows) initialized from `PendingCustomizeTab` on open; `ConfigDraft` (editable `Config` copy) loaded on open and saved on close. `EditorOpen(pub bool)` is at mod.rs:22-23; `game_shell::PendingCustomizeTab` from Task 1.

- [ ] **Step 1: Write failing tests**

Create `crates/gameplay-drums/src/editor/tabs.rs`:

```rust
//! Customize-surface tab state + settings draft lifecycle.

use bevy::prelude::*;
use game_shell::{CustomizeTab, PendingCustomizeTab};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveTab>()
        .init_resource::<ConfigDraft>()
        .add_systems(
            Update,
            (sync_active_tab_on_open, save_draft_on_close)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// Which Customize tab is currently shown. Defaults to Widgets (F2 landing).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ActiveTab(pub CustomizeTab);

impl Default for ActiveTab {
    fn default() -> Self {
        Self(CustomizeTab::Widgets)
    }
}

/// In-memory editable copy of `config.toml`, loaded when the surface opens,
/// saved when it closes. Same persistence contract as the old config screen.
#[derive(Resource, Default, Debug, Clone)]
pub struct ConfigDraft(pub dtx_config::Config);

/// On the frame the surface opens, load the config draft and adopt the pending
/// tab (defaulting to Widgets when none was requested).
fn sync_active_tab_on_open(
    open: Res<super::EditorOpen>,
    mut pending: ResMut<PendingCustomizeTab>,
    mut active: ResMut<ActiveTab>,
    mut draft: ResMut<ConfigDraft>,
) {
    if !open.is_changed() || !open.0 {
        return;
    }
    draft.0 = dtx_config::load(&dtx_config::default_path());
    if let Some(tab) = pending.0.take() {
        active.0 = tab;
    } else {
        active.0 = CustomizeTab::Widgets;
    }
}

/// When the surface closes, persist the draft (settings tabs auto-save on exit).
fn save_draft_on_close(open: Res<super::EditorOpen>, draft: Res<ConfigDraft>) {
    if !open.is_changed() || open.0 {
        return;
    }
    let path = dtx_config::default_path();
    if let Err(e) = dtx_config::save(&path, &draft.0) {
        error!("customize: failed to save config {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_tab_defaults_to_widgets() {
        assert_eq!(ActiveTab::default().0, CustomizeTab::Widgets);
    }

    #[test]
    fn config_draft_defaults_to_config_default() {
        assert_eq!(ConfigDraft::default().0, dtx_config::Config::default());
    }
}
```

- [ ] **Step 2: Register + run to verify fail**

In `crates/gameplay-drums/src/editor/mod.rs`: add `pub mod tabs;` and add `tabs::plugin,` to the submodule plugin list (the `add_plugins((...))` tuple around mod.rs:52-62). Confirm `EditorOpen` is accessible as `super::EditorOpen` from `tabs.rs` (it's defined in mod.rs; if it's private, make it `pub(crate)` or `pub(super)` — check its visibility at mod.rs:22).

Run: `cargo test -p gameplay-drums tabs`
Expected: FAIL first (missing), then compile once module registered. Confirm the two unit tests PASS after registration.

- [ ] **Step 3: Run tests**

Run: `cargo test -p gameplay-drums tabs`
Expected: PASS (2 tests).

- [ ] **Step 4: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/tabs.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/gameplay-drums/src/editor/tabs.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(gameplay-drums): ActiveTab + ConfigDraft open/close lifecycle"
```

---

### Task 4: Tab rail in the left sidebar

**Files:**
- Modify: `crates/gameplay-drums/src/editor/ui.rs`

Context: `ui.rs` (233 lines) spawns the left sidebar on `resource_changed::<EditorOpen>` via `spawn_ui_on_open` (ui.rs:46-92): a 220px absolute node with a title, the widget `Select(WidgetKind)` buttons (one per `WidgetKind::ALL`), and action buttons (`ResetAll/Undo/Redo/Save/Close`) via the `EditorButton` enum (ui.rs:14-22). `handle_buttons` (ui.rs:125-190) matches `Interaction::Pressed`. This task adds a **tab rail** at the top of that sidebar: two labelled groups (SETTINGS/KIT) of clickable tab buttons that set `ActiveTab`, and makes the widget `Select` list render only when `ActiveTab` is `Widgets`. Because the sidebar is respawned whenever it needs to change, add `ActiveTab` to the respawn trigger.

This is an edit to existing UI code — READ `ui.rs` fully first, then adapt. Requirements the implementation must satisfy (write tests/asserts where practical; UI spawn is integration-tested by the schedule guard + manual):

- [ ] **Step 1: Add a `TabButton` component + rail rendering**

Add to `ui.rs`:

```rust
/// A rail button that activates a Customize tab.
#[derive(Component, Clone, Copy)]
pub struct TabButton(pub game_shell::CustomizeTab);
```

In `spawn_ui_on_open`, before the widget list, spawn the rail: a "SETTINGS" group label followed by one button per `game_shell::CustomizeTab::SETTINGS`, then a "KIT" group label followed by one button per `game_shell::CustomizeTab::KIT`. Each button carries `TabButton(tab)` and shows `tab.label()`. Follow the existing button-spawn style in this file (same `Node`/`Button`/`Text`/`BackgroundColor` pattern the action buttons use). Highlight the button whose tab equals the current `ActiveTab` (read `Res<super::tabs::ActiveTab>` in the spawn system).

- [ ] **Step 2: Gate the widget `Select` list on the Widgets tab**

Wrap the existing `WidgetKind::ALL` → `Select` button loop so it only spawns when `active.0 == CustomizeTab::Widgets`. (On other tabs the widget list is hidden; the right panel shows that tab's block.)

- [ ] **Step 3: Respawn the sidebar when the tab changes**

Change `spawn_ui_on_open`'s run condition so it also re-runs when `ActiveTab` changes. Today it's `resource_changed::<EditorOpen>` (ui.rs:46-ish). Make it `resource_changed::<EditorOpen>.or(resource_changed::<super::tabs::ActiveTab>)` (Bevy 0.19 run-condition combinator; if `.or` isn't available on the condition, use a small `should_respawn_ui` condition system that returns true when either changed). The despawn-then-spawn body already handles rebuild.

- [ ] **Step 4: Handle tab clicks**

Add a system `handle_tab_buttons` (register it in this module's `plugin`, gated `editor_open` like `handle_buttons`):

```rust
fn handle_tab_buttons(
    q: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut active: ResMut<super::tabs::ActiveTab>,
) {
    for (interaction, tab) in &q {
        if *interaction == Interaction::Pressed {
            active.0 = tab.0;
        }
    }
}
```

- [ ] **Step 5: Build + verify it compiles and schedule builds**

Run: `cargo test -p gameplay-drums`
Expected: PASS (existing tests + schedule guard still green). There is no pure-unit test for spawn; correctness here is verified by the guard test that the schedule builds and by the manual smoke in Task 9.

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/ui.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/gameplay-drums/src/editor/ui.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(gameplay-drums): tab rail in Customize sidebar (SETTINGS/KIT groups)"
```

---

### Task 5: Settings-row block in the right panel

**Files:**
- Modify: `crates/gameplay-drums/src/editor/panel.rs`

Context: `panel.rs` (820 lines) rebuilds the right panel via `rebuild_panel` (panel.rs:110-297), triggered when `Selection`/`EditorOpen`/`Lanes` changed, with a `Local` signature debounce (panel.rs:120-128). It chooses a **lane block** (`spawn_lane_block`, when the selected widget is `WidgetKind::Playfield`) or a **widget block** (per-widget knobs). `row()` (panel.rs:434-454) is the reusable label+content row primitive. We add a third block: a **settings-row block** shown when `ActiveTab` is a settings tab, listing the `settings_data::settings_items(tab)` rows with a `◂ value ▸` control that adjusts `ConfigDraft`.

READ `panel.rs` fully first (especially `rebuild_panel`, the signature debounce, `row()`, and how `PanelRoot`/`EditorChrome`/`GlobalZIndex` are set), then adapt.

- [ ] **Step 1: Add components for settings rows**

```rust
/// Tags a settings row control with its index into the active tab's item list.
#[derive(Component, Clone, Copy)]
pub struct SettingRow(pub usize);

/// Tags the ◂ / ▸ adjust buttons on a settings row (dir = -1 / +1).
#[derive(Component, Clone, Copy)]
pub struct SettingAdjust {
    pub index: usize,
    pub dir: i32,
}

/// Tags the value text of a settings row for live refresh.
#[derive(Component, Clone, Copy)]
pub struct SettingValueText(pub usize);
```

- [ ] **Step 2: Make the rebuild react to `ActiveTab` and branch on it**

Extend `rebuild_panel`'s change-trigger and `Local` signature to include the active tab. Add `active: Res<super::tabs::ActiveTab>` to the system params. Include `active.0` in the debounce signature tuple so switching tabs forces a rebuild. After the "return if closed" guard and before the widget/lane branching, add:

```rust
// Settings tabs render a config-row block instead of a widget/lane block.
if active.0.is_settings() {
    spawn_settings_block(&mut commands, root_entity, active.0, &draft);
    return;
}
```

You will need `draft: Res<super::tabs::ConfigDraft>` in the params. `root_entity` is the `PanelRoot` you spawn (mirror how `spawn_lane_block` receives its parent). Keep the existing widget/lane branches for the KIT tabs (Widgets → widget block driven by `Selection`; Lanes → `spawn_lane_block`). Note: for the Widgets tab, keep today's behavior exactly (per-widget knobs when a widget is selected). For the Lanes tab, call `spawn_lane_block` regardless of `Selection` (previously it required selecting the Playfield widget; now the Lanes tab implies it).

- [ ] **Step 3: Implement `spawn_settings_block`**

```rust
fn spawn_settings_block(
    commands: &mut Commands,
    root: Entity,
    tab: game_shell::CustomizeTab,
    draft: &super::tabs::ConfigDraft,
) {
    let items = crate::editor::settings_data::settings_items(tab);
    commands.entity(root).with_children(|p| {
        // Title
        p.spawn((
            Text::new(tab.label().to_uppercase()),
            /* match the title styling used by spawn_lane_block / widget block */
        ));
        for (i, item) in items.iter().enumerate() {
            // One row: label + ◂ value ▸. Reuse row() styling conventions.
            p.spawn(( /* row Node, flex */ )).with_children(|row| {
                row.spawn(( Text::new(item.label), /* label style */ ));
                row.spawn((Button, SettingAdjust { index: i, dir: -1 }))
                    .with_children(|b| { b.spawn(Text::new("<")); });
                row.spawn((
                    Text::new((item.value)(&draft.0)),
                    SettingValueText(i),
                    /* value style */
                ));
                row.spawn((Button, SettingAdjust { index: i, dir: 1 }))
                    .with_children(|b| { b.spawn(Text::new(">")); });
            })
            .insert(SettingRow(i));
        }
    });
}
```

Match the concrete `Node`/`Text`/color styling to the existing blocks in this file (do not invent a new visual language — copy the row/label/button styles `spawn_lane_block` uses).

- [ ] **Step 4: Handle settings adjust clicks + live value refresh**

Add two systems, registered in `panel.rs`'s `plugin` gated `editor_open`:

```rust
fn handle_settings_adjust(
    q: Query<(&Interaction, &SettingAdjust), Changed<Interaction>>,
    active: Res<super::tabs::ActiveTab>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
) {
    if !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (interaction, adj) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(item) = items.get(adj.index) {
                (item.adjust)(&mut draft.0, adj.dir);
            }
        }
    }
}

fn refresh_settings_values(
    active: Res<super::tabs::ActiveTab>,
    draft: Res<super::tabs::ConfigDraft>,
    mut q: Query<(&SettingValueText, &mut Text)>,
) {
    if !draft.is_changed() || !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (tag, mut text) in &mut q {
        if let Some(item) = items.get(tag.0) {
            *text = Text::new((item.value)(&draft.0));
        }
    }
}
```

(Keyboard ←/→ adjust is nice-to-have; click arrows are the 2a baseline. If you add keyboard nav, gate it so it doesn't conflict with the widget-drag arrow-nudge on KIT tabs — only act when `active.0.is_settings()`.)

- [ ] **Step 5: Build + test**

Run: `cargo test -p gameplay-drums`
Expected: PASS (schedule guard proves the new systems wire into the FixedUpdate/Update schedule).

- [ ] **Step 6: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(gameplay-drums): settings-row block for Customize settings tabs"
```

---

### Task 6: F1/F2 entry wiring

**Files:**
- Modify: `crates/game-menu/src/title.rs`
- Modify: `crates/game-menu/src/song_select.rs`

Context: `title_input` (title.rs:114-135) handles F2 today: `pick_editor_song` → `session.0 = true` → `selected.0 = Some(path)` → `request_transition(SongLoading)`. F1 must do the same but set `PendingCustomizeTab(Gameplay)`. `song_select.rs:1260-1261` currently does `F1 → request_transition(Config)`; it must instead open the customize session at the Gameplay tab (song select already has a `SelectedSong`, so it can pass the highlighted song). Both files are edition 2024.

- [ ] **Step 1: F1 from Title**

In `title_input`, add `mut pending: ResMut<game_shell::PendingCustomizeTab>` to the params and an F1 branch mirroring F2 but landing on Gameplay:

```rust
} else if keys.just_pressed(KeyCode::F1) {
    match pick_editor_song(&mut db) {
        Some(path) => {
            pending.0 = Some(game_shell::CustomizeTab::Gameplay);
            session.0 = true;
            selected.0 = Some(path);
            request_transition(&mut requests, AppState::SongLoading);
        }
        None => warn!("customize: no songs available (empty SongDb)"),
    }
}
```

Update the title hint text (title.rs:101-104 area) to include "F1 SETTINGS" alongside "F2 LAYOUT EDITOR".

- [ ] **Step 2: F1 from Song Select**

Find the F1 handler at `song_select.rs:1260-1261` (`request_transition(Config)`). Replace it so F1 opens the customize session at the Gameplay tab using the currently-highlighted song. Read the surrounding handler to get the right params (it already has access to the selected song entry + a `TransitionRequest` writer; add `ResMut<game_shell::PendingCustomizeTab>`, `ResMut<game_shell::EditorSession>`, and `ResMut<SelectedSong>` as needed, matching how F2-from-title sets them). Set:

```rust
pending.0 = Some(game_shell::CustomizeTab::Gameplay);
session.0 = true;
// selected.0 = Some(<highlighted song path>);   // use the existing selection
request_transition(&mut requests, AppState::SongLoading);
```

If song select has no valid highlighted song (empty list), fall back to `warn!` and do nothing (don't transition). Update the song-select hint text (song_select.rs:755, "F1 SETTINGS") — the label stays but now opens the merged surface.

- [ ] **Step 3: Build + test**

Run: `cargo test -p game-menu`
Expected: PASS. (This task still references `AppState::Config` nowhere new; the old `F1→Config` line is now gone from song_select. `config.rs` still exists until Task 7 — that's fine, it's just unreachable from song select now. Title F1 previously did nothing, now opens customize.)

- [ ] **Step 4: Format + commit**

```bash
rustfmt --edition 2024 crates/game-menu/src/title.rs crates/game-menu/src/song_select.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings add crates/game-menu/src/title.rs crates/game-menu/src/song_select.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "feat(game-menu): F1 opens Customize surface at Gameplay tab (title + song select)"
```

---

### Task 7: Delete the standalone config screen

**Files:**
- Delete: `crates/game-menu/src/config.rs`
- Modify: `crates/game-menu/src/lib.rs` (drop `mod config;` + plugin registration)
- Modify: `crates/game-shell/src/states.rs` (delete `AppState::Config` variant)

Context: With F1 rewired (Task 6), `AppState::Config` is unreachable. Remove the screen, its plugin, and the state variant. `config.rs` is at game-menu; `AppState::Config` at states.rs:17.

- [ ] **Step 1: Verify nothing still references Config**

Run: `rg -n "AppState::Config|mod config|config::|ConfigDraft|ActiveConfigTab|ConfigTab" crates app -g '*.rs' | rg -v "editor/settings_data|editor/tabs|gameplay-drums/src/editor"`
Expected: only `crates/game-menu/src/config.rs` itself and its `mod config;` + plugin registration in `crates/game-menu/src/lib.rs`, plus the `AppState::Config` definition line. If anything ELSE references them, STOP and report (the `ConfigDraft` in gameplay-drums/editor/tabs.rs is a DIFFERENT type — the `rg -v` filter excludes it; make sure no game-menu code still calls the deleted one).

- [ ] **Step 2: Delete + unregister**

```bash
git -C /home/lazykern/lab/dtxmaniars-input-bindings rm crates/game-menu/src/config.rs
```

In `crates/game-menu/src/lib.rs`: remove `mod config;` (or `pub mod config;`) and the `config::plugin` entry from the `add_plugins` list (find with `rg -n "config" crates/game-menu/src/lib.rs`).

In `crates/game-shell/src/states.rs`: delete the `Config,` variant from the `AppState` enum (states.rs:17). Check for any `match` over `AppState` that must now drop a `Config` arm — `rg -n "AppState::Config" crates app -g '*.rs'` should be empty after this; fix any exhaustive match the compiler flags.

- [ ] **Step 3: Workspace check**

Run: `cargo check --workspace`
Expected: clean. Fix any `AppState::Config` match arm the compiler surfaces (e.g. a `despawn_stage`/transition match).

- [ ] **Step 4: Commit**

```bash
# format only the two hand-edited files (game-menu = edition 2024, game-shell = 2021)
rustfmt --edition 2024 crates/game-menu/src/lib.rs
rustfmt --edition 2021 crates/game-shell/src/states.rs
git -C /home/lazykern/lab/dtxmaniars-input-bindings status --short   # confirm only intended files
git -C /home/lazykern/lab/dtxmaniars-input-bindings add -A
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "refactor: delete standalone config screen (merged into Customize surface)

Settings now live in the F1/F2 Customize surface; AppState::Config removed."
```

---

### Task 8: Full verification

- [ ] **Step 1: Full workspace tests (includes schedule guard tests)**

Run: `cargo test --workspace`
Expected: PASS. The plugin-schedule guard tests must pass — they prove the new `tabs`/`ui`/`panel` systems build into the real schedule.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (match CI; if CI doesn't use `-D warnings`, drop it).

- [ ] **Step 3: Manual smoke (run the app)**

Launch the desktop app. Verify:
1. Title screen shows "F1 SETTINGS" and "F2 LAYOUT EDITOR" hints.
2. **F1 from title** → loads a song, opens the surface on the **Gameplay** tab; the live autoplay plays behind. Adjusting Scroll Speed / VSync via the ◂ ▸ arrows updates the shown value. Esc closes → returns to title; re-open and confirm the changed value persisted (written to `config.toml`).
3. **Tab rail**: clicking Audio/Drums/System switches the right panel to that tab's rows. Clicking **Widgets** shows the widget list + per-widget knobs (drag/scale still works). Clicking **Lanes** shows the lane block (reorder/width/preset).
4. **F2 from title** → opens on the **Widgets** tab directly.
5. **F1 from song select** → opens the surface on Gameplay with the highlighted song.
6. Confirm `config.toml` still round-trips (values you set in the surface match what the old config screen would have written).

- [ ] **Step 4: Final fixups commit (if any)**

```bash
git -C /home/lazykern/lab/dtxmaniars-input-bindings add -A
git -C /home/lazykern/lab/dtxmaniars-input-bindings commit -m "test: customize surface shell fixups"
```

---

## Self-review notes

- **Spec §4 coverage:** §4.1 composition (rail + panel over live session) → Tasks 3-5; §4.3 tabs (settings row-lists ported, Lanes/Widgets existing blocks) → Tasks 2,4,5; §4.5 entries (F1@Gameplay, F2@Widgets, config screen deleted) → Tasks 6,7. §4.2 stage-transform + §5 bindings tab explicitly DEFERRED (stated up top).
- **Deferred, stated:** stage-transform, live-apply, bindings tab, search/dots/footer/reset-tab. `desc` field ported so the footer follow-up is cheap.
- **Type consistency:** `CustomizeTab`/`PendingCustomizeTab` (game-shell) used verbatim in Tasks 3-6; `SettingItem`/`settings_items` (Task 2) used in Task 5; `ActiveTab`/`ConfigDraft` (Task 3) used in Tasks 4-5. Note `ConfigDraft` name collides conceptually with the deleted game-menu `ConfigDraft` — they are different crates/types and the old one is deleted in Task 7; the Task 7 grep guard explicitly disambiguates.
- **Risk:** Tasks 4-5 edit large existing UI files (`ui.rs` 233, `panel.rs` 820). Implementers MUST read the current structure before editing and copy existing styling rather than invent it. The debounce signature in `rebuild_panel` must include `active.0` or tab switches won't repaint.
- **Green-per-commit:** Task 6 leaves `config.rs` present-but-unreachable (compiles); Task 7 removes it. Every task commits a compiling, test-passing state.
