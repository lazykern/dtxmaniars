//! Fullscreen stage: black bg + optional ambient album-art layer
//! (osu-style tint) under a dark overlay. Deliberately minimal —
//! layout carries the design (spec revision 2026-07-05).

use bevy::prelude::*;

use crate::theme::Theme;

/// Fullscreen album-art tint under the dark overlay. `max_alpha`
/// caps opacity; entity also carries `crate::widget::album_art::AlbumArt`
/// so selection swaps crossfade it.
#[derive(Component, Debug, Clone, Copy)]
pub struct AmbientArt {
    pub max_alpha: f32,
}

/// Spawn the stage as children of `parent`. Layer order (back to
/// front): black fill, ambient art, dark overlay.
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
    parent.spawn((
        AmbientArt { max_alpha: 0.30 },
        crate::widget::album_art::AlbumArt::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            color: Color::WHITE.with_alpha(0.0),
            ..default()
        },
    ));
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme.stage_bg.with_alpha(0.55)),
    ));
}

/// Copy `AlbumArt.opacity` (crossfade tween) into the ambient image
/// alpha, capped at `max_alpha`. A hidden art (`Handle::default`)
/// stays fully transparent = black stage.
pub fn ambient_art_apply_system(
    mut q: Query<(&AmbientArt, &crate::widget::album_art::AlbumArt, &mut ImageNode)>,
) {
    for (ambient, art, mut image) in &mut q {
        let target = if image.image == Handle::default() {
            0.0
        } else {
            art.opacity * ambient.max_alpha
        };
        if (image.color.alpha() - target).abs() > 0.001 {
            image.color = image.color.with_alpha(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_spawn_creates_ambient_layer() {
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
        let ambient = world.query::<&AmbientArt>().iter(world).count();
        assert_eq!(ambient, 1);
    }
}
