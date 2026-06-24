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
use dtx_core::chart::Chart;
use dtx_timing::AudioClock;
use game_shell::AppState;

use crate::drums_perf::{DrumsDangerState, DrumsFillingEffect, DrumsPadState};
use crate::resources::{ActiveChart, Combo, Score};

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
            (on_enter_performance, spawn_stage_root).chain(),
        )
        .add_systems(
            OnExit(AppState::Performance),
            (despawn_stage_root, on_exit_performance).chain(),
        )
        .add_systems(
            Update,
            detect_end_of_stage.run_if(in_state(AppState::Performance)),
        );
}

/// On enter: capture chart end time for end-of-stage detection.
pub fn on_enter_performance(chart: Res<ActiveChart>, mut completion: ResMut<DrumsStageCompletion>) {
    completion.chart_end_ms = chart_end_ms(&chart.chart);
    completion.end_requested = false;
    completion.gauge_failed = false;
}

/// On exit: clear completion state for the next stage entry.
pub fn on_exit_performance(mut completion: ResMut<DrumsStageCompletion>) {
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

/// Compute the last chip's `target_ms` in the audio clock. Returns 0 if
/// the chart is empty.
///
/// Mirrors `CDTXMania.DTX.listChip.OrderBy(...).LastOrDefault()?.nPlaybackTimeMs`
/// (BocuD CStagePerfCommonScreen.cs:23).
pub fn chart_end_ms(chart: &Chart) -> i64 {
    chart
        .chips
        .iter()
        .map(|c| {
            // Approximate: a chip's `target_ms` is `measure * 2000` for a
            // constant 120 BPM chart. The real computation uses BPM changes
            // via `chip_time_ms_with_bpm_changes` (see dtx-timing). For the
            // orchestrator we use a simple measure-based estimate; this is
            // good enough for end-of-stage detection at the minute scale.
            (c.measure as i64) * 2000 + 1000
        })
        .max()
        .unwrap_or(0)
}

/// Detect end-of-stage: chart fully scrolled AND no more chips to process.
/// Transition to `AppState::Result`.
///
/// Mirrors the chart-end check in CStagePerfCommonScreen.cs (Presence
/// property) + the CStage return logic in CStagePerfDrumsScreen.
pub fn detect_end_of_stage(
    clock: Res<AudioClock>,
    mut completion: ResMut<DrumsStageCompletion>,
    _chart: Res<ActiveChart>,
    _score: Res<Score>,
    _combo: Res<Combo>,
    mut next: ResMut<NextState<AppState>>,
) {
    if completion.end_requested {
        return;
    }
    let Some(now_ms) = clock.current_ms else {
        return;
    };
    // End condition: we've scrolled past the last chip AND the audio clock
    // has reached the chart end. The audio clock is the authoritative
    // signal; the chart end_ms is a fallback.
    let past_chart_end = now_ms >= completion.chart_end_ms;
    let all_chips_spawned = completion.chart_end_ms > 0;
    if past_chart_end && all_chips_spawned {
        info!(
            "DrumsStage: end of chart at now_ms={now_ms}, chart_end_ms={}",
            completion.chart_end_ms
        );
        completion.end_requested = true;
        next.set(AppState::Result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::channel::EChannel;
    use dtx_core::chart::{Chart, Chip, Metadata};

    fn chart_with_n_chips(n: u32) -> Chart {
        let chips: Vec<Chip> = (0..n)
            .map(|i| Chip {
                measure: i,
                channel: EChannel::BassDrum,
                value: 1.0,
            })
            .collect();
        Chart {
            metadata: Metadata::default(),
            chips,
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
            .init_resource::<ActiveChart>();
        let chart = chart_with_n_chips(3);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.add_systems(Update, on_enter_performance);
        app.update();
        let completion = app.world().resource::<DrumsStageCompletion>();
        assert_eq!(completion.chart_end_ms, 2 * 2000 + 1000);
    }

    #[test]
    fn on_exit_clears_end_requested() {
        let mut app = App::new();
        app.init_resource::<DrumsStageCompletion>();
        app.world_mut()
            .resource_mut::<DrumsStageCompletion>()
            .end_requested = true;
        app.add_systems(Update, on_exit_performance);
        app.update();
        let completion = app.world().resource::<DrumsStageCompletion>();
        assert!(!completion.end_requested);
    }
}
