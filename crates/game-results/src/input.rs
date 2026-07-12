//! Results screen input.

use bevy::prelude::*;
use game_shell::{request_transition, AppState, TransitionRequest};

pub(crate) fn result_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<game_shell::NavAction>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    use game_shell::NavVerb;
    // Either pad verb continues; the mapper's screen-enter grace keeps the
    // song's last note from skipping this screen.
    let pad = actions
        .read()
        .any(|a| matches!(a.verb, NavVerb::Confirm | NavVerb::Back));
    if pad || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}
