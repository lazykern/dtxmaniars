//! Pure chart timing helpers.
//!
//! Kept in `dtx-core` so parser and chart-model tests never pull Bevy.

/// Convert a `current_ms` + a `target_ms` into a signed delta.
///
/// Positive = target is in the future (early hit).
/// Negative = target already passed (late hit).
#[inline]
pub const fn delta_ms(current_ms: i64, target_ms: i64) -> i64 {
    current_ms - target_ms
}

/// Compute the playback time (ms) of a chip from measure + fractional position + chart BPM.
///
/// v1 assumes constant BPM (no BPM-change chips). BPM-change handling
/// lands in M2 with the BPM channel parser.
#[inline]
pub fn chip_time_ms(measure: u32, fraction: f32, bpm: f32) -> i64 {
    if bpm <= 0.0 {
        return 0;
    }
    // ms per whole note = 4 * 60_000 / bpm
    let ms_per_measure = 4.0_f64 * 60_000.0 / (bpm as f64);
    let abs = (measure as f64) + (fraction as f64);
    (abs * ms_per_measure) as i64
}

/// Compute chip playback time scaled by `play_speed` (>1.0 = faster).
/// Faster speed → shorter chart time. `play_speed` ≤ 0 falls back to 1.0.
#[inline]
pub fn chip_time_ms_with_speed(measure: u32, fraction: f32, bpm: f32, play_speed: f32) -> i64 {
    let t = chip_time_ms(measure, fraction, bpm);
    if play_speed <= 0.0 || (play_speed - 1.0).abs() < f32::EPSILON {
        return t;
    }
    ((t as f64) / (play_speed as f64)) as i64
}

/// Compute chip playback time with BPM changes + play-speed scaling.
#[inline]
pub fn chip_time_ms_with_bpm_changes_and_speed(
    measure: u32,
    fraction: f32,
    base_bpm: f32,
    bpm_changes: &[BpmChange],
    play_speed: f32,
) -> i64 {
    let t = chip_time_ms_with_bpm_changes(measure, fraction, base_bpm, bpm_changes);
    if play_speed <= 0.0 || (play_speed - 1.0).abs() < f32::EPSILON {
        return t;
    }
    ((t as f64) / (play_speed as f64)) as i64
}

/// A BPM change event at a specific measure.
///
/// Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1070-1080`
/// — `n現在のBPM` updated by `listBPM変更` on each `BPM` channel chip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BpmChange {
    pub measure: u32,
    pub bpm: f32,
}

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

/// Active BPM strictly *before* measure `m` (a change landing exactly at
/// `m` is excluded). Used only for the chip's own fractional remainder:
/// mirrors the legacy interval algorithm, which never applied a change
/// at the chip's own measure to that chip's partial-measure position
/// (see `bpm_segment.rs::case2_one_change_with_fraction`).
fn active_bpm_before(m: u32, base_bpm: f32, sorted_bpm: &[BpmChange]) -> f64 {
    let mut bpm = base_bpm as f64;
    for c in sorted_bpm {
        if c.measure >= m {
            break;
        }
        bpm = c.bpm as f64;
    }
    bpm
}

/// Active bar-length ratio strictly *before* measure `m`. Same rationale
/// as `active_bpm_before` — applied only to the chip's own fractional
/// remainder, not the full-measure sum.
fn active_bar_ratio_before(m: u32, sorted_bar: &[BarLengthChange]) -> f64 {
    let mut ratio = 1.0f64;
    for c in sorted_bar {
        if c.measure >= m {
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
    let bpm = active_bpm_before(measure, base_bpm, &sorted_bpm);
    let ratio = active_bar_ratio_before(measure, &sorted_bar);
    if bpm > 0.0 {
        total_ms += (fraction as f64) * ratio * 4.0 * 60_000.0 / bpm;
    }
    total_ms as i64
}

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
