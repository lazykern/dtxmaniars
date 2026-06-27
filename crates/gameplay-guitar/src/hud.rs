//! Guitar performance HUD — minimal Bevy UI.
//!
//! ADR-0010 relaxed: free redesign. 5 lanes (R/G/B/Y/P) + hit line +
//! score + combo + gauge bar.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/*`
#![allow(missing_docs)]

use bevy::prelude::*;
use game_shell::AppState;

use crate::resources::{Combo, JudgmentCounts, Score};

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ComboText;

#[derive(Component)]
struct GaugeFill;

#[derive(Component)]
struct LaneRect {
    #[allow(dead_code)]
    lane: u8,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), spawn_hud)
        .add_systems(OnExit(AppState::Performance), despawn_hud)
        .add_systems(
            Update,
            (refresh_score_text, refresh_combo_text, refresh_gauge_bar)
                .run_if(in_state(AppState::Performance)),
        );
}

/// 5 lanes (R/G/B/Y/P) — `GUITAR_LANES`.
pub const LANE_COUNT: usize = 5;
pub const LANE_WIDTH_PX: f32 = 100.0;
pub const LANE_STRIP_LEFT_X: f32 = 220.0;
pub const LANE_STRIP_TOP_Y: f32 = 500.0;
pub const LANE_STRIP_BOTTOM_Y: f32 = 60.0;
pub const HIT_LINE_Y: f32 = 60.0;

/// Lane colors per B/G/R/Y/P convention.
fn lane_color(lane: u8) -> Color {
    match lane {
        0 => Color::srgb(0.9, 0.2, 0.2),  // R - red
        1 => Color::srgb(0.2, 0.7, 0.2),  // G - green
        2 => Color::srgb(0.3, 0.4, 0.9),  // B - blue
        3 => Color::srgb(0.9, 0.85, 0.2), // Y - yellow
        4 => Color::srgb(0.7, 0.4, 0.9),  // P - purple
        _ => Color::srgb(0.5, 0.5, 0.5),
    }
}

fn lane_label(lane: u8) -> &'static str {
    match lane {
        0 => "R",
        1 => "G",
        2 => "B",
        3 => "Y",
        4 => "P",
        _ => "?",
    }
}

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ))
        .with_children(|root| {
            // Lane strip — 5 columns colored RGBYP.
            for lane in 0..LANE_COUNT as u8 {
                root.spawn((
                    LaneRect { lane },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(LANE_STRIP_LEFT_X + (lane as f32) * LANE_WIDTH_PX),
                        top: Val::Px(LANE_STRIP_TOP_Y),
                        width: Val::Px(LANE_WIDTH_PX - 4.0),
                        height: Val::Px(LANE_STRIP_BOTTOM_Y - LANE_STRIP_TOP_Y),
                        ..default()
                    },
                    BackgroundColor(lane_color(lane).with_alpha(0.25)),
                ));
            }

            // Hit line.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(LANE_STRIP_LEFT_X),
                    top: Val::Px(HIT_LINE_Y),
                    width: Val::Px(LANE_COUNT as f32 * LANE_WIDTH_PX),
                    height: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            // Lane labels.
            for lane in 0..LANE_COUNT as u8 {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(LANE_STRIP_LEFT_X + (lane as f32) * LANE_WIDTH_PX + 30.0),
                        bottom: Val::Px(40.0),
                        width: Val::Px(40.0),
                        ..default()
                    },
                    Text::new(lane_label(lane)),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            }

            // Score (top-left).
            root.spawn((
                ScoreText,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(20.0),
                    ..default()
                },
                Text::new("Score: 0"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Combo (top-right).
            root.spawn((
                ComboText,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    top: Val::Px(20.0),
                    ..default()
                },
                Text::new("Combo: 0"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.8, 0.3)),
            ));

            // Gauge fill (bottom-left).
            root.spawn((
                GaugeFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(60.0),
                    height: Val::Px(24.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.4, 0.8, 0.4)),
            ));
            // Gauge border.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(300.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ));
        });
}

fn despawn_hud(mut commands: Commands, query: Query<Entity, With<HudRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn refresh_score_text(score: Res<Score>, mut q: Query<&mut Text, With<ScoreText>>) {
    if !score.is_changed() {
        return;
    }
    for mut t in &mut q {
        *t = Text::new(format!("Score: {}", score.0));
    }
}

fn refresh_combo_text(combo: Res<Combo>, mut q: Query<&mut Text, With<ComboText>>) {
    if !combo.is_changed() {
        return;
    }
    for mut t in &mut q {
        *t = Text::new(format!("Combo: {}", combo.current));
    }
}

fn refresh_gauge_bar(counts: Res<JudgmentCounts>, mut q: Query<&mut Node, With<GaugeFill>>) {
    if !counts.is_changed() {
        return;
    }
    let _total = counts.total();
    let good = (counts.perfect + counts.great + counts.good) as f32;
    let bad = (counts.miss + counts.ok) as f32;
    let mut pct: f32 = 20.0 + good * 0.5 - bad * 3.0;
    pct = pct.clamp(0.0, 100.0);
    let width_px = 300.0 * (pct / 100.0);
    for mut n in &mut q {
        n.width = Val::Px(width_px);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_count_is_5() {
        assert_eq!(LANE_COUNT, 5);
    }

    #[test]
    fn lane_label_all_five() {
        assert_eq!(lane_label(0), "R");
        assert_eq!(lane_label(1), "G");
        assert_eq!(lane_label(2), "B");
        assert_eq!(lane_label(3), "Y");
        assert_eq!(lane_label(4), "P");
    }

    #[test]
    fn lane_label_unknown_is_question() {
        assert_eq!(lane_label(99), "?");
    }

    #[test]
    fn lane_color_distinct() {
        // All 5 lane colors should differ.
        let colors: Vec<_> = (0..LANE_COUNT as u8).map(lane_color).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "lane {i} and {j} same color");
            }
        }
    }
}
