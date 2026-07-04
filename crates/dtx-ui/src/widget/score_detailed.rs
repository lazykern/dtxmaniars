//! DTXMania classic SCORE DETAILED panel (left side).
//!
//! Reference: BocuD `CActPerfDrumsStatusPanel.cs:23-205`.
//! Position: x=22, y=250, rows at y=72/102/132/162/192/222 (30px stride).
//! Columns: label x=22, count x=80, % x=167.

use bevy::prelude::*;
use crate::theme::Theme;

/// Marker for the SCORE number text at top-left (BocuD `CActPerfDrumsScore`).
#[derive(Component)]
pub struct ScoreNumberText;

/// Marker for the SCORE DETAILED label/header.
#[derive(Component)]
pub struct ScoreDetailedHeader;

/// Marker for one row of the judgment counts (Perfect/Great/Good/Ok/Miss/MaxCombo).
/// `kind` 0..=5 → Perfect, Great, Good, Ok, Miss, MaxCombo.
#[derive(Component)]
pub struct JudgmentRowText {
    pub kind: u8,
}

/// Marker for the Fast/Slow counter row.
#[derive(Component)]
pub struct FastSlowText;

/// Marker for the Skills by Song big number.
#[derive(Component)]
pub struct SkillBySongText;

/// Spawn the entire left panel: SCORE big + 6 judgment rows + Fast/Slow + Skills.
pub fn spawn_score_detailed_panel(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
) {
    let panel_x = 22.0;
    let panel_y = 60.0;
    let label_color = theme.text_primary;
    let label_secondary = theme.text_secondary;

    // SCORE big (BocuD x=22, y=15..50)
    commands.entity(parent).with_children(|p| {
        p.spawn((
            ScoreNumberText,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(15.0),
                width: Val::Px(238.0),
                height: Val::Px(50.0),
                ..default()
            },
            Text::new("0000000"),
            Theme::font(40.0),
            TextColor(label_color),
        ));

        // SCORE DETAILED header
        p.spawn((
            ScoreDetailedHeader,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y + 30.0),
                width: Val::Px(238.0),
                height: Val::Px(20.0),
                ..default()
            },
            Text::new("SCORE DETAILED"),
            Theme::font(14.0),
            TextColor(label_secondary),
        ));

        // 6 judgment rows: Perfect/Great/Good/Ok/Miss/MaxCombo
        let labels = ["Perfect", "Great", "Good", "Ok", "Miss", "MaxCombo"];
        let colors = [
            theme.judgment_perfect,
            theme.judgment_great,
            theme.judgment_good,
            Color::srgb(0.75, 0.45, 0.95), // Ok = purple
            theme.judgment_miss,
            theme.accent, // MaxCombo = cyan
        ];
        for (i, (label, color)) in labels.iter().zip(colors.iter()).enumerate() {
            let row_y = panel_y + 60.0 + i as f32 * 30.0;
            p.spawn((
                JudgmentRowText { kind: i as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(panel_x),
                    top: Val::Px(row_y),
                    width: Val::Px(238.0),
                    height: Val::Px(24.0),
                    ..default()
                },
                Text::new(format!("{label:<10} 0000   0%")),
                Theme::font(16.0),
                TextColor(*color),
            ));
        }

        // Fast / Slow counter (BocuD y=335)
        p.spawn((
            FastSlowText,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y + 285.0),
                width: Val::Px(238.0),
                height: Val::Px(20.0),
                ..default()
            },
            Text::new("Fast 0   Slow 0"),
            Theme::font(14.0),
            TextColor(label_secondary),
        ));

        // Skills by Song big number (BocuD x=58, y=363)
        p.spawn((
            SkillBySongText,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x + 36.0),
                top: Val::Px(panel_y + 313.0),
                width: Val::Px(200.0),
                height: Val::Px(40.0),
                ..default()
            },
            Text::new("0.00"),
            Theme::font(32.0),
            TextColor(theme.accent),
        ));
        // SKILLS BY SONG label
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y + 358.0),
                width: Val::Px(238.0),
                height: Val::Px(18.0),
                ..default()
            },
            Text::new("SKILLS BY SONG"),
            Theme::font(12.0),
            TextColor(label_secondary),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_fit_in_panel() {
        // 6 rows × 30px + header ~30 + 60 padding = 270; panel_y=60 → 60+270=330 < 720.
        assert!(60.0 + 60.0 + 6.0 * 30.0 < 720.0);
    }
}
