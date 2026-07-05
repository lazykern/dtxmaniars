# Drums Performance HUD — GITADORA Flat Redesign

**Date:** 2026-07-05
**Status:** Design (approved via visual companion session; supersedes the visual/panel half of `2026-07-05-drums-hud-nx-fidelity-design.md`)

## Problem

The current performance HUD reads poorly (screenshot 20260705_130531): scattered panels with dead space, debug-looking white-bordered stats box, thin low-contrast notes, no coherent visual language with the redesigned GITADORA-style menus.

The prior NX-fidelity spec fixed the *mechanics* (10 visual columns, HHO/LBD folding, variable widths) but copied DTXManiaNX's dated look 1:1. The user chose an original design instead: GITADORA flat-lane layout (DrumMania "flat" view) with the panel language already used by the new Title/Song-Select/Settings screens.

## Approved Decisions (visual companion session)

1. **Direction:** GITADORA Flat Hybrid — GITADORA plate/panel language around a flat vertical strip. No perspective lane.
2. **Layout:** Mirror the GITADORA DrumMania flat-lane screen: full live stats left, song card + phrase meter + live graph right, strip centered.
3. **Chips:** Flat color bars, no white cap, no gloss.
4. **Merged secondaries:** Hollow outline = secondary. HHO = hollow HH-cyan outline, LBD = hollow BD-orange outline. Filled = primary (HH closed, right BD).
5. **Live graph:** Included now (not deferred).
6. **Phrase meter:** Unlabeled density blocks (BocuD port in `phrase.rs`). `.dtx` has no section names — no fake "Intro/Chorus" labels.

## What Carries Over from the NX-Fidelity Spec

Mechanics core stays exactly as specified there (and `lane_geometry.rs` already exists):

- `lane_geometry.rs`: `COLUMNS` table (10 columns, labels LC HH LP SD HT BD LT FT CY RD, NX-derived proportional widths), `column_of(EChannel)`, `chip_color(channel)`, per-column colors (LC `#cc44cc`, HH `#33bbee`, LP `#ff66aa`, SD `#ffdd33`, HT `#ff5555`, BD `#ff8833`, LT `#55dd55`, FT `#3388ff`, CY `#dd66ff`, RD `#66ddcc`).
- 12-lane `LaneId`/`EChannel` model for input/judge/scoring — untouched. Only rendering consults columns.
- `note_height`: 14 ref px; chip fills `col_width - 4`.

**Change vs old spec:** the strip is **centered**, not left-anchored. Total strip width stays 558 ref px; left edge moves 295 → **361** (x 361..919 at 1280×720). `layout.rs` gains a single `STRIP_LEFT` constant; column `ref_x` values become offsets from it (or are shifted by +66).

## Layout (1280×720 reference)

```
┌────────────────────────────────────────────────────────────────────┐
│ pillar│  LEFT PANELS   │███ STRIP x361..919 ███│  RIGHT PANELS │pillar
│       │ SCORE plate    │  faint column tints   │ TIME plate    │
│       │ SCORE DETAILED │  measure lines        │ SONG CARD     │
│       │  Perfect..Miss │      128              │  jacket/diff/ │
│       │  MaxCombo      │     COMBO             │  NOW PLAYING  │
│       │  Fast/Slow     │                       │ PHRASE │ LIVE │
│       │ OPTIONS(SPEED) │  chips (flat bars)    │ METER  │ GRAPH│
│       │ Completion %   │  PERFECT (popup)      │ blocks │ bars │
│       │ Skill plate    │ ══ hit line (yellow)══│ cursor │ S/A/B│
│       │                │ [pad row, color rims] │        │      │
└────────────────────────────────────────────────────────────────────┘
```

### Center strip

- Lane background near-black (`#070709`-equivalent theme color) over the ambient stage background; column tints at ~5% alpha of each column color, ending at the pad row.
- Measure lines: existing `beat_lines.rs`, restyled dim gray, spanning strip width.
- Hit line: 2.5 ref px yellow (`select_yellow` from theme) + glow (`BoxShadow`).
- Chips: solid `chip_color(channel)` bars, `BorderRadius` 2px. Secondary channels (HiHatOpen, LeftBassDrum) render as 2 ref px border outline, transparent fill, same column color.
- Lane flash on hit: vertical gradient (transparent → column color ~22% alpha) from mid-strip to hit line, quick fade. Retarget existing `ReceptorFlash`/burst systems to columns.
- Judgment popup: existing widget, centered on strip, above hit line, judgement-colored italic text.
- Combo: GITADORA style — centered in strip upper third. Big white monospace digits, small yellow letter-spaced `COMBO` label below, scale-pop on increment (existing `perf_combo` mechanics, repositioned).
- Pad row below hit line: one pad per column, dark fill (`#1c1c22`-equivalent), 2 ref px border in column color, rounded-top shape (cymbals/toms more rounded, pedals squarer). Label text only (LC/HH/…), no keybind text. Border+fill brighten ~150ms on hit.

### Left column (top→bottom)

1. **SCORE plate** — italic `SCORE` label + 7-digit zero-padded rolling number. Plate style: dark bg, thin border, yellow left accent bar.
2. **SCORE DETAILED panel** — light tab header ("SCORE DETAILED"), monospace rows: Perfect/Great/Good/Ok/Miss with count + percent, judgement colors from theme; MaxCombo row; `Fast n / Slow n` footer (cyan/orange). Reuses `score_detailed` widget, restyled — no white debug border.
3. **OPTIONS plate** — tab header, `SPEED x.x` (existing `playfield_speed`), room for AUTO flag when active.
4. **Completion Rate plate** — accuracy percent, large white digits.
5. **Skill plate** — live skill value, teal (`clear_green`/skill color from theme).

### Right column (top→bottom)

1. **TIME plate** — remaining time `mm:ss`, monospace green (existing `song_progress` data).
2. **SONG CARD** — jacket thumbnail (preimage, fallback dark square), difficulty badge (`DRUM <label> <level>` pink-bordered), title, artist, yellow `◀ NOW PLAYING` tag. Reuses `now_playing` widget, restyled.
3. **PHRASE METER** (narrow, tall) — existing `phrase_meter` widget: 64 density blocks, variable width, played portion tinted, current-position cursor. No section labels.
4. **LIVE GRAPH** (adjacent to phrase meter, tall) — **new widget `live_graph`**:
   - Fixed 128-slot buffer indexed by song position: `slot = song_pos / total * 128`. On each judged chip, write current accuracy % into the slot (later writes overwrite — slot holds the latest accuracy at that point in the song).
   - Each sample = one thin vertical bar, height ∝ accuracy, cyan.
   - Horizontal threshold lines with right-edge labels at S=95 / A=85 / B=70 / C=50 (`dtx_scoring::Rank` boundaries — no SS rank exists).
   - Empty (no bars) before first judged chip.

### Frame

- Background: existing ambient `stage_background`, gameplay-dimmed.
- Side pillars: two dark vertical bars just outside the strip with subtle chevron marks (`frame_chrome` widget grows this, or static nodes in `hud.rs`).
- Delete: debug keybind row, white stats border, current top-left SCORE/COMBO placement.

## Data Flow

```
chart.chips ─► spawn_notes ─► column_of(channel) ─► col geometry ─► flat bar
                                   │                       (hollow if HHO/LBD)
HitEvent{lane} ─► column_of ─► pad brighten + lane flash (column color)
              └► judgment popup (strip center) + combo pop (strip center)
score state ──► score plate / detailed rows / completion / skill (existing)
song clock ───► time plate, phrase cursor, live_graph sample tick
accuracy() ───► live_graph ring buffer ─► bar heights
```

## Testing

- `lane_geometry`: existing tests keep passing; update geometry tests for centered strip (`col_left(0) == 361*scale`, `col_left(9)+col_width(9) == 919*scale`).
- Chip style: unit test `is_hollow(channel)` true only for `HiHatOpen`, `LeftBassDrum`.
- `live_graph`: sample bucketing (index = pos/total*128 clamped), accuracy mapping to bar height, threshold constants match `dtx_scoring::Rank` boundaries via test against `Rank::from percentage` behavior.
- Existing judge/lane_map/hud tests unaffected (12-lane model untouched). Stale NX tests (`lane_strip` left-anchor expectations) updated.
- Manual: play a chart — verify column order LC..RD, hollow HHO/LBD, combo pop center, pad rim flash, graph fills left→right, phrase cursor tracks.

## Out of Scope

- Perspective lane, textures/sprite art, movie background.
- Section-name detection heuristics for phrase meter.
- Player-info panel (no profiles), arcade CREDIT footer.
- Results screen redesign (separate effort).
- RD/CY position variants; reverse scroll.

## Risk / Notes

- Node budget: live graph ≤128 bars + 4 lines, column tints 10, pads ~30 — trivial for Bevy UI.
- `perf_hotkeys.rs` stats-toggle idea dropped (full stats always on, per user).
- Old NX spec remains in repo for the mechanics tables; its layout/panel sections are superseded by this document. The 32K NX plan (`2026-07-05-drums-hud-nx-fidelity.md`, 0/31 steps done) should be regenerated against this spec before execution.
