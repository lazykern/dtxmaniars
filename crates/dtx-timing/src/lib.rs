//! Authoritative audio clock for hit-window judgment.
//!
//! Engine layer. Reads [`dtx_audio::BgmHandle`] each frame from
//! `bevy_kira_audio`'s playback position and writes a single i64 ms value
//! into [`AudioClock`].
//!
//! ADR-0002: **never** judge on `Time::delta()` accumulated frame time.
//! Use `Res<AudioClock>` for timing windows.
//!
//! Reference: `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs`.
//! Our approach is simpler than the C# original: kira exposes a
//! position-in-seconds callback directly, so we skip the manual
//! timestamp-to-ms conversion.

use bevy::prelude::*;
use bevy_kira_audio::Audio;

use dtx_audio::BgmHandle;

/// The single source of truth for "what ms of BGM playback are we at?".
///
/// `current_ms`:
/// - `None` when no BGM is loaded or BGM is Stopped/Queued.
/// - `Some(ms)` while BGM is Playing or Paused.
///
/// Updated each frame by [`update_audio_clock_system`].
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioClock {
    pub current_ms: Option<i64>,
}

impl AudioClock {
    /// Convenience: ms as i64, 0 if not playing.
    pub fn ms_or_zero(&self) -> i64 {
        self.current_ms.unwrap_or(0)
    }

    /// True iff BGM is currently playing and clock has advanced.
    pub fn is_playing(&self) -> bool {
        self.current_ms.is_some()
    }
}

/// Plugin. Register this AFTER [`dtx_audio::plugin`].
///
/// Idempotent — safe to add even if `BgmHandle` was already initialized.
pub fn plugin(app: &mut App) {
    app.init_resource::<AudioClock>()
        .add_systems(Update, update_audio_clock_system);
}

/// Re-export of the stop-BGM system for callers that want a "stop" command.
pub use dtx_audio::stop_bgm_system;

/// System: read kira position via `BgmHandle`, write `AudioClock.current_ms`.
///
/// Runs every frame. Cheap: two resource reads, one match.
pub fn update_audio_clock_system(
    audio: Res<Audio>,
    bgm: Res<BgmHandle>,
    mut clock: ResMut<AudioClock>,
) {
    clock.current_ms = dtx_audio::position_ms(&audio, &bgm);
}

/// Pure time-math helpers. No bevy, no kira — testable in isolation.
pub mod math {
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
}

#[cfg(test)]
mod tests {
    use super::math::*;
    use super::*;

    #[test]
    fn clock_default_is_none() {
        let c = AudioClock::default();
        assert_eq!(c.current_ms, None);
        assert_eq!(c.ms_or_zero(), 0);
        assert!(!c.is_playing());
    }

    #[test]
    fn clock_with_value_is_playing() {
        let c = AudioClock {
            current_ms: Some(1234),
        };
        assert!(c.is_playing());
        assert_eq!(c.ms_or_zero(), 1234);
    }

    #[test]
    fn delta_ms_signs() {
        assert_eq!(delta_ms(1000, 800), 200); // early
        assert_eq!(delta_ms(800, 1000), -200); // late
        assert_eq!(delta_ms(1000, 1000), 0); // perfect
    }

    #[test]
    fn chip_time_at_120bpm() {
        // 120 BPM → 2000 ms per measure (4 beats × 60000 / 120 = 2000).
        let t = chip_time_ms(0, 0.0, 120.0);
        assert_eq!(t, 0);
        let t = chip_time_ms(1, 0.0, 120.0);
        assert_eq!(t, 2000);
        let t = chip_time_ms(0, 0.5, 120.0);
        assert_eq!(t, 1000);
    }

    #[test]
    fn chip_time_at_150bpm() {
        // 150 BPM → 1600 ms per measure.
        let t = chip_time_ms(1, 0.0, 150.0);
        assert_eq!(t, 1600);
    }

    #[test]
    fn chip_time_zero_bpm_safe() {
        let t = chip_time_ms(5, 0.5, 0.0);
        assert_eq!(t, 0);
    }

    #[test]
    fn bpm_change_construct() {
        use math::BpmChange;
        let c = BpmChange {
            measure: 5,
            bpm: 180.0,
        };
        assert_eq!(c.measure, 5);
        assert!((c.bpm - 180.0).abs() < 0.01);
    }

    #[test]
    fn no_bpm_changes_matches_constant_bpm() {
        use math::{chip_time_ms, chip_time_ms_with_bpm_changes};
        let t1 = chip_time_ms(5, 0.5, 120.0);
        let t2 = chip_time_ms_with_bpm_changes(5, 0.5, 120.0, &[]);
        assert_eq!(t1, t2);
    }

    #[test]
    fn single_bpm_change_increases_speed() {
        use math::{chip_time_ms_with_bpm_changes, BpmChange};
        // 120 BPM for measures 0..4, 240 BPM for measures 4..8
        let changes = [BpmChange {
            measure: 4,
            bpm: 240.0,
        }];
        // Measure 8 at 240 BPM should be earlier than at constant 120.
        let t_double = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &changes);
        let t_const = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &[]);
        assert!(t_double < t_const);
    }

    #[test]
    fn bpm_change_unsorted_works() {
        use math::{chip_time_ms_with_bpm_changes, BpmChange};
        // Same as above but changes provided out of order.
        let changes = [BpmChange {
            measure: 4,
            bpm: 240.0,
        }];
        let sorted_in = [BpmChange {
            measure: 4,
            bpm: 240.0,
        }];
        let t_a = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &changes);
        let t_b = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &sorted_in);
        assert_eq!(t_a, t_b);
    }

    #[test]
    fn bpm_change_at_same_measure() {
        use math::{chip_time_ms_with_bpm_changes, BpmChange};
        let changes = [
            BpmChange {
                measure: 4,
                bpm: 240.0,
            },
            BpmChange {
                measure: 4,
                bpm: 60.0,
            },
        ];
        // Changes AT the chip's measure are skipped (`>= measure` breaks loop).
        // So partial is computed at base 120 BPM × 4 measures = 8000ms.
        let t = chip_time_ms_with_bpm_changes(4, 0.0, 120.0, &changes);
        assert_eq!(t, 8000);
    }

    #[test]
    fn bpm_change_zero_base_safe() {
        use math::{chip_time_ms_with_bpm_changes, BpmChange};
        let changes = [BpmChange {
            measure: 4,
            bpm: 240.0,
        }];
        let t = chip_time_ms_with_bpm_changes(5, 0.5, 0.0, &changes);
        assert_eq!(t, 0);
    }

    #[test]
    fn measure_duration_helper_basic() {
        // 120 BPM = 2000ms/measure. 4 measures = 8000ms.
        let d = measure_duration_ms(0, 4, 120.0);
        assert!((d - 8000.0).abs() < 0.01);
    }

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
        use math::{
            chip_time_ms_with_bpm_and_bar_changes, chip_time_ms_with_bpm_changes, ChartTiming,
        };
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

    fn measure_duration_ms(start: u32, end: u32, bpm: f64) -> f64 {
        let measures = (end - start) as f64;
        measures * 4.0 * 60_000.0 / bpm
    }
}
