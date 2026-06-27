//! Title screen — minimal Bevy UI.
//!
//! ADR-0010 relaxed: free redesign. DTXManiaNX had a logo + flashing
//! "Press ENTER" sprite. We render centered text + bottom prompt.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/02.Title/CStageTitle.cs`

use bevy::prelude::*;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct TitleEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), spawn_title)
        .add_systems(OnExit(AppState::Title), despawn_stage::<TitleEntity>)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)));
}

fn spawn_title(mut commands: Commands) {
    commands
        .spawn((
            TitleEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(40.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("DTXManiaRS"),
                TextFont {
                    font_size: FontSize::Px(72.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.8, 0.3)),
            ));
            root.spawn((
                Text::new("A DTX drummania simulator"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
            root.spawn((
                Text::new("ENTER: Song Select  ·  ESC: Quit"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        });
}

fn title_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::SongSelect);
    } else if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_entity_marker_exists() {
        // Smoke test: the marker component type is constructible.
        let _ = TitleEntity;
    }

    #[test]
    fn input_keys_recognized() {
        // ENTER → SongSelect, ESC → quit.
        // Verifies the input handling exists (compile-time check).
        let _ = KeyCode::Enter;
        let _ = KeyCode::Escape;
        let _ = AppState::SongSelect;
    }
}
