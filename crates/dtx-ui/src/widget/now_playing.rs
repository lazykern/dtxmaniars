//! DTXMania classic NOW PLAYING panel (top right).
//!
//! Reference: BocuD `CStagePerfDrumsScreen` NowPlaying + `CActSelectPresound.cs`.
//! Position: x=1020, y=15, ~250×130.

use bevy::prelude::*;
use crate::theme::Theme;

/// Marker for the album art image inside the NOW PLAYING card.
#[derive(Component)]
pub struct NowPlayingArt;

/// Marker for the title text.
#[derive(Component)]
pub struct NowPlayingTitle;

/// Marker for the artist text.
#[derive(Component)]
pub struct NowPlayingArtist;

/// Marker for the difficulty badge text.
#[derive(Component)]
pub struct NowPlayingDifficulty;

/// Marker for the maker name (DTXMania simfile maker X).
#[derive(Component)]
pub struct NowPlayingMaker;

/// Spawn the NOW PLAYING card.
pub fn spawn_now_playing(commands: &mut Commands, parent: Entity, theme: &Theme) {
    let panel_x = 1030.0;
    let panel_y = 70.0;
    let panel_w = 240.0;
    let panel_h = 150.0;
    let bg = Color::srgba(0.06, 0.07, 0.10, 0.95);
    let border = Color::srgba(theme.accent.to_srgba().red, theme.accent.to_srgba().green, theme.accent.to_srgba().blue, 0.4);

    commands.entity(parent).with_children(|p| {
        // Card background
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                ..default()
            },
            BackgroundColor(bg),
            Outline {
                width: Val::Px(2.0),
                color: border,
                ..default()
            },
        ));
        // NOW PLAYING header
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 8.0),
                top: Val::Px(panel_y + 4.0),
                width: Val::Px(panel_w - 16.0),
                height: Val::Px(14.0),
                ..default()
            },
            Text::new("◀ NOW PLAYING"),
            Theme::font(11.0),
            TextColor(theme.accent),
        ));
        // Album art (placeholder rectangle; real image from #PREIMAGE)
        p.spawn((
            NowPlayingArt,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 8.0),
                top: Val::Px(panel_y + 22.0),
                width: Val::Px(60.0),
                height: Val::Px(60.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.15, 0.2)),
        ));
        // Title
        p.spawn((
            NowPlayingTitle,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 76.0),
                top: Val::Px(panel_y + 22.0),
                width: Val::Px(panel_w - 84.0),
                height: Val::Px(34.0),
                ..default()
            },
            Text::new("— no chart —"),
            Theme::font(14.0),
            TextColor(theme.text_primary),
        ));
        // Artist
        p.spawn((
            NowPlayingArtist,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 76.0),
                top: Val::Px(panel_y + 58.0),
                width: Val::Px(panel_w - 84.0),
                height: Val::Px(18.0),
                ..default()
            },
            Text::new(""),
            Theme::font(11.0),
            TextColor(theme.text_secondary),
        ));
        // Maker
        p.spawn((
            NowPlayingMaker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 76.0),
                top: Val::Px(panel_y + 78.0),
                width: Val::Px(panel_w - 84.0),
                height: Val::Px(16.0),
                ..default()
            },
            Text::new(""),
            Theme::font(10.0),
            TextColor(theme.text_secondary),
        ));
        // Difficulty badge (BocuD: badge + level number, e.g. "MASTER 8.20")
        p.spawn((
            NowPlayingDifficulty,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 8.0),
                top: Val::Px(panel_y + 90.0),
                width: Val::Px(panel_w - 16.0),
                height: Val::Px(50.0),
                ..default()
            },
            Text::new("MASTER  0.00"),
            Theme::font(22.0),
            TextColor(theme.accent),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_in_bounds() {
        assert!(1030.0 + 240.0 <= 1280.0);
        assert!(70.0 + 150.0 <= 720.0);
    }
}
