//! Drums performance HUD — minimal Bevy UI.
//!
//! ADR-0010 relaxed: free redesign. DTXManiaNX had 11 sub-acts for the
//! performance HUD (CActPerfDrumsScore, CActPerfDrumsComboDGB,
//! CActPerfDrumsGauge, CActPerfDrumsStatusPanel, etc). We collapse them
//! to a single minimal overlay: lane strip, hit line, score, combo,
//! gauge bar, judgment counts.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*`

use bevy::prelude::*;
use game_shell::AppState;

use crate::lane_map::LANE_ORDER;
use crate::resources::{Combo, JudgmentCounts, Score};

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ComboText;

#[derive(Component)]
struct GaugeBar;

#[derive(Component)]
struct GaugeFill;

#[derive(Component)]
struct JudgmentCountsText;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), spawn_hud)
        .add_systems(OnExit(AppState::Performance), despawn_hud)
        .add_systems(
            Update,
            (
                refresh_score_text,
                refresh_combo_text,
                refresh_gauge_bar,
                refresh_judgment_counts,
            )
                .run_if(in_state(AppState::Performance)),
        );
}

/// 9 lanes for drums (matches `LANE_ORDER` in lane_map.rs).
pub const LANE_COUNT: usize = 9;
pub const LANE_WIDTH_PX: f32 = 80.0;
pub const HIT_LINE_Y: f32 = 60.0;
pub const LANE_STRIP_TOP_Y: f32 = 500.0;
pub const LANE_STRIP_BOTTOM_Y: f32 = 60.0;
pub const LANE_STRIP_LEFT_X: f32 = 80.0;

fn spawn_hud(mut commands: Commands) {
    // Root: full-screen Node.
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
            // Lane strip — 9 columns.
            for i in 0..LANE_COUNT {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(LANE_STRIP_LEFT_X + (i as f32) * LANE_WIDTH_PX),
                        top: Val::Px(LANE_STRIP_TOP_Y),
                        width: Val::Px(LANE_WIDTH_PX - 4.0),
                        height: Val::Px(LANE_STRIP_BOTTOM_Y - LANE_STRIP_TOP_Y),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.4)),
                ));
            }

            // Hit line (horizontal).
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

            // Gauge bar (bottom-left, 0-100%).
            root.spawn((
                GaugeBar,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(300.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            ));
            // Gauge fill (child of bar).
            // Re-parented below.
            root.spawn((
                GaugeFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(60.0), // initial 20%
                    height: Val::Px(24.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.4, 0.8, 0.4)),
            ));

            // Judgment counts (bottom-right).
            root.spawn((
                JudgmentCountsText,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    ..default()
                },
                Text::new("Perfect: 0  Great: 0  Good: 0  Ok: 0  Miss: 0"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));

            // Lane labels (at the bottom of each lane).
            for (i, ch) in LANE_ORDER.iter().enumerate() {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(LANE_STRIP_LEFT_X + (i as f32) * LANE_WIDTH_PX + 4.0),
                        bottom: Val::Px(40.0),
                        width: Val::Px(LANE_WIDTH_PX - 12.0),
                        ..default()
                    },
                    Text::new(lane_label(*ch)),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                ));
            }
        });
}

fn lane_label(channel: dtx_core::EChannel) -> &'static str {
    use dtx_core::EChannel;
    match channel {
        EChannel::HiHatClose => "HH",
        EChannel::Snare => "SD",
        EChannel::BassDrum => "BD",
        EChannel::HighTom => "HT",
        EChannel::LowTom => "LT",
        EChannel::FloorTom => "FT",
        EChannel::Cymbal => "CY",
        EChannel::HiHatOpen => "HHO",
        EChannel::RideCymbal => "RD",
        _ => "?",
    }
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
    // Rough gauge: starts 20%, +0.5 per Perfect/Great/Good, -3 per Miss.
    // Approximation; real gauge logic in dtx-scoring::gauge.
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

fn refresh_judgment_counts(
    counts: Res<JudgmentCounts>,
    mut q: Query<&mut Text, With<JudgmentCountsText>>,
) {
    if !counts.is_changed() {
        return;
    }
    for mut t in &mut q {
        *t = Text::new(format!(
            "Perfect: {}  Great: {}  Good: {}  Ok: {}  Miss: {}",
            counts.perfect, counts.great, counts.good, counts.ok, counts.miss
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn lane_count_matches_order() {
        assert_eq!(LANE_COUNT, LANE_ORDER.len());
    }

    #[test]
    fn lane_label_all_drum_lanes() {
        let lanes = [
            EChannel::HiHatClose,
            EChannel::Snare,
            EChannel::BassDrum,
            EChannel::HighTom,
            EChannel::LowTom,
            EChannel::FloorTom,
            EChannel::Cymbal,
            EChannel::HiHatOpen,
            EChannel::RideCymbal,
        ];
        let labels: Vec<&'static str> = lanes.iter().map(|c| lane_label(*c)).collect();
        assert_eq!(
            labels,
            vec!["HH", "SD", "BD", "HT", "LT", "FT", "CY", "HHO", "RD"]
        );
    }

    #[test]
    fn lane_label_fallback_for_unknown() {
        // Pick a non-drum channel → "?".
        assert_eq!(lane_label(EChannel::BGM), "?");
    }

    #[test]
    fn lane_width_x_count_matches_lane_count() {
        // Sanity: HUD geometry width = LANE_COUNT * LANE_WIDTH_PX.
        let total = LANE_COUNT as f32 * LANE_WIDTH_PX;
        assert_eq!(total as usize, 9 * 80);
    }
}
