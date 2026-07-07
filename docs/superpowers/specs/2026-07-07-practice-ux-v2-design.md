# Practice Mode UX v2 — Two-Tier HUD + Accuracy Ramp

Date: 2026-07-07
Pillar: Training Loop (pillar 1)
Status: approved design, pre-plan
Supersedes: the pause-panel interaction model from
`2026-07-06-practice-mode-design.md` (engine layer from that spec — seek,
skip-set seeding, A/B loop, rate, stats — is unchanged and stays).

## Problem

Practice v1 shipped complete (all 14 plan tasks) but the interaction lives
inside the pause menu: keyboard-only arrow scrub, no mouse, no visibility of
loop position or session state while playing. It feels bolted-on. This spec
overhauls the UX shell and adds the accuracy-gated speed ramp (Rocksmith
riff-repeater model).

## Scope

- Two-tier practice HUD: quick tier (live, during play) + full HUD (paused).
- Mouse-first scrub/loop-select on a bottom timeline in the full HUD.
- Action-layer input indirection (`PracticeAction`) so quick-tier bindings
  can later map to MIDI pad combos / foot control without logic changes.
- Accuracy-gated rate ramp operating on the armed A/B loop.
- Toast feedback for all quick-tier actions.
- Pause menu no longer hosts practice controls; the full HUD *is* the
  practice pause surface.

## Non-goals (deferred)

- Wait mode (chart waits for correct pad) — separate spec later.
- Pitch-preserving rate stretch (rate still shifts pitch, v1 behavior).
- Replay recording / formats spec (chart hash, section identity) — separate.
- MIDI/pad bindings UI — v2 ships keyboard+mouse; only the indirection lands.
- Timeline zoom.
- SRS queue, checkpoints, per-limb analytics.

## UX

### Two tiers

```
PLAYING (quick tier)                      PAUSED (full HUD, L-shape)
┌────────────────────────────┐   Esc/Tab  ┌────────────────────────────┐
│  gameplay, clean           │ ─────────▶ │  gameplay dimmed           │
│  chip: 0.85× RAMP 3/6      │            │  right rail: rate, ramp    │
│        loop 12–16 · 94%    │ ◀───────── │    config, pre-roll, snap, │
│  mini loop-strip (bottom)  │   Esc/▶    │    attempt history, exit   │
│  toasts on actions         │            │  bottom: density timeline  │
└────────────────────────────┘            │    click=seek, drag=loop   │
                                          └────────────────────────────┘
```

### Quick tier (during play)

Persistent overlay, minimal:

- **Mini loop-strip** — thin bar at screen bottom edge (below pad row):
  playhead + A/B region, full-song extent (density omitted at this size).
- **Status chip** — top-right: current rate, ramp step `3/6`, loop bars,
  last attempt accuracy.
- **Toasts** — every quick action flashes ~1.5 s confirmation top-center
  ("A set @ bar 12", "rate → 0.90×", "ramp: step up").

Hotkeys (defaults; all routed through `PracticeAction`):

| Key | Action |
|---|---|
| `[` / `]` | set A / set B at current position (snapped) |
| `Backspace` | clear loop |
| `-` / `=` | rate −0.05 / +0.05 |
| `R` | restart loop (seek to A with pre-roll) |
| `T` | ramp on/off |
| `Esc` / `Tab` | open full HUD (pauses) |

### Full HUD (paused tier), layout B "L-shape"

Opens on Esc/Tab; gameplay dims and pauses. Replaces the pause overlay
entirely while a practice session exists (exit-to-menu moves into the rail,
with confirm).

- **Bottom timeline** — full-width density strip (existing 128-bucket data)
  with bar ticks, A/B region, playhead, time readout.
  - Click → seek (snapped to current snap divisor).
  - Drag → select A/B region (snapped; min one bar, snaps up).
  - Keyboard scrub kept: arrows as today, plus hold-to-repeat.
- **Right rail** — session settings + state: rate (−/+), ramp config
  (start/target/step/threshold), ramp status, pre-roll, snap divisor,
  attempt history (last 20, existing cap, with accuracy), restart section,
  exit.
- **Transport row** — prev bar / resume / next bar buttons, clickable.

### Ramp protocol (defaults, configurable in rail)

- Start 0.70×, target 1.00×, step +0.05.
- Pass = one loop iteration with accuracy ≥ 90% (from existing
  per-attempt stats).
- 1 clean pass → rate steps up; toast "ramp: 0.75×".
- 2 consecutive failed passes at a step → step down once; floor = start rate.
- Reaching target → toast "ramp complete"; ramp disarms; rate stays at
  target.
- Arming ramp with no armed A/B loop → error toast, no state change.
- Manual rate nudge while ramp armed → ramp adopts the new rate as its
  current step (no fight between manual and auto control).

## Architecture

Modules inside `crates/gameplay-drums/src/practice/` (existing engine files
unchanged):

```
practice/
  mod.rs        session insert/teardown (exists; registers new systems)
  session.rs    + RampConfig { start, target, step, threshold }
                + RampState { armed, current_rate, consecutive_fails }
  actions.rs    NEW  PracticeAction enum, PracticeBindings resource,
                     input→action system (keyboard v2; MIDI later binds here)
  ramp.rs       NEW  pure ramp_step(state, pass_accuracy) -> RampDecision
                     + thin applier system
  toast.rs      NEW  toast queue resource + spawn/fade system
  hud/
    mini_strip.rs   quick-tier loop bar
    chip.rs         quick-tier status chip
    full_hud.rs     paused tier spawn/despawn, rail
    timeline_ui.rs  cursor x → chart ms mapping, click/drag gestures
  ui.rs         DELETED — panel logic migrates into hud/
  ab_loop.rs / rate.rs / stats.rs / (seek.rs, timeline.rs)  unchanged
```

### Data flow

```
keyboard ──▶ actions.rs ──▶ PracticeAction ──┬▶ set A/B    → session (ab_loop reads)
                                             ├▶ rate nudge → rate.rs (existing path)
mouse on timeline ──▶ timeline_ui ───────────┼▶ seek       → SeekToChartTime
                                             │               (shape FROZEN — editor
                                             │                plan 4 depends on it)
stats.rs attempt push ──▶ ramp.rs ───────────┴▶ rate step + toast
```

### Key decisions

- **Full HUD is a fixed overlay, not a `dtx-layout` widget.** No dependency
  on the layout/editor pillar; zero file collision with the editor agent's
  work beyond `pause.rs` and `lib.rs` registration lines.
- **`SeekToChartTime { target_ms, snap, attempt_start_ms }` shape frozen** —
  editor plan 4 (snap + session) consumes it.
- **Ramp protocol is a pure function**; the system only applies decisions.
- **Pause integration:** with a practice session present, `pause.rs`
  suppresses the normal overlay entirely; the full HUD owns
  `PauseState::Paused`. (v1 already half-suppressed it; v2 completes the
  move.)
- **Toast queue is practice-local** for now; generalize only when a second
  consumer appears.

## Error handling

- Ramp armed without loop → error toast, no-op.
- Rate clamped to 0.5–1.5 (existing clamp).
- Drag-select shorter than one bar → snaps up to one bar (existing rule).
- Seek while paused → audio deferred via existing `PendingBgmStart`;
  no change needed.
- Toast queue overflow (rapid keys) → oldest dropped, cap ~4 visible.

## Testing

- **Unit:** `ramp_step` table tests — pass, fail, step-down after 2 fails,
  floor at start rate, completion at target, manual-nudge adoption.
- **Unit:** `timeline_ui` cursor-x → ms mapping (pure math, both directions,
  snap applied).
- **Integration (headless, pattern of `tests/practice_mode.rs`):**
  - action → seek wiring (simulated key events),
  - ramp end-to-end across simulated loop attempts,
  - pause overlay suppressed while practice HUD open,
  - quick-tier entities exist during play, full HUD spawns/despawns on toggle.
- **Schedule guard:** extend `tests/fixed_update_schedule_ordering.rs` with
  the new systems (green unit tests do not prove the FixedUpdate schedule
  builds).
- **Manual:** mouse scrub feel, toast timing, mini-strip legibility.

## Coordination constraints

- Editor agent works on `feat/editor-canvas-selection` (+3 more editor v2
  plans). Shared files: `pause.rs`, `gameplay-drums/src/lib.rs` — expect
  small merge conflicts only. **Merge order: editor branch first**, rebase
  this work after.
- This work happens in a separate worktree branched from `main`
  (`feat/practice-ux-v2`).

## Build sequence (for the plan)

1. `actions.rs` + rewire existing hotkeys through it (no visual change).
2. Full HUD: timeline mouse scrub + drag-loop + rail (pause panel replaced).
3. Quick tier: mini-strip, chip, toasts.
4. Ramp: session fields, pure protocol, system, rail config, tests.
