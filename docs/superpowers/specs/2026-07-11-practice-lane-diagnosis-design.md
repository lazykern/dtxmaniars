# Practice per-lane diagnosis panel — design

Date: 2026-07-11
Status: approved in brainstorm through architecture choice; metric set and
UI details chosen by recommendation while user was away — confirm on review.
Scope: practice full HUD only. No results-screen widget (future extension).

## Problem

Practice aggregates judgments into whole-attempt stats; the lane on each
`JudgmentEvent` is discarded. A drummer drilling a section cannot see
*which limb* is failing or whether they rush or drag a specific lane
("snare fine, hihat −18 ms rushing"). `delta_ms` and `lane: LaneId` are
already on every judgment event — only aggregation and display are missing.

## Decisions

- **Surface:** paused practice full HUD only.
- **Window:** current loop region, accumulated across all attempts; reset
  whenever the loop region changes (same lifecycle as the v3 span-filtered
  attempt history).
- **Collection:** extend the existing `practice/stats.rs` event-consumption
  path (approach A). A separate diagnosis module was rejected: it would
  duplicate the gating rules stats.rs already solved (pre-roll exclusion
  via `chip_idx`, span filtering) and the two would drift.

## 1. Engine: per-lane aggregation

Extend the practice stats layer with a per-lane accumulator:

```rust
#[derive(Default)]
pub struct LaneAgg {
    pub judged: u32,       // judgments counted (excludes pre-roll)
    pub misses: u32,
    pub overhits: u32,     // EmptyHit on this lane
    pub delta_sum_ms: i64, // signed, hits only
    pub delta_count: u32,
}

pub struct LaneDiagnosis {
    pub lanes: HashMap<LaneId, LaneAgg>,
}
```

- Derived values (methods, not stored): `hit_pct = (judged - misses) /
  judged`, `mean_delta_ms = delta_sum_ms / delta_count`.
- Fed from the same systems in `practice/stats.rs` that already consume
  `JudgmentEvent`, `NoteMissed`, and overhit events — one extra call beside
  `apply_judgment`, so pre-roll exclusion and span gating are inherited,
  not reimplemented.
- Lives in `PracticeSession`; cleared at every loop-region change (Set A/B,
  Clear, drag) at the same site the span-filtered history resets.

## 2. UI: full-HUD panel

- Panel in the paused full HUD, one row per lane **that has notes in the
  current region**, sorted worst-first (lowest hit%, ties by |mean delta|).
- Row: lane name · hit% · timing bias · miss/overhit counts.
- Bias label: `mean_delta_ms` past ±10 ms renders as "−18 ms rushing" /
  "+14 ms dragging"; within ±10 ms renders as "on time". Sign convention
  must match the existing hit-feedback `{:+}ms` display.
- Rows with `judged == 0` (region has notes, none judged yet) show counts
  as "—".
- UI reads `LaneDiagnosis` only; no timing or aggregation logic in UI.

## 3. Testing

- Pure aggregation: judgments across lanes → correct per-lane hit%, mean
  delta, miss/overhit counts.
- Reset on region change; survives attempt boundaries within one region.
- Pre-roll judgments excluded (inherited gating — test guards the wiring).
- Overhit attribution to the struck lane.
- Sort order worst-first, bias-label thresholds (±10 ms).
- FixedUpdate ordering guard test still green.

## Out of scope

- Results-screen / normal-game per-lane widget (dtx-layout `WidgetKind`).
- Double-stroke / pattern-level analysis ("kick 60% on doubles").
- Persistence of diagnosis across sessions.
