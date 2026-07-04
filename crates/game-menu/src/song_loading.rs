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

use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use bevy_kira_audio::prelude::{Audio, AudioInstance, AudioSource as KiraAudioSource};
use dtx_audio::BgmHandle;
use dtx_bga::{ActiveChartRes, BgaLayerOverlay, BgaPlayer};
use dtx_core::{resolve_bgm_path, Chart};
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};
use gameplay_drums::resources::ActiveChart as DrumsActiveChart;
use gameplay_guitar::resources::ActiveChart as GuitarActiveChart;

use crate::song_select::SelectedSong;

#[derive(Component)]
pub struct LoadingEntity;

#[derive(Component)]
struct JacketImage;

/// Display the DTX `dlevel` (drums) value as a 2-digit string.
#[derive(Component)]
struct LevelText;

/// Display the chart's difficulty label (or fall back to genre / "—" ).
#[derive(Component)]
struct DifficultyText;

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
) {
    progress.0 = 0.0;
    bank.clear();
    if let Some(path) = selected.0.as_ref() {
        *phase = LoadPhase::Parsing;
        // Header parse (synchronous header peek) is what BocuD does in
        // OnActivate. We don't read SOUND_NOWLOADING until the full parse is
        // done — fall back to "Loading…" if the chart doesn't ship one.
        let path_clone = path.clone();
        let pool = AsyncComputeTaskPool::get();
        task.0 = Some(pool.spawn(async move {
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
    theme: Res<ThemeResource>,
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
            required.0 = sound_bank::preload_slots(&drums_chart, &asset_server, &mut bank, &immediate);
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
                    dtx_scoring::score_ini::read_ghost_lag(
                        dtx_scoring::score_ini::ghost_path(p, "dr", "perfect"),
                    )
                })
                .unwrap_or_default();
            if !ghost.drums.is_empty() {
                info!("SongLoading: loaded {} ghost lag samples", ghost.drums.len());
            }
            // PREIMAGE jacket — async-load the texture and spawn the UI entity.
            spawn_jacket(&mut commands, &theme.0, &chart, path.as_deref(), &asset_server);
            // Level + difficulty text overlays (BocuD DrawLoadingScreenUI parity).
            spawn_level_ui(&mut commands, &theme.0, &chart);
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

fn spawn_loading(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands.spawn((
        LoadingEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(t.bg_bottom),
        children![
            (
                Text::new("Loading"),
                Theme::title_font(),
                TextColor(t.text_primary),
            ),
            (
                Text::new(""),
                Theme::body_font(),
                TextColor(t.text_secondary),
                LoadingStatusText,
            ),
        ],
    ));
}

/// Spawn the PREIMAGE jacket overlay on top of the loading screen. The
/// texture itself loads async via `AssetServer::load`; once it finishes the
/// `Image` node displays it. If the chart has no PREIMAGE or the file is
/// missing we silently skip — no placeholder per ADR-0014.
fn spawn_jacket(
    commands: &mut Commands,
    theme: &dtx_ui::theme::Theme,
    chart: &Chart,
    source_path: Option<&std::path::Path>,
    asset_server: &AssetServer,
) {
    let Some(preimage_name) = chart.metadata.preimage_filename.as_deref() else {
        return;
    };
    let Some(parent_dir) = source_path.and_then(|p| p.parent()) else {
        return;
    };
    let img_path = parent_dir.join(preimage_name);
    if !img_path.is_file() {
        return;
    }
    let handle: Handle<Image> = asset_server.load(img_path.to_string_lossy().to_string());
    commands.spawn((
        LoadingEntity,
        JacketImage,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(384.0),
            height: Val::Px(384.0),
            left: Val::Px(100.0),
            top: Val::Px(85.0),
            ..default()
        },
        ImageNode::new(handle),
    ));
    let _ = theme;
}

/// Render the DTX level number + difficulty label as text overlays on the
/// loading screen. BocuD draws these with skin sprites; we use plain text
/// because there's no skin system yet (M14+).
fn spawn_level_ui(
    commands: &mut Commands,
    theme: &dtx_ui::theme::Theme,
    chart: &Chart,
) {
    let level = chart
        .metadata
        .dlevel
        .map(|n| format!("{n:02}"))
        .unwrap_or_else(|| "--".to_string());
    let difficulty = chart
        .metadata
        .genre
        .clone()
        .or_else(|| chart.metadata.dlevel.map(|_| "BASIC".into()))
        .unwrap_or_else(|| "—".into());

    commands.spawn((
        LoadingEntity,
        LevelText,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(200.0),
            top: Val::Px(160.0),
            ..default()
        },
        Text::new(level),
        TextFont {
            font_size: bevy::prelude::FontSize::Px(96.0),
            ..default()
        },
        TextColor(theme.text_primary),
    ));
    commands.spawn((
        LoadingEntity,
        DifficultyText,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(200.0),
            top: Val::Px(280.0),
            ..default()
        },
        Text::new(difficulty),
        TextFont {
            font_size: bevy::prelude::FontSize::Px(28.0),
            ..default()
        },
        TextColor(theme.text_secondary),
    ));
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
    if matches!(*phase, LoadPhase::Idle | LoadPhase::Ready | LoadPhase::Failed) {
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
    // Reuse the BGM slot for the nowloading jingle so `stop_bgm` on exit
    // cleans it up. Volume 0.6 — soft, doesn't fight the BGMLoadingSound.
    let _ = dtx_audio::play_bgm(audio, asset_server, bgm, instances, &path.to_string_lossy());
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
fn primary_bgm_hint(chart: &Chart, source_path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    let p = source_path?;
    resolve_bgm_path(p, chart)
}

#[derive(Component)]
struct LoadingStatusText;

fn update_status_text(
    phase: Res<LoadPhase>,
    progress: Res<LoadingProgress>,
    mut status_query: Query<&mut Text, With<LoadingStatusText>>,
) {
    let status = match *phase {
        LoadPhase::Idle => String::new(),
        LoadPhase::Parsing => "Parsing chart...".to_string(),
        LoadPhase::LoadingAudio => format!("Loading audio... {}%", (progress.0 * 100.0) as u32),
        LoadPhase::Ready => "Ready".to_string(),
        LoadPhase::Failed => "Failed".to_string(),
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
