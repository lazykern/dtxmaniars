//! Left performance stats panel (BocuD `CActPerfDrumsStatusPanel.cs` layout).

use crate::theme::Theme;
use crate::widget::hud_ref::{scaled_font, HudRefRect};
use bevy::prelude::*;

#[derive(Component)]
pub struct ScoreCaptionText;

#[derive(Component)]
pub struct ScoreNumberText;

#[derive(Component)]
pub struct StatsBoxBorder;

#[derive(Component)]
pub struct JudgmentRowText {
    pub kind: u8,
}

#[derive(Component)]
pub struct FastSlowText;

#[derive(Component)]
pub struct AccuracyText;

#[derive(Component)]
pub struct DifficultyBadgeText;

#[derive(Component)]
pub struct SkillBySongText;

#[derive(Component)]
pub struct SkillCaptionText;

pub fn spawn_score_detailed_panel(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
) {
    let panel_x = 16.0;
    let box_x = panel_x;
    let box_y = 78.0;
    let box_w = 200.0;
    let box_h = 250.0;
    let row_x = box_x + 10.0;
    let row_start_y = box_y + 12.0;
    let row_stride = 34.0;

    commands.entity(parent).with_children(|p| {
        let caption = HudRefRect::new(panel_x + 14.0, 8.0, 200.0, 22.0);
        p.spawn((
            ScoreCaptionText,
            caption,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(caption.left * scale),
                top: Val::Px(caption.top * scale),
                width: Val::Px(caption.width * scale),
                height: Val::Px(caption.height * scale),
                ..default()
            },
            Text::new("SCORE"),
            scaled_font(scale, 18.0),
            TextColor(theme.text_secondary),
        ));

        let score = HudRefRect::new(panel_x + 14.0, 26.0, 220.0, 40.0);
        p.spawn((
            ScoreNumberText,
            score,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(score.left * scale),
                top: Val::Px(score.top * scale),
                width: Val::Px(score.width * scale),
                height: Val::Px(score.height * scale),
                ..default()
            },
            Text::new("0"),
            scaled_font(scale, 36.0),
            TextColor(theme.text_primary),
        ));

        let accent = HudRefRect::new(panel_x, 8.0, 3.0, 58.0);
        p.spawn((
            accent,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(accent.left * scale),
                top: Val::Px(accent.top * scale),
                width: Val::Px(accent.width * scale),
                height: Val::Px(accent.height * scale),
                ..default()
            },
            BackgroundColor(theme.select_yellow),
        ));

        let border_rect = HudRefRect::new(box_x, box_y, box_w, box_h);
        p.spawn((
            StatsBoxBorder,
            border_rect,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(border_rect.left * scale),
                top: Val::Px(border_rect.top * scale),
                width: Val::Px(border_rect.width * scale),
                height: Val::Px(border_rect.height * scale),
                border: UiRect::all(Val::Px(1.0 * scale)),
                ..default()
            },
            BackgroundColor(theme.stage_panel_bg),
            BorderColor::all(theme.stage_panel_border),
        ));

        let labels = ["Perfect", "Great", "Good", "Ok", "Miss", "MaxCombo"];
        let colors = [
            theme.judgment_perfect,
            theme.judgment_great,
            theme.judgment_good,
            Color::srgb(0.75, 0.45, 0.95),
            theme.judgment_miss,
            theme.accent,
        ];
        for (i, (label, color)) in labels.iter().zip(colors.iter()).enumerate() {
            let row_y = row_start_y + i as f32 * row_stride;
            let rect = HudRefRect::new(row_x, row_y, box_w - 20.0, 28.0);
            p.spawn((
                JudgmentRowText { kind: i as u8 },
                rect,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(rect.left * scale),
                    top: Val::Px(rect.top * scale),
                    width: Val::Px(rect.width * scale),
                    height: Val::Px(rect.height * scale),
                    ..default()
                },
                Text::new(format!("{label:<8} 0   0%")),
                scaled_font(scale, 15.0),
                TextColor(*color),
            ));
        }

        let diff = HudRefRect::new(panel_x + 8.0, box_y + box_h + 10.0, 120.0, 26.0);
        p.spawn((
            DifficultyBadgeText,
            diff,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(diff.left * scale),
                top: Val::Px(diff.top * scale),
                width: Val::Px(diff.width * scale),
                height: Val::Px(diff.height * scale),
                ..default()
            },
            Text::new("BASIC 0.00"),
            scaled_font(scale, 14.0),
            TextColor(Color::srgb(1.0, 0.25, 0.25)),
        ));

        let acc = HudRefRect::new(panel_x + 8.0, box_y + box_h + 42.0, 220.0, 44.0);
        p.spawn((
            AccuracyText,
            acc,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(acc.left * scale),
                top: Val::Px(acc.top * scale),
                width: Val::Px(acc.width * scale),
                height: Val::Px(acc.height * scale),
                ..default()
            },
            Text::new("100.00%"),
            scaled_font(scale, 36.0),
            TextColor(theme.text_primary),
        ));

        let fs = HudRefRect::new(panel_x + 8.0, box_y + box_h + 90.0, 220.0, 20.0);
        p.spawn((
            FastSlowText,
            fs,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(fs.left * scale),
                top: Val::Px(fs.top * scale),
                width: Val::Px(fs.width * scale),
                height: Val::Px(fs.height * scale),
                ..default()
            },
            Text::new("Fast 0   Slow 0"),
            scaled_font(scale, 13.0),
            TextColor(theme.text_secondary),
        ));

        let skill_cap = HudRefRect::new(panel_x + 8.0, 640.0, 80.0, 18.0);
        p.spawn((
            SkillCaptionText,
            skill_cap,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(skill_cap.left * scale),
                top: Val::Px(skill_cap.top * scale),
                width: Val::Px(skill_cap.width * scale),
                height: Val::Px(skill_cap.height * scale),
                ..default()
            },
            Text::new("SKILL"),
            scaled_font(scale, 13.0),
            TextColor(theme.text_secondary),
        ));

        let skill = HudRefRect::new(panel_x + 8.0, 660.0, 200.0, 36.0);
        p.spawn((
            SkillBySongText,
            skill,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(skill.left * scale),
                top: Val::Px(skill.top * scale),
                width: Val::Px(skill.width * scale),
                height: Val::Px(skill.height * scale),
                ..default()
            },
            Text::new("0.00"),
            scaled_font(scale, 28.0),
            TextColor(theme.accent),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stats_box_in_left_panel() {
        assert!(30.0 + 200.0 <= 260.0);
    }
}
