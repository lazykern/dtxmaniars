//! Drum pads → `NavAction`: pads navigate menus.
//!
//! Consumes this crate's resolved [`LaneHit`]s (velocity threshold and bindings
//! already applied by the producers) and emits `game_shell::NavAction` with
//! `NavSource::Pad` while a menu context is active. During live play — and
//! while bindings capture or the calibration overlay owns raw hits — pads stay
//! gameplay input and no actions are emitted.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_shell::{AppState, NavAction, NavSource, NavVerb, PauseState};

use crate::editor::bindings_capture::CaptureState;
use crate::editor::calibration::CalibrationState;
use crate::events::LaneHit;

/// Minimum gap between accepted pad nav actions (double-trigger/flam guard).
const DEBOUNCE: Duration = Duration::from_millis(80);
/// Pad nav ignored for this long after entering a screen/context.
const ENTER_GRACE: Duration = Duration::from_millis(500);

/// Which menu surface pads are currently navigating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavContext {
    /// Title screen.
    Title,
    /// Song select wheel.
    SongSelect,
    /// Post-play results screen.
    Result,
    /// Pause overlay during a performance.
    Paused,
    /// Customize (settings) overlay during a performance.
    Editor,
}

/// GITADORA-ish convention. Lane ids per [`crate::lane_map::LANE_ORDER`].
pub(crate) fn verb_for_lane(lane: u8) -> Option<NavVerb> {
    match lane {
        0 | 7 => Some(NavVerb::Up),
        6 | 8 => Some(NavVerb::Down),
        2 => Some(NavVerb::Confirm),
        1 => Some(NavVerb::Back),
        5 => Some(NavVerb::Practice),
        _ => None,
    }
}

/// Debounce + screen-enter grace bookkeeping.
#[derive(Resource, Debug, Default)]
pub struct NavGuard {
    context: Option<NavContext>,
    entered_at: Option<Instant>,
    last_accept: Option<Instant>,
}

impl NavGuard {
    /// Record the active context; resets the grace window on change.
    pub fn enter_context(&mut self, ctx: NavContext, now: Instant) {
        if self.context != Some(ctx) {
            self.context = Some(ctx);
            self.entered_at = Some(now);
            self.last_accept = None;
        }
    }

    /// Forget the active context — pads are gameplay input again.
    pub fn clear_context(&mut self) {
        self.context = None;
        self.entered_at = None;
        self.last_accept = None;
    }

    /// True if a pad hit at `now` may become a [`NavAction`].
    pub fn accept(&mut self, now: Instant) -> bool {
        let Some(entered) = self.entered_at else {
            return false;
        };
        if now.saturating_duration_since(entered) < ENTER_GRACE {
            return false;
        }
        if let Some(last) = self.last_accept {
            if now.saturating_duration_since(last) < DEBOUNCE {
                return false;
            }
        }
        self.last_accept = Some(now);
        true
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NavGuard>()
        .add_systems(Update, pad_nav_mapper);
}

/// `None` = pads are gameplay input, or a capture/calibration overlay owns raw hits.
fn active_context(
    app_state: &AppState,
    pause: &PauseState,
    editor_open: bool,
    capture_armed: bool,
    calibrating: bool,
) -> Option<NavContext> {
    if capture_armed || calibrating {
        return None;
    }
    match app_state {
        AppState::Title => Some(NavContext::Title),
        AppState::SongSelect => Some(NavContext::SongSelect),
        AppState::Result => Some(NavContext::Result),
        AppState::Performance => {
            if editor_open {
                Some(NavContext::Editor)
            } else if *pause == PauseState::Paused {
                Some(NavContext::Paused)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn pad_nav_mapper(
    app_state: Res<State<AppState>>,
    pause: Res<State<PauseState>>,
    editor_open: Res<crate::editor::EditorOpen>,
    capture: Res<CaptureState>,
    calibration: Res<CalibrationState>,
    mut hits: MessageReader<LaneHit>,
    mut guard: ResMut<NavGuard>,
    mut out: MessageWriter<NavAction>,
) {
    let now = Instant::now();
    let ctx = active_context(
        app_state.get(),
        pause.get(),
        editor_open.0,
        !matches!(*capture, CaptureState::Idle),
        !matches!(*calibration, CalibrationState::Idle),
    );
    let Some(ctx) = ctx else {
        guard.clear_context();
        hits.clear();
        return;
    };
    guard.enter_context(ctx, now);
    for hit in hits.read() {
        let Some(verb) = verb_for_lane(hit.lane) else {
            continue;
        };
        if !guard.accept(now) {
            continue;
        }
        out.write(NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_shell::NavVerb;

    #[test]
    fn lane_verbs_follow_gitadora_convention() {
        assert_eq!(verb_for_lane(0), Some(NavVerb::Up)); // HH close
        assert_eq!(verb_for_lane(7), Some(NavVerb::Up)); // HH open
        assert_eq!(verb_for_lane(6), Some(NavVerb::Down)); // CY
        assert_eq!(verb_for_lane(8), Some(NavVerb::Down)); // RD
        assert_eq!(verb_for_lane(2), Some(NavVerb::Confirm)); // BD
        assert_eq!(verb_for_lane(1), Some(NavVerb::Back)); // SD
        assert_eq!(verb_for_lane(5), Some(NavVerb::Practice)); // FT
        assert_eq!(verb_for_lane(3), None); // HT unmapped
        assert_eq!(verb_for_lane(10), None); // LP unmapped
    }

    #[test]
    fn guard_enforces_grace_then_debounce() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelect, t0);
        assert!(!g.accept(t0 + std::time::Duration::from_millis(100)));
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        assert!(!g.accept(t1 + std::time::Duration::from_millis(40)));
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
    }

    #[test]
    fn guard_resets_grace_on_context_change() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelect, t0);
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        g.enter_context(NavContext::SongSelect, t1);
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
        g.enter_context(NavContext::Result, t1 + std::time::Duration::from_millis(200));
        assert!(!g.accept(t1 + std::time::Duration::from_millis(300)));
    }

    #[test]
    fn no_context_during_live_play_or_capture() {
        assert_eq!(
            active_context(&AppState::Performance, &PauseState::Running, false, false, false),
            None
        );
        assert_eq!(
            active_context(&AppState::Performance, &PauseState::Running, true, true, false),
            None
        );
        assert_eq!(
            active_context(&AppState::SongSelect, &PauseState::Running, false, false, true),
            None
        );
        assert_eq!(
            active_context(&AppState::Performance, &PauseState::Paused, false, false, false),
            Some(NavContext::Paused)
        );
        assert_eq!(
            active_context(&AppState::Performance, &PauseState::Running, true, false, false),
            Some(NavContext::Editor)
        );
        assert_eq!(
            active_context(&AppState::SongLoading, &PauseState::Running, false, false, false),
            None
        );
    }
}
