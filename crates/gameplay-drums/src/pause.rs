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

/// Root marker for the normal pause overlay (practice suppresses it; the
/// practice full HUD owns PauseState::Paused — see practice/hud/full_hud.rs).
#[derive(Component)]
pub struct PauseOverlay;

/// One selectable menu row.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum PauseItem {
    Resume,
    Retry,
    Quit,
}

impl PauseItem {
    const ORDER: [PauseItem; 3] = [PauseItem::Resume, PauseItem::Retry, PauseItem::Quit];

    fn label(self) -> &'static str {
        match self {
            PauseItem::Resume => "Resume",
            PauseItem::Retry => "Retry",
            PauseItem::Quit => "Quit to Song Select",
        }
    }
}

/// Currently highlighted pause-menu row.
#[derive(Resource, Default)]
pub struct PauseSelection(pub usize);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PauseSelection>()
        // Always start a performance un-paused.
        .add_systems(OnEnter(AppState::Performance), force_running)
        .add_systems(OnExit(AppState::Performance), force_running)
        .add_systems(
            Update,
            toggle_pause
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
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match state.get() {
            PauseState::Running => PauseState::Paused,
            PauseState::Paused => PauseState::Running,
        });
    }
}

fn pause_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::pause_audio_instance(&mut instances, handle);
    }
    dtx_audio::pause_polyphony(&mut instances, &polyphony);
    active.pause_all(&mut instances);
}

fn resume_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::resume_audio_instance(&mut instances, handle);
    }
    dtx_audio::resume_polyphony(&mut instances, &polyphony);
    active.resume_all(&mut instances);
}

pub fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    if practice.is_some() {
        return; // practice pause panel owns the overlay
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
            for item in PauseItem::ORDER {
                root.spawn((
                    item,
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
    mut rows: Query<(&PauseItem, &mut TextColor)>,
    practice: Option<Res<crate::practice::PracticeSession>>,
) {
    use game_shell::NavVerb;
    if practice.is_some() {
        actions.clear();
        return;
    }
    let count = PauseItem::ORDER.len();
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
    let selected = PauseItem::ORDER[selection.0];
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
            PauseItem::Resume => next_pause.set(PauseState::Running),
            PauseItem::Retry => {
                // Reload the chart from the top via the loading screen.
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongLoading);
            }
            PauseItem::Quit => {
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongSelect);
            }
        }
    }
}
