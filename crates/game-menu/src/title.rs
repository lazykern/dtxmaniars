//! Title screen — osu-style themed menu (ADR-0014).

use bevy::prelude::*;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};

#[derive(Component)]
pub struct TitleEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), spawn_title)
        .add_systems(OnExit(AppState::Title), despawn_stage::<TitleEntity>)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)));
}

fn spawn_title(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands
        .spawn((
            TitleEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(32.0),
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("DTXManiaRS"),
                Theme::title_font(),
                TextColor(t.accent),
            ));
            root.spawn((
                Text::new("DTX drummania — osu-smooth UX"),
                Theme::body_font(),
                TextColor(t.text_secondary),
            ));
            root.spawn((
                Text::new("ENTER · Song Select    ESC · Quit"),
                Theme::label_font(),
                TextColor(t.text_primary),
            ));
        });
}

fn title_input(keys: Res<ButtonInput<KeyCode>>, mut requests: MessageWriter<TransitionRequest>) {
    if keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    } else if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::End);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_entity_marker_exists() {
        let _ = TitleEntity;
    }
}
