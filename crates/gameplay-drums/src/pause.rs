//! Pause overlay (resume / retry / quit).
//!
//! `PauseState` (in `game-shell`) is orthogonal to `AppState`. Pressing Escape
//! during `AppState::Performance` toggles it. While paused the gameplay clock
//! is frozen (see `lib.rs`), input is dropped (see `input.rs`), the BGM
//! instance is paused, chart drum/layer voices are paused, and an overlay menu
//! handles resume, restart, settings, and exit actions.
//!
//! UX is redesigned (ADR-0014); mechanics-neutral. Loosely mirrors
//! `dtxpt/src/overlays/pause.rs`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use dtx_input::SystemVerb;
use dtx_ui::theme::Theme;
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use crate::resources::ActiveDrumSounds;

/// Root marker for the pause overlay. Practice Settings leaves this overlay
/// and enters the dedicated Editing phase.
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
    PracticeSettings,
    ExitPractice,
}

impl PauseItemKind {
    fn label(self) -> &'static str {
        match self {
            PauseItemKind::Resume => "Resume",
            PauseItemKind::Retry => "Retry",
            PauseItemKind::Quit => "Quit to Song Select",
            PauseItemKind::RestartLoop => "Restart loop",
            PauseItemKind::PracticeSettings => "Practice Settings",
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
    PauseItemKind::PracticeSettings,
    PauseItemKind::ExitPractice,
];

/// Rows for the pause overlay: practice gets Resume / Restart loop /
/// Practice Settings / Exit Practice; normal play keeps Resume / Retry / Quit.
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
/// standard pause overlay. `Rail` is retained only for explicit legacy callers;
/// Tab and `OpenSettings` enter Editing without claiming it. Irrelevant outside
/// practice; reset to `Overlay` on every return to `Running` for hygiene.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PracticePauseSurface {
    #[default]
    Overlay,
    Rail,
}

/// Minimum gap between two accepted hits of the SAME system verb. Pads
/// double-fire (flam/retrigger 20-40 ms apart), and an un-guarded verb would
/// toggle pause straight back off. Same reason — and same window — as
/// `menu_nav::DEBOUNCE`, which guards pad *navigation*.
const VERB_DEBOUNCE: Duration = Duration::from_millis(80);

/// Per-verb min-interval guard for the system-verb path.
#[derive(Resource, Debug)]
pub struct VerbGuard {
    min_gap: Duration,
    last: HashMap<SystemVerb, Instant>,
}

impl Default for VerbGuard {
    fn default() -> Self {
        Self {
            min_gap: VERB_DEBOUNCE,
            last: HashMap::new(),
        }
    }
}

impl VerbGuard {
    /// True if `verb` fired at `now` may act. Keyed per verb, so a Pause never
    /// swallows a Restart. Pure in `now` — the caller supplies the clock.
    pub fn accept(&mut self, verb: SystemVerb, now: Instant) -> bool {
        if let Some(last) = self.last.get(&verb) {
            if now.saturating_duration_since(*last) < self.min_gap {
                return false;
            }
        }
        self.last.insert(verb, now);
        true
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PauseSelection>()
        .init_resource::<VerbGuard>()
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

/// Did this frame's batch carry `verb`? Counts — never `any()`, which
/// short-circuits and leaves the rest of the batch unread to replay (and
/// re-toggle) on the next frame. A pad retrigger really does put two hits in
/// one frame, so the reader must be drained to the end.
fn drain_verb(hits: &mut MessageReader<crate::events::SystemVerbHit>, verb: SystemVerb) -> bool {
    hits.read().filter(|hit| hit.verb == verb).count() > 0
}

/// `SystemVerb::Pause` from a pad or a bound key — the distant-kit equivalent of
/// Escape. Toggles both ways, so firing it while paused resumes. Gated to
/// Performance with the editor closed (see `plugin`).
fn system_verb_pause(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut surface: ResMut<PracticePauseSurface>,
    mut guard: ResMut<VerbGuard>,
) {
    // Drain first: the guard decides whether to ACT, never whether to READ.
    if !drain_verb(&mut hits, SystemVerb::Pause) {
        return;
    }
    if !guard.accept(SystemVerb::Pause, Instant::now()) {
        return; // pad retrigger a few frames later — not a second press
    }
    toggle(state.get(), &mut next, &mut surface);
}

/// `SystemVerb::Restart` — re-request `SongLoading`, exactly as the pause menu's
/// Retry row does, preserving `SelectedSong` and `PracticeIntent`. Fires during
/// Performance whether running or paused.
fn system_verb_restart(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut guard: ResMut<VerbGuard>,
) {
    if !drain_verb(&mut hits, SystemVerb::Restart) {
        return;
    }
    if !guard.accept(SystemVerb::Restart, Instant::now()) {
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
    flow: Option<Res<crate::practice::PracticeFlow>>,
) {
    if !should_resume_chart_audio(wait_state.as_deref(), flow.as_deref()) {
        return;
    }
    resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn should_resume_chart_audio(
    wait_state: Option<&crate::practice::wait::WaitState>,
    flow: Option<&crate::practice::PracticeFlow>,
) -> bool {
    wait_state.is_none_or(|state| !state.halted())
        && flow.is_none_or(|flow| {
            flow.phase == crate::practice::PracticePhase::Running
                || flow.preview == crate::practice::PreviewState::Playing
        })
}

pub fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        return; // Explicit legacy-rail callers still own this paused frame.
    }
    selection.0 = 0;
    let theme = Theme::default();
    commands
        .spawn((
            PauseOverlay,
            dtx_ui::ModalDialog::new(vec![
                dtx_ui::DialogAction::Custom(0),
                dtx_ui::DialogAction::Custom(1),
                dtx_ui::DialogAction::Destructive,
            ]),
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
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Display),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            for (index, item) in pause_items(practice.is_some()).iter().enumerate() {
                let tone = if matches!(item, PauseItemKind::Quit | PauseItemKind::ExitPractice) {
                    dtx_ui::InteractionTone::Destructive
                } else {
                    dtx_ui::InteractionTone::Focus
                };
                root.spawn((
                    *item,
                    dtx_ui::ActionButton::new(dtx_ui::DialogAction::Custom(index as u16), tone),
                    Text::new(item.label()),
                    Theme::hud_font(),
                    dtx_ui::SemanticText(dtx_ui::TypographyRole::Hud),
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
pub(crate) fn pause_menu_input(
    mut actions: MessageReader<game_shell::NavAction>,
    mut selection: ResMut<PauseSelection>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
    mut open_settings: MessageWriter<crate::practice::OpenPracticeSettings>,
    mut rows: Query<(&PauseItemKind, &mut TextColor)>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
) {
    use game_shell::NavVerb;
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        actions.clear(); // Current Practice Settings uses Editing; legacy rail owns this input.
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
            PauseItemKind::PracticeSettings => {
                open_settings.write(crate::practice::OpenPracticeSettings);
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
                PauseItemKind::PracticeSettings,
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
        world.init_resource::<Messages<crate::practice::OpenPracticeSettings>>();
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
        let mut world = dispatch_world(3); // Exit Practice row
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
        use crate::practice::actions::PracticeAction;
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
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
    fn practice_confirm_settings_only_requests_editing() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(2);
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
        assert_eq!(
            world
                .resource::<Messages<crate::practice::OpenPracticeSettings>>()
                .iter_current_update_messages()
                .count(),
            1
        );
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
        world.init_resource::<VerbGuard>();
        // Stale value from a previous Tab-opened rail must be overwritten.
        world.insert_resource(PracticePauseSurface::Rail);
        world.write_message(crate::events::SystemVerbHit { verb });
        world
    }

    /// A guard that never debounces — isolates the drain fix from the min-interval one.
    fn open_guard() -> VerbGuard {
        VerbGuard {
            min_gap: Duration::ZERO,
            last: HashMap::new(),
        }
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

    /// An e-kit pad retrigger puts TWO `SystemVerbHit`s in one frame.
    /// `Iterator::any` short-circuits, so the second message stayed unread and
    /// replayed the next frame — pause on, pause off, overlay flashes for one
    /// frame. This app runs a real message lifecycle (`TimePlugin` +
    /// `StatesPlugin`), so a leftover survives into the next update.
    #[test]
    fn a_double_pause_hit_in_one_frame_pauses_once() {
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<PauseState>()
            .init_resource::<PracticePauseSurface>()
            // Debounce OFF: this test must fail on `any()` alone, so the drain
            // is what it proves — the min-interval guard is tested separately.
            .insert_resource(open_guard())
            .add_message::<crate::events::SystemVerbHit>()
            .add_systems(Update, system_verb_pause);

        for _ in 0..2 {
            app.world_mut().write_message(crate::events::SystemVerbHit {
                verb: dtx_input::SystemVerb::Pause,
            });
        }
        // Frame 1 reads the hits and sets NextState; frame 2's StateTransition
        // applies it (transitions run before Update).
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<PauseState>>().get(),
            PauseState::Paused,
            "the first hit pauses"
        );
        // A message lives ~2 frames: an unread leftover replays here and, with
        // `any()`, toggled Paused → Running.
        app.update();
        assert_eq!(
            *app.world().resource::<State<PauseState>>().get(),
            PauseState::Paused,
            "a same-frame retrigger must not toggle pause twice"
        );
    }

    /// A pad retrigger 20-40 ms later lands in a DIFFERENT frame, so draining
    /// the reader can't collapse it — only a min-interval guard can.
    #[test]
    fn verb_guard_debounces_a_pad_retrigger() {
        let mut g = VerbGuard::default();
        let t0 = Instant::now();
        assert!(g.accept(SystemVerb::Pause, t0), "first hit acts");
        assert!(
            !g.accept(SystemVerb::Pause, t0 + Duration::from_millis(20)),
            "a 20 ms retrigger must not toggle pause back off"
        );
        assert!(
            !g.accept(SystemVerb::Pause, t0 + Duration::from_millis(79)),
            "still inside the window"
        );
        // A deliberate second press, well past the window, must act.
        assert!(g.accept(SystemVerb::Pause, t0 + Duration::from_millis(400)));
    }

    /// Keyed per verb: pausing must not swallow a Restart that lands right after.
    #[test]
    fn verb_guard_is_per_verb() {
        let mut g = VerbGuard::default();
        let t0 = Instant::now();
        assert!(g.accept(SystemVerb::Pause, t0));
        assert!(g.accept(SystemVerb::Restart, t0 + Duration::from_millis(10)));
        assert!(!g.accept(SystemVerb::Pause, t0 + Duration::from_millis(20)));
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

        assert!(!should_resume_chart_audio(Some(&halted), None));
        assert!(should_resume_chart_audio(None, None));
    }

    #[test]
    fn stopped_practice_surface_owns_paused_chart_audio() {
        let mut flow = crate::practice::PracticeFlow::default();
        assert!(!should_resume_chart_audio(None, Some(&flow)));

        flow.phase = crate::practice::PracticePhase::Editing;
        assert!(
            !should_resume_chart_audio(None, Some(&flow)),
            "Editing owns frozen chart audio after the pause overlay exits"
        );

        flow.preview = crate::practice::PreviewState::Playing;
        assert!(should_resume_chart_audio(None, Some(&flow)));

        flow.phase = crate::practice::PracticePhase::Running;
        flow.preview = crate::practice::PreviewState::Stopped;
        assert!(should_resume_chart_audio(None, Some(&flow)));
    }
}
