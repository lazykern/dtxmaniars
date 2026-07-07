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
