//! Compact NOW PLAYING card (top-right panel).

use crate::theme::Theme;
use crate::widget::hud_ref::{scaled_font, HudRefRect};
use bevy::prelude::*;

#[derive(Component)]
pub struct NowPlayingArt;

#[derive(Component)]
pub struct NowPlayingTitle;

#[derive(Component)]
pub struct NowPlayingArtist;

#[derive(Component)]
pub struct NowPlayingMaker;

pub fn spawn_now_playing(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    ref_x: f32,
) {
    let panel_y = 8.0;
    let panel_w = 320.0;
    let panel_h = 72.0;
    let bg = Color::srgba(0.06, 0.07, 0.10, 0.92);
    let border = Color::srgba(1.0, 1.0, 1.0, 0.25);

    commands.entity(parent).with_children(|p| {
        p.spawn((
            HudRefRect::new(ref_x, panel_y, panel_w, panel_h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(panel_y * scale),
                width: Val::Px(panel_w * scale),
                height: Val::Px(panel_h * scale),
                ..default()
            },
            BackgroundColor(bg),
            Outline {
                width: Val::Px(1.0),
                color: border,
                ..default()
            },
        ));
        p.spawn((
            NowPlayingArt,
            HudRefRect::new(ref_x + 6.0, panel_y + 6.0, 60.0, 60.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((ref_x + 6.0) * scale),
                top: Val::Px((panel_y + 6.0) * scale),
                width: Val::Px(60.0 * scale),
                height: Val::Px(60.0 * scale),
                ..default()
            },
            // Image starts empty (neutral tile below shows through); the HUD
            // fills it from #PREIMAGE when a chart loads.
            ImageNode {
                color: Color::WHITE.with_alpha(0.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.15, 0.2)),
        ));
        p.spawn((
            NowPlayingTitle,
            HudRefRect::new(ref_x + 72.0, panel_y + 8.0, panel_w - 80.0, 30.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((ref_x + 72.0) * scale),
                top: Val::Px((panel_y + 8.0) * scale),
                width: Val::Px((panel_w - 80.0) * scale),
                height: Val::Px(30.0 * scale),
                ..default()
            },
            Text::new("— no chart —"),
            scaled_font(scale, 14.0),
            TextColor(theme.text_primary),
        ));
        p.spawn((
            NowPlayingArtist,
            HudRefRect::new(ref_x + 72.0, panel_y + 38.0, panel_w - 80.0, 24.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((ref_x + 72.0) * scale),
                top: Val::Px((panel_y + 38.0) * scale),
                width: Val::Px((panel_w - 80.0) * scale),
                height: Val::Px(24.0 * scale),
                ..default()
            },
            Text::new(""),
            scaled_font(scale, 12.0),
            TextColor(theme.text_secondary),
        ));
        p.spawn((
            NowPlayingMaker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-9999.0),
                top: Val::Px(0.0),
                width: Val::Px(1.0),
                height: Val::Px(1.0),
                ..default()
            },
            Text::new(""),
            scaled_font(scale, 1.0),
            TextColor(theme.text_secondary),
            Visibility::Hidden,
        ));
    });
}
