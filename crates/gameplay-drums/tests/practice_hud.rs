//! Headless HUD tests: full HUD spawn/despawn on pause, normal pause
//! overlay suppressed while a practice session exists.

use bevy::prelude::*;
use game_shell::{AppState, PauseState};
use gameplay_drums::pause::PracticePauseSurface;
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
        .init_resource::<PracticePauseSurface>()
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_full_hud
                .run_if(resource_exists::<PracticeSession>)
                .run_if(gameplay_drums::practice::hud::rail_surface_active),
        )
        .add_systems(OnExit(PauseState::Paused), despawn_full_hud);
    app
}

fn set_rail_surface(app: &mut App) {
    app.world_mut().insert_resource(PracticePauseSurface::Rail);
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
    set_rail_surface(&mut app);
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
fn overlay_spawns_in_practice_on_overlay_surface() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .init_resource::<PracticePauseSurface>() // defaults to Overlay
        .add_systems(
            OnEnter(PauseState::Paused),
            gameplay_drums::pause::spawn_overlay,
        );
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(
        overlays, 1,
        "Esc surface shows the pause overlay in practice"
    );
}

#[test]
fn overlay_suppressed_on_rail_surface() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .init_resource::<PracticePauseSurface>()
        .add_systems(
            OnEnter(PauseState::Paused),
            gameplay_drums::pause::spawn_overlay,
        );
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut().insert_resource(PracticePauseSurface::Rail);
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(
        overlays, 0,
        "Tab surface suppresses the overlay; the rail owns it"
    );
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
    // Simulate the Tab opener: the rail owns this pause.
    app.world_mut()
        .insert_resource(gameplay_drums::pause::PracticePauseSurface::Rail);
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
fn hud_plugin_overlay_surface_spawns_no_rail() {
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

    // Esc path: surface stays at its Overlay default.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(PauseState::Paused);
    app.update();

    let huds = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(huds, 0, "Esc surface must not spawn the rail");
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
    assert_eq!(
        mini_strips, 1,
        "mini strip must spawn on entering Performance"
    );

    let chips = app
        .world_mut()
        .query::<&gameplay_drums::practice::hud::chip::StatusChip>()
        .iter(app.world())
        .count();
    assert_eq!(chips, 1, "status chip must spawn on entering Performance");
}

use gameplay_drums::practice::hud::full_hud::{full_hud_input, RailItem};
use gameplay_drums::practice::session::{LoopRegion, PracticeTransport};

#[test]
fn rail_clear_loop_disarms_the_ramp() {
    // Regression: the rail "Clear loop" row must go through
    // `session.clear_loop()` (which disarms) — a raw `loop_region = None`
    // would leave the ramp armed against a now-different span.
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_state::<PauseState>()
        .add_message::<game_shell::TransitionRequest>()
        .add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<GameplayClock>()
        .init_resource::<ChipTimeline>()
        .init_resource::<RailSelection>()
        .add_systems(Update, full_hud_input);

    let mut session = PracticeSession {
        transport: PracticeTransport {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    session.trainer.ramp.armed = true;
    app.world_mut().insert_resource(session);

    // Point the rail selection at the Clear-loop row and press Enter.
    let idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::ClearLoop)
        .expect("ClearLoop is a rail row");
    app.world_mut().resource_mut::<RailSelection>().0 = idx;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();

    let session = app.world().resource::<PracticeSession>();
    assert!(
        session.transport.loop_region.is_none(),
        "rail Clear loop clears the region"
    );
    assert!(
        !session.trainer.ramp.armed,
        "rail Clear loop must disarm the ramp"
    );
}

use gameplay_drums::practice::hud::full_hud::{RailAdjustButton, RailRowButton};

#[test]
fn rail_spawns_17_rows_with_adjust_buttons_at_practice_z() {
    let mut app = build_app();
    app.world_mut().insert_resource(PracticeSession::default());
    set_rail_surface(&mut app);
    set_paused(&mut app, true);

    let rows = app
        .world_mut()
        .query::<&RailRowButton>()
        .iter(app.world())
        .count();
    assert_eq!(rows, 17, "one clickable row per RailItem");

    let adjusts = app
        .world_mut()
        .query::<&RailAdjustButton>()
        .iter(app.world())
        .count();
    assert_eq!(adjusts, 18, "9 value rows x (◂ + ▸)");

    let z = app
        .world_mut()
        .query::<(&FullHudRoot, &GlobalZIndex)>()
        .iter(app.world())
        .map(|(_, z)| z.0)
        .next()
        .expect("full HUD root has a GlobalZIndex");
    assert_eq!(z, 1000, "ui_z::PRACTICE_FULL_HUD");
}

use gameplay_drums::practice::hud::full_hud::{transport_buttons, TransportButton};

/// 2 bars @ 120 BPM: bar starts at 0 and 2000.
fn two_bar_timeline() -> ChipTimeline {
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
    ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 4_000)
}

#[test]
fn next_bar_button_moves_scrub_cursor() {
    let mut app = build_app();
    app.add_systems(Update, transport_buttons);
    app.world_mut().insert_resource(two_bar_timeline());
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            scrub_cursor_ms: Some(0),
            ..Default::default()
        },
        ..Default::default()
    });
    app.world_mut()
        .spawn((Interaction::Pressed, TransportButton::NextBar));
    app.update();
    assert_eq!(
        app.world()
            .resource::<PracticeSession>()
            .transport
            .scrub_cursor_ms,
        Some(2_000),
        "next-bar button advances the scrub cursor one bar"
    );
}

use gameplay_drums::practice::hud::full_hud::rail_mouse;

#[test]
fn adjust_button_click_steps_tempo_and_moves_selection() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut()
        .spawn((Interaction::Pressed, RailAdjustButton(RailItem::Rate, 1)));
    app.update();

    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.transport.user_tempo - 1.05).abs() < 1e-6,
        "▸ on Tempo steps +0.05 like ArrowRight"
    );
    let rate_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::Rate)
        .expect("Rate is a rail row");
    assert_eq!(
        app.world().resource::<RailSelection>().0,
        rate_idx,
        "mouse click moves the shared selection cursor"
    );
}

#[test]
fn row_click_selects_and_activates_set_a() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(two_bar_timeline());
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            scrub_cursor_ms: Some(2_500),
            ..Default::default()
        },
        ..Default::default()
    });
    app.world_mut()
        .spawn((Interaction::Pressed, RailRowButton(RailItem::SetA)));
    app.update();

    let session = app.world().resource::<PracticeSession>();
    assert_eq!(
        session.transport.loop_region.map(|r| r.start_ms),
        Some(2_000),
        "row click on Set A snaps the loop start to the bar"
    );
    let a_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::SetA)
        .expect("SetA is a rail row");
    assert_eq!(app.world().resource::<RailSelection>().0, a_idx);
}

#[test]
fn value_row_click_selects_without_acting() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut()
        .spawn((Interaction::Pressed, RailRowButton(RailItem::Scrub)));
    app.update();

    // Selection moved, but no seek was written (Scrub activation = "play here").
    let scrub_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::Scrub)
        .expect("Scrub is a rail row");
    assert_eq!(app.world().resource::<RailSelection>().0, scrub_idx);
    let seeks = app
        .world()
        .resource::<bevy::ecs::message::Messages<gameplay_drums::seek::SeekToChartTime>>()
        .iter_current_update_messages()
        .count();
    assert_eq!(seeks, 0, "value-row click must not trigger play-here");
}
