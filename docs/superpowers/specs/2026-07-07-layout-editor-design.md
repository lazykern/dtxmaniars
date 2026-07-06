# Layout Editor + Lane Arrangement — Design

Date: 2026-07-07
Status: approved pending user review
Pillar: 2 (Layout Editor), absorbs §2.1 (Lane arrangement) + §5 (HUD editor) from
`docs/notes/2026-07-06-feature-ideas-research.md`.

## Goal

An in-game, mouse-first layout editor (osu!lazer skin-editor style) that lets the
player arrange gameplay HUD widgets and the drum lane layout, live over running
autoplay gameplay, persisted to a single user layout file.

## Scope

**In (v1):**
- Lane arrangement layer: runtime `EChannel → DisplayLane` mapping, presets
  (Classic, XG variants), custom reorder / per-lane width / merge / split.
- Layout editor overlay for the **gameplay scene only**.
- Per-widget: drag (position), 9-point anchor/origin with proximity snap,
  uniform scale, show/hide, z-order. Mouse-first; keyboard nudge as convenience.
- Per-widget play/practice visibility flags with editor mode preview.
- Two anchor spaces: `screen` and `playfield`.
- Single active layout file, code defaults, migration infra from day 1.
- Undo/redo, unsaved-changes confirm.

**Out (v1, additive later — architecture must not block):**
- Song select / results scenes (format reserves scene keys).
- Add/remove/duplicate widget instances (format is an instance list; v1 enforces
  exactly one instance per kind).
- Per-widget custom settings sidebar (format reserves `settings` map).
- Rotation (bevy_ui nodes don't rotate cleanly).
- Named layout profiles (single file → `layouts/<name>.toml` later, format unchanged).
- Per-lane color override (format reserves `color` on `DisplayLane`).

## Architecture

```
                        ┌─────────────────────────────────────┐
                        │  layout.toml  (~/.config/dtxmaniars) │
                        │  version, [scene.gameplay] widgets,  │
                        │  [lanes] arrangement                 │
                        └──────────────┬──────────────────────┘
                              load ⊕ code defaults (+ migrations)
                                       ▼
   ┌────────────────────  dtx-layout (new crate)  ────────────────────┐
   │                                                                  │
   │  ActiveLayout (Resource)          LaneArrangement (Resource)     │
   │   widget instances:                display lanes: order,         │
   │   {kind, space, offset, anchor,    widths, channel→lane map,     │
   │    origin, scale, z, vis_play,     preset id                     │
   │    vis_practice, settings{}}                                     │
   │                                                                  │
   │  WidgetRegistry (Resource)                                       │
   │   kind → { spawn_fn, display_name, default_instance }            │
   └───────────┬──────────────────────────────┬──────────────────────┘
               ▼                              ▼
   ┌── gameplay-drums ──────────┐  ┌── layout-editor (new crate) ────┐
   │ spawn_hud: iterate         │  │ hotkey toggle overlay            │
   │  registry → spawn_fn(      │  │ mouse: select/drag/handles       │
   │  instance_for(kind))       │  │ sidebar: widget list, presets    │
   │ apply system: instance     │  │ lane drag: reorder/resize        │
   │  changed → reposition node │  │ mutates ActiveLayout/Lane-       │
   │ PlayfieldLayout derives    │  │  Arrangement → save on exit      │
   │  from LaneArrangement      │  │ undo: snapshot stack             │
   └────────────────────────────┘  └──────────────────────────────────┘
```

**Crates:**
- `dtx-layout` — data only: types, serde, load/save, migrations, registry type,
  built-in defaults/presets. No UI, no editor. `gameplay-drums` depends on it.
- `layout-editor` — the overlay: selection, drag, handles, sidebars, undo.
  Depends on `dtx-layout`. Only `game-shell` wires its plugin. Delete-safe.

**Flow:** enter Performance → `spawn_hud` reads `ActiveLayout` via registry →
widgets positioned. Editor open → mutates the two resources live →
change-detection systems reposition → save on Ctrl+S / exit. Gameplay code never
knows the editor exists; the editor only touches the two resources.

## Data model

### Anchor math

Bevy UI has no anchors; we compute absolute `left/top` in the existing 1280×720
ref-space (`HudRefRect` scaling system reused unchanged).

```
space rect R = Screen → (0,0,1280,720)
             | Playfield → strip rect (dynamic, derived from LaneArrangement)

A = anchor point on R (9-point: TL TC TR / CL CC CR / BL BC BR)
O = origin point on widget (9-point), widget design size w×h is code-owned
offset = (dx, dy) ref-px

left = A.x + dx − O.x_frac · (w · scale)
top  = A.y + dy − O.y_frac · (h · scale)
```

Anchor snapping (osu trick): while dragging, re-express the same screen position
against the nearest-thirds anchor — element never visually jumps. Fixed
anchor/origin pinnable in the sidebar.

### Types (dtx-layout)

```rust
pub enum AnchorSpace { Screen, Playfield }
pub enum Anchor9 { TopLeft, TopCenter, .., BottomRight }  // 9 variants

pub struct WidgetInstance {
    pub kind: WidgetKind,             // enum, serialized kebab-case string
    pub space: AnchorSpace,           // default Screen
    pub offset: Vec2,                 // ref-px
    pub anchor: Anchor9,
    pub origin: Anchor9,
    pub scale: f32,                   // uniform, default 1.0, clamp [0.25, 3.0]
    pub z: i32,
    pub visible_play: bool,
    pub visible_practice: bool,
    pub settings: BTreeMap<String, toml::Value>, // reserved, unused v1
}

pub struct ActiveLayout { pub widgets: Vec<WidgetInstance> } // Resource; v1: one per kind

pub struct DisplayLane {
    pub id: LaneId,                   // stable string: "hh", "sd", ...
    pub label: String,
    pub width: f32,                   // ref-px, clamp [24, 160]
    pub color: Option<Color>,         // None = theme default; reserved v1
}

pub struct LaneArrangement {          // Resource
    pub preset: LanePreset,           // Classic | XgA | XgB | Custom
    pub lanes: Vec<DisplayLane>,      // display order, variable count
    pub map: HashMap<EChannel, LaneId>, // all 12 channels mapped
}

pub struct WidgetSpawnCtx<'a> {
    pub theme: &'a Theme,
    pub assets: &'a AssetServer,
    pub effective_scale: f32,         // window ref-scale × instance.scale
}

pub struct WidgetDef {
    pub kind: WidgetKind,
    pub display_name: &'static str,
    pub default: WidgetInstance,      // current hardcoded position becomes this
    pub spawn: fn(&mut ChildSpawnerCommands, &WidgetSpawnCtx, &WidgetInstance),
}
pub struct WidgetRegistry(/* ordered map kind → WidgetDef */);
```

### File format

`~/.config/dtxmaniars/layout.toml` (XDG pattern copied from `dtx-config`):

```toml
version = 1

[[scene.gameplay.widgets]]
kind = "score-panel"
space = "screen"
offset = [16.0, 78.0]
anchor = "top-left"
origin = "top-left"
scale = 1.0
z = 10
visible_play = true
visible_practice = true

[lanes]
preset = "custom"          # or "classic", "xg-a", ...
order = ["LC","HH","HHO","LP","SD","HT","BD","LT","FT","CY","RD"]  # 11 = HHO split out
widths = { SD = 64.0, BD = 72.0 }   # only overrides listed
map = { HHO = "HHO" }               # channel→lane overrides; ids new to `order`
                                     # imply new lanes (label/color derived from channel)
```

**Rules:**
- File stores a full instance per *touched* widget; untouched widgets absent →
  registry default. "Reset widget" = delete its entry. "Reset scene" = delete all.
- Unknown `kind` / lane id / enum string → drop that entry, warn, keep rest.
- Missing/corrupt file → pure code defaults; never crash; never overwrite the
  user file until the next explicit save.
- `version` gates a migration chain run at load (see Migrations).

### Migrations (day-1 infra)

Loader: `raw toml → match version { 0 => migrate_0_to_1(..), .. } → parse latest`.
One unit test per migration. This is what makes v1 granularity decisions
(composite `score-panel`) reversible later: a future split ships as
`score-panel → score + accuracy + judge-counts` migration preserving positions.
Version newer than known: best-effort parse, warn, don't destroy on save without
confirm.

## Lane arrangement layer

Replaces compile-time `COLUMNS` / `LANE_ORDER` / `column_of()` consts in
`gameplay-drums/src/lane_geometry.rs`.

- **Merge** = several channels map to the same `LaneId` (today's HHO→HH, LBD→BD
  become data).
- **Split** = give a channel its own lane entry (e.g. HHO separate → 11 columns).
  Falls out of the model; no special case.
- **Judgment untouched** — stays per-channel. Only display x-position and input
  viz change.
- Presets = const tables in `dtx-layout`: `Classic` (current 10-col NX order),
  `XgA`/`XgB` (GITADORA XG orders from research notes). Every preset maps all 12
  channels. Editing anything flips preset to `Custom`.

**Refactor surface (gameplay-drums):**

| file | today | becomes |
|---|---|---|
| `lane_geometry.rs` | `COLUMNS`, `column_of`, `chip_color` | read `LaneArrangement` |
| `layout.rs` | fixed `STRIP_REF_WIDTH`, centered formula | strip width = Σ lane widths; strip origin from playfield widget instance |
| `scroll.rs` | note x/w from const column | arrangement lookup |
| `keyboard_viz.rs` | caps per const lane | iterate arrangement |
| `lane_flush.rs`, pad chips, hit effects | const lookup | same arrangement lookup |

One system: `LaneArrangement` changed → recompute `PlayfieldLayout` → existing
resize/reposition systems (notes already reposition on layout change) react.

## Anchor spaces

- `Screen` widgets anchor to the 1280×720 ref rect.
- `Playfield` widgets anchor to the strip rect, which is **dynamic** — moves and
  resizes with lane edits and playfield placement. `combo` and `judgment-popup`
  default to `Playfield` (they are strip-centered today); they track any lane
  change for free.
- The playfield itself is an ordinary `Screen`-space widget (see inventory) —
  movable/scalable, replacing the centered-by-formula placement.
- Editor: sidebar dropdown switches a widget's space (position re-expressed so it
  doesn't jump); drag guides come from the widget's own space.

## Widget inventory (gameplay scene, v1)

| kind | source today | notes |
|---|---|---|
| `score-panel` | `dtx-ui/widget/score_detailed.rs` | composite: score+caption+judge counts+accuracy+badge+fast/slow. One unit v1 (designed GITADORA box); future split via migration |
| `combo` | `perf_combo.rs` | default `space = playfield` |
| `judgment-popup` | `judgment_popup.rs` | placement = popup anchor point; default `space = playfield` |
| `phrase-meter` | `phrase_meter.rs` | |
| `song-progress` | `song_progress.rs` | |
| `now-playing` | `now_playing.rs` | |
| `live-graph` | `live_graph.rs` | |
| `speed-readout` | `playfield_speed.rs` | |
| `frame-chrome` | `frame_chrome.rs` | hide-only (not movable) — decorative chrome |
| `practice-transport` | `gameplay-drums/src/practice/ui.rs` | default practice-only; becomes anchor=bottom-center widget, same visual default |
| `playfield` | backboard + hit line + lanes + key caps | moves/scales as one block via `PlayfieldLayout` origin; lane internals edited via lane drag |

**Migration mechanics per widget:** widgets today spawn *multiple sibling
nodes* directly under `HudRoot` (frame chrome = bar + 2 pillars; score panel =
many absolute nodes). The editor needs one selectable root per widget, so each
widget is wrapped in a **single root node entity**: absolute-positioned by the
anchor math, sized `design w×h × effective_scale`, carrying the `WidgetKind`
marker and `ZIndex`. Children reposition **relative to the root** (their
current screen-absolute ref coords minus the widget's design origin) — a
mechanical per-widget change to every `spawn_*` fn. A change-detection system
recomputes the root's rect when an instance mutates (editor drag) → subtree
moves as one.

**Widget scale:** scaling a root node's px does *not* scale child text in
bevy_ui. Instead we reuse the existing ref-scale idiom: every spawn fn already
multiplies all px (and fonts) by a uniform scale — the registry passes
`effective_scale = window ref-scale × instance.scale`, so widget scale composes
into the existing math. Scale change → despawn + respawn the widget subtree
(cheap; HUD subtrees are small). During a handle-drag gesture the editor
previews with `Transform::scale` on the root, then respawns exactly on release.

## Editor overlay

**Invocation:**
- Performance: `Ctrl+Shift+E` toggles the editor over the live scene.
- SongSelect: `Ctrl+Shift+E` launches the selected chart with
  `AutoplayEnabled(true)` + editor open; chart loops (reuse seek engine:
  end-of-stage suppressed while editor open, seek back to 0 — same mechanism as
  practice A-B loop).
- While open: gameplay keyboard/pad input gated off (editor owns input); autoplay
  keeps playing. Editor exit returns to wherever invoked from.

**Input & pause gating:** a `LayoutEditorState` resource (open/closed) gates via
`run_if` conditions — drum lane input, pause toggle, and practice hotkeys are
disabled while open (we own all input systems; no osu-style input-manager hack
needed). `Esc` while the editor is open triggers the editor exit flow (with
unsaved-changes confirm), never the pause menu. `PauseState` stays `Running`.

**Hit-testing / drag:** bevy_picking with the UI backend (built into Bevy 0.19)
— `Pointer<Over/Out/Click/Drag/DragEnd>` observers on widget root nodes and lane
columns. No custom ray-casting.

**No viewport shrink** (osu shrinks the game into a rect; our HUD is per-widget
scaled with no single root). Side panels overlay the screen edges,
semi-transparent, collapsible.

```
┌──────────────────────────────────────────────────┐
│ [Save] [Undo] [Redo] [Mode: Play|Practice] [Exit] │  top strip
├────────────┬─────────────────────────┬────────────┤
│ Widgets    │                         │ Selected   │
│ ▸ score    │    live autoplay        │ score-panel│
│ ▸ combo    │    gameplay             │ offset x y │
│ ▸ progress │    (widgets outlined    │ anchor ⚓  │
│ ▸ ...      │     on hover, drag)     │ space ▾    │
│ Lanes      │                         │ scale 1.0  │
│ preset ▾   │                         │ z, vis ☑☑  │
│ (collapse) │                         │ [reset]    │
└────────────┴─────────────────────────┴────────────┘
```

**Mouse interactions:**
- Hover → outline + name tag. Click → select (or click sidebar list entry).
- Drag body → move (offset). Corner handles → uniform scale.
- Anchor auto-snaps to nearest-thirds during drag unless pinned in sidebar.
- Alignment guides: screen/playfield thirds + center + other widgets' edges.
- Lane columns: drag body → reorder (swap on crossing midpoint), drag edge →
  width. Preset dropdown in sidebar.
- Hidden widgets render ghosted in the editor (else unselectable).
- Keyboard nudge: arrows = 1px, Shift+arrows = 8px.

**Mode toggle:** Play/Practice switch previews the corresponding visibility set
(practice widgets appear/ghost). Practice-only widgets (transport strip) preview
fine outside a real practice session: spawn creates the structure and the sync
systems simply idle without `PracticeSession`, so the preview shows zeroed
values — acceptable ghost.

**Undo:** snapshot stack of `(ActiveLayout, LaneArrangement)` clones pushed per
committed gesture (drag-end, toggle, preset switch, nudge batch). Ctrl+Z /
Ctrl+Shift+Z.

**Save/exit:** Ctrl+S or Save writes `layout.toml`. Exit with unsaved changes →
confirm (save / discard / cancel).

## Error handling

- Corrupt/missing file → code defaults; warn; never crash; never clobber.
- Unknown kind / lane id / enum value → drop entry, warn, keep rest.
- Channel missing from `map` → Classic mapping fallback for that channel.
- Off-screen widgets clamped on save (≥16px visible); editor red-tints
  out-of-bounds.
- Clamps: scale [0.25, 3.0]; lane width [24, 160] ref-px.
- File version newer than known → best-effort parse + warn.
- Save I/O failure → error toast; layout stays in memory; retry on next save.

## Testing

- **dtx-layout unit:** serde round-trip (full + minimal file); defaults merge;
  unknown-kind drop; migration chain; anchor math table (anchor × origin ×
  space); clamps; preset completeness (every `EChannel` mapped in every preset).
- **gameplay-drums integration:** spawn HUD from registry with a custom layout →
  assert node positions; mutate instance at runtime → node follows;
  `LaneArrangement` change → `PlayfieldLayout` recomputed and playfield-space
  widgets reposition; split-HHO arrangement → 11 columns and notes land in the
  right column; visibility flags respected in play vs practice.
- **Schedule guard:** extend `tests/fixed_update_schedule_ordering.rs` with any
  new FixedUpdate ordering edges (project gotcha: green tests don't prove the
  real plugin schedule builds).
- **Editor logic:** pure functions tested (hit-test, drag→offset math,
  nearest-thirds re-express with no-jump invariant — screen position before ==
  after, undo stack, lane reorder swap); UI systems stay thin.
- **Manual checklist:** drag feel, handles, guides, hover outlines, save/reload
  persistence round-trip in-game, editor responsiveness over live autoplay.

## Decisions log

- Both halves (lanes + editor) in one spec — user choice.
- Gameplay scene only in v1; format reserves scene keys.
- Approach A ("layout resource + registry") with B-shaped contracts
  (instance list, construct-from-data, reserved settings/scenes, versioned) so
  full osu-style coverage (multi-instance, add/remove, settings sidebar, more
  scenes) is additive, never a rewrite.
- Mouse-first editing (user request), osu skin editor as interaction reference.
- Hotkey overlay over live autoplay (osu `EndlessPlayer` analogue via existing
  autoplay + seek loop).
- Single active layout file; profiles later.
- Play/practice visibility tags in v1.
- Two anchor spaces (screen/playfield) instead of "playfield special case" —
  needed because variable lanes make the strip rect dynamic.
- `score-panel` stays composite in v1; migration infra from day 1 makes a future
  split safe.
- No rotation in v1 (bevy_ui limitation).
- Each widget wrapped in a single root node (selection/drag unit); children
  root-relative. Widget scale composes into the existing ref-scale multiply +
  respawn on commit (child text can't be scaled via node px).
- `WidgetSpawnCtx` bundles theme/assets/scale so the registry fn signature
  survives future widget needs.
- Editor gating via `LayoutEditorState` + `run_if` (we own input systems);
  hit-testing via bevy_picking UI backend.
