#![allow(clippy::type_complexity)]
//! CStageSongSelectionNew — song select screen (M5: real SongDb).
//!
//! Merged from `song_select.rs` (M5 song list logic) +
//! `song_select_full.rs` (strict-port status panel / density / sort / search).
//! Single plugin, no double-spawn. The song list spawns on OnEnter(SongSelect);
//! the persistent status panel + density + sort + search spawn on Startup.
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
use dtx_audio::{
    BgmHandle, PreviewPlayer, PreviewSwapDirection, PreviewSwapEvent, get_or_load_audio_handle,
    stop_bgm_system,
};
use dtx_library::{SongDb, SongInfo, SortMode};
use dtx_ui::ThemeResource;
use dtx_ui::theme::Theme;
use dtx_ui::widget::album_art::AlbumArt;
use game_shell::{AppState, TransitionRequest, request_transition};

// ===== Layout positions (verbatim from reference files) =====

/// StatusPanel position (StatusPanel.cs:10-12).
pub const STATUS_PANEL_DRUMS_X: f32 = 430.0;
pub const STATUS_PANEL_DRUMS_Y: f32 = 720.0;
pub const STATUS_PANEL_GUITAR_X: f32 = 200.0;
pub const STATUS_PANEL_GUITAR_Y: f32 = 720.0;

/// DensityGraph bar geometry (DensityGraph.cs:30-46).
pub const DENSITY_GRAPH_BAR_COUNT: usize = 8;
pub const DENSITY_GRAPH_BAR_DX: f32 = 12.0;
pub const DENSITY_GRAPH_BAR_W: f32 = 4.0;
pub const DENSITY_GRAPH_BAR_H: f32 = 252.0;
pub const DENSITY_GRAPH_BAR_BASE_X: f32 = 36.0;
pub const DENSITY_GRAPH_BAR_BASE_Y: f32 = 284.0;
pub const DENSITY_NOTE_TEXT_DRUMS: (f32, f32) = (150.0, 333.0);
pub const DENSITY_NOTE_TEXT_GB: (f32, f32) = (102.0, 333.0);

/// SortMenu container (SortMenuContainer.cs:25-26).
pub const SORT_MENU_W: f32 = 662.0;
pub const SORT_MENU_H: f32 = 92.0;
pub const SORT_MENU_ELEMENT_SPACING: f32 = 90.0;

/// SongSearchMenu layout (SongSearchMenu.cs:13-22).
pub const SONG_SEARCH_W: f32 = 500.0;
pub const SONG_SEARCH_H: f32 = 300.0;
pub const SONG_SEARCH_TEXT_INPUT_Y: f32 = 30.0;
pub const SONG_SEARCH_DESC_Y: f32 = 60.0;
pub const SONG_SEARCH_STATUS_Y: f32 = 250.0;

/// Album-art placeholder size (ADR-0015 followup). Real image
/// loading from `#PREIMAGE:` is a separate task; we render a tinted
/// panel of this size and crossfade its opacity.
pub const ALBUM_ART_W: f32 = 240.0;
pub const ALBUM_ART_H: f32 = 180.0;

/// CommandHistory buffer size (CommandHistory.cs:10).
pub const COMMAND_HISTORY_BUF: usize = 16;

// ===== Resources (shared with other crates) =====

/// The currently-selected song path. Set by SongSelect, consumed by SongLoading.
#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedSong(pub Option<PathBuf>);

/// One folder's worth of DTX charts, deduplicated from `db.songs`.
/// One `SongFolderView` per row in the song-select list.
///
/// Corresponds to BocuD's per-song entry in `CActSelectSongList`
/// (folder containing `bsc.dtx`, `adv.dtx`, `ext.dtx`, `mas.dtx`,
/// `edit.dtx`). Cycling difficulty per row picks which chart inside
/// the folder plays.
#[derive(Debug, Clone)]
pub struct SongFolderView {
    /// Folder path (parent directory of the DTX files).
    pub folder: PathBuf,
    /// Song title (taken from the first chart's metadata).
    pub title: String,
    /// Song artist (taken from the first chart's metadata).
    pub artist: String,
    /// Indices into `db.songs`, sorted by `dlevel` ascending so
    /// 0 = easiest, then Adv/Ext/Mas/Edit. Stable within ties.
    pub chart_indices: Vec<usize>,
}

impl SongFolderView {
    pub fn difficulty_count(&self) -> usize {
        self.chart_indices.len()
    }

    pub fn difficulty_label(difficulty: u8) -> &'static str {
        match difficulty {
            0 => "BASIC",
            1 => "ADV",
            2 => "EXT",
            3 => "MAS",
            4 => "EDIT",
            _ => "?",
        }
    }
}

/// Currently selected folder + chart, search/sort state, and the
/// deduplicated visible list.
#[derive(Resource, Default, Debug, Clone)]
pub struct SongSelectSelection {
    /// Selected chart (`db.songs[idx]`), updated by `render_selected_song`.
    pub song: Option<SongInfo>,
    /// Difficulty within the current folder (mirrors `Selection.difficulty`).
    pub difficulty: u8,
    /// Active sort mode.
    pub sort_mode: SortMode,
    /// Search query (empty = no filter).
    pub search_query: String,
    /// Visible folders after sort + filter (one row per folder).
    pub visible: Vec<SongFolderView>,
    /// Set by callers (Tab, F5) to trigger `recompute`.
    pub dirty: bool,
}

impl SongSelectSelection {
    pub fn matches_search(&self, song: &SongInfo) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        let q = self.search_query.to_lowercase();
        song.title.to_lowercase().contains(&q) || song.artist.to_lowercase().contains(&q)
    }

    /// Group charts by parent folder, sort within folder by `dlevel`
    /// ascending. Top-level sort mode applies to the folder list.
    pub fn recompute(&mut self, all: &[SongInfo]) {
        use std::collections::BTreeMap;
        let mut by_folder: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
        for (idx, song) in all.iter().enumerate() {
            if !self.matches_search(song) {
                continue;
            }
            let folder = song
                .path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            by_folder.entry(folder).or_default().push(idx);
        }
        let mut v: Vec<SongFolderView> = by_folder
            .into_iter()
            .map(|(folder, mut indices)| {
                indices.sort_by(|&a, &b| {
                    let la = all[a].dlevel.unwrap_or(u32::MAX);
                    let lb = all[b].dlevel.unwrap_or(u32::MAX);
                    la.cmp(&lb).then_with(|| a.cmp(&b))
                });
                let title = all[indices[0]].title.clone();
                let artist = all[indices[0]].artist.clone();
                SongFolderView {
                    folder,
                    title,
                    artist,
                    chart_indices: indices,
                }
            })
            .collect();
        match self.sort_mode {
            SortMode::Default => {}
            SortMode::ByTitle => v.sort_by(|a, b| a.title.cmp(&b.title)),
            SortMode::ByArtist => v.sort_by(|a, b| a.artist.cmp(&b.title)),
        }
        self.visible = v;
    }
}

// ===== CommandHistory (CommandHistory.cs) =====

/// One pad command entry (CommandHistory.cs:11-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEntry {
    /// 0=Drums, 1=Guitar, 2=Bass.
    pub instrument: u8,
    /// Pad bitflag (1=LC, 2=HH, 4=SD, 8=BD, ...).
    pub pad: u16,
    /// Time of input (ms, audio clock).
    pub time_ms: i64,
}

/// 16-deep ring buffer of pad inputs (CommandHistory.cs:18-19, 33-46).
#[derive(Resource, Debug, Clone, Default)]
pub struct CommandHistory {
    entries: Vec<CommandEntry>,
}

impl CommandHistory {
    /// Append a new command. Removes oldest if at capacity (CommandHistory.cs:33-46).
    pub fn add(&mut self, instrument: u8, pad: u16, time_ms: i64) {
        if self.entries.len() >= COMMAND_HISTORY_BUF {
            self.entries.remove(0);
        }
        self.entries.push(CommandEntry {
            instrument,
            pad,
            time_ms,
        });
    }

    /// Returns true if `pattern` (sequence of pad flags) was just entered for
    /// `instrument` within the last 500ms (StatusPanel.cs:36-50 logic).
    pub fn check_command(&self, instrument: u8, pattern: &[u16], time_ms: i64) -> bool {
        if pattern.is_empty() || self.entries.len() < pattern.len() {
            return false;
        }
        let recent: Vec<&CommandEntry> = self
            .entries
            .iter()
            .rev()
            .take_while(|e| e.instrument == instrument && time_ms - e.time_ms <= 500)
            .collect();
        if recent.len() < pattern.len() {
            return false;
        }
        recent
            .iter()
            .rev()
            .zip(pattern.iter())
            .all(|(e, p)| e.pad == *p)
    }
}

// ===== Components =====

#[derive(Component)]
pub struct SongSelectEntity;

#[derive(Component)]
struct SongRowEntity {
    index: usize,
}

#[derive(Component)]
struct SortModeText;

#[derive(Component)]
struct SelectedSongInfo;

/// Cursor into the song-select list. Two-level: which song folder,
/// which chart inside it (the latter is the difficulty index).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Selection {
    /// Index into `SongSelectSelection.visible` (one per folder).
    pub folder: usize,
    /// Index into the folder's `chart_indices` (0 = easiest).
    pub difficulty: u8,
}

impl Selection {
    /// Resolve the cursor to a concrete `db.songs` chart index, or
    /// `None` if the visible list is empty / out of bounds.
    pub fn chart_index(&self, sel: &SongSelectSelection) -> Option<usize> {
        sel.visible
            .get(self.folder)
            .and_then(|f| f.chart_indices.get(self.difficulty as usize))
            .copied()
    }

    /// Clamp folder + difficulty to whatever is currently visible.
    /// Called after `recompute` so a Tab/sort/rescan that shrinks the
    /// visible list can't leave the cursor dangling.
    pub fn clamp_to_visible(&mut self, sel: &SongSelectSelection) {
        if sel.visible.is_empty() {
            self.folder = 0;
            self.difficulty = 0;
            return;
        }
        if self.folder >= sel.visible.len() {
            self.folder = sel.visible.len() - 1;
        }
        let count = sel.visible[self.folder].difficulty_count();
        if count == 0 {
            self.difficulty = 0;
        } else if (self.difficulty as usize) >= count {
            self.difficulty = (count - 1) as u8;
        }
    }
}

/// Sort menu element (one slot in the ring buffer).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortMenuElement {
    pub mode: SortMode,
    /// Animation offset (0 = centered, ±1 = neighbor).
    pub offset: i8,
}

/// Mark the entity holding the sort menu container UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct SortMenuContainerComp;

/// Mark the entity holding the search menu UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct SongSearchMenuComp;

/// Mark the entity holding the density graph UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct DensityGraphComp;

/// Overlay chrome hidden until SongSelect (ADR-0014).
#[derive(Component, Debug, Clone, Copy)]
struct SongSelectOverlay;

/// Mark the entity holding the status panel UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusPanelComp;

/// Mark the entity holding the album art image (ADR-0015 item e).
/// Used by `update_album_art_image` to find the entity and swap its
/// image on selection change.
#[derive(Component, Debug, Clone, Copy)]
pub struct AlbumArtEntity;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusPaneKind {
    Drums,
    Guitar,
    Bass,
}

// ===== Plugin =====

pub fn plugin(app: &mut App) {
    app.init_resource::<SelectedSong>()
        .init_resource::<SongSelectSelection>()
        .init_resource::<CommandHistory>()
        .init_resource::<Selection>()
        .add_systems(Startup, spawn_song_select_overlay)
        .add_systems(
            OnEnter(AppState::SongSelect),
            (
                ensure_song_db_loaded,
                recompute_visible,
                spawn_song_select,
                show_song_select_overlay,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppState::SongSelect),
            (
                hide_song_select_overlay,
                stop_preview_system,
                stop_bgm_system,
                despawn_song_select,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                maybe_recompute_visible,
                song_select_navigation,
                render_selected_song,
                bgm_preview_on_change,
                update_album_art_image,
                update_status_panes,
                update_density_graph,
                update_search_filter,
            )
                .run_if(in_state(AppState::SongSelect)),
        );
}

// ===== M5: song list logic (OnEnter/OnExit) =====

/// Default song directory to scan when SongDb is empty.
/// Delegates to `dtx_library::default_song_dir` (XDG-aware, env-overridable).
fn default_song_dir() -> PathBuf {
    dtx_library::default_song_dir()
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

fn spawn_song_select(
    mut commands: Commands,
    db: Res<SongDb>,
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    theme: Res<ThemeResource>,
) {
    let t = theme.0;
    commands
        .spawn((
            SongSelectEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-48.0),
                    top: Val::Px(64.0),
                    width: Val::Px(560.0),
                    height: Val::Px(420.0),
                    ..default()
                },
                BackgroundColor(t.bg_top.with_alpha(0.28)),
            ));

            root.spawn((Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },))
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("Song Select"),
                        Theme::font(36.0),
                        TextColor(t.accent),
                    ));
                    hdr.spawn((
                    Text::new(
                        "↑↓: Navigate  ENTER: Play  TAB: Sort  F5: Refresh  F1: Config  ESC: Title",
                    ),
                    Theme::font(14.0),
                    TextColor(t.text_secondary),
                ));
                    hdr.spawn((
                        Text::new(format!("Sort: {:?}", db.sort_mode)),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                        SortModeText,
                    ));
                });

            root.spawn((Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                padding: UiRect {
                    left: Val::Px(24.0),
                    right: Val::Px(24.0),
                    bottom: Val::Px(24.0),
                    top: Val::Px(0.0),
                },
                ..default()
            },))
                .with_children(|body| {
                    body.spawn((
                        Node {
                            width: Val::Px(96.0),
                            height: Val::Px(520.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::End,
                            justify_content: JustifyContent::SpaceEvenly,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(t.panel_bg),
                    ))
                    .with_children(|density| {
                        for i in 0..DENSITY_GRAPH_BAR_COUNT {
                            let frac = 0.25 + (i as f32 * 0.09).min(0.55);
                            density.spawn((
                                Node {
                                    width: Val::Px(DENSITY_GRAPH_BAR_W + 4.0),
                                    height: Val::Px(DENSITY_GRAPH_BAR_H * frac),
                                    ..default()
                                },
                                BackgroundColor(t.accent.with_alpha(0.65)),
                            ));
                        }
                    });

                    body.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            max_height: Val::Px(520.0),
                            overflow: Overflow::scroll_y(),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(t.panel_bg),
                    ))
                    .with_children(|list| {
                        for (i, folder) in selection_state.visible.iter().enumerate() {
                            let label = format!(
                                "{} \u{2014} {} [{}]",
                                folder.title,
                                folder.artist,
                                SongFolderView::difficulty_label(selection.difficulty)
                            );
                            list.spawn((
                                SongRowEntity { index: i },
                                Node {
                                    width: Val::Percent(100.0),
                                    min_height: Val::Px(36.0),
                                    margin: UiRect::vertical(Val::Px(2.0)),
                                    padding: UiRect::all(Val::Px(8.0)),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(if i == selection.folder {
                                    t.selection_highlight
                                } else {
                                    Color::NONE
                                }),
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    Text::new(label.clone()),
                                    Theme::font(18.0),
                                    TextColor(t.text_primary),
                                ));
                            });
                        }
                    });

                    body.spawn((
                        Node {
                            width: Val::Px(280.0),
                            height: Val::Px(520.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.0)),
                            row_gap: Val::Px(12.0),
                            ..default()
                        },
                        BackgroundColor(t.panel_bg),
                    ))
                    .with_children(|info| {
                        // Album-art crossfade placeholder. Holds both
                        // an ImageNode (for #PREIMAGE: when present) and
                        // a BackgroundColor (placeholder when not). The
                        // `update_album_art_image` system toggles which
                        // is visible based on `song.preimage_path`.
                        info.spawn((
                            Node {
                                width: Val::Px(ALBUM_ART_W),
                                height: Val::Px(ALBUM_ART_H),
                                margin: UiRect::bottom(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(t.accent.with_alpha(0.18)),
                            ImageNode {
                                color: Color::WHITE.with_alpha(0.0),
                                ..default()
                            },
                            AlbumArt::default(),
                            AlbumArtEntity,
                        ));
                        info.spawn((
                            Text::new("Chart info"),
                            Theme::font(20.0),
                            TextColor(t.accent),
                        ));
                        let detail = selection
                            .chart_index(&selection_state)
                            .and_then(|i| db.songs.get(i))
                            .map(format_song_detail)
                            .unwrap_or_else(|| "No songs in library.\nF5 to rescan.".into());
                        info.spawn((
                            SelectedSongInfo,
                            Text::new(detail),
                            Theme::font(14.0),
                            TextColor(t.text_primary),
                        ));
                    });
                });
        });
}

fn show_song_select_overlay(mut q: Query<&mut Visibility, With<SongSelectOverlay>>) {
    for mut vis in &mut q {
        *vis = Visibility::Visible;
    }
}

fn hide_song_select_overlay(mut q: Query<&mut Visibility, With<SongSelectOverlay>>) {
    for mut vis in &mut q {
        *vis = Visibility::Hidden;
    }
}

fn format_song_detail(song: &dtx_library::SongInfo) -> String {
    let mut detail = format!(
        "Title:  {}\nArtist: {}\nBPM:    {}\nLevel:  {}\nNotes:  {}",
        song.title,
        song.artist,
        song.bpm
            .map(|v| format!("{v}"))
            .unwrap_or_else(|| "?".into()),
        song.dlevel
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into()),
        song.notes_total(),
    );
    // BocuD-compatible best score, read from <chart>.score.ini if present.
    let ini_path = dtx_scoring::score_ini::score_ini_path(&song.path);
    if let Some(best) = dtx_scoring::score_ini::read_best(&ini_path) {
        detail.push_str(&format!(
            "\nBest:   {} ({})  x{}",
            best.score, best.rank, best.max_combo
        ));
    }
    detail
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

fn song_select_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut db: ResMut<SongDb>,
    mut selection: ResMut<Selection>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut selected_song: ResMut<SelectedSong>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if selection_state.visible.is_empty() {
        if keys.just_pressed(KeyCode::F5) {
            if let Err(e) = db.rescan(&default_song_dir()) {
                warn!("SongSelect: refresh failed: {}", e);
            }
        }
        return;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        let max = selection_state.visible.len() - 1;
        selection.folder = (selection.folder + 1).min(max);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.folder = selection.folder.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        if let Some(folder) = selection_state.visible.get(selection.folder) {
            let count = folder.difficulty_count();
            if count > 0 {
                selection.difficulty = ((selection.difficulty as usize + 1) % count) as u8;
            }
        }
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        if let Some(folder) = selection_state.visible.get(selection.folder) {
            let count = folder.difficulty_count();
            if count > 0 {
                selection.difficulty =
                    ((selection.difficulty as usize + count - 1) % count) as u8;
            }
        }
    } else if keys.just_pressed(KeyCode::Tab) {
        selection_state.sort_mode = selection_state.sort_mode.next();
        selection_state.dirty = true;
    } else if keys.just_pressed(KeyCode::Enter) {
        if let Some(chart_idx) = selection.chart_index(&selection_state)
            && let Some(song) = db.songs.get(chart_idx)
        {
            info!(
                "SongSelect: selected {} ({})",
                song.title,
                SongFolderView::difficulty_label(selection.difficulty)
            );
            selected_song.0 = Some(song.path.clone());
            request_transition(&mut requests, AppState::SongLoading);
        }
    } else if keys.just_pressed(KeyCode::Escape) {
        request_transition(&mut requests, AppState::Title);
    } else if keys.just_pressed(KeyCode::F1) {
        request_transition(&mut requests, AppState::Config);
    } else if keys.just_pressed(KeyCode::F5) {
        if let Err(e) = db.rescan(&default_song_dir()) {
            warn!("SongSelect: refresh failed: {}", e);
        }
    }
}

fn render_selected_song(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    theme: Res<ThemeResource>,
    mut query: Query<&mut Text, With<SelectedSongInfo>>,
    mut rows: Query<(&SongRowEntity, &mut BackgroundColor)>,
) {
    if !selection.is_changed() {
        return;
    }
    let t = theme.0;
    if let Some(chart_idx) = selection.chart_index(&selection_state)
        && let Some(song) = db.songs.get(chart_idx)
    {
        let detail = format_song_detail(song);
        for mut text in &mut query {
            *text = Text::new(detail.clone());
        }
    }
    for (row_entity, mut bg) in &mut rows {
        bg.0 = if row_entity.index == selection.folder {
            t.selection_highlight
        } else {
            t.panel_bg
        };
    }
}

fn update_album_art_image(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    asset_server: Res<AssetServer>,
    mut query: Query<(&AlbumArtEntity, &mut ImageNode, &mut BackgroundColor)>,
) {
    if !selection.is_changed() {
        return;
    }
    let Some(chart_idx) = selection.chart_index(&selection_state) else {
        return;
    };
    let Some(song) = db.songs.get(chart_idx) else {
        return;
    };
    for (_, mut image, mut bg) in &mut query {
        if let Some(path) = &song.preimage_path {
            // Real #PREIMAGE: present: show the image, hide the placeholder.
            image.image = asset_server.load(path.to_string_lossy().to_string());
            image.color = image.color.with_alpha(1.0);
            bg.0 = bg.0.with_alpha(0.0);
        } else {
            // No image: show the placeholder, hide the image.
            image.image = Handle::default();
            image.color = image.color.with_alpha(0.0);
            bg.0 = bg.0.with_alpha(0.18);
        }
    }
}

fn stop_preview_system(
    mut player: ResMut<PreviewPlayer>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    // Align with the 300ms OutQuint screen fade (ADR-0014, dtx-ui).
    // By the time the new screen's fade-in completes, the preview is
    // silent and ready for the new BGM. (ADR-0015 Phase 4.)
    player.stop(&mut instances, 300);
    player.previous_index = None;
}

fn bgm_preview_on_change(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut bgm: ResMut<BgmHandle>,
    mut cache: ResMut<dtx_audio::AudioHandleCache>,
    mut player: ResMut<PreviewPlayer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut events: MessageWriter<PreviewSwapEvent>,
) {
    if !selection.is_changed() {
        return;
    }
    let Some(chart_idx) = selection.chart_index(&selection_state) else {
        info!(
            "SongSelect preview: no chart for folder={} difficulty={}",
            selection.folder, selection.difficulty
        );
        return;
    };
    let Some(song) = db.songs.get(chart_idx) else {
        info!("SongSelect preview: chart_idx={} missing from SongDb", chart_idx);
        return;
    };
    info!(
        "SongSelect preview: selected folder={} difficulty={} chart_idx={} title={}",
        selection.folder, selection.difficulty, chart_idx, song.title
    );
    let Some(preview_path) = song.preview_path.clone() else {
        // No preview for this song: stop whatever's currently
        // playing so we don't leak a stale preview from a prior
        // selection. (`stop` releases the kira instance via
        // `stop_with_fade` — see PreviewPlayer docs.)
        info!("SongSelect preview: no preview path; stopping current preview");
        player.stop(&mut instances, 0);
        return;
    };

    // Clear gameplay BGM before starting preview. If the tracked
    // handle is stale, `stop_bgm` may fall back to `audio.stop()`;
    // doing it first avoids killing the newly-started preview.
    if bgm.instance.is_some() {
        info!("SongSelect preview: stopping stale BgmHandle before preview");
        dtx_audio::stop_bgm(&audio, &mut bgm, &mut instances);
    }
    info!(
        "SongSelect preview: request path={} loopable={}",
        preview_path.display(), song.preview_is_loopable
    );

    // Loop flag follows the source: #PREVIEW: file loops (short
    // clip), fallback to full BGM plays through. (ADR-0015 Q1.)
    player.set_looping(song.preview_is_loopable);

    // Direction uses the folder index, not the absolute chart index,
    // so cycling difficulty within the same folder reads as "None".
    let direction = match player.previous_index {
        None => PreviewSwapDirection::None,
        Some(prev) if selection.folder > prev => PreviewSwapDirection::Next,
        Some(prev) if selection.folder < prev => PreviewSwapDirection::Prev,
        Some(_) => PreviewSwapDirection::None,
    };
    let source = get_or_load_audio_handle(&mut cache, &asset_server, &preview_path);
    let accepted = player.play(
        &audio,
        source,
        preview_path,
        &mut events,
        direction,
        &mut instances,
    );
    info!(
        "SongSelect preview: play accepted={} path={}",
        accepted,
        player
            .current_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    if accepted {
        player.previous_index = Some(selection.folder);
    }
}

// ===== Strict-port overlay: status panel / density / sort / search (Startup) =====

fn spawn_song_select_overlay(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;

    for kind in [
        StatusPaneKind::Drums,
        StatusPaneKind::Guitar,
        StatusPaneKind::Bass,
    ] {
        let (x, y) = match kind {
            StatusPaneKind::Drums => (STATUS_PANEL_DRUMS_X, STATUS_PANEL_DRUMS_Y),
            StatusPaneKind::Guitar => (STATUS_PANEL_GUITAR_X, STATUS_PANEL_GUITAR_Y),
            StatusPaneKind::Bass => (STATUS_PANEL_DRUMS_X, STATUS_PANEL_DRUMS_Y),
        };
        commands.spawn((
            SongSelectOverlay,
            Visibility::Hidden,
            StatusPanelComp,
            kind,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(220.0),
                height: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(t.panel_bg),
            Text::new("(no song)"),
            Theme::font(14.0),
            TextColor(t.text_secondary),
        ));
    }

    commands.spawn((
        SongSelectOverlay,
        Visibility::Hidden,
        DensityGraphComp,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(200.0),
            height: Val::Px(350.0),
            ..default()
        },
    ));
    for i in 0..DENSITY_GRAPH_BAR_COUNT {
        commands.spawn((
            SongSelectOverlay,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(DENSITY_GRAPH_BAR_BASE_X + (i as f32) * DENSITY_GRAPH_BAR_DX),
                top: Val::Px(DENSITY_GRAPH_BAR_BASE_Y - DENSITY_GRAPH_BAR_H),
                width: Val::Px(DENSITY_GRAPH_BAR_W),
                height: Val::Px(DENSITY_GRAPH_BAR_H * 0.5),
                ..default()
            },
            BackgroundColor(t.accent.with_alpha(0.75)),
        ));
    }

    let sorters = [SortMode::Default, SortMode::ByTitle, SortMode::ByArtist];
    commands.spawn((
        SongSelectOverlay,
        Visibility::Hidden,
        SortMenuContainerComp,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(SORT_MENU_W),
            height: Val::Px(SORT_MENU_H),
            flex_direction: FlexDirection::Row,
            ..default()
        },
        BackgroundColor(t.panel_bg),
    ));
    for (i, mode) in sorters.iter().enumerate() {
        let offset = i as i8 - 2;
        commands.spawn((
            SongSelectOverlay,
            Visibility::Hidden,
            SortMenuElement {
                mode: *mode,
                offset,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(SORT_MENU_ELEMENT_SPACING * (i as f32) - 30.0),
                top: Val::Px(40.0),
                width: Val::Px(80.0),
                height: Val::Px(40.0),
                ..default()
            },
            BackgroundColor(t.selection_highlight),
            Text::new(mode_label(*mode)),
            Theme::font(18.0),
            TextColor(t.text_primary),
        ));
    }

    commands.spawn((
        SongSelectOverlay,
        Visibility::Hidden,
        SongSearchMenuComp,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(380.0),
            top: Val::Px(150.0),
            width: Val::Px(SONG_SEARCH_W),
            height: Val::Px(SONG_SEARCH_H),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(t.panel_bg),
        Text::new("Search\n\n(description)\n\nStatus: (typing...)"),
        Theme::font(16.0),
        TextColor(t.text_secondary),
    ));
}

fn mode_label(m: SortMode) -> &'static str {
    match m {
        SortMode::Default => "Default",
        SortMode::ByTitle => "Title",
        SortMode::ByArtist => "Artist",
    }
}

fn update_status_panes(
    sel: Res<SongSelectSelection>,
    mut q: Query<(&StatusPaneKind, &mut Text), With<StatusPanelComp>>,
) {
    if !sel.is_changed() {
        return;
    }
    let song = sel.song.as_ref();
    for (kind, mut text) in &mut q {
        let label = match kind {
            StatusPaneKind::Drums => "Drums",
            StatusPaneKind::Guitar => "Guitar",
            StatusPaneKind::Bass => "Bass",
        };
        *text = Text::new(match song {
            Some(s) => format!(
                "{}\n{}\nBPM: {}\nLevel: {}\nNotes: {}\n({})",
                label,
                s.title,
                s.bpm
                    .map(|v| format!("{:.1}", v))
                    .unwrap_or_else(|| "?".into()),
                s.dlevel
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
                s.notes_total(),
                kind_str(*kind)
            ),
            None => format!("{}\n(no song)", label),
        });
    }
}

fn kind_str(k: StatusPaneKind) -> &'static str {
    match k {
        StatusPaneKind::Drums => "drums",
        StatusPaneKind::Guitar => "guitar",
        StatusPaneKind::Bass => "bass",
    }
}

fn update_density_graph(
    sel: Res<SongSelectSelection>,
    bars: Query<
        Entity,
        (
            With<Node>,
            Without<StatusPanelComp>,
            Without<SortMenuContainerComp>,
            Without<StatusPaneKind>,
        ),
    >,
) {
    let _ = sel;
    let _ = bars;
}

fn update_search_filter(mut sel: ResMut<SongSelectSelection>) {
    if sel.is_changed() && sel.search_query.len() > 64 {
        sel.search_query.truncate(64);
    }
}

/// On SongSelect entry: dedupe the chart list into folders and clamp
/// the cursor to whatever's actually visible. Runs after
/// `ensure_song_db_loaded` (which may have just scanned the dir) and
/// before `spawn_song_select` reads `selection_state.visible`.
fn recompute_visible(
    mut sel: ResMut<SongSelectSelection>,
    db: Res<SongDb>,
    mut selection: ResMut<Selection>,
) {
    sel.recompute(&db.songs);
    selection.clamp_to_visible(&sel);
}

/// On Update: re-run recompute when the dirty flag is set (Tab cycles
/// sort mode, future search-input wiring) or `db.songs` was mutated
/// (F5 rescan).
fn maybe_recompute_visible(
    mut sel: ResMut<SongSelectSelection>,
    db: Res<SongDb>,
    mut selection: ResMut<Selection>,
) {
    if sel.dirty || db.is_changed() {
        sel.recompute(&db.songs);
        sel.dirty = false;
        selection.clamp_to_visible(&sel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_song(title: &str, artist: &str) -> SongInfo {
        SongInfo {
            // Per-title folder so each test song ends up in a distinct
            // group after `recompute`. Otherwise all `/X.dtx` paths
            // share parent `/` and dedupe to one row.
            path: std::path::PathBuf::from(format!("/{title}/{title}.dtx")),
            title: title.into(),
            artist: artist.into(),
            bpm: Some(120.0),
            dlevel: Some(50),
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: None,
        }
    }

    // Position / layout constants (from old song_select_full.rs)
    #[test]
    fn status_panel_positions_match_reference() {
        // StatusPanel.cs:9-13
        assert_eq!(STATUS_PANEL_DRUMS_X, 430.0);
        assert_eq!(STATUS_PANEL_DRUMS_Y, 720.0);
        assert_eq!(STATUS_PANEL_GUITAR_X, 200.0);
    }

    #[test]
    fn density_graph_geometry_matches_reference() {
        assert_eq!(DENSITY_GRAPH_BAR_COUNT, 8);
        assert_eq!(DENSITY_GRAPH_BAR_DX, 12.0);
        assert_eq!(DENSITY_GRAPH_BAR_W, 4.0);
        assert_eq!(DENSITY_GRAPH_BAR_H, 252.0);
        assert_eq!(DENSITY_GRAPH_BAR_BASE_X, 36.0);
        assert_eq!(DENSITY_GRAPH_BAR_BASE_Y, 284.0);
        assert_eq!(DENSITY_NOTE_TEXT_DRUMS, (150.0, 333.0));
    }

    #[test]
    fn sort_menu_constants_match_reference() {
        assert_eq!(SORT_MENU_W, 662.0);
        assert_eq!(SORT_MENU_H, 92.0);
        assert_eq!(SORT_MENU_ELEMENT_SPACING, 90.0);
    }

    #[test]
    fn song_search_constants_match_reference() {
        assert_eq!(SONG_SEARCH_W, 500.0);
        assert_eq!(SONG_SEARCH_H, 300.0);
        assert_eq!(SONG_SEARCH_TEXT_INPUT_Y, 30.0);
        assert_eq!(SONG_SEARCH_DESC_Y, 60.0);
        assert_eq!(SONG_SEARCH_STATUS_Y, 250.0);
    }

    #[test]
    fn command_history_buffer_size() {
        assert_eq!(COMMAND_HISTORY_BUF, 16);
    }

    #[test]
    fn command_history_add_overflows() {
        let mut h = CommandHistory::default();
        for i in 0..20 {
            h.add(0, 1, i * 10);
        }
        assert_eq!(h.entries.len(), COMMAND_HISTORY_BUF);
        assert_eq!(h.entries[0].time_ms, 40);
        assert_eq!(h.entries[COMMAND_HISTORY_BUF - 1].time_ms, 190);
    }

    #[test]
    fn command_history_check_command_basic() {
        let mut h = CommandHistory::default();
        h.add(0, 2, 100);
        h.add(0, 2, 150);
        assert!(h.check_command(0, &[2, 2], 200));
        assert!(!h.check_command(0, &[2, 4], 200));
    }

    #[test]
    fn command_history_check_too_old() {
        let mut h = CommandHistory::default();
        h.add(0, 2, 0);
        h.add(0, 2, 100);
        assert!(!h.check_command(0, &[2, 2], 700));
    }

    #[test]
    fn search_query_matches_substring() {
        let sel = SongSelectSelection {
            search_query: "abc".into(),
            ..Default::default()
        };
        assert!(sel.matches_search(&make_song("ABCDEF", "X")));
        assert!(sel.matches_search(&make_song("X", "ABcdef")));
        assert!(!sel.matches_search(&make_song("X", "Y")));
    }

    #[test]
    fn search_query_empty_matches_all() {
        let sel = SongSelectSelection::default();
        assert!(sel.matches_search(&make_song("A", "B")));
    }

    #[test]
    fn recompute_sorts_by_title() {
        let mut sel = SongSelectSelection {
            sort_mode: SortMode::ByTitle,
            ..Default::default()
        };
        let all = vec![
            make_song("Charlie", "X"),
            make_song("Alpha", "Y"),
            make_song("Bravo", "Z"),
        ];
        sel.recompute(&all);
        // Each test song has a per-title folder → 3 distinct rows.
        assert_eq!(sel.visible.len(), 3);
        assert_eq!(sel.visible[0].title, "Alpha");
        assert_eq!(sel.visible[1].title, "Bravo");
        assert_eq!(sel.visible[2].title, "Charlie");
    }

    #[test]
    fn recompute_dedupes_same_folder_into_one_row() {
        let mut sel = SongSelectSelection::default();
        let chart = |file: &str, level: u32| SongInfo {
            path: std::path::PathBuf::from(format!("/songs/Alpha/{file}")),
            title: "Alpha".into(),
            artist: "X".into(),
            bpm: Some(120.0),
            dlevel: Some(level),
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: None,
        };
        let all = vec![chart("bsc.dtx", 50), chart("adv.dtx", 70), chart("mas.dtx", 95)];
        sel.recompute(&all);
        assert_eq!(sel.visible.len(), 1);
        let folder = &sel.visible[0];
        assert_eq!(folder.title, "Alpha");
        assert_eq!(folder.difficulty_count(), 3);
        // Within folder: easiest (lowest dlevel) first.
        assert_eq!(all[folder.chart_indices[0]].dlevel, Some(50));
        assert_eq!(all[folder.chart_indices[2]].dlevel, Some(95));
    }

    #[test]
    fn difficulty_label_helpers() {
        assert_eq!(SongFolderView::difficulty_label(0), "BASIC");
        assert_eq!(SongFolderView::difficulty_label(1), "ADV");
        assert_eq!(SongFolderView::difficulty_label(2), "EXT");
        assert_eq!(SongFolderView::difficulty_label(3), "MAS");
        assert_eq!(SongFolderView::difficulty_label(4), "EDIT");
    }

    #[test]
    fn recompute_filters_via_search() {
        let mut sel = SongSelectSelection {
            search_query: "bra".into(),
            ..Default::default()
        };
        let all = vec![
            make_song("Charlie", "X"),
            make_song("Alpha", "Y"),
            make_song("Bravo", "Z"),
        ];
        sel.recompute(&all);
        assert_eq!(sel.visible.len(), 1);
        assert_eq!(sel.visible[0].title, "Bravo");
    }

    #[test]
    fn sort_menu_default_count() {
        let sorters = [SortMode::Default, SortMode::ByTitle, SortMode::ByArtist];
        assert_eq!(sorters.len(), 3);
    }

    // Tests from old song_select.rs (preserved in merge).

    #[test]
    fn selected_song_resource_starts_empty() {
        let s = SelectedSong::default();
        assert!(s.0.is_none());
    }

    #[test]
    fn selection_default_is_zero() {
        let s = Selection::default();
        assert_eq!(s.folder, 0);
        assert_eq!(s.difficulty, 0);
    }

    #[test]
    fn selection_chart_index_resolves_folder_and_difficulty() {
        let songs = vec![
            SongInfo {
                path: std::path::PathBuf::from("/songs/A/bsc.dtx"),
                title: "A".into(),
                artist: "X".into(),
                bpm: Some(120.0),
                dlevel: Some(50),
                bgm_path: None,
                preview_path: None,
                preview_is_loopable: false,
                preimage_path: None,
            },
            SongInfo {
                path: std::path::PathBuf::from("/songs/A/mas.dtx"),
                title: "A".into(),
                artist: "X".into(),
                bpm: Some(120.0),
                dlevel: Some(95),
                bgm_path: None,
                preview_path: None,
                preview_is_loopable: false,
                preimage_path: None,
            },
        ];
        let mut sel_state = SongSelectSelection::default();
        sel_state.recompute(&songs);
        assert_eq!(
            Selection { folder: 0, difficulty: 0 }.chart_index(&sel_state),
            Some(0)
        );
        assert_eq!(
            Selection { folder: 0, difficulty: 1 }.chart_index(&sel_state),
            Some(1)
        );
        assert_eq!(
            Selection { folder: 1, difficulty: 0 }.chart_index(&sel_state),
            None
        );
    }

    #[test]
    fn selection_clamp_to_visible_clamps_difficulty() {
        let mut sel_state = SongSelectSelection::default();
        sel_state.visible.push(SongFolderView {
            folder: std::path::PathBuf::from("/songs/A"),
            title: "A".into(),
            artist: "X".into(),
            chart_indices: vec![0, 1],
        });
        let mut cursor = Selection { folder: 0, difficulty: 5 };
        cursor.clamp_to_visible(&sel_state);
        assert_eq!(cursor.difficulty, 1);
    }

    #[test]
    fn selection_clamp_to_visible_clamps_folder() {
        let mut sel_state = SongSelectSelection::default();
        sel_state.visible.push(SongFolderView {
            folder: std::path::PathBuf::from("/songs/A"),
            title: "A".into(),
            artist: "X".into(),
            chart_indices: vec![0],
        });
        let mut cursor = Selection { folder: 5, difficulty: 0 };
        cursor.clamp_to_visible(&sel_state);
        assert_eq!(cursor.folder, 0);
    }

    #[test]
    fn selection_clamp_to_empty_resets_to_zero() {
        let sel_state = SongSelectSelection::default();
        let mut cursor = Selection { folder: 3, difficulty: 2 };
        cursor.clamp_to_visible(&sel_state);
        assert_eq!(cursor.folder, 0);
        assert_eq!(cursor.difficulty, 0);
    }

    #[test]
    fn format_song_detail_includes_bpm_and_level() {
        let song = make_song("X", "Y");
        let s = format_song_detail(&song);
        assert!(s.contains("X"));
        assert!(s.contains("Y"));
        assert!(s.contains("120"));
        assert!(s.contains("50"));
    }

    #[test]
    #[test]
    fn make_song_with_preimage() {
        let song = SongInfo {
            path: std::path::PathBuf::from("/x.dtx"),
            title: "X".into(),
            artist: "Y".into(),
            bpm: Some(120.0),
            dlevel: Some(50),
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: Some(std::path::PathBuf::from("/x/cover.jpg")),
        };
        assert_eq!(song.preimage_path, Some(std::path::PathBuf::from("/x/cover.jpg")));
    }

    #[test]
    fn sort_mode_cycles_through_three() {
        assert_eq!(SortMode::Default.next(), SortMode::ByTitle);
        assert_eq!(SortMode::ByTitle.next(), SortMode::ByArtist);
        assert_eq!(SortMode::ByArtist.next(), SortMode::Default);
    }
}
