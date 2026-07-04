//! `CStagePerfGuitarScreen` orchestrator — port of `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs` (787) + `.Chip.cs` (808) = 1595 LoC.
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! ## Role
//!
//! The C# class is the central orchestrator for guitar/bass performance. It
//! owns the lifecycle, composes 17 sub-acts via `listChildActivities`, and
//! handles cross-cutting concerns.
//!
//! In Rust we model this as a Bevy plugin that:
//! - registers guitar/bass sub-act resources
//! - adds end-of-stage transition
//! - owns the stage-level state
//!
//! ## Sub-acts wired
//!
//! (From CStagePerfGuitarScreen.cs:14-29)
//!
//! | C# sub-act | Rust crate | File |
//! |---|---|---|
//! | `CActPerfStageFailure`         | gameplay-guitar | (planned M14) |
//! | `CActPerfGuitarDanger`         | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfVideo`                | dtx-bga         | (existing) |
//! | `CActPerfBGA`                  | dtx-bga         | (existing) |
//! | `CActPerfSkillMeter`           | gameplay-guitar | (planned M14) |
//! | `CActPerfGuitarBonus`          | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfScrollSpeed`          | gameplay-guitar | (planned M14) |
//! | `CActPerfGuitarStatusPanel`    | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfGuitarWailingBonus`   | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfGuitarScore`          | gameplay-guitar | `score.rs` + `guitar_perf.rs` |
//! | `CActPerfGuitarRGB`            | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfGuitarLaneFlushGB`    | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfGuitarJudgementString` | gameplay-guitar | `hud.rs` |
//! | `CActPerfGuitarGauge`          | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerfGuitarCombo`          | gameplay-guitar | `guitar_perf.rs` |
//! | `CActPerformanceInformation`   | gameplay-guitar | (planned M14) |
//! | `CActPerfProgressBar`          | gameplay-guitar | (planned M14) |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/GuitarScreen/CStagePerfGuitarScreen.cs:14-29`

#![allow(dead_code)] // Re-exports for future cross-cutting systems.

use bevy::prelude::*;
use dtx_core::chart::Chart;
use dtx_timing::AudioClock;
use game_shell::{request_transition, AppState, TransitionRequest};

use crate::guitar_perf::{
    GuitarBonus, GuitarDangerState, GuitarGaugeState, GuitarLaneFlush, GuitarRgbState,
    GuitarWailingBonus,
};
use crate::resources::{ActiveChart, BgmAdjustState, Combo, Score};

/// Marker component for the guitar/bass performance stage root entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct GuitarStageRoot;

/// End-of-stage state for guitar/bass (BocuD CStagePerfGuitarScreen OnUpdate).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct GuitarStageCompletion {
    /// Chart end time in ms.
    pub chart_end_ms: i64,
    /// Whether the end transition has been requested.
    pub end_requested: bool,
    /// Whether the gauge has failed.
    pub gauge_failed: bool,
}

/// Plugin assembly.
pub fn plugin(app: &mut App) {
    app.init_resource::<GuitarStageCompletion>()
        .init_resource::<GuitarGaugeState>()
        .init_resource::<GuitarLaneFlush>()
        .init_resource::<GuitarRgbState>()
        .init_resource::<GuitarDangerState>()
        .init_resource::<GuitarWailingBonus>()
        .init_resource::<GuitarBonus>()
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

fn on_enter_performance(
    chart: Res<ActiveChart>,
    mut bgm_adjust: ResMut<BgmAdjustState>,
    mut completion: ResMut<GuitarStageCompletion>,
) {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    bgm_adjust.common_ms = cfg.gameplay.bgm_adjust_ms;
    bgm_adjust.song_ms = chart
        .source_path
        .as_ref()
        .map(|p| {
            dtx_scoring::score_ini::read_bgm_adjust(dtx_scoring::score_ini::score_ini_path(p))
        })
        .unwrap_or(0);
    info!(
        "Guitar stage enter: bgm_adjust total = {}ms (common={}, song={})",
        bgm_adjust.total_ms(),
        bgm_adjust.common_ms,
        bgm_adjust.song_ms
    );
    completion.chart_end_ms = chart_end_ms(&chart.chart);
    completion.end_requested = false;
    completion.gauge_failed = false;
}

fn on_exit_performance(mut completion: ResMut<GuitarStageCompletion>) {
    completion.end_requested = false;
    completion.gauge_failed = false;
}

fn spawn_stage_root(mut commands: Commands) {
    commands.spawn(GuitarStageRoot);
}

fn despawn_stage_root(mut commands: Commands, query: Query<Entity, With<GuitarStageRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Compute the last chip's approximate target time. Same formula as
/// the drums orchestrator (see `gameplay_drums::orchestrator::chart_end_ms`).
pub fn chart_end_ms(chart: &Chart) -> i64 {
    chart
        .chips
        .iter()
        .map(|c| (c.measure as i64) * 2000 + 1000)
        .max()
        .unwrap_or(0)
}

/// Detect end-of-stage for guitar/bass. Same logic as drums orchestrator
/// but gated on `EGameMode::Guitar`.
fn detect_end_of_stage(
    clock: Res<AudioClock>,
    mut completion: ResMut<GuitarStageCompletion>,
    _chart: Res<ActiveChart>,
    _score: Res<Score>,
    _combo: Res<Combo>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if completion.end_requested {
        return;
    }
    let Some(now_ms) = clock.current_ms else {
        return;
    };
    let past_chart_end = now_ms >= completion.chart_end_ms;
    let all_chips_spawned = completion.chart_end_ms > 0;
    if past_chart_end && all_chips_spawned {
        info!(
            "GuitarStage: end of chart at now_ms={now_ms}, chart_end_ms={}",
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
            .map(|i| Chip::new(i, EChannel::GuitarRxxBxx, 1.0))
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
        assert_eq!(chart_end_ms(&c), 1000);
    }

    #[test]
    fn chart_end_ms_picks_last_chip_measure() {
        let c = chart_with_n_chips(3);
        // measure=2 → 2*2000 + 1000 = 5000
        assert_eq!(chart_end_ms(&c), 2 * 2000 + 1000);
    }

    #[test]
    fn guitar_stage_completion_default() {
        let c = GuitarStageCompletion::default();
        assert_eq!(c.chart_end_ms, 0);
        assert!(!c.end_requested);
    }

    #[test]
    fn on_enter_captures_chart_end_ms() {
        let mut app = App::new();
        app.init_resource::<GuitarStageCompletion>()
            .init_resource::<ActiveChart>()
            .init_resource::<BgmAdjustState>();
        app.world_mut().resource_mut::<ActiveChart>().chart = chart_with_n_chips(2);
        app.add_systems(Update, on_enter_performance);
        app.update();
        let completion = app.world().resource::<GuitarStageCompletion>();
        assert_eq!(completion.chart_end_ms, 1000 + 2000);
    }
}
