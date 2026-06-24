#![allow(missing_docs)]
//! Guitar HUD — minimal text overlay showing score + combo.
//!
//! M6b ships a single screen-center text. Visual lane strip + judgment
//! flashes + per-lane hit indicators are M6.1 (port
//! CActPerfGuitarLaneFlushGB.cs + CActPerfGuitarJudgementString.cs).

use bevy::prelude::*;

use crate::resources::{Combo, JudgmentCounts, Score};

#[derive(Component)]
pub struct GuitarHud;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_guitar_hud)
        .add_systems(Update, update_score_text)
        .add_systems(Update, update_combo_text)
        .add_systems(Update, update_judgment_text);
}

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ComboText;

#[derive(Component)]
struct JudgmentText;

fn spawn_guitar_hud(mut commands: Commands) {
    commands
        .spawn((
            GuitarHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Px(20.0),
                width: Val::Px(300.0),
                height: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .with_children(|parent| {
            parent.spawn((
                ScoreText,
                Text::new("Score: 0"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                ComboText,
                Text::new("Combo: 0"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.9, 1.0)),
            ));
            parent.spawn((
                JudgmentText,
                Text::new("P:0  G:0  Gd:0  Ok:0  M:0"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
        });
}

fn update_score_text(score: Res<Score>, mut q: Query<&mut Text, With<ScoreText>>) {
    if score.is_changed() {
        for mut t in &mut q {
            *t = Text::new(format!("Score: {}", score.0));
        }
    }
}

fn update_combo_text(combo: Res<Combo>, mut q: Query<&mut Text, With<ComboText>>) {
    if combo.is_changed() {
        for mut t in &mut q {
            *t = Text::new(format!("Combo: {} (max {})", combo.current, combo.max));
        }
    }
}

fn update_judgment_text(counts: Res<JudgmentCounts>, mut q: Query<&mut Text, With<JudgmentText>>) {
    if counts.is_changed() {
        for mut t in &mut q {
            *t = Text::new(format!(
                "P:{}  Gr:{}  Gd:{}  Ok:{}  M:{}",
                counts.perfect, counts.great, counts.good, counts.ok, counts.miss
            ));
        }
    }
}
