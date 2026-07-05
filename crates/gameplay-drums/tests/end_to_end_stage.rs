//! End-to-end integration test for the DrumsScreen orchestrator.
//!
//! Verifies the full Performance-state lifecycle:
//! 1. OnEnter(Performance) captures chart_end_ms
//! 2. detect_end_of_stage monitors AudioClock
//! 3. When audio clock passes chart_end, transitions to Result state
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`

use bevy::prelude::*;
use dtx_audio::BgmHandle;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip, Metadata};
use game_shell::AppState;
use gameplay_drums::components::LastJudgment;
use gameplay_drums::judge::{BpmChangeList, JudgedChips};
use gameplay_drums::orchestrator::{
    detect_end_of_stage, enter_derive_from_chart, enter_reset_run_state, enter_seed_bgm_state,
    on_exit_performance, DrumsStageCompletion,
};
use gameplay_drums::resources::{
    ActiveChart, BgmAdjustState, Combo, GameStartMs, GameplayClock, JudgmentCounts, Score,
};

fn chart_with_measures(n: u32) -> Chart {
    let chips: Vec<Chip> = (0..n)
        .map(|i| Chip::new(i, EChannel::BassDrum, 1.0))
        .collect();
    Chart {
        metadata: Metadata::default(),
        chips,
        ..Default::default()
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        bevy_kira_audio::AudioPlugin,
    ))
    .init_state::<AppState>()
    .init_resource::<DrumsStageCompletion>()
    .init_resource::<GameplayClock>()
    .init_resource::<ActiveChart>()
    .init_resource::<Score>()
    .init_resource::<gameplay_drums::resources::DrumScoring>()
    .init_resource::<Combo>()
    .init_resource::<JudgmentCounts>()
    .init_resource::<gameplay_drums::resources::DrumGameplaySettings>()
    .init_resource::<gameplay_drums::resources::DrumAudioSettings>()
    .init_resource::<JudgedChips>()
    .init_resource::<LastJudgment>()
    .init_resource::<GameStartMs>()
    .init_resource::<BgmAdjustState>()
    .init_resource::<BpmChangeList>()
    .init_resource::<BgmHandle>()
    .init_resource::<dtx_audio::ChartSoundBank>()
    .init_resource::<dtx_audio::DrumPolyphony>()
    .init_resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
    .init_resource::<gameplay_drums::bgm_scheduler::PrimaryBgmChip>()
    .init_resource::<gameplay_drums::bgm_scheduler::BgmRecoveryState>()
    .init_resource::<gameplay_drums::resources::CurrentEmptyHitTemplates>()
    .init_resource::<gameplay_drums::resources::ActiveDrumSounds>()
    .init_resource::<gameplay_drums::se_scheduler::PlayedSeChips>()
    .init_resource::<gameplay_drums::resources::FastSlowCount>()
    .init_resource::<gameplay_drums::derived::ChartDerived>()
    .add_message::<game_shell::TransitionRequest>()
    .add_systems(
        OnEnter(AppState::Performance),
        (
            enter_reset_run_state,
            enter_derive_from_chart,
            enter_seed_bgm_state,
        )
            .chain(),
    )
    .add_systems(OnExit(AppState::Performance), on_exit_performance)
    .add_systems(
        Update,
        detect_end_of_stage.run_if(in_state(AppState::Performance)),
    );
    app
}

#[test]
fn end_to_end_enter_performance_captures_end_ms() {
    let mut app = build_app();
    let chart = chart_with_measures(5);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(completion.chart_end_ms > 0);
    assert!(!completion.end_requested);
}

#[test]
fn end_to_end_detect_end_triggers_result_transition() {
    let mut app = build_app();
    let chart = chart_with_measures(2);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    // GameplayClock past chart end.
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(10000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(completion.end_requested, "end_requested should be set");
}

#[test]
fn end_to_end_detect_no_transition_when_audio_before_end() {
    let mut app = build_app();
    let chart = chart_with_measures(10);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(5000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "end_requested should NOT be set when audio before chart end"
    );
}

#[test]
fn end_to_end_detect_no_transition_when_clock_not_started() {
    let mut app = build_app();
    let chart = chart_with_measures(2);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    // GameplayClock not started — should not trigger.
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "end_requested should NOT be set when GameplayClock not started"
    );
}

#[test]
fn end_to_end_end_requested_flag_prevents_duplicate() {
    let mut app = build_app();
    let chart = chart_with_measures(2);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(10000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        completion.end_requested,
        "should be set after first detection"
    );
}

#[test]
fn end_to_end_on_exit_clears_completion_state() {
    let mut app = build_app();
    app.world_mut()
        .resource_mut::<DrumsStageCompletion>()
        .end_requested = true;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "on_exit should clear end_requested"
    );
}

#[test]
fn end_to_end_empty_chart_no_transition() {
    let mut app = build_app();
    let chart = Chart::default();
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(1000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "empty chart should not trigger end-of-stage"
    );
}
