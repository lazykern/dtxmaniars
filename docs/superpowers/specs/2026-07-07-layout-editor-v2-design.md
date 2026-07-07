# Layout Editor v2 — osu!-Style Direct Manipulation

**Status:** Approved design, ready for planning
**Builds on:** `2026-07-07-layout-editor-design.md` (v1, shipped) — all v1 architecture
(dtx-layout crate, `WidgetLayouts`/`Lanes` resources, `WidgetContainer`, editor module,
undo/save) is unchanged foundation. v2 is additive UI on top.
**References:** `references/osu-lazer/osu.Game/Overlays/SkinEditor/` —
`SkinBlueprint.cs` (per-component selection blueprint), `SkinSelectionHandler.cs`
(drag + anchor rewrite), `SkinSelectionScaleHandler.cs`, `SkinSettingsToolbox.cs`
(right settings panel), `SkinEditorSceneLibrary.cs` (scene entry buttons);
`references/osu-lazer/osu.Game/Screens/Edit/Compose/Components/SelectionBox*.cs`
(selection box + handles).

## Goal

Close the UX gap between v1 (sidebar-first, blind drag) and the osu! skin editor
(direct manipulation): click widgets on the canvas, get a selection box with handles,
edit values in a per-widget settings panel, auto-snap anchors while dragging, manage
lanes (reorder / width / group / ungroup) in a panel, and enter the editor as a
dedicated session from the title screen.

Explicitly OUT of scope (deferred, all additive):

- Hit-group (judgment) UI — `DrumsConfig` stays config-file-only. Display lane
  grouping is IN scope and is a different axis (drawing position, not judgment).
- Rotation handles, non-uniform scale/stretch.
- Scene tabs / editing song-select or title scenes.
- Live rendered previews in the widget list (needs render-to-texture).
- Channel→lane map free-form editing beyond split/merge (the `[lanes.map]` TOML
  stays hand-editable for exotic mappings).
- Playfield moving/scaling as a widget (needs `PlayfieldLayout` origin offset).
- Named layout profiles, File/Edit menu bar.

## Big Picture

```
                  ┌─ Performance screen (editor open) ──────────────────────────┐
                  │                                                             │
┌─ left sidebar ─┐│        canvas (live autoplay)         ┌─ right panel ──────┐│
│ widget list    ││                                       │ Settings (Combo)   ││
│ (click=select) ││  hover widget → outline + name label  │ ┌─────┐            ││
│                ││  click → select                       │ │ 3×3 │ anchor     ││
│ actions:       ││   ┌─────────────────┐◄─ selection box │ └─────┘            ││
│ save / undo /  ││  ▣┤  COMBO    42x   ├▣ corner scale   │ offset X  [−][+]   ││
│ redo / reset / ││   └─────────────────┘   handles       │ offset Y  [−][+]   ││
│ close          ││  ▣  name tag        ▣                 │ scale   ───●───    ││
└────────────────┘│   anchor line ╲                       │ z         [−][+]   ││
                  │   to origin dot ●                     │ ☑ play  ☑ practice ││
                  │                                       │ ↺ per-row reset    ││
                  │  drag → move; anchor auto-snaps to    ├────────────────────┤│
                  │  nearest ninth (dashed guide lines)   │ Lanes (Playfield   ││
                  │                                       │        selected)   ││
                  │                                       │ preset ◄ classic ► ││
                  │                                       │ [▲][▼] HH (HH·HHO) ││
                  │                                       │        width ──●── ││
                  │                                       │ ...                ││
                  └───────────────────────────────────────┴────────────────────┘│
                  └──────────────────────────────────────────────────────────────┘
```

Single mutation path preserved from v1: every gesture/knob mutates `WidgetLayouts`
or `Lanes`; `apply_widget_layout` / `PlayfieldLayout` react via `resource_changed`;
`UndoStack` snapshots on gesture start; Ctrl+S / Save button persists `layout.toml`.
Nothing new writes to disk directly.

## Components

All new editor code lives in `crates/gameplay-drums/src/editor/` next to the v1
modules (`mod.rs`, `drag.rs`, `undo.rs`, `save.rs`, `ui.rs`). Reusable UI controls
go in `crates/dtx-ui`.

### 1. `editor/picking.rs` — on-canvas hit-testing

No `bevy_picking` on gameplay nodes (v1 containers carry `Pickable::IGNORE` and the
canvas is not a UI interaction surface). Manual hit-test instead:

- Each frame while the editor is open, compute a content AABB per widget: union of
  the `WidgetContainer`'s descendants' `ComputedNode` rects (via `GlobalTransform` +
  `ComputedNode.size()`), scaled to physical→logical as needed.
- The Playfield gets its AABB from `PlayfieldLayout` (strip origin + width/height).
- `Hovered(Option<WidgetKind>)` resource: topmost AABB under `Window::cursor_position()`.
  Priority: higher `z` first, ties broken by smaller AABB area (small widget on top
  of a big one stays grabbable). Alt+click cycles through all candidates under the
  cursor (osu parity).
- Click (left press, not over panel/sidebar/selection-handle) sets
  `Selection(Some(kind))`. Click on empty canvas clears selection.
- Order of input precedence per frame: settings panel / sidebar (Bevy UI
  `Interaction`) → selection-box handles → canvas widgets. The panel and sidebar
  AABBs mask the canvas: cursor over them ⇒ no canvas hover.
- Hidden-in-current-mode widgets (`visible_play=false` while previewing play) are
  not hit-testable on canvas but stay selectable from the left list.
- Empty AABB fallback (e.g. judgment popup between hits): last non-empty AABB is
  cached per widget; minimum grab box 24×24 at the widget's resolved position.

### 2. `editor/selection_box.rs` — selection overlay

A UI overlay (child of the editor UI root, `GlobalZIndex` above the sidebar) that
tracks the selected widget's AABB every frame:

- 1px accent border around the AABB; dashed styling when the widget is hidden in
  the current mode.
- Name tag (widget `display_name`) attached to the box edge.
- Anchor visualization: dot at the widget's origin point, line from the origin dot
  to the anchor point on its parent space (screen or playfield strip) — the osu red
  line. Drawn with `bevy::ui` nodes (line = thin rotated node) or gizmos; gizmo layer
  is acceptable since this is editor-only.
- Four corner scale handles (▣ 10×10). Dragging a handle scales uniformly: scale
  factor = (distance from AABB center to cursor) / (distance at drag start), applied
  to the gesture-start scale, clamped to `MIN/MAX_WIDGET_SCALE` (0.25–3.0). Handles
  hit-test before canvas widgets.
- Playfield selected: box + name tag shown, no move (drag disabled) and no scale
  handles in v2; the lane panel is the Playfield's editing surface.

### 3. `editor/snap.rs` — anchor auto-snap while dragging

osu behavior (`SkinSelectionHandler.ApplyClosestAnchorOrigin`): while a widget drag
is in progress, the anchor follows the widget.

- Compute the widget AABB center as a fraction of its parent space (screen for
  `AnchorSpace::Screen`, playfield strip for `AnchorSpace::Playfield`).
- Nearest ninth: each axis independently snaps to Start (<1/3), Center (1/3–2/3),
  End (>2/3) → one of the 9 `Anchor9` variants.
- When the nearest anchor differs from the current one: rewrite `anchor` (origin
  follows anchor, matching v1's convention) and recompute `offset` so
  `resolve_top_left(...)` yields the identical position — the widget must not jump.
  This invariant is unit-tested.
- While dragging, draw dashed guide lines at the parent-space thirds (both axes)
  and highlight the currently-snapped anchor point.
- Manual anchor choice from the 3×3 grid in the panel does NOT re-enable auto-snap
  retroactively; auto-snap only runs during canvas drags (osu parity: closest-anchor
  is the default mode, explicit anchor pins it). A small "auto" toggle cell on the
  3×3 grid switches back to closest-anchor mode. `WidgetInstance` gains
  `anchor_auto: bool` (default true; serialized, default-skipped so v1 files load
  unchanged).

### 4. `dtx-ui` controls — `crates/dtx-ui/src/widget/controls.rs`

Bevy UI has no built-in form controls. Minimal reusable set (editor now, settings
screens later):

- `slider(min, max, value)` — horizontal track + handle, drag to set; emits changed
  value via component marker read by the caller's system.
- `stepper(value)` — `[−] value [+]` row; click = ±step, Shift+click = ±10×step.
- `toggle(bool)` — checkbox-style.
- `grid3x3(selected: Anchor9, auto: bool)` — anchor picker; nine cells + "auto" cell.
- `icon_button` — small square button (▲ ▼ ✕ ↺ ◄ ►) reused across panel rows.

All follow the existing `dtx_ui::theme::Theme` styling and the `EditorButton`-style
marker-component + `Interaction` pattern from v1 `ui.rs`.

### 5. `editor/panel.rs` — right settings panel

260px right sidebar, spawned/despawned with the editor (same lifecycle as v1 left
sidebar). Content rebuilt when `Selection` changes (`resource_changed::<Selection>`).

Generic block (any widget selected):

| Row | Control | Writes |
|-----|---------|--------|
| Anchor | `grid3x3` (+auto cell) | `inst.anchor` (+origin), `inst.anchor_auto` |
| Offset X / Y | `stepper` (step 1, Shift 10) | `inst.offset` |
| Scale | `slider` 0.25–3.0 | `inst.scale` |
| Z | `stepper` (step 1) | `inst.z` |
| Visible in play / practice | 2× `toggle` | `inst.visible_play/practice` |
| Per-row ↺ | `icon_button` | that field → `default_instance(kind)` value |

Every discrete change pushes one undo snapshot (steppers/toggles/grid: per click;
slider: on drag start, like canvas drags).

Lane block (Playfield selected only), replaces the generic block:

- Preset row: `◄ classic ►` cycles named presets; any manual lane edit below flips
  the arrangement to `custom` (resource-side, mirrors `LanesSection` semantics).
- One row per display lane, in display order:
  `[▲][▼]  <ID>  (<chip·chip·…>)  width ──●──  [✕]`
  - `▲▼` reorder (swap with neighbor; mutates lane order).
  - Chips = channel short names mapped to this lane (primary first). Clicking a
    non-primary chip splits that channel out: new single-channel lane inserted
    directly after this lane, channel remapped to it. (Ungroup: HH lane `(HH·HHO)`
    → click `HHO` → own HHO lane.)
  - Width slider: `MIN_LANE_WIDTH..MAX_LANE_WIDTH` (24–160 ref px).
  - `✕` merges this lane away: all channels mapped here remap to the nearest
    surviving neighbor lane (prefer left), lane removed from order. Disabled when
    it is the last lane. (Group: kill RD lane → RD chips draw on CY lane.)
- All lane mutations go through `Lanes` (same `UndoStack` snapshot covers them, as
  in v1) and round-trip through `LanesSection::from_arrangement` on save.

Left sidebar slims down to: widget select list + actions (Save / Undo / Redo /
Reset All / Close). "Reset Widget" and "Next Lane Preset" move into the panel.

### 6. Editor session — dedicated entry (osu scene-library equivalent)

New: enter the editor deliberately from the title screen instead of only mid-song.

```
Title ──Enter──► SongSelect ──► SongLoading ──► Performance (normal)
  │                                                ▲ Ctrl+Shift+E kept: quick
  └──F2──► EditorSession(true)                       tweak overlay mid-song
              │
              ├─ song pick: config.gameplay `last_played` path if it still exists,
              │  else random from SongDb (lazy-scan default dir if empty — same
              │  logic as song-select's ensure_song_db_loaded)
              ▼
        SelectedSong set ──► SongLoading ──► Performance in editor mode:
              · autoplay forced on, EditorOpen=true from the first frame
              · song end ⇒ seamless restart of the same chart (no stage_end
                results transition while EditorSession is true)
              · Esc (with no selection active) ⇒ close editor, transition to
                Title — never to Results
```

Pieces:

- `EditorSession(pub bool)` resource in `game-shell` (both `game-menu` and
  `gameplay-drums` need it). Default false.
- Title screen: `F2 LAYOUT EDITOR` hint line (styled like the existing `ESC QUIT`),
  F2 handler sets `EditorSession(true)`, picks the song, sets `SelectedSong`,
  requests `SongLoading`.
- `last_played`: new optional `gameplay.last_played: Option<PathBuf>` in
  `dtx-config`, written on every normal Performance entry (song-loading already
  knows the path). Validated (`Path::exists`) before use.
- `stage_end`: gated — while `EditorSession` is true, instead of transitioning to
  Results, restart the chart (reuse the practice-mode/system path that rebuilds the
  performance for the same `SelectedSong`; if no cheap in-place restart exists,
  re-request `SongLoading` for the same song).
- Esc handling: first Esc with a selection active deselects; Esc with nothing
  selected closes the overlay (v1 behavior), and in session mode additionally
  requests transition to Title and clears `EditorSession`.
- Ctrl+Shift+E mid-song behavior is untouched (overlay opens, Esc returns to the
  running song).

## Input Precedence (one frame, editor open)

```
1. Bevy UI Interaction (panel + sidebar buttons/sliders)   — masks everything below
2. Selection-box handles (manual AABB test)
3. Canvas widget hover / click / drag (manual AABB test)
4. Hotkeys: arrows nudge, Ctrl+Z/Y, Ctrl+S, Esc, Alt+click cycle
```

`editor_closed` gating from v1 stays mandatory for all gameplay hotkey systems
(perf_hotkeys gotcha).

## Persistence & Compatibility

- `layout.toml` schema change: `WidgetInstance.anchor_auto: bool` (serde default
  true, skip-if-default). v1 files parse unchanged; `LATEST_VERSION` stays 1 (pure
  additive field, no migration needed).
- `config.toml`: `gameplay.last_played` optional path (serde default None).
- Lane split/merge/reorder/width serialize through the existing
  `preset="custom"` + `order` + `widths` + `map` form — no schema change.

## Error Handling

- Hit-test with no widgets rendered yet (first frames): AABB cache empty → no hover,
  no crash; canvas clicks clear selection.
- `last_played` missing/moved: fall back to random; SongDb empty after scan: show
  the existing "no songs" path (title F2 does nothing but log a warning — no crash).
- Lane merge of the last lane: `✕` disabled; resolver invariant (every channel
  mapped) already enforced by `LanesSection::resolve` repair logic.
- Slider/stepper writes clamp at the same constants the file loader clamps at
  (`MIN/MAX_LANE_WIDTH`, `MIN/MAX_WIDGET_SCALE`).

## Testing

- Unit (pure fns): AABB priority ordering (z, then area; alt-cycle sequence),
  nearest-ninth classification, anchor-rewrite no-jump invariant
  (`resolve_top_left` identical before/after), scale-handle factor math + clamp,
  stepper/slider value math, lane split/merge/reorder transforms (channel map
  repaired, order correct, preset flips to custom).
- Integration (headless App): simulated cursor + injected AABBs → hover/select;
  panel button press → `WidgetLayouts` mutation → undo restores; lane row actions
  → `Lanes` mutations; `EditorSession` song-end loops instead of Results; Esc in
  session → Title requested.
- Real-binary launch check per plan merge (schedule-cycle gotcha — hand-wired test
  apps don't prove the real plugin schedule builds).
- Manual checklist (needs display): drag feel, snap guide visuals, handle grab
  accuracy, panel styling, session entry flow.

## Plan Split (~4 plans)

1. **On-canvas selection** — picking.rs (AABB, hover, click, alt-cycle),
   selection_box.rs (border, name tag, anchor line, corner scale handles),
   input precedence. Editor becomes direct-manipulation.
2. **Settings panel** — dtx-ui controls (slider/stepper/toggle/grid3x3/icon_button),
   panel.rs generic block, left sidebar slim-down, per-row resets, undo wiring.
3. **Lane panel** — Playfield selection block: preset cycle, reorder, width sliders,
   chip split, lane merge; `Lanes` transform fns + tests.
4. **Anchor snap + editor session** — snap.rs (nearest-ninth live rewrite, guides,
   `anchor_auto` field), title F2 entry, `EditorSession`, `last_played`, song-end
   loop, Esc-to-title.

Each plan lands independently behind the existing editor toggle; no plan breaks v1
behavior when its feature is unused.
