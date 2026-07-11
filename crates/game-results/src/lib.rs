//! CStageResult — animated stat reveals (ADR-0014).

// Bevy systems take many params and queries use deeply nested generic tuples;
// both trip these lints across this crate's systems. Bevy-idiomatic
// false-positives, allowed crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource};
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::{
    despawn_stage, request_transition, AppState, ScoreStoreResource, TransitionRequest,
};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::stage_end::LastStageOutcome;

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

fn pct(count: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        count as f32 / total as f32 * 100.0
    }
}

fn result_rank(counts: &JudgmentCounts, max_combo: u32, total: u32) -> Rank {
    Rank::from_bocud_counts(
        total,
        counts.perfect,
        counts.great,
        counts.good,
        counts.ok,
        counts.miss,
        max_combo,
    )
}

fn chart_identity(chart: &ActiveChart) -> ChartIdentity {
    let canonical = canonical_chart_hash(&chart.chart);
    let raw = chart
        .source_path
        .as_ref()
        .and_then(|path| raw_file_sha256(path).ok());
    ChartIdentity::new(canonical, raw, chart.source_path.clone())
}

fn native_score_entry(
    chart: ChartIdentity,
    title: String,
    artist: String,
    score: u32,
    max_combo: u32,
    counts: &JudgmentCounts,
    rank: Rank,
    played_at: u64,
) -> ScoreEntry {
    ScoreEntry {
        id: format!("native:{}:{score}:{played_at}", chart.canonical_hash),
        chart,
        title,
        artist,
        score,
        max_combo,
        judgments: JudgmentTotals {
            perfect: counts.perfect,
            great: counts.great,
            good: counts.good,
            poor: counts.ok,
            miss: counts.miss,
        },
        rank,
        played_at,
        source: ScoreSource::Native,
        replay_ref: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_result(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    midi: Option<Res<game_shell::MidiConnected>>,
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
    let total = scoring.total_notes;
    let rank = result_rank(&counts, combo.max, total);
    let t = theme.0;

    let stat_rows: Vec<(String, f32)> = vec![
        (title.to_string(), 0.0),
        (format!("{artist}  Lv.{difficulty}"), STAGGER_MS),
        (String::new(), STAGGER_MS * 2.0),
        (format!("Score     {}", score.0), STAGGER_MS * 3.0),
        (format!("Max Combo {}", combo.max), STAGGER_MS * 4.0),
        (format!("Rank      {rank}"), STAGGER_MS * 5.0),
        (String::new(), STAGGER_MS * 6.0),
        (
            format!(
                "Perfect   {} ({:.1}%)",
                counts.perfect,
                pct(counts.perfect, total)
            ),
            STAGGER_MS * 7.0,
        ),
        (
            format!(
                "Great     {} ({:.1}%)",
                counts.great,
                pct(counts.great, total)
            ),
            STAGGER_MS * 8.0,
        ),
        (
            format!(
                "Good      {} ({:.1}%)",
                counts.good,
                pct(counts.good, total)
            ),
            STAGGER_MS * 9.0,
        ),
        (
            format!("Poor      {} ({:.1}%)", counts.ok, pct(counts.ok, total)),
            STAGGER_MS * 10.0,
        ),
        (
            format!(
                "Miss      {} ({:.1}%)",
                counts.miss,
                pct(counts.miss, total)
            ),
            STAGGER_MS * 11.0,
        ),
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

    if midi.is_some_and(|m| m.0) {
        commands.entity(inner).with_children(|p| {
            dtx_ui::widget::nav_legend::spawn_nav_legend(p, &t, &[("BD", "continue")]);
        });
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

fn result_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<game_shell::NavAction>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    use game_shell::NavVerb;
    // Either pad verb continues; the mapper's screen-enter grace keeps the
    // song's last note from skipping this screen.
    let pad = actions
        .read()
        .any(|a| matches!(a.verb, NavVerb::Confirm | NavVerb::Back));
    if pad || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}

fn save_result_then_despawn(
    commands: Commands,
    practice: Option<Res<gameplay_drums::practice::PracticeSession>>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    outcome: Res<LastStageOutcome>,
    mut store: ResMut<ScoreStoreResource>,
    query: Query<Entity, With<ResultEntity>>,
) {
    // Practice runs are never persisted (no ScoreStore entry, no
    // score.ini update) — only the UI teardown happens.
    if practice.is_some() {
        despawn_stage::<ResultEntity>(commands, query);
        return;
    }
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
    let total = scoring.total_notes;
    let rank = result_rank(&counts, combo.max, total);
    let played_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let entry = native_score_entry(
        chart_identity(&chart),
        title,
        artist,
        score.0 as u32,
        combo.max,
        &counts,
        rank,
        played_at,
    );

    store.add(entry);
    if let Err(e) = store.save() {
        warn!("game-results: save failed: {e}");
    }

    // Also write a BocuD-compatible <chart>.score.ini next to the chart so
    // song select (and DTXManiaNX itself) can read the best score.
    if let Some(chart_path) = chart.source_path.as_ref() {
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
        let cleared = outcome.cleared && total > 0;
        if let Err(e) = dtx_scoring::score_ini::write_result(&ini_path, &record, cleared) {
            warn!("game-results: score.ini write failed: {e}");
        }
    }

    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_rank_uses_bocud_xg_formula() {
        let counts = JudgmentCounts {
            perfect: 90,
            great: 10,
            good: 0,
            ok: 0,
            miss: 0,
        };
        assert_eq!(result_rank(&counts, 100, 100), Rank::SS);
    }

    #[test]
    fn pct_zero_total_is_zero() {
        assert_eq!(pct(1, 0), 0.0);
    }

    #[test]
    fn native_score_entry_uses_chart_identity_and_poor_counts() {
        let chart_identity =
            dtx_scoring::identity::ChartIdentity::new("dtx1:test".into(), Some("raw".into()), None);
        let counts = JudgmentCounts {
            perfect: 3,
            great: 2,
            good: 1,
            ok: 4,
            miss: 5,
        };

        let entry = native_score_entry(
            chart_identity,
            "Title".into(),
            "Artist".into(),
            12345,
            9,
            &counts,
            Rank::A,
            42,
        );

        assert_eq!(entry.chart.canonical_hash, "dtx1:test");
        assert_eq!(entry.chart.raw_sha256.as_deref(), Some("raw"));
        assert_eq!(entry.judgments.poor, 4);
        assert_eq!(entry.source, dtx_scoring::ScoreSource::Native);
    }
}
