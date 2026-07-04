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
    pub fn chip_time_ms_with_speed(
        measure: u32,
        fraction: f32,
        bpm: f32,
        play_speed: f32,
    ) -> i64 {
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

    /// Compute chip playback time (ms) with a list of BPM change events.
    ///
    /// Algorithm (mirrors `CChip.cs:ComputeTime` in BocuD):
    /// 1. Sort `bpm_changes` by measure.
    /// 2. For each interval (start_measure, end_measure] where start_measure is
    ///    the previous BPM change (or 0) and end_measure is the next one, use
    ///    the BPM from `start_measure`'s change (or `base_bpm` if no prior).
    /// 3. If the chip's measure falls past the last change, use the last
    ///    change's BPM.
    /// 4. Sum interval durations in ms, then add the final partial-measure.
    ///
    /// Pass `bpm_changes = &[]` to behave like [`chip_time_ms`].
    pub fn chip_time_ms_with_bpm_changes(
        measure: u32,
        fraction: f32,
        base_bpm: f32,
        bpm_changes: &[BpmChange],
    ) -> i64 {
        if base_bpm <= 0.0 {
            return 0;
        }
        // Sort changes by measure (BocuD does this once on chart load).
        let mut sorted: Vec<BpmChange> = bpm_changes.to_vec();
        sorted.sort_by_key(|c| c.measure);

        let mut total_ms: f64 = 0.0;
        let mut current_bpm: f64 = base_bpm as f64;
        let mut interval_start: u32 = 0;

        for ch in &sorted {
            if ch.measure >= measure {
                break;
            }
            if ch.measure > interval_start {
                // Close out the interval [interval_start, ch.measure) at current_bpm.
                total_ms += measure_duration_ms(interval_start, ch.measure, current_bpm);
            } else if ch.measure == interval_start {
                // BPM changes at the same measure → ignore prior duration.
            }
            current_bpm = ch.bpm as f64;
            interval_start = ch.measure;
        }

        // Final partial: [interval_start, measure + fraction) at current_bpm.
        let partial_measures = (measure - interval_start) as f64 + fraction as f64;
        total_ms += partial_measures * 4.0 * 60_000.0 / current_bpm;

        total_ms as i64
    }

    /// Compute the duration in ms of measures [start, end) at a given BPM.
    #[inline]
    fn measure_duration_ms(start: u32, end: u32, bpm: f64) -> f64 {
        if bpm <= 0.0 {
            return 0.0;
        }
        let measures = (end - start) as f64;
        measures * 4.0 * 60_000.0 / bpm
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

    fn measure_duration_ms(start: u32, end: u32, bpm: f64) -> f64 {
        let measures = (end - start) as f64;
        measures * 4.0 * 60_000.0 / bpm
    }
}
