//! CStageSongSelectionNew — song select screen (M5: real SongDb).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs`
//!
//! M5 ports the LOGIC: EReturnValue (Selected/ReturnToTitle/CallConfig),
//! arrow nav, BGM preview on row select (per CActSelectPresound.cs).
//! Visuals simplified per ADR-0012 (no bigAlbumArt/density graphs/sort menus
//! in the UI, but the SortMode enum + cycle_sort exist for completeness).
//!
//! ## M5 changes from M4
//!
//! - Removed hardcoded `m4_song_list()`. Now reads `Res<SongDb>` from
//!   `dtx-library`.
//! - On AppState::SongSelect OnEnter: if SongDb is empty, scan default dir.
//! - On row select change: trigger BGM preview via `dtx-audio::play_bgm`.
//! - On OnExit: stop BGM.
//! - TAB key cycles sort mode.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_audio::{BgmHandle, play_bgm, stop_bgm_system};
use dtx_library::SongDb;
use game_shell::AppState;
use game_shell::EGameMode;
use game_shell::fade::start_fade;

/// The currently-selected song path. Set by SongSelect, consumed by SongLoading.
#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedSong(pub Option<PathBuf>);

#[derive(Component)]
pub struct SongSelectEntity;

#[derive(Resource, Debug, Default, Clone, Copy)]
struct SelectionIndex(usize);

#[derive(Component)]
struct SongRowEntity {
    index: usize,
}

/// Default song directory to scan when SongDb is empty.
/// Override via `DTX_SONG_DIR` env var.
fn default_song_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DTX_SONG_DIR") {
        return PathBuf::from(p);
    }
    // Fallback: fixture dir so first-launch still shows something.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dtx-core")
        .join("tests")
        .join("fixtures")
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SelectionIndex>()
        .add_plugins(dtx_audio::plugin)
        .add_systems(
            OnEnter(AppState::SongSelect),
            (ensure_song_db_loaded, spawn_song_select, start_fade).chain(),
        )
        .add_systems(
            OnExit(AppState::SongSelect),
            (stop_bgm_system, despawn_song_select).chain(),
        )
        .add_systems(
            Update,
            (
                song_select_navigation,
                render_selected_song,
                bgm_preview_on_change,
            )
                .run_if(in_state(AppState::SongSelect)),
        );
}

/// ponytail: bevy 0.19 removed `despawn_recursive`; do it manually.
fn despawn_song_select(
    mut commands: Commands,
    parents: Query<Entity, With<SongSelectEntity>>,
    children: Query<&Children>,
) {
    for parent in &parents {
        despawn_recursive(&mut commands, parent, &children);
    }
}

fn despawn_recursive(commands: &mut Commands, entity: Entity, children: &Query<&Children>) {
    if let Ok(c) = children.get(entity) {
        for child in c.iter() {
            despawn_recursive(commands, child, children);
        }
    }
    commands.entity(entity).despawn();
}

/// On entering SongSelect, scan the default dir if SongDb is empty.
fn ensure_song_db_loaded(mut db: ResMut<SongDb>) {
    if db.is_empty() {
        let dir = default_song_dir();
        info!("SongSelect: SongDb empty, scanning {}", dir.display());
        if let Err(e) = db.rescan(&dir) {
            warn!("SongSelect: scan failed: {}", e);
        }
    }
}

fn spawn_song_select(mut commands: Commands, db: Res<SongDb>) {
    commands
        .spawn((
            SongSelectEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(40.0)),
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Song Select"),
                TextFont {
                    font_size: FontSize::Px(36.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(
                    "↑↓: Navigate  ENTER: Play  TAB: Sort  F5: Refresh  F1: Config  ESC: Title",
                ),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
            parent.spawn((
                Text::new(format!("Sort: {:?}", db.sort_mode)),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
                SortModeText,
            ));

            for (i, song) in db.songs.iter().enumerate() {
                parent
                    .spawn((
                        SongRowEntity { index: i },
                        Node {
                            width: Val::Px(500.0),
                            height: Val::Px(30.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(if i == 0 {
                            Color::srgb(0.3, 0.5, 0.8)
                        } else {
                            Color::srgb(0.15, 0.15, 0.2)
                        }),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(format!("{}  -  {}", song.title, song.artist)),
                            TextFont {
                                font_size: FontSize::Px(18.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }

            if let Some(song) = db.songs.first() {
                parent.spawn((
                    SelectedSongInfo,
                    Text::new(format_song_detail(song)),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
            }
        });
}

#[derive(Component)]
struct SelectedSongInfo;

#[derive(Component)]
struct SortModeText;

fn format_song_detail(song: &dtx_library::SongInfo) -> String {
    let mut s = format!(
        "Selected:\n  Title: {}\n  Artist: {}\n  BPM: {}\n  Drums level: {:?}",
        song.title,
        song.artist,
        song.bpm
            .map(|b| format!("{b}"))
            .unwrap_or_else(|| "?".to_string()),
        song.dlevel
    );
    if let Some(bgm) = &song.bgm_path {
        s.push_str(&format!("\n  BGM: {}", bgm.display()));
    }
    s
}

fn song_select_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<SelectionIndex>,
    mut selected_song: ResMut<SelectedSong>,
    mut db: ResMut<SongDb>,
    mut mode: ResMut<EGameMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    if db.is_empty() {
        return;
    }
    let max = db.len() - 1;

    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = selection.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::Enter) {
        let song = &db.songs[selection.0];
        selected_song.0 = Some(song.path.clone());
        next.set(AppState::SongLoading);
    } else if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    } else if keys.just_pressed(KeyCode::F1) {
        next.set(AppState::Config);
    } else if keys.just_pressed(KeyCode::F2) {
        *mode = mode.next();
        info!("EGameMode: {:?}", *mode);
    } else if keys.just_pressed(KeyCode::Tab) {
        db.cycle_sort();
        // Clamp selection in case list shrank.
        selection.0 = selection.0.min(db.len().saturating_sub(1));
    } else if keys.just_pressed(KeyCode::F5) {
        // F5: refresh SongDb from the same scan root.
        if let Err(e) = db.refresh() {
            warn!("SongSelect: refresh failed: {}", e);
        } else {
            info!("SongSelect: refreshed ({} songs)", db.len());
        }
        // Reset selection to first row.
        selection.0 = 0;
    }
}

fn render_selected_song(
    selection: Res<SelectionIndex>,
    mut rows: Query<(&SongRowEntity, &mut BackgroundColor)>,
    mut text_query: Query<&mut Text, Or<(With<SelectedSongInfo>, With<SortModeText>)>>,
    db: Res<SongDb>,
    mode: Res<EGameMode>,
) {
    if db.is_empty() {
        return;
    }

    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.index == selection.0 {
            Color::srgb(0.3, 0.5, 0.8)
        } else {
            Color::srgb(0.15, 0.15, 0.2)
        };
    }

    if let Some(song) = db.songs.get(selection.0) {
        let detail = format_song_detail(song);
        for mut text in &mut text_query {
            if text.0.is_empty() || text.0.starts_with("Selected") {
                *text = Text::new(detail.clone());
            } else if text.0.starts_with("Sort") {
                *text = Text::new(format!("Sort: {:?}", db.sort_mode));
            } else if text.0.starts_with("Mode") {
                *text = Text::new(format!("Mode: {}", mode.label()));
            }
        }
    }
}

/// BGM preview: when the selected row changes, start playing that song's BGM.
/// Reference: CActSelectPresound.cs — plays preview audio on selection change.
fn bgm_preview_on_change(
    selection: Res<SelectionIndex>,
    db: Res<SongDb>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut bgm: ResMut<BgmHandle>,
    mut last_played: Local<Option<PathBuf>>,
) {
    if db.is_empty() {
        return;
    }
    let Some(song) = db.songs.get(selection.0) else {
        return;
    };
    let Some(bgm_path) = &song.bgm_path else {
        return;
    };
    if last_played.as_ref() == Some(bgm_path) {
        return;
    }
    *last_played = Some(bgm_path.clone());
    let path_str = bgm_path.to_string_lossy().to_string();
    info!("BGM preview: {}", path_str);
    play_bgm(&audio, &asset_server, &mut bgm, &path_str);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_library::{SongInfo, SortMode};

    fn fake_song(title: &str, artist: &str, bpm: f32, level: u32) -> SongInfo {
        SongInfo {
            path: PathBuf::from(format!("/songs/{title}.dtx")),
            title: title.into(),
            artist: artist.into(),
            bpm: Some(bpm),
            dlevel: Some(level),
            bgm_path: None,
        }
    }

    #[test]
    fn selected_song_resource_starts_empty() {
        let s = SelectedSong::default();
        assert!(s.0.is_none());
    }

    #[test]
    fn selection_index_starts_at_zero() {
        assert_eq!(SelectionIndex::default().0, 0);
    }

    #[test]
    fn arrow_up_saturates_at_zero() {
        let mut sel = SelectionIndex(0);
        sel.0 = sel.0.saturating_sub(1);
        assert_eq!(sel.0, 0);
    }

    #[test]
    fn arrow_down_within_bounds() {
        let mut sel = SelectionIndex(0);
        sel.0 = (sel.0 + 1).min(2);
        assert_eq!(sel.0, 1);
    }

    #[test]
    fn format_song_detail_includes_bpm_and_level() {
        let song = fake_song("X", "Y", 150.0, 85);
        let s = format_song_detail(&song);
        assert!(s.contains("X"));
        assert!(s.contains("Y"));
        assert!(s.contains("150"));
        assert!(s.contains("85"));
    }

    #[test]
    fn sort_mode_cycles_through_three() {
        assert_eq!(SortMode::Default.next(), SortMode::ByTitle);
        assert_eq!(SortMode::ByTitle.next(), SortMode::ByArtist);
        assert_eq!(SortMode::ByArtist.next(), SortMode::Default);
    }
}
