//! Scroll speed readout at the bottom-right of the playfield.

use crate::theme::Theme;
use crate::widget::hud_ref::{scaled_font, HudRefRect};
use bevy::prelude::*;

#[derive(Component)]
pub struct PlayfieldSpeedText;

pub fn spawn_playfield_speed(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    ref_x: f32,
) {
    let y = 676.0;
    commands.entity(parent).with_children(|p| {
        p.spawn((
            PlayfieldSpeedText,
            HudRefRect::new(ref_x, y, 100.0, 18.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(y * scale),
                width: Val::Px(100.0 * scale),
                height: Val::Px(18.0 * scale),
                ..default()
            },
            Text::new("SPEED 1.0"),
            scaled_font(scale, 13.0),
            TextColor(theme.text_secondary),
        ));
    });
}
