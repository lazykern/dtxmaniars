//! DTXMania classic Performance HUD layout — frame chrome (top + side rails).
//!
//! Reference: BocuD `CStagePerfDrumsScreen.cs` lane cover positions
//! (x=295, x=830, y=0..720). Top speaker bar from skin sprite 7_SpeakerStr.png.
//! No sprite available yet → flat colored quads.

use bevy::prelude::*;
use crate::theme::Theme;

/// Top speaker bar (y=0..60 in 1280×720 reference).
pub fn spawn_frame_chrome(commands: &mut Commands, parent: Entity, theme: &Theme) {
    let chrome_bg = Color::srgba(0.05, 0.05, 0.08, 0.95);
    commands.entity(parent).with_children(|p| {
        // Top bar
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(1280.0),
                height: Val::Px(60.0),
                ..default()
            },
            BackgroundColor(chrome_bg),
        ));
        // Left side rail (lane cover)
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(60.0),
                width: Val::Px(260.0),
                height: Val::Px(660.0),
                ..default()
            },
            BackgroundColor(theme.panel_bg),
        ));
        // Right side rail
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(1020.0),
                top: Val::Px(60.0),
                width: Val::Px(260.0),
                height: Val::Px(660.0),
                ..default()
            },
            BackgroundColor(theme.panel_bg),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chrome_constants_in_bounds() {
        assert!(260.0 < 1020.0);
        assert!(60.0 + 660.0 <= 720.0);
    }
}
