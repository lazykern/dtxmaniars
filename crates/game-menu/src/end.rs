//! CStageEnd — exit countdown (ADR-0014 minimal).

use bevy::prelude::*;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct EndEntity;

#[derive(Resource, Default)]
struct EndCountdown {
    remaining_ms: f32,
    started: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<EndCountdown>()
        .add_systems(OnEnter(AppState::End), spawn_end)
        .add_systems(OnExit(AppState::End), despawn_stage::<EndEntity>)
        .add_systems(Update, tick_end.run_if(in_state(AppState::End)));
}

fn spawn_end(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    mut countdown: ResMut<EndCountdown>,
) {
    countdown.remaining_ms = 1000.0;
    countdown.started = true;
    let t = theme.0;
    commands.spawn((
        EndEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(t.bg_bottom),
        children![(
            Text::new("Thanks for playing"),
            Theme::title_font(),
            TextColor(t.text_primary),
        )],
    ));
}

fn tick_end(time: Res<Time>, mut countdown: ResMut<EndCountdown>) {
    if !countdown.started {
        return;
    }
    countdown.remaining_ms -= time.delta_secs() * 1000.0;
    if countdown.remaining_ms <= 0.0 {
        std::process::exit(0);
    }
}
