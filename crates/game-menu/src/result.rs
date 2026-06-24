//! CStageResult — post-play results.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/*`
//! M5 will implement the real results screen. M3 stub: blank + ESC → Title.

use bevy::prelude::*;

use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct ResultEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Result), (spawn_result, start_fade))
        .add_systems(OnExit(AppState::Result), despawn_stage::<ResultEntity>)
        .add_systems(Update, result_input.run_if(in_state(AppState::Result)));
}

fn spawn_result(mut commands: Commands) {
    commands.spawn((
        ResultEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        children![(
            Text::new("Result — M5"),
            TextFont {
                font_size: FontSize::Px(28.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.6)),
        )],
    ));
}

fn result_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::SongSelect);
    }
}
