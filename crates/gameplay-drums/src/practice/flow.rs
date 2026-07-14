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
    use game_shell::PracticeSeed;

    #[test]
    fn defaults_are_stopped_song_select_setup_without_a_snapshot() {
        assert_eq!(PracticePhase::default(), PracticePhase::Setup);
        assert_eq!(PreviewState::default(), PreviewState::Stopped);

        let flow = PracticeFlow::default();
        assert_eq!(flow.phase, PracticePhase::Setup);
        assert_eq!(flow.preview, PreviewState::Stopped);
        assert_eq!(flow.origin, PracticeOrigin::SongSelect);
        assert!(flow.edit_snapshot.is_none());

        let running = PracticeFlow::running();
        assert_eq!(running.phase, PracticePhase::Running);
        assert_eq!(running.preview, PreviewState::Stopped);
        assert_eq!(running.origin, PracticeOrigin::SongSelect);
        assert!(running.edit_snapshot.is_none());
    }

    #[test]
    fn request_and_edit_transitions_preserve_every_origin() {
        for origin in [
            PracticeOrigin::SongSelect,
            PracticeOrigin::Results,
            PracticeOrigin::NormalPause,
        ] {
            let request = PracticeRequest {
                origin,
                seed: PracticeSeed::Manual,
            };
            let mut flow = PracticeFlow::from_request(&request);
            assert_eq!(flow.origin, origin);
            assert_eq!(flow.phase, PracticePhase::Setup);
            assert_eq!(flow.preview, PreviewState::Stopped);
            assert!(flow.edit_snapshot.is_none());

            flow.phase = PracticePhase::Running;
            flow.preview = PreviewState::Playing;
            let (flow, _) = flow.open_settings(2_500, &mut PracticeSession::default());
            assert_eq!(flow.origin, origin);
        }
    }

    #[test]
    fn opening_settings_stops_preview_and_freezes_the_invalidated_session() {
        let mut session = PracticeSession::default();
        session.transport.user_tempo = 0.75;
        let flow = PracticeFlow {
            phase: PracticePhase::Running,
            preview: PreviewState::Playing,
            origin: PracticeOrigin::Results,
            edit_snapshot: None,
        };

        let (flow, snapshot) = flow.open_settings(2_500, &mut session);
        session.transport.user_tempo = 1.25;

        assert_eq!(flow.phase, PracticePhase::Editing);
        assert_eq!(flow.preview, PreviewState::Stopped);
        assert_eq!(flow.origin, PracticeOrigin::Results);
        assert!(!session.current_attempt_eligible);
        assert_eq!(snapshot.chart_ms, 2_500);
        assert!(!snapshot.session.current_attempt_eligible);
        assert_eq!(snapshot.session.transport.user_tempo, 0.75);
        let stored = flow.edit_snapshot.as_ref().expect("snapshot stored");
        assert_eq!(stored.chart_ms, snapshot.chart_ms);
        assert_eq!(stored.session.transport.user_tempo, 0.75);
        assert!(!stored.session.current_attempt_eligible);
    }

    #[test]
    fn run_conditions_keep_normal_play_active_without_practice_surfaces() {
        assert!(!practice_running_value(None));
        assert!(!practice_surface_open_value(None));
        assert!(gameplay_input_active_value(None));
        assert!(chart_clock_active_value(None));
    }

    #[test]
    fn run_conditions_cover_every_phase_and_preview_combination() {
        let cases = [
            (
                PracticePhase::Setup,
                PreviewState::Stopped,
                false,
                true,
                false,
                false,
            ),
            (
                PracticePhase::Setup,
                PreviewState::Playing,
                false,
                true,
                false,
                true,
            ),
            (
                PracticePhase::Running,
                PreviewState::Stopped,
                true,
                false,
                true,
                true,
            ),
            (
                PracticePhase::Running,
                PreviewState::Playing,
                true,
                false,
                true,
                true,
            ),
            (
                PracticePhase::Editing,
                PreviewState::Stopped,
                false,
                true,
                false,
                false,
            ),
            (
                PracticePhase::Editing,
                PreviewState::Playing,
                false,
                true,
                false,
                true,
            ),
        ];

        for (phase, preview, running, surface_open, input_active, clock_active) in cases {
            let flow = PracticeFlow {
                phase,
                preview,
                ..Default::default()
            };
            assert_eq!(
                practice_running_value(Some(&flow)),
                running,
                "practice_running for {phase:?} / {preview:?}"
            );
            assert_eq!(
                practice_surface_open_value(Some(&flow)),
                surface_open,
                "practice_surface_open for {phase:?} / {preview:?}"
            );
            assert_eq!(
                gameplay_input_active_value(Some(&flow)),
                input_active,
                "gameplay_input_active for {phase:?} / {preview:?}"
            );
            assert_eq!(
                chart_clock_active_value(Some(&flow)),
                clock_active,
                "chart_clock_active for {phase:?} / {preview:?}"
            );
        }
    }
}
