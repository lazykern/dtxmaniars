//! DTXMania classic OPTIONS panel (Speed / Risky / Auto / Mirror).
//!
//! Reference: BocuD `CActPerfDrumsStatusPanel.cs:60-65` shows the nInfoType
//! concept; the in-performance options display is read from `ConfigIni`.
//!
//! Per user request this is a read-only display of the current settings.

use bevy::prelude::*;
use crate::theme::Theme;

/// Marker for each option row text.
#[derive(Component, Copy, Clone)]
pub struct OptionRowText {
    pub kind: u8, // 0=Speed, 1=Risky, 2=Auto, 3=Mirror
}

/// Spawn OPTIONS panel: 4 rows under SCORE DETAILED, before pad chips.
pub fn spawn_options_panel(commands: &mut Commands, parent: Entity, theme: &Theme) {
    let panel_x = 22.0;
    let panel_y = 460.0; // after Skills by Song

    commands.entity(parent).with_children(|p| {
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y - 18.0),
                width: Val::Px(238.0),
                height: Val::Px(16.0),
                ..default()
            },
            Text::new("OPTIONS"),
            Theme::font(12.0),
            TextColor(theme.text_secondary),
        ));
        let labels = ["Speed", "Risky", "Auto", "Mirror"];
        for (i, label) in labels.iter().enumerate() {
            p.spawn((
                OptionRowText { kind: i as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(panel_x),
                    top: Val::Px(panel_y + i as f32 * 18.0),
                    width: Val::Px(238.0),
                    height: Val::Px(16.0),
                    ..default()
                },
                Text::new(format!("{label:<8} —")),
                Theme::font(13.0),
                TextColor(theme.text_primary),
            ));
        }
    });
}
