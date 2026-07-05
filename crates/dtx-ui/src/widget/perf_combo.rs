//! Right-side combo counter — compact osu-style corner readout.

use bevy::prelude::*;

use crate::theme::Theme;
use crate::widget::combo_display::ComboDisplay;
use crate::widget::hud_ref::{scaled_font, HudRefRect};

#[derive(Component)]
pub struct PerfComboNumber;

#[derive(Component)]
pub struct PerfComboCaption;

pub fn spawn_perf_combo(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    ref_x: f32,
    ref_y: f32,
) {
    let num_w = 360.0;
    let num_h = 80.0;

    commands.entity(parent).with_children(|p| {
        p.spawn((
            ComboDisplay::default(),
            PerfComboNumber,
            HudRefRect::new(ref_x, ref_y + 18.0, num_w, num_h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px((ref_y + 18.0) * scale),
                width: Val::Px(num_w * scale),
                height: Val::Px(num_h * scale),
                ..default()
            },
            Text::new("0"),
            scaled_font(scale, 64.0),
            TextColor(theme.text_primary),
        ));
        p.spawn((
            PerfComboCaption,
            HudRefRect::new(ref_x, ref_y, num_w, 18.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(ref_y * scale),
                width: Val::Px(num_w * scale),
                height: Val::Px(18.0 * scale),
                ..default()
            },
            Text::new("COMBO"),
            scaled_font(scale, 14.0),
            TextColor(theme.text_secondary),
        ));
    });
}
