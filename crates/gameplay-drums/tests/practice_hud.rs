//! Headless HUD tests: full HUD spawn/despawn on pause, normal pause
//! overlay suppressed while a practice session exists.

use bevy::prelude::*;
use game_shell::{AppState, PauseState};
use gameplay_drums::practice::hud::full_hud::{
    despawn_full_hud, spawn_full_hud, FullHudRoot, RailSelection,
};
use gameplay_drums::practice::session::PracticeSession;
use gameplay_drums::resources::GameplayClock;
use gameplay_drums::timeline::ChipTimeline;

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_state::<PauseState>()
        .init_resource::<GameplayClock>()
        .init_resource::<ChipTimeline>()
        .init_resource::<RailSelection>()
        .init_resource::<gameplay_drums::practice::hud::full_hud::ExitArmed>()
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_full_hud.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), despawn_full_hud);
    app
}

fn set_paused(app: &mut App, paused: bool) {
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(if paused {
            PauseState::Paused
        } else {
            PauseState::Running
        });
    app.update();
}

#[test]
fn full_hud_spawns_on_pause_and_despawns_on_resume() {
    let mut app = build_app();
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "full HUD present while paused");

    set_paused(&mut app, false);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "full HUD gone after resume");
}

#[test]
fn full_hud_absent_without_practice_session() {
    let mut app = build_app();
    set_paused(&mut app, true);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "normal pause never spawns the practice HUD");
}

#[test]
fn normal_pause_overlay_suppressed_in_practice() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .add_systems(OnEnter(PauseState::Paused), gameplay_drums::pause::spawn_overlay);
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(overlays, 0, "practice suppresses the normal pause overlay");
}

// The top-level `gameplay_drums::plugin` also wires `orchestrator`, `autoplay`,
// `bgm_scheduler`, `editor`, etc., which pull in real audio/asset/config-file
// I/O — too heavy for a headless schedule-build smoke test. Instead we build
// the real `practice::hud::plugin` (promoted from `pub(super)` to `pub` for
// this test) directly, wired with the minimum states/resources/messages it
// declares dependencies on (game_shell's AppState/PauseState/TransitionRequest,
// GameplayClock, ChipTimeline, PracticeSession, SeekToChartTime,
// PracticeAction), and drive it through OnEnter(Performance) +
// OnEnter(Paused) + a couple of `Update` ticks. This proves the run-condition
// chains and system params in the real plugin fn actually resolve, closing
// the gap where every other HUD test hand-wires a handful of systems instead
// of the real plugin registration.
#[test]
fn real_hud_plugin_schedule_builds_headlessly() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
    ))
        .init_state::<AppState>()
        .init_state::<PauseState>()
        .add_message::<game_shell::TransitionRequest>()
        .add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .init_resource::<GameplayClock>()
        .init_resource::<ChipTimeline>()
        .world_mut()
        .insert_resource(PracticeSession::default());

    gameplay_drums::practice::hud::plugin(&mut app);

    // Drive Performance + Paused so every run_if-gated system in the plugin
    // (spawn/despawn, mouse/input/transport/marker update chain) actually
    // gets scheduled at least once.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(PauseState::Paused);
    app.update();
    app.update();

    let huds = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(huds, 1, "real plugin schedule spawned the full HUD");
}

#[test]
fn quick_tier_entities_spawn_on_entering_performance() {
    // Spec: mini strip + status chip must exist while playing (Running),
    // independent of the full HUD which is pause-gated. Wire the real
    // hud::plugin (mini_strip + chip + full_hud) headlessly and drive
    // OnEnter(Performance) only — no pause.
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
    ))
    .init_state::<AppState>()
    .init_state::<PauseState>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_message::<gameplay_drums::practice::actions::PracticeAction>()
    .init_resource::<GameplayClock>()
    .init_resource::<ChipTimeline>()
    .world_mut()
    .insert_resource(PracticeSession::default());

    gameplay_drums::practice::hud::plugin(&mut app);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();

    let mini_strips = app
        .world_mut()
        .query::<&gameplay_drums::practice::hud::mini_strip::MiniStripRoot>()
        .iter(app.world())
        .count();
    assert_eq!(mini_strips, 1, "mini strip must spawn on entering Performance");

    let chips = app
        .world_mut()
        .query::<&gameplay_drums::practice::hud::chip::StatusChip>()
        .iter(app.world())
        .count();
    assert_eq!(chips, 1, "status chip must spawn on entering Performance");
}

use gameplay_drums::practice::hud::full_hud::{transport_buttons, TransportButton};

#[test]
fn next_bar_button_moves_scrub_cursor() {
    let mut app = build_app();
    app.add_systems(Update, transport_buttons);
    // 2 bars @ 120 BPM: bar starts at 0 and 2000.
    let chart = dtx_core::chart::Chart {
        metadata: dtx_core::chart::Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![dtx_core::chart::Chip::new(
            0,
            dtx_core::channel::EChannel::BassDrum,
            0.0,
        )],
        ..Default::default()
    };
    let bpm = gameplay_drums::judge::BpmChangeList::from_chart(&chart);
    let bar = gameplay_drums::judge::BarLengthChangeList::from_chart(&chart);
    app.world_mut().insert_resource(ChipTimeline::from_chart(
        &chart, &bpm, &bar, 0, 4_000,
    ));
    app.world_mut().insert_resource(PracticeSession {
        scrub_cursor_ms: Some(0),
        ..Default::default()
    });
    app.world_mut()
        .spawn((Interaction::Pressed, TransportButton::NextBar));
    app.update();
    assert_eq!(
        app.world()
            .resource::<PracticeSession>()
            .scrub_cursor_ms,
        Some(2_000),
        "next-bar button advances the scrub cursor one bar"
    );
}
