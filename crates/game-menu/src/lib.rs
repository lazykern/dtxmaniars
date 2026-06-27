//! game-menu — Menu stages (Title, SongSelect, Config, Result, etc.).
//!
//! One plugin per stage, registered in [`GameMenuPlugin`].
//!
//! ADR-0010: Mechanics-only port — UI/skin files stripped. Song selection
//! logic kept; visual layer (Title, Config tabs, etc.) is osu-style placeholder.

use bevy::prelude::*;

pub mod config;
pub mod config_key_assign;
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
            startup::plugin,
            title::plugin,
            song_select::plugin,
            config::plugin,
            config_key_assign::plugin,
            song_loading::plugin,
        ));
    }
}
