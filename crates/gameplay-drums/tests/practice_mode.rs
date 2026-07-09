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
    .init_resource::<gameplay_drums::seek::LastSeekFrom>()
    .init_resource::<game_shell::EditorSession>()
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
fn a_only_loop_region_does_not_suppress_end_of_stage() {
    // Regression: an A marker with no B (end_ms == i64::MAX, not armed)
    // must not suppress end-of-stage — the loop watcher only seeks back
    // once armed, so suppressing here would softlock the stage.
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 0,
            end_ms: i64::MAX,
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
        completion.end_requested,
        "an A-only (unarmed) loop must not suppress end-of-stage"
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

use gameplay_drums::practice::actions::{
    apply_practice_actions, emit_practice_actions, PracticeAction, PracticeBindings,
};

fn add_action_wiring(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<PracticeBindings>()
        .init_resource::<gameplay_drums::practice::toast::ToastQueue>()
        .init_state::<game_shell::PauseState>()
        .add_message::<PracticeAction>()
        .add_systems(
            Update,
            (emit_practice_actions, apply_practice_actions)
                .chain()
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(resource_exists::<PracticeSession>),
        );
}

#[test]
fn bracket_key_sets_loop_start_snapped_to_bar() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(4_700));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::BracketLeft);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    let region = session.loop_region.expect("A marker set");
    assert_eq!(region.start_ms, 4_000, "A snaps down to the bar start");
}

#[test]
fn restart_key_seeks_to_loop_start() {
    let mut app = build_app();
    add_action_wiring(&mut app);
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
        clock.sync(Some(5_000));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(
        app.world().resource::<GameplayClock>().current_ms,
        2_000,
        "R restarts the loop at A"
    );
}

use gameplay_drums::events::{JudgmentEvent, NoteMissed};

fn add_ramp_wiring(app: &mut App) {
    if !app.world().contains_resource::<Messages<PracticeAction>>() {
        app.add_message::<PracticeAction>();
    }
    app.add_message::<JudgmentEvent>()
        .add_message::<NoteMissed>()
        .init_resource::<gameplay_drums::practice::toast::ToastQueue>()
        .add_systems(
            Update,
            gameplay_drums::practice::ramp::handle_toggle_ramp
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            gameplay_drums::practice::ab_loop::loop_watcher
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            (
                gameplay_drums::practice::stats::track_attempt_stats,
                gameplay_drums::practice::ramp::apply_ramp,
            )
                .chain()
                .after(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        );
}

fn send_practice_action(app: &mut App, action: PracticeAction) {
    app.world_mut()
        .resource_mut::<Messages<PracticeAction>>()
        .write(action);
}

fn looped_session(rate: f32) -> PracticeSession {
    let mut s = PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        preroll: gameplay_drums::practice::session::PrerollSetting::Off,
        rate,
        ..Default::default()
    };
    s.ramp.armed = true;
    s.ramp.current_rate = rate;
    s.current_attempt.start_ms = 2_000;
    s
}

/// Run the clock past B so the loop watcher rolls one attempt.
fn finish_loop_pass(app: &mut App, perfect_hits: u32) {
    for _ in 0..perfect_hits {
        app.world_mut()
            .resource_mut::<Messages<JudgmentEvent>>()
            .write(JudgmentEvent {
                lane: 3,
                kind: dtx_scoring::JudgmentKind::Perfect,
                delta_ms: 0,
                chip_idx: 0, // chip 0 sits at 2000ms — inside the loop
            });
    }
    if perfect_hits == 0 {
        app.world_mut()
            .resource_mut::<Messages<NoteMissed>>()
            .write(NoteMissed {
                lane: 3,
                audio_ms: 5_000,
            });
    }
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.update();
}

#[test]
fn ramp_steps_rate_up_after_clean_pass() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.70));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.75).abs() < 1e-6,
        "clean pass steps 0.70 → 0.75, got {}",
        session.rate
    );
    assert!(session.ramp.armed);
}

#[test]
fn two_failed_passes_step_rate_down() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.80));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 0); // fail #1 → hold
    assert!((app.world().resource::<PracticeSession>().rate - 0.80).abs() < 1e-6);
    finish_loop_pass(&mut app, 0); // fail #2 → step down
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.75).abs() < 1e-6,
        "second fail steps 0.80 → 0.75, got {}",
        session.rate
    );
}

#[test]
fn skip_next_roll_ignores_the_stale_pre_arm_attempt() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.ramp.skip_next_roll = true;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.70).abs() < 1e-6,
        "the roll right after arming must not step the ramp"
    );
    assert!(!session.ramp.skip_next_roll, "flag consumed");
}

#[test]
fn toggle_ramp_without_loop_is_a_noop_error_toast() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    send_practice_action(&mut app, PracticeAction::ToggleRamp);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        !session.ramp.armed,
        "arming without an A/B loop must be a no-op"
    );
    let toasts = app
        .world()
        .resource::<gameplay_drums::practice::toast::ToastQueue>();
    assert!(
        !toasts.0.is_empty(),
        "arming without a loop must push an error toast"
    );
}

#[test]
fn toggle_ramp_with_loop_arms() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        preroll: gameplay_drums::practice::session::PrerollSetting::Off,
        ..Default::default()
    });
    send_practice_action(&mut app, PracticeAction::ToggleRamp);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(session.ramp.armed, "arming with an A/B loop must succeed");
}
