//! Integration tests for practice mode: seek, gates, loop.

use bevy::prelude::*;
use dtx_audio::BgmHandle;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip, Metadata};
use game_shell::AppState;
use gameplay_drums::components::LastJudgment;
use gameplay_drums::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use gameplay_drums::orchestrator::{
    detect_end_of_stage, enter_derive_from_chart, enter_reset_run_state, enter_seed_bgm_state,
    DrumsStageCompletion,
};
use gameplay_drums::practice::session::{LoopRegion, PracticeSession};
use gameplay_drums::resources::{
    ActiveChart, BgmAdjustState, Combo, GameStartMs, GameplayClock, JudgmentCounts, Score,
};
use gameplay_drums::se_scheduler::PlayedSeChips;
use gameplay_drums::seek::SeekToChartTime;
use gameplay_drums::timeline::build_chip_timeline;

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
    .init_resource::<BarLengthChangeList>()
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
    .init_resource::<gameplay_drums::resources::SkillValue>()
    .init_resource::<gameplay_drums::derived::ChartDerived>()
    .init_resource::<gameplay_drums::resources::TimingLineCrossed>()
    .init_resource::<gameplay_drums::timeline::ChipTimeline>()
    .init_resource::<gameplay_drums::seek::PendingBgmStart>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_systems(
        OnEnter(AppState::Performance),
        (
            enter_reset_run_state,
            enter_derive_from_chart,
            enter_seed_bgm_state,
            build_chip_timeline,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (gameplay_drums::seek::apply_seek_system, detect_end_of_stage)
            .chain()
            .run_if(in_state(AppState::Performance)),
    );
    app
}

fn enter_performance(app: &mut App, chart: Chart) {
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
}

#[test]
fn active_loop_region_suppresses_end_of_stage() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 0,
            end_ms: 2_000,
        }),
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(50_000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "active A/B loop must suppress end-of-stage"
    );
}

#[test]
fn cleared_loop_region_restores_end_of_stage() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(50_000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        completion.end_requested,
        "practice without a loop region ends the stage normally"
    );
}

#[test]
fn loop_watcher_seeks_back_to_region_start() {
    let mut app = build_app();
    // Register the watcher in front of the seek system.
    app.add_systems(
        Update,
        gameplay_drums::practice::ab_loop::loop_watcher
            .before(gameplay_drums::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        preroll: gameplay_drums::practice::session::PrerollSetting::Off,
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(6_100));
    }
    app.update();
    let clock = app.world().resource::<GameplayClock>();
    assert_eq!(
        clock.current_ms, 2_000,
        "past region end the clock must land back on A"
    );
    // Chip timing here lands exactly on measure boundaries (`chip_target_ms`
    // is unclamped): chip 0 sits exactly at A (2000ms), chip 2 exactly at B
    // (6000ms) — neither is strictly before A, so both stay live post-seek.
    let judged = &app.world().resource::<JudgedChips>().0;
    assert!(
        !judged.contains(&0),
        "chip at A (2000ms) is live, not seeded"
    );
    assert!(
        !judged.contains(&2),
        "chip at B (6000ms) is live, not seeded"
    );
}

fn send_seek(app: &mut App, target_ms: i64) {
    app.world_mut()
        .resource_mut::<Messages<SeekToChartTime>>()
        .write(SeekToChartTime {
            target_ms,
            snap: None,
            attempt_start_ms: None,
        });
}

#[test]
fn forward_seek_seeds_skip_sets_and_jumps_clock() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 9_000);
    let judged = &app.world().resource::<JudgedChips>().0;
    // Chips 0..=3 land before 9000ms at default 120 BPM (measure=2000ms).
    assert!(judged.contains(&0) && judged.contains(&3));
    assert!(!judged.contains(&4), "chips past target stay live");
}

#[test]
fn backward_seek_prunes_skip_sets() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();
    send_seek(&mut app, 0);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 0);
    assert!(
        app.world().resource::<JudgedChips>().0.is_empty(),
        "backward seek must un-mark judged chips"
    );
    assert!(app.world().resource::<PlayedSeChips>().0.is_empty());
}

#[test]
fn seek_is_inert_without_practice_in_normal_play() {
    // Regression guard: with no PracticeSession and no seek messages,
    // a normal stage run is untouched by the new systems.
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(10_000));
    }
    app.update();
    assert!(
        app.world().resource::<DrumsStageCompletion>().end_requested,
        "normal end-of-stage unchanged"
    );
}
