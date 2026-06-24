//! CStageResult port — post-play results screen.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/CStageResult.cs` (811 lines)
//!
//! ## M6a: CScoreIni persistence
//!
//! On OnExit(Result), append a `ScoreEntry` to the global `ScoreStore` and
//! save it to disk. The store is loaded on Startup by `main.rs`.
//!
//! ## M5 scope
//!
//! - Display: title, difficulty, score, max combo, per-judgment counts + %
//! - Rank from perfect percentage (S/A/B/C/D/E)
//! - ESC/ENTER → SongSelect

use bevy::prelude::*;
use dtx_scoring::{compute_chart_hash, Rank, ScoreEntry, ScoreStore, ScoreStoreError};
use game_shell::fade::start_fade;
use game_shell::{despawn_stage, AppState};
use gameplay_drums::resources::{ActiveChart, Combo, JudgmentCounts, Score};

#[derive(Component)]
pub struct ResultEntity;

/// Bevy wrapper around `dtx_scoring::ScoreStore`. Inserted once at startup.
/// Holds the persisted score history for the session.
#[derive(Resource, Deref, DerefMut, Default, Debug, Clone)]
pub struct ScoreStoreResource(pub ScoreStore);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Result), (spawn_result, start_fade))
        .add_systems(OnExit(AppState::Result), save_result_then_despawn)
        .add_systems(Update, result_input.run_if(in_state(AppState::Result)))
        .add_plugins(result_full::plugin)
        .init_resource::<result_stage::CStageResultState>();
}

/// Re-export as struct form for callers that prefer `add_plugins(...)` syntax.
pub use plugin as GameResultsPlugin;

pub mod result_full;
pub mod result_stage;
pub mod result_sub_acts;

fn spawn_result(
    mut commands: Commands,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
) {
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
    let bpm = chart
        .chart
        .metadata
        .bpm
        .map(|b| format!("{b}"))
        .unwrap_or_else(|| "?".into());
    let level = chart
        .chart
        .metadata
        .dlevel
        .map(|l| format!("{l}"))
        .unwrap_or_else(|| "?".into());
    let total = counts.total();
    let pct = counts.perfect_pct();
    let rank = Rank::from_perfect_pct(pct);

    let detail = format!(
        "Result\n\n\
         Title:    {title}\n\
         Artist:   {artist}\n\
         BPM:      {bpm}\n\
         Drums Lv: {level}\n\n\
         Score:    {}\n\
         Max Combo: {}\n\
         Rank:     {rank}\n\n\
         Perfect:  {} ({:.1}%)\n\
         Great:    {}\n\
         Good:     {}\n\
         Ok:       {}\n\
         Miss:     {}\n\
         Total:    {total}\n\n\
         ESC/ENTER → SongSelect",
        score.0, combo.max, counts.perfect, pct, counts.great, counts.good, counts.ok, counts.miss,
    );

    commands.spawn((
        ResultEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(40.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        children![(
            Text::new(detail),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
}

fn result_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::SongSelect);
    }
}

/// OnExit(Result): persist current play to ScoreStore, then despawn UI.
///
/// Order: save FIRST so despawn errors can't lose the entry.
fn save_result_then_despawn(
    mut commands: Commands,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    mut store: ResMut<ScoreStoreResource>,
    query: Query<Entity, With<ResultEntity>>,
) {
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
    let total = counts.total();
    let pct = if total == 0 {
        0.0
    } else {
        counts.perfect as f32 / total as f32 * 100.0
    };
    let rank = Rank::from_perfect_pct(pct);

    // Chart hash: SHA-256 of file contents; fall back to path if file missing.
    let chart_hash = chart
        .source_path
        .as_ref()
        .map(|p| compute_chart_hash(p))
        .unwrap_or_else(|| "unknown".into());

    let entry = ScoreEntry {
        chart_hash,
        title,
        artist,
        score: score.0 as u32,
        max_combo: combo.max,
        perfect: counts.perfect,
        great: counts.great,
        good: counts.good,
        ok: counts.ok,
        miss: counts.miss,
        rank,
        played_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    store.add(entry);
    if let Err(e) = store.save() {
        warn!("game-results: save failed: {e}");
    }

    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_scoring::Rank;

    #[test]
    fn rank_s_at_95_plus() {
        assert_eq!(Rank::from_perfect_pct(100.0), Rank::S);
        assert_eq!(Rank::from_perfect_pct(95.0), Rank::S);
    }

    #[test]
    fn rank_a_85_to_95() {
        assert_eq!(Rank::from_perfect_pct(94.9), Rank::A);
        assert_eq!(Rank::from_perfect_pct(85.0), Rank::A);
    }

    #[test]
    fn rank_b_70_to_85() {
        assert_eq!(Rank::from_perfect_pct(84.9), Rank::B);
        assert_eq!(Rank::from_perfect_pct(70.0), Rank::B);
    }

    #[test]
    fn rank_c_50_to_70() {
        assert_eq!(Rank::from_perfect_pct(69.9), Rank::C);
        assert_eq!(Rank::from_perfect_pct(50.0), Rank::C);
    }

    #[test]
    fn rank_d_25_to_50() {
        assert_eq!(Rank::from_perfect_pct(49.9), Rank::D);
        assert_eq!(Rank::from_perfect_pct(25.0), Rank::D);
    }

    #[test]
    fn rank_e_below_25() {
        assert_eq!(Rank::from_perfect_pct(24.9), Rank::E);
        assert_eq!(Rank::from_perfect_pct(0.0), Rank::E);
    }

    #[test]
    fn rank_display_strings() {
        assert_eq!(format!("{}", Rank::S), "S");
        assert_eq!(format!("{}", Rank::E), "E");
    }
}
