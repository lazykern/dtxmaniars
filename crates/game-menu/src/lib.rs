//! game-menu — Menu stages (Title, SongSelect, Config, Result, etc.).
//!
//! One plugin per stage, registered in [`GameMenuPlugin`].
//!
//! ADR-0010: Mechanics-only port — UI/skin files stripped. Song selection
//! logic kept; visual layer (Title, Config tabs, etc.) is osu-style placeholder.

use bevy::prelude::*;

pub mod chart_stats;
pub mod end;
pub mod song_loading;
pub mod song_select;
pub mod startup;
pub mod title;

pub use song_select::SelectedSong;

/// Root plugin. Registers all menu-stage sub-plugins.
pub struct GameMenuPlugin;

impl Plugin for GameMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedSong>().add_plugins((
            dtx_bga::plugin,
            startup::plugin,
            title::plugin,
            song_select::plugin,
            chart_stats::plugin,
            song_loading::plugin,
            end::plugin,
        ));
    }
}
