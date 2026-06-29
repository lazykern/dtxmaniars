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

use bevy::prelude::*;
use dtx_assets::DtxCache;
use dtx_bga::{ActiveChartRes, BgaLayerOverlay, BgaPlayer};
use dtx_ui::{Theme, ThemeResource};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};
use gameplay_drums::resources::ActiveChart as DrumsActiveChart;
use gameplay_guitar::resources::ActiveChart as GuitarActiveChart;

use crate::song_select::SelectedSong;

#[derive(Component)]
pub struct LoadingEntity;

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LoadingProgress(pub f32);

/// Tracks which phase the loader is in.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
enum LoadPhase {
    #[default]
    Idle,
    Parsing,
    Ready,
    Failed,
}

#[derive(Resource, Default)]
struct LoadingAdvanceGate(bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<LoadingProgress>()
        .init_resource::<LoadPhase>()
        .init_resource::<LoadingAdvanceGate>()
        .add_systems(
            OnEnter(AppState::SongLoading),
            (reset_advance_gate, start_load, spawn_loading).chain(),
        )
        .add_systems(
            OnExit(AppState::SongLoading),
            despawn_stage::<LoadingEntity>,
        )
        .add_systems(OnExit(AppState::Performance), cleanup_bga_overlays)
        .add_systems(
            Update,
            (tick_loading_progress, advance_when_loaded).run_if(in_state(AppState::SongLoading)),
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

fn start_load(
    selected: Res<SelectedSong>,
    mut phase: ResMut<LoadPhase>,
    mut cache: ResMut<DtxCache>,
    mut drums_chart: ResMut<DrumsActiveChart>,
    mut guitar_chart: ResMut<GuitarActiveChart>,
    mut bga_player: ResMut<BgaPlayer>,
    mut commands: Commands,
) {
    *phase = LoadPhase::Parsing;
    if let Some(path) = selected.0.as_ref() {
        match cache.get_or_load(path) {
            Ok(chart) => {
                info!(
                    "Loaded DTX: {} ({} chips, BPM {:?})",
                    path.display(),
                    chart.chips.len(),
                    chart.metadata.bpm
                );
                drums_chart.chart = chart.clone();
                drums_chart.source_path = Some(path.clone());
                // M6b: also load into the guitar crate so Guitar mode is playable.
                guitar_chart.chart = chart.clone();
                guitar_chart.source_path = Some(path.clone());
                // M7: populate BGA events for the player.
                let events = dtx_core::bga::bga_events(chart);
                bga_player.reset();
                bga_player.event_count = events.len();
                let bga_res = ActiveChartRes {
                    bpm: chart.metadata.bpm.unwrap_or(120.0),
                    events,
                };
                commands.insert_resource(bga_res);
                *phase = LoadPhase::Ready;
            }
            Err(e) => {
                error!("Failed to load DTX {}: {}", path.display(), e);
                *phase = LoadPhase::Failed;
            }
        }
    } else {
        warn!("SongLoading entered with no SelectedSong; using empty chart");
        *phase = LoadPhase::Ready;
    }
}

fn reset_advance_gate(mut gate: ResMut<LoadingAdvanceGate>) {
    gate.0 = false;
}

fn spawn_loading(
    mut commands: Commands,
    mut progress: ResMut<LoadingProgress>,
    theme: Res<ThemeResource>,
) {
    progress.0 = 0.0;
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

#[derive(Component)]
struct LoadingStatusText;

fn tick_loading_progress(
    time: Res<Time>,
    mut progress: ResMut<LoadingProgress>,
    phase: Res<LoadPhase>,
    mut status_query: Query<&mut Text, With<LoadingStatusText>>,
) {
    // M4 stub: simulate 1.0s linear load. Real impl waits for asset handles (M5).
    // If we already hit Ready/Failed phase, snap to 1.0.
    progress.0 = match *phase {
        LoadPhase::Ready | LoadPhase::Failed => 1.0,
        LoadPhase::Parsing | LoadPhase::Idle => {
            (progress.0 + time.delta().as_secs_f32() / 1.0).min(1.0)
        }
    };

    let status = match *phase {
        LoadPhase::Idle => "",
        LoadPhase::Parsing => "Parsing DTX...",
        LoadPhase::Ready => "Ready",
        LoadPhase::Failed => "Failed",
    };
    for mut text in &mut status_query {
        *text = Text::new(status.to_string());
    }
}

fn advance_when_loaded(
    progress: Res<LoadingProgress>,
    phase: Res<LoadPhase>,
    mut requests: MessageWriter<TransitionRequest>,
    mut gate: ResMut<LoadingAdvanceGate>,
) {
    if gate.0 {
        return;
    }
    if progress.0 >= 1.0 {
        if *phase == LoadPhase::Failed {
            warn!("SongLoading: load failed, returning to SongSelect");
            request_transition(&mut requests, AppState::SongSelect);
        } else {
            request_transition(&mut requests, AppState::Performance);
        }
        gate.0 = true;
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
