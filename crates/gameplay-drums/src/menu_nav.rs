//! Publishes which surface owns semantic input onto the shared context stack.
//!
//! The pad→verb mapper, `NavContext`, and `NavGuard` moved to
//! `game_shell::navigation` (menu-nav extraction, 2026-07-15 spec). This
//! module keeps the one job game-shell cannot do: computing the owning
//! context from gameplay-drums' own surface states (editor, capture,
//! calibration, practice phase) and writing it to `NavContextStack` each
//! frame, ordered before the mapper and the router.
//!
//! Transitional: the whole stack is recomputed per frame here so its contents
//! mirror today's `ActiveNavContext` semantics exactly. Screens will take
//! over their own push/pop in later PRs. The transitional `ActiveNavContext`
//! consumed by the pad mapper is derived from the stack top by game-shell's
//! `mirror_stack_to_active`.

use bevy::prelude::*;
use game_shell::navigation::{ActiveNavContext, NavContextStack, NavMapSet, NavStackWriteSet};
use game_shell::{AppState, PauseState};

use crate::editor::bindings_capture::CaptureState;
use crate::editor::calibration::CalibrationState;

// Compat adapter (migration): keeps `gameplay_drums::menu_nav::…` paths alive
// for the Practice branch and integration tests.
pub use game_shell::navigation::{InputSource, NavAction, NavContext, NavGuard, SystemVerb};

pub(super) fn plugin(app: &mut App) {
    // NavGuard/ActiveNavContext/NavContextStack are normally registered by
    // game-shell's navigation plugin; init here too (idempotent) so
    // drums-only test apps that poke `menu_nav::NavGuard` keep working
    // without GameShellPlugin.
    app.init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>()
        .init_resource::<NavContextStack>()
        .add_systems(
            Update,
            publish_nav_context
                .in_set(NavStackWriteSet)
                .before(NavMapSet),
        );
}

/// The context that owns semantic input for the current global state.
/// `NavContext::BindingCapture` while a capture/calibration overlay owns raw
/// hits; `NavContext::LiveGameplay` during live judged play (both mirror to
/// `ActiveNavContext(None)`, keeping pads as gameplay/raw input). `None` only
/// for states with no input surface at all (startup).
fn owning_context(
    app_state: &AppState,
    pause: &PauseState,
    editor_open: bool,
    capture_armed: bool,
    calibrating: bool,
    practice_phase: Option<crate::practice::PracticePhase>,
) -> Option<NavContext> {
    if capture_armed || calibrating {
        return Some(NavContext::BindingCapture);
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
                Some(NavContext::LiveGameplay)
            }
        }
        _ => None,
    }
}

/// Transitional reconcile: the stack contains exactly `[ctx]` (or nothing)
/// until screens own their push/pop.
fn set_stack(stack: &mut NavContextStack, ctx: Option<NavContext>) {
    stack.clear();
    if let Some(ctx) = ctx {
        stack.push(ctx);
    }
}

fn publish_nav_context(
    app_state: Res<State<AppState>>,
    pause: Res<State<PauseState>>,
    editor_open: Res<crate::editor::EditorOpen>,
    capture: Res<CaptureState>,
    calibration: Res<CalibrationState>,
    practice: Option<Res<crate::practice::PracticeFlow>>,
    mut stack: ResMut<NavContextStack>,
) {
    let next = owning_context(
        app_state.get(),
        pause.get(),
        editor_open.0,
        !matches!(*capture, CaptureState::Idle),
        !matches!(*calibration, CalibrationState::Idle),
        practice.as_deref().map(|flow| flow.phase),
    );
    if stack.top() != next {
        set_stack(&mut stack, next);
    }
}

#[cfg(test)]
mod tests {
    use game_shell::navigation::mirror_stack_to_active;

    use super::*;

    /// What the pad mapper will see for a given owning context, via
    /// game-shell's mirror.
    fn mirrored(ctx: Option<NavContext>) -> Option<NavContext> {
        let mut app = App::new();
        let mut stack = NavContextStack::default();
        set_stack(&mut stack, ctx);
        app.insert_resource(stack)
            .init_resource::<ActiveNavContext>()
            .add_systems(Update, mirror_stack_to_active);
        app.update();
        app.world().resource::<ActiveNavContext>().0
    }

    #[test]
    fn live_play_and_capture_own_the_stack_but_mirror_to_none() {
        let live = owning_context(
            &AppState::Performance,
            &PauseState::Running,
            false,
            false,
            false,
            None,
        );
        assert_eq!(live, Some(NavContext::LiveGameplay));
        assert_eq!(mirrored(live), None, "pads stay gameplay input");

        let capture = owning_context(
            &AppState::Performance,
            &PauseState::Running,
            true,
            true,
            false,
            None,
        );
        assert_eq!(capture, Some(NavContext::BindingCapture));
        assert_eq!(mirrored(capture), None, "capture suppresses pad nav");

        let calibrating = owning_context(
            &AppState::SongSelect,
            &PauseState::Running,
            false,
            false,
            true,
            None,
        );
        assert_eq!(calibrating, Some(NavContext::BindingCapture));
        assert_eq!(mirrored(calibrating), None, "calibration suppresses pads");
    }

    #[test]
    fn menu_surfaces_own_the_stack_and_mirror_through() {
        for (state, expected) in [
            (AppState::Title, NavContext::Home),
            (AppState::SongSelect, NavContext::SongSelectSongs),
            (AppState::Result, NavContext::Results),
            (AppState::SongLoading, NavContext::SongLoading),
        ] {
            let ctx = owning_context(&state, &PauseState::Running, false, false, false, None);
            assert_eq!(ctx, Some(expected), "{state:?}");
            assert_eq!(mirrored(ctx), Some(expected), "{state:?}");
        }
        let paused = owning_context(
            &AppState::Performance,
            &PauseState::Paused,
            false,
            false,
            false,
            None,
        );
        assert_eq!(paused, Some(NavContext::PauseMenu));
        assert_eq!(mirrored(paused), Some(NavContext::PauseMenu));
        let editor = owning_context(
            &AppState::Performance,
            &PauseState::Running,
            true,
            false,
            false,
            None,
        );
        assert_eq!(editor, Some(NavContext::LayoutEditor));
        assert_eq!(mirrored(editor), Some(NavContext::LayoutEditor));
        let startup = owning_context(
            &AppState::Startup,
            &PauseState::Running,
            false,
            false,
            false,
            None,
        );
        assert_eq!(startup, None, "no surface: stack stays empty");
        assert_eq!(mirrored(startup), None);
    }

    #[test]
    fn practice_setup_and_editing_own_pad_navigation_but_running_does_not() {
        for phase in [
            crate::practice::PracticePhase::Setup,
            crate::practice::PracticePhase::Editing,
        ] {
            let ctx = owning_context(
                &AppState::Performance,
                &PauseState::Running,
                false,
                false,
                false,
                Some(phase),
            );
            assert_eq!(ctx, Some(NavContext::PracticeSetupSettings), "{phase:?}");
            assert_eq!(
                mirrored(ctx),
                Some(NavContext::PracticeSetupSettings),
                "{phase:?}"
            );
        }
        let running = owning_context(
            &AppState::Performance,
            &PauseState::Running,
            false,
            false,
            false,
            Some(crate::practice::PracticePhase::Running),
        );
        assert_eq!(running, Some(NavContext::LiveGameplay));
        assert_eq!(mirrored(running), None, "pads are gameplay input");
    }

    #[test]
    fn set_stack_reconciles_to_exactly_one_or_zero_contexts() {
        let mut stack = NavContextStack::default();
        set_stack(&mut stack, Some(NavContext::Home));
        assert_eq!(stack.top(), Some(NavContext::Home));
        set_stack(&mut stack, Some(NavContext::SongSelectSongs));
        assert_eq!(stack.top(), Some(NavContext::SongSelectSongs));
        set_stack(&mut stack, None);
        assert_eq!(stack.top(), None, "previous context must not linger");
    }
}
