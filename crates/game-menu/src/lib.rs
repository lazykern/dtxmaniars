//! game-menu — Menu stages (Title, SongSelect, Config, Result, etc.).
//!
//! One plugin per stage, registered in [`GameMenuPlugin`].
//!
//! ## Reference (per ADR-0010 port-first)
//!
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/02.Title/CStageTitle.cs` (378 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs` (21.5KB)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/SortMenuContainer.cs`
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CStageConfig.cs` (531 lines)
//!
//! ## M4 scope
//!
//! Real implementations:
//! - Title (CStageTitle port — minimal: version + "Press ENTER", F1→Config)
//! - SongSelect (CStageSongSelectionNew port — LOGIC: hardcoded song list,
//!   arrow navigation, ENTER→SongLoading, ESC→Title. Visuals deferred to M4.1.)
//! - Config (CStageConfig port — minimal list with item navigation)
//!
//! Stubs:
//! - Startup, SongLoading (real DTX load via dtx-assets), Result, ChangeSkin, End
//!
//! See ADR-0012 for song-select visual simplification.

use bevy::prelude::*;

pub mod change_skin;
pub mod config;
pub mod config_full;
pub mod config_list;
pub mod end;
pub mod result;
pub mod song_loading;
pub mod song_select;
pub mod song_select_full;
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
            song_select_full::plugin,
            config::plugin,
            config_full::plugin,
            config_list::plugin,
            song_loading::plugin,
            result::plugin,
            change_skin::plugin,
            end::plugin,
        ));
    }
}
