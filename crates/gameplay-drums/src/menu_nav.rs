//! Publishes which menu surface owns pad navigation.
//!
//! The pad→verb mapper, `NavContext`, and `NavGuard` moved to
//! `game_shell::navigation` (menu-nav extraction, 2026-07-15 spec). This
//! module keeps the one job game-shell cannot do: computing the active
//! context from gameplay-drums' own surface states (editor, capture,
//! calibration, practice phase) and publishing it each frame, ordered before
//! the mapper.

use bevy::prelude::*;
use game_shell::navigation::{ActiveNavContext, NavMapSet};
use game_shell::{AppState, PauseState};

use crate::editor::bindings_capture::CaptureState;
use crate::editor::calibration::CalibrationState;

// Compat adapter (migration): keeps `gameplay_drums::menu_nav::…` paths alive
// for the Practice branch and integration tests.
pub use game_shell::navigation::{InputSource, NavAction, NavContext, NavGuard, SystemVerb};

pub(super) fn plugin(app: &mut App) {
    // NavGuard/ActiveNavContext are normally registered by game-shell's
    // navigation plugin; init here too (idempotent) so drums-only test apps
    // that poke `menu_nav::NavGuard` keep working without GameShellPlugin.
    app.init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>()
        .add_systems(Update, publish_nav_context.before(NavMapSet));
}

/// `None` = pads are gameplay input, or a capture/calibration overlay owns raw hits.
fn active_context(
    app_state: &AppState,
    pause: &PauseState,
    editor_open: bool,
    capture_armed: bool,
    calibrating: bool,
    practice_phase: Option<crate::practice::PracticePhase>,
) -> Option<NavContext> {
    if capture_armed || calibrating {
        return None;
    }
    match app_state {
        AppState::Title => Some(NavContext::Home),
        AppState::SongSelect => Some(NavContext::SongSelectSongs),
        AppState::Result => Some(NavContext::Results),
        AppState::SongLoading => Some(NavContext::SongLoading),
        AppState::Performance => {
            if editor_open {
                Some(NavContext::LayoutEditor)
            } else if *pause == PauseState::Paused {
                Some(NavContext::PauseMenu)
            } else if matches!(
                practice_phase,
                Some(
                    crate::practice::PracticePhase::Setup | crate::practice::PracticePhase::Editing
                )
            ) {
                Some(NavContext::PracticeSetupSettings)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn publish_nav_context(
    app_state: Res<State<AppState>>,
    pause: Res<State<PauseState>>,
    editor_open: Res<crate::editor::EditorOpen>,
    capture: Res<CaptureState>,
    calibration: Res<CalibrationState>,
    practice: Option<Res<crate::practice::PracticeFlow>>,
    mut ctx: ResMut<ActiveNavContext>,
) {
    let next = active_context(
        app_state.get(),
        pause.get(),
        editor_open.0,
        !matches!(*capture, CaptureState::Idle),
        !matches!(*calibration, CalibrationState::Idle),
        practice.as_deref().map(|flow| flow.phase),
    );
    if ctx.0 != next {
        ctx.0 = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_context_during_live_play_or_capture() {
        assert_eq!(
            active_context(
                &AppState::Performance,
                &PauseState::Running,
                false,
                false,
                false,
                None,
            ),
            None
        );
        assert_eq!(
            active_context(
                &AppState::Performance,
                &PauseState::Running,
                true,
                true,
                false,
                None,
            ),
            None
        );
        assert_eq!(
            active_context(
                &AppState::SongSelect,
                &PauseState::Running,
                false,
                false,
                true,
                None,
            ),
            None
        );
        assert_eq!(
            active_context(
                &AppState::Performance,
                &PauseState::Paused,
                false,
                false,
                false,
                None,
            ),
            Some(NavContext::PauseMenu)
        );
        assert_eq!(
            active_context(
                &AppState::Performance,
                &PauseState::Running,
                true,
                false,
                false,
                None,
            ),
            Some(NavContext::LayoutEditor)
        );
        assert_eq!(
            active_context(
                &AppState::SongLoading,
                &PauseState::Running,
                false,
                false,
                false,
                None,
            ),
            Some(NavContext::SongLoading)
        );
    }

    #[test]
    fn practice_setup_and_editing_own_pad_navigation_but_running_does_not() {
        for phase in [
            crate::practice::PracticePhase::Setup,
            crate::practice::PracticePhase::Editing,
        ] {
            assert_eq!(
                active_context(
                    &AppState::Performance,
                    &PauseState::Running,
                    false,
                    false,
                    false,
                    Some(phase),
                ),
                Some(NavContext::PracticeSetupSettings),
                "{phase:?}",
            );
        }
        assert_eq!(
            active_context(
                &AppState::Performance,
                &PauseState::Running,
                false,
                false,
                false,
                Some(crate::practice::PracticePhase::Running),
            ),
            None,
        );
    }
}
