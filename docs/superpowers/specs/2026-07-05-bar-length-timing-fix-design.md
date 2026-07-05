# Bar-length (meter change) timing fix

## Problem

Reported symptom: notes arrive ahead of the music; the chart ends before the
song does (e.g. `雑踏、僕らの街` MASTER: `chart_end_ms=88929` logged vs a real
`bgm_d.ogg` duration of 90070ms).

## Root cause

DTX channel `02` (`EChannel::BarLength`) encodes a per-measure length ratio
(e.g. `#01402: 1.5` = measure 14 is 1.5x a normal 4/4 measure, `#02102: 0.75`
= measure 21 is 0.75x). This chart uses it:

```
#01402: 1.5
#02102: 0.75
#02202: 1
#02702: 0.75
#03002: 1
```

`BarLength` chips are parsed (`dtx-core::chip_classify.rs`) and consumed by
`dtx-core::beat_lines.rs` for visual beat-line *density* inside a measure —
but the actual chip **playback-time** math in `dtx-timing::math` (`chip_time_ms`,
`chip_time_ms_with_bpm_changes`) assumes every measure is a fixed 4 beats. It
never reads bar-length ratios.

The audio clock (`GameplayClock`) is driven by real BGM playback position, so
it's authoritative and correct. Chip `target_ms` values are computed assuming
uniform measure length, so during a scaled measure the precomputed target
drifts away from where the audio actually is — notes arrive early during a
1.5x-length measure region's aftermath, early during 0.75x regions, etc. Net
drift happens to cancel out by the last chip in this particular chart (net
meter delta ≈ 0), which is why the *song* still has ~3s of outro after
`chart_end_ms` rather than a large residual gap — but the mid-song feel is
notes-ahead-of-music the whole way through, and the reported "chart ends
early" is the specific case of `chart_end_ms_real` under-shooting the true
chart-end time for this chart's measure layout.

## Fix

### Core (dtx-timing crate)

- New type `BarLengthChange { measure: u32, ratio: f32 }`, mirrors the
  existing `BpmChange { measure: u32, bpm: f32 }`.
- New pure fn:

  ```rust
  pub fn chip_time_ms_with_bpm_and_bar_changes(
      measure: u32,
      fraction: f32,
      base_bpm: f32,
      bpm_changes: &[BpmChange],
      bar_changes: &[BarLengthChange],
  ) -> i64
  ```

  Algorithm: walk measures `0..measure`, for each measure `m` compute
  `duration_ms(m) = active_bar_ratio(m) * 4.0 * 60_000.0 / active_bpm(m)`,
  where `active_bar_ratio(m)` is the ratio from the **last** `BarLengthChange`
  at or before `m` (default `1.0` before any change — **sticky until the
  next `BarLengthChange`**, same "changes at a measure persist until
  explicitly overridden" model already used for `BpmChange`), and
  `active_bpm(m)` is the BPM from the last `BpmChange` at or before `m` (or
  `base_bpm`). Sum full measures, then add the scaled partial-measure
  fraction using measure `measure`'s own ratio/bpm.
  Loop-based (no precomputed table), consistent with the existing
  `chip_time_ms_with_bpm_changes` per-chip pure-function calling
  convention — called once per chip at chart-load, O(measure count) per
  call, trivial at chart sizes involved (hundreds of measures).

  **Sticky vs. per-measure-only — verified empirically against this
  chart**, since a wrong model here silently fails to fix anything:
  this chart's bar-length chips (`m14=1.5, m21=0.75, m22=1, m27=0.75,
  m30=1`) are non-monotonic with explicit resets back to `1.0` at
  m22/m30. Under a "ratio applies only to the exact measure named,
  default 1.0 elsewhere" model, those resets are no-ops and the net
  deviation across the chart is `+0.5 - 0.25 - 0.25 = 0` — i.e.
  mathematically **identical** to today's buggy (no-bar-length-at-all)
  output. Computed against this chart's real numbers (171 BPM constant,
  last chip at raw measure 61.937, real `bgm_d.ogg` = 90070ms,
  `GameStartMs` = 1805): the sticky model lands `chart_end_ms_real` at
  92438ms, only **+563ms past** the real song end (91875ms in chart-ms
  space) — a safe margin consistent with the function's own "+2000ms
  buffer for BGM tail" intent. The per-measure-only model reproduces
  today's exact 88929ms, **2946ms before** the song ends. Sticky is the
  only one of the two that fixes the reported bug at all, so that's what
  ships.
- Old `chip_time_ms_with_bpm_changes` is left untouched (equivalent to
  calling the new fn with `bar_changes = &[]`). No existing caller breaks
  by default; callers are migrated individually (see below).

### Wiring (gameplay-drums crate)

- New `BarLengthChangeList` resource, exact structural mirror of
  `BpmChangeList` (`judge.rs`): `.changes: Vec<BarLengthChange>`, built via
  `BarLengthChangeList::from_chart(&chart)` (filters chips on
  `EChannel::BarLength`), registered in `judge.rs`'s `plugin()` via
  `init_resource`, populated in `orchestrator::enter_derive_from_chart`
  alongside `BpmChangeList`.
- `judge::chip_target_ms`, `chip_target_ms_with_speed`, `auto_chip_target_ms`
  gain a trailing `bar_changes: &[BarLengthChange]` parameter and delegate to
  the new core fn.

### Call sites to update

Mechanical: each adds one trailing slice param sourced from
`Res<BarLengthChangeList>` at the system boundary, same shape as the
existing `bpm_changes` threading. Rust's exhaustiveness catches any missed
call site at compile time (signature change is a hard compile error until
fixed).

**Tier 1 — required for the reported bug (audio/gameplay-critical):**

| File | Role |
|---|---|
| `judge.rs` | `chip_target_ms`/`_with_speed`/`auto_chip_target_ms` wrappers |
| `scroll.rs` | `spawn_notes` — actual note Y-position/spawn timing (the visible bug) |
| `drum_groups.rs` | ~15 `resolve_*_pad` fns — hit-judging window matching |
| `orchestrator.rs` | `chart_end_ms_real` (directly explains "ends early") + `enter_seed_bgm_state` (`GameStartMs`) |
| `bgm_scheduler.rs`, `se_scheduler.rs` | BGM/SE auto-chip scheduling |
| `hit_sound.rs` | hit-sound playback timing |
| `autoplay.rs` | bot autoplay chip-hit emission |

**Tier 2 — consistency (direct `dtx_timing` calls bypassing the judge.rs wrapper):**

| File | Role |
|---|---|
| `beat_lines.rs` (gameplay-drums) | scrolling grid-line Y-position (visual guide only, not the actual note chips) |
| `phrase.rs` | phrase-meter/skill-value boundary timing |
| `dtx-core/cdtx_config.rs::chip_to_ms` | dead code (no callers outside its own unit tests) — trivial 1-line update or leave as-is |

## Testing

- `dtx-timing` unit tests for `chip_time_ms_with_bpm_and_bar_changes`:
  - `bar_changes = &[]` behaves identically to `chip_time_ms_with_bpm_changes`
    (regression safety net).
  - Single scaled measure (e.g. ratio 2.0 at measure 1, no reset after —
    every subsequent chip's time is shifted by the extra measure length,
    proving stickiness).
  - Multiple ratio changes with explicit reset-to-1.0 chips (this chart's
    exact m14=1.5/m21=0.75/m22=1/m27=0.75/m30=1 pattern) — assert the last
    chip (raw measure 61.937, 171 BPM constant) computes to ≈90438ms
    (not the buggy 86929ms a uniform-measure model would give).
  - `chart_end_ms_real` on this chart's full chip set lands at ≈92438ms
    (measured `bgm_d.ogg` duration is 90070ms, `GameStartMs`=1805 →
    real song end = 91875ms in chart-ms space) — i.e. a few hundred ms
    *after* the song ends, not ~3s *before* it.
- Existing `orchestrator.rs`/`judge.rs`/`drum_groups.rs` tests that pass
  `bpm_changes = &[]` (and will now also pass `bar_changes = &[]`) are
  unaffected — same output as before.

## Out of scope

- No vendored `references/DTXmaniaNX-BocuD/.../CDTX.cs` exists in this repo
  (checked) to port bit-for-bit; this design follows standard DTX/BMS
  bar-length semantics instead of an exact C# port.
- Not attempting a shared precomputed per-chart timeline resource (rejected
  in favor of extending the existing pure-function convention — see
  clarifying-question answer in conversation history).
