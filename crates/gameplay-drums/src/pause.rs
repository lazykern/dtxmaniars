//! Pause overlay (resume / retry / quit).
//!
//! `PauseState` (in `game-shell`) is orthogonal to `AppState`. Pressing Escape
//! during `AppState::Performance` toggles it. While paused the gameplay clock
//! is frozen (see `lib.rs`), input is dropped (see `input.rs`), the BGM
//! instance is paused, chart drum/layer voices are paused, and an overlay menu
//!
//! UX is redesigned (ADR-0014); mechanics-neutral. Loosely mirrors
//! `dtxpt/src/overlays/pause.rs`.

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use dtx_ui::theme::Theme;
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use crate::resources::ActiveDrumSounds;

/// Root marker for the pause overlay. In practice this spawns for the Esc
/// surface; Tab opens the full rail instead (see PracticePauseSurface).
#[derive(Component)]
pub struct PauseOverlay;

/// One selectable pause-menu row. The set differs between normal play and
/// practice — see [`pause_items`].
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseItemKind {
    Resume,
    Retry,
    Quit,
    RestartLoop,
    ExitPractice,
}

impl PauseItemKind {
    fn label(self) -> &'static str {
        match self {
            PauseItemKind::Resume => "Resume",
            PauseItemKind::Retry => "Retry",
            PauseItemKind::Quit => "Quit to Song Select",
            PauseItemKind::RestartLoop => "Restart loop",
            PauseItemKind::ExitPractice => "Exit Practice",
        }
    }
}

const NORMAL_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::Retry,
    PauseItemKind::Quit,
];
const PRACTICE_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::RestartLoop,
    PauseItemKind::ExitPractice,
];

/// Rows for the pause overlay: practice gets Resume / Restart loop /
/// Exit Practice; normal play keeps Resume / Retry / Quit exactly as-is.
pub fn pause_items(practice: bool) -> &'static [PauseItemKind] {
    if practice {
        PRACTICE_ITEMS
    } else {
        NORMAL_ITEMS
    }
}

/// Currently highlighted pause-menu row.
#[derive(Resource, Default)]
pub struct PauseSelection(pub usize);

/// Which surface owns `PauseState::Paused` during practice. Esc opens the
/// standard pause overlay; Tab opens the full practice rail. Irrelevant
/// outside practice (the overlay always spawns); reset to `Overlay` on
/// every return to `Running` for hygiene.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PracticePauseSurface {
    #[default]
    Overlay,
    Rail,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PauseSelection>()
        .init_resource::<PracticePauseSurface>()
        .add_systems(OnEnter(PauseState::Running), reset_pause_surface)
        // Always start a performance un-paused.
        .add_systems(OnEnter(AppState::Performance), force_running)
        .add_systems(OnExit(AppState::Performance), force_running)
        .add_systems(
            Update,
            // Both write NextState<PauseState>, but they compute the same
            // transition from the same current state, so a same-frame Escape +
            // pad hit is idempotent — no ordering constraint needed.
            (toggle_pause, system_verb_pause, system_verb_restart)
                .run_if(in_state(AppState::Performance))
                .run_if(crate::editor::editor_closed),
        )
        .add_systems(
            OnEnter(PauseState::Paused),
            (pause_chart_audio, spawn_overlay),
        )
        .add_systems(
            OnExit(PauseState::Paused),
            (resume_chart_audio, despawn_overlay),
        )
        .add_systems(
            Update,
            (pause_kb_emit, pause_menu_input)
                .chain()
                .run_if(in_state(PauseState::Paused)),
        );
}

fn force_running(mut next: ResMut<NextState<PauseState>>) {
    next.set(PauseState::Running);
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut surface: ResMut<PracticePauseSurface>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        toggle(state.get(), &mut next, &mut surface);
    }
}

/// `SystemVerb::Pause` from a pad or a bound key — the distant-kit equivalent of
/// Escape. Toggles both ways, so firing it while paused resumes. Gated to
/// Performance with the editor closed (see `plugin`).
fn system_verb_pause(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut surface: ResMut<PracticePauseSurface>,
) {
    if hits
        .read()
        .any(|hit| hit.verb == dtx_input::SystemVerb::Pause)
    {
        toggle(state.get(), &mut next, &mut surface);
    }
}

/// `SystemVerb::Restart` — re-request `SongLoading`, exactly as the pause menu's
/// Retry row does, preserving `SelectedSong` and `PracticeIntent`. Fires during
/// Performance whether running or paused.
fn system_verb_restart(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if !hits
        .read()
        .any(|hit| hit.verb == dtx_input::SystemVerb::Restart)
    {
        return;
    }
    next_pause.set(PauseState::Running);
    request_transition(&mut requests, AppState::SongLoading);
}

/// Shared by Escape and `SystemVerb::Pause`. Claims the overlay surface before
/// pausing, or the practice rail would keep it.
fn toggle(
    state: &PauseState,
    next: &mut NextState<PauseState>,
    surface: &mut PracticePauseSurface,
) {
    match state {
        PauseState::Running => {
            *surface = PracticePauseSurface::Overlay;
            next.set(PauseState::Paused);
        }
        PauseState::Paused => next.set(PauseState::Running),
    }
}

fn reset_pause_surface(mut surface: ResMut<PracticePauseSurface>) {
    *surface = PracticePauseSurface::Overlay;
}

pub(crate) fn pause_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::pause_audio_instance(instances, handle);
    }
    dtx_audio::pause_polyphony(instances, polyphony);
    active.pause_all(instances);
}

pub(crate) fn resume_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::resume_audio_instance(instances, handle);
    }
    dtx_audio::resume_polyphony(instances, polyphony);
    active.resume_all(instances);
}

fn pause_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn resume_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
) {
    if !should_resume_chart_audio(wait_state.as_deref()) {
        return;
    }
    resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn should_resume_chart_audio(wait_state: Option<&crate::practice::wait::WaitState>) -> bool {
    wait_state.is_none_or(|state| !state.halted())
}

pub fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        return; // Tab-opened pause: the practice rail owns this surface
    }
    selection.0 = 0;
    let theme = Theme::default();
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(crate::ui_z::PAUSE),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("PAUSED"),
                Theme::title_font(),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            for item in pause_items(practice.is_some()) {
                root.spawn((
                    *item,
                    Text::new(item.label()),
                    Theme::hud_font(),
                    TextColor(theme.text_secondary),
                ));
            }
            if midi.is_some_and(|m| m.0) {
                dtx_ui::widget::nav_legend::spawn_nav_legend(
                    root,
                    &theme,
                    &[
                        ("HH", "up"),
                        ("CY", "down"),
                        ("BD", "select"),
                        ("SD", "resume"),
                    ],
                );
            }
        });
}

fn despawn_overlay(mut commands: Commands, overlays: Query<Entity, With<PauseOverlay>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

/// Keyboard → `NavAction` for the pause overlay. Esc keeps its own toggle path.
fn pause_kb_emit(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<game_shell::NavAction>) {
    use game_shell::{NavAction, NavSource, NavVerb};
    let verb = if keys.just_pressed(KeyCode::ArrowDown) {
        NavVerb::Down
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        NavVerb::Up
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        NavVerb::Confirm
    } else {
        return;
    };
    out.write(NavAction {
        verb,
        source: NavSource::Keyboard,
        coarse: false,
    });
}

#[allow(clippy::too_many_arguments)]
fn pause_menu_input(
    mut actions: MessageReader<game_shell::NavAction>,
    mut selection: ResMut<PauseSelection>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
    mut rows: Query<(&PauseItemKind, &mut TextColor)>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
) {
    use game_shell::NavVerb;
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        actions.clear(); // rail owns this pause; don't double-handle keys/pads
        return;
    }
    let items = pause_items(practice.is_some());
    let count = items.len();
    let mut confirm = false;
    let mut resume = false;
    for action in actions.read() {
        match action.verb {
            NavVerb::Down => selection.0 = (selection.0 + 1) % count,
            NavVerb::Up => selection.0 = (selection.0 + count - 1) % count,
            NavVerb::Confirm => confirm = true,
            // SD resumes: the pad equivalent of Esc.
            NavVerb::Back => resume = true,
            _ => {}
        }
    }

    let theme = Theme::default();
    let selected = items[selection.0 % count];
    for (item, mut color) in &mut rows {
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }

    if resume {
        next_pause.set(PauseState::Running);
        return;
    }
    if confirm {
        match selected {
            PauseItemKind::Resume => next_pause.set(PauseState::Running),
            PauseItemKind::Retry => {
                // Reload the chart from the top via the loading screen.
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongLoading);
            }
            PauseItemKind::Quit | PauseItemKind::ExitPractice => {
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongSelect);
            }
            PauseItemKind::RestartLoop => {
                // Reuse the quick-tier effect verbatim: apply_practice_actions
                // (gated Running) reads this message the frame after the resume
                // transition applies — messages live two update cycles.
                practice_actions.write(crate::practice::actions::PracticeAction::RestartLoop);
                next_pause.set(PauseState::Running);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::wait::{WaitPhase, WaitSet, WaitState};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn esc_opener_sets_overlay_surface() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Running));
        world.init_resource::<NextState<PauseState>>();
        // Stale value from a previous Tab-opened rail must be overwritten.
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn esc_while_paused_resumes_and_leaves_surface_alone() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Paused));
        world.init_resource::<NextState<PauseState>>();
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        // The OnEnter(Running) reset handles hygiene; the toggle itself only closes.
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Rail
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn surface_resets_to_overlay_on_running() {
        let mut world = World::new();
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(reset_pause_surface)
            .expect("reset runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay
        );
    }

    #[test]
    fn pause_items_normal_vs_practice() {
        assert_eq!(
            pause_items(false),
            &[
                PauseItemKind::Resume,
                PauseItemKind::Retry,
                PauseItemKind::Quit
            ]
        );
        assert_eq!(
            pause_items(true),
            &[
                PauseItemKind::Resume,
                PauseItemKind::RestartLoop,
                PauseItemKind::ExitPractice
            ]
        );
    }

    fn dispatch_world(selection: usize) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<game_shell::NavAction>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.init_resource::<Messages<crate::practice::actions::PracticeAction>>();
        world.insert_resource(PauseSelection(selection));
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<PracticePauseSurface>(); // Overlay
        world.insert_resource(crate::practice::PracticeSession::default());
        world.write_message(game_shell::NavAction {
            verb: game_shell::NavVerb::Confirm,
            source: game_shell::NavSource::Keyboard,
            coarse: false,
        });
        world
    }

    #[test]
    fn practice_confirm_exit_goes_to_song_select() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(2); // Exit Practice row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongSelect]);
    }

    #[test]
    fn practice_confirm_restart_loop_emits_action_and_resumes() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use crate::practice::actions::PracticeAction;
        let mut world = dispatch_world(1); // Restart loop row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let actions: Vec<PracticeAction> = world
            .resource::<Messages<PracticeAction>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(actions, vec![PracticeAction::RestartLoop]);
    }

    #[test]
    fn rail_surface_clears_actions_and_does_nothing() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(0);
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
        assert_eq!(
            world
                .resource::<Messages<TransitionRequest>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    fn verb_world(state: PauseState, verb: dtx_input::SystemVerb) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<crate::events::SystemVerbHit>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.insert_resource(State::new(state));
        world.init_resource::<NextState<PauseState>>();
        // Stale value from a previous Tab-opened rail must be overwritten.
        world.insert_resource(PracticePauseSurface::Rail);
        world.write_message(crate::events::SystemVerbHit { verb });
        world
    }

    #[test]
    fn pause_verb_opens_the_overlay_surface() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay,
            "the practice rail must not steal the surface"
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn pause_verb_while_paused_resumes() {
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_does_not_toggle_pause() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn restart_verb_requests_song_loading_while_running() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_also_fires_while_paused() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
    }

    #[test]
    fn pause_verb_does_not_restart() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        assert_eq!(
            world
                .resource::<Messages<TransitionRequest>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    #[test]
    fn leaving_pause_keeps_wait_halted_audio_paused() {
        let halted = WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![7],
            }),
            ..default()
        };

        assert!(!should_resume_chart_audio(Some(&halted)));
        assert!(should_resume_chart_audio(None));
    }
}
