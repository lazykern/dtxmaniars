# Practice mode — training-centric research (external agent, 2026-07-07)

Condensed from a research-agent report reviewing practice v2 source +
genre references (Rocksmith Riff Repeater, Clone Hero practice, YARG
sections, Guitar Pro Speed Trainer, DTXMania loop-section docs).
Full product vision; v3 adopts a subset (see spec), rest parked here.

## Thesis

Keep the engine (seek.rs, ab_loop.rs, rate.rs, actions.rs, save gate,
HUD infra). Overhaul the model: transport-centric → training-centric.
Performance answers "can I clear + score"; practice answers "what am I
bad at, what next". Practice must never write normal scores/records.

Define as first-class concepts before more UI:
`PracticeSegment`, `AttemptRecord`, `PassCondition`, `LaneStats`,
`MasteryRecord`.

## Verified code findings (adopted into v3)

1. `NoteMissed` lacks `chip_idx` (events.rs) — misses can't map to
   bar/chip/section. Highest-value small fix.
2. `EmptyHit` not tracked in practice stats — overhit spam invisible.
3. Snapped-seek bug: timeline click sends unsnapped `target_ms` with
   `attempt_start_ms: None`; apply_seek snaps later → attempt start ≠
   playback start, gap chips wrongly excluded as pre-roll.
4. Ramp reacts to raw seeks — manual restart can count as a loop pass.
   Fix: explicit `PracticeLoopCompleted` event; ramp reads only that.
5. Ramp "complete" fires on *reaching* target (pass at 0.95 → 1.00 +
   disarm) without ever passing AT target. Mastery = pass at target.
6. `PracticeTransport` still in dtx-layout widget registry while HUD is
   a fixed overlay — halfway state, remove/deprecate.
7. Shift+Enter practice entry invisible — song select needs `[P]`.
8. Score/gauge/skill widgets visible by default in practice.

## Adopted design points

- `PracticeSession` split: transport / trainer / attempt / history
  sub-structs (domain module first; `dtx-practice` crate only if earned).
- `Rate` → `Tempo` user-facing (vs scroll speed confusion).
- `PassCondition { min_accuracy_pct, required_success_streak, ... }` —
  v3 keeps accuracy-only + streak default 1; multi-criteria later.
- Loop-wrap micro-report (1.5–2s, toast-sized): accuracy, misses,
  early/late ms. Feedback at loop boundaries, not during play.
- Loop reset jumps to lead-in (pre-roll), combo/judgement per-attempt.

## Rejected / superseded

- Practice summary screen on song end → superseded by implicit
  whole-song loop (no end exists in practice; wrap = seek).
- Pre-entry setup screen → friction; tune in full HUD rail. Revisit
  when segments exist.
- Default 2-pass streak per ramp step → configurable, default 1.

## Parked (needs foundation phase 0: chart hash + persistence)

- Segments: manual / chart markers / auto 4-8-16 bar / favorites /
  sidecar JSON metadata (never edit .dtx).
- Lane focus / limb isolation: judged vs ghosted vs autoplay per lane;
  presets (hands only, feet only, kick+snare, …). Three-state lanes.
- Per-lane + per-bar attempt stats; timing stddev; worst lane/bar.
- Weak-spot analyzer: post-performance "practice weakest section /
  missed fills / bass pattern"; miss heatmap on timeline.
- Generated drills: fill drill, bass consistency, pattern extraction.
- MasteryRecord: best tempo/accuracy per segment, stability; entry
  points from result screen.
- Practice records DB: PracticeProfile / Segment / Session / Attempt /
  MasteryRecord entities.
- `normal_score_valid = performance && tempo==100% && full_song &&
  no assists && no autoplay` — the record-validity rule.

## Interaction story (second research doc, same day)

Core principle: player never "configures practice mode" — they move
between play / inspect / repeat / slow down / prove mastery.
"A/B loop is implementation language; players think in sections,
bars, fills, and problems."

The outer loop to build eventually (all parked on analyzer +
segments + persistence):

```
Play song → Result: "Weak spots detected" → [Practice Weakest Section]
  → one-card intent confirm (Start default) → lead-in → loop
  → loop feedback → auto-ramp → SECTION MASTERED prompt
    → [Practice Next Weak Spot] / [Return to Full Song]
  → exit: practice summary (best-per-tempo table, main issue,
    recommendation), never the normal result screen
```

- `PracticeSuggestion { segment, reason, recommended_tempo,
  recommended_focus, recommended_goal }` from post-run analysis.
- `PracticeIntent` gains segment/tempo/focus/source
  (Manual|WeakSpot|Favorite|LastFailed|FullSong).
- Lane focus matrix: judged / visible / audible / autoplay per lane;
  presets hide the matrix (Kick Only = all visible, kick judged,
  rest ghosted). Phase judged-lane filtering first.
- Mastered = pass condition met at target tempo repeatedly (not one
  lucky FC); prompt offers next action instead of "success".
- UX hierarchy: L1 one-button useful action → L2 tempo/restart/
  section → L3 trainer config (pause menu) → L4 deep analysis
  (post-loop/session only).
- Anti-traps list confirmed v3 decisions: no config wall, ramp only
  on completed loops, pre-roll unjudged, no perf scoring as primary
  feedback, visible practice entry, in-stage adjustability.
- v3 addition sourced from this doc: loop-region change while ramp
  armed disarms the ramp (section changed = claim invalidated).

## Reference links

- Rocksmith+ phrases/sections: ubisoft.com/en-gb/game/rocksmith/plus/
  rocksmith-workshop/tutorials/phrases-sections-and-tones
- DTXMania loop-section: dtxmania.net/documents/loop-section/
- DTXMania play modes: dtxmania.net/documents/aboud-playmodes/
