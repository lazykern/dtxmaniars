//! Shared entry adapter for the existing Customize/Settings route.

use std::path::PathBuf;

use bevy::prelude::*;
use game_shell::{AppState, CustomizeTab, TransitionRequest, request_transition};

/// Enter the existing Gameplay Settings route without duplicating its setup.
///
/// The editor still needs a chart-backed Performance session, so use the
/// configured `gameplay.last_played` when it remains valid and otherwise pick
/// an available chart from `SongDb` (lazy-scanning the default song folder).
pub(crate) fn request_gameplay_settings(
    db: &mut dtx_library::SongDb,
    pending: &mut game_shell::PendingCustomizeTab,
    session: &mut game_shell::EditorSession,
    selected: &mut crate::song_select::SelectedSong,
    requests: &mut MessageWriter<TransitionRequest>,
) -> bool {
    let Some(path) = pick_customize_song(db) else {
        return false;
    };
    pending.0 = Some(CustomizeTab::Gameplay);
    session.0 = true;
    selected.0 = Some(path);
    request_transition(requests, AppState::SongLoading);
    true
}

fn pick_customize_song(db: &mut dtx_library::SongDb) -> Option<PathBuf> {
    let cfg = dtx_config::load(&dtx_config::default_path());
    if let Some(last) = cfg.gameplay.last_played.filter(|path| path.exists()) {
        return Some(last);
    }
    if db.is_empty() {
        let dir = dtx_library::default_song_dir();
        if let Err(error) = db.rescan(&dir) {
            warn!("customize: song scan failed: {error}");
        }
    }
    first_available_chart(db)
}

fn first_available_chart(db: &dtx_library::SongDb) -> Option<PathBuf> {
    db.songs.first().map(|song| song.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_chart_fallback_is_deterministic() {
        let mut db = dtx_library::SongDb::default();
        assert_eq!(first_available_chart(&db), None);
        db.songs.push(dtx_library::SongInfo {
            path: PathBuf::from("/songs/first/chart.dtx"),
            title: "First".into(),
            artist: String::new(),
            bpm: None,
            dlevel: None,
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: None,
        });
        assert_eq!(
            first_available_chart(&db),
            Some(PathBuf::from("/songs/first/chart.dtx"))
        );
    }
}
