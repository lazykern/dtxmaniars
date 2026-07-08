# Customize Surface — Visual Punchlist (tackle AFTER Phase 3)

User smoke 2026-07-08 (Opus session). Chrome structure works but visuals diverge badly from the prototype artifact. Deferred by user ("we need to tackle this after"). Fix once Phase 3 (bindings + MIDI) lands.

## STATUS (2026-07-08): P0 ✅ FIXED (`08eeb71`), P1 ✅ FIXED (`1679d2f`). P2/P3/P4 pending user re-smoke.

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
