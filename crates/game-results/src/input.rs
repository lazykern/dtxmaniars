//! Results screen input.

use bevy::prelude::*;
use game_shell::{request_transition, AppState, NavVerb, TransitionRequest};

pub(crate) fn result_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<game_shell::NavAction>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    // Either pad verb continues; the mapper's screen-enter grace keeps the
    // song's last note from skipping this screen.
    let pad = actions
        .read()
        .any(|a| matches!(a.verb, NavVerb::Confirm | NavVerb::Back));
    if pad || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}

/// The verb the cursor sits on. Resets to Continue on every Result enter.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ResultVerb {
    #[default]
    Continue,
    Retry,
    Practice,
}

#[allow(dead_code)] // wired into the driver in Task 7
impl ResultVerb {
    fn prev(self) -> Self {
        match self {
            ResultVerb::Continue | ResultVerb::Retry => ResultVerb::Continue,
            ResultVerb::Practice => ResultVerb::Retry,
        }
    }

    fn next(self) -> Self {
        match self {
            ResultVerb::Continue => ResultVerb::Retry,
            ResultVerb::Retry | ResultVerb::Practice => ResultVerb::Practice,
        }
    }
}

/// What one nav verb means given the current cursor. Pure.
#[allow(dead_code)] // wired into the driver in Task 7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultAction {
    Moved(ResultVerb),
    Activate(ResultVerb),
    ContinueNow,
    PracticeNow,
    None,
}

/// HH/CY (Up/Down) and keyboard ←/→ (mapped to Up/Down by the driver) move
/// the cursor, clamped at the ends. BD/Enter activates, SD/Esc continues,
/// FT jumps to practice.
#[allow(dead_code)] // wired into the driver in Task 7
pub(crate) fn reduce_result_nav(cursor: ResultVerb, verb: NavVerb) -> ResultAction {
    match verb {
        NavVerb::Up | NavVerb::Dec => {
            let moved = cursor.prev();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        NavVerb::Down | NavVerb::Inc => {
            let moved = cursor.next();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        NavVerb::Confirm => ResultAction::Activate(cursor),
        NavVerb::Back => ResultAction::ContinueNow,
        NavVerb::Practice => ResultAction::PracticeNow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_result_nav_moves_and_clamps() {
        use ResultVerb::{Continue, Practice, Retry};
        // Clamped at both ends, no wrap.
        assert_eq!(reduce_result_nav(Continue, NavVerb::Up), ResultAction::None);
        assert_eq!(
            reduce_result_nav(Practice, NavVerb::Down),
            ResultAction::None
        );
        // Moves along Continue ↔ Retry ↔ Practice.
        assert_eq!(
            reduce_result_nav(Continue, NavVerb::Down),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Down),
            ResultAction::Moved(Practice)
        );
        assert_eq!(
            reduce_result_nav(Practice, NavVerb::Up),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Up),
            ResultAction::Moved(Continue)
        );
        // Dec/Inc alias the same axis (keyboard adjust verbs).
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Dec),
            ResultAction::Moved(Continue)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Inc),
            ResultAction::Moved(Practice)
        );
    }

    #[test]
    fn reduce_result_nav_confirm_activates_cursor() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, NavVerb::Confirm),
            ResultAction::Activate(ResultVerb::Retry)
        );
    }

    #[test]
    fn reduce_result_nav_back_and_practice_shortcuts() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, NavVerb::Back),
            ResultAction::ContinueNow
        );
        assert_eq!(
            reduce_result_nav(ResultVerb::Continue, NavVerb::Practice),
            ResultAction::PracticeNow
        );
    }
}
