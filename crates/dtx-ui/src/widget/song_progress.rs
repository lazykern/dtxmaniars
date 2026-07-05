//! Horizontal song progress bar at the bottom of the playfield.

use crate::theme::Theme;
use crate::widget::hud_ref::HudRefRect;
use bevy::prelude::*;

#[derive(Component)]
pub struct SongProgressTrack;

#[derive(Component)]
pub struct SongProgressFill;

pub fn spawn_song_progress(
    commands: &mut Commands,
    parent: Entity,
    _theme: &Theme,
    scale: f32,
    ref_x: f32,
    ref_w: f32,
) {
    let y = 696.0;
    let h = 14.0;
    let fill_color = Color::srgb(0.0, 0.82, 0.95);

    commands.entity(parent).with_children(|p| {
        p.spawn((
            SongProgressTrack,
            HudRefRect::new(ref_x, y, ref_w, h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(y * scale),
                width: Val::Px(ref_w * scale),
                height: Val::Px(h * scale),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        ));
        p.spawn((
            SongProgressFill,
            HudRefRect::new(ref_x, y, 0.0, h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(y * scale),
                width: Val::Px(0.0),
                height: Val::Px(h * scale),
                ..default()
            },
            BackgroundColor(fill_color),
        ));
    });
}
