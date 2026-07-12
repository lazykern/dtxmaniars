# Practice Rail Redesign + Pause Unification — Design

Stream 2 of the post-audit UX work. Fixes audit findings U2 (fixed-px rail
collides with Now-Playing at 1080p, overflows at 720p, uniform 32px text, no
mouse) and F20 (Esc semantics fork inside practice; double-Enter rail exit).
Also creates the practice-paused pad surface that stream 5 (distant-kit
grammar, F2-minimum) extends. Research base:
`docs/notes/2026-07-12-streams-research.md` §Stream 2.

## Goals

- Esc in practice opens the standard pause overlay (extended for practice)
  instead of the full rail; exit-practice lives there.
- Tab keeps opening the full rail for deep tweaking.
- Rail becomes ref-px scaled, typographically hierarchical, and
  mouse-operable.
- Kill `ExitArmed` double-Enter exit.

## Non-goals

- No pad navigation inside the full rail (keyboard + mouse only; pads get the
  pause overlay — stream 5 extends from there).
- No change to practice mechanics (loop, ramp, wait-mode, scoring gates).
- No new theme tokens.

## Part A — Pause unification

### Surfaces and openers

Two surfaces can appear in `PauseState::Paused` during practice:

- **Pause overlay** (Esc) — verbs.
- **Full rail** (Tab) — parameter editing.

Discriminator resource in `gameplay-drums`:

```rust
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum PracticePauseSurface { #[default] Overlay, Rail }
```

- `toggle_pause` (Esc, `pause.rs`) sets `PracticePauseSurface::Overlay` before
  entering `Paused` (and keeps its existing toggle-to-Running behavior).
- `apply_practice_actions` `OpenFullHud` (Tab, `actions.rs:140`) sets
  `PracticePauseSurface::Rail` before entering `Paused`.
- `spawn_full_hud` gains run condition: `PracticeSession` exists **and**
  surface == `Rail`.
- `pause::spawn_overlay` drops its practice early-return and instead runs when
  (no `PracticeSession`) **or** (surface == `Overlay`).
- While paused: Esc closes either surface (existing toggle). Tab while paused
  does nothing (opener runs only in `Running`). Outside practice the resource
  is irrelevant (overlay always spawns) but is reset to `Overlay` on
  `OnEnter(PauseState::Running)` for hygiene.

### Overlay rows in practice

`PauseItem` becomes context-dependent. When `PracticeSession` exists, rows are:

| Row | Effect |
|---|---|
| Resume | `PauseState::Running` (unchanged) |
| Restart loop | seek to loop start (reuse `PracticeAction::RestartLoop` effect) + `PauseState::Running` |
| Exit Practice | `PauseState::Running` + `request_transition(AppState::SongSelect)` (session removed on SongSelect enter, as today) |

Normal play keeps Resume / Retry / Quit exactly as-is. Implementation: a
`pause_items(practice: bool) -> &'static [PauseItemKind]` pure helper; the
existing selection/nav/legend systems operate on the active list. Pad grammar
identical in both variants (HH/CY move, BD confirm, SD = resume). Pad legend
unchanged.

`pause_menu_input` drops its practice guard (`pause.rs:231-234`); its Confirm
arm dispatches by `PauseItemKind`.

### Rail exit removal

- `RailItem::ExitPractice` variant, its `ORDER` slot, `ExitArmed` resource and
  all arming logic are deleted (17 rows remain).
- The mini-strip quick-key legend line stays as-is (Tab still opens the rail);
  no legend text changes needed beyond what exit removal implies (rail never
  mentioned exit in the legend).

## Part B — Rail redesign

### Coordinate conversion

`spawn_full_hud` queries `PlayfieldLayout` (`Res`), computes `scale`, and:

- The rail container and every sized child get `HudRefRect` components with
  ref-px rects; initial `Node` values written with the current scale (same
  dual-write convention as `now_playing.rs`). The existing
  `apply_hud_ref_layout` system (`hud.rs:393-411`) handles live resize free.
- All fonts via `scaled_font(scale, ref_size)` (`hud_ref.rs:35-37`).
- Root keeps full-screen scrim `srgba(0,0,0,0.6)`; z becomes
  `ui_z::PRACTICE_FULL_HUD` (replacing the hardcoded `GlobalZIndex(1000)`).

### Rail geometry (ref-px)

- Width 300, anchored right edge (`right: 0` with width `300 * scale`), top 0,
  bottom above the 72-ref-px timeline row.
- Rows: height ~22, `row_gap` 4, padding 12.
- Fit check at scale = 1 (720p): 3 headers × ~16 + 17 rows × ~22 + gaps ≈
  470px < 648px available. At 1080p everything scales ×1.5 uniformly with the
  Now-Playing card — no collision by construction (both ref-px; rail occupies
  the same right band the Now-Playing card tops).
- Now-Playing overlap: the full HUD is a scrim over gameplay; the rail may
  cover the Now-Playing card while paused — acceptable and existing behavior;
  the fix is that they now scale together so the rail no longer spills over
  unrelated HUD at high scale.

### Typography and row anatomy

- Section headers (TRANSPORT / LOOP / TRAINER): `scaled_font(scale, 11.0)`,
  `text_secondary`, letter-spaced label style (uppercase text as today).
- Rows: label left, `scaled_font(scale, 16.0)`; value right, same size,
  `theme.accent` when the row is selected, `text_primary` otherwise.
- Selected row: `theme.selection_highlight` background + accent value (was
  color-only). Non-selected: transparent.
- Attempt history + lane diagnosis blocks: `scaled_font(scale, 12.0)`,
  `text_secondary`, width-constrained to the rail with wrap enabled.

### Mouse interaction

Reuse the in-file `TransportButton` Interaction pattern (enum component +
`Changed<Interaction>` system) — not `controls.rs` (fixed-px, ControlValue
plumbing this screen doesn't need):

```rust
#[derive(Component)] struct RailRowButton(RailItem);      // whole row: click = select (+ act for action rows)
#[derive(Component)] struct RailAdjustButton(RailItem, i8); // ◂ = -1, ▸ = +1
```

- Value rows (Rate, Snap, Pre-roll, Ramp*) render `◂ value ▸` where the glyphs
  are small Buttons (`RailAdjustButton`); clicking adjusts exactly like
  keyboard Left/Right (same code path: the adjust logic is extracted from
  `full_hud_input` into `fn adjust_rail_item(item, dir, &mut session, ...)`
  called by both).
- Action rows (Set A, Set B, Clear loop, Restart, Ramp arm) — row click
  selects **and** activates (same as keyboard Enter, extracted into
  `fn activate_rail_item(...)`).
- Toggle rows (Count-in, Wait mode) — row click toggles.
- Row click always updates `RailSelection` so keyboard and mouse share one
  cursor.

Keyboard behavior unchanged (Up/Down select, Left/Right adjust, Enter/Space
act) minus the exit row.

### Bottom timeline row

Unchanged in behavior (transport buttons, density strip, gestures, time
text); converted to ref-px sizing (72 ref-px height, `scaled_font(scale,
16.0)` time text — was 32px screen-px) and tagged with `HudRefRect` like the
rail.

## Error handling

- `PlayfieldLayout` missing (should not happen in Performance): fall back to
  `scale = 1.0` rather than panic.
- Loop rows with no loop set, ramp rows with ramp unarmed: existing label
  logic (`rail_label`) already renders placeholders; unchanged.

## Testing

Unit:

- `pause_items(practice)` returns the right row sets.
- `PracticePauseSurface` selection: Esc-opener sets Overlay, Tab-opener sets
  Rail (test the pure decision, not the Bevy systems, where practical; system
  tests via `World::run_system_once` where needed).
- `adjust_rail_item` / `activate_rail_item`: extracted pure-ish helpers keep
  the existing per-row mutation tests meaningful; port existing `full_hud`
  tests; delete `ExitArmed` tests.
- Rail geometry: pure fit check — computed rail content height at scale 1.0
  fits 648.

Runtime (BRP smoke): enter practice (pad FT path or keyboard from wheel);
Esc → verify pause overlay with Resume/Restart/Exit Practice; BD/SD pad
verbs if MIDI test rig available, else keyboard; Exit Practice → song select.
Re-enter, Tab → verify rail renders scaled, click a `▸` on Rate → verify value
changes, click Set A row → verify loop start toast.

## Acceptance criteria

1. Esc in practice opens the pause overlay (Resume / Restart loop / Exit
   Practice) with working keyboard + pad nav; SD resumes.
2. Normal-play pause overlay unchanged.
3. Tab opens the rail; rail scales with window (no Now-Playing spill at
   1080p; no overflow at 720p).
4. Every rail row operable by mouse (select / adjust / act) and keyboard
   equally; one shared selection.
5. `ExitArmed` gone; no double-Enter exit anywhere.
6. Full gates: workspace check + clippy `-D warnings` + `cargo test -p
   gameplay-drums` green.
