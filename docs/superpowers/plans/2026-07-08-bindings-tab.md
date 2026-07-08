# Bindings Tab (Phase 3a — keyboard-first) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a **Bindings** tab to the Customize surface that rebinds keyboard (and, once Phase 3b lands, MIDI) keys to the 12 drum channels — with a live-editable `InputBindings` resource saved to `bindings.toml`, a per-channel list with bind chips + a capture flow, un-gated drum input that flashes lanes + plays the kit voice (feedback without judgment) while the surface is open, and a spatial display of the selected channel's binds on the playfield.

**Architecture:** Phase 1 shipped the `bindings.toml` schema + `BindResolver`. This phase makes bindings **live-editable**: introduce an `InputBindings` Bevy resource seeded on Performance enter, rebuild `BindResolver` + persist to disk whenever it changes, and drive both from a new Bindings panel block. A `Bindings` variant joins `CustomizeTab` (SETTINGS group → Offset stage preset). Drum-hit feedback (lane flash + kit sound) is decoupled from judgment by adding a `LaneHit`→sound reader (the key-cap flash already reads `LaneHit` directly) and un-gating keyboard capture while the surface is open. The spatial display reuses the `StageRect`+`Lanes` substrate the pads/key-caps already use, so it follows the stage transform automatically.

**Tech Stack:** Rust, Bevy 0.19. Crates: `game-shell` (`CustomizeTab`), `dtx-config` (`InputBindings` gains `Resource`), `gameplay-drums` (resource lifecycle, input un-gate, feedback sound, panel block, capture, spatial display). Depends on Phase 1 (bindings backend) + 2a (tabs) + 2b (StageRect) on branch `feat/customize-surface`.

**Spec:** `docs/superpowers/specs/2026-07-07-customize-surface-design.md` §5 (bindings tab UX) + §3 (data model) + §4.5 (Esc saves on exit).

**Explicitly DEFERRED to Phase 3b (separate plan — needs `midir` + web research per infra rules):**
- Real MIDI device connection (`midir` `MidiSource` impl behind the `midi` feature), **port enumeration + dropdown + rescan**, and the **velocity live meter** (needs a real device feeding velocity). Today `VirtualSource` is the only source and velocity is read-then-discarded in `poll_midi` — no real notes arrive in the app.
- 3a's capture flow DOES listen to the MIDI event path (drains `VirtualSource`), so MIDI binds "just work" the moment 3b wires a real device — but 3a ships keyboard-functional. The device box in 3a exposes only the **velocity-threshold** control (persisted, honored by `BindResolver`); the port dropdown + meter are 3b.

**Investigation anchors (file:line):**
- `dtx-config/src/bindings.rs`: `InputBindings` (`:82`, `midi`+`map: HashMap<EChannel, Vec<BindSource>>`, NOT a Resource), `bind()` steal-semantics (`:201`), `to_file()`/`resolve()` (`:210`/`:228`), `load_bindings`/`save_bindings`/`default_bindings_path` (`:280-301`), `BINDABLE_CHANNELS` (`:18`), `BindSource::{Key(KeyCode),Midi{note}}` (`:36`).
- `gameplay-drums/src/bindings.rs`: `BindResolver` Resource (`:20`), `from_bindings` (`:36`), `reload_bindings` on `OnEnter(Performance)` (`:14`,`:72`).
- `gameplay-drums/src/lane_map.rs`: `LANE_ORDER` (`:18`), `lane_of(ch)` (`:34`), `lane_channel(lane)` (`:42`).
- `dtx-core/src/channel.rs`: `EChannel`, `short_name` (`:137`), drum channel bytes (`:19-31`).
- `dtx-layout/src/presets.rs:30-39`: per-channel classic colors. `gameplay-drums/src/lanes.rs`: `Lanes` resource, `col_of(ch)`, `column_color(col)` (used by `keyboard_viz.rs:122,143`).
- `gameplay-drums/src/input.rs`: `capture_key_to_lane_input` gated `editor_closed` (`:26-31`) — un-gate target.
- `gameplay-drums/src/lib.rs` `mod midi_consumer`: `poll_midi` gated `Performance` only (`:309`), velocity dropped after threshold gate (`:337`).
- `gameplay-drums/src/judge.rs:79`: `judge_lane_hit_system` (Performance only) — must NOT judge while surface open.
- `gameplay-drums/src/keyboard_viz.rs:116`: `flash_key_caps_on_hit` reads `LaneHit` directly → the feedback-without-judgment precedent.
- `gameplay-drums/src/hit_sound.rs`: `play_judgment_sounds` reads `JudgmentEvent` (needs chart); NO `LaneHit`→sound path exists — we add one.
- `game-shell/src/states.rs`: `CustomizeTab` (`:112`), `ALL`/`SETTINGS`/`KIT` (`:123`/`:132`/`:139`), `label`/`is_settings` (`:142`/`:154`), partition test (`:169`).
- `gameplay-drums/src/editor/panel.rs`: `rebuild_panel` (`:128`), debounce sig (`:138`), `is_lanes`/`is_settings` branches (`:158`/`:183`).
- `gameplay-drums/src/editor/ui.rs`: rail loops `SETTINGS` (`:92`) + `KIT` (`:96`).
- `gameplay-drums/src/editor/stage.rs`: `preset_rect` uses `is_settings()` → Offset.

**Critical conventions:**
- NEVER `cargo fmt`/`--all`/`-p`. ONLY `rustfmt --edition <ed> <files you edited>`. `game-shell`/`dtx-config`/`gameplay-drums` = edition **2021**.
- A format-on-save DAEMON in this environment reorders `use` imports (harmless churn). Before editing a file, `git -C <wt> checkout -- <that file>` to start clean; before `git add`, `git -C <wt> status --short` and stage ONLY intended files (do not stage drifted files).
- Work from worktree `/home/lazykern/lab/dtxmaniars-customize` (branch `feat/customize-surface`). Run all cargo/git with that cwd.
- Bevy 0.19: `UiTransform`/`UiGlobalTransform` not `GlobalTransform`; `MessageWriter`/`add_message` not `EventWriter`; `windows.single()` returns `Result`.
- Green unit tests do NOT prove the schedule builds — the final task runs `cargo test --workspace` incl. the schedule guard.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/game-shell/src/states.rs` | Modify | Add `CustomizeTab::Bindings` to SETTINGS group; `ALL`→7, `SETTINGS`→5, `label`, `is_settings`; fix partition test |
| `crates/dtx-config/src/bindings.rs` | Modify | `#[derive(Resource)]` on `InputBindings` (feature-gated on bevy? — use a newtype in gameplay-drums instead if dtx-config has no bevy dep; see Task 2) |
| `crates/gameplay-drums/src/bindings.rs` | Modify | `InputBindings` runtime resource lifecycle: seed on Performance enter, rebuild `BindResolver` + save on change |
| `crates/gameplay-drums/src/hit_feedback.rs` | Create | `LaneHit`→kit-sound + lane-flash reader (chart-independent), active while surface open |
| `crates/gameplay-drums/src/input.rs` | Modify | Un-gate `capture_key_to_lane_input` while surface open; suppress judgment while open |
| `crates/gameplay-drums/src/editor/bindings_panel.rs` | Create | Bindings panel block: device box (velocity threshold) + channel list + bind chips + `+`/`×` |
| `crates/gameplay-drums/src/editor/bindings_capture.rs` | Create | Capture-flow state machine: listen key+virtual-midi, conflict/steal, Esc cancel, reserved-key refusal |
| `crates/gameplay-drums/src/editor/bindings_spatial.rs` | Create | Selected channel's lane outline + bound-source labels on the playfield (via `StageRect`+`Lanes`) |
| `crates/gameplay-drums/src/editor/panel.rs` | Modify | `rebuild_panel`: `Bindings` render branch BEFORE `is_settings` |
| `crates/gameplay-drums/src/editor/mod.rs` | Modify | Register the 3 new editor modules |

---

### Task 1: `CustomizeTab::Bindings` variant

**Files:** Modify `crates/game-shell/src/states.rs`.

Context: `CustomizeTab` has no `Bindings` variant. Put it in the **SETTINGS** group (it uses the Offset stage preset like the settings tabs, and the strict 2-group partition test requires every variant in exactly SETTINGS xor KIT). `is_settings()` stays true for `Bindings` (drives the Offset preset), but `panel.rs` (Task 8) branches on `Bindings` BEFORE the settings-rows branch, and `settings_data::settings_items(Bindings)` already returns `&[]`, so no empty settings block renders.

- [ ] **Step 1: Update the partition test**

In the `#[cfg(test)]` block, the existing `customize_tab_groups_partition_all_variants` already asserts `SETTINGS.len()+KIT.len()==ALL.len()` and XOR membership — it will pass automatically once `Bindings` is added to both `ALL` and `SETTINGS`. Add one assertion:

```rust
#[test]
fn bindings_is_a_settings_tab() {
    assert!(CustomizeTab::Bindings.is_settings());
    assert!(CustomizeTab::SETTINGS.contains(&CustomizeTab::Bindings));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p game-shell bindings_is_a_settings_tab`. Expected: FAIL — no `Bindings` variant.

- [ ] **Step 3: Add the variant**

- Add `Bindings` to the enum (place it after `System`, before `Lanes`, so the rail shows it at the bottom of SETTINGS).
- `ALL: [CustomizeTab; 7]` — insert `CustomizeTab::Bindings` after `System`.
- `SETTINGS: [CustomizeTab; 5]` — insert `CustomizeTab::Bindings` after `System`.
- `label()`: add `CustomizeTab::Bindings => "Bindings"`.
- `is_settings()` unchanged (derives from `SETTINGS.contains`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p game-shell`. Expected: PASS (partition test + new test + all existing).

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/game-shell/src/states.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/game-shell/src/states.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(game-shell): add CustomizeTab::Bindings (settings group)"
```

---

### Task 2: Live `InputBindings` resource + rebuild/save on change

**Files:** Modify `crates/gameplay-drums/src/bindings.rs` (+ maybe `dtx-config/src/bindings.rs`).

Context: `InputBindings` is not a runtime resource — only `BindResolver` is, reloaded from disk `OnEnter(Performance)`. To edit live, hold `InputBindings` as a resource, rebuild `BindResolver::from_bindings` whenever it changes, and save to `bindings.toml` (debounced). `dtx-config` likely has NO bevy dependency (check: `rg -n "bevy" crates/dtx-config/Cargo.toml`). If it does not, do NOT add one — instead wrap it in a newtype resource IN `gameplay-drums`: `#[derive(Resource)] pub struct LiveBindings(pub dtx_config::InputBindings);`.

- [ ] **Step 1: Write failing test**

In `gameplay-drums/src/bindings.rs` tests:

```rust
#[test]
fn resolver_tracks_live_binding_edit() {
    let mut ib = dtx_config::InputBindings::default();
    // Bind a fresh key to Snare and confirm the resolver routes it.
    let sd = dtx_core::EChannel::Snare;
    ib.bind(sd, dtx_config::BindSource::Key(bevy::input::keyboard::KeyCode::KeyP));
    let resolver = BindResolver::from_bindings(&ib);
    assert_eq!(resolver.lane_for_key(bevy::input::keyboard::KeyCode::KeyP),
               Some(crate::lane_map::lane_of(sd)));
}
```

- [ ] **Step 2: Run to verify fail (or pass if already supported)**

Run: `cargo test -p gameplay-drums resolver_tracks_live_binding_edit`. If `from_bindings` + `bind` already make this pass, good — it documents the contract. If `EChannel`/`BindSource` re-export paths differ, fix the test imports.

- [ ] **Step 3: Add the resource + lifecycle systems**

```rust
/// Live, editable bindings (the Bindings tab mutates this; the resolver + disk follow).
#[derive(Resource, Debug, Clone)]
pub struct LiveBindings(pub dtx_config::InputBindings);

impl Default for LiveBindings {
    fn default() -> Self { Self(dtx_config::InputBindings::default()) }
}
```

In this module's `plugin`:
- `init_resource::<LiveBindings>()`.
- On `OnEnter(AppState::Performance)`: seed `LiveBindings.0 = dtx_config::load_bindings(&dtx_config::default_bindings_path())` (do this in `reload_bindings`, and rebuild `BindResolver` from it — so both resolver and LiveBindings come from the same load).
- Add `apply_live_bindings.run_if(resource_changed::<LiveBindings>)` in `Update`: rebuild `*resolver = BindResolver::from_bindings(&live.0)` AND save to disk. Save should be debounced/best-effort: write on change; on error `error!`. (Frequent writes are fine at human edit rate; if you want, gate save behind surface-close instead — but per spec §4.5 bindings save on exit AND Ctrl+S; simplest for 3a: rebuild resolver on every change, and save on change. Match the config-draft pattern from Phase 2a if you prefer save-on-close.)

Decide save trigger to match Phase 2a's `ConfigDraft` (which saves on surface close). For consistency: rebuild `BindResolver` live on every `LiveBindings` change (so feedback is immediate), but SAVE to disk on surface close (mirror `tabs::save_draft_on_close`). Implement a `save_bindings_on_close` reading `EditorOpen` change→false.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums bindings`. Expected: PASS.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/bindings.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/bindings.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): live LiveBindings resource; resolver+disk follow edits"
```

---

### Task 3: Feedback without judgment + un-gate input while surface open

**Files:** Create `crates/gameplay-drums/src/hit_feedback.rs`; Modify `crates/gameplay-drums/src/input.rs`, `crates/gameplay-drums/src/lib.rs` (register module), `crates/gameplay-drums/src/judge.rs` (suppress judgment while open).

Context spec §5 "post-bind verification loop": while the surface is open, hitting a pad must flash its lane + play the kit voice + drive feedback, but NOT judge (autoplay owns judgment; and on the Bindings tab there may be no meaningful judgment). `flash_key_caps_on_hit` (`keyboard_viz.rs:116`) already flashes from `LaneHit` with no judge dependency. Sound today only comes from `JudgmentEvent` (needs a chart). Add a `LaneHit`→kit-sound reader. Un-gate keyboard capture (`input.rs`) while the surface is open. Ensure `judge_lane_hit_system` does not run (or produces no score effects) while the surface is open — check whether it already no-ops without an active chart; if it would mis-judge, gate it with `editor_closed`.

- [ ] **Step 1: Add the `LaneHit`→sound reader**

`hit_feedback.rs`:

```rust
//! Chart-independent hit feedback for the Customize surface: a LaneHit plays
//! the lane's kit voice + (via existing key-cap flash) lights the lane, without
//! any judgment. Active only while the editor/Customize surface is open.

use bevy::prelude::*;
use crate::events::LaneHit;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        play_kit_voice_on_hit
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(crate::editor::editor_open),
    );
}

fn play_kit_voice_on_hit(
    mut hits: MessageReader<LaneHit>,
    // + the sound-playing resource used by hit_sound.rs (SoundBank / kit voices)
) {
    for hit in hits.read() {
        // play the default kit voice for hit.lane (mirror hit_sound.rs's player,
        // but keyed off the lane's channel default sound rather than JudgmentEvent).
    }
}
```

READ `hit_sound.rs` + `sound_bank.rs` to find the exact voice-playing API (which asset/handle + how it's triggered). Reuse it. If per-lane kit voices aren't readily available, play a single generic hit sample — the point is audible feedback while binding. Keep it minimal.

- [ ] **Step 2: Un-gate keyboard capture while surface open**

In `input.rs`, `capture_key_to_lane_input` is `.run_if(crate::editor::editor_closed)`. Change so it ALSO runs while the surface is open (so hits reach `LaneHit` for feedback). Simplest: drop the `editor_closed` gate on the CAPTURE (it emits `LaneHit`), but ensure the DOWNSTREAM judgment is what's gated instead (Step 3). Verify MIDI's `poll_midi` (already un-gated) stays consistent.

- [ ] **Step 3: Suppress judgment while surface open**

In `judge.rs`, add `.run_if(crate::editor::editor_closed)` to `judge_lane_hit_system` so no scoring/judgment happens while the surface is open — the hit still flashes + sounds via Steps 1-2. Confirm this doesn't break normal play (surface closed → judgment runs as before).

- [ ] **Step 4: Register + test**

Add `mod hit_feedback;` + `hit_feedback::plugin` in `lib.rs`. Run `cargo test -p gameplay-drums`. Expected: PASS incl. schedule guard. Existing judge/feedback tests: confirm they run with the surface closed (default), so they stay green.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/hit_feedback.rs crates/gameplay-drums/src/input.rs crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/lib.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/hit_feedback.rs crates/gameplay-drums/src/input.rs crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/lib.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): feedback-without-judgment while Customize surface open"
```

---

### Task 4: Bindings panel block (device box + channel list)

**Files:** Create `crates/gameplay-drums/src/editor/bindings_panel.rs`; Modify `crates/gameplay-drums/src/editor/mod.rs` (register), `crates/gameplay-drums/src/editor/panel.rs` (branch — done in Task 8; here just build the block fn).

Context: The Bindings tab renders (spec §5): a **device box** (3a: velocity-threshold row only — a `◂ value ▸` like the settings rows, reading/writing `LiveBindings.0.midi.velocity_threshold`), then a **channel list**: one row per `BINDABLE_CHANNELS` — lane color swatch, channel short name, its current bind chips (keyboard chips neutral; a `×` per chip to remove), and a `+` to start capture. Match the settings-row + lane-block styling from `panel.rs` / `bindings`-adjacent code.

- [ ] **Step 1: Components + block fn**

```rust
#[derive(Component, Clone, Copy)]
pub struct ChannelRow(pub dtx_core::EChannel);
#[derive(Component, Clone, Copy)]
pub struct BindChipRemove { pub channel: dtx_core::EChannel, pub index: usize }
#[derive(Component, Clone, Copy)]
pub struct CaptureStartButton(pub dtx_core::EChannel);
#[derive(Component, Clone, Copy)]
pub struct VelocityThresholdAdjust(pub i32); // ±1

pub fn spawn_bindings_block(
    commands: &mut Commands,
    root: Entity,
    theme: &dtx_ui::theme::Theme,
    live: &crate::bindings::LiveBindings,
) { /* title, velocity-threshold row, then a row per BINDABLE_CHANNELS */ }
```

Render each channel row: swatch color = the channel's classic color (from `dtx_layout` presets / `Lanes::column_color`), label = `channel.short_name()`, chips = `live.0.map.get(&channel)` rendered as chip nodes (key name via a `KeyCode`→string helper — add one, or reuse any existing), each with a `×` `BindChipRemove`, and a trailing `+` `CaptureStartButton`.

- [ ] **Step 2: Interaction systems** (registered in `bindings_panel::plugin`, gated `editor_open`)

- `handle_velocity_adjust`: on `◂`/`▸`, clamp-adjust `LiveBindings.0.midi.velocity_threshold` in `[0,127]`.
- `handle_bind_chip_remove`: on `×`, remove that source from that channel's `Vec` in `LiveBindings` (mutating `LiveBindings` triggers Task 2's resolver rebuild).
- `handle_capture_start`: on `+`, set the capture state (Task 5) to `Capturing(channel)`.
- A `refresh_bindings_block` that rebuilds the block when `LiveBindings` changes or the active tab becomes Bindings (mirror `panel.rs` rebuild triggers, or just re-run `rebuild_panel` — since `rebuild_panel` already rebuilds on tab change, and you can add `resource_changed::<LiveBindings>` to its trigger in Task 8).

- [ ] **Step 3: Register + test**

Add `pub mod bindings_panel;` + `bindings_panel::plugin` to `editor/mod.rs`. Run `cargo test -p gameplay-drums`. Expected: PASS (schedule guard). Add a small unit test for the `KeyCode`→label helper if you write one.

- [ ] **Step 4: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/bindings_panel.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/bindings_panel.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): bindings panel block (device box + channel list)"
```

---

### Task 5: Capture flow

**Files:** Create `crates/gameplay-drums/src/editor/bindings_capture.rs`; Modify `crates/gameplay-drums/src/editor/mod.rs` (register).

Context spec §5 capture flow: `+`/Enter → capture state (banner + lane pulse); listens keyboard AND (virtual) MIDI, first event wins; Esc cancels; reserved keys refused (Esc, F1-F12, Tab, Ctrl-combos) with footer explanation; conflict → steal confirm (`"X is bound to SD — Enter steal / Esc cancel"`); no silent steal.

- [ ] **Step 1: Capture state resource + tests**

```rust
#[derive(Resource, Default, Debug, Clone)]
pub enum CaptureState {
    #[default] Idle,
    Capturing(dtx_core::EChannel),
    ConfirmSteal { channel: dtx_core::EChannel, source: dtx_config::BindSource, from: dtx_core::EChannel },
}

/// Reserved keys that cannot be bound.
pub fn is_reserved(key: KeyCode) -> bool { /* Esc, F1..F12, Tab, plus caller checks Ctrl held */ }
```

Test `is_reserved` for Escape/F5/Tab = true, `KeyX` = false.

- [ ] **Step 2: Capture system**

`capture_binding` (gated `editor_open`, run when `CaptureState != Idle`):
- Read `ButtonInput<KeyCode>::get_just_pressed` (skip reserved; skip when Ctrl/Alt/Super held). Also drain new MIDI notes from `VirtualSource` (so 3b works later).
- First event → candidate `BindSource`. If already bound to another channel (`live.0.channel_for(src)` returns a different channel) → transition to `ConfirmSteal`. Else `live.0.bind(channel, src)` (which also handles same-channel dedupe) → `Idle`.
- In `ConfirmSteal`: Enter → `live.0.bind(...)` (steal), Esc → cancel; both → `Idle`.
- Esc in `Capturing` → `Idle` (cancel).

- [ ] **Step 3: Pad-hit auto-select** (spec §5: hitting a pad auto-selects its channel row) — a system reading `LaneHit` while on the Bindings tab that sets a `SelectedChannel(EChannel)` resource (used by Task 6 spatial display). Keep minimal.

- [ ] **Step 4: Register + test**

Add `pub mod bindings_capture;` + plugin. Run `cargo test -p gameplay-drums`. Expected: PASS.

- [ ] **Step 5: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): binding capture flow (steal-confirm, reserved keys)"
```

---

### Task 6: Spatial bind display on the playfield

**Files:** Create `crates/gameplay-drums/src/editor/bindings_spatial.rs`; Modify `crates/gameplay-drums/src/editor/mod.rs` (register).

Context spec §5: the selected channel's lane is outlined on the playfield, and its bound sources are drawn at the lane bottom (DJMAX-style). Reuse the `StageRect`+`Lanes` substrate (so it follows the stage transform). Only render while the Bindings tab is active.

- [ ] **Step 1: Outline + labels system**

A system (gated `editor_open` + `active.0 == Bindings`) that, for the `SelectedChannel` (Task 5) — or all channels if none selected — computes the lane column rect via `Lanes::col_of(channel)` + lane width mapped through `StageRect` (mirror how `keyboard_viz.rs` / pads compute lane x), and draws: (a) a lane outline node, (b) small text chips of the bound source names at the lane bottom. Despawn/hide when off the Bindings tab or surface closed (mirror `editor/stage.rs`'s `StageOutline` lifecycle from 2b Task 8 — that's the reusable pattern).

- [ ] **Step 2: Register + test**

Add `pub mod bindings_spatial;` + plugin. Run `cargo test -p gameplay-drums`. Expected: PASS (schedule guard).

- [ ] **Step 3: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/bindings_spatial.rs crates/gameplay-drums/src/editor/mod.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): spatial bind display on playfield"
```

---

### Task 7 (folded): `panel.rs` Bindings render branch

**Files:** Modify `crates/gameplay-drums/src/editor/panel.rs`.

Context: `rebuild_panel` must render the bindings block for the Bindings tab. Add a branch BEFORE the `is_settings()` branch, and add `resource_changed::<crate::bindings::LiveBindings>` to the rebuild trigger + the `Local` debounce signature so chip edits repaint.

- [ ] **Step 1: Add the branch**

After the closed-guard + root spawn, before the widget/lane/settings branches:

```rust
if active.0 == game_shell::CustomizeTab::Bindings {
    crate::editor::bindings_panel::spawn_bindings_block(&mut commands, root_entity, &t, &live);
    return;
}
```

Add `live: Res<crate::bindings::LiveBindings>` to `rebuild_panel` params. Extend the trigger `.or(resource_changed::<crate::bindings::LiveBindings>())` and add a bindings-signature element to the `Local` debounce tuple (e.g. a cheap hash/len of the map, or just include a bump counter that Task 4's edit systems increment — simplest: add `resource_changed::<LiveBindings>` to the trigger AND force rebuild when on Bindings tab by including a "bindings revision" in the signature; a `u64` revision resource incremented on edit is cleanest).

- [ ] **Step 2: Test**

Run: `cargo test -p gameplay-drums`. Expected: PASS.

- [ ] **Step 3: Format + commit**

```bash
rustfmt --edition 2021 crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-customize status --short
git -C /home/lazykern/lab/dtxmaniars-customize add crates/gameplay-drums/src/editor/panel.rs
git -C /home/lazykern/lab/dtxmaniars-customize commit -m "feat(gameplay-drums): panel renders Bindings tab block"
```

---

### Task 8: Full verification + manual smoke

- [ ] **Step 1:** `cargo test --workspace` — PASS incl. schedule guard.
- [ ] **Step 2:** `cargo clippy -p gameplay-drums --all-targets` — no new warnings in new files.
- [ ] **Step 3: Manual smoke** (`cargo run -p dtxmaniars-desktop`):
  1. F1 → surface; click **Bindings** tab (bottom of SETTINGS). Channel list shows 12 channels with color swatches + current key chips.
  2. `+` on Snare → capture banner; press a free key → it binds (chip appears); press an already-bound key → steal-confirm prompt (Enter steals / Esc cancels); press Esc → cancels.
  3. `×` on a chip removes it.
  4. Velocity threshold ◂/▸ adjusts.
  5. With the surface open, hitting a bound key **flashes its lane + plays a sound but does NOT judge/score**; hitting a pad auto-selects its channel row; the selected channel's lane is outlined with its binds drawn at the bottom.
  6. Esc closes → `bindings.toml` written; reopen → binds persisted; **close and play normally → new binds are in effect** (BindResolver rebuilt).
- [ ] **Step 4:** Final fixups commit if any.

---

## Self-review notes

- **Spec §5 coverage:** device box velocity-threshold → Task 4 (port dropdown + meter = 3b); channel list + chips + `+`/`×` → Task 4; capture flow (steal, reserved, Esc) → Task 5; post-bind verification (feedback no judgment, pad auto-select) → Tasks 3+5; spatial display → Task 6; save on exit → Task 2. §3 data model reuse → Task 2 (`InputBindings`/`bind`/`to_file`).
- **Deferred (stated up top):** all real-MIDI-device infrastructure → Phase 3b (needs `midir` + web research). 3a is keyboard-functional and MIDI-capture-ready.
- **Type consistency:** `LiveBindings(InputBindings)` (Task 2) used in Tasks 4-7; `CaptureState`/`SelectedChannel` (Task 5) in Task 6; `CustomizeTab::Bindings` (Task 1) in Tasks 4/7/stage-preset. `spawn_bindings_block` (Task 4) called from `panel.rs` (Task 7).
- **Risk:** Task 3 (un-gate input + suppress judgment) touches the live play path — the regression is "normal play still judges/scores correctly (surface closed)"; the schedule guard + existing judge tests (which run surface-closed) cover it, plus the Step-3 manual smoke. Task 6 spatial math must go through `StageRect` (2b) or it won't follow the transform — reuse the pads/key-caps substrate, do not read raw window size.
- **Green-per-commit:** each task compiles + tests pass; the tab renders progressively (Task 4 block, Task 7 wires it into the panel).
```
