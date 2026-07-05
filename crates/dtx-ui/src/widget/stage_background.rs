//! Fullscreen stage: black bg + optional ambient album-art layer
//! (osu-style tint) under a dark overlay. Deliberately minimal —
//! layout carries the design (spec revision 2026-07-05).

use bevy::prelude::*;

use crate::theme::Theme;

/// Spawn the stage as children of `parent`: a solid black fill.
/// Ambient album-art layer removed (2026-07-05, user request).
pub fn spawn_stage_background(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme.stage_bg),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_spawn_creates_black_fill() {
        let mut app = bevy::app::App::new();
        let theme = Theme::default();
        let root = app.world_mut().spawn(Node::default()).id();

        {
            let mut commands = app.world_mut().commands();
            commands.entity(root).with_children(|p| {
                spawn_stage_background(p, &theme);
            });
        }
        app.world_mut().flush();

        let world = app.world_mut();
        let bg_count = world
            .query::<(&Node, &BackgroundColor)>()
            .iter(world)
            .count();
        assert!(bg_count >= 1);
    }
}
