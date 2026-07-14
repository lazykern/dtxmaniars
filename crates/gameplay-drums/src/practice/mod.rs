//! Practice mode: seek/scrub, A/B loop, playback rate, attempt stats.
//!
//! `PracticeSession` present = practice; absent = normal play with zero
//! behavior change. Inserted on Performance enter when
//! `game_shell::PracticeIntent` is set, removed on returning to song
//! select (it must survive StageClear/Result so the save gate sees it).

pub mod ab_loop;
pub mod actions;
pub mod diagnosis;
pub mod draft;
pub mod flow;
pub mod hud;
pub mod metronome;
pub mod preview;
pub mod ramp;
pub mod rate;
pub mod session;
pub mod stats;
pub mod toast;
pub mod wait;

use bevy::prelude::*;
use game_shell::{AppState, PracticeIntent, PracticeRequest};

pub use draft::{
    PracticeDraft, PracticeDraftSource, PracticeTrainerDraft, PracticeTrainerMode, ValidatedDraft,
};
pub use flow::{
    chart_clock_active, gameplay_input_active, practice_running, practice_surface_open,
    PracticeEditSnapshot, PracticeFlow, PracticePhase, PreviewState,
};
pub use preview::{CancelPracticeSettings, OpenPracticeSettings, PreviewAction, PreviewController};
pub use session::PracticeSession;

use crate::gauge::StageGauge;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

#[derive(Message, Debug, Clone, PartialEq)]
pub enum PresetCommand {
    RecordLastUsed { draft: PracticeDraft },
}

/// The practice action chain and its full gating. Split out of `plugin` so the
/// test can register the PRODUCTION wiring (notably the `editor_closed` gate)
/// without booting the rest of practice mode, which needs the whole game's
/// audio/asset/theme resources.
fn add_action_systems(app: &mut App) {
    app.init_resource::<actions::PracticeBindings>()
        .add_message::<actions::PracticeAction>()
        .add_systems(
            Update,
            (
                actions::emit_practice_actions,
                actions::apply_practice_actions,
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(resource_exists::<PracticeSession>)
                .run_if(gameplay_input_active)
                .run_if(crate::editor::editor_closed),
        );
}

pub(super) fn plugin(app: &mut App) {
    add_action_systems(app);
    app.init_resource::<toast::ToastQueue>()
        .add_message::<PresetCommand>();
    app.add_systems(
        OnEnter(AppState::Performance),
        enter_practice_setup.before(crate::orchestrator::DrumsEnterSet),
    )
    .add_systems(OnExit(AppState::Performance), remove_practice_surface)
    .add_systems(OnEnter(AppState::SongSelect), remove_practice_session)
    .add_systems(
        FixedUpdate,
        freeze_gauge_in_practice
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>)
            .run_if(gameplay_input_active),
    )
    .add_plugins((
        ab_loop::plugin,
        hud::plugin,
        metronome::plugin,
        preview::plugin,
        ramp::plugin,
        rate::plugin,
        stats::plugin,
        wait::plugin,
    ));
}

fn enter_practice_setup(
    intent: Res<PracticeIntent>,
    mut commands: Commands,
    mut wait_state: ResMut<wait::WaitState>,
    mut chord_hits: ResMut<wait::ChordHitTimes>,
    mut deferred: ResMut<wait::DeferredWaitJudgments>,
) {
    wait_state.phase = wait::WaitPhase::Flowing;
    wait_state.waited_chips.clear();
    wait_state.enabled_from_ms = None;
    chord_hits.0.clear();
    deferred.0.clear();
    if let Some(request) = intent.request() {
        let mut session = PracticeSession::default();
        let mut draft = PracticeDraft::default();
        let mut flow = PracticeFlow::default();
        begin_practice_setup(&request, &mut session, &mut draft, &mut flow);
        commands.insert_resource(session);
        commands.insert_resource(draft);
        commands.insert_resource(flow);
    } else {
        commands.remove_resource::<PracticeSession>();
        commands.remove_resource::<PracticeDraft>();
        commands.remove_resource::<PracticeFlow>();
    }
}

pub fn begin_practice_setup(
    request: &PracticeRequest,
    session: &mut PracticeSession,
    draft: &mut PracticeDraft,
    flow: &mut PracticeFlow,
) {
    *session = PracticeSession::default();
    session.current_attempt_eligible = false;
    *draft = PracticeDraft::from_request(request);
    *flow = PracticeFlow::from_request(request);
}

pub fn start_or_continue_practice(
    mut commands: Commands,
    timeline: Res<ChipTimeline>,
    mut draft: ResMut<PracticeDraft>,
    mut session: ResMut<PracticeSession>,
    mut flow: ResMut<PracticeFlow>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut preset_commands: MessageWriter<PresetCommand>,
    mut toasts: ResMut<toast::ToastQueue>,
    mut lane_hits: ResMut<Messages<crate::events::LaneHit>>,
    mut input_hits: ResMut<Messages<crate::events::InputHit>>,
) {
    if !matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing) {
        return;
    }
    let validated = match draft.validate(&timeline) {
        Ok(validated) => validated,
        Err(never) => match never {},
    };
    if let Some(warning) = &validated.warning {
        toasts.push(warning.clone());
    }
    let committed = validated.draft;
    committed.apply_to_session(&mut session);
    let attempt_start_ms = committed.loop_region.map_or(0, |region| region.start_ms);
    session.current_attempt = session::AttemptStats {
        start_ms: attempt_start_ms,
        ..Default::default()
    };
    session.current_attempt_eligible = true;
    crate::input::clear_pending_lane_inputs(&mut commands);
    lane_hits.clear();
    input_hits.clear();
    *draft = committed.clone();
    preset_commands.write(PresetCommand::RecordLastUsed { draft: committed });
    flow.preview = PreviewState::Stopped;
    flow.edit_snapshot = None;
    flow.phase = PracticePhase::Running;
    seeks.write(SeekToChartTime {
        target_ms: session::preroll_target(&timeline, session.transport.preroll, attempt_start_ms),
        snap: None,
        attempt_start_ms: Some(attempt_start_ms),
    });
}

pub fn cancel_initial_setup(
    flow: Res<PracticeFlow>,
    mut result_return: ResMut<game_shell::ResultReturnState>,
    mut requests: MessageWriter<game_shell::TransitionRequest>,
    mut toasts: ResMut<toast::ToastQueue>,
) {
    if flow.phase != PracticePhase::Setup {
        return;
    }
    let target = match flow.origin {
        game_shell::PracticeOrigin::SongSelect | game_shell::PracticeOrigin::NormalPause => {
            AppState::SongSelect
        }
        game_shell::PracticeOrigin::Results if result_return.available => {
            result_return.skip_processing_once = true;
            AppState::Result
        }
        game_shell::PracticeOrigin::Results => {
            result_return.skip_processing_once = false;
            toasts.push("Previous Results are unavailable — returning to Song Select");
            AppState::SongSelect
        }
    };
    game_shell::request_transition(&mut requests, target);
}

fn remove_practice_session(mut commands: Commands, mut intent: ResMut<PracticeIntent>) {
    commands.remove_resource::<PracticeSession>();
    commands.remove_resource::<PracticeDraft>();
    commands.remove_resource::<PracticeFlow>();
    *intent = PracticeIntent::None;
}

fn remove_practice_surface(mut commands: Commands) {
    commands.remove_resource::<PracticeDraft>();
    commands.remove_resource::<PracticeFlow>();
}

/// Gauge is meaningless in practice: pin it full so it can never fail
/// the stage and the HUD reads as neutral.
fn freeze_gauge_in_practice(mut gauge: ResMut<StageGauge>) {
    gauge.value = 1.0;
    gauge.failed = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::wait::{WaitSet, WaitState};

    #[test]
    fn entering_normal_play_clears_wait_halt() {
        let mut app = App::new();
        app.insert_resource(PracticeIntent::None);
        app.insert_resource(WaitState {
            phase: wait::WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![7],
            }),
            waited_chips: [7].into(),
            ..default()
        });
        app.init_resource::<wait::ChordHitTimes>();
        app.init_resource::<wait::DeferredWaitJudgments>();
        app.add_systems(Update, enter_practice_setup);
        app.update();

        let state = app.world().resource::<WaitState>();
        assert!(!state.halted());
        assert!(state.waited_chips.is_empty());
        assert!(!app.world().contains_resource::<PracticeSession>());
    }

    #[test]
    fn recommended_intent_seeds_the_setup_draft() {
        let intent = PracticeIntent::recommended(
            game_shell::PracticeOrigin::Results,
            game_shell::PracticeRecommendation::weak_section(1_000, 5_000, Some(3)),
        );
        let mut session = PracticeSession::default();
        let mut draft = PracticeDraft::default();
        let mut flow = PracticeFlow::default();
        begin_practice_setup(
            &intent.request().expect("recommendation requests practice"),
            &mut session,
            &mut draft,
            &mut flow,
        );

        assert_eq!(
            draft.loop_region,
            Some(session::LoopRegion {
                start_ms: 1_000,
                end_ms: 5_000,
            })
        );
        assert_eq!(draft.preroll, session::PrerollSetting::OneBar);
        assert_eq!(draft.user_tempo, 1.0);
        assert_eq!(flow.phase, PracticePhase::Setup);
        assert_eq!(flow.preview, PreviewState::Stopped);
        assert!(session.attempt_history.is_empty());
    }

    /// Registers the PRODUCTION chain (`add_action_systems`, called by
    /// `plugin`), not a re-stated gate: dropping `.run_if(editor_closed)` from
    /// it fails this test.
    #[test]
    fn practice_actions_are_dead_while_editor_is_open() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(AppState::Performance)
            .insert_state(game_shell::PauseState::Running)
            .init_resource::<ButtonInput<KeyCode>>()
            // What `apply_practice_actions` (the chain's second half) reads.
            .init_resource::<crate::timeline::ChipTimeline>()
            .init_resource::<crate::resources::GameplayClock>()
            .init_resource::<toast::ToastQueue>()
            .init_resource::<crate::pause::PracticePauseSurface>()
            .add_message::<crate::seek::SeekToChartTime>()
            .insert_resource(PracticeSession::default())
            .insert_resource(crate::editor::EditorOpen(true));
        add_action_systems(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<actions::PracticeAction>>()
                .is_empty(),
            "editor open must gate practice actions (Tab = OpenFullHud)"
        );

        // Editor closed: the same press emits again. `reset_all` (not `clear`)
        // — Tab must leave the pressed set or the re-press is no `just_pressed`.
        app.world_mut()
            .resource_mut::<crate::editor::EditorOpen>()
            .0 = false;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        assert!(!app
            .world()
            .resource::<Messages<actions::PracticeAction>>()
            .is_empty());
    }
}
