# Results Analysis and Recommended Practice Handoff — Design

## Context

Cycle 4 extends the approved end-to-end improvement program without changing
DTXManiaNX scoring, timing windows, score-file compatibility, or the existing
practice transport. The prior results-screen initiative deliberately keeps the
main card compact; the prior practice initiative already supplies loop,
pre-roll, and tempo controls. This cycle supplies the missing bridge between
them: a normal play can explain a concrete weakness and open practice at it.

`references/DTXmaniaNX/DTXMania/Stage/07.Result/CStageResult.cs` establishes
the result-stage lifecycle and record-oriented presentation. The analysis and
recommendation UI are new product UX, so their presentation follows the
existing osu-inspired results/practice initiatives rather than copying the
reference layout.

## Goals

- Record a bounded, in-memory event stream for normal plays only.
- Show an actionable normal-play summary: early/late tendency, timing spread,
  weakest lane, weakest chart section, and score delta to the prior comparable
  best when one exists.
- Keep the existing results card as the primary hierarchy; reveal detailed
  analysis only on a focused details surface.
- Replace the boolean practice flag with a typed, cross-crate request that can
  carry a recommended loop, pre-roll, tempo, and player-facing reason.
- Open a recommendation in the existing practice mode with its loop selected.
- Preserve the existing rule that practice and modified-speed normal runs do
  not create normal score history or PB comparisons.

## Non-goals

- Persisting raw hit telemetry, introducing a new score-store schema, or
  changing score.ini.
- Changing judgment windows, scoring, lane mapping, or playback-rate policy.
- Creating a new practice screen or changing manual practice controls.
- Inferring hardware latency from one result; calibration remains Cycle 3.

## Data collection and analysis

`gameplay-drums` owns a `NormalPlayEventStream` resource. It resets with each
Performance entry and accepts at most 8,192 events, retaining the earliest
events and exposing an explicit truncation flag. Each entry contains:

- drum lane;
- judgment kind, including scroll misses;
- signed timing error in milliseconds (`negative = early`, matching the
  existing `FastSlowCount` convention);
- chart chip index; and
- chip judgment time in chart milliseconds from `ChipTimeline`.

Its recorder consumes both `JudgmentEvent` and `NoteMissed` after the normal
judgment path. It is gated by absence of `PracticeSession`; practice, editor,
and assisted attempts therefore never enter the normal-play report. A miss
uses the missed chip's timeline time, rather than the current scroll/audio
time, so all events describe the chart target consistently.

Analysis is a pure helper over this stream and the bar boundaries already in
`ChipTimeline`:

- **Bias** is the median signed error of hit events. Negative is labelled
  "early", positive "late", and a median within 3 ms is "centred".
- **Spread** is the median absolute deviation from that median, reported in
  milliseconds. Misses are intentionally excluded from these two timing
  measures because they have no captured input time.
- **Lane weakness** is the lane with the greatest average error weight across
  at least three judged/missed notes. Weights are Perfect 0.0, Great 0.2, Good
  0.4, Poor 0.7, and Miss 1.0; ties resolve to the lower lane id. This compares
  consistency rather than simply selecting the busiest lane.
- **Section weakness** evaluates each bar with at least three events using the
  same average error weight; ties resolve to the earlier bar. Its recommended
  loop starts one bar before the weak bar where possible and ends after the
  weak bar plus one following bar. At a chart edge it clamps to the first/last
  bar. The candidate must have a positive duration.

When there is not enough evidence for a metric, it is absent rather than
invented. Analysis is never written to `ScoreStore` or score.ini.

## Practice intent

`game-shell` replaces `PracticeIntent(pub bool)` with a default `None` or a
typed request:

```rust
pub enum PracticeIntent {
    None,
    Manual,
    Recommended(PracticeRecommendation),
}

pub struct PracticeRecommendation {
    pub loop_start_ms: i64,
    pub loop_end_ms: i64,
    pub pre_roll: PracticePreRoll,
    pub initial_tempo: f32,
    pub reason: PracticeReason,
}
```

The shell types use only primitives/enums so `game-shell` remains independent
of `gameplay-drums`. `PracticePreRoll::OneBar` converts to the existing
practice setting. The initial recommended tempo is 1.0; it is carried now so a
future recommendation can choose a slower start without another API change.
`PracticeReason` carries the lane and/or section wording needed by the
practice HUD, not an opaque preformatted sentence.

Song select sets `Manual` for its existing Practice action and `None` for
normal play. Retry preserves the current intent. The result shortcut creates
`Recommended` only when the analysis has a valid section; otherwise it creates
`Manual`, keeping the established Practice verb useful for sparse charts.

On Performance entry, `gameplay-drums` creates a default practice session for
either request. A recommended request additionally applies its clamped loop,
one-bar pre-roll, and initial tempo before the transport begins. It does not
seek immediately: the existing loop watcher owns the first pre-roll and all
later wraps. Returning to Song Select removes the session and resets the
intent, as today.

## Results UX and PB comparison

On Result entry, the game computes a `ResultAnalysis` resource before the
score store is mutated. A PB delta is eligible only for a native-rate normal
run and compares the current score with the existing canonical-chart best. It
is displayed as `+N vs PB`, `N below PB`, or omitted when no comparable prior
entry exists. It never treats the just-saved record as its own PB.

The existing headline card gains only two compact, secondary lines after the
save status: PB delta when eligible and a short weakness summary (for example,
"Weakest: BD · bar 24"). The main verb becomes "Practice weakest section"
when a recommendation exists; otherwise its existing label remains
"Practice".

A Details control on the result screen toggles a dedicated analysis panel.
The panel contains the early/late bias, spread, per-judgment distribution,
ranked lane values, and the selected section's chart-time range. It is
keyboard/pad reachable and follows the existing reveal/selection treatment.
Closing details restores the compact card without leaving Result. This keeps
the primary result glanceable and makes diagnostic depth intentional.

## Error handling

- A missing or empty event stream renders "No timing analysis for this run";
  all normal result actions remain available.
- A truncated stream is labelled in Details. It does not prevent use of the
  analysis that is available.
- Invalid, reversed, or out-of-chart recommended bounds are clamped; if they
  cannot form a positive loop, practice falls back to manual whole-song mode.
- Missing score history omits PB text rather than treating zero as a record.
- Practice and modified-speed run kinds produce no PB delta and no regular
  score persistence; existing `SaveStatus` remains authoritative.

## Testing

- Unit-test stream capacity/reset/gating and recording hit plus scroll miss at
  the timeline target time.
- Unit-test median, MAD, lane weighting/tie rules, bar selection, recommendation
  clamping, and insufficient-evidence behaviour.
- Unit-test typed practice intents, manual/retry preservation, and applying a
  recommended request to a `PracticeSession`.
- Unit-test PB comparison before mutation, including no history, ahead, behind,
  practice, and modified-speed cases.
- Extend result-input/UI tests for the dynamic practice label and Details
  toggle; verify its first input still only skips the existing reveal.
- Run package tests for `game-shell`, `gameplay-drums`, `game-results`,
  formatting, workspace check, and workspace Clippy with warnings denied.

## Acceptance criteria

1. A completed normal run can identify an early/late tendency, timing spread,
   weakest lane, and a bar-aligned weakest-section loop when evidence exists.
2. The raw data remains bounded and in memory only; practice runs cannot enter
   normal analysis/persistence.
3. Results remains compact by default, with detailed distribution accessible
   on demand.
4. PB deltas compare only against prior, compatible native normal history.
5. Selecting the recommended practice action opens the existing practice mode
   with loop, one-bar pre-roll, and 1.0 tempo preselected.
6. Manual practice, Retry, and all existing score-integrity gates retain their
   current behavior.
