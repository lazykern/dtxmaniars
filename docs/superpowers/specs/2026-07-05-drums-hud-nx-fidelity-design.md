# Drums Performance HUD — DTXManiaNX Fidelity Pass

**Date:** 2026-07-05
**Status:** Superseded — layout/panel/visual sections replaced by `2026-07-05-drums-hud-gitadora-flat-design.md`; mechanics core (lane_geometry, column mapping, colors) still authoritative there.

## Problem

Current drums performance HUD (screenshots 20260705_003536 / _003653) reads poorly:

1. **Lane order wrong.** `lane_map.rs` claims to match BocuD `CActPerfDrumsLaneFlushD.cs` but renders `HH SD BD HT LT FT CY HHO RD LC LP LBD`. NX order is `LC HH LP SD HT BD LT FT CY RD`.
2. **HHO / LBD are own columns.** NX draws open hi-hat on the HH column (different chip graphic) and left bass on the BD column. Two extra columns → 12 uniform lanes → strip too wide, notes diluted.
3. **Uniform width + uniform gray.** Every lane is `720/12` wide, same gray. NX uses variable per-lane widths and per-pad color so position reads peripherally.
4. **Notes too small / low contrast.** `note_height = 8px` ref, thin bars lost against gray.
5. **Weak hit feedback.** Thin yellow line, no pad glyphs, no legible flash/burst/judgement at the hit line. Screenshot 2: 255 miss / 37% with zero visible reaction.
6. **Panel dead space + debug artifacts.** ~60% black screen; COMBO clips under song-info panel; keycap row shows raw keybinds (`LC Digit0`, `LP Minus`) overlapping the SPEED label.

## Approved Decisions

- **Lanes:** Match NX exactly — 10 visual columns, variable widths, HHO folded onto HH, LBD onto BD.
- **Feedback:** Full NX-style — pad glyphs at hit line + lane flash + chip-fire burst + judgement popup + combo pop.
- **Panels:** Full pass — reposition SCORE/COMBO, remove debug keybind row, fix combo clip, tighten stats box, use dead space.

Reference resolution is 1280×720, identical to `REF_WIDTH`/`REF_HEIGHT`, so NX pixel coordinates are used directly.

## Architecture

### Core idea: decouple *visual column* from *lane/channel*

Today `LaneId (0..11)` doubles as both the input/judge lane **and** the visual x-position (`lane_left(lane) = ref_lane_left + lane * uniform_w`). NX has no such 1:1 mapping — 12 channels collapse into 10 columns.

Introduce an explicit **column table**. Judgement, input, and scoring stay on the existing 12 `EChannel`/`LaneId`. Only *rendering* consults the column table.

```
EChannel / LaneId (12)         VisualColumn (10)          screen geometry
─────────────────────          ─────────────────          ───────────────
HiHatClose ─┐
HiHatOpen  ─┴──────────────►   HH  (col 1) ───────────►   x=367 w=49
BassDrum   ─┐
LeftBassDrum┴──────────────►   BD  (col 5) ───────────►   x=573 w=69
Snare ─────────────────────►   SD  (col 3) ───────────►   x=467 w=57
LeftCymbal ────────────────►   LC  (col 0) ───────────►   x=295 w=72
... (1:1 for the rest)
```

### New module: `lane_geometry.rs` (in `gameplay-drums`)

Single source of truth for the visual strip. Pure data + lookups, no Bevy systems — unit-testable in isolation.

```rust
pub const COLUMN_COUNT: usize = 10;

pub struct Column {
    pub label: &'static str,   // "LC","HH","LP","SD","HT","BD","LT","FT","CY","RD"
    pub ref_x: f32,            // left edge at 1280x720
    pub ref_w: f32,            // width at 1280x720
    pub color: Srgba,          // base chip color for the column
}

pub const COLUMNS: [Column; COLUMN_COUNT];         // ordered left→right

/// EChannel → visual column index. HHO→HH col, LBD→BD col. None if not a drum chip.
pub fn column_of(channel: EChannel) -> Option<usize>;

/// Chip color for a channel — column base color, with a distinct variant
/// for the "merged secondary" chips so HHO reads different from HH, LBD from BD.
pub fn chip_color(channel: EChannel) -> Color;
```

**Column table** (ref px, derived from `CActPerfDrumsPad.cs` pad bases + `CActPerfDrumsLaneFlushD.cs` flush rects, EType.A / RCRD default):

| col | label | ref_x | ref_w |
|-----|-------|------|------|
| 0 | LC | 295 | 72 |
| 1 | HH | 367 | 49 |
| 2 | LP | 416 | 51 |
| 3 | SD | 467 | 57 |
| 4 | HT | 524 | 49 |
| 5 | BD | 573 | 69 |
| 6 | LT | 642 | 49 |
| 7 | FT | 691 | 54 |
| 8 | CY | 745 | 70 |
| 9 | RD | 815 | 38 |

Strip spans x = 295 … 853 (width 558). Left-anchored (NX authentic): left region 0…295 holds the score/judgement panel; right region 853…1280 holds song-info, combo, gauge — this fills the current dead space.

**Column colors** (DTX-family, distinct + readable — tunable during impl):

| col | color | col | color |
|-----|-------|-----|-------|
| LC | `#cc44cc` purple | BD | `#ff8833` orange |
| HH | `#33bbee` cyan   | LT | `#55dd55` green |
| LP | `#ff66aa` pink   | FT | `#3388ff` blue |
| SD | `#ffdd33` yellow | CY | `#dd66ff` violet |
| HT | `#ff5555` red    | RD | `#66ddcc` teal |

Merged secondaries: HHO = HH cyan lightened (hollow/bright), LBD = BD orange darkened. Distinct enough to read, same column so position is stable.

### `layout.rs` changes

Replace the uniform `ref_lane_w()` / `lane_left(lane)` / single `lane_width()` with column-driven geometry:

```rust
pub fn col_left(&self, col: usize) -> f32;    // COLUMNS[col].ref_x * scale
pub fn col_width(&self, col: usize) -> f32;   // COLUMNS[col].ref_w * scale
pub fn strip_left(&self) -> f32;              // col_left(0)
pub fn strip_width(&self) -> f32;             // 558 * scale
```

`note_height`: 8 → 14 ref px (NX chip proportion). Note fills `col_width - 4px`.

Existing consumers (`scroll.rs`, `playfield_viz.rs`, `keyboard_viz.rs`, `hud.rs` backboard/progress) switch from `lane_left(lane)` to `col_left(column)`. Beat lines / backboard span `strip_left … strip_left+strip_width`.

### Note rendering (`scroll.rs`)

- `lane_color(lane)` → deleted; use `lane_geometry::chip_color(channel)`.
- Spawn maps `chip.channel → column_of()`; x = `col_left(col) + 2`, width = `col_width(col) - 4`, height = `note_height()`.
- Notes that map to no column (shouldn't happen for drum channels) are skipped as today.

### Hit feedback (`playfield_viz.rs`, `keyboard_viz.rs`, `hud.rs`)

Feedback keys off `LaneId`/`EChannel` today; retarget to columns for positioning:

1. **Pad glyphs** at hit line — `keyboard_viz` keycaps become per-column pad boxes at `judge_y`, labeled with column label only (drop the `\n{key_label}` debug line). Colored with a dim column tint; brighten on hit.
2. **Lane flash** — `ReceptorFlash` per column, flashes column color on any hit mapping to it.
3. **Chip-fire burst** — existing `spawn_hit_burst` retargeted to `col_left/col_width`; brief upward spark at the hit line in column color.
4. **Judgement popup** — existing `JudgmentPopup` re-centered on the strip (`strip_left + strip_width/2`).
5. **Combo pop** — existing `perf_combo` scale/flash on increment (keep, reposition to right panel).

### Panel layout (`hud.rs` + widgets)

Left region (x 0…295): SCORE (top), judgement stats (Perfect/Great/Good/Ok/Miss/MaxCombo), accuracy %, difficulty badge, Fast/Slow, SKILL — tightened, no debug border box (use subtle panel bg instead of white 1px outline).

Right region (x 853…1280): song-info card (top), COMBO number below it (fix current clip — COMBO label was overlapping the card), gauge, SPEED. Progress bar under the strip.

No new widgets invented; reposition existing `score_detailed`, `perf_combo`, `now_playing`, `playfield_speed`, `song_progress`, `frame_chrome`.

## Data Flow

```
chart.chips ──► spawn_notes ──► column_of(channel) ──► col geometry ──► Node+chip_color
input key  ──► LaneMap ──► LaneId ──► judge (unchanged) ──► HitEvent{lane}
HitEvent ──► column_of(lane_channel(lane)) ──► receptor flash + burst + pad glyph brighten (col)
           └► JudgmentPopup (strip center) + combo pop (right panel)
```

Judgement / scoring / autoplay / input remain on the 12-lane model — **untouched**. Only the render layer learns about columns.

## Testing

- `lane_geometry`: `column_of` maps all 12 channels correctly (HHO→HH col, LBD→BD col); `COLUMNS` ordered ascending by `ref_x` and non-overlapping; strip width == 558; `chip_color` returns distinct values for HH vs HHO and BD vs LBD.
- `layout`: `col_left(0)` == 295*scale; `col_left(9)+col_width(9)` == 853*scale; monotonic increasing.
- Existing lane_map / judge / hud tests keep passing (12-lane model unchanged). Update the stale `lane_order_matches_bocud` test and `default_labels_match_lane_order` to the corrected expectations.
- Manual: run a chart, verify order LC HH LP SD HT BD LT FT CY RD, HHO chips land on HH column with distinct color, hits flash + burst + popup, no debug keybind text, no combo clip.

## Out of Scope

- Chip texture sprites (stay with flat colored nodes).
- RD/CY position config (`eRDPosition`) and lane-type A/B/C/D variants — fix to NX default (EType.A / RCRD).
- Reverse scroll, dark mode, movie background.
- Input rebinding UI.

## Risk / Notes

- Left-anchored strip (not centered) is intentional NX authenticity; `lane_strip_centered` test in `layout.rs` gets removed/replaced.
- `note_width()` currently used by `playfield_viz`/`scroll` with a single value — becomes per-column; audit all callers.
- LANE_COUNT (12) stays for the data model; COLUMN_COUNT (10) is the new render constant. Keep them clearly named to avoid confusion.
