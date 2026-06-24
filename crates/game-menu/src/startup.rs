//! CStageStartup — boot + splash. Stub for M3; auto-advances to Title.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/01.Startup/CStageStartup.cs`
//! Full DTXManiaNX behavior loads config + sound banks. M3 stub skips both.

use bevy::prelude::*;

// fade UI removed (ADR-0010 relaxed)
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct StartupEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Startup), spawn_startup)
        .add_systems(OnExit(AppState::Startup), despawn_stage::<StartupEntity>)
        .add_systems(Update, advance_to_title.run_if(in_state(AppState::Startup)));
}

fn spawn_startup(mut commands: Commands) {
    commands.spawn((
        StartupEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
        children![(
            Text::new("DTXManiaNX"),
            TextFont {
                font_size: FontSize::Px(48.0),
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
}

fn advance_to_title(mut next: ResMut<NextState<AppState>>, timer: Res<Time>) {
    // M3: advance after ~0.5s. Real CStageStartup waits for config + sound bank load.
    let _ = timer; // suppress unused — placeholder for real "ready" signal
    next.set(AppState::Title);
}
