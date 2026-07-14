//! Applies the practice tempo to the shared playback-rate path.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::AppState;

use super::PracticeSession;
use crate::resources::EffectivePlaybackRate;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        apply_practice_rate
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}

fn apply_practice_rate(
    session: Res<PracticeSession>,
    flow: Option<Res<super::PracticeFlow>>,
    draft: Option<Res<super::PracticeDraft>>,
    mut rate: ResMut<EffectivePlaybackRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut applied: Local<f64>,
) {
    let target = f64::from(
        flow.as_ref()
            .filter(|flow| flow.phase != super::PracticePhase::Running)
            .and(draft.as_deref())
            .map_or_else(|| session.effective_tempo(), super::preview::preview_tempo),
    );
    let next = EffectivePlaybackRate::practice(target);
    // Local starts at 0.0, so the first run always applies (incl. 1.0).
    if (*applied - target).abs() < 1e-9 && *rate == next {
        return;
    }
    *applied = target;
    crate::playback_rate::apply_playback_rate(next, &mut rate, &audio, &bgm, &mut instances);
}
