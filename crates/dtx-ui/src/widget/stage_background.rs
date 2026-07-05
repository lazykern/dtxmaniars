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
        AmbientArt { max_alpha: 0.14 },
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
        BackgroundColor(theme.stage_bg.with_alpha(0.72)),
    ));
}

/// Copy `AlbumArt.opacity` (crossfade tween) into the ambient image
/// alpha, capped at `max_alpha`. A hidden art stays fully transparent
/// = black stage. "Hidden" is either the transparent placeholder
/// (`TRANSPARENT_IMAGE_HANDLE`, what `ImageNode::default()` uses) or
/// the default white-fallback handle (`Handle::default()`, what
/// `update_album_art_image` assigns when a song has no art).
pub fn ambient_art_apply_system(
    mut q: Query<(&AmbientArt, &crate::widget::album_art::AlbumArt, &mut ImageNode)>,
) {
    for (ambient, art, mut image) in &mut q {
        let hidden = image.image.id() == bevy::image::TRANSPARENT_IMAGE_HANDLE.id()
            || image.image == Handle::default();
        let target = if hidden {
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

    #[test]
    fn ambient_alpha_is_opacity_times_cap() {
        let mut app = bevy::app::App::new();
        app.add_systems(Update, ambient_art_apply_system);

        let handle: Handle<bevy::image::Image> = Handle::Uuid(
            bevy::asset::uuid::Uuid::from_u128(0x1234),
            std::marker::PhantomData,
        );
        let entity = app
            .world_mut()
            .spawn((
                AmbientArt { max_alpha: 0.30 },
                crate::widget::album_art::AlbumArt {
                    opacity: 0.5,
                    ..Default::default()
                },
                ImageNode {
                    color: Color::WHITE.with_alpha(0.0),
                    image: handle,
                    ..default()
                },
            ))
            .id();
        app.update();

        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert!(
            (image.color.alpha() - 0.15).abs() < 0.001,
            "ambient alpha should be opacity (0.5) * max_alpha (0.30) = 0.15, got {}",
            image.color.alpha()
        );
    }
}
