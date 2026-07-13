use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::{AppState, PracticeIntent};

use crate::orchestrator::DrumsEnterSet;
use crate::resources::EffectivePlaybackRate;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        configure_playback_rate.before(DrumsEnterSet),
    )
    .add_systems(OnExit(AppState::Performance), reset_playback_rate);
}

pub(crate) fn apply_playback_rate(
    next: EffectivePlaybackRate,
    rate: &mut EffectivePlaybackRate,
    audio: &Audio,
    bgm: &dtx_audio::BgmHandle,
    instances: &mut Assets<AudioInstance>,
) {
    *rate = next;
    audio.set_playback_rate(next.value);
    if let Some(handle) = &bgm.instance {
        if let Some(mut instance) = instances.get_mut(handle) {
            instance.set_playback_rate(next.value, AudioTween::default());
        }
    }
}

pub(crate) fn initial_playback_rate(
    configured_play_speed: f64,
    practice: bool,
) -> EffectivePlaybackRate {
    if practice {
        EffectivePlaybackRate::practice(1.0)
    } else {
        EffectivePlaybackRate::normal(configured_play_speed)
    }
}

fn configure_playback_rate(
    intent: Res<PracticeIntent>,
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let cfg = dtx_config::load(&dtx_config::default_path());
    let next = initial_playback_rate(
        f64::from(dtx_config::play_speed_multiplier(cfg.gameplay.play_speed)),
        intent.is_requested(),
    );
    apply_playback_rate(next, &mut rate, &audio, &bgm, &mut instances);
}

fn reset_playback_rate(
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    apply_playback_rate(
        EffectivePlaybackRate::native(),
        &mut rate,
        &audio,
        &bgm,
        &mut instances,
    );
}

#[cfg(test)]
mod tests {
    use super::initial_playback_rate;
    use crate::resources::PlaybackRateSource;

    #[test]
    fn normal_initial_rate_uses_configured_play_speed() {
        let rate = initial_playback_rate(0.75, false);
        assert_eq!(rate.source, PlaybackRateSource::NormalPlaySetting);
        assert!((rate.value - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn practice_initial_rate_overrides_normal_setting() {
        let rate = initial_playback_rate(0.75, true);
        assert_eq!(rate.source, PlaybackRateSource::PracticeTempo);
        assert!((rate.value - 1.0).abs() < f64::EPSILON);
    }
}
