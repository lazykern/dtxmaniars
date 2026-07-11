# Chord simultaneity in wait mode — design

Date: 2026-07-11
Status: brainstormed with user; approved pending write-up review.
Scope: practice mode, wait feature only. Builds on
`2026-07-11-practice-wait-mode-design.md` and the halted-set judge filter
landed same day (`judge.rs` — only halted chord's chips are judgeable while
frozen).

## Problem

Wait mode's chord rule (`is_cleared`) only checks that every chip in the
chord has been judged — in any order, with any gap between hits. A player
can tap one pad, wait a full second, tap the other, and the chord clears
as if played correctly. This defeats the point of a chord: the skill being
trained is pressing both pads *together*, not sequentially.

## Model: judged-together, not just judged

Chord clears only when every chip in the wait-set is judged **and** the
spread between the earliest and latest judged-hit timestamp is within a
fixed tolerance window. Outside the window, the attempt is rejected: both
(or all) chip judgments in this chord are undone and the halt continues —
the player retries the same chord from scratch. Notes before this chord
are untouched.

Rationale (from options considered):
- Count-only (existing behavior) needs no change but doesn't fix the
  actual complaint — it isn't training simultaneity.
- Windowed-with-hard-fail (reset the whole attempt) was rejected: too
  punishing for a low-stakes practice tool.
- **Windowed + retry-in-place** (chosen): enforces real togetherness
  without losing progress on anything before the chord. Matches practice
  mode's existing philosophy — halts are learning points, not penalties.

## Rules

- **Window:** 50ms, fixed (not user-configurable). Chosen to match
  typical "Perfect" judge windows — if both hits would earn Perfect
  individually, they clear the chord.
- **Spread definition:** `max(hit_ms) - min(hit_ms)` across all chips in
  the wait-set, using the same input-offset-adjusted timestamp already
  computed in `judge_lane_hit_system`. For 3+ note chords this is the
  full spread, not pairwise distances.
- **Accept:** once every chip in the set has a recorded hit timestamp and
  spread ≤ 50ms → clear the set, resume flow (existing behavior from
  here).
- **Reject:** once every chip in the set has a recorded hit timestamp but
  spread > 50ms → un-judge every chip in the set (remove from
  `JudgedChips`), clear their recorded timestamps, emit `ChordDesynced`
  for feedback, remain halted on the same `WaitSet`. Player re-hits all
  notes in the chord.
- **Partial state (not all chips hit yet):** no check happens — same as
  today, keep waiting.
- **Wrong-pad / off-target hits:** unaffected, still routed to `EmptyHit`
  by the existing halted-set filter in `judge.rs`; they never enter
  `ChordHitTimes`.
- **Seek/reset during halt:** `reset_wait_on_seek` must also clear
  `ChordHitTimes`, or stale timestamps from an abandoned attempt could
  leak into a new one.

## Architecture

```
LaneHit → judge_lane_hit_system (existing halted-set filter, unchanged)
              │ chip judged, adjusted hit_ms known
              ▼
      ChordHitTimes (new resource: HashMap<chip_idx, i64>)
        write only for chip idx inside the current WaitSet
              │
              ▼
      wait_watcher (existing halt driver, extended)
        ├─ not all chips in set have a ChordHitTimes entry → stay halted
        ├─ all present, spread ≤ 50ms → clear ChordHitTimes for this set,
        │    resume flow (existing path)
        └─ all present, spread > 50ms → reject:
              remove this set's chip idx from JudgedChips (un-judge)
              remove this set's entries from ChordHitTimes
              emit ChordDesynced { chips }
              stay halted, same WaitSet unchanged
```

New/changed pieces:

- `ChordHitTimes` resource (new, `wait.rs`) — `HashMap<usize, i64>`,
  chip_idx → adjusted hit timestamp. Populated by
  `judge_lane_hit_system` for chips that pass the halted-set filter (it
  already knows `adjusted_hit_ms` at that point).
- `wait_watcher` signature changes from `Res<JudgedChips>` to
  `ResMut<JudgedChips>` (needs to un-judge on reject) and gains
  `ResMut<ChordHitTimes>`.
- New event `ChordDesynced { chips: Vec<usize> }` for UI feedback (toast
  or lane-flash variant — reuse existing `practice/toast.rs` patterns,
  exact visual TBD at implementation time, not a design blocker).
- `reset_wait_on_seek` gains `ResMut<ChordHitTimes>`, clears it alongside
  existing `WaitState` reset.

Pure-core split preserved: spread calculation and accept/reject decision
should live as a testable pure function in `wait.rs` (alongside
`check_halt` / `is_cleared`), not inlined in the Bevy system — consistent
with the existing module's engine-layer/system-layer separation.

## UI

- On reject: brief feedback that the chord desynced (toast message or a
  flash on the involved lanes, reusing `hit_feedback`/`toast.rs` visuals).
  No score/stat penalty — practice mode stays low-stakes.
- No new persistent UI surface; the existing "Wait: on/off" rail toggle
  is untouched.

## Testing

Pure-core (in `wait.rs`):

- Spread ≤ 50ms with 2-note chord → accept.
- Spread > 50ms with 2-note chord → reject.
- 3-note chord: spread = max−min, not pairwise; one outlier hit rejects
  the whole set.
- Reject only clears this set's chip indices from `JudgedChips` —
  earlier-judged notes (before the chord) are untouched.
- Partial set (not all chips judged yet) never triggers accept/reject
  check.

Integration (`practice_mode.rs`):

- Full cycle: halt → spread-out hits → reject → retry within window →
  clear → resume.
- Seek during a pending (not yet evaluated) chord clears `ChordHitTimes`
  cleanly, no stale entries leak into the next attempt.

## Out of scope

- Configurable window size (fixed 50ms for v1).
- Visual countdown/timer showing remaining window during a halt.
- Applying simultaneity checks outside wait mode (e.g. normal play).
- Stat tracking of desync count (future, if useful for the wrap report).
