# Customize Surface — Visual Punchlist (tackle AFTER Phase 3)

User smoke 2026-07-08 (Opus session). Chrome structure works but visuals diverge badly from the prototype artifact. Deferred by user ("we need to tackle this after"). Fix once Phase 3 (bindings + MIDI) lands.

## STATUS (2026-07-08): P0 ✅ (`08eeb71`), P1 ✅ (`1679d2f`), P3 topbar ✅ REMOVED (`f285ac7`). BRP loop now working (launch dtxmaniars from worktree, Ctrl+Shift+E opens surface, click at logical coords = physical/1.65). P2 widget-bleed + aesthetic still open.

### BRP-verified this session (`f285ac7`)
- Topbar deleted → garbled top-left overlap gone.
- Bindings: click a channel row to select (was spawned-but-never-queried → un-selectable). Row highlights, lane outlines on playfield, source labels draw at pad bottom, selection HOLDS.
- Bindings: autoplay no longer drives selection (was chasing the judged note). Real MIDI NoteOn still auto-selects (spec §5).

### P2 FIXED (`3f66805`..`a0a292c`) — single HudRoot transform (osu SetCustomRect)
Abandoned the per-widget-StageRect route (whack-a-mole). Adopted the artifact's model: the WHOLE scene (playfield + every HUD widget, all children of HudRoot) rides ONE uniform `UiTransform` on HudRoot. PlayfieldLayout now always full-window; shrink = the transform (`stage_rect::stage_xform`/`apply_stage_transform`). preset_rect: settings tabs shift full playfield into the gap (scale 1, HUD hidden via P0); kit tabs shrink the whole screen into the gap (inspector reserved only on Widgets+selection). bindings_spatial overlay reparented under HudRoot. Drag divides Δ by pfl.scale*stage_scale. Rounded StageOutline frames the miniature. **BRP-verified: normal play identity; Widgets miniature with HUD contained; Bindings overlay glued; settings shift clean.** 1304 workspace tests pass.

### Dim preview DONE (`9ca7be1`) — user chose "dim the preview" + "keep the domes"
A full-window scrim at `GlobalZIndex(1500)` (above all HUD incl. the GlobalZIndex combo, below chrome 2000 + outline 1900) at `srgba(0.02,0.024,0.035,0.72)`. First tried a scrim as a HudRoot CHILD (local z) — failed: GlobalZIndex HUD (combo) escaped it. Top-level GlobalZIndex scrim covers everything uniformly. Spawned/despawned with the bounds outline; hidden on Tab-peek; absent in normal play. BRP-verified: dimmed calm miniature open, full-brightness identity closed. Tune darkness via the one `BackgroundColor` alpha constant in `spawn_outline_on_open`.

Domes kept (user's call) — not swapped for the artifact's flat labels. Adjust the scrim alpha if the user wants more/less dim.

### Remaining aesthetic (subjective — get user steer)
- Dim the whole preview (artifact is dim/translucent; impl is full brightness).
- Pad domes (LC/HH/SD arches) vs artifact flat thin lane labels — domes are core playfield render, gate on preview-mode, don't delete.
- Widgets-tab: many real HUD widgets (density strip, live graph, judgement breakdown) vs artifact's few — clutter is inherent to our richer HUD.

## P0 — Settings preview shows the WHOLE HUD; should show only lanes + notes — ✅ FIXED `08eeb71`

**User:** "why does it always show the full game window like layout editor even when i am not in layout editor setting? it should just show only the lane and notes, no need for other things."

On SETTINGS tabs (Gameplay/Audio/Drums/System) the live preview renders the entire HUD — score panel (80.00% / BASIC / SPEED / SKILL / MaxCombo text at window-left), the note-density histogram strip, combo, live graph (green, far right), etc. — cluttering the preview and bleeding under the chrome.

**Intended (design refinement):** settings tabs preview = **lanes + notes ONLY** (clean minimal playfield, no HUD widgets). Full HUD only on the **Widgets** tab (that's where you edit widget layout). Likely implementation: on non-Widgets tabs, hide all HUD widget entities except the playfield (lanes + notes + judge line); show them on the Widgets tab. This also sidesteps P1 for settings tabs.

## P1 — Stage transform doesn't shrink all widgets (corner-anchored widgets bleed into chrome) — ✅ FIXED `1679d2f`

The Fit preset shrinks the playfield/combo into a central band, BUT screen-corner-anchored widgets (score panel top-left, skill bottom-left, live graph right, note-density strip far-left) render at FULL WINDOW edges — overlapping the left settings panel + right inspector. So the "stage rect" isn't actually containing all widgets. Either `apply_widget_layout`'s Screen-anchor path isn't applying `StageRect.origin` for corner placement, or some HUD elements render via a system that bypasses `apply_widget_layout`/`PlayfieldLayout` (i.e. still read raw window size — the 2b "completeness" risk). Investigate which widgets bypass StageRect and route them through it. (P0's hide-on-settings-tabs makes this only matter for the Widgets tab.)

## P2 — Widgets (layout editor) tab doesn't match the prototype

Prototype (artifact screenshots 4-5): clean shrunk miniature centered in the gap between left panel and right inspector, whole screen visible with a thin bounds outline, widget list in the LEFT panel (Score/Combo/Gauge/Judgement/...), selected widget → right inspector (ANCHOR 3×3 + Offset X/Y sliders + Scale slider + Z + In-play/In-practice toggles + Reset Widget). Impl (screenshot 3): game not cleanly shrunk, widgets overlap, three vertical band outlines instead of one screen-bounds rect, inspector present but the miniature is broken. Needs the P1 fix + the Fit rect + a single screen-bounds outline (not the per-column/3-band artifact currently drawn).

## P3 — Topbar text overlap

`CUSTOMIZE ▸ <song> · BPM <n>` overlaps the score-panel text bleeding through from the preview (because widgets render at window-left under the topbar). Fixed largely by P0/P1 (hiding/shrinking widgets). Also verify the topbar has an opaque background above the stage.

## P4 — Prototype polish still missing (lower priority)

- Search box was intentionally DROPPED (spec) — prototype shows it but we don't build it.
- Right inspector sliders (Offset X/Y, Scale) vs impl steppers — prototype uses sliders.
- Settings sliders styling (orange fill knob) vs impl — cosmetic.
- Modified dots live-update; focus-row highlight; RESET TAB confirm.

## Root-cause hypothesis

The surface reuses the live autoplay HUD as-is. The prototype treats the preview as a *controlled miniature*: settings = minimal lanes+notes, kit = full shrunk screen. The fix is a **preview-mode** concept driven by ActiveTab: `Minimal` (lanes+notes, hide HUD) for settings/bindings tabs, `Full` (all widgets, shrunk via StageRect) for Widgets tab, `Fit`-shrunk for Lanes. Implement a `PreviewMode` resource + a system that toggles HUD widget visibility by tab, and finish routing every widget through StageRect for the Full mode.
