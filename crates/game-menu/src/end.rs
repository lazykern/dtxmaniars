//! CStageEnd — exit screen.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/08.End/*`
//! M3 stub: "Goodbye" + ESC closes app via AppExit event.

use bevy::prelude::*;

use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct EndEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::End), (spawn_end, start_fade))
        .add_systems(OnExit(AppState::End), despawn_stage::<EndEntity>)
        .add_systems(Update, end_input.run_if(in_state(AppState::End)));
}

fn spawn_end(mut commands: Commands) {
    commands.spawn((
        EndEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.0, 0.0, 0.0)),
        children![
            (
                Text::new("Goodbye"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ),
            (
                Text::new("Press ESC to exit"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ),
        ],
    ));
}

fn end_input(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
