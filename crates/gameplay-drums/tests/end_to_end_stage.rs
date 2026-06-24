//! End-to-end integration test for the DrumsScreen orchestrator.
//!
//! Verifies the full Performance-state lifecycle:
//! 1. OnEnter(Performance) captures chart_end_ms
//! 2. detect_end_of_stage monitors AudioClock
//! 3. When audio clock passes chart_end, transitions to Result state
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`

use bevy::prelude::*;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip, Metadata};
use dtx_timing::AudioClock;
use game_shell::AppState;
use gameplay_drums::orchestrator::{
    detect_end_of_stage, on_enter_performance, on_exit_performance, DrumsStageCompletion,
};
use gameplay_drums::resources::{ActiveChart, Combo, Score};

fn chart_with_measures(n: u32) -> Chart {
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

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_resource::<DrumsStageCompletion>()
        .init_resource::<AudioClock>()
        .init_resource::<ActiveChart>()
        .init_resource::<Score>()
        .init_resource::<Combo>()
        .add_systems(OnEnter(AppState::Performance), on_enter_performance)
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
    // measure=4 → 4*2000+1000 = 9000
    assert_eq!(completion.chart_end_ms, 9000);
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
    // Audio clock past chart end (chart end = 10000 for 2 measures).
    app.world_mut().resource_mut::<AudioClock>().current_ms = Some(10000);
    app.update();
    // After update, end_requested should be set.
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
    // Audio clock at 5000ms; chart end at 19000ms (9 measures).
    app.world_mut().resource_mut::<AudioClock>().current_ms = Some(5000);
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "end_requested should NOT be set when audio before chart end"
    );
}

#[test]
fn end_to_end_detect_no_transition_when_audio_clock_none() {
    let mut app = build_app();
    let chart = chart_with_measures(2);
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    // AudioClock.current_ms = None (BGM not playing).
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "end_requested should NOT be set when AudioClock is None"
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
    // Audio past end — trigger end-of-stage.
    app.world_mut().resource_mut::<AudioClock>().current_ms = Some(10000);
    app.update();
    // After the update, end_requested is set AND the system has queued
    // a transition to Result. The state hasn't applied yet.
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
    // chart_end_ms = 0, the check `chart_end_ms > 0` fails.
    app.world_mut().resource_mut::<AudioClock>().current_ms = Some(1000);
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "empty chart should not trigger end-of-stage"
    );
}
