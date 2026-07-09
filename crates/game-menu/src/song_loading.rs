//! CStageSongLoading — chart + BGM preview load (M4 minimum viable).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines)
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

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use bevy_kira_audio::prelude::{Audio, AudioInstance, AudioSource as KiraAudioSource};
use dtx_audio::BgmHandle;
use dtx_bga::{ActiveChartRes, BgaLayerOverlay, BgaPlayer};
use dtx_core::{Chart, resolve_bgm_path};
use dtx_ui::motion::EnterChoreo;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::panel;
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};
use gameplay_drums::resources::ActiveChart as DrumsActiveChart;
use gameplay_guitar::resources::ActiveChart as GuitarActiveChart;

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
/// `references/DTXmaniaNX-BocuD/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs:535-580`.
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
struct ChartParseTask(Option<Task<Result<Chart, String>>>);

/// Immediate-tier WAV handles the loader blocks on before starting gameplay
/// (deferred BGM/SE handles decode in the background — see `sound_bank`).
#[derive(Resource, Default)]
struct RequiredAudio(Vec<Handle<KiraAudioSource>>);

pub fn plugin(app: &mut App) {
    app.init_resource::<LoadingProgress>()
        .init_resource::<LoadPhase>()
        .init_resource::<LoadingAdvanceGate>()
        .init_resource::<CancelRequested>()
        .init_resource::<GhostLag>()
        .init_resource::<ChartParseTask>()
        .init_resource::<RequiredAudio>()
        .add_systems(
            OnEnter(AppState::SongLoading),
            (reset_advance_gate, start_load, spawn_loading).chain(),
        )
        .add_systems(OnEnter(AppState::SongLoading), persist_last_played)
        .add_systems(
            OnExit(AppState::SongLoading),
            (stop_nowloading, despawn_stage::<LoadingEntity>).chain(),
        )
        .add_systems(OnExit(AppState::Performance), cleanup_bga_overlays)
        .add_systems(
            Update,
            (
                watch_cancel_key,
                poll_chart_parse,
                wait_for_audio.after(poll_chart_parse),
                update_status_text.after(wait_for_audio),
                advance_when_loaded.after(update_status_text),
            )
                .run_if(in_state(AppState::SongLoading)),
        );
}

/// On leaving Performance: despawn any BGA image-layer placeholder overlays and
/// reset the player so state does not bleed into Result/SongSelect. Movie decode
/// remains deferred to M7.1.
fn cleanup_bga_overlays(
    mut commands: Commands,
    overlays: Query<Entity, With<BgaLayerOverlay>>,
    mut bga_player: ResMut<BgaPlayer>,
) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
    bga_player.reset();
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
    mut progress: ResMut<LoadingProgress>,
    mut cancel: ResMut<CancelRequested>,
) {
    progress.0 = 0.0;
    cancel.0 = false;
    bank.clear();
    if let Some(path) = selected.0.as_ref() {
        *phase = LoadPhase::Parsing;
        // Header parse (synchronous header peek) is what BocuD does in
        // OnActivate. We don't read SOUND_NOWLOADING until the full parse is
        // done — fall back to "Loading…" if the chart doesn't ship one.
        let path_clone = path.clone();
        let pool = AsyncComputeTaskPool::get();
        task.0 =
            Some(pool.spawn(async move {
                dtx_assets::load_dtx(&path_clone).map_err(|e| e.to_string())
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
    mut guitar_chart: ResMut<GuitarActiveChart>,
    mut bga_player: ResMut<BgaPlayer>,
    mut bank: ResMut<dtx_audio::ChartSoundBank>,
    mut required: ResMut<RequiredAudio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut ghost: ResMut<GhostLag>,
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
        Ok(chart) => {
            if cancel.0 {
                *phase = LoadPhase::Failed;
                return;
            }
            info!(
                "Parsed DTX ({} chips, BPM {:?}, dlevel {:?})",
                chart.chips.len(),
                chart.metadata.bpm,
                chart.metadata.dlevel
            );
            drums_chart.chart = chart.clone();
            drums_chart.source_path = path.clone();
            // M6b: also load into the guitar crate so Guitar mode is playable.
            guitar_chart.chart = chart.clone();
            guitar_chart.source_path = path.clone();
            // M7: populate BGA events for the player.
            let events = dtx_core::bga::bga_events(&chart);
            bga_player.reset();
            bga_player.event_count = events.len();
            commands.insert_resource(ActiveChartRes {
                bpm: chart.metadata.bpm.unwrap_or(120.0),
                events,
            });
            // Tiered preload (mirrors dtxpt setup.rs:72-90): request the
            // immediate note-referenced WAVs and wait on them; fire-and-forget
            // the deferred BGM/SE stems so they decode in the background while
            // gameplay begins.
            use gameplay_drums::sound_bank;
            let immediate = sound_bank::collect_immediate_wav_slots(&chart);
            let deferred = sound_bank::collect_deferred_wav_slots(&chart);
            required.0 =
                sound_bank::preload_slots(&drums_chart, &asset_server, &mut bank, &immediate);
            let deferred_handles =
                sound_bank::preload_slots(&drums_chart, &asset_server, &mut bank, &deferred);
            info!(
                "SongLoading: waiting on {} immediate WAVs, {} deferred in background",
                required.0.len(),
                deferred_handles.len()
            );
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
            error!("Failed to load DTX: {e}");
            *phase = LoadPhase::Failed;
        }
    }
}

/// Wait for every requested WAV handle to reach a terminal load state (Loaded
/// or Failed). Progress reflects the real decoded fraction.
fn wait_for_audio(
    mut phase: ResMut<LoadPhase>,
    required: Res<RequiredAudio>,
    asset_server: Res<AssetServer>,
    mut progress: ResMut<LoadingProgress>,
) {
    if *phase != LoadPhase::LoadingAudio {
        return;
    }
    let total = required.0.len();
    if total == 0 {
        progress.0 = 1.0;
        *phase = LoadPhase::Ready;
        return;
    }
    let done = required
        .0
        .iter()
        .filter(|handle| match asset_server.get_load_state(handle.id()) {
            Some(state) => state.is_loaded() || state.is_failed(),
            // No load state yet means the handle hasn't been registered; treat
            // as pending.
            None => false,
        })
        .count();
    progress.0 = done as f32 / total as f32;
    if done >= total {
        *phase = LoadPhase::Ready;
    }
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
            crate::song_select::SongFolderView::difficulty_label(selection.difficulty).to_string(),
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
                        TextColor(t.text_secondary),
                    ));
                    col.spawn((
                        Text::new(title),
                        Theme::font(34.0),
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
                            TextColor(t.text_secondary),
                        ));
                        meta.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(t.difficulty_color(2)),
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
                        TextColor(t.text_secondary),
                        LoadingStatusText,
                    ));
                });
            });
        });
}

/// Watch for Esc during load. On press, mark the load as cancelled. The next
/// `poll_chart_parse` tick will see the flag and fail-fast; `advance_when_loaded`
/// will then route back to SongSelect.
fn watch_cancel_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut cancel: ResMut<CancelRequested>,
    phase: Res<LoadPhase>,
) {
    if cancel.0 {
        return;
    }
    if matches!(
        *phase,
        LoadPhase::Idle | LoadPhase::Ready | LoadPhase::Failed
    ) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        info!("SongLoading: Esc pressed — cancelling load");
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
    let status = match *phase {
        LoadPhase::Idle => String::new(),
        LoadPhase::Parsing => "parsing chart…".to_string(),
        LoadPhase::LoadingAudio => format!(
            "loading audio chips… {}/{}",
            ((progress.0 * total as f32).round() as usize).min(total),
            total
        ),
        LoadPhase::Ready => "ready".to_string(),
        LoadPhase::Failed => "failed — returning to song select".to_string(),
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
) {
    if gate.0 {
        return;
    }
    if cancel.0 && !matches!(*phase, LoadPhase::Ready | LoadPhase::Failed) {
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
}
