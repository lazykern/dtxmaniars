//! DTXManiaRS desktop binary entrypoint.
//!
//! M6: full menu flow with real SongDb + BGM preview + result persistence
//! + CScoreIni save + guitar mode + dtx-input extraction.

use bevy::prelude::*;
use dtx_assets::DtxAssetsPlugin;
use dtx_input::InputPlugin as DtxInputPlugin;
use dtx_library::SongDbPlugin;
use dtx_scoring::ScoreStore;
use dtx_timing as dtx_timing_plugin;
use game_menu::GameMenuPlugin;
use game_results::{GameResultsPlugin, ScoreStoreResource};
use game_shell::{AppState, EGameMode, GameShellPlugin};
use gameplay_drums::DrumsPlugin;
use gameplay_guitar::GuitarPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(dtx_timing_plugin::plugin)
        .add_plugins(GameShellPlugin)
        .add_plugins(DtxAssetsPlugin)
        .add_plugins(SongDbPlugin)
        .add_plugins(GameMenuPlugin)
        .add_plugins(GameResultsPlugin)
        .add_plugins(DtxInputPlugin)
        // Both gameplay modes registered. M6b: both can run simultaneously;
        // each has its own LaneMap (drums = digits, guitar = letters), so
        // keyboard events don't collide.
        .add_plugins(DrumsPlugin)
        .add_plugins(GuitarPlugin)
        .init_resource::<EGameMode>()
        .init_resource::<ScoreStoreResource>()
        .add_systems(Startup, (load_score_store, log_boot))
        .add_systems(Update, log_state_transitions)
        .run();
}

/// Load persisted scores from disk on startup. M6a.
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
    if last.map_or(true, |s| s != current) {
        info!("AppState: {:?}", current);
        *last = Some(current);
    }
}
