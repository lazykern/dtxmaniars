# Bar-length (meter change) timing fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix DTX chip timing so bar-length (meter change) chips (`#02` / `EChannel::BarLength`) actually scale measure duration, instead of being parsed and then ignored — this is why notes drift ahead of the music and charts end before the song does.

**Architecture:** Add a `ChartTiming<'a> { bpm_changes, bar_changes }` bundle (`Copy`) to `dtx-timing::math`, plus a new `chip_time_ms_with_bpm_and_bar_changes` core fn that walks measures 0..target applying a **sticky** bar-length ratio (persists until the next `BarLengthChange`, same semantics as `BpmChange` — verified empirically against a real chart in the design spec). A new `BarLengthChangeList` resource (gameplay-drums) mirrors the existing `BpmChangeList`. Every system that currently reads `Res<BpmChangeList>` and threads `&bpm_changes.changes` down to `chip_target_ms`/`chip_target_ms_with_speed`/`auto_chip_target_ms` also reads the new resource, builds one `ChartTiming`, and passes that instead. Intermediate pure functions (mostly `drum_groups.rs`) only need their parameter's type widened from `&[BpmChange]` to `ChartTiming<'_>` — bodies are untouched since they just forward the value.

**Tech Stack:** Rust, Bevy ECS (resources + systems), existing `dtx-timing`/`dtx-core`/`gameplay-drums` crates.

**Spec:** `docs/superpowers/specs/2026-07-05-bar-length-timing-fix-design.md`

---

## File structure

| File | Change |
|---|---|
| `crates/dtx-timing/src/lib.rs` | Add `BarLengthChange`, `ChartTiming<'a>`, `chip_time_ms_with_bpm_and_bar_changes`; redefine `chip_time_ms_with_bpm_changes` as a thin delegating wrapper; remove now-dead `measure_duration_ms(start,end,bpm)` helper; new tests |
| `crates/gameplay-drums/src/judge.rs` | Add `BarLengthChangeList` resource; widen `chip_target_ms`/`chip_target_ms_with_speed`/`auto_chip_target_ms` to `ChartTiming`; update `judge_lane_hit_system` |
| `crates/gameplay-drums/src/drum_groups.rs` | Widen ~15 functions' `bpm_changes: &[BpmChange]` param to `timing: ChartTiming<'_>` (mechanical rename, bodies unchanged); update tests |
| `crates/gameplay-drums/src/scroll.rs` | `spawn_notes_system` builds `ChartTiming`, passes to `chip_target_ms_with_speed` |
| `crates/gameplay-drums/src/autoplay.rs` | `autoplay_system` builds `ChartTiming`; test harness inits `BarLengthChangeList` |
| `crates/gameplay-drums/src/bgm_scheduler.rs` | `find_primary_bgm_chip` + `schedule_bgm_chips` widen to `ChartTiming` |
| `crates/gameplay-drums/src/se_scheduler.rs` | `schedule_se_chips` widens to `ChartTiming` |
| `crates/gameplay-drums/src/hit_sound.rs` | 3 systems + `resolve_judgment_sound`/`resolve_empty_hit_sound`/`find_nearest_chip_wav` widen to `ChartTiming` |
| `crates/gameplay-drums/src/beat_lines.rs` | `spawn_timing_lines` + `tick_metronome_on_cross` widen to `ChartTiming` |
| `crates/gameplay-drums/src/phrase.rs` | `PhraseMeter::from_chart` widens to `ChartTiming` |
| `crates/gameplay-drums/src/derived.rs` | `compute_from_chart` takes `ChartTiming`, forwards to `PhraseMeter::from_chart` |
| `crates/gameplay-drums/src/orchestrator.rs` | New `BarLengthChangeList` init; `enter_derive_from_chart` builds it + `ChartTiming`; `enter_seed_bgm_state` uses it; `chart_end_ms_real` widens; new regression test reproducing the real chart's numbers |

`crates/dtx-core/src/cdtx_config.rs::chip_to_ms` is **out of scope** — dead code, no callers outside its own unit tests (verified via `grep -rn "chip_to_ms" crates/`). Left untouched.

---

### Task 1: Core timing math (`dtx-timing`)

**Files:**
- Modify: `crates/dtx-timing/src/lib.rs:1-193` (the `pub mod math` block)
- Test: same file, `#[cfg(test)] mod tests` at the bottom

- [ ] **Step 1: Write the failing tests**

Add these to the `#[cfg(test)] mod tests` block in `crates/dtx-timing/src/lib.rs`, right after the existing `measure_duration_helper_basic` test (before the closing `}` of `mod tests`):

```rust
    #[test]
    fn bar_change_construct() {
        use math::BarLengthChange;
        let c = BarLengthChange {
            measure: 14,
            ratio: 1.5,
        };
        assert_eq!(c.measure, 14);
        assert!((c.ratio - 1.5).abs() < 0.01);
    }

    #[test]
    fn no_bar_changes_matches_bpm_only_timing() {
        use math::{chip_time_ms_with_bpm_and_bar_changes, chip_time_ms_with_bpm_changes, ChartTiming};
        let t1 = chip_time_ms_with_bpm_changes(5, 0.5, 120.0, &[]);
        let t2 = chip_time_ms_with_bpm_and_bar_changes(5, 0.5, 120.0, ChartTiming::default());
        assert_eq!(t1, t2);
    }

    #[test]
    fn single_scaled_measure_is_sticky() {
        use math::{chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, ChartTiming};
        // 120 BPM = 2000ms/measure normally. Measure 1 onward is ratio 2.0
        // (no reset chip), so it stays doubled for every later measure too.
        let bar_changes = [BarLengthChange {
            measure: 1,
            ratio: 2.0,
        }];
        let timing = ChartTiming {
            bpm_changes: &[],
            bar_changes: &bar_changes,
        };
        // Measure 0: unscaled (2000ms). Measure 1: scaled (4000ms).
        let t_measure_1 = chip_time_ms_with_bpm_and_bar_changes(1, 0.0, 120.0, timing);
        assert_eq!(t_measure_1, 2000);
        let t_measure_2 = chip_time_ms_with_bpm_and_bar_changes(2, 0.0, 120.0, timing);
        // measure 0 (2000) + measure 1 scaled (4000) = 6000
        assert_eq!(t_measure_2, 6000);
    }

    #[test]
    fn bar_change_reset_stops_stickiness() {
        use math::{chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, ChartTiming};
        // Measure 1 doubles (2000->4000), measure 2 resets to 1.0 (back to 2000).
        let bar_changes = [
            BarLengthChange {
                measure: 1,
                ratio: 2.0,
            },
            BarLengthChange {
                measure: 2,
                ratio: 1.0,
            },
        ];
        let timing = ChartTiming {
            bpm_changes: &[],
            bar_changes: &bar_changes,
        };
        // measure 0 (2000) + measure 1 scaled (4000) + measure 2 unscaled (2000) = 8000
        let t = chip_time_ms_with_bpm_and_bar_changes(3, 0.0, 120.0, timing);
        assert_eq!(t, 8000);
    }

    #[test]
    fn reproduces_real_chart_sticky_bar_lengths() {
        // Regression test for the reported bug: 雑踏、僕らの街 (MASTER),
        // 171 BPM constant, bar-length chips at m14=1.5/m21=0.75/m22=1/
        // m27=0.75/m30=1. Last drum chip sits at raw measure 61.9369125
        // (measure 61, fraction ~0.9369125) — see
        // docs/superpowers/specs/2026-07-05-bar-length-timing-fix-design.md.
        // Expected ~90438ms (not the buggy uniform-measure 86929ms).
        use math::{chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, ChartTiming};
        let bar_changes = [
            BarLengthChange {
                measure: 14,
                ratio: 1.5,
            },
            BarLengthChange {
                measure: 21,
                ratio: 0.75,
            },
            BarLengthChange {
                measure: 22,
                ratio: 1.0,
            },
            BarLengthChange {
                measure: 27,
                ratio: 0.75,
            },
            BarLengthChange {
                measure: 30,
                ratio: 1.0,
            },
        ];
        let timing = ChartTiming {
            bpm_changes: &[],
            bar_changes: &bar_changes,
        };
        let t = chip_time_ms_with_bpm_and_bar_changes(61, 0.9369125, 171.0, timing);
        assert!(
            (t - 90438).abs() <= 2,
            "expected ~90438ms, got {t}ms (bug reproduces if this is ~86929ms)"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-timing bar_change_construct no_bar_changes_matches_bpm_only_timing single_scaled_measure_is_sticky bar_change_reset_stops_stickiness reproduces_real_chart_sticky_bar_lengths`
Expected: FAIL to compile — `BarLengthChange`, `ChartTiming`, `chip_time_ms_with_bpm_and_bar_changes` don't exist yet.

- [ ] **Step 3: Implement the core types and function**

In `crates/dtx-timing/src/lib.rs`, inside `pub mod math { ... }`, add this **after** the existing `BpmChange` struct definition (after line 132, before `chip_time_ms_with_bpm_changes` at line 146):

```rust
    /// A bar-length (meter change) event at a specific measure — DTX channel
    /// `02` / `EChannel::BarLength`. `ratio` scales that measure's duration
    /// (e.g. `1.5` = 1.5x a normal 4-beat measure, `0.75` = 3/4 length).
    ///
    /// **Sticky**: like `BpmChange`, a ratio persists until the next
    /// `BarLengthChange` — verified empirically against a real chart (see
    /// `docs/superpowers/specs/2026-07-05-bar-length-timing-fix-design.md`).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct BarLengthChange {
        pub measure: u32,
        pub ratio: f32,
    }

    /// Bundled BPM + bar-length change lists for a chart, passed by value
    /// (`Copy`) instead of threading two parallel slice parameters through
    /// every timing call site.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ChartTiming<'a> {
        pub bpm_changes: &'a [BpmChange],
        pub bar_changes: &'a [BarLengthChange],
    }

    /// Active BPM at measure `m`: the most recent `BpmChange` at or before
    /// `m` (sorted ascending by measure), or `base_bpm` if none yet.
    fn active_bpm_at(m: u32, base_bpm: f32, sorted_bpm: &[BpmChange]) -> f64 {
        let mut bpm = base_bpm as f64;
        for c in sorted_bpm {
            if c.measure > m {
                break;
            }
            bpm = c.bpm as f64;
        }
        bpm
    }

    /// Active bar-length ratio at measure `m`: the most recent
    /// `BarLengthChange` at or before `m` (sorted ascending by measure), or
    /// `1.0` if none yet.
    fn active_bar_ratio_at(m: u32, sorted_bar: &[BarLengthChange]) -> f64 {
        let mut ratio = 1.0f64;
        for c in sorted_bar {
            if c.measure > m {
                break;
            }
            ratio = c.ratio as f64;
        }
        ratio
    }

    /// Compute chip playback time (ms) with BPM changes AND bar-length
    /// (meter) changes folded in. Walks measures `0..measure` summing each
    /// measure's duration (`bar_ratio(m) * 4 * 60_000 / bpm(m)`), then adds
    /// the scaled partial-measure fraction. O(measure count) per call —
    /// trivial at chart-load time (called once per chip).
    ///
    /// Pass `timing.bar_changes = &[]` to behave like
    /// `chip_time_ms_with_bpm_changes`.
    pub fn chip_time_ms_with_bpm_and_bar_changes(
        measure: u32,
        fraction: f32,
        base_bpm: f32,
        timing: ChartTiming<'_>,
    ) -> i64 {
        if base_bpm <= 0.0 {
            return 0;
        }
        let mut sorted_bpm: Vec<BpmChange> = timing.bpm_changes.to_vec();
        sorted_bpm.sort_by_key(|c| c.measure);
        let mut sorted_bar: Vec<BarLengthChange> = timing.bar_changes.to_vec();
        sorted_bar.sort_by_key(|c| c.measure);

        let mut total_ms = 0.0f64;
        for m in 0..measure {
            let bpm = active_bpm_at(m, base_bpm, &sorted_bpm);
            let ratio = active_bar_ratio_at(m, &sorted_bar);
            if bpm > 0.0 {
                total_ms += ratio * 4.0 * 60_000.0 / bpm;
            }
        }
        let bpm = active_bpm_at(measure, base_bpm, &sorted_bpm);
        let ratio = active_bar_ratio_at(measure, &sorted_bar);
        if bpm > 0.0 {
            total_ms += (fraction as f64) * ratio * 4.0 * 60_000.0 / bpm;
        }
        total_ms as i64
    }
```

Then **replace** the existing `chip_time_ms_with_bpm_changes` function body (lines 146-182 — the whole function including its doc comment) with a thin delegating wrapper:

```rust
    /// Compute chip playback time (ms) with a list of BPM change events.
    /// Equivalent to `chip_time_ms_with_bpm_and_bar_changes` with no bar
    /// changes (kept as a convenience for callers that don't need
    /// bar-length awareness, e.g. call sites not yet migrated).
    ///
    /// Pass `bpm_changes = &[]` to behave like [`chip_time_ms`].
    pub fn chip_time_ms_with_bpm_changes(
        measure: u32,
        fraction: f32,
        base_bpm: f32,
        bpm_changes: &[BpmChange],
    ) -> i64 {
        chip_time_ms_with_bpm_and_bar_changes(
            measure,
            fraction,
            base_bpm,
            ChartTiming {
                bpm_changes,
                bar_changes: &[],
            },
        )
    }
```

Finally, **delete** the now-unused private helper (immediately below the old function, was lines 184-192):

```rust
    /// Compute the duration in ms of measures [start, end) at a given BPM.
    #[inline]
    fn measure_duration_ms(start: u32, end: u32, bpm: f64) -> f64 {
        if bpm <= 0.0 {
            return 0.0;
        }
        let measures = (end - start) as f64;
        measures * 4.0 * 60_000.0 / bpm
    }
```
(This whole block is deleted — `chip_time_ms_with_bpm_changes` no longer calls it, and nothing else in the file does either. The test module's own local `measure_duration_ms` at the bottom of the file, lines 335-338, is a separate self-contained copy used only by `measure_duration_helper_basic` — leave that one alone.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-timing`
Expected: PASS — all existing tests (regression) plus the 5 new ones from Step 1.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-timing/src/lib.rs
git commit -m "feat(dtx-timing): apply bar-length ratio to chip playback time

Bar-length (meter change) chips were parsed but never folded into chip
timing math, causing notes to drift ahead of the audio clock and charts
to end before the song does. New chip_time_ms_with_bpm_and_bar_changes
walks measures applying a sticky ratio (persists until the next change,
verified against a real chart). chip_time_ms_with_bpm_changes now
delegates to it with bar_changes=[]."
```

---

### Task 2: `judge.rs` — `BarLengthChangeList` resource + wrapper widening

**Files:**
- Modify: `crates/gameplay-drums/src/judge.rs`

- [ ] **Step 1: Add the resource and widen the wrapper functions**

Replace the import line (currently `use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};`):

```rust
use dtx_timing::math::{
    chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, BpmChange, ChartTiming,
};
```

Add this right after the existing `BpmChangeList` impl block (after its closing `}`, before `pub(super) fn plugin`):

```rust
/// Sorted list of bar-length (meter change) events parsed from `#02` chips.
#[derive(Resource, Default, Debug, Clone)]
pub struct BarLengthChangeList {
    pub changes: Vec<BarLengthChange>,
}

impl BarLengthChangeList {
    pub fn from_chart(chart: &dtx_core::Chart) -> Self {
        let mut changes: Vec<BarLengthChange> = chart
            .chips
            .iter()
            .filter(|c| c.channel == dtx_core::EChannel::BarLength)
            .map(|c| BarLengthChange {
                measure: c.measure,
                ratio: c.value,
            })
            .collect();
        changes.sort_by_key(|c| c.measure);
        Self { changes }
    }
}
```

In `plugin()`, change:
```rust
    app.init_resource::<JudgedChips>()
        .init_resource::<BpmChangeList>()
```
to:
```rust
    app.init_resource::<JudgedChips>()
        .init_resource::<BpmChangeList>()
        .init_resource::<BarLengthChangeList>()
```

In `judge_lane_hit_system`, add a new param and build `ChartTiming`. Change:
```rust
pub(crate) fn judge_lane_hit_system(
    mut lane_hits: MessageReader<LaneHit>,
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    drum_settings: Res<DrumGameplaySettings>,
    input_offset: Res<crate::resources::InputOffsetMs>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
    mut empty_hits: MessageWriter<EmptyHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);

    for hit in lane_hits.read() {
```
to:
```rust
pub(crate) fn judge_lane_hit_system(
    mut lane_hits: MessageReader<LaneHit>,
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    drum_settings: Res<DrumGameplaySettings>,
    input_offset: Res<crate::resources::InputOffsetMs>,
    mut judged: ResMut<JudgedChips>,
    mut events: MessageWriter<JudgmentEvent>,
    mut empty_hits: MessageWriter<EmptyHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };

    for hit in lane_hits.read() {
```

Then change the `resolve_judgments` call inside the loop from:
```rust
        let results = resolve_judgments(
            pad,
            adjusted_hit_ms,
            &chart.chart,
            &judged.0,
            base_bpm,
            &bpm_changes.changes,
            &drum_settings.groups,
        );
```
to:
```rust
        let results = resolve_judgments(
            pad,
            adjusted_hit_ms,
            &chart.chart,
            &judged.0,
            base_bpm,
            timing,
            &drum_settings.groups,
        );
```

Widen the three free functions. Replace:
```rust
pub fn chip_target_ms(chip: &dtx_core::Chip, base_bpm: f32, bpm_changes: &[BpmChange]) -> i64 {
    chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, bpm_changes)
}

/// Chip target with optional play-speed scaling (`nPlaySpeed / 20.0`).
/// Speed = 1.0 is a no-op; >1.0 makes the chart finish earlier.
pub fn chip_target_ms_with_speed(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    play_speed: f32,
) -> i64 {
    if play_speed <= 0.0 || (play_speed - 1.0).abs() < f32::EPSILON {
        return chip_target_ms(chip, base_bpm, bpm_changes);
    }
    ((chip_target_ms(chip, base_bpm, bpm_changes) as f64) / (play_speed as f64)) as i64
}

/// Chart time for auto-play chips (BGM/SE) including BGM adjust offset.
/// `play_speed` is applied to the chip time before adding the BGM offset,
/// matching BocuD semantics (chip time scales, BGMAdjust stays absolute).
pub fn auto_chip_target_ms(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    bgm_adjust_ms: i32,
) -> i64 {
    chip_target_ms(chip, base_bpm, bpm_changes) + i64::from(bgm_adjust_ms)
}
```
with:
```rust
pub fn chip_target_ms(chip: &dtx_core::Chip, base_bpm: f32, timing: ChartTiming<'_>) -> i64 {
    chip_time_ms_with_bpm_and_bar_changes(chip.measure, chip.value, base_bpm, timing)
}

/// Chip target with optional play-speed scaling (`nPlaySpeed / 20.0`).
/// Speed = 1.0 is a no-op; >1.0 makes the chart finish earlier.
pub fn chip_target_ms_with_speed(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    timing: ChartTiming<'_>,
    play_speed: f32,
) -> i64 {
    if play_speed <= 0.0 || (play_speed - 1.0).abs() < f32::EPSILON {
        return chip_target_ms(chip, base_bpm, timing);
    }
    ((chip_target_ms(chip, base_bpm, timing) as f64) / (play_speed as f64)) as i64
}

/// Chart time for auto-play chips (BGM/SE) including BGM adjust offset.
/// `play_speed` is applied to the chip time before adding the BGM offset,
/// matching BocuD semantics (chip time scales, BGMAdjust stays absolute).
pub fn auto_chip_target_ms(
    chip: &dtx_core::Chip,
    base_bpm: f32,
    timing: ChartTiming<'_>,
    bgm_adjust_ms: i32,
) -> i64 {
    chip_target_ms(chip, base_bpm, timing) + i64::from(bgm_adjust_ms)
}
```

- [ ] **Step 2: Update judge.rs's own tests**

Five call sites in `#[cfg(test)] mod tests` pass `&[]` as the old `bpm_changes` arg to `resolve_judgments`; change each to `ChartTiming::default()`:

- `judge_selects_closest_chip_in_window` (around line 175-183)
- `judge_prefers_smallest_delta_over_earlier_chip` (around line 201-209)
- `judge_rejects_hits_outside_nx_poor_window` (around line 225-233)
- `empty_chart_produces_no_judgment` (around line 243-251)

In each, change the `&[],` line (the 6th positional arg to `resolve_judgments`) to `ChartTiming::default(),`.

The `judge_with_bpm_change_uses_new_bpm` test (around line 275-289) calls `chip_target_ms` directly:
```rust
        let bpm_changes = BpmChangeList::from_chart(&chart);
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let target_ms = chip_target_ms(&chart.chips[0], base_bpm, &bpm_changes.changes);
        assert_eq!(12000 - target_ms, 0);
```
Change the `chip_target_ms` call to:
```rust
        let bpm_changes = BpmChangeList::from_chart(&chart);
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let timing = ChartTiming {
            bpm_changes: &bpm_changes.changes,
            bar_changes: &[],
        };
        let target_ms = chip_target_ms(&chart.chips[0], base_bpm, timing);
        assert_eq!(12000 - target_ms, 0);
```

- [ ] **Step 3: Verify it compiles and existing tests pass**

Run: `cargo test -p gameplay-drums judge::`
Expected: compile error until Task 3 (drum_groups.rs) is also done, since `resolve_judgments`'s signature there still expects `&[BpmChange]`. **This task and Task 3 must land together before the crate compiles** — that's expected; proceed straight to Task 3, then run this test at the end of Task 3.

- [ ] **Step 4: Commit** (bundled with Task 3 — see Task 3 Step 4)

---

### Task 3: `drum_groups.rs` — widen the pass-through functions

**Files:**
- Modify: `crates/gameplay-drums/src/drum_groups.rs`

This is the mechanical bulk: ~13 functions currently take `bpm_changes: &[BpmChange]` purely to forward it to a nested call. Each one's signature changes from `bpm_changes: &[BpmChange]` to `bpm_changes: ChartTiming<'_>` (**keep the parameter name identical** — only the type widens, so every forwarding call site inside this file that already writes the bare identifier `bpm_changes` needs zero further edits).

- [ ] **Step 1: Update the import**

Replace:
```rust
use dtx_timing::math::BpmChange;
```
with:
```rust
use dtx_timing::math::ChartTiming;
```

- [ ] **Step 2: Widen every function signature**

In each of the following functions, change the parameter line `bpm_changes: &[BpmChange],` to `bpm_changes: ChartTiming<'_>,`. Nothing else in any of these function bodies changes — they only forward the identifier `bpm_changes` into nested calls, which now carries the wider type all the way down.

- `resolve_judgments` (the `pub fn`, param list includes `bpm_changes: &[BpmChange],`)
- `single_channel_hit`
- `closest_candidate`
- `candidates_for_channels`
- `resolve_ft_group`
- `resolve_hh_pad`
- `resolve_hho_pad`
- `resolve_cy_pad`
- `resolve_rd_pad`
- `resolve_lc_pad`
- `resolve_bd_pedal_group`
- `resolve_bd_pad`
- `resolve_lp_pad`
- `resolve_lbd_pad`
- `nearest_chip_on_channel`

The two functions that actually *call* `chip_target_ms` (`closest_candidate` at its `chip_target_ms(chip, base_bpm, bpm_changes)` line, and `nearest_chip_on_channel` at its own `chip_target_ms(chip, base_bpm, bpm_changes)` line) need **no changes to those call lines** — `chip_target_ms` was widened in Task 2 to accept `ChartTiming<'_>`, which is exactly what `bpm_changes` now is.

- [ ] **Step 3: Update this file's own tests**

Seven test call sites pass `&[]` as the 6th arg to `resolve_judgments`. Change each `&[],` (immediately before the trailing `&groups` argument) to `ChartTiming::default(),`:

- `cy_separate_only_hits_cy`
- `cy_common_accepts_ride_on_cy_pad`
- `cymbal_free_cy_separate_accepts_lc`
- `ft_common_lt_hits_ft_chip`
- `lc_separate_all_hits_lc_only`
- `lc_common_all_without_cymbal_free_picks_earliest`
- `lc_cymbal_free_separate_hits_lc_not_hh`

- [ ] **Step 4: Run tests, then commit both Task 2 and Task 3 together**

Run: `cargo test -p gameplay-drums judge:: drum_groups::`
Expected: PASS (this is the first point the crate compiles again since Task 2 started).

```bash
git add crates/gameplay-drums/src/judge.rs crates/gameplay-drums/src/drum_groups.rs
git commit -m "refactor(drums): thread ChartTiming through judging instead of bpm-only slice

chip_target_ms/_with_speed/auto_chip_target_ms and the drum_groups.rs
pad-resolution functions now carry both BPM and bar-length change data
via one Copy bundle, so hit judging picks up the bar-length timing fix."
```

---

### Task 4: `scroll.rs` — note spawn/scroll (the visible bug)

**Files:**
- Modify: `crates/gameplay-drums/src/scroll.rs`

- [ ] **Step 1: Update import and system**

Replace:
```rust
use crate::judge::{BpmChangeList, JudgedChips};
```
with:
```rust
use crate::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use dtx_timing::math::ChartTiming;
```

In `spawn_notes_system`, add the new resource param and build `ChartTiming`. Change:
```rust
fn spawn_notes_system(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    judged: Res<JudgedChips>,
    existing: Query<&Note>,
    hud_root: Query<Entity, With<HudRoot>>,
) {
```
to:
```rust
fn spawn_notes_system(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    judged: Res<JudgedChips>,
    existing: Query<&Note>,
    hud_root: Query<Entity, With<HudRoot>>,
) {
```

Then, right after the existing `let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);` line, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Finally change the `chip_target_ms_with_speed` call from:
```rust
        let target_ms = crate::judge::chip_target_ms_with_speed(
            chip,
            base_bpm,
            &bpm_changes.changes,
            scroll.play_speed,
        );
```
to:
```rust
        let target_ms = crate::judge::chip_target_ms_with_speed(
            chip,
            base_bpm,
            timing,
            scroll.play_speed,
        );
```

- [ ] **Step 2: Run tests**

Run: `cargo build -p gameplay-drums` (scroll.rs's own unit tests don't exercise `spawn_notes_system` directly — they test `top_for_note`/`lookahead_ms` — so a build check is the right verification here).
Expected: builds clean.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/scroll.rs
git commit -m "fix(drums): apply bar-length timing to note spawn/scroll position

This is the visible half of the reported bug: notes were spawned/scrolled
using BPM-only timing, so they visually arrived ahead of the audio during
scaled measures."
```

---

### Task 5: `autoplay.rs`

**Files:**
- Modify: `crates/gameplay-drums/src/autoplay.rs`

- [ ] **Step 1: Update import and system**

Replace:
```rust
use crate::judge::{chip_target_ms, BpmChangeList, JudgedChips};
```
with:
```rust
use crate::judge::{chip_target_ms, BarLengthChangeList, BpmChangeList, JudgedChips};
use dtx_timing::math::ChartTiming;
```

Change:
```rust
pub fn autoplay_system(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    mut judged: ResMut<JudgedChips>,
    mut lane_hits: MessageWriter<LaneHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let current_ms = clock.current_ms;

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
```
to:
```rust
pub fn autoplay_system(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    mut judged: ResMut<JudgedChips>,
    mut lane_hits: MessageWriter<LaneHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let current_ms = clock.current_ms;

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change the `chip_target_ms` call from:
```rust
        let target_ms = chip_target_ms(chip, base_bpm, &bpm_changes.changes);
```
to:
```rust
        let target_ms = chip_target_ms(chip, base_bpm, timing);
```

- [ ] **Step 2: Update the test harness**

Both `build_app()` and `build_pipeline_app()` in the `#[cfg(test)] mod tests` block call `.init_resource::<BpmChangeList>()`. Add `.init_resource::<BarLengthChangeList>()` right after each, e.g. change:
```rust
            .init_resource::<JudgedChips>()
            .init_resource::<BpmChangeList>()
            .init_resource::<AutoplayEnabled>()
```
to:
```rust
            .init_resource::<JudgedChips>()
            .init_resource::<BpmChangeList>()
            .init_resource::<BarLengthChangeList>()
            .init_resource::<AutoplayEnabled>()
```
(this pattern appears twice — once in `build_app()`, once in `build_pipeline_app()` — update both).

- [ ] **Step 3: Run tests**

Run: `cargo test -p gameplay-drums autoplay::`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/autoplay.rs
git commit -m "fix(drums): apply bar-length timing to autoplay bot"
```

---

### Task 6: `bgm_scheduler.rs`

**Files:**
- Modify: `crates/gameplay-drums/src/bgm_scheduler.rs`

- [ ] **Step 1: Update import**

Replace:
```rust
use crate::judge::{auto_chip_target_ms, BpmChangeList};
```
with:
```rust
use crate::judge::{auto_chip_target_ms, BarLengthChangeList, BpmChangeList};
use dtx_timing::math::ChartTiming;
```

- [ ] **Step 2: Widen `find_primary_bgm_chip`**

Change:
```rust
/// Find the chip index of the earliest BGM chip (by chart time).
pub fn find_primary_bgm_chip(chart: &Chart, bpm_changes: &BpmChangeList) -> Option<usize> {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    chart
        .chips
        .iter()
        .enumerate()
        .filter(|(_, c)| c.channel == EChannel::BGM && c.wav_slot != 0)
        .min_by_key(|(_, c)| {
            chip_time_ms_with_bpm_changes(c.measure, c.value, base_bpm, &bpm_changes.changes)
        })
        .map(|(idx, _)| idx)
}
```
to:
```rust
/// Find the chip index of the earliest BGM chip (by chart time).
pub fn find_primary_bgm_chip(chart: &Chart, timing: ChartTiming<'_>) -> Option<usize> {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    chart
        .chips
        .iter()
        .enumerate()
        .filter(|(_, c)| c.channel == EChannel::BGM && c.wav_slot != 0)
        .min_by_key(|(_, c)| {
            dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(c.measure, c.value, base_bpm, timing)
        })
        .map(|(idx, _)| idx)
}
```
(This also drops the now-unused `chip_time_ms_with_bpm_changes` import — check the top-level `use dtx_timing::math::chip_time_ms_with_bpm_changes;` line; since this was the only call site in the file, remove that import line entirely.)

- [ ] **Step 3: Widen `schedule_bgm_chips`**

Add a new param and build `ChartTiming`. Change:
```rust
fn schedule_bgm_chips(
    gameplay_clock: Res<crate::resources::GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
```
to:
```rust
fn schedule_bgm_chips(
    gameplay_clock: Res<crate::resources::GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
```

Right after the existing `let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);` line inside `schedule_bgm_chips`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change the `auto_chip_target_ms` call from:
```rust
        let target_ms = auto_chip_target_ms(chip, base_bpm, &bpm_changes.changes, bgm_shift);
```
to:
```rust
        let target_ms = auto_chip_target_ms(chip, base_bpm, timing, bgm_shift);
```

- [ ] **Step 4: Update this file's own test**

`find_primary_bgm_chip_picks_earliest` calls:
```rust
        let bpm = BpmChangeList::from_chart(&chart);
        assert_eq!(find_primary_bgm_chip(&chart, &bpm), Some(1));
```
Change to:
```rust
        let bpm = BpmChangeList::from_chart(&chart);
        let timing = ChartTiming {
            bpm_changes: &bpm.changes,
            bar_changes: &[],
        };
        assert_eq!(find_primary_bgm_chip(&chart, timing), Some(1));
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums bgm_scheduler::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/bgm_scheduler.rs
git commit -m "fix(drums): apply bar-length timing to BGM chip scheduling"
```

---

### Task 7: `se_scheduler.rs`

**Files:**
- Modify: `crates/gameplay-drums/src/se_scheduler.rs`

- [ ] **Step 1: Update import and system**

Replace:
```rust
use crate::judge::{auto_chip_target_ms, BpmChangeList};
```
with:
```rust
use crate::judge::{auto_chip_target_ms, BarLengthChangeList, BpmChangeList};
use dtx_timing::math::ChartTiming;
```

Change:
```rust
fn schedule_se_chips(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
```
to:
```rust
fn schedule_se_chips(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
```

Right after `let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change:
```rust
        let target_ms = auto_chip_target_ms(chip, base_bpm, &bpm_changes.changes, bgm_shift);
```
to:
```rust
        let target_ms = auto_chip_target_ms(chip, base_bpm, timing, bgm_shift);
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums se_scheduler::`
Expected: PASS (this file's tests don't exercise `schedule_se_chips` directly, so this is a build check via `cargo build -p gameplay-drums`).

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/se_scheduler.rs
git commit -m "fix(drums): apply bar-length timing to SE chip scheduling"
```

---

### Task 8: `hit_sound.rs`

**Files:**
- Modify: `crates/gameplay-drums/src/hit_sound.rs`

- [ ] **Step 1: Update import**

Replace:
```rust
use crate::judge::{auto_chip_target_ms, chip_target_ms, BpmChangeList};
```
with:
```rust
use crate::judge::{auto_chip_target_ms, chip_target_ms, BarLengthChangeList, BpmChangeList};
use dtx_timing::math::ChartTiming;
```

- [ ] **Step 2: `capture_empty_hit_templates`**

Change:
```rust
fn capture_empty_hit_templates(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut templates: ResMut<CurrentEmptyHitTemplates>,
) {
    if !clock.is_ready() || chart.chart.empty_hit_events.is_empty() {
        return;
    }
    let now = clock.current_ms;
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let bgm_shift = bgm_adjust.total_ms();
    for event in &chart.chart.empty_hit_events {
        let target_ms = auto_chip_target_ms(
            &dtx_core::Chip::with_wav(
                event.measure,
                EChannel::HiHatClose,
                event.value,
                event.wav_slot,
            ),
            base_bpm,
            &bpm_changes.changes,
            bgm_shift,
        );
```
to:
```rust
fn capture_empty_hit_templates(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut templates: ResMut<CurrentEmptyHitTemplates>,
) {
    if !clock.is_ready() || chart.chart.empty_hit_events.is_empty() {
        return;
    }
    let now = clock.current_ms;
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let bgm_shift = bgm_adjust.total_ms();
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    for event in &chart.chart.empty_hit_events {
        let target_ms = auto_chip_target_ms(
            &dtx_core::Chip::with_wav(
                event.measure,
                EChannel::HiHatClose,
                event.value,
                event.wav_slot,
            ),
            base_bpm,
            timing,
            bgm_shift,
        );
```

- [ ] **Step 3: `play_judgment_sounds` + `resolve_judgment_sound`**

Change:
```rust
fn play_judgment_sounds(
    mut events: MessageReader<JudgmentEvent>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    audio: Res<Audio>,
```
to:
```rust
fn play_judgment_sounds(
    mut events: MessageReader<JudgmentEvent>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    audio: Res<Audio>,
```

Right after `let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);` inside `play_judgment_sounds`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change the loop body's `resolve_judgment_sound` call from:
```rust
        let Some((wav_slot, channel)) = resolve_judgment_sound(
            pad,
            ev.chip_idx,
            ev.delta_ms
                + chip_target_ms(
                    &chart.chart.chips[ev.chip_idx],
                    base_bpm,
                    &bpm_changes.changes,
                ),
            &chart,
            &drum_settings,
            &bpm_changes,
            base_bpm,
        ) else {
            continue;
        };
```
to:
```rust
        let Some((wav_slot, channel)) = resolve_judgment_sound(
            pad,
            ev.chip_idx,
            ev.delta_ms + chip_target_ms(&chart.chart.chips[ev.chip_idx], base_bpm, timing),
            &chart,
            &drum_settings,
            timing,
        ) else {
            continue;
        };
```

Change `resolve_judgment_sound`'s own signature+body from:
```rust
fn resolve_judgment_sound(
    pad: DrumPad,
    judged_idx: usize,
    audio_ms: i64,
    chart: &ActiveChart,
    drum_settings: &DrumGameplaySettings,
    bpm_changes: &BpmChangeList,
    base_bpm: f32,
) -> Option<(u32, EChannel)> {
    let judged = chart.chart.chips.get(judged_idx)?;
    if chip_over_pad(pad, &drum_settings.config) {
        if judged.wav_slot == 0 {
            return None;
        }
        return Some((judged.wav_slot, judged.channel));
    }
    let pad_ch = sound_pad_channel(pad, &drum_settings.presence);
    if let Some((_idx, wav_slot, channel)) = nearest_chip_on_channel(
        pad_ch,
        audio_ms,
        &chart.chart,
        base_bpm,
        &bpm_changes.changes,
    ) {
        if wav_slot != 0 {
            return Some((wav_slot, channel));
        }
    }
    if judged.wav_slot == 0 {
        return None;
    }
    Some((judged.wav_slot, judged.channel))
}
```
to (drops the separate `base_bpm` param since `nearest_chip_on_channel` still needs it — keep `base_bpm` as a param, just widen the timing arg; note `nearest_chip_on_channel` is widened in Task 3-adjacent — it lives in `drum_groups.rs` and was already listed there):
```rust
fn resolve_judgment_sound(
    pad: DrumPad,
    judged_idx: usize,
    audio_ms: i64,
    chart: &ActiveChart,
    drum_settings: &DrumGameplaySettings,
    timing: ChartTiming<'_>,
) -> Option<(u32, EChannel)> {
    let judged = chart.chart.chips.get(judged_idx)?;
    if chip_over_pad(pad, &drum_settings.config) {
        if judged.wav_slot == 0 {
            return None;
        }
        return Some((judged.wav_slot, judged.channel));
    }
    let pad_ch = sound_pad_channel(pad, &drum_settings.presence);
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    if let Some((_idx, wav_slot, channel)) =
        nearest_chip_on_channel(pad_ch, audio_ms, &chart.chart, base_bpm, timing)
    {
        if wav_slot != 0 {
            return Some((wav_slot, channel));
        }
    }
    if judged.wav_slot == 0 {
        return None;
    }
    Some((judged.wav_slot, judged.channel))
}
```
(`base_bpm` moves inside the function body, computed the same way every other call site does, since the caller no longer needs to pass it separately — the outer `play_judgment_sounds` call site above was already updated to not pass `base_bpm` to `resolve_judgment_sound`.)

- [ ] **Step 4: `play_empty_hit_sounds` + `resolve_empty_hit_sound` + `find_nearest_chip_wav`**

Change:
```rust
fn play_empty_hit_sounds(
    mut events: MessageReader<EmptyHit>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    templates: Res<CurrentEmptyHitTemplates>,
    audio: Res<Audio>,
```
to:
```rust
fn play_empty_hit_sounds(
    mut events: MessageReader<EmptyHit>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    templates: Res<CurrentEmptyHitTemplates>,
    audio: Res<Audio>,
```

Right after `let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);` inside `play_empty_hit_sounds`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change the `resolve_empty_hit_sound` call from:
```rust
        let (wav_slot, channel) = resolve_empty_hit_sound(
            pad,
            hit.audio_ms,
            &chart,
            &bpm_changes,
            &templates,
            &drum_settings,
            base_bpm,
        );
```
to:
```rust
        let (wav_slot, channel) = resolve_empty_hit_sound(
            pad,
            hit.audio_ms,
            &chart,
            timing,
            &templates,
            &drum_settings,
        );
```

Change `resolve_empty_hit_sound` + `find_nearest_chip_wav` from:
```rust
fn resolve_empty_hit_sound(
    pad: DrumPad,
    audio_ms: i64,
    chart: &ActiveChart,
    bpm_changes: &BpmChangeList,
    templates: &CurrentEmptyHitTemplates,
    drum_settings: &DrumGameplaySettings,
    base_bpm: f32,
) -> (u32, EChannel) {
    for &lane in empty_hit_fallback_lanes(pad, &drum_settings.groups) {
        if let Some(ev) = templates.get(lane) {
            if ev.wav_slot != 0 {
                return (
                    ev.wav_slot,
                    lane_channel(lane).unwrap_or(EChannel::HiHatClose),
                );
            }
        }
        if let Some((wav_slot, channel)) =
            find_nearest_chip_wav(&chart.chart, lane, audio_ms, base_bpm, &bpm_changes.changes)
        {
            return (wav_slot, channel);
        }
    }
    (0, lane_channel(pad.lane()).unwrap_or(EChannel::HiHatClose))
}

fn find_nearest_chip_wav(
    chart: &dtx_core::Chart,
    lane: u8,
    audio_ms: i64,
    base_bpm: f32,
    bpm_changes: &[dtx_timing::math::BpmChange],
) -> Option<(u32, EChannel)> {
    let lane_ch = lane_channel(lane)?;
    let mut best: Option<(u32, EChannel, i64)> = None;
    for chip in chart.chips.iter() {
        if chip.channel != lane_ch || chip.wav_slot == 0 {
            continue;
        }
        let target_ms = chip_target_ms(chip, base_bpm, bpm_changes);
        let dist = (audio_ms - target_ms).abs();
        match best {
            Some((_, _, d)) if d <= dist => {}
            _ => best = Some((chip.wav_slot, chip.channel, dist)),
        }
    }
    best.map(|(w, c, _)| (w, c))
}
```
to:
```rust
fn resolve_empty_hit_sound(
    pad: DrumPad,
    audio_ms: i64,
    chart: &ActiveChart,
    timing: ChartTiming<'_>,
    templates: &CurrentEmptyHitTemplates,
    drum_settings: &DrumGameplaySettings,
) -> (u32, EChannel) {
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    for &lane in empty_hit_fallback_lanes(pad, &drum_settings.groups) {
        if let Some(ev) = templates.get(lane) {
            if ev.wav_slot != 0 {
                return (
                    ev.wav_slot,
                    lane_channel(lane).unwrap_or(EChannel::HiHatClose),
                );
            }
        }
        if let Some((wav_slot, channel)) =
            find_nearest_chip_wav(&chart.chart, lane, audio_ms, base_bpm, timing)
        {
            return (wav_slot, channel);
        }
    }
    (0, lane_channel(pad.lane()).unwrap_or(EChannel::HiHatClose))
}

fn find_nearest_chip_wav(
    chart: &dtx_core::Chart,
    lane: u8,
    audio_ms: i64,
    base_bpm: f32,
    timing: ChartTiming<'_>,
) -> Option<(u32, EChannel)> {
    let lane_ch = lane_channel(lane)?;
    let mut best: Option<(u32, EChannel, i64)> = None;
    for chip in chart.chips.iter() {
        if chip.channel != lane_ch || chip.wav_slot == 0 {
            continue;
        }
        let target_ms = chip_target_ms(chip, base_bpm, timing);
        let dist = (audio_ms - target_ms).abs();
        match best {
            Some((_, _, d)) if d <= dist => {}
            _ => best = Some((chip.wav_slot, chip.channel, dist)),
        }
    }
    best.map(|(w, c, _)| (w, c))
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums hit_sound::`
Expected: PASS (this file's existing tests don't call these functions directly, so also run `cargo build -p gameplay-drums` to confirm compilation).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/hit_sound.rs
git commit -m "fix(drums): apply bar-length timing to hit-sound resolution"
```

---

### Task 9: `beat_lines.rs` (gameplay-drums — scrolling grid lines)

**Files:**
- Modify: `crates/gameplay-drums/src/beat_lines.rs`

- [ ] **Step 1: Update import**

Replace:
```rust
use dtx_timing::math::chip_time_ms_with_bpm_changes;
```
with:
```rust
use dtx_timing::math::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};
```
And replace:
```rust
use crate::judge::BpmChangeList;
```
with:
```rust
use crate::judge::{BarLengthChangeList, BpmChangeList};
```

- [ ] **Step 2: `spawn_timing_lines`**

Change:
```rust
fn spawn_timing_lines(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    bpm_changes: Res<BpmChangeList>,
    layout: Res<PlayfieldLayout>,
```
to:
```rust
fn spawn_timing_lines(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    layout: Res<PlayfieldLayout>,
```

Right after `let base_bpm = lines.base_bpm;` inside `spawn_timing_lines`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change:
```rust
        let target_ms = chip_time_ms_with_bpm_changes(
            measure,
            fraction,
            base_bpm,
            &bpm_changes.changes,
        );
```
to:
```rust
        let target_ms = chip_time_ms_with_bpm_and_bar_changes(measure, fraction, base_bpm, timing);
```

- [ ] **Step 3: `tick_metronome_on_cross`**

Change:
```rust
fn tick_metronome_on_cross(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    show_lines: Res<ShowTimingLines>,
    metronome_on: Res<MetronomeEnabled>,
    metronome_sound: Res<MetronomeSound>,
    audio_settings: Res<DrumAudioSettings>,
    mut crossed: ResMut<TimingLineCrossed>,
    bpm_changes: Res<BpmChangeList>,
    audio: Res<Audio>,
) {
```
to:
```rust
fn tick_metronome_on_cross(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    show_lines: Res<ShowTimingLines>,
    metronome_on: Res<MetronomeEnabled>,
    metronome_sound: Res<MetronomeSound>,
    audio_settings: Res<DrumAudioSettings>,
    mut crossed: ResMut<TimingLineCrossed>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    audio: Res<Audio>,
) {
```

Right after `let base_bpm = lines.base_bpm;` inside `tick_metronome_on_cross`, add:
```rust
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
```

Change:
```rust
        let target_ms = chip_time_ms_with_bpm_changes(
            measure,
            fraction,
            base_bpm,
            &bpm_changes.changes,
        );
```
to:
```rust
        let target_ms = chip_time_ms_with_bpm_and_bar_changes(measure, fraction, base_bpm, timing);
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums beat_lines::`
Expected: PASS (existing test `bar_line_brighter_than_beat` doesn't touch these systems; this is a build check — `cargo build -p gameplay-drums`).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/beat_lines.rs
git commit -m "fix(drums): apply bar-length timing to scrolling grid lines

Visual consistency: the grid lines should track the same corrected
timeline as the actual notes."
```

---

### Task 10: `phrase.rs` + `derived.rs`

**Files:**
- Modify: `crates/gameplay-drums/src/phrase.rs`
- Modify: `crates/gameplay-drums/src/derived.rs`

- [ ] **Step 1: Widen `PhraseMeter::from_chart`**

In `phrase.rs`, replace:
```rust
use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};
```
with:
```rust
use dtx_timing::math::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};
```

Change:
```rust
    /// Build from a chart using its drum chips.
    pub fn from_chart(chart: &Chart, base_bpm: f32, bpm_changes: &[BpmChange]) -> Self {
        let mut sections = [0u32; PHRASE_SECTION_COUNT];
        let mut last_ms: i64 = 0;
        let mut total: u32 = 0;

        for chip in chart.drum_chips() {
            let t = chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, bpm_changes);
            if t < 0 {
                continue;
            }
            if t > last_ms {
                last_ms = t;
            }
            total += 1;
        }

        if total > 0 {
            for chip in chart.drum_chips() {
                let t =
                    chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, bpm_changes);
```
to:
```rust
    /// Build from a chart using its drum chips.
    pub fn from_chart(chart: &Chart, base_bpm: f32, timing: ChartTiming<'_>) -> Self {
        let mut sections = [0u32; PHRASE_SECTION_COUNT];
        let mut last_ms: i64 = 0;
        let mut total: u32 = 0;

        for chip in chart.drum_chips() {
            let t = chip_time_ms_with_bpm_and_bar_changes(chip.measure, chip.value, base_bpm, timing);
            if t < 0 {
                continue;
            }
            if t > last_ms {
                last_ms = t;
            }
            total += 1;
        }

        if total > 0 {
            for chip in chart.drum_chips() {
                let t = chip_time_ms_with_bpm_and_bar_changes(
                    chip.measure,
                    chip.value,
                    base_bpm,
                    timing,
                );
```

- [ ] **Step 2: Update `phrase.rs`'s own tests**

Five tests call `PhraseMeter::from_chart(&chart, 120.0, &[])` or `&changes`. Add `use dtx_timing::math::BpmChange;` back to the test module's imports if `BpmChange` is still referenced there (check: `bpm_change_shifts_time_buckets` constructs a `BpmChange` literal — keep that import inside `#[cfg(test)] mod tests` via `use super::*;` plus an explicit `use dtx_timing::math::BpmChange;` if `super::*` no longer re-exports it after Step 1's import change).

Change each call site:
- `empty_chart_zero_sections`: `PhraseMeter::from_chart(&Chart::default(), 120.0, &[])` → `PhraseMeter::from_chart(&Chart::default(), 120.0, ChartTiming::default())`
- `single_chip_buckets_into_first_section`: `PhraseMeter::from_chart(&chart, 120.0, &[])` → `PhraseMeter::from_chart(&chart, 120.0, ChartTiming::default())`
- `two_chips_at_same_time_share_section`: same substitution
- `bpm_change_shifts_time_buckets`:
  ```rust
        let changes = vec![BpmChange {
            measure: 1,
            bpm: 240.0,
        }];
        let p = PhraseMeter::from_chart(&chart, 120.0, &changes);
  ```
  becomes:
  ```rust
        let changes = vec![BpmChange {
            measure: 1,
            bpm: 240.0,
        }];
        let timing = ChartTiming {
            bpm_changes: &changes,
            bar_changes: &[],
        };
        let p = PhraseMeter::from_chart(&chart, 120.0, timing);
  ```
- `block_units_capped_at_max`: `PhraseMeter::from_chart(&chart, 120.0, &[])` → `PhraseMeter::from_chart(&chart, 120.0, ChartTiming::default())`

- [ ] **Step 3: `derived.rs` — thread `ChartTiming` through `compute_from_chart`**

In `crates/gameplay-drums/src/derived.rs`, replace:
```rust
use crate::judge::BpmChangeList;
use crate::phrase::PhraseMeter;
```
with:
```rust
use crate::judge::{BarLengthChangeList, BpmChangeList};
use crate::phrase::PhraseMeter;
use dtx_timing::math::ChartTiming;
```

Change:
```rust
pub fn compute_from_chart(
    derived: &mut ChartDerived,
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    drum_chip_count: u32,
) {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    derived.phrase = PhraseMeter::from_chart(chart, base_bpm, &bpm_changes.changes);
```
to:
```rust
pub fn compute_from_chart(
    derived: &mut ChartDerived,
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    bar_changes: &BarLengthChangeList,
    drum_chip_count: u32,
) {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    derived.phrase = PhraseMeter::from_chart(chart, base_bpm, timing);
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums phrase::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/phrase.rs crates/gameplay-drums/src/derived.rs
git commit -m "fix(drums): apply bar-length timing to phrase-meter boundaries"
```

---

### Task 11: `orchestrator.rs` — resource wiring, `chart_end_ms_real`, regression test

**Files:**
- Modify: `crates/gameplay-drums/src/orchestrator.rs`

This is where the reported "chart ends before the song" bug is fixed directly, and where the empirical regression test from the design spec belongs (it needs `chart_end_ms_real` + a `BarLengthChangeList`, both of which live in this file/module pair).

- [ ] **Step 1: Update imports and resource registration**

Replace:
```rust
use crate::judge::{BpmChangeList, JudgedChips};
```
with:
```rust
use crate::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use dtx_timing::math::ChartTiming;
```

In `plugin()`, no change needed here — `BarLengthChangeList` is `init_resource`'d in `judge.rs`'s own `plugin()` (Task 2), and all `gameplay_drums` sub-plugins are registered by the parent crate, so it's already available. (Verify: `cargo build -p gameplay-drums` after this task — if `BarLengthChangeList` isn't found as a resource at runtime, that means judge.rs's plugin isn't wired into the parent; check `crates/gameplay-drums/src/lib.rs` for `judge::plugin` registration — it's already there since `BpmChangeList` already works today.)

- [ ] **Step 2: `enter_derive_from_chart` builds `BarLengthChangeList` + `ChartTiming`**

Change:
```rust
pub fn enter_derive_from_chart(
    chart: Res<ActiveChart>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut bpm_changes: ResMut<BpmChangeList>,
    mut gameplay_clock: ResMut<GameplayClock>,
    mut derived: ResMut<ChartDerived>,
) {
    let has_bgm = crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart)
        || chart
            .source_path
            .as_ref()
            .and_then(|path| dtx_core::resolve_bgm_path(path, &chart.chart))
            .is_some();
    if has_bgm {
        gameplay_clock.start_audio_required();
    } else {
        gameplay_clock.start_wall_clock();
    }
    let drum_chip_count = chart.chart.drum_chips().count();
    *bpm_changes = BpmChangeList::from_chart(&chart.chart);
    completion.chart_end_ms = chart_end_ms_real(&chart.chart, &bpm_changes);
    completion.end_requested = false;
    completion.gauge_failed = false;
    crate::derived::compute_from_chart(
        &mut derived,
        &chart.chart,
        &bpm_changes,
        drum_chip_count as u32,
    );
}
```
to:
```rust
pub fn enter_derive_from_chart(
    chart: Res<ActiveChart>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut bpm_changes: ResMut<BpmChangeList>,
    mut bar_changes: ResMut<BarLengthChangeList>,
    mut gameplay_clock: ResMut<GameplayClock>,
    mut derived: ResMut<ChartDerived>,
) {
    let has_bgm = crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart)
        || chart
            .source_path
            .as_ref()
            .and_then(|path| dtx_core::resolve_bgm_path(path, &chart.chart))
            .is_some();
    if has_bgm {
        gameplay_clock.start_audio_required();
    } else {
        gameplay_clock.start_wall_clock();
    }
    let drum_chip_count = chart.chart.drum_chips().count();
    *bpm_changes = BpmChangeList::from_chart(&chart.chart);
    *bar_changes = BarLengthChangeList::from_chart(&chart.chart);
    completion.chart_end_ms = chart_end_ms_real(&chart.chart, &bpm_changes, &bar_changes);
    completion.end_requested = false;
    completion.gauge_failed = false;
    crate::derived::compute_from_chart(
        &mut derived,
        &chart.chart,
        &bpm_changes,
        &bar_changes,
        drum_chip_count as u32,
    );
}
```

- [ ] **Step 3: `enter_seed_bgm_state` builds `ChartTiming`**

Change:
```rust
pub fn enter_seed_bgm_state(
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut primary_bgm: ResMut<crate::bgm_scheduler::PrimaryBgmChip>,
    mut bgm_recovery: ResMut<crate::bgm_scheduler::BgmRecoveryState>,
    mut start_ms: ResMut<GameStartMs>,
) {
    played_bgm.0.clear();
    *bgm_recovery = crate::bgm_scheduler::BgmRecoveryState::default();
    primary_bgm.0 = crate::bgm_scheduler::find_primary_bgm_chip(&chart.chart, &bpm_changes);
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    start_ms.0 = primary_bgm
        .0
        .and_then(|idx| chart.chart.chips.get(idx))
        .map(|chip| {
            crate::judge::auto_chip_target_ms(
                chip,
                base_bpm,
                &bpm_changes.changes,
                bgm_adjust.total_ms(),
            )
        })
        .unwrap_or(0);
```
to:
```rust
pub fn enter_seed_bgm_state(
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut primary_bgm: ResMut<crate::bgm_scheduler::PrimaryBgmChip>,
    mut bgm_recovery: ResMut<crate::bgm_scheduler::BgmRecoveryState>,
    mut start_ms: ResMut<GameStartMs>,
) {
    played_bgm.0.clear();
    *bgm_recovery = crate::bgm_scheduler::BgmRecoveryState::default();
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    primary_bgm.0 = crate::bgm_scheduler::find_primary_bgm_chip(&chart.chart, timing);
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    start_ms.0 = primary_bgm
        .0
        .and_then(|idx| chart.chart.chips.get(idx))
        .map(|chip| crate::judge::auto_chip_target_ms(chip, base_bpm, timing, bgm_adjust.total_ms()))
        .unwrap_or(0);
```

- [ ] **Step 4: Widen `chart_end_ms_real`**

Change:
```rust
/// Compute the last drum chip's `target_ms` using BPM-change-aware timing.
/// Returns 0 if the chart is empty. Adds a 2000ms buffer for BGM tail.
pub fn chart_end_ms_real(chart: &Chart, bpm_changes: &BpmChangeList) -> i64 {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    chart
        .drum_chips()
        .map(|c| crate::judge::chip_target_ms(c, base_bpm, &bpm_changes.changes))
        .max()
        .unwrap_or(0)
        .saturating_add(2000)
}
```
to:
```rust
/// Compute the last drum chip's `target_ms` using BPM-change- and
/// bar-length-aware timing. Returns 0 if the chart is empty. Adds a 2000ms
/// buffer for BGM tail.
pub fn chart_end_ms_real(
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    bar_changes: &BarLengthChangeList,
) -> i64 {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    chart
        .drum_chips()
        .map(|c| crate::judge::chip_target_ms(c, base_bpm, timing))
        .max()
        .unwrap_or(0)
        .saturating_add(2000)
}
```

- [ ] **Step 5: Update `on_enter_captures_chart_end_ms` test to add the new resource + write the regression test**

In the existing test `on_enter_captures_chart_end_ms`, add `.init_resource::<BarLengthChangeList>()` to the `App` builder chain (right after `.init_resource::<BpmChangeList>()`):
```rust
            .init_resource::<BpmChangeList>()
            .init_resource::<BarLengthChangeList>()
```

Then add this new test to the `#[cfg(test)] mod tests` block, right after `on_enter_captures_chart_end_ms`:

```rust
    #[test]
    fn chart_end_ms_real_applies_sticky_bar_length() {
        // Regression test for the reported bug: without the bar-length fix,
        // this chart's shape (171 BPM constant, bar-length chips at
        // m14=1.5/m21=0.75/m22=1/m27=0.75/m30=1, last drum chip at raw
        // measure 61 + fraction 0.9369125) computes chart_end_ms_real as
        // 88929 — ~2946ms *before* the real bgm_d.ogg ends (90070ms,
        // GameStartMs=1805 -> 91875ms in chart-ms space). With the fix it
        // should land a few hundred ms *after* real song end instead.
        // See docs/superpowers/specs/2026-07-05-bar-length-timing-fix-design.md.
        use dtx_core::channel::EChannel;
        use dtx_core::chart::{Chart, Chip, Metadata};
        use dtx_timing::math::BarLengthChange;

        let mut chart = Chart {
            metadata: Metadata {
                bpm: Some(171.0),
                ..Default::default()
            },
            ..Default::default()
        };
        chart.chips.push(Chip::new(61, EChannel::BassDrum, 0.9369125));
        for (measure, ratio) in [(14, 1.5), (21, 0.75), (22, 1.0), (27, 0.75), (30, 1.0)] {
            chart.chips.push(Chip::new(measure, EChannel::BarLength, ratio));
        }

        let bpm_changes = BpmChangeList::from_chart(&chart);
        let bar_changes = BarLengthChangeList::from_chart(&chart);
        let end_ms = chart_end_ms_real(&chart, &bpm_changes, &bar_changes);

        // 90438 (computed target) + 2000 (buffer) = 92438, real song end in
        // chart-ms space is 1805 + 90070 = 91875. Allow a small tolerance.
        assert!(
            (end_ms - 92438).abs() <= 5,
            "expected ~92438ms, got {end_ms}ms"
        );
        assert!(
            end_ms > 91875,
            "chart must end at/after the real song end (91875ms), got {end_ms}ms \
             (bug reproduces if this is ~88929ms, ~2946ms too early)"
        );
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p gameplay-drums orchestrator::`
Expected: PASS, including the new `chart_end_ms_real_applies_sticky_bar_length`.

- [ ] **Step 7: Commit**

```bash
git add crates/gameplay-drums/src/orchestrator.rs
git commit -m "fix(drums): wire BarLengthChangeList through orchestrator, fix chart-end timing

This is the direct fix for the reported bug: chart_end_ms_real now
accounts for bar-length (meter change) chips, so the stage no longer
ends ~3s before the BGM finishes on charts with meter changes."
```

---

### Task 12: Full workspace verification

**Files:** none (verification only)

- [ ] **Step 1: Build the whole workspace**

Run: `cargo build --workspace`
Expected: no errors, no warnings about unused `BpmChange`/`measure_duration_ms` imports (clean up any stragglers if the compiler flags them).

- [ ] **Step 2: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests PASS, including:
- `dtx-timing`'s 5 new tests from Task 1
- `gameplay-drums::orchestrator::chart_end_ms_real_applies_sticky_bar_length` from Task 11
- every pre-existing test across `dtx-timing`, `dtx-core`, and `gameplay-drums` (regression safety)

- [ ] **Step 3: Manual verification (if a display/audio device is available in this environment)**

Launch the game, load `雑踏、僕らの街` (MASTER or EXTRA — the chart used throughout this fix), play or autoplay through it, and confirm via the log line `DrumsStage: end of chart at now_ms=..., chart_end_ms=...` that `chart_end_ms` is now close to (or slightly past) the real BGM duration rather than ~3s short. If no display/audio is available in this environment, say so explicitly rather than claiming this was verified — the unit test in Task 11 is the fallback correctness check, but it doesn't confirm the *felt* fix (notes visually in sync with the music) the way playing the chart does.

- [ ] **Step 4: Final commit (only if Step 3 turned up follow-up fixes; otherwise nothing to commit here)**
