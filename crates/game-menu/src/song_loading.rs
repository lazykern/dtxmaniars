//! CStageSongLoading — chart + BGM preview load (M4 minimum viable).
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines)
//!
//! DTXManiaNX behavior:
//! 1. OnActivate: read selected song from SongSelectionNew
//! 2. Load DTX file, parse (CDTX.cs), build CChartData
//! 3. Load BGM via DSound (CActPerfDrumsDGB.cs)
//! 4. Show progress bar (0..1)
//! 5. On finish → CStagePerfDrumsScreen
//!
//! M4 ports:
//! - Read SelectedSong (set by SongSelect)
//! - Load DTX via dtx-assets::DtxCache
//! - Store in gameplay_drums::ActiveChart
//! - Show "Loading..." + simulated progress
//! - Transition to Performance when load completes (real or simulated)
//!
//! BGM playback + real asset progress → M5.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use bevy_kira_audio::prelude::{Audio, AudioInstance};
use dtx_audio::BgmHandle;
use dtx_bga::{ActiveChartRes, BgaPlayer};
use dtx_core::{Chart, ParseReport, resolve_bgm_path};
use dtx_ui::motion::EnterChoreo;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::panel;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{
    AppState, NavAction, SystemVerb, TransitionRequest, despawn_stage, request_transition,
};
use gameplay_drums::resources::ActiveChart as DrumsActiveChart;

use crate::song_select::{SelectedSong, Selection, SongSelectSelection};

#[derive(Component)]
pub struct LoadingEntity;

/// Fill bar of the hero-card progress track. Width is lerped toward the
/// phase-derived target percent every frame in `update_status_text`.
#[derive(Component)]
struct LoadingBarFill;

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LoadingProgress(pub f32);

/// User-requested cancellation via Esc. Set in `watch_cancel_key`; consumed by
/// `poll_chart_parse` (drops the task) and `advance_when_loaded`.
#[derive(Resource, Debug, Default, Clone, Copy)]
struct CancelRequested(bool);

/// Per-chart ghost lag read from `<chart>.perfect.dr.ghost` on load. Stored
/// for later wiring to a ghost-replay lane (M14+); we just read + store here.
///
/// `CStageSongLoading.LoadSongDataAsync` does the same read in
/// `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs:535-580`.
#[derive(Resource, Default, Debug)]
struct GhostLag {
    /// Lag samples in chip-occurrence order. Empty when no ghost file exists.
    pub drums: Vec<i16>,
}

/// Tracks which phase the loader is in.
///
/// `Parsing` → chart is being read + parsed on a background thread.
/// `LoadingAudio` → chart parsed, waiting for WAV asset handles to finish
/// decoding (real `AssetServer` load-state, not a timer).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
enum LoadPhase {
    #[default]
    Idle,
    Parsing,
    LoadingAudio,
    Ready,
    Failed,
}

#[derive(Resource, Default)]
struct LoadingAdvanceGate(bool);

/// Background chart-parse task. Keeps the parse off the main thread so the
/// loading screen stays smooth (mirrors `dtxpt`'s `ChartLoad` background task).
#[derive(Resource, Default)]
struct ChartParseTask(Option<Task<Result<ParseReport, String>>>);

/// Chart WAV handles the loader blocks on before starting gameplay.
/// BocuD loads every used WAV before entering Performance
/// (`CStageSongLoading.cs:700-708`).
#[derive(Resource, Default)]
struct RequiredAudio(Vec<gameplay_drums::sound_bank::PreloadedAudio>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadProblemKind {
    ParserWarning,
    MissingAudio,
    UnsupportedAudio,
    AudioSubstitution,
    DecoderFailure,
    MissingVisual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadProblem {
    kind: LoadProblemKind,
    path: Option<std::path::PathBuf>,
    detail: String,
}

#[derive(Resource, Debug, Default)]
struct LoadDiagnostics {
    fatal: Option<String>,
    warnings: Vec<LoadProblem>,
    advance_not_before: Option<std::time::Instant>,
}

impl LoadDiagnostics {
    fn push_warning(&mut self, problem: LoadProblem) {
        if self
            .warnings
            .iter()
            .any(|existing| existing.kind == problem.kind && existing.path == problem.path)
        {
            return;
        }
        self.warnings.push(problem);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadSupport {
    Supported,
    Degraded { problems: Vec<String> },
    Rejected { problems: Vec<String> },
}

fn load_support(diagnostics: &LoadDiagnostics) -> LoadSupport {
    if let Some(fatal) = &diagnostics.fatal {
        LoadSupport::Rejected {
            problems: vec![fatal.clone()],
        }
    } else if diagnostics.warnings.is_empty() {
        LoadSupport::Supported
    } else {
        LoadSupport::Degraded {
            problems: diagnostics
                .warnings
                .iter()
                .map(|problem| problem.detail.clone())
                .collect(),
        }
    }
}

fn load_failure_for(chart: &Chart) -> Option<String> {
    chart
        .drum_chips()
        .next()
        .is_none()
        .then(|| "selected conditional branch contains no playable drum chips".to_string())
}

const fn failure_hold_seconds() -> f64 {
    2.5
}

fn warning_hold_seconds(warnings: &[LoadProblem]) -> f64 {
    if warnings.is_empty() { 0.0 } else { 0.75 }
}

fn required_audio_slots(chart: &Chart) -> std::collections::BTreeSet<u32> {
    gameplay_drums::sound_bank::collect_preload_wav_slots(chart)
}

fn apply_chart_audio_report(
    report: gameplay_drums::sound_bank::ChartAudioReport,
    diagnostics: &mut LoadDiagnostics,
) {
    use gameplay_drums::sound_bank::PreloadIssueKind;

    for substitution in report.substitutions {
        diagnostics.push_warning(LoadProblem {
            kind: LoadProblemKind::AudioSubstitution,
            path: Some(substitution.resolved.clone()),
            detail: format!(
                "chart audio slot {} substituted {} with {}",
                substitution.slot,
                substitution.requested.display(),
                substitution.resolved.display()
            ),
        });
    }
    for issue in report.warnings {
        let kind = match issue.kind {
            PreloadIssueKind::Missing => LoadProblemKind::MissingAudio,
            PreloadIssueKind::Unsupported => LoadProblemKind::UnsupportedAudio,
        };
        diagnostics.push_warning(LoadProblem {
            kind,
            path: Some(issue.path.clone()),
            detail: format!(
                "chart audio slot {} at {}: {}",
                issue.slot,
                issue.path.display(),
                issue.guidance
            ),
        });
    }
    if !report.required_failures.is_empty() {
        diagnostics.fatal = Some(
            report
                .required_failures
                .into_iter()
                .map(|issue| {
                    format!(
                        "required BGM slot {} at {} cannot be loaded: {}",
                        issue.slot,
                        issue.path.display(),
                        issue.guidance
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<LoadingProgress>()
        .init_resource::<LoadPhase>()
        .init_resource::<LoadingAdvanceGate>()
        .init_resource::<CancelRequested>()
        .init_resource::<GhostLag>()
        .init_resource::<ChartParseTask>()
        .init_resource::<RequiredAudio>()
        .init_resource::<LoadDiagnostics>()
        .add_systems(
            OnEnter(AppState::SongLoading),
            (reset_advance_gate, start_load, spawn_loading).chain(),
        )
        .add_systems(OnEnter(AppState::SongLoading), persist_last_played)
        .add_systems(
            OnExit(AppState::SongLoading),
            (
                stop_nowloading,
                clear_required_audio,
                despawn_stage::<LoadingEntity>,
            )
                .chain(),
        )
        .add_systems(OnExit(AppState::Performance), dtx_bga::clear_visuals)
        .add_systems(
            Update,
            (
                watch_cancel_key.after(game_shell::NavRouterSet),
                poll_chart_parse,
                wait_for_audio.after(poll_chart_parse),
                update_status_text.after(wait_for_audio),
                advance_when_loaded.after(update_status_text),
            )
                .run_if(in_state(AppState::SongLoading)),
        );
}

/// Kick off the background parse. Clears the chart sound bank for the new song
/// so `wait_for_audio` only waits on the handles this chart preloads.
fn start_load(
    selected: Res<SelectedSong>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut phase: ResMut<LoadPhase>,
    mut task: ResMut<ChartParseTask>,
    mut bank: ResMut<dtx_audio::ChartSoundBank>,
    mut required: ResMut<RequiredAudio>,
    mut progress: ResMut<LoadingProgress>,
    mut cancel: ResMut<CancelRequested>,
    mut diagnostics: ResMut<LoadDiagnostics>,
) {
    progress.0 = 0.0;
    cancel.0 = false;
    diagnostics.fatal = None;
    diagnostics.warnings.clear();
    diagnostics.advance_not_before = None;
    bank.clear();
    required.0.clear();
    if let Some(path) = selected.0.as_ref() {
        *phase = LoadPhase::Parsing;
        // Header parse (synchronous header peek) is what BocuD does in
        // OnActivate. We don't read SOUND_NOWLOADING until the full parse is
        // done — fall back to "Loading…" if the chart doesn't ship one.
        let path_clone = path.clone();
        let pool = AsyncComputeTaskPool::get();
        task.0 = Some(pool.spawn(async move {
            dtx_assets::load_chart_report(&path_clone).map_err(|e| e.to_string())
        }));
    } else {
        warn!("SongLoading entered with no SelectedSong; using empty chart");
        task.0 = None;
        *phase = LoadPhase::Ready;
    }
    // Sound effects only — fires once on enter and stops on exit.
    let _ = (&audio, &asset_server);
}

/// Poll the background parse. When it finishes, publish the chart to the drums,
/// guitar and BGA resources, then start the audio WAV preload and move to the
/// `LoadingAudio` phase to wait on the asset handles.
#[allow(clippy::too_many_arguments)]
fn poll_chart_parse(
    selected: Res<SelectedSong>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut task: ResMut<ChartParseTask>,
    cancel: Res<CancelRequested>,
    mut phase: ResMut<LoadPhase>,
    mut drums_chart: ResMut<DrumsActiveChart>,
    mut bga_player: ResMut<BgaPlayer>,
    mut bank: ResMut<dtx_audio::ChartSoundBank>,
    mut required: ResMut<RequiredAudio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut ghost: ResMut<GhostLag>,
    mut diagnostics: ResMut<LoadDiagnostics>,
    mut commands: Commands,
) {
    let Some(active) = task.0.as_mut() else {
        return;
    };
    let Some(result) = block_on(future::poll_once(active)) else {
        return;
    };
    task.0 = None;

    let path = selected.0.clone();
    match result {
        Ok(report) => {
            if cancel.0 {
                *phase = LoadPhase::Failed;
                return;
            }
            for diagnostic in &report.diagnostics {
                warn!(
                    path = ?path,
                    line = ?diagnostic.line,
                    kind = ?diagnostic.kind,
                    detail = %diagnostic.detail,
                    recovery = ?diagnostic.recovery,
                    "chart compatibility diagnostic"
                );
                diagnostics.push_warning(LoadProblem {
                    kind: LoadProblemKind::ParserWarning,
                    path: None,
                    detail: match diagnostic.line {
                        Some(line) => format!("line {line}: {}", diagnostic.detail),
                        None => diagnostic.detail.clone(),
                    },
                });
            }
            let chart = report.chart;
            if let Some(fatal) = load_failure_for(&chart) {
                error!("SongLoading: {fatal}");
                diagnostics.fatal = Some(fatal);
                diagnostics.advance_not_before = Some(
                    std::time::Instant::now()
                        + std::time::Duration::from_secs_f64(failure_hold_seconds()),
                );
                *phase = LoadPhase::Failed;
                return;
            }
            info!(
                "Parsed {:?} ({} chips, BPM {:?}, dlevel {:?})",
                chart.format,
                chart.chips.len(),
                chart.metadata.bpm,
                chart.metadata.dlevel
            );
            drums_chart.chart = chart.clone();
            drums_chart.source_path = path.clone();
            // M7.1: publish prepared visual events + resolved asset paths.
            let active_visuals = ActiveChartRes::from_chart(&chart, path.as_deref());
            record_missing_visuals(&chart, &active_visuals, &mut diagnostics);
            bga_player.reset();
            bga_player.event_count = active_visuals.events.len();
            commands.insert_resource(active_visuals);
            // BocuD loads every used WAV before entering Performance
            // (CStageSongLoading.cs:700-708). Waiting prevents unloaded BGM/SE
            // play commands from releasing together as an audible burst.
            use gameplay_drums::sound_bank;
            let slots = required_audio_slots(&chart);
            let batch =
                sound_bank::preload_slots_report(&drums_chart, &asset_server, &mut bank, &slots);
            apply_chart_audio_report(batch.report, &mut diagnostics);
            if diagnostics.fatal.is_some() {
                diagnostics.advance_not_before = Some(
                    std::time::Instant::now()
                        + std::time::Duration::from_secs_f64(failure_hold_seconds()),
                );
                *phase = LoadPhase::Failed;
                return;
            }
            required.0 = batch.assets;
            info!("SongLoading: waiting on {} chart WAVs", required.0.len());
            // Read the perfect-drums ghost (BocuD CStageSongLoading.cs:535-580).
            // Stored for later replay-lane wiring; reverse-score computation is
            // M14+ (needs judgement classifier integration).
            ghost.drums = path
                .as_ref()
                .and_then(|p| {
                    dtx_scoring::score_ini::read_ghost_lag(dtx_scoring::score_ini::ghost_path(
                        p, "dr", "perfect",
                    ))
                })
                .unwrap_or_default();
            if !ghost.drums.is_empty() {
                info!(
                    "SongLoading: loaded {} ghost lag samples",
                    ghost.drums.len()
                );
            }
            // Per-chart SOUND_NOWLOADING cue (BocuD #SOUND_NOWLOADING). Falls
            // back to silence when the chart doesn't ship one — there's no
            // skin-level "soundNowLoading" yet, so we don't synthesize one.
            play_nowloading(
                &chart,
                path.as_deref(),
                &audio,
                &asset_server,
                &mut bgm,
                &mut instances,
            );
            *phase = LoadPhase::LoadingAudio;
        }
        Err(e) => {
            error!("Failed to load chart: {e}");
            diagnostics.fatal = Some(e);
            diagnostics.advance_not_before = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs_f64(failure_hold_seconds()),
            );
            *phase = LoadPhase::Failed;
        }
    }
}

fn record_missing_visuals(
    chart: &Chart,
    active_visuals: &ActiveChartRes,
    diagnostics: &mut LoadDiagnostics,
) {
    let source_dir = active_visuals.source_dir.as_deref();
    for (&id, filename) in chart
        .assets
        .bmp
        .by_id
        .iter()
        .chain(chart.assets.bga.by_id.iter())
    {
        if !active_visuals.bmp_paths.contains_key(&id) {
            diagnostics.push_warning(LoadProblem {
                kind: LoadProblemKind::MissingVisual,
                path: source_dir.map(|dir| dir.join(filename.replace('\\', "/"))),
                detail: format!("image asset {id:02X} ({filename})"),
            });
        }
    }
    for (&id, filename) in &chart.assets.avi.by_id {
        if !active_visuals.avi_paths.contains_key(&id) {
            diagnostics.push_warning(LoadProblem {
                kind: LoadProblemKind::MissingVisual,
                path: source_dir.map(|dir| dir.join(filename.replace('\\', "/"))),
                detail: format!("movie asset {id:02X} ({filename})"),
            });
        }
    }
}

/// Wait for every requested WAV handle to reach a terminal load state (Loaded
/// or Failed). Progress reflects the real decoded fraction.
fn wait_for_audio(
    mut phase: ResMut<LoadPhase>,
    mut required: ResMut<RequiredAudio>,
    asset_server: Res<AssetServer>,
    mut progress: ResMut<LoadingProgress>,
    mut diagnostics: ResMut<LoadDiagnostics>,
) {
    if *phase != LoadPhase::LoadingAudio {
        return;
    }
    let total = required.0.len();
    if total == 0 {
        progress.0 = 1.0;
        *phase = LoadPhase::Ready;
        diagnostics.advance_not_before = (!diagnostics.warnings.is_empty()).then(|| {
            std::time::Instant::now()
                + std::time::Duration::from_secs_f64(warning_hold_seconds(&diagnostics.warnings))
        });
        return;
    }
    let done = required
        .0
        .iter()
        .filter(|asset| {
            matches!(
                asset_server.get_load_state(asset.handle.id()),
                Some(state) if state.is_loaded() || state.is_failed()
            )
        })
        .count();
    for asset in &required.0 {
        if let Some(state) = asset_server.get_load_state(asset.handle.id())
            && state.is_failed()
            && !diagnostics.warnings.iter().any(|problem| {
                problem.kind == LoadProblemKind::DecoderFailure
                    && problem.path.as_ref() == Some(&asset.path)
            })
        {
            let detail = format!("audio decoder rejected {}", asset.path.display());
            if asset.requirement == gameplay_drums::sound_bank::AudioRequirement::RequiredBgm {
                diagnostics.fatal = Some(format!("required BGM {detail}"));
            } else {
                diagnostics.push_warning(LoadProblem {
                    kind: LoadProblemKind::DecoderFailure,
                    path: Some(asset.path.clone()),
                    detail,
                });
            }
        }
    }
    progress.0 = done as f32 / total as f32;
    if done >= total {
        *phase = if diagnostics.fatal.is_some() {
            LoadPhase::Failed
        } else {
            LoadPhase::Ready
        };
        diagnostics.advance_not_before =
            (diagnostics.fatal.is_some() || !diagnostics.warnings.is_empty()).then(|| {
                std::time::Instant::now()
                    + std::time::Duration::from_secs_f64(if diagnostics.fatal.is_some() {
                        failure_hold_seconds()
                    } else {
                        warning_hold_seconds(&diagnostics.warnings)
                    })
            });
        required.0.clear();
    }
}

fn clear_required_audio(mut required: ResMut<RequiredAudio>) {
    required.0.clear();
}

fn reset_advance_gate(mut gate: ResMut<LoadingAdvanceGate>) {
    gate.0 = false;
}

/// Hero-card loading screen: stage background + a centered panel showing
/// the selected song's art, title, artist/BPM/difficulty chip, a yellow
/// progress bar (`LoadingBarFill`), and a status line (`LoadingStatusText`).
/// Song metadata is read from `Selection`/`SongSelectSelection`/`SongDb` —
/// all populated by SongSelect before the transition, so it's available
/// immediately on enter (no need to wait for the chart parse to finish).
fn spawn_loading(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    asset_server: Res<AssetServer>,
) {
    let t = theme.0;
    let difficulty_index = selection.difficulty;
    commands.insert_resource(game_shell::SelectedDifficulty(difficulty_index));
    let song = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .cloned();
    let (title, artist, bpm, dlevel, difficulty, art) = match &song {
        Some(s) => (
            s.title.clone(),
            s.artist.clone(),
            s.bpm,
            s.dlevel,
            crate::song_select::SongFolderView::difficulty_label_for(&s.path, selection.difficulty),
            s.preimage_path.clone(),
        ),
        None => (
            "Unknown".into(),
            String::new(),
            None,
            None,
            String::new(),
            None,
        ),
    };

    commands
        .spawn((
            LoadingEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);
            root.spawn((
                panel(
                    &t,
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(24.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        ..default()
                    },
                ),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 40.0), 0.0, 250.0),
            ))
            .with_children(|card| {
                let mut img = ImageNode {
                    color: Color::WHITE.with_alpha(if art.is_some() { 1.0 } else { 0.0 }),
                    ..default()
                };
                if let Some(p) = &art {
                    img.image = asset_server.load(p.to_string_lossy().to_string());
                }
                card.spawn((
                    Node {
                        width: Val::Px(160.0),
                        height: Val::Px(160.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                    BorderColor::all(t.select_yellow),
                    img,
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new("NOW LOADING"),
                        Theme::font(12.0),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                        TextColor(t.text_secondary),
                    ));
                    col.spawn((
                        Text::new(title),
                        Theme::font(34.0),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Title),
                        TextColor(t.text_primary),
                    ));
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|meta| {
                        meta.spawn((
                            Text::new(format!(
                                "{artist} · BPM {}",
                                bpm.map(|v| (v.round() as i32).to_string())
                                    .unwrap_or_else(|| "?".into())
                            )),
                            Theme::font(15.0),
                            dtx_ui::SemanticText(dtx_ui::TypographyRole::Body),
                            TextColor(t.text_secondary),
                        ));
                        meta.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(t.difficulty_color(difficulty_index)),
                        ))
                        .with_children(|chip| {
                            chip.spawn((
                                Text::new(format!(
                                    "{difficulty} {}",
                                    dlevel
                                        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
                                        .unwrap_or_else(|| "--".into())
                                )),
                                Theme::font(12.0),
                                dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                                TextColor(t.text_primary),
                            ));
                        });
                    });
                    col.spawn((
                        Node {
                            width: Val::Px(420.0),
                            height: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.0)),
                            margin: UiRect::top(Val::Px(14.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.13, 0.13, 0.13)),
                        BorderColor::all(t.stage_panel_border),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            LoadingBarFill,
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(t.select_yellow),
                            BoxShadow::new(
                                t.select_yellow.with_alpha(0.5),
                                Val::Px(0.0),
                                Val::Px(0.0),
                                Val::Px(1.0),
                                Val::Px(8.0),
                            ),
                        ));
                    });
                    col.spawn((
                        Text::new(""),
                        Theme::font(12.0),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                        TextColor(t.text_secondary),
                        LoadingStatusText,
                    ));
                    col.spawn((
                        Text::new("Esc / SD — cancel"),
                        Theme::font(12.0),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                        TextColor(t.text_secondary),
                    ));
                });
            });
        });
}

/// Watch for a cancel during load: `SystemVerb::Back` as a `NavAction` — the
/// router delivers keyboard Esc, the pad mapper delivers kit SD while
/// `NavContext::SongLoading` is active. On cancel, mark the load; the next
/// `poll_chart_parse` tick sees the flag and fails fast, and
/// `advance_when_loaded` routes back to SongSelect.
fn watch_cancel_key(
    mut actions: MessageReader<NavAction>,
    mut cancel: ResMut<CancelRequested>,
    phase: Res<LoadPhase>,
) {
    if cancel.0 {
        actions.clear();
        return;
    }
    if matches!(
        *phase,
        LoadPhase::Idle | LoadPhase::Ready | LoadPhase::Failed
    ) {
        actions.clear();
        return;
    }
    if actions.read().any(|action| action.verb == SystemVerb::Back) {
        info!("SongLoading: cancel requested — cancelling load");
        cancel.0 = true;
    }
}

/// Play the per-chart `#SOUND_NOWLOADING` jingle (BocuD CStageSongLoading.cs:220-230).
/// Fires once after the parse completes — no loop, no skin fallback.
fn play_nowloading(
    chart: &Chart,
    source_path: Option<&std::path::Path>,
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
) {
    let Some(snd) = chart.metadata.sound_nowloading.as_deref() else {
        return;
    };
    let Some(parent_dir) = source_path.and_then(|p| p.parent()) else {
        return;
    };
    let path = parent_dir.join(snd);
    if !path.is_file() {
        return;
    }
    let cfg = dtx_config::load(&dtx_config::default_path());
    if !cfg.audio.bgm_enabled {
        return;
    }
    // Reuse the BGM slot for the nowloading jingle so `stop_bgm` on exit cleans it up.
    let _ = dtx_audio::play_bgm_with_volume(
        audio,
        asset_server,
        bgm,
        instances,
        &path.to_string_lossy(),
        cfg.audio.master_volume * cfg.audio.bgm_volume,
        0,
    );
}

/// Stop any nowloading clip on exit so it doesn't bleed into Performance.
fn stop_nowloading(
    audio: Res<Audio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    dtx_audio::stop_bgm(&audio, &mut bgm, &mut instances);
}

/// Resolve the chart's primary BGM path so callers can decide whether a
/// `drums.ogg` heuristic file exists. Currently unused; surfaced for callers
/// that want to short-circuit load when only BGM is missing.
#[allow(dead_code)]
fn primary_bgm_hint(
    chart: &Chart,
    source_path: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    let p = source_path?;
    resolve_bgm_path(p, chart)
}

#[derive(Component)]
struct LoadingStatusText;

/// Drives the hero card's smooth progress bar + friendlier status line.
/// The bar lerps toward a phase-derived target percent every frame so it
/// never jumps backward or snaps; the status text mirrors DTXManiaNX's
/// "loading audio chips… N/M" wording.
fn update_status_text(
    time: Res<Time>,
    phase: Res<LoadPhase>,
    progress: Res<LoadingProgress>,
    required: Res<RequiredAudio>,
    diagnostics: Res<LoadDiagnostics>,
    mut status_query: Query<&mut Text, With<LoadingStatusText>>,
    mut bar: Query<&mut Node, With<LoadingBarFill>>,
) {
    let target_pct = match *phase {
        LoadPhase::Parsing => 8.0,
        LoadPhase::LoadingAudio => 10.0 + progress.0 * 88.0,
        LoadPhase::Ready => 100.0,
        _ => 0.0,
    };
    for mut node in &mut bar {
        let current = match node.width {
            Val::Percent(p) => p,
            _ => 0.0,
        };
        let next = current + (target_pct - current) * (8.0 * time.delta_secs()).min(1.0);
        node.width = Val::Percent(next.clamp(0.0, 100.0));
    }
    let total = required.0.len();
    let support = load_support(&diagnostics);
    let status = match *phase {
        LoadPhase::Idle => String::new(),
        LoadPhase::Parsing => "parsing chart…".to_string(),
        LoadPhase::LoadingAudio => format!(
            "loading audio chips… {}/{}",
            ((progress.0 * total as f32).round() as usize).min(total),
            total
        ),
        LoadPhase::Ready => match support {
            LoadSupport::Supported => "ready — supported".to_string(),
            LoadSupport::Degraded { problems } => format!(
                "ready — degraded with warning ({}); continuing…",
                problems.len()
            ),
            LoadSupport::Rejected { .. } => "rejected — returning to song select".to_string(),
        },
        LoadPhase::Failed => diagnostics
            .fatal
            .clone()
            .unwrap_or_else(|| "failed — returning to song select".to_string()),
    };
    for mut text in &mut status_query {
        *text = Text::new(status.clone());
    }
}

fn advance_when_loaded(
    phase: Res<LoadPhase>,
    cancel: Res<CancelRequested>,
    mut requests: MessageWriter<TransitionRequest>,
    mut gate: ResMut<LoadingAdvanceGate>,
    diagnostics: Res<LoadDiagnostics>,
) {
    if gate.0 {
        return;
    }
    if cancel.0 && !matches!(*phase, LoadPhase::Ready | LoadPhase::Failed) {
        return;
    }
    if diagnostics
        .advance_not_before
        .is_some_and(|deadline| std::time::Instant::now() < deadline)
    {
        return;
    }
    match *phase {
        LoadPhase::Failed => {
            warn!("SongLoading: load failed, returning to SongSelect");
            request_transition(&mut requests, AppState::SongSelect);
            gate.0 = true;
        }
        LoadPhase::Ready => {
            request_transition(&mut requests, AppState::Performance);
            gate.0 = true;
        }
        _ => {}
    }
}

/// Remember the song for the editor session (`gameplay.last_played`).
/// Normal runs only — the editor session must not overwrite it with itself.
fn persist_last_played(selected: Res<SelectedSong>, session: Res<game_shell::EditorSession>) {
    if session.0 {
        return;
    }
    let Some(path) = selected.0.clone() else {
        return;
    };
    let cfg_path = dtx_config::default_path();
    let mut cfg = dtx_config::load(&cfg_path);
    if cfg.gameplay.last_played.as_ref() == Some(&path) {
        return;
    }
    cfg.gameplay.last_played = Some(path);
    if let Err(e) = dtx_config::save(&cfg_path, &cfg) {
        warn!("failed to persist last_played: {e}");
    }
}

#[cfg(test)]
mod tests {
    //! Pure logic tests for SongLoading state machine.

    use super::*;

    #[test]
    fn load_phase_default_is_idle() {
        assert_eq!(LoadPhase::default(), LoadPhase::Idle);
    }

    #[test]
    fn loading_progress_default_is_zero() {
        assert_eq!(LoadingProgress::default().0, 0.0);
    }

    #[test]
    fn load_support_names_supported_degraded_and_rejected_states() {
        let mut diagnostics = LoadDiagnostics::default();
        assert_eq!(load_support(&diagnostics), LoadSupport::Supported);
        diagnostics.push_warning(LoadProblem {
            kind: LoadProblemKind::MissingAudio,
            path: Some("optional.ogg".into()),
            detail: "optional audio missing".into(),
        });
        assert!(matches!(
            load_support(&diagnostics),
            LoadSupport::Degraded { .. }
        ));
        diagnostics.fatal = Some("required gameplay structure unsupported".into());
        assert!(matches!(
            load_support(&diagnostics),
            LoadSupport::Rejected { .. }
        ));
    }

    #[test]
    fn xa_audio_report_maps_required_failures_to_rejected_and_optional_to_degraded() {
        use gameplay_drums::sound_bank::{
            AudioRequirement, ChartAudioReport, PreloadIssue, PreloadIssueKind,
        };

        let issue = |requirement| PreloadIssue {
            slot: 1,
            path: "music.xa".into(),
            kind: PreloadIssueKind::Unsupported,
            requirement,
            guidance: "provide a same-stem OGG, WAV, or MP3 file".into(),
        };
        let mut diagnostics = LoadDiagnostics::default();
        apply_chart_audio_report(
            ChartAudioReport {
                warnings: vec![issue(AudioRequirement::Optional)],
                ..default()
            },
            &mut diagnostics,
        );
        assert!(matches!(
            load_support(&diagnostics),
            LoadSupport::Degraded { .. }
        ));

        apply_chart_audio_report(
            ChartAudioReport {
                required_failures: vec![issue(AudioRequirement::RequiredBgm)],
                ..default()
            },
            &mut diagnostics,
        );
        assert!(matches!(
            load_support(&diagnostics),
            LoadSupport::Rejected { .. }
        ));
    }

    #[test]
    fn load_policy_rejects_empty_chart_and_accepts_playable_drums() {
        assert!(load_failure_for(&Chart::default()).is_some());
        let playable = Chart {
            chips: vec![dtx_core::Chip::with_wav(
                0,
                dtx_core::EChannel::Snare,
                0.0,
                1,
            )],
            ..default()
        };
        assert_eq!(load_failure_for(&playable), None);
    }

    #[test]
    fn load_policy_uses_readable_failure_and_warning_holds() {
        assert_eq!(failure_hold_seconds(), 2.5);
        assert_eq!(
            warning_hold_seconds(&[LoadProblem {
                kind: LoadProblemKind::ParserWarning,
                path: None,
                detail: "line 1: x".into(),
            }]),
            0.75
        );
        assert_eq!(warning_hold_seconds(&[]), 0.0);
    }

    #[test]
    fn required_audio_includes_bgm_and_auto_se() {
        use dtx_core::{Chip, EChannel};

        let chart = Chart {
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),
                Chip::with_wav(0, EChannel::SE01, 0.0, 2),
                Chip::with_wav(0, EChannel::Snare, 0.0, 3),
            ],
            ..Default::default()
        };

        let required = required_audio_slots(&chart);

        assert!(required.contains(&1), "BGM WAV must finish loading");
        assert!(required.contains(&2), "auto-SE WAV must finish loading");
        assert!(required.contains(&3), "drum WAV must finish loading");
    }

    /// SD (`SystemVerb::Back`) from the kit cancels the load, same as Esc.
    #[test]
    fn pad_back_cancels_the_load() {
        assert!(run_cancel_watch(Some(SystemVerb::Back), LoadPhase::Parsing));
    }

    #[test]
    fn pad_confirm_does_not_cancel_the_load() {
        assert!(!run_cancel_watch(
            Some(SystemVerb::Confirm),
            LoadPhase::Parsing
        ));
    }

    #[test]
    fn pad_back_after_load_finished_does_not_cancel() {
        assert!(!run_cancel_watch(Some(SystemVerb::Back), LoadPhase::Ready));
    }

    /// Drive `watch_cancel_key` once with an optional pad action queued.
    fn run_cancel_watch(verb: Option<SystemVerb>, phase: LoadPhase) -> bool {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use game_shell::InputSource;

        let mut world = World::new();
        world.init_resource::<Messages<NavAction>>();
        world.init_resource::<CancelRequested>();
        world.insert_resource(phase);
        if let Some(verb) = verb {
            world.write_message(NavAction {
                verb,
                source: InputSource::MidiKit,
                coarse: false,
                repeated: false,
            });
        }
        world
            .run_system_once(watch_cancel_key)
            .expect("watch_cancel_key runs");
        world.resource::<CancelRequested>().0
    }
}
