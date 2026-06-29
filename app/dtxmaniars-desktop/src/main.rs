//! DTXManiaRS desktop binary entrypoint.

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, Window, WindowMode, WindowPlugin};
use dtx_assets::DtxAssetsPlugin;
use dtx_audio;
use dtx_input::InputPlugin as DtxInputPlugin;
use dtx_library::SongDbPlugin;
use dtx_scoring::ScoreStore;
use dtx_timing as dtx_timing_plugin;
use dtx_ui::REF_HEIGHT;
use game_menu::GameMenuPlugin;
use game_results::{GameResultsPlugin, ScoreStoreResource};
use game_shell::{AppState, EGameMode, GameShellPlugin};
use gameplay_drums::DrumsPlugin;
use gameplay_guitar::GuitarPlugin;

#[cfg(feature = "brp")]
use bevy_brp_extras::BrpExtrasPlugin;

fn main() {
    let windowed = std::env::var("DTXMANIARS_WINDOWED").is_ok();

    let mut app = App::new();
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
            load_config_summary,
            log_boot,
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

fn load_config_summary() {
    use dtx_config::{default_path, load};
    let cfg = load(&default_path());
    info!(
        "config: skin={}, master_vol={:.0}%, scroll={:.2}x, vsync={}",
        cfg.skin,
        cfg.audio.master_volume * 100.0,
        cfg.gameplay.scroll_speed,
        cfg.system.vsync,
    );
}
