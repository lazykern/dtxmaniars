#![allow(clippy::type_complexity)]
//! CStageSongSelectionNew — song select screen (M5: real SongDb).
//!
//! GITADORA stage layout (2026-07-05 redesign): plain black stage,
//! left cluster (album art + skill/bpm badges), center column
//! (density graph + difficulty grid) and a right-side song wheel
//! that springs toward the selection. The visual layer sits on top
//! of the `dtx-ui` stage widgets (`stage_background`, `stage_panel`,
//! `density_graph`, `difficulty_grid`, `song_wheel`); this file wires
//! them to `SongSelectSelection`/`Selection` and the M5 song-list
//! logic below.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/CStageSongSelectionNew.cs`
//!
//! M5 ports the LOGIC: EReturnValue (Selected/ReturnToTitle/CallConfig),
//! arrow nav, BGM preview on row select (per CActSelectPresound.cs).
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
use dtx_ui::motion::EnterChoreo;
use dtx_ui::theme::Theme;
use dtx_ui::widget::album_art::AlbumArt;
use dtx_ui::widget::density_graph::spawn_density_graph;
use dtx_ui::widget::difficulty_grid::{
    DifficultyGridData, DifficultySlot, DifficultySlotLabel, DifficultySlotLevel,
    DifficultySlotPanel, DifficultySlotScore, GRID_MAX_SLOTS, level_text, score_text,
    spawn_difficulty_grid,
};
use dtx_ui::widget::song_wheel::{SongWheel, VISIBLE_HALF, WheelRow, WheelSpring, row_geometry};
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::widget::stage_panel::{BadgeValueText, panel, set_panel_selected, spawn_badge_row};
use game_shell::{AppState, TransitionRequest, despawn_stage, request_transition};

// ===== Layout constants =====

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
/// `edit.dtx`). ←/→ picks chart inside folder (clamped, no wrap).
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

/// Wheel row text (title/artist), tagged for per-frame updates.
#[derive(Component)]
struct WheelRowTitle;
#[derive(Component)]
struct WheelRowMeta;
/// Left-cluster dynamic texts.
#[derive(Component)]
struct SearchText;
#[derive(Component)]
struct SortChipText;
/// Big art panel in the left column.
#[derive(Component)]
struct BigAlbumArt;

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

/// Mark the entity holding the album art image (ADR-0015 item e).
/// Used by `update_album_art_image` to find the entity and swap its
/// image on selection change.
#[derive(Component, Debug, Clone, Copy)]
pub struct AlbumArtEntity;

// ===== Type-to-search (Task 12) =====

pub fn apply_search_char(query: &mut String, c: char) {
    if query.len() >= 64 || c.is_control() {
        return;
    }
    query.push(c);
}

pub fn apply_search_backspace(query: &mut String) {
    query.pop();
}

// ===== Plugin =====

pub fn plugin(app: &mut App) {
    app.init_resource::<SelectedSong>()
        .init_resource::<SongSelectSelection>()
        .init_resource::<CommandHistory>()
        .init_resource::<Selection>()
        .add_systems(
            OnEnter(AppState::SongSelect),
            (
                ensure_song_db_loaded,
                reset_search,
                recompute_visible,
                reset_wheel_spring,
                spawn_song_select,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppState::SongSelect),
            (
                stop_preview_system,
                stop_bgm_system,
                despawn_stage::<SongSelectEntity>,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                maybe_recompute_visible,
                song_select_navigation,
                search_input,
                respawn_wheel_on_change,
                wheel_layout_system,
                update_left_cluster,
                render_difficulty_grid,
                bgm_preview_on_change,
                update_album_art_image,
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

/// Clear the search query on screen entry so a stale filter from a
/// previous visit doesn't hide songs. Runs before `recompute_visible`
/// so the cleared query is applied when the visible list is first
/// computed.
fn reset_search(mut sel: ResMut<SongSelectSelection>) {
    sel.search_query.clear();
    sel.dirty = true;
}

/// Reset the wheel spring to the (post-clamp) selected folder so the
/// wheel doesn't animate in from a stale position left over from the
/// previous visit to this screen.
fn reset_wheel_spring(selection: Res<Selection>, mut spring: ResMut<WheelSpring>) {
    spring.0 = dtx_ui::motion::SpringValue::wheel(selection.folder as f32);
}

fn spawn_song_select(
    mut commands: Commands,
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
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);

            // ---- top bar
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(52.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, -52.0), 0.0, 200.0),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new("DTXMANIARS"),
                    Theme::font(22.0),
                    TextColor(t.text_primary),
                ));
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|chips| {
                    chips.spawn((
                        SearchText,
                        Text::new("type to search…"),
                        Theme::font(13.0),
                        TextColor(t.text_secondary),
                    ));
                    chips
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(t.select_yellow),
                        ))
                        .with_children(|c| {
                            c.spawn((
                                SortChipText,
                                Text::new("SORT: DEFAULT"),
                                Theme::font(12.0),
                                TextColor(Color::BLACK),
                            ));
                        });
                });
            });

            // ---- left column: art + skill/bpm
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(64.0),
                    width: Val::Px(300.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 30.0, 220.0),
            ))
            .with_children(|left| {
                left.spawn((
                    BigAlbumArt,
                    AlbumArt::default(),
                    AlbumArtEntity,
                    panel(
                        &t,
                        Node {
                            width: Val::Px(300.0),
                            height: Val::Px(300.0),
                            ..default()
                        },
                    ),
                    ImageNode {
                        color: Color::WHITE.with_alpha(0.0),
                        ..default()
                    },
                ));
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "SKILL BY SONG", "0.00", true);
                });
                left.spawn(panel(
                    &t,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|p| {
                    spawn_badge_row(p, &t, "BPM", "---", false);
                });
            });

            // ---- center column: density graph + difficulty grid
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(336.0),
                    top: Val::Px(64.0),
                    width: Val::Px(280.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(-340.0, 0.0), 60.0, 220.0),
            ))
            .with_children(|center| {
                center
                    .spawn(panel(
                        &t,
                        Node {
                            width: Val::Px(120.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                    ))
                    .with_children(|p| spawn_density_graph(p, &t));
                center
                    .spawn(Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|p| spawn_difficulty_grid(p, &t));
            });

            // ---- right: song wheel container (rows spawned separately)
            root.spawn((
                SongWheel,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(52.0),
                    width: Val::Px(620.0),
                    height: Val::Px(632.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|wheel| {
                spawn_wheel_rows(wheel, &selection_state, &t);
            });

            // ---- bottom hint bar
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(18.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 34.0), 0.0, 200.0),
            ))
            .with_children(|bar| {
                for (label, hot) in [
                    ("↑↓ SELECT", false),
                    ("←→ DIFFICULTY", false),
                    ("ENTER PLAY", true),
                    ("TAB SORT", false),
                    ("F5 RESCAN", false),
                    ("F1 SETTINGS", false),
                    ("ESC BACK", false),
                ] {
                    bar.spawn((
                        Text::new(label),
                        Theme::font(12.0),
                        TextColor(if hot {
                            t.select_yellow
                        } else {
                            t.text_secondary
                        }),
                    ));
                }
            });
        });
}

/// Spawn one absolute-positioned row per visible folder. Positions are
/// written every frame by `wheel_layout_system`.
fn spawn_wheel_rows(
    wheel: &mut ChildSpawnerCommands,
    selection_state: &SongSelectSelection,
    t: &Theme,
) {
    if selection_state.visible.is_empty() {
        wheel.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(60.0),
                top: Val::Px(280.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(t.stage_panel_bg),
            Text::new(format!(
                "no songs found — put song folders in {}\npress F5 to rescan",
                dtx_library::default_song_dir().display()
            )),
            Theme::font(16.0),
            TextColor(t.text_secondary),
        ));
        return;
    }
    for (i, folder) in selection_state.visible.iter().enumerate() {
        wheel
            .spawn((
                WheelRow { index: i },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(600.0),
                    height: Val::Px(dtx_ui::widget::song_wheel::ROW_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    padding: UiRect::horizontal(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.stage_panel_border),
                BoxShadow::new(
                    Color::NONE,
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                ),
                Visibility::Hidden,
            ))
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        WheelRowTitle,
                        Text::new(folder.title.clone()),
                        Theme::font(19.0),
                        TextColor(t.text_primary),
                    ));
                    col.spawn((
                        WheelRowMeta,
                        Text::new(folder.artist.clone()),
                        Theme::font(12.0),
                        TextColor(t.text_secondary),
                    ));
                });
            });
    }
}

/// Drive the wheel spring toward the selected index and lay out rows.
/// The selected row's yellow glow pulses in time with the previewed
/// song's BPM (audio-reactive, spec 2026-07-05).
fn wheel_layout_system(
    time: Res<Time>,
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    theme: Res<ThemeResource>,
    mut spring: ResMut<WheelSpring>,
    mut phase: Local<f32>,
    mut rows: Query<(
        &WheelRow,
        &mut Node,
        &mut Visibility,
        &mut BorderColor,
        &mut BoxShadow,
        &mut BackgroundColor,
    )>,
) {
    let t = theme.0;
    spring.0.set_target(selection.folder as f32);
    spring.0.tick(time.delta_secs());
    let center = spring.0.value;

    // Advance the beat phase using the selected chart's BPM.
    let bpm = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .and_then(|s| s.bpm)
        .unwrap_or(120.0)
        .max(1.0);
    *phase = (*phase + time.delta_secs() * bpm / 60.0).rem_euclid(1.0);
    let glow = 0.30 + 0.25 * (1.0 - *phase).powi(2);

    const WHEEL_H: f32 = 632.0;
    for (row, mut node, mut vis, mut border, mut shadow, mut bg) in &mut rows {
        let offset = row.index as f32 - center;
        if offset.abs() > (VISIBLE_HALF as f32 + 1.0) {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let g = row_geometry(offset);
        node.top = Val::Px(WHEEL_H / 2.0 + g.center_y - g.height / 2.0);
        node.left = Val::Px(g.indent);
        node.height = Val::Px(g.height);
        let selected = offset.abs() < 0.5;
        if selected {
            *border = BorderColor::all(t.select_yellow);
            *shadow = BoxShadow::new(
                t.select_yellow.with_alpha(glow),
                Val::Px(0.0),
                Val::Px(0.0),
                Val::Px(2.0),
                Val::Px(14.0),
            );
        } else {
            set_panel_selected(&t, false, &mut border, &mut shadow);
        }
        bg.0 = t.stage_panel_bg.with_alpha(0.93 * g.alpha);
    }
}

/// Push selection → difficulty grid, skill/bpm badges, sort chip.
fn update_left_cluster(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    mut grid: ResMut<DifficultyGridData>,
    mut badge_texts: Query<(&BadgeValueText, &mut Text)>,
    mut sort_chip: Query<&mut Text, (With<SortChipText>, Without<BadgeValueText>)>,
) {
    if !selection.is_changed() && !selection_state.is_changed() {
        return;
    }
    // difficulty grid
    let mut data = DifficultyGridData {
        selected: selection.difficulty as usize,
        ..Default::default()
    };
    if let Some(folder) = selection_state.visible.get(selection.folder) {
        for (slot_i, chart_idx) in folder.chart_indices.iter().enumerate().take(GRID_MAX_SLOTS) {
            let Some(song) = db.songs.get(*chart_idx) else {
                continue;
            };
            let ini = dtx_scoring::score_ini::score_ini_path(&song.path);
            let best = dtx_scoring::score_ini::read_best(&ini);
            data.slots[slot_i] = DifficultySlot {
                present: true,
                label: format!("DRUM · {}", SongFolderView::difficulty_label(slot_i as u8)),
                level: song.dlevel.map(|v| v as f32 / 10.0),
                achievement: best.as_ref().map(|b| b.accuracy()),
                rank: best.as_ref().map(|b| b.rank.clone()),
            };
        }
    }
    *grid = data;

    // skill + bpm badges
    let (skill, bpm) = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|song| {
            let ini = dtx_scoring::score_ini::score_ini_path(&song.path);
            let acc = dtx_scoring::score_ini::read_best(&ini)
                .map(|b| b.accuracy())
                .unwrap_or(0.0);
            (
                crate::chart_stats::skill_points(song.dlevel, acc),
                song.bpm.unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0));
    for (badge, mut text) in &mut badge_texts {
        *text = Text::new(if badge.decimals {
            format!("{skill:.2}")
        } else if bpm > 0.0 {
            format!("{}", bpm.round() as i32)
        } else {
            "---".into()
        });
    }
    for mut text in &mut sort_chip {
        *text = Text::new(format!(
            "SORT: {}",
            match selection_state.sort_mode {
                SortMode::Default => "DEFAULT",
                SortMode::ByTitle => "TITLE",
                SortMode::ByArtist => "ARTIST",
            }
        ));
    }
}

/// Write grid slot data into the widget's text/border entities.
fn render_difficulty_grid(
    grid: Res<DifficultyGridData>,
    theme: Res<ThemeResource>,
    mut panels: Query<(
        &DifficultySlotPanel,
        &mut BorderColor,
        &mut BoxShadow,
        &mut BackgroundColor,
    )>,
    mut labels: Query<
        (&DifficultySlotLabel, &mut Text),
        (Without<DifficultySlotLevel>, Without<DifficultySlotScore>),
    >,
    mut levels: Query<
        (&DifficultySlotLevel, &mut Text),
        (Without<DifficultySlotLabel>, Without<DifficultySlotScore>),
    >,
    mut scores: Query<
        (&DifficultySlotScore, &mut Text),
        (Without<DifficultySlotLabel>, Without<DifficultySlotLevel>),
    >,
) {
    if !grid.is_changed() {
        return;
    }
    let t = theme.0;
    for (panel, mut border, mut shadow, mut bg) in &mut panels {
        let slot = &grid.slots[panel.0];
        let selected = slot.present && panel.0 == grid.selected;
        set_panel_selected(&t, selected, &mut border, &mut shadow);
        bg.0 = if slot.present {
            t.stage_panel_bg
        } else {
            t.stage_panel_bg.with_alpha(0.35)
        };
    }
    for (label, mut text) in &mut labels {
        *text = Text::new(grid.slots[label.0].label.clone());
    }
    for (level, mut text) in &mut levels {
        *text = Text::new(level_text(grid.slots[level.0].level));
    }
    for (score, mut text) in &mut scores {
        *text = Text::new(score_text(&grid.slots[score.0]));
    }
}

/// When `SongSelectSelection.visible` changes (sort/search/rescan),
/// despawn the wheel row entities and respawn from the new list.
fn respawn_wheel_on_change(
    mut commands: Commands,
    selection_state: Res<SongSelectSelection>,
    theme: Res<ThemeResource>,
    wheel: Query<Entity, With<SongWheel>>,
    rows: Query<Entity, With<WheelRow>>,
) {
    if !selection_state.is_changed() {
        return;
    }
    let Ok(wheel_entity) = wheel.single() else {
        return;
    };
    for row in &rows {
        commands.entity(row).despawn();
    }
    let t = theme.0;
    commands.entity(wheel_entity).with_children(|w| {
        spawn_wheel_rows(w, &selection_state, &t);
    });
}

#[cfg(test)]
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

fn song_select_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut db: ResMut<SongDb>,
    mut selection: ResMut<Selection>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut selected_song: ResMut<SelectedSong>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if selection_state.visible.is_empty() {
        if keys.just_pressed(KeyCode::F5)
            && let Err(e) = db.rescan(&default_song_dir())
        {
            warn!("SongSelect: refresh failed: {}", e);
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
                let max = (count - 1) as u8;
                selection.difficulty = (selection.difficulty + 1).min(max);
            }
        }
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        selection.difficulty = selection.difficulty.saturating_sub(1);
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
    } else if keys.just_pressed(KeyCode::F5)
        && let Err(e) = db.rescan(&default_song_dir())
    {
        warn!("SongSelect: refresh failed: {}", e);
    }
}

/// Live type-to-search: printable keys append, Backspace deletes,
/// filter recomputes immediately. Nav/hotkeys still work (arrows,
/// Enter, Tab, F-keys, Esc are not printable characters).
fn search_input(
    mut chars: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut search_text: Query<&mut Text, With<SearchText>>,
) {
    use bevy::input::keyboard::Key;
    let mut changed = false;
    for ev in chars.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    apply_search_char(&mut selection_state.search_query, c);
                }
                changed = true;
            }
            Key::Space => {
                apply_search_char(&mut selection_state.search_query, ' ');
                changed = true;
            }
            Key::Backspace => {
                apply_search_backspace(&mut selection_state.search_query);
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        selection_state.dirty = true;
        let q = selection_state.search_query.clone();
        for mut text in &mut search_text {
            *text = Text::new(if q.is_empty() {
                "type to search…".to_string()
            } else {
                format!("search: {q}")
            });
        }
    }
}

fn update_album_art_image(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    asset_server: Res<AssetServer>,
    mut query: Query<(&AlbumArtEntity, &mut ImageNode, &mut BackgroundColor)>,
    mut ambient: Query<
        &mut ImageNode,
        (
            With<dtx_ui::widget::stage_background::AmbientArt>,
            Without<AlbumArtEntity>,
        ),
    >,
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
    for mut ambient_image in &mut ambient {
        if let Some(path) = &song.preimage_path {
            ambient_image.image = asset_server.load(path.to_string_lossy().to_string());
        } else {
            // No art: hold the ambient layer at alpha 0 (black stage).
            ambient_image.image = Handle::default();
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

#[allow(clippy::too_many_arguments)]
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
        info!(
            "SongSelect preview: chart_idx={} missing from SongDb",
            chart_idx
        );
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
        preview_path.display(),
        song.preview_is_loopable
    );

    // Loop flag follows the source: #PREVIEW: file loops (short
    // clip), fallback to full BGM plays through. (ADR-0015 Q1.)
    player.set_looping(song.preview_is_loopable);

    // Direction uses the folder index, not the absolute chart index.
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
        let all = vec![
            chart("bsc.dtx", 50),
            chart("adv.dtx", 70),
            chart("mas.dtx", 95),
        ];
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
            Selection {
                folder: 0,
                difficulty: 0
            }
            .chart_index(&sel_state),
            Some(0)
        );
        assert_eq!(
            Selection {
                folder: 0,
                difficulty: 1
            }
            .chart_index(&sel_state),
            Some(1)
        );
        assert_eq!(
            Selection {
                folder: 1,
                difficulty: 0
            }
            .chart_index(&sel_state),
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
        let mut cursor = Selection {
            folder: 0,
            difficulty: 5,
        };
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
        let mut cursor = Selection {
            folder: 5,
            difficulty: 0,
        };
        cursor.clamp_to_visible(&sel_state);
        assert_eq!(cursor.folder, 0);
    }

    #[test]
    fn selection_clamp_to_empty_resets_to_zero() {
        let sel_state = SongSelectSelection::default();
        let mut cursor = Selection {
            folder: 3,
            difficulty: 2,
        };
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
        assert_eq!(
            song.preimage_path,
            Some(std::path::PathBuf::from("/x/cover.jpg"))
        );
    }

    #[test]
    fn sort_mode_cycles_through_three() {
        assert_eq!(SortMode::Default.next(), SortMode::ByTitle);
        assert_eq!(SortMode::ByTitle.next(), SortMode::ByArtist);
        assert_eq!(SortMode::ByArtist.next(), SortMode::Default);
    }

    #[test]
    fn apply_search_edit_appends_and_deletes() {
        let mut q = String::new();
        apply_search_char(&mut q, 'a');
        apply_search_char(&mut q, 'B');
        assert_eq!(q, "aB");
        apply_search_backspace(&mut q);
        assert_eq!(q, "a");
        apply_search_backspace(&mut q);
        apply_search_backspace(&mut q);
        assert_eq!(q, "");
    }

    #[test]
    fn apply_search_char_caps_length() {
        let mut q = "x".repeat(64);
        apply_search_char(&mut q, 'y');
        assert_eq!(q.len(), 64);
    }
}
