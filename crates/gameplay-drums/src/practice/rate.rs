//! Applies the practice rate to audio (BGM instance + main channel) and
//! to the gameplay clock via `AudioRate`.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::AppState;

use super::PracticeSession;
use crate::resources::AudioRate;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        apply_practice_rate
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), reset_audio_rate);
}

fn apply_practice_rate(
    session: Res<PracticeSession>,
    mut rate: ResMut<AudioRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut applied: Local<f64>,
) {
    let target = session.transport.user_tempo as f64;
    // Local starts at 0.0, so the first run always applies (incl. 1.0).
    if (*applied - target).abs() < 1e-9 {
        return;
    }
    *applied = target;
    rate.0 = target;
    // Channel-wide: retunes currently playing sounds and is inherited by
    // future plays in the main channel (keysounds, SE, restarted BGM).
    audio.set_playback_rate(target);
    // Belt and braces for the tracked BGM instance (immediate tween).
    if let Some(handle) = &bgm.instance {
        if let Some(mut inst) = instances.get_mut(handle) {
            inst.set_playback_rate(target, AudioTween::default());
        }
    }
}

fn reset_audio_rate(mut rate: ResMut<AudioRate>, audio: Res<Audio>) {
    rate.0 = 1.0;
    audio.set_playback_rate(1.0);
}
