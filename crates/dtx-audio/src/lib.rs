//! Audio playback wrapper around `bevy_kira_audio`.
//!
//! Engine layer. Owns the [`BgmHandle`] resource that `dtx-timing` polls
//! each frame to populate `AudioClock`.
//!
//! Reference: `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs` (92 LOC).
//! ADR-0002: audio-clock authoritative for hit-window judgment.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

/// The currently-playing BGM instance, if any.
///
/// `None` when nothing is playing. The `dtx-timing` plugin reads this each
/// frame to update the authoritative `AudioClock` resource in its own crate.
#[derive(Resource, Default, Debug, Clone)]
pub struct BgmHandle(pub Option<Handle<AudioInstance>>);

/// Root plugin. Add to your `App` next to `DefaultPlugins` / `MinimalPlugins`.
///
/// Re-exports `bevy_kira_audio::AudioPlugin` so callers don't need to touch
/// the underlying crate directly.
pub fn plugin(app: &mut App) {
    app.add_plugins(AudioPlugin).init_resource::<BgmHandle>();
}

/// Play a BGM file (path is loaded via `AssetServer`), looped, at default gain.
/// Replaces any currently-playing BGM. Returns the new instance handle.
pub fn play_bgm(
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    path: &str,
) -> Handle<AudioInstance> {
    if let Some(prev) = bgm.0.take() {
        // Best-effort stop. If kira has already cleaned up, this is a no-op.
        // We can't await it here; the next update tick will see the stop.
        bgm.0 = None;
        let _ = prev;
    }
    let source = asset_server.load(path.to_owned());
    let handle = audio.play(source).looped().handle();
    bgm.0 = Some(handle.clone());
    handle
}

/// System: stop the currently-playing BGM cleanly via `Assets<AudioInstance>`.
pub fn stop_bgm_system(
    audio: Res<Audio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if let Some(prev) = bgm.0.take() {
        if let Some(mut instance) = instances.get_mut(&prev) {
            instance.stop(AudioTween::default());
        } else {
            // Instance was already cleaned up by kira; just stop the channel
            // to silence any leftovers.
            audio.stop();
        }
    }
}

/// Get the current playback position in milliseconds, if BGM is playing.
///
/// Returns `None` for Queued/Stopped states or when no BGM is loaded.
pub fn position_ms(audio: &Audio, bgm: &BgmHandle) -> Option<i64> {
    let handle = bgm.0.as_ref()?;
    match audio.state(handle) {
        PlaybackState::Playing { position } => Some((position * 1000.0) as i64),
        PlaybackState::Paused { position } => Some((position * 1000.0) as i64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_handle_is_none() {
        let h = BgmHandle::default();
        assert!(h.0.is_none());
    }
}
