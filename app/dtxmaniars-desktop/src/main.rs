//! DTXManiaRS desktop binary entrypoint.

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy::window::{MonitorSelection, Window, WindowMode, WindowPlugin};
use dtx_assets::DtxAssetsPlugin;
use dtx_input::InputPlugin as DtxInputPlugin;
use dtx_library::SongDbPlugin;
use dtx_scoring::ScoreStore;
use dtx_timing as dtx_timing_plugin;
use dtx_ui::REF_HEIGHT;
use game_menu::GameMenuPlugin;
use game_results::GameResultsPlugin;
use game_shell::{AppState, EGameMode, GameShellPlugin, ScoreStoreResource};
use gameplay_drums::DrumsPlugin;
use gameplay_guitar::GuitarPlugin;

#[cfg(feature = "brp")]
use bevy_brp_extras::BrpExtrasPlugin;

fn main() {
    let windowed = std::env::var("DTXMANIARS_WINDOWED").is_ok();
    let config_report = dtx_config::load_with_report(&dtx_config::default_path());
    let accessibility_policy =
        dtx_ui::AccessibilityPolicy::from(&config_report.config.accessibility);
    let startup_config_warning = dtx_ui::StartupConfigWarning(config_report.warning);

    let mut app = App::new();
    app.insert_resource(accessibility_policy)
        .insert_resource(startup_config_warning);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "DTXManiaRS".into(),
                    mode: if windowed {
                        WindowMode::Windowed
                    } else {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                    },
                    resolution: if windowed {
                        (1280u32, 720u32).into()
                    } else {
                        (1280u32, REF_HEIGHT as u32).into()
                    },
                    present_mode: if config_report.config.system.vsync {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    },
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                filter: format!("{},icu_provider=error", bevy::log::DEFAULT_FILTER),
                ..default()
            })
            .set(bevy::asset::AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
            }),
    )
    .add_plugins(bevy_framepace::FramepacePlugin)
    .add_plugins(dtx_timing_plugin::plugin)
    .add_plugins(dtx_audio::plugin)
    .add_plugins(GameShellPlugin)
    .add_plugins(DtxAssetsPlugin)
    .add_plugins(SongDbPlugin)
    .add_plugins(GameMenuPlugin)
    .add_plugins(GameResultsPlugin)
    .add_plugins(DtxInputPlugin)
    .add_plugins(DrumsPlugin)
    .add_plugins(GuitarPlugin)
    .init_resource::<EGameMode>()
    .init_resource::<ScoreStoreResource>()
    .add_systems(
        Startup,
        (
            load_score_store,
            log_boot,
            log_config_summary,
            spawn_ui_camera,
        ),
    )
    .add_systems(Update, log_state_transitions);

    #[cfg(feature = "brp")]
    app.add_plugins(BrpExtrasPlugin::default());

    app.run();
}

fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn load_score_store(mut store: ResMut<ScoreStoreResource>) {
    let mut inner = ScoreStore::with_path(ScoreStore::default_path());
    if let Err(e) = inner.load() {
        warn!("score store load failed: {e}");
    }
    info!(
        "score store: {} entries, {} charts",
        inner.len(),
        inner.chart_count()
    );
    **store = inner;
}

fn log_boot() {
    info!(
        "dtxmaniars v{} — starting (Default AppState: {:?})",
        env!("CARGO_PKG_VERSION"),
        AppState::default()
    );
}

fn log_state_transitions(state: Res<State<AppState>>, mut last: Local<Option<AppState>>) {
    let current = *state.get();
    if last.is_none_or(|s| s != current) {
        info!("AppState: {:?}", current);
        *last = Some(current);
    }
}

fn log_config_summary(policy: Res<dtx_ui::AccessibilityPolicy>) {
    info!(
        "accessibility: text={:.2}x, transition={}ms, background_motion={}",
        policy.text_multiplier(),
        policy.screen_transition_ms(),
        policy.background_motion(),
    );
}
