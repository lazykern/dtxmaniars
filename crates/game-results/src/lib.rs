//! CStageResult — animated stat reveals (ADR-0014).

// Bevy systems take many params and queries use deeply nested generic tuples;
// both trip these lints across this crate's systems. Bevy-idiomatic
// false-positives, allowed crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod input;
mod ui;

use bevy::prelude::*;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource};
use game_shell::{AppState, ScoreStoreResource};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::stage_end::LastStageOutcome;

#[derive(Component)]
pub struct ResultEntity;

/// Outcome of the on-entry persistence attempt, shown as the save-status line.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SaveStatus {
    #[default]
    Practice, // nothing to save
    Saved,
    Failed,
}

pub struct GameResultsPlugin;

impl Plugin for GameResultsPlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SaveStatus>()
        .init_resource::<input::ResultVerb>()
        .add_systems(
            OnEnter(AppState::Result),
            (save_result, ui::spawn_result).chain(),
        )
        .add_systems(OnExit(AppState::Result), ui::despawn_result)
        .add_systems(
            Update,
            (
                input::result_nav,
                ui::sync_verb_row,
                ui::animate_staggered_reveal,
            )
                .chain()
                .run_if(in_state(AppState::Result)),
        );
}

pub(crate) fn result_rank(counts: &JudgmentCounts, max_combo: u32, total: u32) -> Rank {
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

fn save_result(
    practice: Option<Res<gameplay_drums::practice::PracticeSession>>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    outcome: Res<LastStageOutcome>,
    mut store: ResMut<ScoreStoreResource>,
    mut status: ResMut<SaveStatus>,
) {
    // Practice runs are never persisted (no ScoreStore entry, no
    // score.ini update).
    if practice.is_some() {
        *status = SaveStatus::Practice;
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
    *status = if let Err(e) = store.save() {
        warn!("game-results: save failed: {e}");
        SaveStatus::Failed
    } else {
        SaveStatus::Saved
    };

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
            *status = SaveStatus::Failed;
        }
    }
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

    #[test]
    fn save_status_defaults_to_practice() {
        assert_eq!(SaveStatus::default(), SaveStatus::Practice);
    }

    #[test]
    fn save_result_persists_entry_and_sets_saved() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.init_resource::<SaveStatus>();
        world.insert_resource(Score(1234));
        world.insert_resource(Combo { current: 0, max: 9 });
        world.insert_resource(JudgmentCounts {
            perfect: 1,
            great: 0,
            good: 0,
            ok: 0,
            miss: 0,
        });
        // No source_path: skips raw hashing and the score.ini write.
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: None,
        });
        world.insert_resource(DrumScoring {
            total_notes: 1,
            ..Default::default()
        });
        world.insert_resource(LastStageOutcome { cleared: true });
        // Path-less store: save() succeeds without touching the filesystem.
        world.insert_resource(ScoreStoreResource::default());

        world
            .run_system_once(save_result)
            .expect("save_result runs");

        assert_eq!(world.resource::<ScoreStoreResource>().entries.len(), 1);
        assert_eq!(*world.resource::<SaveStatus>(), SaveStatus::Saved);
    }
}
