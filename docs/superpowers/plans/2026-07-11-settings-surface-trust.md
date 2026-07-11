# Honest Settings Surface + Guitar Non-Goal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every exposed setting does exactly what it says; guitar mode's stale "shipped" claims are corrected and its build is opt-in.

**Architecture:** The 2026-07-11 audit found the surface is already almost honest: of 22 exposed rows, 21 have live runtime consumers (the settings-overhaul live-apply shipped in `editor/tabs.rs:69-129`). Remaining work: (1) **Lane Display** promises four states but the runtime only consumes a boolean — collapse it to an honest two-state toggle; (2) **guitar** is dead-unreachable at runtime while code comments claim it ships — fix the docs and gate the plugin behind an off-by-default cargo feature.

**Tech Stack:** existing settings-row tables, cargo features.

**Source basis (verified 2026-07-11):**
- Settings rows: `SettingItem` tables in `crates/gameplay-drums/src/editor/settings_data.rs` (System :44-80, Gameplay :84-215, Audio :219-295, Drums :299-415), dispatched by `settings_items(tab)` (:535-543).
- Consumer audit: every exposed row traced to a live read-site EXCEPT **Lane Display** (`gameplay.lane_display`, row at `settings_data.rs:195-213`): its 4 variants `AllOn/Half/LineOff/AllOff` (`crates/dtx-config/src/lib.rs:167-183`) collapse to `shows_timing_lines()` = `{AllOn,Half}` vs `{LineOff,AllOff}` (:180-182), consumed only as `ShowTimingLines` at `beat_lines.rs:104,256`. The lane-*background* dimension has NO consumer; row desc "Toggle visibility of lane backgrounds and bar/beat lines" overpromises.
- Correctly-hidden dead config (no action needed): `SystemConfig::{bg_alpha, movie_alpha, bga_enabled, movie_enabled, log_enabled}`, `GameplayConfig::{tight, reverse, dark_mode, fillin_enabled, stage_failed_enabled}` — zero consumers, not exposed.
- Guitar: full crate `crates/gameplay-guitar`, plugin registered unconditionally at `app/dtxmaniars-desktop/src/main.rs:70`; mode selector `EGameMode { Drums, Guitar }` (`crates/game-shell/src/states.rs:40-65`); `EGameMode::next()` (:59, doc claims "Used by F2 in SongSelect") is **never called** outside tests; no UI mutates the mode. `states.rs:39` claims "M6b ships Drums + Guitar". Guitar is unreachable — hiding is "keep it unreachable, stop lying about it, stop paying its compile cost by default".

---

### Task 1: Collapse Lane Display to an honest two-state toggle

Keep the `LaneDisplay` config enum (serialized in existing config.toml files — schema stays back-compatible); the UI writes only the two states the runtime can distinguish.

**Files:**
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs:195-213`
- Modify: `crates/dtx-config/src/lib.rs` (one helper)

- [ ] **Step 1: Write the failing tests**

In `crates/dtx-config/src/lib.rs` tests:

```rust
#[test]
fn lane_display_toggle_normalizes_to_two_states() {
    assert_eq!(LaneDisplay::AllOn.toggled(), LaneDisplay::AllOff);
    assert_eq!(LaneDisplay::AllOff.toggled(), LaneDisplay::AllOn);
    // legacy intermediate values normalize by what the runtime actually shows
    assert_eq!(LaneDisplay::Half.toggled(), LaneDisplay::AllOff);   // was showing lines -> off
    assert_eq!(LaneDisplay::LineOff.toggled(), LaneDisplay::AllOn); // was hiding lines -> on
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-config -j 2 lane_display_toggle`
Expected: FAIL — `toggled` not found.

- [ ] **Step 3: Implement the helper**

On `impl LaneDisplay` (next to `shows_timing_lines()` at `lib.rs:180-182`):

```rust
/// Flip between the only two states the runtime distinguishes.
/// (Half/LineOff are legacy values from older configs; they normalize on
/// first toggle.)
pub fn toggled(self) -> Self {
    if self.shows_timing_lines() {
        LaneDisplay::AllOff
    } else {
        LaneDisplay::AllOn
    }
}
```

- [ ] **Step 4: Rewrite the settings row**

In `settings_data.rs:195-213`, change the Lane Display `SettingItem` to a toggle-style control (match how the boolean rows like VSync/Metronome are defined in the System table at :44-80 — same `SettingControl` variant and closure shapes):

- label: `"Timing Lines"`
- desc: `"Show bar and beat lines across the lanes"`
- value closure: `if cfg.gameplay.lane_display.shows_timing_lines() { "On" } else { "Off" }`
- adjust/set closures: `cfg.gameplay.lane_display = cfg.gameplay.lane_display.toggled();`
- reset: `LaneDisplay` default.

- [ ] **Step 5: Run settings tests + live-apply check**

Run: `cargo test -p gameplay-drums --lib editor::settings_data -j 2 && cargo test -p dtx-config -j 2`
Expected: PASS (the table test `all_tabs_have_rows` and round-trip tests must still pass). Verify `apply_draft_live` (`editor/tabs.rs:69-129`) already maps `lane_display` → `ShowTimingLines` — no change needed there (grep `lane_display` in tabs.rs to confirm).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/settings_data.rs crates/dtx-config/src/lib.rs
git commit -m "fix(settings): collapse Lane Display to honest two-state Timing Lines toggle"
```

---

### Task 2: Correct the guitar claims and add a reachability guard

**Files:**
- Modify: `crates/game-shell/src/states.rs:39,58`
- Create test in: `crates/game-shell/tests/` (or extend `all_stages_reachable.rs`)

- [ ] **Step 1: Fix the stale docs**

- `states.rs:39`: replace the "M6b ships Drums + Guitar" claim with:
  `/// Game mode. Drums is the product; Guitar is an experimental mechanics port with no runtime entry point (roadmap non-goal).`
- `states.rs:58` (`EGameMode::next()` doc "Used by F2 in SongSelect" — false): replace with:
  `/// Cycles modes. No production caller: guitar is intentionally unreachable. Used by tests.`

- [ ] **Step 2: Add the source-level reachability guard**

The repo convention for pinning invariants is `include_str!` source assertions (see `practice/stats.rs:178-198`). Add to `crates/game-shell/tests/all_stages_reachable.rs`:

```rust
/// Guitar is a roadmap non-goal: no production code may switch the mode.
/// If you add a guitar entry point deliberately, delete this test and label
/// the mode "experimental" in its UI.
#[test]
fn guitar_mode_has_no_production_entry_point() {
    let song_select = include_str!("../../game-menu/src/song_select.rs");
    assert!(
        !song_select.contains("EGameMode"),
        "song_select now touches EGameMode — guitar must be labeled experimental in UI"
    );
    let states = include_str!("../src/states.rs");
    assert!(states.contains("intentionally unreachable"));
}
```

(Path check: `game-shell/tests/` → `../../game-menu/src/song_select.rs`. Verify `game-menu` is a dev-dependency-free include — `include_str!` is textual, no dep needed.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p game-shell -j 2`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/game-shell
git commit -m "docs(guitar): correct stale mode claims; guard unreachability"
```

---

### Task 3: Feature-gate the guitar plugin (off by default)

Players can't reach guitar, but every build pays its compile cost and any future selector would expose incomplete mechanics silently. Make it opt-in.

**Files:**
- Modify: `app/dtxmaniars-desktop/Cargo.toml`
- Modify: `app/dtxmaniars-desktop/src/main.rs:18,70`

- [ ] **Step 1: Add the feature**

In `app/dtxmaniars-desktop/Cargo.toml`:

```toml
[features]
default = ["brp", "midi"]
guitar = []
# existing brp/midi features unchanged
```

Make the `gameplay-guitar` dependency optional and tie it to the feature:

```toml
gameplay-guitar = { workspace = true, optional = true }
```
and change the feature to `guitar = ["dep:gameplay-guitar"]`.

- [ ] **Step 2: Gate the registration**

In `main.rs`, wrap the import (:18) and registration (:70):

```rust
#[cfg(feature = "guitar")]
use gameplay_guitar::GuitarPlugin;
```

```rust
#[cfg(feature = "guitar")]
app.add_plugins(GuitarPlugin);
```

(If the current code uses a builder chain, restructure that one call into a statement-form `app` mutation as the surrounding code allows.) Keep `init_resource::<EGameMode>()` (:71) UNGATED — `game-shell` types reference the mode resource regardless.

- [ ] **Step 3: Verify both configurations build**

Run: `cargo check -p dtxmaniars-desktop -j 2 && cargo check -p dtxmaniars-desktop --features guitar -j 2`
Expected: both clean. Also `cargo test -p gameplay-guitar -j 2` still passes (the crate keeps building in CI's test matrix independent of the app feature).

- [ ] **Step 4: Commit**

```bash
git add app/dtxmaniars-desktop
git commit -m "build(guitar): make guitar plugin an opt-in cargo feature"
```

---

## Verification (whole plan)

1. `cargo test -p dtx-config -p gameplay-drums -p game-shell -p gameplay-guitar -j 2` green.
2. Manual (bevy-brp): Customize → Gameplay shows "Timing Lines On/Off"; toggling live-hides/shows bar/beat lines mid-song; old config with `lane_display = "Half"` loads and shows "On", first toggle writes `AllOff`.
3. Exposed-settings audit stays clean: all 22 rows have consumers (21 pre-existing + the recut Timing Lines). Dead config fields remain unexposed.
4. Default build contains no guitar code (`cargo tree -p dtxmaniars-desktop | grep -c gameplay-guitar` → 0); `--features guitar` restores it.

## Explicitly not done (and why)

- Wiring lane *backgrounds* (the dead half of Lane Display): would be a new rendering feature, not honesty work. If wanted later, reintroduce a multi-state row then.
- Removing dead config fields (`tight`, `reverse`, `dark_mode`, `bg_alpha`, ...): they're hidden (surface already honest) and several are earmarked by other plans (`bg_alpha` → accessibility dimming). Leave them.
