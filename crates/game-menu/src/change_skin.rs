//! CStageChangeSkin — skin selection.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/09.ChangeSkin/*`
//! M6+. M3 stub: blank + ESC → Title.

use bevy::prelude::*;

use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct ChangeSkinEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::ChangeSkin),
        (spawn_change_skin, start_fade),
    )
    .add_systems(
        OnExit(AppState::ChangeSkin),
        despawn_stage::<ChangeSkinEntity>,
    )
    .add_systems(
        Update,
        change_skin_input.run_if(in_state(AppState::ChangeSkin)),
    );
}

fn spawn_change_skin(mut commands: Commands) {
    commands.spawn((
        ChangeSkinEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
        children![(
            Text::new("Change Skin — M6+"),
            TextFont {
                font_size: FontSize::Px(28.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.6)),
        )],
    ));
}

fn change_skin_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
}
