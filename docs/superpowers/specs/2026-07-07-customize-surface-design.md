# Customize Surface — Design Spec

Date: 2026-07-07
Status: Approved design, pre-implementation
Prototype: interactive HTML mock validated during brainstorming (osu-pattern stage transform, bindings capture, velocity meter, lane reorder)

## 1. Goal

Merge the settings screen (F1, `game-menu/config.rs`) and the layout editor (F2, `gameplay-drums/editor/`) into a single **Customize** surface rendered over a live autoplay session, and build the currently-missing input rebinding system (keyboard + MIDI) it exposes.

Motivations:

- Keybinds/MIDI mappings are hardcoded in four disjoint places, none user-editable, none persisted.
- Most settings are feel-tuning (scroll speed, offsets, volumes, velocity threshold) and are tuned blind on a static menu screen today.
- "Where does X live?" split between two surfaces disappears; one mental home for customizing the kit.

## 2. Decisions (with alternatives rejected)

| Decision | Chosen | Rejected |
|---|---|---|
| Scope | Full: rebind wiring + UX overhaul | UX-only; bindings-only |
| Devices v1 | Keyboard + MIDI | Joypad, Mouse (enum stays, UI later) |
| Bind target | `EChannel` (12 `DRUM_CHANNELS`) | BocuD 23-pad model; `Pad0..9` |
| Surface | One merged Customize overlay (M1) | Settings Input tab (B); dedicated screen (C); split-by-preview-need (M2) |
| Panel side | Left, floating over full-window stage | Far-left rail + far-right panel sandwich |
| Kit-tab editing | Whole game shrinks into masked rect (osu skin-editor pattern) | Panels floating over full-size game |
| Persistence | Third file `bindings.toml` | Fold into `config.toml` |

Advisor findings that shaped this: osu!lazer is the only shipped game with settings-over-live-autoplay (left sidebar + search); AAA convergent pattern = description footer + button legend + per-row modified indicator + restore defaults; DJMAX draws bindings spatially on the lane picture.

## 3. Data model

### 3.1 `InputBindings` (new module `dtx-config/src/bindings.rs`)

```rust
pub struct InputBindings {
    pub version: u32,                              // 1; migration hook like layout.toml
    pub midi: MidiDeviceConfig,                    // port name filter, velocity threshold
    pub map: HashMap<EChannel, Vec<BindSource>>,   // bindable channels = DRUM_CHANNELS
}

pub enum BindSource {
    Key(KeyCode),      // serialized as string ("KeyX", "Space")
    Midi { note: u8 }, // device-agnostic in v1
}

pub struct MidiDeviceConfig {
    pub port: Option<String>,   // None = first available
    pub velocity_threshold: u8, // hits below are ignored; default 0
}
```

Rules:

- **One source → one channel.** Capturing a source already bound elsewhere requires explicit steal (confirm). No duplicate sources across channels.
- **One channel ← many sources** (rim+head MIDI notes, two keyboard keys).
- Bindable set = the 12 entries of `dtx_layout::DRUM_CHANNELS`. HH/HHO/LBD distinct at channel level; their display lane comes from `LaneArrangement.map` (many channels → one lane).
- Persisted at `$XDG_CONFIG_HOME/dtxmaniars/bindings.toml` via same load/save + `parse_with_migrations` pattern as `layout.toml`.
- Defaults = today's hardcodes: `lane_map::default_drums()` keys + `mapping.rs` GM notes, **completed** for toms/cymbals (closes the "M6c partial" TODO in `dtx-input/src/mapping.rs:22`).

### 3.2 Input wiring (kills the quadruplication)

```
bindings.toml ──load──▶ Res<InputBindings>
                            │
         ┌──────────────────┴──────────────────┐
         ▼                                     ▼
  gameplay-drums input system           dtx-input midi system
  KeyCode → EChannel                    note (≥ vel threshold) → EChannel
         └──────────── EChannel ──Lanes.map──▶ LaneId → judgment/feedback
```

- Input systems resolve `BindSource → EChannel`, then `EChannel → LaneId` via the existing `Lanes` resource. Lane re-arrangement automatically re-routes input.
- Velocity threshold applied in `dtx-input/src/midi.rs` before dispatch.
- **Delete:** `dtx-config/src/key_assign.rs` (unused BocuD port), `game-menu/src/config_key_assign.rs` (unreachable stub). **Demote to default-builders:** `gameplay-drums/src/lane_map.rs`, `dtx-input/src/mapping.rs`.

## 4. Surface architecture

### 4.1 Composition

Full-window live autoplay session (existing `EditorSession` infra: force-autoplay, chart loop, no results). Chrome floats over it:

```
┌ topbar: CUSTOMIZE · song · entry hints ───────────────────────────────┐
│┌rail──┐┌panel(348)─┐                                                  │
││SETTIN││ tab title  │        stage = FULL WINDOW game screen          │
││ Gamep││ RESET TAB  │        (transformed per tab preset, below)      │
││ Audio││ rows…      │                                       ┌inspect┐ │
││ Drums││ rows…      │                                       │widget │ │
││ Syste││            │                                       │knobs  │ │
││KIT   ││            │                                       │(kit)  │ │
││ Bindi││            │                                       └───────┘ │
││ Lanes││            │                                                 │
││ Widge││            │                                                 │
│└──────┘└───────────┘                                                  │
└ footer: description of hovered/focused row · key legend ──────────────┘
```

### 4.2 Stage transform — one mechanism, three presets

Equivalent of osu `ScalingContainer.SetCustomRect` (`SkinEditorOverlay.cs:226`, `ScalingContainer.cs:171`): the whole rendered game screen is offset/scaled into a target rect, animated (~450ms ease-out). In Bevy: scale + translate the stage root (or camera viewport sub-rect).

| Preset | When | Transform |
|---|---|---|
| **Offset** | Settings tabs (Gameplay/Audio/Drums/System/Bindings) | Scale 1, translate X by half chrome width → playfield centers in visible gap. True scale preserved for feel-tuning; right-edge widgets may clip (not the subject). |
| **Fit** | Kit tabs (Lanes/Widgets) | Uniform min-scale into the gap between panel and (optional) inspector, centered both axes, masked with visible screen-bounds outline. Whole screen visible incl. edge-anchored widgets → true WYSIWYG for anchors. |
| **Identity** | Peek (hold Tab) and surface closed | No transform; all chrome hidden during peek → exact play view. |

### 4.3 Tabs

- **Gameplay / Audio / Drums / System** — row lists ported from `config.rs` `ConfigItem` tables (labels, adjust(±1) semantics preserved). All live-apply. System tab has no preview value; it rides along so there is exactly one settings home.
- **Bindings** — see §5.
- **Lanes** — existing lane panel: preset cycler (Classic/NX-B/NX-D/Custom), per-lane reorder/width, split/merge chips. Runs under Fit preset.
- **Widgets** — existing widget editor: list, on-canvas drag/scale, anchor auto-snap, undo/redo. Selected widget opens a **right inspector** (236px) with anchor 3×3 grid, offsets, scale, z, visibility toggles. Runs under Fit preset; miniature re-fits when inspector opens.

### 4.4 Chrome behaviors (AAA baseline)

- **Footer**: full-width; left = 1–2 line description of hovered/focused row; right = key legend (`↑↓` row, `←→` adjust, `Tab` peek, `Ctrl+S` save, `Esc` close).
- **Modified indicators**: amber dot on any row whose value ≠ default; per-tab `RESET TAB` with confirm.
- **Keyboard navigability**: every row reachable with arrows; `←→` adjusts; Enter activates (capture on binding rows). (No search box — dropped from scope.)
- **Live-apply everything**; no Apply button. Display-mode changes (vsync etc.) may apply-with-revert later.

### 4.5 Entries / exits

- **F1** (title or song select) → Customize @ Gameplay tab. Song = selected (song select) or last-played/random (title).
- **F2** (title) → Customize @ Widgets tab.
- **Ctrl+Shift+E** (in-song, non-session) → Customize @ Widgets (existing toggle path).
- `AppState::Config` and `game-menu/src/config.rs` screen deleted after row tables are ported. Song-select hint text updated.
- **Esc** closes: config + bindings save on exit (today's config semantics); layout saves via explicit Ctrl+S (today's semantics) with unsaved-changes prompt on exit.

## 5. Bindings tab UX

Panel top → **Device box**: MIDI port dropdown (+ rescan), velocity threshold slider with **live meter** (bar shows last hit velocity vs threshold line; below-threshold hits show "ignored" state and dim lane flash).

Below → **channel list**: one row per `DRUM_CHANNELS` entry: lane color swatch, channel id, description, bind chips (keyboard = neutral, MIDI = blue, `×` to remove), `+` to capture.

**Selection feedback**: selected channel's lane outlined on the playfield; its bound sources drawn at the lane bottom (DJMAX-style spatial binding display).

**Capture flow**:

1. `+` (or Enter on row) → capture state: banner + lane pulses.
2. Listens to keyboard AND MIDI simultaneously; first event wins.
3. Esc cancels. Backspace on a focused row clears that channel's binds.
4. Reserved keys refused with footer explanation: Esc, F1–F12, Tab, Ctrl-combos.
5. Conflict: `"X is bound to SD — steal? Enter steal / Esc cancel"`. No silent steal.

**Post-bind verification loop**: drum input is NOT gated while the surface is open (unlike today's editor). Hits flash lanes + play hit sound + drive the meter but never judge (autoplay owns judgment). Hitting a pad auto-selects its channel row.

**Defaults**: per-channel reset; tab reset restores full default table with confirm.

## 6. Resolution / aspect-ratio strategy

Existing system is kept and is the same concept osu uses (`DrawSizePreservingFillContainer`, Strategy.Minimum):

- Author everything in REF 1280×720 (`theme.rs:7`), uniform `s = min(w/1280, h/720)`, stage stretched to `window/s` REF units — no letterboxing; extra space in the unconstrained axis becomes stage, harvested by `Anchor9` screen-anchored widgets.
- Playfield: relative height, fixed-ratio centered lanes (`PlayfieldLayout`) — matches osu!mania stage model.
- Panel/rail/inspector: fixed logical width in REF units × `s`, clamped to a physical minimum (~300px) for readability; relative height; rendered at native resolution (crisp).
- Verify every screen routes through `stage_metrics`-style scaling (song select + gameplay already do; the config screen is deleted).
- Future (v2): user UI-Scale multiplier applied to chrome only, à la osu `ScalingMode.ExcludeOverlays`.

## 7. Persistence summary

| File | Contents | Save trigger |
|---|---|---|
| `config.toml` | `Config` (system/gameplay/audio/drums) | on surface exit |
| `bindings.toml` | `InputBindings` (new) | on surface exit + Ctrl+S |
| `layout.toml` | `LayoutFile` (lanes + scene) | Ctrl+S (explicit), prompt on dirty exit |

All three share the versioned-TOML + migrations pattern. Undo/redo history covers kit tabs only (v1), unchanged from today's editor.

## 8. Testing

- `InputBindings` serde round-trip, migration from missing file → defaults, conflict/steal invariants (property: no source appears under two channels).
- Resolve pipeline unit tests: `BindSource → EChannel → LaneId` incl. HHO→HH lane and LBD→BD lane merges; lane reorder re-routes input.
- Velocity threshold: below-threshold MIDI event produces no channel event.
- Headless plugin-schedule build test for the Customize plugin set (per repo guard-test convention — green unit tests do not prove the FixedUpdate schedule builds).
- UI queries: Bevy 0.19 UI nodes use `UiGlobalTransform` (not `GlobalTransform`) — picking/drag code must query it (known silent-failure trap).
- Manual: capture flow with real e-drum kit (steal, threshold meter, HH/HHO distinct notes).

## 9. Out of scope (v2 candidates)

- Hi-hat pedal CC#4 open/close derivation (v1 relies on distinct notes per TD-style kits).
- Per-device MIDI bindings (`BindSource::Midi { device }`).
- Joypad/mouse binding UI.
- Settings undo; apply-with-revert for display modes.
- Reflection-style auto-generated settings controls (osu `[SettingSource]` pattern) — current explicit `PanelField`/`ConfigItem` enums are fine at this scale.

## 10. Implementation anchors

- Editor session/autoplay loop: `gameplay-drums/src/editor/session.rs`, `mod.rs`.
- Panel block-swap pattern to generalize into tabs: `gameplay-drums/src/editor/panel.rs`.
- Config row tables to port: `game-menu/src/config.rs:107-310`.
- Lane edit ops: `dtx-layout/src/lane_edit.rs`; placement: `dtx-layout/src/widgets.rs`.
- osu references (in `references/osu-lazer`): `SkinEditorOverlay.cs:226` (custom rect computation), `ScalingContainer.cs:171-238` (animated fit + mask), `SettingsPanel.cs` (overlay sidebar + search), `SkinSelectionHandler.cs:280` (closest-anchor thirds — already ported in `editor/snap.rs`).
