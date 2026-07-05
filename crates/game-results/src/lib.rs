//! CStageResult — animated stat reveals (ADR-0014).

use bevy::prelude::*;
use dtx_scoring::{compute_chart_hash, Rank, ScoreEntry, ScoreStore};
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::{despawn_stage, request_transition, AppState, TransitionRequest};
use gameplay_drums::resources::{ActiveChart, Combo, JudgmentCounts, Score};

#[derive(Component)]
pub struct ResultEntity;

#[derive(Component)]
struct ResultPanel;

/// Marks a stat row for staggered reveal.
#[derive(Component)]
struct StatRow {
    reveal_at_ms: f32,
}

#[derive(Resource)]
struct ResultReveal {
    elapsed_ms: f32,
}

/// Bevy wrapper around `dtx_scoring::ScoreStore`.
#[derive(Resource, Deref, DerefMut, Default, Debug, Clone)]
pub struct ScoreStoreResource(pub ScoreStore);

pub struct GameResultsPlugin;

impl Plugin for GameResultsPlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Result), spawn_result)
        .add_systems(OnExit(AppState::Result), save_result_then_despawn)
        .add_systems(
            Update,
            (result_input, animate_staggered_reveal).run_if(in_state(AppState::Result)),
        );
}

const STAGGER_MS: f32 = 120.0;
const FADE_DURATION_MS: f32 = 300.0;

fn spawn_result(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
) {
    commands.insert_resource(ResultReveal { elapsed_ms: 0.0 });

    let title = chart
        .chart
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .chart
        .metadata
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let difficulty = chart
        .chart
        .metadata
        .dlevel
        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
        .unwrap_or_else(|| "--".into());
    let total = counts.total();
    let pct = counts.perfect_pct();
    let rank = Rank::from_perfect_pct(pct);
    let t = theme.0;

    let stat_rows: Vec<(String, f32)> = vec![
        (format!("{title}"), 0.0),
        (format!("{artist}  Lv.{difficulty}"), STAGGER_MS),
        (String::new(), STAGGER_MS * 2.0),
        (format!("Score     {}", score.0), STAGGER_MS * 3.0),
        (format!("Max Combo {}", combo.max), STAGGER_MS * 4.0),
        (format!("Rank      {rank}"), STAGGER_MS * 5.0),
        (String::new(), STAGGER_MS * 6.0),
        (
            format!("Perfect   {} ({pct:.1}%)", counts.perfect),
            STAGGER_MS * 7.0,
        ),
        (format!("Great     {}", counts.great), STAGGER_MS * 8.0),
        (format!("Good      {}", counts.good), STAGGER_MS * 9.0),
        (format!("Poor      {}", counts.ok), STAGGER_MS * 10.0),
        (format!("Miss      {}", counts.miss), STAGGER_MS * 11.0),
        (format!("Total     {total}"), STAGGER_MS * 12.0),
        (String::new(), STAGGER_MS * 13.0),
        ("ESC / ENTER → Song Select".to_string(), STAGGER_MS * 14.0),
    ];

    let panel = commands
        .spawn((
            ResultEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .id();

    let inner = commands
        .spawn((
            ResultPanel,
            Node {
                padding: UiRect::all(Val::Px(48.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(400.0),
                ..default()
            },
            BackgroundColor(t.panel_bg),
        ))
        .id();

    commands.entity(panel).add_child(inner);

    for (text, delay) in stat_rows {
        if text.is_empty() {
            let spacer = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Node {
                        height: Val::Px(16.0),
                        ..default()
                    },
                ))
                .id();
            commands.entity(inner).add_child(spacer);
        } else {
            let row = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Text::new(text),
                    Theme::label_font(),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                ))
                .id();
            commands.entity(inner).add_child(row);
        }
    }
}

fn animate_staggered_reveal(
    time: Res<Time>,
    mut reveal: ResMut<ResultReveal>,
    mut q: Query<(&StatRow, &mut TextColor)>,
) {
    reveal.elapsed_ms += time.delta_secs() * 1000.0;
    for (stat, mut color) in &mut q {
        let since = reveal.elapsed_ms - stat.reveal_at_ms;
        if since < 0.0 {
            continue;
        }
        let alpha = (since / FADE_DURATION_MS).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(alpha);
    }
}

fn result_input(keys: Res<ButtonInput<KeyCode>>, mut requests: MessageWriter<TransitionRequest>) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}

fn save_result_then_despawn(
    commands: Commands,
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
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .chart
        .metadata
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let total = counts.total();
    let pct = if total == 0 {
        0.0
    } else {
        counts.perfect as f32 / total as f32 * 100.0
    };
    let rank = Rank::from_perfect_pct(pct);

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

    // Also write a BocuD-compatible <chart>.score.ini next to the chart so
    // song select (and DTXManiaNX itself) can read the best score.
    if let Some(chart_path) = chart.source_path.as_ref() {
        let played_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ini_path = dtx_scoring::score_ini::score_ini_path(chart_path);
        let bgm_adjust = dtx_scoring::score_ini::read_bgm_adjust(&ini_path);
        let record = dtx_scoring::score_ini::DrumScoreIni {
            score: score.0 as u32,
            perfect: counts.perfect,
            great: counts.great,
            good: counts.good,
            poor: counts.ok,
            miss: counts.miss,
            max_combo: combo.max,
            total_chips: total,
            rank: rank.to_string(),
            play_count: 1,
            clear_count: 0,
            bgm_adjust,
            date_time: dtx_scoring::score_ini::format_datetime(played_at),
        };
        // Cleared when the player reached the end with any judged chips and
        // did not fail — approximated here as "produced a rank with chips".
        let cleared = total > 0;
        if let Err(e) = dtx_scoring::score_ini::write_result(&ini_path, &record, cleared) {
            warn!("game-results: score.ini write failed: {e}");
        }
    }

    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use dtx_scoring::Rank;

    #[test]
    fn rank_s_at_95_plus() {
        assert_eq!(Rank::from_perfect_pct(100.0), Rank::S);
        assert_eq!(Rank::from_perfect_pct(95.0), Rank::S);
    }

    #[test]
    fn rank_e_below_25() {
        assert_eq!(Rank::from_perfect_pct(0.0), Rank::E);
    }
}
