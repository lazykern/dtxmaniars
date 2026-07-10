//! Two-track audio crossfade helpers for the preview BGM.
//!
//! ADR-0015 Phase 2. Matches osu-lazer's `MusicController.changeTrack`
//! constants: 150ms fade-out + 220ms fade-in with 30ms pre-roll delay
//! (`osu-lazer/osu.Game/Overlays/MusicController.cs:41,519-520`).
//!
//! These are thin wrappers over `bevy_kira_audio::AudioInstance::set_decibels`.
//! The state machine in [`crate::preview::PreviewState`] schedules the
//! delayed fade-in; this module only knows the durations and the kira
//! tween shapes.
//!
//! Layer: Engine. No Pure or Game deps.

use std::time::Duration;

use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::AudioInstance;

/// Fade-out duration in milliseconds (osu `MusicController.cs:520`).
pub const PREVIEW_FADE_OUT_MS: u32 = 150;

/// Fade-in duration in milliseconds (osu `MusicController.cs:519`).
pub const PREVIEW_FADE_IN_MS: u32 = 220;

/// Pre-roll delay before fade-in starts, in milliseconds
/// (osu `DELAY_BEFORE_FADE = 30`, `MusicController.cs:41`).
pub const PREVIEW_FADE_DELAY_MS: u32 = 30;

/// Start a fade-out on the given audio instance using `Easing::Out`,
/// then stop it. Matches osu-lazer's `MusicController.changeTrack`:
/// `lastTrack.VolumeTo(0, 150, Easing.Out)`
/// (`osu-lazer/osu.Game/Overlays/MusicController.cs:520`).
///
/// Volume tweens to silence over `ms` milliseconds; the underlying
/// kira sound handle is removed from the audio engine once the tween
/// completes. This is the *correct* way to fade-and-release a preview
/// — `set_decibels(-60)` only mutes, leaving the handle alive and
/// holding the audio source in kira's mixer (leak under mash).
///
/// No-op if the instance no longer exists in `instances` (e.g. dropped
/// during a re-entrant swap).
pub fn stop_with_fade(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
    ms: u32,
) {
    if let Some(mut instance) = instances.get_mut(handle) {
        instance.stop(AudioTween::new(
            Duration::from_millis(ms as u64),
            AudioEasing::OutPowi(2),
        ));
    }
}

/// Volume-only fade-out with `Easing::Out`. Use this only when you
/// intend to keep the kira instance alive afterwards (rare — e.g.
/// `play()` mutes a fresh handle to -60dB before the crossfade
/// fade-in lifts it back). For releasing a preview, prefer
/// [`stop_with_fade`].
pub fn start_fade_out(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
    ms: u32,
) {
    if let Some(mut instance) = instances.get_mut(handle) {
        instance.set_decibels(
            -60.0,
            AudioTween::new(Duration::from_millis(ms as u64), AudioEasing::OutPowi(2)),
        );
    }
}

/// Start a linear fade-in on the given audio instance, after a delay.
///
/// The tween runs over `delay_ms + fade_ms` total. During the first
/// `delay_ms` the value is held near the start (the current value, which
/// the caller is expected to have set to -60dB). The value then
/// interpolates linearly to 0dB across the remaining `fade_ms`.
///
/// For precise "held then tween" semantics (silent for exactly
/// `delay_ms`, then linear to 0dB over `fade_ms`), drive the call from
/// the [`crate::preview::PreviewState`] state machine at the exact
/// `delay_ms` tick boundary.
///
/// Returns `false` if the instance does not exist yet, allowing callers to retry.
pub fn start_fade_in_with_delay(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
    fade_ms: u32,
    delay_ms: u32,
) -> bool {
    start_fade_in_with_delay_to_db(instances, handle, fade_ms, delay_ms, 0.0)
}

pub fn start_fade_in_with_delay_to_db(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
    fade_ms: u32,
    delay_ms: u32,
    target_db: f32,
) -> bool {
    let Some(mut instance) = instances.get_mut(handle) else {
        return false;
    };
    let total = Duration::from_millis(delay_ms as u64 + fade_ms as u64);
    instance.set_decibels(target_db, AudioTween::linear(total));
    true
}

/// Mute an audio instance using the project's "instant" convention.
///
/// Uses `AudioTween::default()` which is a 10ms linear fade — below
/// human perception threshold, so audibly instant. Matches the
/// convention in `crate::stop_bgm` and friends.
pub fn mute(instances: &mut Assets<AudioInstance>, handle: &Handle<AudioInstance>) {
    if let Some(mut instance) = instances.get_mut(handle) {
        instance.set_decibels(-60.0, AudioTween::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::Handle;

    #[test]
    fn constants_match_osu_reference() {
        // osu-lazer/osu.Game/Overlays/MusicController.cs:41,519-520
        assert_eq!(PREVIEW_FADE_OUT_MS, 150);
        assert_eq!(PREVIEW_FADE_IN_MS, 220);
        assert_eq!(PREVIEW_FADE_DELAY_MS, 30);
    }

    #[test]
    fn helpers_tolerate_missing_instance() {
        // A default handle won't resolve to a real AudioInstance in a
        // bare Assets<AudioInstance>. Each helper must short-circuit
        // gracefully rather than panic.
        let mut instances = Assets::<AudioInstance>::default();
        let handle = Handle::<AudioInstance>::default();

        start_fade_out(&mut instances, &handle, 150);
        start_fade_in_with_delay(&mut instances, &handle, 220, 30);
        mute(&mut instances, &handle);
        stop_with_fade(&mut instances, &handle, 150);
    }
}
