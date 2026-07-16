//! Results screen input.

use bevy::prelude::*;
use dtx_ui::motion::EnterChoreo;
use game_shell::{
    request_transition, AppState, NavAction, PracticeIntent, PracticeOrigin, SystemVerb,
    TransitionRequest,
};
use gameplay_drums::resources::ActiveChart;

use crate::ui::{ResultDetailsOpen, RevealState, StatRow};
use crate::ResultDisplaySnapshot;

/// The verb the cursor sits on. Resets to Continue on every Result enter.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ResultVerb {
    #[default]
    Continue,
    Retry,
    Practice,
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultAction {
    Moved(ResultVerb),
    Activate(ResultVerb),
    ContinueNow,
    None,
}

/// HH/CY (Up/Down) and keyboard ←/→ (mapped to Up/Down by the driver) move
/// the cursor, clamped at the ends. BD/Enter activates, SD/Esc continues,
/// FT jumps to practice.
pub(crate) fn reduce_result_nav(cursor: ResultVerb, verb: SystemVerb) -> ResultAction {
    match verb {
        SystemVerb::NavigateUp | SystemVerb::Decrease => {
            let moved = cursor.prev();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        SystemVerb::NavigateDown | SystemVerb::Increase => {
            let moved = cursor.next();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        SystemVerb::Confirm => ResultAction::Activate(cursor),
        SystemVerb::Back => ResultAction::ContinueNow,
        _ => ResultAction::None,
    }
}

/// Results input driver. While the reveal is running, the first input of any
/// kind finishes it and is consumed; afterwards pads and keys drive the verb
/// row through `reduce_result_nav`.
pub(crate) fn result_nav(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<NavAction>,
    mut cursor: ResMut<ResultVerb>,
    mut reveal: ResMut<RevealState>,
    mut practice_intent: ResMut<PracticeIntent>,
    display: Res<ResultDisplaySnapshot>,
    mut details: ResMut<ResultDetailsOpen>,
    chart: Res<ActiveChart>,
    mut requests: MessageWriter<TransitionRequest>,
    mut fades: Query<(
        &StatRow,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
    mut sliding: Query<&mut EnterChoreo>,
) {
    let Some(display) = display.0.as_ref() else {
        return;
    };

    // Pads (mapper's screen-enter grace already filters the song's last
    // notes) + keyboard, folded onto the same verbs. ←/→ are the natural
    // axis for a horizontal row; pads reuse Up/Down.
    let mut verbs: Vec<SystemVerb> = actions.read().map(|a| a.verb).collect();
    if keys.just_pressed(KeyCode::ArrowLeft) {
        verbs.push(SystemVerb::NavigateUp);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        verbs.push(SystemVerb::NavigateDown);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        verbs.push(SystemVerb::Confirm);
    }
    if keys.just_pressed(KeyCode::Escape) {
        verbs.push(SystemVerb::Back);
    }
    let retry_key = keys.just_pressed(KeyCode::KeyR);
    let toggle_details = keys.just_pressed(KeyCode::Tab);
    if verbs.is_empty() && !retry_key && !toggle_details {
        return;
    }

    if !reveal.done {
        // Skip: snap every fade to its target, fast-forward every slide
        // (enter_choreo_system zeroes the transform and removes it), and
        // consume the input.
        reveal.done = true;
        for (stat, text, bg) in &mut fades {
            if let Some(mut c) = text {
                c.0 = c.0.with_alpha(stat.target_alpha);
            } else if let Some(mut b) = bg {
                // Same guard as animate_staggered_reveal: text entities carry
                // a default (transparent black) BackgroundColor that must not
                // be faded in.
                b.0 = b.0.with_alpha(stat.target_alpha);
            }
        }
        for mut choreo in &mut sliding {
            choreo.elapsed_ms = choreo.delay_ms + choreo.duration_ms;
        }
        return;
    }

    if toggle_details {
        details.0 = !details.0;
    }

    for verb in verbs {
        let action = reduce_result_nav(*cursor, verb);
        apply(
            action,
            &mut cursor,
            &mut practice_intent,
            display.analysis.recommendation,
            &chart,
            &mut requests,
        );
    }
    if retry_key {
        apply(
            ResultAction::Activate(ResultVerb::Retry),
            &mut cursor,
            &mut practice_intent,
            display.analysis.recommendation,
            &chart,
            &mut requests,
        );
    }
}

/// Applies one reduced action. Retry/Practice fall back to Continue when the
/// chart has no source path (nothing SongLoading could reload — defensive,
/// stands in for the spec's missing-SelectedSong guard without a game-menu
/// dependency edge).
fn apply(
    action: ResultAction,
    cursor: &mut ResultVerb,
    practice_intent: &mut PracticeIntent,
    recommendation: Option<game_shell::PracticeRecommendation>,
    chart: &ActiveChart,
    requests: &mut MessageWriter<TransitionRequest>,
) {
    match action {
        ResultAction::Moved(v) => *cursor = v,
        ResultAction::ContinueNow | ResultAction::Activate(ResultVerb::Continue) => {
            request_transition(requests, AppState::SongSelect);
        }
        ResultAction::Activate(ResultVerb::Retry) => {
            if chart.source_path.is_some() {
                // SelectedSong + PracticeIntent are untouched: SongLoading
                // relaunches the same chart; a practice run retries as practice.
                request_transition(requests, AppState::SongLoading);
            } else {
                request_transition(requests, AppState::SongSelect);
            }
        }
        ResultAction::Activate(ResultVerb::Practice) => {
            if chart.source_path.is_some() {
                *practice_intent = practice_intent_for_result(recommendation);
                request_transition(requests, AppState::SongLoading);
            } else {
                request_transition(requests, AppState::SongSelect);
            }
        }
        ResultAction::None => {}
    }
}

fn practice_intent_for_result(
    recommendation: Option<game_shell::PracticeRecommendation>,
) -> PracticeIntent {
    recommendation
        .map(|recommendation| PracticeIntent::recommended(PracticeOrigin::Results, recommendation))
        .unwrap_or_else(|| PracticeIntent::manual(PracticeOrigin::Results))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recommendation() -> game_shell::PracticeRecommendation {
        game_shell::PracticeRecommendation::weak_section(1_000, 5_000, Some(3))
    }

    #[test]
    fn result_practice_action_keeps_results_origin() {
        let intent = practice_intent_for_result(Some(recommendation()));
        assert_eq!(
            intent.request().expect("request").origin,
            game_shell::PracticeOrigin::Results
        );
        assert!(matches!(
            intent.request().expect("request").seed,
            game_shell::PracticeSeed::Recommended(_)
        ));
    }

    #[test]
    fn returning_from_setup_skips_result_processing_once() {
        let mut state = game_shell::ResultReturnState {
            available: true,
            skip_processing_once: true,
        };
        let mut display = ResultDisplaySnapshot(Some(crate::ResultDisplay::default()));
        assert!(!crate::should_process_result(&state, &display));
        assert_eq!(crate::finish_result_entry(&mut state, &mut display), None);
        assert!(crate::should_process_result(&state, &display));
    }

    #[test]
    fn reduce_result_nav_moves_and_clamps() {
        use ResultVerb::{Continue, Practice, Retry};
        // Clamped at both ends, no wrap.
        assert_eq!(
            reduce_result_nav(Continue, SystemVerb::NavigateUp),
            ResultAction::None
        );
        assert_eq!(
            reduce_result_nav(Practice, SystemVerb::NavigateDown),
            ResultAction::None
        );
        // Moves along Continue ↔ Retry ↔ Practice.
        assert_eq!(
            reduce_result_nav(Continue, SystemVerb::NavigateDown),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, SystemVerb::NavigateDown),
            ResultAction::Moved(Practice)
        );
        assert_eq!(
            reduce_result_nav(Practice, SystemVerb::NavigateUp),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, SystemVerb::NavigateUp),
            ResultAction::Moved(Continue)
        );
        // Dec/Inc alias the same axis (keyboard adjust verbs).
        assert_eq!(
            reduce_result_nav(Retry, SystemVerb::Decrease),
            ResultAction::Moved(Continue)
        );
        assert_eq!(
            reduce_result_nav(Retry, SystemVerb::Increase),
            ResultAction::Moved(Practice)
        );
    }

    #[test]
    fn reduce_result_nav_confirm_activates_cursor() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, SystemVerb::Confirm),
            ResultAction::Activate(ResultVerb::Retry)
        );
    }

    #[test]
    fn reduce_result_nav_back_continues() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, SystemVerb::Back),
            ResultAction::ContinueNow
        );
        // Practice is a visible verb-row choice, not a shared semantic verb:
        // no shortcut verb jumps there anymore.
        assert_eq!(
            reduce_result_nav(ResultVerb::Continue, SystemVerb::Preview),
            ResultAction::None
        );
    }

    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use dtx_ui::motion::EnterChoreo;
    use game_shell::{NavAction, NavSource, PracticeIntent};
    use gameplay_drums::resources::ActiveChart;

    use crate::ui::{RevealState, StatRow};

    fn driver_world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<NavAction>>();
        world.init_resource::<Messages<game_shell::TransitionRequest>>();
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ResultVerb::default());
        world.insert_resource(PracticeIntent::default());
        world.insert_resource(ResultDisplaySnapshot(Some(crate::ResultDisplay::default())));
        world.insert_resource(ResultDetailsOpen::default());
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: true,
        });
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: Some(std::path::PathBuf::from("song.dtx")),
        });
        world
    }

    fn pad(verb: SystemVerb) -> NavAction {
        NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        }
    }

    fn drain_requests(world: &mut World) -> Vec<AppState> {
        world
            .resource_mut::<Messages<game_shell::TransitionRequest>>()
            .drain()
            .map(|r| r.0)
            .collect()
    }

    #[test]
    fn result_nav_back_continues_to_song_select() {
        let mut world = driver_world();
        world.write_message(pad(SystemVerb::Back));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
    }

    #[test]
    fn result_nav_moves_cursor_then_confirm_retries() {
        let mut world = driver_world();
        world.write_message(pad(SystemVerb::NavigateDown));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(*world.resource::<ResultVerb>(), ResultVerb::Retry);
        assert!(drain_requests(&mut world).is_empty());

        world.resource_mut::<Messages<NavAction>>().clear();
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
        assert!(
            *world.resource::<PracticeIntent>() == PracticeIntent::None,
            "plain retry keeps intent"
        );
    }

    #[test]
    fn retry_after_cancelling_results_practice_is_normal_play() {
        let mut world = driver_world();
        let practice_intent = PracticeIntent::manual(PracticeOrigin::Results);
        let practice_request = practice_intent.request().expect("practice request");
        world.insert_resource(practice_intent);
        world.insert_resource(game_shell::ResultReturnState {
            available: true,
            skip_processing_once: false,
        });
        world.insert_resource(gameplay_drums::practice::PracticeFlow::setup(
            practice_request,
        ));
        world.insert_resource(gameplay_drums::practice::toast::ToastQueue::default());

        world
            .run_system_once(gameplay_drums::practice::cancel_initial_setup)
            .expect("cancel system runs");

        assert_eq!(drain_requests(&mut world), vec![AppState::Result]);
        assert_eq!(*world.resource::<PracticeIntent>(), PracticeIntent::None);
        assert!(
            world
                .resource::<game_shell::ResultReturnState>()
                .skip_processing_once
        );

        world.resource_scope(|world, mut state: Mut<game_shell::ResultReturnState>| {
            let mut display = world.resource_mut::<ResultDisplaySnapshot>();
            assert_eq!(crate::finish_result_entry(&mut state, &mut display), None);
        });
        assert!(world.resource::<ResultDisplaySnapshot>().0.is_some());

        world.insert_resource(ResultVerb::Retry);
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");

        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
        assert_eq!(*world.resource::<PracticeIntent>(), PracticeIntent::None);
    }

    #[test]
    fn result_nav_practice_activates_from_the_verb_row() {
        let mut world = driver_world();
        world.insert_resource(ResultVerb::Practice);
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
        assert_eq!(
            *world.resource::<PracticeIntent>(),
            PracticeIntent::manual(PracticeOrigin::Results)
        );
    }

    #[test]
    fn result_nav_r_key_retries() {
        let mut world = driver_world();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
    }

    #[test]
    fn result_nav_retry_without_source_falls_back_to_continue() {
        let mut world = driver_world();
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: None,
        });
        world.insert_resource(ResultVerb::Retry);
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
        assert_eq!(*world.resource::<PracticeIntent>(), PracticeIntent::None);
    }

    #[test]
    fn result_nav_first_input_skips_reveal_without_acting() {
        let mut world = driver_world();
        world.insert_resource(RevealState {
            elapsed_ms: 100.0,
            total_ms: 1_130.0,
            done: false,
        });
        let text = world
            .spawn((
                StatRow {
                    reveal_at_ms: 600.0,
                    target_alpha: 0.5,
                },
                TextColor(Color::WHITE.with_alpha(0.0)),
            ))
            .id();
        let slid = world
            .spawn(EnterChoreo::slide(Vec2::new(0.0, 24.0), 600.0, 350.0))
            .id();

        // First input: consumed, finishes the reveal, no verb action.
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert!(world.resource::<RevealState>().done);
        assert!(drain_requests(&mut world).is_empty(), "skip consumes input");
        let color = world.get::<TextColor>(text).expect("text kept");
        assert_eq!(color.0.alpha(), 0.5, "alpha snapped to target");
        let choreo = world.get::<EnterChoreo>(slid).expect("choreo kept");
        assert!(choreo.finished(), "choreo fast-forwarded");

        // Second input acts normally.
        world.resource_mut::<Messages<NavAction>>().clear();
        world.write_message(pad(SystemVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
    }
}
