//! CStageChangeSkin — minimal M13 placeholder.
//!
//! Full skin browsing is M14+. M13 only needs the state to boot and return
//! cleanly so the full stage graph is navigable.

use bevy::prelude::*;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};

#[derive(Component)]
pub struct ChangeSkinEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::ChangeSkin), spawn_change_skin)
        .add_systems(
            OnExit(AppState::ChangeSkin),
            despawn_stage::<ChangeSkinEntity>,
        )
        .add_systems(
            Update,
            change_skin_navigation.run_if(in_state(AppState::ChangeSkin)),
        );
}

fn spawn_change_skin(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands.spawn((
        ChangeSkinEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(18.0),
            ..default()
        },
        BackgroundColor(t.bg_bottom),
        children![
            (
                Text::new("Change Skin"),
                Theme::title_font(),
                TextColor(t.text_primary),
            ),
            (
                Text::new("Skin browser is scheduled for M14. Press Enter or Esc to return."),
                Theme::body_font(),
                TextColor(t.text_secondary),
            ),
        ],
    ));
}

fn change_skin_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_type_is_constructible() {
        let _ = ChangeSkinEntity;
    }
}
