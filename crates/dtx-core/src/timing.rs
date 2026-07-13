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

/// A BPM change event at a specific measure **and fractional position within
/// that measure** (`0.0..1.0`).
///
/// DTX encodes BPM changes as slot sequences on channels `03`/`08`, exactly
/// like note channels, so a single measure can hold several changes at
/// different positions (e.g. `#20208: 090B` = BPM09 at 0.0, BPM0B at 0.5).
/// Snapping them to the measure boundary mistimes everything downstream.
///
/// Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1070-1080`
/// — `n現在のBPM` updated by `listBPM変更` as chips are walked in position order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BpmChange {
    pub measure: u32,
    pub bpm: f32,
    /// Position within `measure`, `0.0..1.0`.
    pub fraction: f32,
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

/// Collect every BPM change in `chart` (channels `03` and `08`), sorted by
/// measure then in-measure position.
///
/// Single source of truth: gameplay-drums, gameplay-guitar and dtx-bga all
/// need this list and must agree on it, or a chip's audio time and its visual
/// time drift apart.
pub fn bpm_changes_from_chart(chart: &crate::Chart) -> Vec<BpmChange> {
    let mut changes: Vec<BpmChange> = chart
        .chips
        .iter()
        .filter(|c| matches!(c.channel, crate::EChannel::BPM | crate::EChannel::BPMEx))
        .map(|c| BpmChange {
            measure: c.measure,
            bpm: c.value,
            fraction: c.fraction,
        })
        .collect();
    changes.sort_by(|a, b| {
        (a.measure, a.fraction)
            .partial_cmp(&(b.measure, b.fraction))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    changes
}

/// Collect every bar-length (meter) change in `chart` (channel `02`), sorted
/// by measure. Bar length is measure-granular — no fractional position.
pub fn bar_changes_from_chart(chart: &crate::Chart) -> Vec<BarLengthChange> {
    let mut changes: Vec<BarLengthChange> = chart
        .chips
        .iter()
        .filter(|c| c.channel == crate::EChannel::BarLength)
        .map(|c| BarLengthChange {
            measure: c.measure,
            ratio: c.value,
        })
        .collect();
    changes.sort_by_key(|c| c.measure);
    changes
}

/// Active bar-length ratio at measure `m`: the most recent
/// `BarLengthChange` at or before `m` (sorted ascending by measure), or
/// `1.0` if none yet. Bar length is measure-granular by DTX spec (channel
/// `02` carries one decimal per measure), so it has no fractional position.
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

/// Duration (ms) of a whole measure at `bpm`, scaled by the bar-length `ratio`.
fn measure_ms(bpm: f64, ratio: f64) -> f64 {
    ratio * 4.0 * 60_000.0 / bpm
}

/// Compute chip playback time (ms) with BPM changes AND bar-length
/// (meter) changes folded in.
///
/// This is a true segment integral, not a per-measure average: a measure
/// containing BPM changes is split at each change's fractional position and
/// each sub-interval is accumulated at its own BPM. That matches
/// DTXManiaNX, which walks chips in position order and updates the running
/// BPM as it goes (`CDTX.cs:1070-1080`) — a change at position 0.5 takes
/// effect halfway through the measure, not at the next bar line.
///
/// O(measures × changes-per-measure) per call — trivial at chart-load time
/// (called once per chip).
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
    let mut sorted_bpm: Vec<BpmChange> = timing
        .bpm_changes
        .iter()
        .copied()
        .filter(|c| c.bpm > 0.0)
        .collect();
    sorted_bpm.sort_by(|a, b| {
        (a.measure, a.fraction)
            .partial_cmp(&(b.measure, b.fraction))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut sorted_bar: Vec<BarLengthChange> = timing.bar_changes.to_vec();
    sorted_bar.sort_by_key(|c| c.measure);

    // Running BPM, updated as we sweep measures in order. Starts at the
    // chart's `#BPM` and carries across bar lines (changes are sticky).
    let mut bpm = base_bpm as f64;
    let mut total_ms = 0.0f64;
    let mut next_change = 0usize;

    // Sweep every measure up to and including the chip's own, integrating the
    // full measure for the ones before it and only up to `fraction` for its own.
    for m in 0..=measure {
        let ratio = active_bar_ratio_at(m, &sorted_bar);
        // How far into this measure we integrate: all of it, except the chip's
        // own measure, where we stop at the chip. `fraction` may exceed 1.0
        // (callers use it as a measure-relative offset); the overflow is
        // integrated at the chip measure's own BPM and bar ratio, as before.
        let limit = if m == measure {
            (fraction as f64).max(0.0)
        } else {
            1.0
        };

        // Skip changes that landed in measures we've already passed but were
        // not consumed (can't happen with sorted input, but keeps the cursor
        // honest against malformed lists).
        while next_change < sorted_bpm.len() && sorted_bpm[next_change].measure < m {
            bpm = sorted_bpm[next_change].bpm as f64;
            next_change += 1;
        }

        let mut cursor = 0.0f64;
        while next_change < sorted_bpm.len() && sorted_bpm[next_change].measure == m {
            let pos = (sorted_bpm[next_change].fraction as f64).clamp(0.0, 1.0);
            if pos > limit {
                // Change lies after the chip inside this measure — it does not
                // affect this chip's time. Leave it for later chips.
                break;
            }
            total_ms += (pos - cursor).max(0.0) * measure_ms(bpm, ratio);
            cursor = pos;
            bpm = sorted_bpm[next_change].bpm as f64;
            next_change += 1;
        }
        total_ms += (limit - cursor).max(0.0) * measure_ms(bpm, ratio);

        // A change sitting exactly at the chip's own position contributes zero
        // width above, so it is already applied — which is what DTXManiaNX does.
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
