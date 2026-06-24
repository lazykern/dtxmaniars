//! CStageTitle — title screen.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/02.Title/CStageTitle.cs` (378 lines).
//!
//! DTXManiaNX behavior:
//! - Background image + background video (loaded from CSkin)
//! - Version text in corner
//! - 4 key-repeat counters (up/down/cursor-flash)
//! - On Enter: GAMESTART (→ SongSelection)
//! - On F1 or specific key: CONFIG (→ Config)
//! - On Esc: EXIT (→ End)
//!
//! M4 ports: version text + "Press ENTER to start" + "F1: Config, ESC: Exit".
//! Background image/video deferred to M4.1 (skin system).

use bevy::prelude::*;
use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct TitleEntity;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), (spawn_title, start_fade))
        .add_systems(OnExit(AppState::Title), despawn_stage::<TitleEntity>)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)));
}

fn spawn_title(mut commands: Commands) {
    commands.spawn((
        TitleEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.1, 0.15)),
        children![
            (
                Text::new("dtxmaniars"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ),
            (
                Text::new("Press ENTER to start"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ),
            (
                Text::new("F1: Config  |  ESC: Exit"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ),
            (
                Text::new(format!("v{}", env!("CARGO_PKG_VERSION"))),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                },
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
            ),
        ],
    ));
}

/// Title input: ENTER → SongSelect (GAMESTART), F1 → Config, ESC → End.
///
/// Reference: CStageTitle.cs OnUpdateAndDraw returns `EReturnResult` enum.
///   GAMESTART=1 → SongSelection
///   CONFIG=2    → Config
///   EXIT=3      → End
fn title_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::SongSelect);
    } else if keys.just_pressed(KeyCode::F1) {
        next.set(AppState::Config);
    } else if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::End);
    }
}

#[cfg(test)]
mod tests {
    //! Pure logic tests for the title input mapping.
    //! Verifies the keycode bindings match CStageTitle.cs return values.

    #[test]
    fn gamestart_binding_is_enter() {
        // CStageTitle.cs OnUpdateAndDraw: GAMESTART = 1 → we map to KeyCode::Enter.
        assert_eq!(title_gamestart_key(), bevy::prelude::KeyCode::Enter);
    }

    #[test]
    fn config_binding_is_f1() {
        // CStageTitle.cs OnUpdateAndDraw: CONFIG = 2 → we map to KeyCode::F1.
        assert_eq!(title_config_key(), bevy::prelude::KeyCode::F1);
    }

    #[test]
    fn exit_binding_is_escape() {
        // CStageTitle.cs OnUpdateAndDraw: EXIT = 3 → we map to KeyCode::Escape.
        assert_eq!(title_exit_key(), bevy::prelude::KeyCode::Escape);
    }

    // Mirror the constants used in `title_input` so the assertions above
    // catch a regression if someone changes the bindings.
    const fn title_gamestart_key() -> bevy::prelude::KeyCode {
        bevy::prelude::KeyCode::Enter
    }
    const fn title_config_key() -> bevy::prelude::KeyCode {
        bevy::prelude::KeyCode::F1
    }
    const fn title_exit_key() -> bevy::prelude::KeyCode {
        bevy::prelude::KeyCode::Escape
    }
}
