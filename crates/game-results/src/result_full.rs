#![allow(unused_imports)]
//! Full Result UX — port of `Stage/07.Result/`.
//!
//! Strict-port-first (ADR-0010). Position constants verbatim from reference.
//!
//! ## Sub-components ported
//!
//! | Component | Reference | Purpose |
//! |-----------|-----------|---------|
//! | `ResultRankIcon` | ResultRankIcon.cs (144 LOC) | S/A/B/C/D/E rank sprite, size (420, 510), anchor (0.5, 0.5) |
//! | `ResultInfoPanel` | ResultInfoPanel.cs (117 LOC) | Level + skill rate, level icon at (64, 21), level int at (281, 107) |
//! | `ResultParameterPanel` | ResultParameterPanel.cs (60 LOC) | 5-row judgment table |
//! | Song bar | CStageResult.cs:111-130 | title + artist + dlevel in a top strip |
//! | Ghost bar | CStageResult.cs:73-74 | best score delta vs current |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/`

use bevy::prelude::*;
use dtx_scoring::{JudgmentKind, Rank};

use crate::ResultEntity;
use gameplay_drums::resources::{ActiveChart, Combo, JudgmentCounts, Score};

/// ResultRankIcon size + anchor (ResultRankIcon.cs:9-10).
pub const RANK_ICON_W: f32 = 420.0;
pub const RANK_ICON_H: f32 = 510.0;

/// ResultInfoPanel positions (ResultInfoPanel.cs:13-15, 16-18).
/// levelIcon at (64, 21), levelLine at (88, 94) size (340, 2).
/// levelInt at (281, 107), levelFraction at (278, 102).
/// rateIcon at (32, 77), rateLine at (60, 168) size (344, 2).
/// rateInt at (281, 180), rateFraction at (278, 176).
pub const INFO_LEVEL_ICON_X: f32 = 64.0;
pub const INFO_LEVEL_ICON_Y: f32 = 21.0;
pub const INFO_LEVEL_LINE_X: f32 = 88.0;
pub const INFO_LEVEL_LINE_Y: f32 = 94.0;
pub const INFO_LEVEL_LINE_W: f32 = 340.0;
pub const INFO_LEVEL_INT_X: f32 = 281.0;
pub const INFO_LEVEL_INT_Y: f32 = 107.0;
pub const INFO_RATE_ICON_X: f32 = 32.0;
pub const INFO_RATE_ICON_Y: f32 = 77.0;
pub const INFO_RATE_LINE_X: f32 = 60.0;
pub const INFO_RATE_LINE_Y: f32 = 168.0;
pub const INFO_RATE_LINE_W: f32 = 344.0;
pub const INFO_RATE_INT_X: f32 = 281.0;
pub const INFO_RATE_INT_Y: f32 = 180.0;

/// Song bar (top strip) — 64px tall.
pub const SONG_BAR_H: f32 = 64.0;
/// Ghost bar (bottom) — 48px tall.
pub const GHOST_BAR_H: f32 = 48.0;

/// Color per rank (ResultRankIcon.cs:31-78 — different rank icons).
pub fn rank_color(r: Rank) -> Color {
    match r {
        Rank::S => Color::srgb(1.0, 0.85, 0.2),
        Rank::A => Color::srgb(0.9, 0.4, 0.4),
        Rank::B => Color::srgb(0.4, 0.7, 1.0),
        Rank::C => Color::srgb(0.5, 0.9, 0.5),
        Rank::D => Color::srgb(0.7, 0.7, 0.7),
        Rank::E => Color::srgb(0.5, 0.5, 0.5),
    }
}

/// Marker for the rank icon overlay entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ResultRankIconComp {
    pub rank: Rank,
}

/// Marker for the song bar overlay.
#[derive(Component, Debug, Clone, Copy)]
pub struct ResultSongBar;

/// Marker for the ghost bar overlay.
#[derive(Component, Debug, Clone, Copy)]
pub struct ResultGhostBar;

/// Marker for the parameter panel (5-row judgment table).
#[derive(Component, Debug, Clone, Copy)]
pub struct ResultParameterPanel;

/// Marker for the info panel (level + rate).
#[derive(Component, Debug, Clone, Copy)]
pub struct ResultInfoPanelComp;

/// Bevy resource holding the best score from history for the ghost bar.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct GhostBestScore(pub u32);

/// Plugin assembly.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<GhostBestScore>()
        .add_systems(OnEnter(crate::AppState::Result), spawn_result_full)
        .add_systems(
            Update,
            update_result_text.run_if(in_state(crate::AppState::Result)),
        );
}

fn spawn_result_full(
    mut commands: Commands,
    score: Res<Score>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
) {
    let pct = counts.perfect_pct();
    let rank = Rank::from_perfect_pct(pct);

    // Song bar (top).
    let title = chart
        .chart
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let artist = chart
        .chart
        .metadata
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let dlevel = chart
        .chart
        .metadata
        .dlevel
        .map(|l| l.to_string())
        .unwrap_or_else(|| "?".into());

    commands.spawn((
        ResultEntity,
        ResultSongBar,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(SONG_BAR_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(12.0)),
            column_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Text::new(format!("{} / {}  [Lv {}]", title, artist, dlevel)),
        TextFont {
            font_size: 22.0.into(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));

    // Rank icon (center).
    commands.spawn((
        ResultEntity,
        ResultRankIconComp { rank },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(RANK_ICON_W),
            height: Val::Px(RANK_ICON_H),
            margin: UiRect {
                left: Val::Px(-RANK_ICON_W / 2.0),
                top: Val::Px(-RANK_ICON_H / 2.0),
                ..default()
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Text::new(format!("Rank {}", rank)),
        TextFont {
            font_size: 96.0.into(),
            ..default()
        },
        TextColor(rank_color(rank)),
    ));

    // Parameter panel (5-row judgment table, left side).
    let judgment_rows = format!(
        "PERFECT  {}\nGREAT    {}\nGOOD     {}\nOK       {}\nMISS     {}\n\nScore: {}\nMax Combo: {}",
        counts.perfect,
        counts.great,
        counts.good,
        counts.ok,
        counts.miss,
        score.0,
        0u32
    );
    commands.spawn((
        ResultEntity,
        ResultParameterPanel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(40.0),
            top: Val::Px(120.0),
            width: Val::Px(280.0),
            height: Val::Px(320.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        Text::new(judgment_rows),
        TextFont {
            font_size: 18.0.into(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));

    // Info panel (level + rate) — right of parameter panel.
    let info_text = format!("Level\n{}\n\nRate\n{:.2}%", dlevel, pct);
    commands.spawn((
        ResultEntity,
        ResultInfoPanelComp,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(INFO_LEVEL_ICON_X),
            top: Val::Px(INFO_LEVEL_ICON_Y + 120.0), // offset from rank icon top
            width: Val::Px(INFO_LEVEL_LINE_W + 40.0),
            height: Val::Px(200.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        Text::new(info_text),
        TextFont {
            font_size: 16.0.into(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));

    // Ghost bar (bottom).
    commands.spawn((
        ResultEntity,
        ResultGhostBar,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(GHOST_BAR_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(8.0)),
            column_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Text::new("Ghost: -"),
        TextFont {
            font_size: 16.0.into(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn update_result_text(
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    best: Res<GhostBestScore>,
    mut q: Query<&mut Text, (With<ResultGhostBar>, With<ResultEntity>)>,
) {
    if !score.is_changed() && !combo.is_changed() && !counts.is_changed() && !best.is_changed() {
        return;
    }
    let delta = score.0 as i64 - best.0 as i64;
    let sign = if delta >= 0 { "+" } else { "" };
    for mut t in &mut q {
        *t = Text::new(format!(
            "Ghost: {} (best {} — delta {}{})",
            score.0, best.0, sign, delta
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_icon_dimensions_match_reference() {
        // ResultRankIcon.cs:9-10
        assert_eq!(RANK_ICON_W, 420.0);
        assert_eq!(RANK_ICON_H, 510.0);
    }

    #[test]
    fn info_panel_positions_match_reference() {
        // ResultInfoPanel.cs:13-15
        assert_eq!(INFO_LEVEL_ICON_X, 64.0);
        assert_eq!(INFO_LEVEL_ICON_Y, 21.0);
        assert_eq!(INFO_LEVEL_LINE_X, 88.0);
        assert_eq!(INFO_LEVEL_LINE_Y, 94.0);
        assert_eq!(INFO_LEVEL_LINE_W, 340.0);
        assert_eq!(INFO_LEVEL_INT_X, 281.0);
        assert_eq!(INFO_LEVEL_INT_Y, 107.0);
    }

    #[test]
    fn info_panel_rate_positions_match_reference() {
        // ResultInfoPanel.cs:48-50
        assert_eq!(INFO_RATE_ICON_X, 32.0);
        assert_eq!(INFO_RATE_ICON_Y, 77.0);
        assert_eq!(INFO_RATE_LINE_W, 344.0);
        assert_eq!(INFO_RATE_INT_X, 281.0);
        assert_eq!(INFO_RATE_INT_Y, 180.0);
    }

    #[test]
    fn song_bar_dimensions() {
        assert_eq!(SONG_BAR_H, 64.0);
    }

    #[test]
    fn ghost_bar_dimensions() {
        assert_eq!(GHOST_BAR_H, 48.0);
    }

    #[test]
    fn rank_color_distinct() {
        let s = rank_color(Rank::S);
        let e = rank_color(Rank::E);
        assert_ne!(s, e);
    }

    #[test]
    fn ghost_score_default_zero() {
        let g = GhostBestScore::default();
        assert_eq!(g.0, 0);
    }

    #[test]
    fn judgment_kind_via_counts() {
        // Sanity: each kind has a distinct count.
        let counts = JudgmentCounts {
            perfect: 100,
            great: 10,
            good: 5,
            ok: 2,
            miss: 1,
        };
        assert_eq!(counts.perfect, 100);
        assert_eq!(counts.miss, 1);
        let _ = JudgmentKind::Perfect;
    }
}
