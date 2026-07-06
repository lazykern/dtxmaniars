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
use dtx_audio::{BgmHandle, DrumPolyphony};
use dtx_core::chart::Chart;
use game_shell::{request_transition, AppState, TransitionRequest};

use crate::components::LastJudgment;
use crate::derived::ChartDerived;
use crate::drums_perf::{DrumsDangerState, DrumsFillingEffect, DrumsPadState};
use crate::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use crate::resources::{
    ActiveChart, ActiveDrumSounds, BgmAdjustState, Combo, DrumGameplaySettings, DrumScoring,
    FastSlowCount, GameStartMs, GameplayClock, JudgmentCounts, Score, SkillValue,
};
use dtx_timing::math::ChartTiming;

/// Marker set wrapping the drums `OnEnter(Performance)` chain.
///
/// Allows other plugins to order systems relative to the whole enter sequence
/// via `.before(DrumsEnterSet)` / `.after(DrumsEnterSet)`.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DrumsEnterSet;

/// Per-system labels for the drums `OnEnter(Performance)` chain.
///
/// Used because `IntoScheduleConfigs::chain` for a single 5-tuple of generic
/// systems blows past Rust's trait-solver depth (the type parameters of
/// `on_enter_performance` alone reach 17 HRTBs). Splitting each system into
/// its own `add_systems` call and ordering the enum variants via
/// `configure_sets(...).chain()` sidesteps that limit.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum DrumsEnterStep {
    ResetRuntime,
    OnEnter,
    PreloadSounds,
    StartBgm,
    SpawnStage,
}

/// Marker component for the drums performance stage root entity.
///
/// Mirrors `CStagePerfDrumsScreen` constructor (BocuD CStagePerfDrumsScreen.cs:11-12)
/// which sets `eStageID = EStage.Performance_6`.
#[derive(Component, Debug, Clone, Copy)]
pub struct DrumsStageRoot;

/// Marker set wrapping the drums `OnExit(Performance)` chain.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DrumsExitSet;

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
                reset_drum_runtime_keep_bank,
                enter_reset_run_state,
                enter_derive_from_chart,
                enter_seed_bgm_state,
                // Safety net: re-request any WAV slots the loading screen missed.
                // Idempotent — already-loaded handles are reused, not re-decoded.
                crate::sound_bank::preload_chart_sounds_on_enter,
                start_bgm_on_enter,
                spawn_stage_root,
            )
                .chain()
                .in_set(DrumsEnterSet),
        )
        .add_systems(
            OnExit(AppState::Performance),
            (despawn_stage_root, on_exit_performance, reset_drum_runtime)
                .chain()
                .in_set(DrumsExitSet),
        )
        .add_systems(
            Update,
            detect_end_of_stage.run_if(in_state(AppState::Performance)),
        );
}

/// Reset drum audio/hit-sound runtime state, including the chart sound bank.
/// Used on stage exit (the next chart's loading screen owns bank population).
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

/// Reset drum runtime on stage *enter* WITHOUT clearing the sound bank — the
/// SongLoading screen preloads the bank for this chart, so clearing here would
/// force a redundant re-decode and drop the wait-on-handles guarantee.
fn reset_drum_runtime_keep_bank(
    mut empty_templates: ResMut<crate::resources::CurrentEmptyHitTemplates>,
    mut active_sounds: ResMut<crate::resources::ActiveDrumSounds>,
    mut played_se: ResMut<crate::se_scheduler::PlayedSeChips>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
) {
    empty_templates.reset();
    active_sounds.reset();
    played_se.0.clear();
    polyphony.reset();
}

/// On enter: reset per-run state (score, combo, counts, judged, fast/slow,
/// drum settings). Split out from the chart-derive step to keep param count
/// below Bevy 0.19's HRTB solver ceiling (~16 params per system).
pub fn enter_reset_run_state(
    chart: Res<ActiveChart>,
    mut score: ResMut<Score>,
    mut scoring: ResMut<DrumScoring>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
    mut judged: ResMut<JudgedChips>,
    mut last: ResMut<LastJudgment>,
    mut fast_slow: ResMut<FastSlowCount>,
    mut drum_settings: ResMut<DrumGameplaySettings>,
) {
    let drum_chip_count = chart.chart.drum_chips().count();
    score.0 = 0;
    scoring.reset(drum_chip_count as u32);
    combo.current = 0;
    combo.max = 0;
    counts.reset();
    judged.0.clear();
    last.0 = None;
    fast_slow.fast = 0;
    fast_slow.slow = 0;
    drum_settings.rebuild_from_chart(&chart.chart);
}

/// On enter: derive chart-level state (completion, bpm list, derived cache,
/// gameplay-clock mode). Kept slim so the chain stays within trait-solver
/// limits when listed in a tuple.
pub fn enter_derive_from_chart(
    chart: Res<ActiveChart>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut bpm_changes: ResMut<BpmChangeList>,
    mut bar_changes: ResMut<BarLengthChangeList>,
    mut gameplay_clock: ResMut<GameplayClock>,
    mut derived: ResMut<ChartDerived>,
    mut skill: ResMut<SkillValue>,
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
    let drum_chip_count = chart.chart.drum_chips().count();
    *bpm_changes = BpmChangeList::from_chart(&chart.chart);
    *bar_changes = BarLengthChangeList::from_chart(&chart.chart);
    completion.chart_end_ms = chart_end_ms_real(&chart.chart, &bpm_changes, &bar_changes);
    completion.end_requested = false;
    completion.gauge_failed = false;
    crate::derived::compute_from_chart(
        &mut derived,
        &chart.chart,
        &bpm_changes,
        &bar_changes,
        drum_chip_count as u32,
    );
    skill.current = 0.0;
    skill.max = derived.max_skill;
}

/// On enter: seed BGM playback state (played list, primary chip, recovery,
/// start offset). Reads `bpm_changes` set by `enter_derive_from_chart`.
pub fn enter_seed_bgm_state(
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut primary_bgm: ResMut<crate::bgm_scheduler::PrimaryBgmChip>,
    mut bgm_recovery: ResMut<crate::bgm_scheduler::BgmRecoveryState>,
    mut start_ms: ResMut<GameStartMs>,
) {
    played_bgm.0.clear();
    *bgm_recovery = crate::bgm_scheduler::BgmRecoveryState::default();
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    primary_bgm.0 = crate::bgm_scheduler::find_primary_bgm_chip(&chart.chart, timing);
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    start_ms.0 = primary_bgm
        .0
        .and_then(|idx| chart.chart.chips.get(idx))
        .map(|chip| {
            crate::judge::auto_chip_target_ms(chip, base_bpm, timing, bgm_adjust.total_ms())
        })
        .unwrap_or(0);
    info!(
        "Performance enter: {} total chips, {} drum-lane chips, bgm_chart_ms={}",
        chart.chart.chips.len(),
        chart.chart.drum_chips().count(),
        start_ms.0,
    );
}

/// On enter: start fallback BGM only for charts without BGM chips.
/// BGM-channel charts are chart-timed by `bgm_scheduler`, so pre-BGM count-in
/// chips can play before the primary BGM starts.
fn start_bgm_on_enter(
    chart: Res<ActiveChart>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart) {
        return;
    }
    let Some(source_path) = chart.source_path.as_ref() else {
        warn!("Performance: no source_path on ActiveChart, cannot start BGM");
        return;
    };
    if let Some(bgm_path) = dtx_core::resolve_bgm_path(source_path, &chart.chart) {
        let path_str = bgm_path.to_string_lossy().to_string();
        info!("Performance: starting BGM (no chips) from {path_str}");
        dtx_audio::play_bgm(
            &audio,
            &asset_server,
            &mut bgm,
            &mut instances,
            &path_str,
            dtx_ui::SCREEN_TRANSITION_MS as u32,
        );
    } else {
        warn!(
            "Performance: no BGM file found near {}",
            source_path.display()
        );
    }
}

/// On exit: stop all chart audio and clear completion state.
pub fn on_exit_performance(
    audio: Res<Audio>,
    active_sounds: Res<ActiveDrumSounds>,
    polyphony: Res<DrumPolyphony>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut played_bgm: ResMut<crate::bgm_scheduler::PlayedBgmChips>,
    mut gameplay_clock: ResMut<GameplayClock>,
) {
    active_sounds.stop_all(&mut instances);
    dtx_audio::stop_polyphony(&mut instances, &polyphony);
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

/// Compute the last drum chip's `target_ms` using BPM-change- and
/// bar-length-aware timing. Returns 0 if the chart is empty. Adds a 2000ms
/// buffer for BGM tail.
pub fn chart_end_ms_real(
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    bar_changes: &BarLengthChangeList,
) -> i64 {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    chart
        .drum_chips()
        .map(|c| crate::judge::chip_target_ms(c, base_bpm, timing))
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
    mut score: ResMut<Score>,
    mut scoring: ResMut<DrumScoring>,
    counts: Res<JudgmentCounts>,
    _combo: Res<Combo>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if completion.end_requested {
        return;
    }
    // An armed A/B loop owns the stage end: the loop watcher seeks back
    // before the chart end is ever reached "for real".
    if practice.as_ref().is_some_and(|s| s.loop_region.is_some()) {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let now_ms = clock.current_ms;
    let past_chart_end = now_ms >= completion.chart_end_ms;
    let all_chips_spawned = completion.chart_end_ms > 0;
    if past_chart_end && all_chips_spawned {
        // Apply DTXManiaNX end-of-song bonus (FC +15k / EXC +30k, XG, 0 miss & 0 poor).
        if !scoring.end_bonus_applied {
            let bonus = dtx_scoring::xg_score::xg_end_bonus(
                counts.miss,
                counts.ok,
                counts.perfect,
                scoring.total_notes,
            );
            if bonus != 0 {
                scoring.accum += bonus as f64;
                score.0 = scoring.accum.round().max(0.0) as u64;
            }
            scoring.end_bonus_applied = true;
        }
        info!(
            "DrumsStage: end of chart at now_ms={now_ms}, chart_end_ms={}",
            completion.chart_end_ms
        );
        completion.end_requested = true;
        // Survived to the end → clear banner. The gauge-fail path (handled in
        // `stage_end::detect_stage_failure`) routes to `StageFailed` instead.
        request_transition(&mut requests, AppState::StageClear);
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
            .init_resource::<DrumScoring>()
            .init_resource::<Combo>()
            .init_resource::<JudgmentCounts>()
            .init_resource::<JudgedChips>()
            .init_resource::<LastJudgment>()
            .init_resource::<GameStartMs>()
            .init_resource::<BpmChangeList>()
            .init_resource::<BarLengthChangeList>()
            .init_resource::<GameplayClock>()
            .init_resource::<crate::derived::ChartDerived>()
            .init_resource::<SkillValue>()
            .init_resource::<crate::resources::CurrentEmptyHitTemplates>()
            .init_resource::<crate::resources::ActiveDrumSounds>()
            .init_resource::<crate::se_scheduler::PlayedSeChips>()
            .init_resource::<crate::bgm_scheduler::PlayedBgmChips>()
            .init_resource::<crate::bgm_scheduler::PrimaryBgmChip>()
            .init_resource::<crate::bgm_scheduler::BgmRecoveryState>()
            .init_resource::<DrumGameplaySettings>()
            .init_resource::<BgmAdjustState>();
        let chart = chart_with_n_chips(3);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.add_systems(Update, enter_derive_from_chart);
        app.update();
        let completion = app.world().resource::<DrumsStageCompletion>();
        assert!(completion.chart_end_ms > 0);
    }

    #[test]
    fn chart_end_ms_real_applies_sticky_bar_length() {
        // Regression test for the reported bug: without the bar-length fix,
        // this chart's shape (171 BPM constant, bar-length chips at
        // m14=1.5/m21=0.75/m22=1/m27=0.75/m30=1, last drum chip at raw
        // measure 61 + fraction 0.9369125) computes chart_end_ms_real as
        // 88929 — ~2946ms *before* the real bgm_d.ogg ends (90070ms,
        // GameStartMs=1805 -> 91875ms in chart-ms space). With the fix it
        // should land a few hundred ms *after* real song end instead.
        // See docs/superpowers/specs/2026-07-05-bar-length-timing-fix-design.md.
        use dtx_core::channel::EChannel;
        use dtx_core::chart::{Chart, Chip, Metadata};
        use dtx_timing::math::BarLengthChange;

        let mut chart = Chart {
            metadata: Metadata {
                bpm: Some(171.0),
                ..Default::default()
            },
            ..Default::default()
        };
        chart
            .chips
            .push(Chip::new(61, EChannel::BassDrum, 0.9369125));
        for (measure, ratio) in [(14, 1.5), (21, 0.75), (22, 1.0), (27, 0.75), (30, 1.0)] {
            chart
                .chips
                .push(Chip::new(measure, EChannel::BarLength, ratio));
        }

        let bpm_changes = BpmChangeList::from_chart(&chart);
        let bar_changes = BarLengthChangeList::from_chart(&chart);
        let end_ms = chart_end_ms_real(&chart, &bpm_changes, &bar_changes);

        // 90438 (computed target) + 2000 (buffer) = 92438, real song end in
        // chart-ms space is 1805 + 90070 = 91875. Allow a small tolerance.
        assert!(
            (end_ms - 92438).abs() <= 5,
            "expected ~92438ms, got {end_ms}ms"
        );
        assert!(
            end_ms > 91875,
            "chart must end at/after the real song end (91875ms), got {end_ms}ms \
             (bug reproduces if this is ~88929ms, ~2946ms too early)"
        );
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
        .init_resource::<dtx_audio::DrumPolyphony>()
        .init_resource::<crate::resources::ActiveDrumSounds>()
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
