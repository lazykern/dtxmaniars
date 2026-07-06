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

/// Root marker for the pause overlay UI.
#[derive(Component)]
struct PauseOverlay;

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
struct PauseSelection(usize);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PauseSelection>()
        // Always start a performance un-paused.
        .add_systems(OnEnter(AppState::Performance), force_running)
        .add_systems(OnExit(AppState::Performance), force_running)
        .add_systems(
            Update,
            toggle_pause.run_if(in_state(AppState::Performance)),
        )
        .add_systems(OnEnter(PauseState::Paused), (pause_chart_audio, spawn_overlay))
        .add_systems(OnExit(PauseState::Paused), (resume_chart_audio, despawn_overlay))
        .add_systems(
            Update,
            pause_menu_input.run_if(in_state(PauseState::Paused)),
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

fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
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
            GlobalZIndex(1000),
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
        });
}

fn despawn_overlay(mut commands: Commands, overlays: Query<Entity, With<PauseOverlay>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn pause_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<PauseSelection>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut rows: Query<(&PauseItem, &mut TextColor)>,
    practice: Option<Res<crate::practice::PracticeSession>>,
) {
    if practice.is_some() {
        return;
    }
    let count = PauseItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
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

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
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
