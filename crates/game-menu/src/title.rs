//! Title screen — osu-style themed menu (ADR-0014).

use bevy::prelude::*;
use dtx_ui::motion::{BeatPulse, EnterChoreo};
use dtx_ui::widget::stage_background::spawn_stage_background;
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
                row_gap: Val::Px(48.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(48.0), Val::Px(18.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.text_primary),
                BoxShadow::new(
                    Color::srgba(0.0, 0.667, 1.0, 0.25),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(4.0),
                    Val::Px(30.0),
                ),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, -120.0), 0.0, 450.0),
            ))
            .with_children(|logo| {
                logo.spawn((
                    Text::new("DTXMANIARS"),
                    Theme::font(56.0),
                    TextColor(t.text_primary),
                ));
            });
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(32.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(t.select_yellow),
                BoxShadow::new(
                    t.select_yellow.with_alpha(0.4),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(2.0),
                    Val::Px(18.0),
                ),
                UiTransform::default(),
                BeatPulse::new(60.0, 0.06),
                EnterChoreo::slide(Vec2::new(0.0, 60.0), 150.0, 300.0),
            ))
            .with_children(|chip| {
                chip.spawn((
                    Text::new("PRESS ENTER"),
                    Theme::font(20.0),
                    TextColor(Color::BLACK),
                ));
            });
            root.spawn((Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(20.0)),
                ..default()
            },))
                .with_children(|bar| {
                    bar.spawn((
                        Text::new(format!("v{}", env!("CARGO_PKG_VERSION"))),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
                    bar.spawn((
                        Text::new("ESC QUIT"),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
                });
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
