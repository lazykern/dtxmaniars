# Practice wait mode — design

Date: 2026-07-11
Status: designed by agent at user's request ("design everything yourself");
confirm on review.
Scope: practice mode only.

## Problem

Learning a fill note-by-note needs the chart to stop and wait for the
correct pad instead of scrolling past. This is the standard "wait mode" of
instrument trainers: Synthesia's melody practice halts until the correct
key and ignores tempo entirely; step-mode drum trainers do the same per
pad. Nothing in current practice supports it — the closest tool is rate
0.5, which still imposes a clock.

## Model: wait-at-line (not stop-at-every-note)

The clock runs normally. When the earliest unhit note's `target_ms` is
reached with the note still pending, the clock halts at that note until
the correct pad(s) are struck, then resumes. Notes hit within the normal
early judgment window never cause a halt.

Rationale: stop-at-every-note destroys all rhythm context even when the
player is succeeding. Wait-at-line keeps normal flow while you play well
and intervenes exactly where you fail — the failure point is also the
learning point. This matches Synthesia's behavior.

## Rules

- **Halt trigger:** `clock_ms >= target_ms` of the earliest pending unhit
  note inside the attempt span → halt. Frame-quantized overshoot (a few
  ms) is accepted; resume continues from the actual position.
- **Wait-set (chords):** all drum notes sharing the halted `target_ms`
  form one set. Correct pads clear their entry in any order, each firing
  its keysound. Set empty → resume. Re-hitting an already-cleared pad
  re-fires the keysound, no other effect.
- **Wrong pad while halted:** recorded as overhit (existing EmptyHit
  path), keysound per existing behavior, no advance, no further penalty.
- **Span gating:** only notes in `[attempt intent, region end]` can halt.
  Pre-roll notes never halt (they are already stats-excluded).
- **No misses:** the clock never passes an unhit note, so `NoteMissed`
  cannot fire inside the span. Halted notes get a new stat category:
  **waited**.
- **Judgment:** hits landing before a halt are judged normally (they were
  in time). Cleared-while-halted notes are not timing-judged.
- **Flow%:** per-attempt metric = notes cleared without halting / total
  notes in span. Shown in the wrap micro-report and status chip. This is
  the wait-mode analogue of achievement%.
- **Ramp exclusion:** wait mode and the tempo ramp are mutually
  exclusive — tempo-free play makes achievement% meaningless. Enabling
  wait disarms the ramp; arming the ramp disables wait. Toast explains.
- **Loop/seek:** A/B loop, wrap, click-seek, restart all work; any seek
  clears halt state and re-derives pending notes (the seek engine's
  forward-seed/backward-prune already owns pending-note state).
- **Pre-roll + count-in metronome:** run normally before the span.

## Audio during halt

Halting pauses the BGM instance and chart layer voices using the existing
pause plumbing (`pause_audio_instance` / `pause_polyphony` — the same
mechanism `PauseState::Paused` uses); resume resumes them. The gameplay
clock freezes automatically because it derives from the BGM audio clock
(`sync_gameplay_clock`).

Known tradeoff: BGM stutters at each halt/resume. Accepted for v1 — wait
mode is a learning tool, not a performance mode. Future option: "mute BGM
while wait mode is on" toggle, which removes the stutter entirely.

## Architecture

New `crates/gameplay-drums/src/practice/wait.rs`, engine layer, pure core:

```rust
pub struct WaitSet {
    pub target_ms: i64,
    pub pending: Vec<LaneId>, // uncleared pads
}

pub enum WaitPhase {
    Flowing,
    Halted(WaitSet),
}

/// Earliest unhit chart note at/before clock_ms inside the span, if any.
pub fn check_halt(pending_notes: ..., clock_ms: i64, span_start_ms: i64)
    -> Option<WaitSet>;

/// Clear a pad from the set; reports Cleared / AlreadyClear / WrongPad
/// and whether the set is now complete.
pub fn apply_hit(set: &mut WaitSet, lane: LaneId) -> HitOutcome;
```

Systems (practice-only, gated on the wait toggle):

- **Halt check** runs after `ClockSync`, before judge: on trigger, enter
  `Halted`, pause BGM + polyphony, clamp displayed position.
- **Halted input**: pad hits route to `apply_hit`; correct pads fire
  keysounds; completion resumes audio and returns to `Flowing`.
- Scroll, beat lines, and HUD freeze for free — they follow the clock.
- Seek/region-change/toggle-off all reset `WaitPhase` to `Flowing` and
  resume audio if halted.

State lives beside the other trainer state in `PracticeSession::trainer`;
`waited` count and flow% live on `AttemptRecord` next to the existing
counts.

## UI

- Rail row in TRAINER group: "Wait: on/off".
- Status chip shows `WAIT` while enabled; wrap micro-report shows flow%
  and waited count instead of the ramp line.
- While halted, the pending pads of the wait-set are highlighted on their
  lanes (reuse existing lane-flash/hit-feedback visuals; no new widget).

## Testing

Pure-core tests:

- Single note halt at `target_ms`; early-window hit prevents halt.
- Chord wait-set: any-order clearing, duplicate pad re-hit harmless,
  wrong pad reported and set unchanged.
- Span gating: pre-roll notes never halt.
- Flow% arithmetic: cleared-without-halt vs waited.
- Seek/region change resets phase.

Integration guards:

- Ramp/wait mutual exclusion in both directions.
- No `NoteMissed` inside span while wait enabled.
- FixedUpdate ordering guard test still green (real plugin schedule).

## Out of scope

- BGM mute toggle for stutter-free halts (future).
- Wait mode outside practice.
- Per-lane wait filtering (limb layering was dropped 2026-07-11).
- Persistence of flow% history.
