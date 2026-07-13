//! game-menu — Menu stages (Title, SongSelect, Config, Result, etc.).
//!
//! One plugin per stage, registered in [`GameMenuPlugin`].
//!
//! ADR-0010: Mechanics-only port — UI/skin files stripped. Song selection
//! logic kept; visual layer (Title, Config tabs, etc.) follows the product UI.

// Bevy systems take many params and queries use deeply nested generic tuples;
// both trip these lints across this crate's systems. Bevy-idiomatic
// false-positives, allowed crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;

pub mod chart_stats;
pub mod discovery;
pub mod end;
pub mod import_ui;
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
            import_ui::plugin,
            chart_stats::plugin,
            song_loading::plugin,
            end::plugin,
        ));
    }
}

#[cfg(test)]
mod discovery_tests {
    use std::path::PathBuf;

    use dtx_library::{LibraryPreferences, SongInfo};
    use dtx_scoring::ScoreStore;

    #[test]
    fn favorites_filter_keeps_only_marked_charts() {
        let songs = vec![song("A", 50), song("B", 60)];
        let mut preferences = LibraryPreferences::with_path(PathBuf::from("unused.json"));
        preferences.toggle_favorite(&songs[1].path);
        let filters = crate::discovery::DiscoveryFilters {
            favorites_only: true,
            ..Default::default()
        };

        let result = crate::discovery::filtered_indices(
            &songs,
            &preferences,
            &ScoreStore::default(),
            &filters,
        );
        assert_eq!(result, vec![1]);
    }

    fn song(title: &str, dlevel: u32) -> SongInfo {
        SongInfo {
            path: PathBuf::from(format!("/songs/{title}.dtx")),
            title: title.into(),
            artist: "Artist".into(),
            bpm: None,
            dlevel: Some(dlevel),
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: None,
        }
    }
}
