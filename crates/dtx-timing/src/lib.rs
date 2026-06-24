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
}
