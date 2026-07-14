use bevy::prelude::{Res, Resource};
use game_shell::{PracticeOrigin, PracticeRequest};

use super::session::PracticeSession;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PracticePhase {
    #[default]
    Setup,
    Running,
    Editing,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewState {
    #[default]
    Stopped,
    Playing,
}

#[derive(Debug, Clone)]
pub struct PracticeEditSnapshot {
    pub chart_ms: i64,
    pub session: PracticeSession,
}

#[derive(Resource, Debug, Clone)]
pub struct PracticeFlow {
    pub phase: PracticePhase,
    pub preview: PreviewState,
    pub origin: PracticeOrigin,
    pub edit_snapshot: Option<PracticeEditSnapshot>,
}

impl Default for PracticeFlow {
    fn default() -> Self {
        Self::setup(PracticeOrigin::SongSelect)
    }
}

impl PracticeFlow {
    pub fn setup(origin: PracticeOrigin) -> Self {
        Self {
            phase: PracticePhase::Setup,
            preview: PreviewState::Stopped,
            origin,
            edit_snapshot: None,
        }
    }

    pub fn from_request(request: &PracticeRequest) -> Self {
        Self::setup(request.origin)
    }

    pub fn running() -> Self {
        Self {
            phase: PracticePhase::Running,
            ..Self::default()
        }
    }

    pub fn open_settings(
        mut self,
        chart_ms: i64,
        session: &mut PracticeSession,
    ) -> (Self, PracticeEditSnapshot) {
        session.invalidate_current_attempt();
        let snapshot = PracticeEditSnapshot {
            chart_ms,
            session: session.clone(),
        };
        self.phase = PracticePhase::Editing;
        self.preview = PreviewState::Stopped;
        self.edit_snapshot = Some(snapshot.clone());
        (self, snapshot)
    }
}

pub fn practice_running(flow: Option<Res<PracticeFlow>>) -> bool {
    practice_running_value(flow.as_deref())
}

pub fn practice_surface_open(flow: Option<Res<PracticeFlow>>) -> bool {
    practice_surface_open_value(flow.as_deref())
}

pub fn gameplay_input_active(flow: Option<Res<PracticeFlow>>) -> bool {
    gameplay_input_active_value(flow.as_deref())
}

pub fn chart_clock_active(flow: Option<Res<PracticeFlow>>) -> bool {
    chart_clock_active_value(flow.as_deref())
}

fn practice_running_value(flow: Option<&PracticeFlow>) -> bool {
    flow.is_some_and(|flow| flow.phase == PracticePhase::Running)
}

fn practice_surface_open_value(flow: Option<&PracticeFlow>) -> bool {
    flow.is_some_and(|flow| matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing))
}

fn gameplay_input_active_value(flow: Option<&PracticeFlow>) -> bool {
    flow.is_none_or(|flow| flow.phase == PracticePhase::Running)
}

fn chart_clock_active_value(flow: Option<&PracticeFlow>) -> bool {
    flow.is_none_or(|flow| {
        flow.phase == PracticePhase::Running || flow.preview == PreviewState::Playing
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::PracticeSession;

    #[test]
    fn opening_settings_marks_current_pass_ineligible() {
        let mut session = PracticeSession::default();
        let flow = PracticeFlow::running();

        let (flow, snapshot) = flow.open_settings(2_500, &mut session);

        assert_eq!(flow.phase, PracticePhase::Editing);
        assert_eq!(flow.preview, PreviewState::Stopped);
        assert!(!session.current_attempt_eligible);
        assert_eq!(snapshot.chart_ms, 2_500);
        assert_eq!(flow.edit_snapshot.as_ref().map(|s| s.chart_ms), Some(2_500));
    }

    #[test]
    fn run_conditions_keep_normal_play_active_and_surface_play_inert() {
        assert!(gameplay_input_active_value(None));
        assert!(chart_clock_active_value(None));

        let setup = PracticeFlow::default();
        assert!(!practice_running_value(Some(&setup)));
        assert!(practice_surface_open_value(Some(&setup)));
        assert!(!gameplay_input_active_value(Some(&setup)));
        assert!(!chart_clock_active_value(Some(&setup)));

        let mut preview = setup.clone();
        preview.preview = PreviewState::Playing;
        assert!(!gameplay_input_active_value(Some(&preview)));
        assert!(chart_clock_active_value(Some(&preview)));
    }
}
