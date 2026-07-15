use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use game_shell::{AppState, PauseState};

use super::ab_loop::active_region;
use super::{PracticeDraft, PracticeFlow, PracticePhase, PracticeSession, PreviewState};
use crate::resources::{ActiveDrumSounds, EffectivePlaybackRate, GameplayClock};
use crate::seek::{SeekAcknowledgement, SeekResult, SeekToChartTime};
use crate::timeline::{ChipTimeline, SnapDivisor};

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    Play,
    Pause,
    Seek(i64),
    PrevBar,
    NextBar,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenPracticeSettings;

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelPracticeSettings;

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PreviewController {
    pub restore_ms: Option<i64>,
    pub start_pending: bool,
}

#[doc(hidden)]
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PendingCancel {
    stage: CancelStage,
}

#[derive(Debug, Clone, Copy, Default)]
enum CancelStage {
    #[default]
    Idle,
    Restoring {
        revision: u64,
        request: SeekToChartTime,
        fallback_target_ms: i64,
        attempt_start_ms: i64,
    },
    FallingBack {
        revision: u64,
        request: SeekToChartTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelAdvance {
    Wait,
    Restored,
    Fallback(SeekToChartTime),
    FallbackApplied(i64),
    FallbackFailed,
}

impl PendingCancel {
    fn restoring(
        revision: u64,
        request: SeekToChartTime,
        fallback_target_ms: i64,
        attempt_start_ms: i64,
    ) -> Self {
        Self {
            stage: CancelStage::Restoring {
                revision,
                request,
                fallback_target_ms,
                attempt_start_ms,
            },
        }
    }

    fn advance(&self, acknowledgement: &SeekAcknowledgement) -> CancelAdvance {
        let (revision, request) = match self.stage {
            CancelStage::Idle => return CancelAdvance::Wait,
            CancelStage::Restoring {
                revision, request, ..
            }
            | CancelStage::FallingBack { revision, request } => (revision, request),
        };
        if acknowledgement.revision < revision {
            return CancelAdvance::Wait;
        }
        let matches_request =
            acknowledgement.revision == revision && acknowledgement.request == Some(request);
        match self.stage {
            CancelStage::Restoring { .. }
                if matches_request
                    && matches!(acknowledgement.result, Some(SeekResult::Applied { .. })) =>
            {
                CancelAdvance::Restored
            }
            CancelStage::Restoring {
                fallback_target_ms,
                attempt_start_ms,
                ..
            } => CancelAdvance::Fallback(SeekToChartTime {
                target_ms: fallback_target_ms,
                snap: None,
                attempt_start_ms: Some(attempt_start_ms),
            }),
            CancelStage::FallingBack { .. }
                if matches_request
                    && matches!(acknowledgement.result, Some(SeekResult::Applied { .. })) =>
            {
                CancelAdvance::FallbackApplied(
                    request.attempt_start_ms.unwrap_or(request.target_ms),
                )
            }
            CancelStage::FallingBack { .. } => CancelAdvance::FallbackFailed,
            CancelStage::Idle => CancelAdvance::Wait,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
struct InitialSetupRebuild(bool);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PreviewController>()
        .init_resource::<PendingCancel>()
        .init_resource::<InitialSetupRebuild>()
        .add_message::<PreviewAction>()
        .add_message::<OpenPracticeSettings>()
        .add_message::<CancelPracticeSettings>()
        .add_systems(
            OnEnter(AppState::Performance),
            (reset_preview_transients, pause_initial_setup_audio)
                .chain()
                .after(crate::orchestrator::DrumsEnterSet),
        )
        .add_systems(OnExit(AppState::Performance), reset_preview_transients)
        .add_systems(
            Update,
            (
                open_practice_settings
                    .after(super::actions::apply_practice_actions)
                    .after(crate::pause::pause_menu_input),
                cancel_practice_settings,
                apply_preview_actions,
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeFlow>),
        )
        .add_systems(
            FixedUpdate,
            (
                (
                    cancel_initial_rebuild_for_existing_seek,
                    rebuild_initial_stopped_setup,
                )
                    .chain()
                    .before(crate::seek::apply_seek_system),
                wrap_preview_loop
                    .before(crate::seek::apply_seek_system)
                    .run_if(in_state(PauseState::Running)),
                finish_preview_start.after(crate::seek::start_pending_bgm),
                finish_cancel_after_restore.after(super::stats::track_attempt_stats),
            )
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeFlow>),
        );
}

fn reset_preview_transients(
    mut controller: ResMut<PreviewController>,
    mut pending_cancel: ResMut<PendingCancel>,
    mut initial_rebuild: ResMut<InitialSetupRebuild>,
    flow: Option<Res<PracticeFlow>>,
) {
    *controller = PreviewController::default();
    *pending_cancel = PendingCancel::default();
    initial_rebuild.0 = flow.is_some_and(|flow| flow.phase == PracticePhase::Setup);
}

pub fn preview_tempo(draft: &PracticeDraft) -> f32 {
    if draft.trainer.mode == super::PracticeTrainerMode::Ramp {
        draft.trainer.ramp_config.start_tempo
    } else {
        draft.user_tempo
    }
}

fn draft_region(draft: &PracticeDraft, timeline: &ChipTimeline) -> super::session::LoopRegion {
    let mut preview_session = PracticeSession::default();
    preview_session.transport.loop_region = draft.loop_region;
    active_region(&preview_session, timeline)
}

fn normalize_preview_draft(
    draft: &mut PracticeDraft,
    timeline: &ChipTimeline,
    toasts: &mut super::toast::ToastQueue,
) {
    let validated = match draft.validate(timeline) {
        Ok(validated) => validated,
        Err(never) => match never {},
    };
    if let Some(warning) = validated.warning {
        toasts.push(warning);
    }
    *draft = validated.draft;
}

fn pause_initial_setup_audio(
    flow: Option<Res<PracticeFlow>>,
    bgm: Res<dtx_audio::BgmHandle>,
    polyphony: Res<dtx_audio::DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if flow.is_some_and(|flow| flow.phase == PracticePhase::Setup) {
        crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
    }
}

pub fn open_practice_settings(
    mut requests: MessageReader<OpenPracticeSettings>,
    mut flow: ResMut<PracticeFlow>,
    mut session: ResMut<PracticeSession>,
    mut draft: ResMut<PracticeDraft>,
    clock: Res<GameplayClock>,
    mut controller: ResMut<PreviewController>,
    mut next_pause: ResMut<NextState<PauseState>>,
    bgm: Res<dtx_audio::BgmHandle>,
    polyphony: Res<dtx_audio::DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if requests.read().next().is_none() || flow.phase != PracticePhase::Running {
        return;
    }
    let chart_ms = clock.current_ms;
    let (next_flow, snapshot) = flow.clone().open_settings(chart_ms, &mut session);
    crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
    *flow = next_flow;
    *draft = PracticeDraft::from(&snapshot.session);
    controller.restore_ms = Some(chart_ms);
    next_pause.set(PauseState::Running);
}

pub fn cancel_practice_settings(
    mut requests: MessageReader<CancelPracticeSettings>,
    mut flow: ResMut<PracticeFlow>,
    mut session: ResMut<PracticeSession>,
    mut draft: ResMut<PracticeDraft>,
    mut controller: ResMut<PreviewController>,
    mut pending_cancel: ResMut<PendingCancel>,
    acknowledgement: Res<SeekAcknowledgement>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut next_pause: ResMut<NextState<PauseState>>,
    bgm: Res<dtx_audio::BgmHandle>,
    polyphony: Res<dtx_audio::DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if requests.read().next().is_none() || flow.phase != PracticePhase::Editing {
        return;
    }
    crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
    let Some(snapshot) = flow.edit_snapshot.take() else {
        return;
    };
    *session = snapshot.session;
    session.invalidate_current_attempt();
    *draft = PracticeDraft::from(&*session);
    flow.preview = PreviewState::Stopped;
    let restore_ms = controller.restore_ms.take().unwrap_or(snapshot.chart_ms);
    let restore = SeekToChartTime {
        target_ms: restore_ms,
        snap: None,
        attempt_start_ms: None,
    };
    seeks.write(restore);
    let attempt_start_ms = session
        .transport
        .loop_region
        .map_or(0, |region| region.start_ms);
    *pending_cancel = PendingCancel::restoring(
        acknowledgement.revision.wrapping_add(1),
        restore,
        super::session::preroll_target(&timeline, session.transport.preroll, attempt_start_ms),
        attempt_start_ms,
    );
    next_pause.set(PauseState::Paused);
}

fn finish_cancel_after_restore(
    mut flow: ResMut<PracticeFlow>,
    mut pending_cancel: ResMut<PendingCancel>,
    acknowledgement: Res<SeekAcknowledgement>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut toasts: ResMut<super::toast::ToastQueue>,
    mut session: ResMut<PracticeSession>,
    mut combo: ResMut<crate::resources::Combo>,
) {
    match pending_cancel.advance(&acknowledgement) {
        CancelAdvance::Wait => {}
        CancelAdvance::Restored => {
            *pending_cancel = PendingCancel::default();
            flow.phase = PracticePhase::Running;
        }
        CancelAdvance::Fallback(request) => {
            toasts.push("Could not restore the frozen position — returning to pre-roll");
            seeks.write(request);
            pending_cancel.stage = CancelStage::FallingBack {
                revision: acknowledgement.revision.wrapping_add(1),
                request,
            };
        }
        CancelAdvance::FallbackApplied(attempt_start_ms) => {
            session.current_attempt = super::session::AttemptStats {
                start_ms: attempt_start_ms,
                ..Default::default()
            };
            session.current_attempt_lane_diag.clear();
            session.current_attempt_eligible = true;
            combo.current = 0;
            *pending_cancel = PendingCancel::default();
            flow.phase = PracticePhase::Running;
        }
        CancelAdvance::FallbackFailed => {
            toasts.push("Could not seek to pre-roll — practice remains paused");
            *pending_cancel = PendingCancel::default();
            flow.phase = PracticePhase::Running;
        }
    }
}

fn initial_setup_rebuild_target(
    flow: &PracticeFlow,
    draft: &PracticeDraft,
    clock_started: bool,
    timeline_end_ms: i64,
) -> Option<i64> {
    (flow.phase == PracticePhase::Setup && flow.preview == PreviewState::Stopped && clock_started)
        .then(|| {
            draft
                .loop_region
                .map_or(0, |region| region.start_ms)
                .clamp(0, timeline_end_ms.max(0))
        })
}

fn rebuild_initial_stopped_setup(
    mut pending: ResMut<InitialSetupRebuild>,
    flow: Res<PracticeFlow>,
    draft: Res<PracticeDraft>,
    clock: Res<GameplayClock>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    if !pending.0 {
        return;
    }
    let Some(target_ms) =
        initial_setup_rebuild_target(&flow, &draft, clock.is_started(), timeline.end_ms)
    else {
        if flow.phase != PracticePhase::Setup || flow.preview != PreviewState::Stopped {
            pending.0 = false;
        }
        return;
    };
    seeks.write(SeekToChartTime {
        target_ms,
        snap: None,
        attempt_start_ms: None,
    });
    pending.0 = false;
}

fn cancel_initial_rebuild_for_existing_seek(
    mut seeks: MessageReader<SeekToChartTime>,
    mut pending: ResMut<InitialSetupRebuild>,
) {
    if pending.0 && seeks.read().next().is_some() {
        pending.0 = false;
    }
}

fn finish_preview_start(mut controller: ResMut<PreviewController>) {
    controller.start_pending = false;
}

#[allow(clippy::too_many_arguments)]
fn apply_preview_actions(
    mut actions: MessageReader<PreviewAction>,
    mut flow: ResMut<PracticeFlow>,
    mut draft: ResMut<PracticeDraft>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut seeks: MessageWriter<SeekToChartTime>,
    audio: Res<bevy_kira_audio::Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    polyphony: Res<dtx_audio::DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut rate: ResMut<EffectivePlaybackRate>,
    mut toasts: ResMut<super::toast::ToastQueue>,
    mut controller: ResMut<PreviewController>,
) {
    if !matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing) {
        actions.clear();
        return;
    }
    for action in actions.read() {
        match action {
            PreviewAction::Play => {
                normalize_preview_draft(&mut draft, &timeline, &mut toasts);
                let region = draft_region(&draft, &timeline);
                let target_ms = if (region.start_ms..region.end_ms).contains(&clock.current_ms) {
                    clock.current_ms
                } else {
                    region.start_ms
                };
                seeks.write(SeekToChartTime {
                    target_ms,
                    snap: None,
                    attempt_start_ms: None,
                });
                crate::playback_rate::apply_playback_rate(
                    EffectivePlaybackRate::practice(f64::from(preview_tempo(&draft))),
                    &mut rate,
                    &audio,
                    &bgm,
                    &mut instances,
                );
                controller.start_pending = true;
                flow.preview = PreviewState::Playing;
            }
            PreviewAction::Pause => {
                crate::pause::pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
                flow.preview = PreviewState::Stopped;
            }
            PreviewAction::Seek(target_ms) => {
                seeks.write(SeekToChartTime {
                    target_ms: *target_ms,
                    snap: None,
                    attempt_start_ms: None,
                });
            }
            PreviewAction::PrevBar | PreviewAction::NextBar => {
                let direction = if matches!(action, PreviewAction::PrevBar) {
                    -1
                } else {
                    1
                };
                seeks.write(SeekToChartTime {
                    target_ms: timeline.snap_neighbor(
                        clock.current_ms,
                        SnapDivisor::Bar,
                        direction,
                    ),
                    snap: None,
                    attempt_start_ms: None,
                });
            }
        }
    }
}

fn wrap_preview_loop(
    flow: Res<PracticeFlow>,
    mut draft: ResMut<PracticeDraft>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut toasts: ResMut<super::toast::ToastQueue>,
) {
    if flow.preview != PreviewState::Playing
        || !matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing)
        || !clock.is_ready()
        || timeline.end_ms <= 0
    {
        return;
    }
    normalize_preview_draft(&mut draft, &timeline, &mut toasts);
    let region = draft_region(&draft, &timeline);
    if clock.current_ms >= region.end_ms {
        seeks.write(SeekToChartTime {
            target_ms: region.start_ms,
            snap: None,
            attempt_start_ms: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_uses_user_tempo_except_for_ramp_start() {
        let mut draft = PracticeDraft {
            user_tempo: 0.85,
            ..Default::default()
        };
        assert!((preview_tempo(&draft) - 0.85).abs() < f32::EPSILON);

        draft.trainer.mode = super::super::PracticeTrainerMode::Ramp;
        draft.trainer.ramp_config.start_tempo = 0.65;
        assert!((preview_tempo(&draft) - 0.65).abs() < f32::EPSILON);
    }

    #[test]
    fn cancel_restore_waits_for_applied_ack_and_rejects_to_preroll() {
        use crate::seek::{SeekAcknowledgement, SeekRejection, SeekResult};

        let restore = SeekToChartTime {
            target_ms: 5_000,
            snap: None,
            attempt_start_ms: None,
        };
        let pending = PendingCancel::restoring(3, restore, 2_000, 4_000);
        assert_eq!(
            pending.advance(&SeekAcknowledgement::default()),
            CancelAdvance::Wait
        );
        let rejected = SeekAcknowledgement {
            revision: 3,
            request: Some(restore),
            result: Some(SeekResult::Rejected(SeekRejection::ClockNotStarted)),
        };
        assert_eq!(
            pending.advance(&rejected),
            CancelAdvance::Fallback(SeekToChartTime {
                target_ms: 2_000,
                snap: None,
                attempt_start_ms: Some(4_000),
            })
        );
        let applied = SeekAcknowledgement {
            revision: 3,
            request: Some(restore),
            result: Some(SeekResult::Applied { resolved_ms: 5_000 }),
        };
        assert_eq!(pending.advance(&applied), CancelAdvance::Restored);
    }

    #[test]
    fn initial_stopped_setup_rebuilds_at_draft_start_once_clock_is_ready() {
        let flow = PracticeFlow::default();
        let draft = PracticeDraft {
            loop_region: Some(super::super::session::LoopRegion {
                start_ms: 3_000,
                end_ms: 6_000,
            }),
            ..Default::default()
        };
        assert_eq!(
            initial_setup_rebuild_target(&flow, &draft, false, 10_000),
            None
        );
        assert_eq!(
            initial_setup_rebuild_target(&flow, &draft, true, 10_000),
            Some(3_000)
        );
    }
}
