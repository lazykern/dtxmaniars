//! CStageResult — animated stat reveals (ADR-0014).

// Bevy systems take many params and queries use deeply nested generic tuples;
// both trip these lints across this crate's systems. Bevy-idiomatic
// false-positives, allowed crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod input;
mod ui;

use bevy::prelude::*;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use dtx_scoring::skill::{drum_performance_skill, drum_song_skill, DrumAutoPlay};
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource};
use game_shell::{
    AppState, CompletedRunContext, PracticeRecommendation, ResultReturnState, RunKind,
    ScoreStoreResource,
};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::results_analysis::{NormalPlayEventStream, PerformanceAnalysis};
use gameplay_drums::stage_end::LastStageOutcome;
use gameplay_drums::timeline::ChipTimeline;

#[derive(Component)]
pub struct ResultEntity;

/// Outcome of the on-entry persistence attempt, shown as the save-status line.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub(crate) enum SaveStatus {
    #[default]
    Practice, // nothing to save
    Saved,
    Failed,
    NoFail,
    ModifiedSpeed {
        rate: f64,
    },
}

/// Ephemeral diagnostic context for the current Result entry.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(crate) struct ResultAnalysis {
    pub report: PerformanceAnalysis,
    pub pb_delta: Option<i64>,
    pub recommendation: Option<PracticeRecommendation>,
}

/// Result-owned copy of every completed-run value used by the presentation.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(crate) struct ResultDisplaySnapshot {
    pub title: String,
    pub artist: String,
    pub drum_level: Option<f32>,
    pub difficulty: u8,
    pub score: i64,
    pub max_combo: u32,
    pub counts: JudgmentCounts,
    pub total_notes: u32,
    pub failed: bool,
    pub status: SaveStatus,
    pub analysis: ResultAnalysis,
}

pub struct GameResultsPlugin;

impl Plugin for GameResultsPlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SaveStatus>()
        .init_resource::<ResultAnalysis>()
        .init_resource::<ResultDisplaySnapshot>()
        .init_resource::<input::ResultVerb>()
        .add_systems(
            OnEnter(AppState::Result),
            (
                snapshot_result_analysis_system.run_if(process_result_entry),
                save_result.run_if(process_result_entry),
                snapshot_result_display_system.run_if(process_result_entry),
                finish_result_entry_system,
                ui::spawn_result,
            )
                .chain(),
        )
        .add_systems(OnExit(AppState::Result), ui::despawn_result)
        .add_systems(
            Update,
            (
                input::result_nav,
                ui::sync_verb_row,
                ui::sync_details_panel,
                ui::scroll_result,
                ui::animate_staggered_reveal,
            )
                .chain()
                .run_if(in_state(AppState::Result)),
        );
}

pub(crate) fn should_process_result(state: &ResultReturnState) -> bool {
    !state.skip_processing_once
}

fn process_result_entry(state: Res<ResultReturnState>) -> bool {
    should_process_result(&state)
}

pub(crate) fn finish_result_entry(state: &mut ResultReturnState) {
    if state.skip_processing_once {
        state.skip_processing_once = false;
    } else {
        state.available = true;
    }
}

fn finish_result_entry_system(mut state: ResMut<ResultReturnState>) {
    finish_result_entry(&mut state);
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
    score: i64,
    total: u32,
    chart_level: f64,
    max_combo: u32,
    counts: &JudgmentCounts,
    rank: Rank,
    played_at: u64,
    no_fail: bool,
) -> ScoreEntry {
    let performance_skill = drum_performance_skill(
        total,
        counts.perfect,
        counts.great,
        counts.good,
        counts.ok,
        counts.miss,
        max_combo,
        DrumAutoPlay::default(),
    );
    ScoreEntry {
        id: format!(
            "native:{}:{score}:{played_at}{}",
            chart.canonical_hash,
            if no_fail { ":nf" } else { "" }
        ),
        chart,
        title,
        artist,
        score,
        chart_level,
        performance_skill,
        song_skill: drum_song_skill(chart_level, performance_skill, false),
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
        no_fail,
    }
}

fn save_result(
    run: Res<game_shell::CompletedRunContext>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    outcome: Res<LastStageOutcome>,
    mut store: ResMut<ScoreStoreResource>,
    mut status: ResMut<SaveStatus>,
) {
    if run.kind == game_shell::RunKind::Practice {
        *status = SaveStatus::Practice;
        return;
    }
    if (run.playback_rate - 1.0).abs() >= 1e-9 {
        *status = SaveStatus::ModifiedSpeed {
            rate: run.playback_rate,
        };
        return;
    }
    let no_fail = run.modifiers.no_fail;
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
    let chart_level = chart
        .chart
        .metadata
        .display_drum_level()
        .map(f64::from)
        .unwrap_or(0.0);
    let performance_skill = drum_performance_skill(
        total,
        counts.perfect,
        counts.great,
        counts.good,
        counts.ok,
        counts.miss,
        combo.max,
        DrumAutoPlay::default(),
    );
    let played_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let entry = native_score_entry(
        chart_identity(&chart),
        title,
        artist,
        score.0,
        total,
        chart_level,
        combo.max,
        &counts,
        rank,
        played_at,
        no_fail,
    );

    store.add(entry);
    *status = if let Err(e) = store.save() {
        warn!("game-results: save failed: {e}");
        SaveStatus::Failed
    } else if no_fail {
        SaveStatus::NoFail
    } else {
        SaveStatus::Saved
    };

    // A No Fail play is recorded in native history only: score.ini has no way
    // to mark an assisted run, so writing it would poison the DTXManiaNX best
    // score.
    if no_fail {
        return;
    }

    // Also write a BocuD-compatible <chart>.score.ini next to the chart so
    // song select (and DTXManiaNX itself) can read the best score.
    if let Some(chart_path) = chart.source_path.as_ref() {
        let ini_path = dtx_scoring::score_ini::score_ini_path(chart_path);
        let bgm_adjust = dtx_scoring::score_ini::read_bgm_adjust(&ini_path);
        let record = dtx_scoring::score_ini::DrumScoreIni {
            score: score.0,
            play_skill: performance_skill,
            song_skill: drum_song_skill(chart_level, performance_skill, false),
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

fn snapshot_result_analysis_system(
    run: Res<CompletedRunContext>,
    score: Res<Score>,
    chart: Res<ActiveChart>,
    store: Res<ScoreStoreResource>,
    events: Res<NormalPlayEventStream>,
    timeline: Res<ChipTimeline>,
    mut analysis: ResMut<ResultAnalysis>,
) {
    *analysis = snapshot_result_analysis(&run, score.0, &chart, &store, &events, &timeline);
}

fn snapshot_result_display_system(
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    difficulty: Res<game_shell::SelectedDifficulty>,
    outcome: Res<LastStageOutcome>,
    status: Res<SaveStatus>,
    analysis: Res<ResultAnalysis>,
    mut display: ResMut<ResultDisplaySnapshot>,
) {
    *display = ResultDisplaySnapshot {
        title: chart
            .metadata()
            .title
            .clone()
            .unwrap_or_else(|| "Unknown".into()),
        artist: chart
            .metadata()
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown".into()),
        drum_level: chart.metadata().display_drum_level(),
        difficulty: difficulty.0,
        score: score.0,
        max_combo: combo.max,
        counts: *counts,
        total_notes: scoring.total_notes,
        failed: !outcome.cleared,
        status: *status,
        analysis: analysis.clone(),
    };
}

fn snapshot_result_analysis(
    run: &CompletedRunContext,
    score: i64,
    chart: &ActiveChart,
    store: &ScoreStoreResource,
    events: &NormalPlayEventStream,
    timeline: &ChipTimeline,
) -> ResultAnalysis {
    let report = PerformanceAnalysis::from_stream(events, &timeline.bar_ms);
    let comparable = run.kind == RunKind::Normal
        && (run.playback_rate - 1.0).abs() < 1e-9
        && !run.modifiers.no_fail;
    if !comparable {
        return ResultAnalysis {
            report,
            ..Default::default()
        };
    }

    let identity = chart_identity(chart);
    let pb_delta = store
        .best_for_chart(&identity.canonical_hash)
        .map(|best| score - best.score);
    let recommendation = report.weakest_section.map(|section| {
        PracticeRecommendation::weak_section(
            section.loop_start_ms,
            section.loop_end_ms,
            report.weakest_lane.map(|lane| lane.lane),
        )
    });
    ResultAnalysis {
        report,
        pb_delta,
        recommendation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_world(source_path: Option<std::path::PathBuf>, rate: f64) -> World {
        let mut world = World::new();
        world.init_resource::<SaveStatus>();
        world.insert_resource(game_shell::CompletedRunContext::normal(
            rate,
            game_shell::RunModifiers::default(),
        ));
        world.insert_resource(Score(1234));
        world.insert_resource(Combo { current: 0, max: 9 });
        world.insert_resource(JudgmentCounts {
            perfect: 1,
            great: 0,
            good: 0,
            ok: 0,
            miss: 0,
        });
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path,
        });
        world.insert_resource(DrumScoring {
            total_notes: 1,
            ..Default::default()
        });
        world.insert_resource(LastStageOutcome { cleared: true });
        world.insert_resource(ScoreStoreResource::default());
        world
    }

    #[test]
    fn no_fail_result_records_history_but_never_writes_score_ini() {
        use bevy::ecs::system::RunSystemOnce;

        let dir = std::env::temp_dir().join(format!("dtxmaniars-no-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let chart_path = dir.join("chart.dtx");
        std::fs::write(&chart_path, b"#TITLE: Assisted\n#00113: 01\n").unwrap();
        let mut world = result_world(Some(chart_path.clone()), 1.0);
        world.insert_resource(game_shell::CompletedRunContext::normal(
            1.0,
            game_shell::RunModifiers { no_fail: true },
        ));

        world.run_system_once(save_result).unwrap();

        assert_eq!(*world.resource::<SaveStatus>(), SaveStatus::NoFail);
        let store = world.resource::<ScoreStoreResource>();
        assert_eq!(store.entries.len(), 1);
        assert!(store.entries[0].no_fail);
        assert!(store
            .best_for_chart(&store.entries[0].chart.canonical_hash)
            .is_none());
        let hits = store.history_for_path(&chart_path, 8);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].no_fail);
        assert!(!dtx_scoring::score_ini::score_ini_path(&chart_path).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

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
            15,
            8.2,
            9,
            &counts,
            Rank::A,
            42,
            false,
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
    fn result_analysis_compares_before_saving_and_recommends_a_section() {
        use gameplay_drums::results_analysis::{NormalPlayEventStream, RecordedJudgment};
        use gameplay_drums::timeline::ChipTimeline;

        let chart = ActiveChart::default();
        let mut store = ScoreStoreResource::default();
        store.add(native_score_entry(
            chart_identity(&chart),
            "Prior".into(),
            "Player".into(),
            800,
            3,
            0.0,
            3,
            &JudgmentCounts::default(),
            Rank::Unknown,
            1,
            false,
        ));
        let events = NormalPlayEventStream {
            events: vec![
                RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Miss, 0, 0, 2_100),
                RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Poor, -20, 1, 2_200),
                RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Poor, -25, 2, 2_300),
            ],
            truncated: false,
        };
        let timeline = ChipTimeline {
            bar_ms: vec![0, 2_000, 4_000, 6_000],
            ..Default::default()
        };

        let result = snapshot_result_analysis(
            &game_shell::CompletedRunContext::normal(1.0, game_shell::RunModifiers::default()),
            900,
            &chart,
            &store,
            &events,
            &timeline,
        );
        assert_eq!(result.pb_delta, Some(100));
        assert_eq!(
            result.recommendation.expect("weak section").loop_start_ms,
            0
        );
    }

    #[test]
    fn save_result_persists_entry_and_sets_saved() {
        use bevy::ecs::system::RunSystemOnce;

        // No source_path: skips raw hashing and the score.ini write. The
        // path-less store succeeds without touching the filesystem.
        let mut world = result_world(None, 1.0);

        world
            .run_system_once(save_result)
            .expect("save_result runs");

        assert_eq!(world.resource::<ScoreStoreResource>().entries.len(), 1);
        assert_eq!(*world.resource::<SaveStatus>(), SaveStatus::Saved);
    }

    #[test]
    fn save_result_skips_modified_speed_native_and_score_ini_writes() {
        use bevy::ecs::system::RunSystemOnce;

        let dir = std::env::temp_dir().join(format!(
            "dtxmaniars-modified-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture directory");
        let chart_path = dir.join("chart.dtx");
        std::fs::write(&chart_path, b"#TITLE: Modified\n#00113: 01\n")
            .expect("write chart fixture");
        let mut world = result_world(Some(chart_path.clone()), 0.75);

        world
            .run_system_once(save_result)
            .expect("save_result runs");

        assert!(world.resource::<ScoreStoreResource>().entries.is_empty());
        assert_eq!(
            *world.resource::<SaveStatus>(),
            SaveStatus::ModifiedSpeed { rate: 0.75 }
        );
        assert!(!dtx_scoring::score_ini::score_ini_path(&chart_path).exists());
        std::fs::remove_dir_all(dir).expect("remove fixture directory");
    }

    #[test]
    fn returning_from_setup_uses_the_fresh_result_snapshot_once() {
        use bevy::input::mouse::MouseWheel;
        use gameplay_drums::results_analysis::{NormalPlayEventStream, RecordedJudgment};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<game_shell::PracticeIntent>()
            .init_resource::<ResultReturnState>()
            .add_message::<game_shell::NavAction>()
            .add_message::<game_shell::TransitionRequest>()
            .add_message::<MouseWheel>()
            .insert_resource(dtx_ui::ThemeResource::default())
            .insert_resource(game_shell::SelectedDifficulty(2))
            .insert_resource(CompletedRunContext::normal(
                1.0,
                game_shell::RunModifiers::default(),
            ))
            .insert_resource(Score(1234))
            .insert_resource(Combo { current: 0, max: 9 })
            .insert_resource(JudgmentCounts {
                perfect: 3,
                great: 0,
                good: 0,
                ok: 0,
                miss: 0,
            })
            .insert_resource(DrumScoring {
                total_notes: 3,
                ..Default::default()
            })
            .insert_resource(LastStageOutcome { cleared: true })
            .insert_resource(ScoreStoreResource::default())
            .insert_resource(NormalPlayEventStream {
                events: vec![
                    RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Miss, 0, 0, 2_100),
                    RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Poor, -20, 1, 2_200),
                    RecordedJudgment::new(3, dtx_scoring::JudgmentKind::Poor, -25, 2, 2_300),
                ],
                truncated: false,
            })
            .insert_resource(ChipTimeline {
                bar_ms: vec![0, 2_000, 4_000, 6_000],
                ..Default::default()
            });
        let mut chart = ActiveChart::default();
        chart.chart.metadata.title = Some("Original result".into());
        app.insert_resource(chart);
        plugin(&mut app);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Result);
        app.update();

        assert_eq!(
            app.world().resource::<ScoreStoreResource>().entries.len(),
            1
        );
        let original_analysis = app.world().resource::<ResultAnalysis>().clone();

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Performance);
        app.update();
        *app.world_mut().resource_mut::<Score>() = Score::default();
        *app.world_mut().resource_mut::<Combo>() = Combo::default();
        *app.world_mut().resource_mut::<JudgmentCounts>() = JudgmentCounts::default();
        *app.world_mut().resource_mut::<DrumScoring>() = DrumScoring::default();
        *app.world_mut().resource_mut::<LastStageOutcome>() = LastStageOutcome { cleared: false };
        app.world_mut()
            .resource_mut::<ActiveChart>()
            .chart
            .metadata
            .title = Some("Reset gameplay".into());
        app.world_mut()
            .resource_mut::<NormalPlayEventStream>()
            .clear();
        app.world_mut()
            .resource_mut::<ResultReturnState>()
            .skip_processing_once = true;

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Result);
        app.update();

        assert_eq!(
            app.world().resource::<ScoreStoreResource>().entries.len(),
            1,
            "the return entry must not save a second score"
        );
        assert_eq!(
            *app.world().resource::<ResultAnalysis>(),
            original_analysis,
            "the return entry must not recompute analysis"
        );
        let return_state = app.world().resource::<ResultReturnState>();
        assert!(return_state.available);
        assert!(!return_state.skip_processing_once);
        let mut entities = app.world_mut().query::<&ResultEntity>();
        assert_eq!(entities.iter(app.world()).count(), 1, "result UI respawned");
        let mut text_query = app.world_mut().query::<&Text>();
        let texts = text_query
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(texts.iter().any(|text| text == "Original result"));
        assert!(texts.iter().any(|text| text == "1,234"));
        assert!(!texts.iter().any(|text| text == "Reset gameplay"));
        assert!(!texts.iter().any(|text| text == "STAGE FAILED"));
    }
}
