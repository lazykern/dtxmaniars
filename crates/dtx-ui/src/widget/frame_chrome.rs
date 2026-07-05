//! Performance HUD frame — top speaker bar plus dark side pillars flanking the
//! lane strip.

use crate::theme::Theme;
use crate::theme::{REF_HEIGHT, REF_WIDTH};
use crate::widget::hud_ref::HudRefRect;
use bevy::prelude::*;

pub fn spawn_frame_chrome(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    strip_left_ref: f32,
    strip_right_ref: f32,
) {
    let chrome_bg = Color::srgba(0.05, 0.05, 0.08, 0.95);
    commands.entity(parent).with_children(|p| {
        p.spawn((
            HudRefRect::new(0.0, 0.0, REF_WIDTH, 60.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(60.0 * scale),
                ..default()
            },
            BackgroundColor(chrome_bg),
        ));
    });

    let pillar_w = 10.0;
    let pillar_color = Color::srgb(0.08, 0.08, 0.10);
    let edge = theme.stage_panel_border;
    for x in [strip_left_ref - pillar_w - 2.0, strip_right_ref + 2.0] {
        commands.entity(parent).with_children(|p| {
            p.spawn((
                HudRefRect::new(x, 0.0, pillar_w, REF_HEIGHT),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x * scale),
                    top: Val::Px(0.0),
                    width: Val::Px(pillar_w * scale),
                    height: Val::Px(REF_HEIGHT * scale),
                    border: UiRect::horizontal(Val::Px(1.0 * scale)),
                    ..default()
                },
                BackgroundColor(pillar_color),
                BorderColor::all(edge),
            ));
        });
    }
}
