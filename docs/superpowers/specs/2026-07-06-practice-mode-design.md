# Practice Mode Design

Date: 2026-07-06
Status: approved design, pre-implementation
Pillar: Training Loop (pillar 1)
Research inputs: `docs/notes/2026-07-06-foundation-research.md`,
`docs/notes/2026-07-06-feature-ideas-research.md`

## Overview

Practice mode lets a drummer work on a song section by section: seek anywhere
in the chart, loop a marked A/B region, slow playback down, and see accuracy
per attempt. It is built on a shared seek-engine primitive that the live
autoplay song-select preview also consumes.

## Goals (v1)

- Seek to any chart position via a scrub bar (bar-snapped by default).
- A/B loop over a bar-snapped region with automatic loop-back.
- Playback rate 0.5x–1.5x (pitch shifts with rate; keysounds shift with BGM).
- Per-attempt section stats with in-session history.
- Entry from song select as an explicit Practice option.
- No score submission, no result saving.

## Non-goals (v1, deliberate deferrals)

- Wait mode (pause-until-hit) and accuracy-gated speed ramp — v2 trainers,
  designed as consumers of this substrate.
- Pitch-preserving time-stretch — later, internal to dtx-audio.
- Persistence of attempt history — lands with the formats spec
  (section identity keyed on `(canonical chart hash, bar_start, bar_end)`).
- Waveform rendering on the scrub bar — v2 upgrade of the density widget.
- Entering practice from the normal-play pause menu.
- Widget/layout system — pillar 2a; v1 obeys its contract (see UI Layering).

## Decisions

| Question | Decision |
|---|---|
| v1 scope | Seek + scrub, A/B loop, rate ship together; trainers later on same substrate |
| Entry point | Song select Practice option (not pause-menu switch, not separate scene) |
| Controls | Pause-menu centric; transport strip interactive only while paused |
| Stats on seek | Per-attempt section stats with history (not simple reset, not freeze) |
| Architecture | Mode-flag on existing gameplay stage (approach A) |
| UI | Same gameplay UI + practice widget layer (not a separate practice UI) |

## Architecture

New module `crates/gameplay-drums/src/practice/` (`mod.rs`, `session.rs`,
`seek.rs`, `ab_loop.rs`, `ui.rs`). The seek op itself lives engine-side in
gameplay-drums core, not under `practice/`, because the song-select preview
consumes it too. No new crate in v1; extraction is possible later once the
event surface stabilizes.

```
song_select ── "Practice" option ──▶ stage load
                                        │ inserts PracticeSession resource
                                        ▼
              ┌──────────── gameplay stage (systems unchanged) ───────────┐
              │                                                           │
              │  SeekToChartTime(ms, snap) event   ◀── practice UI        │
              │        │                            ◀── loop watcher      │
              │        ▼                            ◀── (later: preview,  │
              │  seek_system (single owner, fixed order):    trainers)    │
              │    stop BGM/SE instances → rebuild skip-sets              │
              │    → despawn Notes → BGM restart at offset                │
              │    → GameplayClock::seek(target) → pre-roll               │
              │                                                           │
              │  gates on PracticeSession present:                        │
              │    · save_result_then_despawn  SKIPPED                    │
              │    · detect_end_of_stage       suppressed inside A/B loop │
              │    · pause menu content        practice panel variant     │
              │    · per-attempt stats         collected                  │
              └───────────────────────────────────────────────────────────┘
```

Contracts:

- `SeekToChartTime` is a public Bevy event and the only entry to seeking.
  Practice UI, the A/B loop watcher, and later wait mode and the live
  preview all send it. One consumer system owns the ordering.
- `PracticeSession` resource is the mode flag plus session state. Absent
  means normal play with zero behavior change.
- v2 trainers are new systems reading `PracticeSession` and existing judge
  events. No engine rework.

## Seek Op

One system, fixed order, completes within a single tick:

```
on SeekToChartTime(target_ms, snap):
 1. resolve target: snap to bar (default) / beat / 1/4 via TimingLineList;
    sliced-BGM charts: clamp to nearest BGM-chip boundary
 2. stop audio: BGM + layer instances + polyphony one-shots
 3. rebuild skip-sets from scratch (not patch):
    PlayedBgmChips / PlayedSeChips / JudgedChips / TimingLineCrossed
      = { idx | chip_ms < target }   (autoplay rules respected)
 4. despawn all live Note entities; spawner refills next tick from new now
 5. restart BGM: play_bgm_from_seconds(target − primary_bgm_chip_ms)
 6. GameplayClock::seek(target): new method, snaps all fields, resets
    drift-corrector state so it does not fight the jump
 7. pre-roll: playback actually starts at target − preroll_ms (default one
    bar, minimum 1 s); chips inside the pre-roll window are judged normally
 8. ignore stale position_ms for the first audio callback after seek
```

Details:

- Rebuild-not-patch: O(n) over `chart.chips`; charts are small; correctness
  over cleverness.
- A sorted `(target_ms, idx)` timeline is built once at stage load and
  shared by seek, the A/B watcher, and the density widget (binary search).
- Pre-roll is implemented by seeking to `target − preroll`; the A-marker
  stores user intent (`target`). Loop restart uses the same path.
- Race with `despawn_missed_notes_system`: the seek system is ordered before
  judge/miss systems in the same schedule; the skip-set rebuild wins.
- Rate: `set_playback_rate` on the BGM instance (pitch shifts, accepted for
  v1); keysound one-shots play at the same rate. Chart-time math already
  handles `play_speed`; the audio-side rate is now actually applied. A rate
  change mid-song needs no seek (kira tween, ~50 ms).
- No-BGM charts: skip steps 2/5/8; the clock free-runs from target.
- Sliced-BGM charts: never seek inside a slice; snap to slice boundaries.

## Session State

```rust
struct PracticeSession {
    loop_region: Option<LoopRegion>,   // A/B, bar-snapped ms + bar indices
    rate: f32,                          // 0.5..=1.5, step 0.05, presets 50/75/90/100
    snap: SnapDivisor,                  // Bar | Beat | Quarter (default Bar)
    preroll: PrerollSetting,            // OneBar (default) | Seconds(f32) | Off
    current_attempt: AttemptStats,
    attempt_history: Vec<AttemptRecord>,
}
```

### A/B loop

- Set A / set B in the pause panel (plus a "set A here" quick action).
  B before A swaps them. Minimum region: one bar.
- Loop watcher system: when `clock.now >= region.end_ms`, send
  `SeekToChartTime(region.start)` and start a new attempt. Same seek path.
- `detect_end_of_stage` is suppressed while `loop_region` is active and the
  region end is before chart end. Clearing the region restores normal
  end-of-stage, so a full practice run-through works.

### Per-attempt stats

- An attempt is a seek-to-seek span (loop iteration or manual seek). On each
  seek: finalize `current_attempt`, push
  `AttemptRecord { region, rate, judgements per grade, accuracy %,
  max_combo, mean_error_ms }`, reset.
- Only chips judged inside the attempt span count. Pre-roll chips before the
  A-point are judged (sound and feedback) but excluded from attempt stats.
- HUD shows current-attempt accuracy/combo; the pause panel lists history
  (last 20, session-only in v1). `AttemptRecord` is shaped so the section
  identity key `(chart_hash, bar_start, bar_end)` drops in when the formats
  spec lands.
- Gauge/groove is frozen at full during practice; failure and rescue are
  meaningless here, and judge feedback carries the training value.

### Extension hooks (v2-ready, not built)

- Wait mode: a system that pauses the clock until the correct pad is hit,
  reading `PracticeSession` and judge events.
- Speed ramp: mutates `rate` between attempts based on `attempt_history`
  accuracy.

Both require zero engine change.

## UI

### Layering principle

Same gameplay UI plus a practice widget layer — never a separate practice
UI. Training transfer demands the playfield, lane geometry, and judgement
feedback be identical between practice and real play.

```
┌────────────────────────────────────────────────────────────┐
│  CORE layer (always): playfield, lanes, notes, judgement   │
│  text, combo — identical in play and practice              │
├────────────────────────────────────────────────────────────┤
│  PLAY layer (mode: play): gauge, skill display,            │
│  score counter, full-run progress                          │
├────────────────────────────────────────────────────────────┤
│  PRACTICE layer (mode: practice): transport strip,         │
│  attempt stats chip, A/B markers, rate badge               │
│  (gauge and skill display hidden — frozen = noise)         │
└────────────────────────────────────────────────────────────┘
```

Layout-editor alignment (pillar 2): the future editor edits widgets tagged
with mode visibility (`modes: [play]`, `[practice]`, or both), with a mode
preview toggle. Practice UI then gets editor coverage for free.

**v1 contract:** every practice UI element is a discrete, self-contained
Bevy UI entity/component with no tendrils into `hud.rs` internals. When
pillar 2a introduces the widget registry, practice elements register as
widgets — a move, not a rewrite. The core playfield layer stays identical
across modes, permanently.

### Transport strip (persistent, bottom, thin — osu!lazer-inspired)

```
┌─────────────────────────────────────────────────────────────────┐
│ 01:23.4  152 BPM  ▂▃▅▇▅▃▂▁▂▅▇█▇▅▂▁▁▂▃▅▂  x0.75  attempt #4 91% │
│                    A════╪══════B                                 │
└─────────────────────────────────────────────────────────────────┘
  time    bpm       density buckets (128) +       rate   attempt
                    playhead ╪ + A/B markers
```

- During play: display-only. While paused: focusable — ←/→ move the playhead
  by the snap unit, hold accelerates, Enter seeks and resumes. Mouse click
  and drag also seek.
- The density widget is new in `dtx-ui`: time-bucketed over song length
  (128 buckets, same shape as `AccuracyHistory`), not the per-lane
  `DensityGraph`.
- On sliced-BGM charts the strip shows valid snap points (slice boundaries).

### Practice pause panel (replaces normal pause content in practice)

```
▶ Resume            (pre-roll applies)
⟲ Restart section   (seek to A / last seek point)
A  Set A here   B  Set B here   ✕ Clear loop
Rate      ◀ 0.75x ▶   [50][75][90][100]
Snap      ◀ Bar ▶
Pre-roll  ◀ 1 bar ▶
Attempts  #4 91.2% ±6ms │ #3 88.0% │ #2 82.4% …
⏏ Exit practice
```

Keyboard/pad navigable, same navigation patterns as the existing pause menu.

### Entry

The song select difficulty/options panel gains a Practice option alongside
normal play start. It loads the same stage and inserts `PracticeSession`.
Exit returns to song select; nothing is saved in v1.

## Edge Cases

- Seek during pre-roll: fine; the new seek wins.
- Seek past chart end: clamp to the last bar.
- Rate change mid-loop: applies immediately, tagged on the next attempt
  record.
- Sliced BGM: scrubbing snaps to slice boundaries only.
- No BGM: clock free-runs from target; transport strip works unchanged.

## Testing

End-to-end style, following `crates/gameplay-drums/tests/end_to_end_stage.rs`:

- Seek forward and backward: assert skip-set contents, no chip burst on the
  next tick, clock position, notes despawned and respawned.
- A/B loop: assert loop-back fires, the attempt is finalized, and
  end-of-stage is suppressed inside the region and restored after clearing.
- Rate: assert the audio rate is applied and judge timing windows are
  unaffected at 0.75x.
- Chart-shape fixtures: single-BGM, sliced-BGM (snap behavior), no-BGM.
- Normal play regression: with no `PracticeSession`, behavior is unchanged
  (result saving, end-of-stage, pause menu).

## Dependencies & Sequencing

- Formats spec (Phase 0: canonical chart hash, replay record, section
  identity) is a separate small spec. Practice v1 does not block on it;
  `AttemptRecord` is shaped to adopt the section identity key later.
- The seek op is the same primitive the live autoplay preview plan
  (`docs/superpowers/plans/2026-07-06-live-autoplay-preview.md`, Task 4)
  needs. Implement the primitive once here; the preview consumes
  `SeekToChartTime` (or its start-at-time variant) afterwards.
- Pillar 2a (lane mapping / widget registry) is independent; practice UI
  obeys the widget contract so it can register later without rework.
