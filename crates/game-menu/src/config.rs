//! CStageConfig — settings screen (M4 minimum viable).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CStageConfig.cs` (531 lines)
//! DTXManiaNX has many sub-menus (Drums / Guitar / Bass / System / Skin /
//! Audio / Graphics / Gameplay / Menu / Velocity). Each is a CActConfigList.
//!
//! M4 ports the LOGIC: item-list navigation, ENTER to drill in, ESC to back out.
//! Real sub-menus + persistence (Config.ini) land in M6+.

use bevy::prelude::*;
use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

#[derive(Component)]
pub struct ConfigEntity;

#[derive(Resource, Debug, Default, Clone, Copy)]
struct ConfigSelection(usize);

/// M4 stub: hardcoded list of top-level menu groups.
/// Real CStageConfig has 11+ groups (System, Skin, Gameplay, Drums, ...).
const MENU_GROUPS: &[(&str, &str)] = &[
    ("Drums", "M6+ — drum key bindings, auto-play, velocity"),
    ("System", "M6+ — fullscreen, audio buffer, log"),
    ("Skin", "M6+ — skin directory + reload"),
    ("Gameplay", "M6+ — timing windows, scroll speed"),
];

pub fn plugin(app: &mut App) {
    app.init_resource::<ConfigSelection>()
        .add_systems(OnEnter(AppState::Config), (spawn_config, start_fade))
        .add_systems(OnExit(AppState::Config), despawn_stage::<ConfigEntity>)
        .add_systems(
            Update,
            (config_navigation, render_config_selection).run_if(in_state(AppState::Config)),
        );
}

fn spawn_config(mut commands: Commands) {
    commands
        .spawn((
            ConfigEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(40.0)),
                row_gap: Val::Px(15.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Config"),
                TextFont {
                    font_size: FontSize::Px(36.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new("↑↓: Navigate  ENTER: Drill in (stub)  ESC: Back"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));

            for (i, (name, _desc)) in MENU_GROUPS.iter().enumerate() {
                parent
                    .spawn((
                        ConfigItemEntity(i),
                        Node {
                            width: Val::Px(400.0),
                            height: Val::Px(28.0),
                            margin: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(if i == 0 {
                            Color::srgb(0.3, 0.5, 0.8)
                        } else {
                            Color::srgb(0.15, 0.15, 0.2)
                        }),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(*name),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });
}

#[derive(Component)]
struct ConfigItemEntity(usize);

fn config_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<ConfigSelection>,
    mut next: ResMut<NextState<AppState>>,
) {
    let max = MENU_GROUPS.len().saturating_sub(1);
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = selection.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::Enter) {
        // M4 stub: no real sub-menus yet. Stay on Config screen.
        // M6+ will drill into the selected sub-menu.
        info!("Config: drill into '{}' (stub)", MENU_GROUPS[selection.0].0);
    } else if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
}

fn render_config_selection(
    selection: Res<ConfigSelection>,
    mut rows: Query<(&ConfigItemEntity, &mut BackgroundColor)>,
) {
    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.0 == selection.0 {
            Color::srgb(0.3, 0.5, 0.8)
        } else {
            Color::srgb(0.15, 0.15, 0.2)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_groups_not_empty() {
        assert!(
            !MENU_GROUPS.is_empty(),
            "Config must have at least one menu group"
        );
    }

    #[test]
    fn selection_index_starts_at_zero() {
        assert_eq!(ConfigSelection::default().0, 0);
    }

    #[test]
    fn arrow_up_saturates_at_zero() {
        let mut sel = ConfigSelection(0);
        sel.0 = sel.0.saturating_sub(1);
        assert_eq!(sel.0, 0);
    }

    #[test]
    fn arrow_down_within_bounds() {
        let mut sel = ConfigSelection(0);
        let max = MENU_GROUPS.len() - 1;
        sel.0 = (sel.0 + 1).min(max);
        assert_eq!(sel.0, 1.min(max));
    }
}
