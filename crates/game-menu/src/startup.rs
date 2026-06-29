//! CStageStartup — boot splash with theme (ADR-0014).

use bevy::prelude::*;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};

#[derive(Component)]
pub struct StartupEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Startup), spawn_startup)
        .add_systems(OnExit(AppState::Startup), despawn_stage::<StartupEntity>)
        .add_systems(Update, advance_to_title.run_if(in_state(AppState::Startup)));
}

fn spawn_startup(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands.spawn((
        StartupEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(24.0),
            ..default()
        },
        BackgroundColor(t.bg_top),
        children![
            (
                Text::new("DTXManiaRS"),
                Theme::title_font(),
                TextColor(t.text_primary),
            ),
            (
                Text::new("Loading..."),
                Theme::body_font(),
                TextColor(t.text_secondary),
            ),
        ],
    ));
}

fn advance_to_title(
    mut requests: MessageWriter<TransitionRequest>,
    timer: Res<Time>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    if timer.elapsed_secs() > 0.5 {
        request_transition(&mut requests, AppState::Title);
        *done = true;
    }
}
