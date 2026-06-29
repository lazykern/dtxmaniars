//! `CStagePerfDrumsScreen` orchestrator — port of `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs` (3671 LoC).
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! ## Role
//!
//! The C# class is the central orchestrator for drums performance. It owns
//! the lifecycle (`OnActivate`/`OnDeactivate`/`OnUpdate`/`OnDraw`), composes
//! all 11 sub-acts via `listChildActivities`, and handles cross-cutting
//! concerns (stage transitions, gauge-driven danger, end-of-stage).
//!
//! In Rust we model this as a Bevy plugin that:
//! - registers all sub-act resources (already done by sibling plugins)
//! - adds cross-cutting systems (end-of-stage transition, gauge-drain loop)
//! - owns the stage-level state machine
//!
//! ## Sub-acts wired
//!
//! (From CStagePerfDrumsScreen.cs:11-30)
//!
//! | C# sub-act | Rust crate | File |
//! |---|---|---|
//! | `CActPerfDrumsPad`         | gameplay-drums | `drums_perf.rs` |
//! | `CActPerfDrumsComboDGB`    | gameplay-drums | `hud.rs` (combo display) |
//! | `CActPerfDrumsDanger`      | gameplay-drums | `drums_perf.rs` |
//! | `CActPerfDrumsGauge`       | gameplay-drums | `hud.rs` (gauge) |
//! | `CActPerfSkillMeter`       | gameplay-drums | (planned M14) |
//! | `CActPerfDrumsJudgementString` | gameplay-drums | `hud.rs` |
//! | `CActPerfDrumsLaneFlushD`  | gameplay-drums | `perf_sub_acts_3.rs` |
//! | `CActPerfDrumsScore`       | gameplay-drums | `hud.rs` + `score.rs` |
//! | `CActPerfDrumsStatusPanel` | gameplay-drums | `hud.rs` |
//! | `CActPerfScrollSpeed`      | gameplay-drums | (planned M14) |
//! | `CActPerfVideo`            | dtx-bga | `lib.rs` |
//! | `CActPerfBGA`              | dtx-bga | `lib.rs` |
//! | `CActPerfStageFailure`     | gameplay-drums | (planned M14) |
//! | `CActPerformanceInformation` | gameplay-drums | (planned M14) |
//! | `CActPerfDrumsFillingEffect` | gameplay-drums | `drums_perf.rs` |
//! | `CActPerfProgressBar`      | gameplay-drums | (planned M14) |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs:11-30`

#![allow(dead_code)] // Re-exports for future cross-cutting systems.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_audio::BgmHandle;
use dtx_core::chart::Chart;
use game_shell::{request_transition, AppState, TransitionRequest};

use crate::components::LastJudgment;
use crate::drums_perf::{DrumsDangerState, DrumsFillingEffect, DrumsPadState};
use crate::judge::{BpmChangeList, JudgedChips};
use crate::resources::{
    ActiveChart, Combo, DrumGameplaySettings, GameStartMs, GameplayClock, JudgmentCounts, Score,
};

/// Marker component for the drums performance stage root entity.
///
/// Mirrors `CStagePerfDrumsScreen` constructor (BocuD CStagePerfDrumsScreen.cs:11-12)
/// which sets `eStageID = EStage.Performance_6`.
#[derive(Component, Debug, Clone, Copy)]
pub struct DrumsStageRoot;

/// End-of-stage state for drums (BocuD CStagePerfDrumsScreen OnUpdate logic).
///
/// When the chart's last chip has been judged and `now_ms >= end_ms`, the
/// stage transitions to `AppState::Result`. The transition is rate-limited
/// to once per stage entry.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DrumsStageCompletion {
    /// Chart end time in ms (BocuD CStagePerfCommonScreen.cs uses
    /// `nEndTimeMs` for the Discord rich presence + end detection).
    pub chart_end_ms: i64,
    /// Whether the end transition has been requested.
    pub end_requested: bool,
    /// Whether gauge has reached zero (failed) — triggers failure path
    /// (BocuD CActPerfStageFailure.cs).
    pub gauge_failed: bool,
}

/// Plugin assembly. Registers the stage-root marker + the completion
/// resource + the end-of-stage detection system.
///
/// All sub-act plugins (`hud`, `scroll`, `judge`, `score`, etc.) are
/// registered by the parent `gameplay_drums::plugin` — this orchestrator
/// only adds stage-level coordination.
pub fn plugin(app: &mut App) {
    app.init_resource::<DrumsStageCompletion>()
        .init_resource::<DrumsPadState>()
        .init_resource::<DrumsDangerState>()
        .init_resource::<DrumsFillingEffect>()
        .add_systems(
            OnEnter(AppState::Performance),
            (
                reset_drum_runtime,
                on_enter_performance,
                crate::sound_bank::preload_chart_sounds_on_enter,
                start_bgm_on_enter,
                spawn_stage_root,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppState::Performance),
            (despawn_stage_root, on_exit_performance, reset_drum_runtime).chain(),
        )
        .add_systems(
            Update,
            detect_end_of_stage.run_if(in_state(AppState::Performance)),
        );
}

/// Reset drum audio/hit-sound runtime state on enter/exit.
fn reset_drum_runtime(
    mut empty_templates: ResMut<crate::resources::CurrentEmptyHitTemplates>,
    mut active_sounds: ResMut<crate::resources::ActiveDrumSounds>,
    mut played_se: ResMut<crate::se_scheduler::PlayedSeChips>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    mut sound_bank: ResMut<dtx_audio::ChartSoundBank>,
) {
    empty_templates.reset();
    active_sounds.reset();
    played_se.0.clear();
    polyphony.reset();
    sound_bank.clear();
}

/// On enter: capture chart end time, reset gameplay state, build BPM list.
pub fn on_enter_performance(
    chart: Res<ActiveChart>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut score: ResMut<Score>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
    mut judged: ResMut<JudgedChips>,
    mut last: ResMut<LastJudgment>,
    mut start_ms: ResMut<GameStartMs>,
    mut bpm_changes: ResMut<BpmChangeList>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut primary_bgm: ResMut<crate::bgm_scheduler::PrimaryBgmChip>,
    mut gameplay_clock: ResMut<GameplayClock>,
    mut drum_settings: ResMut<DrumGameplaySettings>,
) {
    let has_bgm = crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart)
        || chart
            .source_path
            .as_ref()
            .and_then(|path| dtx_core::resolve_bgm_path(path, &chart.chart))
            .is_some();
    if has_bgm {
        gameplay_clock.start_audio_required();
    } else {
        gameplay_clock.start_wall_clock();
    }
    drum_settings.rebuild_from_chart(&chart.chart);
    *bpm_changes = BpmChangeList::from_chart(&chart.chart);
    completion.chart_end_ms = chart_end_ms_real(&chart.chart, &bpm_changes);
    completion.end_requested = false;
    completion.gauge_failed = false;

    score.0 = 0;
    combo.current = 0;
    combo.max = 0;
    counts.reset();
    judged.0.clear();
    last.0 = None;
    start_ms.0 = 0;
    played_bgm.0.clear();
    primary_bgm.0 = crate::bgm_scheduler::find_primary_bgm_chip(&chart.chart, &bpm_changes);
}

/// On enter: bootstrap BGM (chip path or heuristic fallback).
fn start_bgm_on_enter(
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    primary: Res<crate::bgm_scheduler::PrimaryBgmChip>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    settings: Res<crate::resources::DrumAudioSettings>,
) {
    if crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart) {
        crate::bgm_scheduler::bootstrap_primary_bgm_chip(
            &chart,
            &bpm_changes,
            &primary,
            &mut played_bgm,
            &audio,
            &asset_server,
            &mut bgm,
            &mut instances,
            &sound_bank,
            settings.master_volume,
        );
        return;
    }
    let Some(source_path) = chart.source_path.as_ref() else {
        warn!("Performance: no source_path on ActiveChart, cannot start BGM");
        return;
    };
    if let Some(bgm_path) = dtx_core::resolve_bgm_path(source_path, &chart.chart) {
        let path_str = bgm_path.to_string_lossy().to_string();
        info!("Performance: starting BGM (no chips) from {path_str}");
        dtx_audio::play_bgm(&audio, &asset_server, &mut bgm, &mut instances, &path_str);
    } else {
        warn!(
            "Performance: no BGM file found near {}",
            source_path.display()
        );
    }
}

/// On exit: clear completion state and stop BGM.
pub fn on_exit_performance(
    audio: Res<Audio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut gameplay_clock: ResMut<GameplayClock>,
) {
    dtx_audio::stop_bgm(&audio, &mut bgm, &mut instances);
    played_bgm.0.clear();
    gameplay_clock.reset();
    completion.end_requested = false;
    completion.gauge_failed = false;
}

fn spawn_stage_root(mut commands: Commands) {
    commands.spawn(DrumsStageRoot);
}

fn despawn_stage_root(mut commands: Commands, query: Query<Entity, With<DrumsStageRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Compute the last drum chip's `target_ms` using BPM-change-aware timing.
/// Returns 0 if the chart is empty. Adds a 2000ms buffer for BGM tail.
pub fn chart_end_ms_real(chart: &Chart, bpm_changes: &BpmChangeList) -> i64 {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    chart
        .drum_chips()
        .map(|c| crate::judge::chip_target_ms(c, base_bpm, &bpm_changes.changes))
        .max()
        .unwrap_or(0)
        .saturating_add(2000)
}

/// Legacy estimate for tests that don't have a BpmChangeList.
#[cfg(test)]
pub fn chart_end_ms(chart: &Chart) -> i64 {
    chart
        .chips
        .iter()
        .map(|c| (c.measure as i64) * 2000 + 1000)
        .max()
        .unwrap_or(0)
}

/// Detect end-of-stage: chart fully scrolled AND no more chips to process.
/// Transition to `AppState::Result`.
///
/// Mirrors the chart-end check in CStagePerfCommonScreen.cs (Presence
/// property) + the CStage return logic in CStagePerfDrumsScreen.
pub fn detect_end_of_stage(
    clock: Res<GameplayClock>,
    mut completion: ResMut<DrumsStageCompletion>,
    _chart: Res<ActiveChart>,
    _score: Res<Score>,
    _combo: Res<Combo>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if completion.end_requested {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let now_ms = clock.current_ms;
    let past_chart_end = now_ms >= completion.chart_end_ms;
    let all_chips_spawned = completion.chart_end_ms > 0;
    if past_chart_end && all_chips_spawned {
        info!(
            "DrumsStage: end of chart at now_ms={now_ms}, chart_end_ms={}",
            completion.chart_end_ms
        );
        completion.end_requested = true;
        request_transition(&mut requests, AppState::Result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::channel::EChannel;
    use dtx_core::chart::{Chart, Chip, Metadata};

    fn chart_with_n_chips(n: u32) -> Chart {
        let chips: Vec<Chip> = (0..n)
            .map(|i| Chip::new(i, EChannel::BassDrum, 1.0))
            .collect();
        Chart {
            metadata: Metadata::default(),
            chips,
            ..Default::default()
        }
    }

    #[test]
    fn chart_end_ms_empty_chart() {
        let c = Chart::default();
        assert_eq!(chart_end_ms(&c), 0);
    }

    #[test]
    fn chart_end_ms_single_chip() {
        let c = chart_with_n_chips(1);
        // Chip at measure=0 → 0*2000 + 1000 = 1000
        assert_eq!(chart_end_ms(&c), 1000);
    }

    #[test]
    fn chart_end_ms_picks_last_chip_measure() {
        let c = chart_with_n_chips(5);
        // measure=4 → 4*2000 + 1000 = 9000
        assert_eq!(chart_end_ms(&c), 4 * 2000 + 1000);
    }

    #[test]
    fn drums_stage_completion_default() {
        let c = DrumsStageCompletion::default();
        assert_eq!(c.chart_end_ms, 0);
        assert!(!c.end_requested);
        assert!(!c.gauge_failed);
    }

    #[test]
    fn on_enter_captures_chart_end_ms() {
        let mut app = App::new();
        app.init_resource::<DrumsStageCompletion>()
            .init_resource::<ActiveChart>()
            .init_resource::<Score>()
            .init_resource::<Combo>()
            .init_resource::<JudgmentCounts>()
            .init_resource::<JudgedChips>()
            .init_resource::<LastJudgment>()
            .init_resource::<GameStartMs>()
            .init_resource::<BpmChangeList>()
            .init_resource::<GameplayClock>()
            .init_resource::<crate::resources::CurrentEmptyHitTemplates>()
            .init_resource::<crate::resources::ActiveDrumSounds>()
            .init_resource::<crate::se_scheduler::PlayedSeChips>()
            .init_resource::<crate::bgm_scheduler::PlayedBgmChips>()
            .init_resource::<crate::bgm_scheduler::PrimaryBgmChip>()
            .init_resource::<DrumGameplaySettings>();
        let chart = chart_with_n_chips(3);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.add_systems(Update, on_enter_performance);
        app.update();
        let completion = app.world().resource::<DrumsStageCompletion>();
        assert!(completion.chart_end_ms > 0);
    }

    #[test]
    fn on_exit_clears_end_requested() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy_kira_audio::AudioPlugin,
        ))
        .init_resource::<DrumsStageCompletion>()
        .init_resource::<BgmHandle>()
        .init_resource::<GameplayClock>()
        .init_resource::<crate::resources::CurrentEmptyHitTemplates>()
        .init_resource::<crate::resources::ActiveDrumSounds>()
        .init_resource::<crate::se_scheduler::PlayedSeChips>()
        .init_resource::<crate::bgm_scheduler::PlayedBgmChips>();
        app.world_mut()
            .resource_mut::<DrumsStageCompletion>()
            .end_requested = true;
        app.add_systems(Update, on_exit_performance);
        app.update();
        let completion = app.world().resource::<DrumsStageCompletion>();
        assert!(!completion.end_requested);
    }
}
