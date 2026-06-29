//! Screen transition director — gates AppState changes behind fade overlay.
//!
//! ADR-0014: 300ms OutQuint fade on all screen changes.

use bevy::prelude::*;
use dtx_ui::{FadePhase, ScreenFade};

use crate::AppState;

/// Request a screen transition (preferred over raw `NextState`).
#[derive(Message, Debug, Clone, Copy)]
pub struct TransitionRequest(pub AppState);

/// Pending target state while fade-out completes.
#[derive(Resource, Default)]
pub struct PendingTransition {
    pub target: Option<AppState>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PendingTransition>()
        .add_message::<TransitionRequest>()
        .add_systems(
            Update,
            (
                collect_transition_requests,
                drive_transition_fade.after(collect_transition_requests),
            ),
        );
}

fn collect_transition_requests(
    mut requests: MessageReader<TransitionRequest>,
    mut pending: ResMut<PendingTransition>,
) {
    for req in requests.read() {
        pending.target = Some(req.0);
    }
}

fn drive_transition_fade(
    mut fade: ResMut<ScreenFade>,
    mut pending: ResMut<PendingTransition>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    let delta_ms = time.delta_secs() * 1000.0;

    match fade.phase {
        FadePhase::Idle => {
            if pending.target.is_some() && !fade.is_busy() {
                fade.start_fade_out();
            }
        }
        FadePhase::FadeOut => {
            if fade.tick(delta_ms) {
                if let Some(target) = pending.target.take() {
                    next.set(target);
                }
                fade.start_fade_in();
            }
        }
        FadePhase::FadeIn => {
            if fade.tick(delta_ms) {
                fade.finish();
            }
        }
    }
}

/// Helper for input systems — request transition instead of setting NextState directly.
pub fn request_transition(requests: &mut MessageWriter<TransitionRequest>, target: AppState) {
    requests.write(TransitionRequest(target));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameShellPlugin;
    use bevy::app::App;
    use dtx_ui::ScreenFade;

    #[test]
    fn transition_request_carries_state() {
        let r = TransitionRequest(AppState::Title);
        assert_eq!(r.0, AppState::Title);
    }

    #[test]
    fn game_shell_plugin_registers_screen_fade() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(GameShellPlugin);
        assert!(app.world().get_resource::<ScreenFade>().is_some());
        assert!(app.world().get_resource::<PendingTransition>().is_some());
    }
}
