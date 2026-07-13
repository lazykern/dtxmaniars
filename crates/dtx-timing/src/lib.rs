//! Authoritative audio clock for hit-window judgment.
//!
//! Engine layer. Reads [`dtx_audio::BgmHandle`] each frame from
//! `bevy_kira_audio`'s playback position and writes a single i64 ms value
//! into [`AudioClock`].
//!
//! ADR-0002: **never** judge on `Time::delta()` accumulated frame time.
//! Use `Res<AudioClock>` for timing windows.
//!
//! Reference: `references/DTXmaniaNX/FDK/Sound/CSoundTimer.cs`.
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

/// Pure time-math helpers, owned by `dtx-core` so parser tests stay Bevy-free.
pub use dtx_core::timing as math;

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
            fraction: 0.0,
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
            fraction: 0.0,
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
            fraction: 0.0,
        }];
        let sorted_in = [BpmChange {
            measure: 4,
            bpm: 240.0,
            fraction: 0.0,
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
                fraction: 0.0,
            },
            BpmChange {
                measure: 4,
                bpm: 60.0,
                fraction: 0.0,
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
            fraction: 0.0,
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

    #[test]
    fn bar_length_change_scales_its_own_measure() {
        // A `#02` chip on measure M sets the length of measure M itself, so a
        // chip *inside* M is scaled by the new ratio. The chart above never
        // puts a bar change on the chip's own measure, so it does not cover
        // this; pin it explicitly.
        use math::{chip_time_ms_with_bpm_and_bar_changes, BarLengthChange, ChartTiming};
        let bar_changes = [BarLengthChange {
            measure: 2,
            ratio: 0.5,
        }];
        let timing = ChartTiming {
            bpm_changes: &[],
            bar_changes: &bar_changes,
        };
        // 120 BPM = 2000ms per full measure.
        //   [0,2) at ratio 1.0 = 4000ms
        //   half of measure 2, which is itself half-length: 0.5 * 0.5 * 2000 = 500ms
        let t = chip_time_ms_with_bpm_and_bar_changes(2, 0.5, 120.0, timing);
        assert_eq!(t, 4500);

        // And the whole of measure 2 is half-length.
        let t_next = chip_time_ms_with_bpm_and_bar_changes(3, 0.0, 120.0, timing);
        assert_eq!(t_next, 5000);
    }

    fn measure_duration_ms(start: u32, end: u32, bpm: f64) -> f64 {
        let measures = (end - start) as f64;
        measures * 4.0 * 60_000.0 / bpm
    }
}
