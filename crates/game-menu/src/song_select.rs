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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_audio::{
    BgmHandle, PreviewPlayer, PreviewSwapDirection, PreviewSwapEvent, get_or_load_audio_handle,
    stop_bgm_system,
};
use dtx_library::{SongDb, SongInfo, SortMode};
use dtx_ui::ThemeResource;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::theme::{REF_HEIGHT, REF_WIDTH, Theme};
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
use game_shell::{
    AppState, NavAction, PracticeIntent, TransitionRequest, despawn_stage, request_transition,
};

// ===== Layout constants =====

/// Album-art placeholder size (ADR-0015 followup). Real image
/// loading from `#PREIMAGE:` is a separate task; we render a tinted
/// panel of this size and crossfade its opacity.
pub const ALBUM_ART_W: f32 = 240.0;
pub const ALBUM_ART_H: f32 = 180.0;

/// CommandHistory buffer size (CommandHistory.cs:10).
pub const COMMAND_HISTORY_BUF: usize = 16;

/// Song wheel anchors inside the stage (REF units).
const WHEEL_TOP: f32 = 52.0;
const WHEEL_BOTTOM: f32 = 40.0;

/// Uniform stage scale plus the stage size (in REF units) that fills
/// the window edge-to-edge at that scale. Content never renders
/// smaller than the 1280×720 design; the stage grows in whichever
/// dimension the window has spare room (taller on 16:10, wider on
/// ultrawide) so there is no letterboxing.
fn stage_metrics(win_w: f32, win_h: f32) -> (f32, Vec2) {
    let s = (win_w / REF_WIDTH).min(win_h / REF_HEIGHT);
    (s, Vec2::new(win_w / s, win_h / s))
}

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

    /// Group charts by parent folder. If set.def exists, use its L1..L5
    /// chart order; otherwise sort within folder by display level.
    /// Top-level sort mode applies to the folder list.
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
                let set_order = read_set_def_order(&folder);
                indices.sort_by(|&a, &b| {
                    let oa = set_order_for(&set_order, &all[a].path);
                    let ob = set_order_for(&set_order, &all[b].path);
                    match (oa, ob) {
                        (Some(oa), Some(ob)) => oa.cmp(&ob).then_with(|| a.cmp(&b)),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => display_level_key(&all[a])
                            .cmp(&display_level_key(&all[b]))
                            .then_with(|| a.cmp(&b)),
                    }
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
            SortMode::ByArtist => v.sort_by(|a, b| a.artist.cmp(&b.artist)),
        }
        self.visible = v;
    }
}

fn display_level_key(song: &SongInfo) -> u32 {
    song.dlevel
        .map(|v| (dtx_core::display_dlevel(v) * 100.0).round() as u32)
        .unwrap_or(u32::MAX)
}

fn set_order_for(order: &HashMap<String, usize>, path: &Path) -> Option<usize> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    order.get(&name).copied()
}

fn read_set_def_order(folder: &Path) -> HashMap<String, usize> {
    let Ok(bytes) = std::fs::read(folder.join("set.def")) else {
        return HashMap::new();
    };
    let text = decode_set_def(&bytes);
    let mut order = HashMap::new();
    for line in text.lines().map(str::trim) {
        let upper = line.to_ascii_uppercase();
        for i in 1..=5 {
            let key = format!("#L{i}FILE");
            if upper.starts_with(&key) {
                let file = line[key.len()..].trim_matches([':', ' ', '\t']);
                let file = Path::new(file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(file)
                    .to_ascii_lowercase();
                if !file.is_empty() {
                    order.insert(file, i - 1);
                }
            }
        }
    }
    order
}

fn decode_set_def(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) {
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
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

/// The 1280×720 reference-space stage; scaled to fill the window by
/// `scale_song_select_stage`.
#[derive(Component)]
struct SongSelectStage;

/// Wheel row title text, tagged for per-frame updates.
#[derive(Component)]
struct WheelRowTitle;
/// Wheel row jacket thumbnail image.
#[derive(Component)]
struct WheelRowJacket;
/// Wheel row skill number text (yellow).
#[derive(Component)]
struct WheelRowSkill;
/// Wheel row progress-bar fill node (width driven at spawn).
#[derive(Component)]
struct WheelRowBar;
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

/// Artist text shown directly under the big jacket.
#[derive(Component)]
struct SelectedArtistText;

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
        .init_resource::<PadWheelLevel>()
        .add_systems(
            OnEnter(AppState::SongSelect),
            (
                ensure_song_db_loaded,
                reset_search,
                recompute_visible,
                reset_wheel_spring,
                reset_pad_wheel_level,
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
                song_select_hotkeys,
                (song_select_kb_emit, song_select_nav_consumer).chain(),
                update_song_select_legend,
                search_input,
                respawn_wheel_on_change,
                wheel_layout_system,
                update_left_cluster,
                render_difficulty_grid,
                bgm_preview_on_change,
                update_album_art_image,
                scale_song_select_stage,
            )
                .run_if(in_state(AppState::SongSelect)),
        );
}

/// Scale the reference stage to fill the window edge-to-edge:
/// uniform `min(w/REF_W, h/REF_H)` scale (mirrors gameplay), with the
/// stage node stretched to `window / scale` REF units so no
/// letterbox bands remain.
fn scale_song_select_stage(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut stage: Query<(&mut UiTransform, &mut Node), With<SongSelectStage>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let (s, size) = stage_metrics(win.width(), win.height());
    for (mut tf, mut node) in &mut stage {
        if tf.scale != Vec2::splat(s) {
            tf.scale = Vec2::splat(s);
        }
        if node.width != Val::Px(size.x) || node.height != Val::Px(size.y) {
            node.width = Val::Px(size.x);
            node.height = Val::Px(size.y);
        }
    }
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
    db: Res<SongDb>,
    assets: Res<AssetServer>,
    theme: Res<ThemeResource>,
) {
    let t = theme.0;
    commands
        .spawn((
            SongSelectEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .with_children(|outer| {
            outer
                .spawn((
                    SongSelectStage,
                    Node {
                        width: Val::Px(REF_WIDTH),
                        height: Val::Px(REF_HEIGHT),
                        ..default()
                    },
                    UiTransform::default(),
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

                    // ---- far-left column: skill + bpm
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(20.0),
                            top: Val::Px(72.0),
                            width: Val::Px(180.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            ..default()
                        },
                        UiTransform::default(),
                        EnterChoreo::slide(Vec2::new(-340.0, 0.0), 30.0, 220.0),
                    ))
                    .with_children(|left| {
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

                    // ---- center-top: big square jacket + artist
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(216.0),
                            top: Val::Px(68.0),
                            width: Val::Px(240.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        UiTransform::default(),
                        EnterChoreo::slide(Vec2::new(-340.0, 0.0), 45.0, 220.0),
                    ))
                    .with_children(|mid| {
                        mid.spawn((
                            BigAlbumArt,
                            AlbumArt::default(),
                            AlbumArtEntity,
                            panel(
                                &t,
                                Node {
                                    width: Val::Px(240.0),
                                    height: Val::Px(240.0),
                                    ..default()
                                },
                            ),
                            ImageNode {
                                color: Color::WHITE.with_alpha(0.0),
                                ..default()
                            },
                        ));
                        mid.spawn((
                            SelectedArtistText,
                            Text::new(""),
                            Theme::font(14.0),
                            TextColor(t.text_secondary),
                        ));
                    });

                    // ---- center-bottom: density graph + difficulty ladder (side by side)
                    root.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(216.0),
                            top: Val::Px(344.0),
                            width: Val::Px(396.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        },
                        UiTransform::default(),
                        EnterChoreo::slide(Vec2::new(-340.0, 0.0), 60.0, 220.0),
                    ))
                    .with_children(|bottom| {
                        bottom
                            .spawn(panel(
                                &t,
                                Node {
                                    width: Val::Px(100.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(8.0)),
                                    ..default()
                                },
                            ))
                            .with_children(|p| spawn_density_graph(p, &t));
                        bottom
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
                            left: Val::Percent(51.0),
                            right: Val::Px(16.0),
                            top: Val::Px(WHEEL_TOP),
                            bottom: Val::Px(WHEEL_BOTTOM),
                            // Clip only vertically: rows scroll off the
                            // top/bottom, but the arc bulges the selected
                            // row leftward and must not be horizontally
                            // clipped (else its jacket gets cut off).
                            overflow: Overflow::clip_y(),
                            ..default()
                        },
                    ))
                    .with_children(|wheel| {
                        spawn_wheel_rows(wheel, &selection_state, &db, &assets, &t);
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
                            ("SHIFT+ENTER PRACTICE", false),
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
        });
}

/// Representative chart for a folder's wheel row: the highest-dlevel
/// chart present (falls back to the first). Returns the `db.songs`
/// index, or `None` for an empty folder.
fn folder_display_chart(folder: &SongFolderView, db: &SongDb) -> Option<usize> {
    folder
        .chart_indices
        .iter()
        .copied()
        .filter(|idx| db.songs.get(*idx).is_some())
        .max_by_key(|idx| db.songs[*idx].dlevel.unwrap_or(0))
        .or_else(|| folder.chart_indices.first().copied())
}

/// Spawn one absolute-positioned row per visible folder. Positions are
/// written every frame by `wheel_layout_system`.
fn spawn_wheel_rows(
    wheel: &mut ChildSpawnerCommands,
    selection_state: &SongSelectSelection,
    db: &SongDb,
    assets: &AssetServer,
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
                    width: Val::Percent(92.0),
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
                // Jacket thumbnail (or tinted placeholder).
                let display = folder_display_chart(folder, db).and_then(|i| db.songs.get(i));
                let jacket_image = display
                    .and_then(|s| s.preimage_path.as_ref())
                    .map(|p| assets.load(p.to_string_lossy().to_string()))
                    .unwrap_or_default();
                row.spawn((
                    WheelRowJacket,
                    Node {
                        width: Val::Px(58.0),
                        height: Val::Px(58.0),
                        ..default()
                    },
                    BackgroundColor(t.stage_panel_border),
                    ImageNode {
                        image: jacket_image,
                        ..default()
                    },
                ));
                // Right column: skill+bar row, then title.
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col| {
                    let (skill, ach) = display
                        .map(|s| {
                            let ini = dtx_scoring::score_ini::score_ini_path(&s.path);
                            let acc = dtx_scoring::score_ini::read_best(&ini)
                                .map(|b| b.accuracy())
                                .unwrap_or(0.0);
                            (crate::chart_stats::skill_points(s.dlevel, acc), acc)
                        })
                        .unwrap_or((0.0, 0.0));
                    // Skill number + progress bar on one line.
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            WheelRowSkill,
                            Text::new(crate::chart_stats::row_skill_text(skill)),
                            Theme::font(15.0),
                            TextColor(t.select_yellow),
                        ));
                        // Progress-bar track.
                        line.spawn((
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(6.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            BorderColor::all(t.stage_panel_border),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                WheelRowBar,
                                Node {
                                    width: Val::Percent(crate::chart_stats::bar_fill_pct(ach)),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(t.select_yellow),
                            ));
                        });
                    });
                    col.spawn((
                        WheelRowTitle,
                        Text::new(folder.title.clone()),
                        Theme::font(18.0),
                        TextColor(t.text_primary),
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
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
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

    // Wheel container height in REF units (stage stretches to the
    // window, wheel stretches between its top/bottom anchors).
    let wheel_h = windows
        .single()
        .map(|w| stage_metrics(w.width(), w.height()).1.y)
        .unwrap_or(REF_HEIGHT)
        - WHEEL_TOP
        - WHEEL_BOTTOM;
    for (row, mut node, mut vis, mut border, mut shadow, mut bg) in &mut rows {
        let offset = row.index as f32 - center;
        if offset.abs() > (VISIBLE_HALF as f32 + 1.0) {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let g = row_geometry(offset);
        node.top = Val::Px(wheel_h / 2.0 + g.center_y - g.height / 2.0);
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
    mut artist_text: Query<
        &mut Text,
        (
            With<SelectedArtistText>,
            Without<BadgeValueText>,
            Without<SortChipText>,
        ),
    >,
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
                level: song.dlevel.map(dtx_core::display_dlevel),
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

    let artist = selection_state
        .visible
        .get(selection.folder)
        .map(|f| f.artist.clone())
        .unwrap_or_default();
    for mut text in &mut artist_text {
        *text = Text::new(artist.clone());
    }
}

/// Write grid slot data into the widget's text/border entities.
fn render_difficulty_grid(
    grid: Res<DifficultyGridData>,
    theme: Res<ThemeResource>,
    mut panels: Query<
        (
            &DifficultySlotPanel,
            &mut BorderColor,
            &mut BoxShadow,
            &mut BackgroundColor,
        ),
        Without<DifficultySlotLabel>,
    >,
    mut labels: Query<
        (&DifficultySlotLabel, &mut Text, &mut BackgroundColor),
        (
            Without<DifficultySlotLevel>,
            Without<DifficultySlotScore>,
            Without<DifficultySlotPanel>,
        ),
    >,
    mut levels: Query<
        (&DifficultySlotLevel, &mut Text, &mut TextColor),
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
    for (label, mut text, mut bg) in &mut labels {
        let slot = &grid.slots[label.0];
        *text = Text::new(slot.label.clone());
        let base = t.difficulty_color(label.0 as u8);
        bg.0 = if slot.present {
            base
        } else {
            base.with_alpha(0.3)
        };
    }
    for (level, mut text, mut color) in &mut levels {
        let slot = &grid.slots[level.0];
        *text = Text::new(level_text(slot.level));
        color.0 = if slot.present {
            t.text_primary
        } else {
            t.text_primary.with_alpha(0.3)
        };
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
    db: Res<SongDb>,
    assets: Res<AssetServer>,
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
        spawn_wheel_rows(w, &selection_state, &db, &assets, &t);
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

/// Pad two-level song select: the wheel (folders), then difficulty. Keyboard
/// stays flat (Up/Down = folder, Left/Right = difficulty) and never reads this.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PadWheelLevel {
    /// HH/CY move folders, BD descends to difficulty, SD leaves for Title.
    #[default]
    Wheel,
    /// HH/CY cycle difficulty, BD plays, FT practices, SD returns to the wheel.
    Difficulty,
}

impl PadWheelLevel {
    fn on_verb(self, verb: game_shell::NavVerb) -> Self {
        use game_shell::NavVerb;
        match (self, verb) {
            (PadWheelLevel::Wheel, NavVerb::Confirm) => PadWheelLevel::Difficulty,
            (PadWheelLevel::Difficulty, NavVerb::Back) => PadWheelLevel::Wheel,
            (level, _) => level,
        }
    }
}

fn reset_pad_wheel_level(mut level: ResMut<PadWheelLevel>) {
    *level = PadWheelLevel::Wheel;
}

/// Pad legend above the keyboard hint bar; hidden when no MIDI device is on.
fn update_song_select_legend(
    mut commands: Commands,
    midi: Option<Res<game_shell::MidiConnected>>,
    level: Res<PadWheelLevel>,
    theme: Res<dtx_ui::ThemeResource>,
    legends: Query<Entity, With<dtx_ui::widget::nav_legend::NavLegend>>,
    mut last_sig: Local<Option<(PadWheelLevel, bool)>>,
) {
    let connected = midi.is_some_and(|m| m.0);
    let sig = (*level, connected);
    let missing = connected && legends.is_empty();
    if last_sig.as_ref() == Some(&sig) && !missing {
        return;
    }
    *last_sig = Some(sig);
    for e in &legends {
        commands.entity(e).despawn();
    }
    if !connected {
        return;
    }
    let items: &[(&str, &str)] = match *level {
        PadWheelLevel::Wheel => &[
            ("HH", "up"),
            ("CY", "down"),
            ("BD", "difficulty"),
            ("SD", "title"),
        ],
        PadWheelLevel::Difficulty => &[
            ("HH", "prev diff"),
            ("CY", "next diff"),
            ("BD", "play"),
            ("FT", "practice"),
            ("SD", "songs"),
        ],
    };
    let t = theme.0;
    commands
        .spawn((
            SongSelectEntity,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(34.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|p| {
            dtx_ui::widget::nav_legend::spawn_nav_legend(p, &t, items);
        });
}

/// Raw keyboard affordances with no pad equivalent: sort, customize, rescan.
fn song_select_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut db: ResMut<SongDb>,
    selection: Res<Selection>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut selected_song: ResMut<SelectedSong>,
    mut requests: MessageWriter<TransitionRequest>,
    mut pending: ResMut<game_shell::PendingCustomizeTab>,
    mut session: ResMut<game_shell::EditorSession>,
) {
    if keys.just_pressed(KeyCode::F5) {
        if let Err(e) = db.rescan(&default_song_dir()) {
            warn!("SongSelect: refresh failed: {}", e);
        }
        return;
    }
    if selection_state.visible.is_empty() {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        selection_state.sort_mode = selection_state.sort_mode.next();
        selection_state.dirty = true;
    } else if keys.just_pressed(KeyCode::F1) {
        if let Some(chart_idx) = selection.chart_index(&selection_state)
            && let Some(song) = db.songs.get(chart_idx)
        {
            pending.0 = Some(game_shell::CustomizeTab::Gameplay);
            session.0 = true;
            selected_song.0 = Some(song.path.clone());
            request_transition(&mut requests, AppState::SongLoading);
        } else {
            warn!("customize: no song highlighted");
        }
    }
}

/// Keyboard → `NavAction`. Shift+Enter is Practice, plain Enter is Confirm.
fn song_select_kb_emit(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<NavAction>) {
    use game_shell::{NavSource, NavVerb};
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let verb = if keys.just_pressed(KeyCode::ArrowDown) {
        NavVerb::Down
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        NavVerb::Up
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        NavVerb::Inc
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        NavVerb::Dec
    } else if keys.just_pressed(KeyCode::Enter) {
        if shift { NavVerb::Practice } else { NavVerb::Confirm }
    } else if keys.just_pressed(KeyCode::Escape) {
        NavVerb::Back
    } else {
        return;
    };
    out.write(NavAction {
        verb,
        source: NavSource::Keyboard,
        coarse: false,
    });
}

fn song_select_nav_consumer(
    mut actions: MessageReader<NavAction>,
    mut level: ResMut<PadWheelLevel>,
    db: Res<SongDb>,
    mut selection: ResMut<Selection>,
    selection_state: Res<SongSelectSelection>,
    mut selected_song: ResMut<SelectedSong>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_intent: ResMut<PracticeIntent>,
) {
    use game_shell::{NavSource, NavVerb};
    if selection_state.visible.is_empty() {
        actions.clear();
        return;
    }
    for action in actions.read() {
        // Keyboard is flat; pads move whichever axis the current level owns.
        let (folder_step, diff_step) = match (action.source, *level, action.verb) {
            (_, _, NavVerb::Dec) => (0, -1),
            (_, _, NavVerb::Inc) => (0, 1),
            (NavSource::Keyboard, _, NavVerb::Up) => (-1, 0),
            (NavSource::Keyboard, _, NavVerb::Down) => (1, 0),
            (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Up) => (-1, 0),
            (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Down) => (1, 0),
            (NavSource::Pad, PadWheelLevel::Difficulty, NavVerb::Up) => (0, -1),
            (NavSource::Pad, PadWheelLevel::Difficulty, NavVerb::Down) => (0, 1),
            _ => (0, 0),
        };
        if folder_step > 0 {
            let max = selection_state.visible.len() - 1;
            selection.folder = (selection.folder + 1).min(max);
            selection.clamp_to_visible(&selection_state);
        } else if folder_step < 0 {
            selection.folder = selection.folder.saturating_sub(1);
            selection.clamp_to_visible(&selection_state);
        }
        if diff_step > 0 {
            if let Some(folder) = selection_state.visible.get(selection.folder) {
                let count = folder.difficulty_count();
                if count > 0 {
                    selection.difficulty = (selection.difficulty + 1).min((count - 1) as u8);
                }
            }
        } else if diff_step < 0 {
            selection.difficulty = selection.difficulty.saturating_sub(1);
        }

        // Pads only start a song from the difficulty level; keyboard from anywhere.
        let pad_at_difficulty =
            action.source == NavSource::Keyboard || *level == PadWheelLevel::Difficulty;
        let start = match action.verb {
            NavVerb::Confirm if pad_at_difficulty => Some(false),
            NavVerb::Practice if pad_at_difficulty => Some(true),
            _ => None,
        };
        if let Some(practice) = start
            && let Some(chart_idx) = selection.chart_index(&selection_state)
            && let Some(song) = db.songs.get(chart_idx)
        {
            practice_intent.0 = practice;
            info!(
                "SongSelect: selected {} ({}){}",
                song.title,
                SongFolderView::difficulty_label(selection.difficulty),
                if practice { " [practice]" } else { "" }
            );
            selected_song.0 = Some(song.path.clone());
            request_transition(&mut requests, AppState::SongLoading);
        }

        let leaves = matches!(
            (action.source, *level, action.verb),
            (NavSource::Keyboard, _, NavVerb::Back)
                | (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Back)
        );
        if leaves {
            request_transition(&mut requests, AppState::Title);
        }

        if action.source == NavSource::Pad {
            *level = level.on_verb(action.verb);
        }
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
    let cfg = dtx_config::load(&dtx_config::default_path());
    if !cfg.audio.bgm_enabled {
        player.stop(&mut instances, 0);
        return;
    }
    let Some(preview_path) = song.preview_path.clone() else {
        // No preview for this song: stop whatever's currently
        // playing so we don't leak a stale preview from a prior
        // selection. (`stop` releases the kira instance via
        // `stop_with_fade` — see PreviewPlayer docs.)
        info!("SongSelect preview: no preview path; stopping current preview");
        player.stop(&mut instances, 0);
        return;
    };

    // Clear gameplay BGM before starting preview; stale handles are no-ops.
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
    player.set_volume(cfg.audio.master_volume * cfg.audio.bgm_volume);

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

    #[test]
    fn pad_wheel_levels() {
        use game_shell::NavVerb;
        let mut level = PadWheelLevel::Wheel;
        level = level.on_verb(NavVerb::Confirm);
        assert_eq!(level, PadWheelLevel::Difficulty);
        level = level.on_verb(NavVerb::Back);
        assert_eq!(level, PadWheelLevel::Wheel);
        // Back at the wheel exits to Title; the level itself is unchanged.
        assert_eq!(
            PadWheelLevel::Wheel.on_verb(NavVerb::Back),
            PadWheelLevel::Wheel
        );
        // Moves never change level.
        assert_eq!(
            PadWheelLevel::Difficulty.on_verb(NavVerb::Down),
            PadWheelLevel::Difficulty
        );
        // Confirm at difficulty starts the song, staying put.
        assert_eq!(
            PadWheelLevel::Difficulty.on_verb(NavVerb::Confirm),
            PadWheelLevel::Difficulty
        );
    }

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
    fn stage_metrics_fills_window_without_bands() {
        // Exact 16:9: stage is the reference size.
        let (s, size) = stage_metrics(2560.0, 1440.0);
        assert_eq!(s, 2.0);
        assert_eq!(size, Vec2::new(REF_WIDTH, REF_HEIGHT));
        // 16:10: width-constrained, stage grows taller than 720.
        let (s, size) = stage_metrics(1280.0, 800.0);
        assert_eq!(s, 1.0);
        assert_eq!(size, Vec2::new(1280.0, 800.0));
        // Ultrawide: height-constrained, stage grows wider than 1280.
        let (s, size) = stage_metrics(3440.0, 1440.0);
        assert_eq!(s, 2.0);
        assert_eq!(size, Vec2::new(1720.0, 720.0));
    }

    #[test]
    fn folder_display_chart_picks_highest_dlevel() {
        let mut db = SongDb::default();
        let mut a = make_song("a", "");
        a.dlevel = Some(30);
        let mut b = make_song("b", "");
        b.dlevel = Some(90);
        let mut c = make_song("c", "");
        c.dlevel = Some(50);
        db.songs.push(a);
        db.songs.push(b);
        db.songs.push(c);
        let folder = SongFolderView {
            folder: std::path::PathBuf::from("/x"),
            title: "t".into(),
            artist: "x".into(),
            chart_indices: vec![0, 1, 2],
        };
        assert_eq!(folder_display_chart(&folder, &db), Some(1));
    }

    #[test]
    fn folder_display_chart_empty_is_none() {
        let db = SongDb::default();
        let folder = SongFolderView {
            folder: std::path::PathBuf::from("/x"),
            title: "t".into(),
            artist: "x".into(),
            chart_indices: vec![],
        };
        assert_eq!(folder_display_chart(&folder, &db), None);
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
        // Within folder: easiest (lowest displayed level) first.
        assert_eq!(all[folder.chart_indices[0]].dlevel, Some(50));
        assert_eq!(all[folder.chart_indices[2]].dlevel, Some(95));
    }

    #[test]
    fn recompute_uses_set_def_chart_order() {
        let root = std::env::temp_dir().join(format!(
            "dtxmaniars-setdef-{}-{}",
            std::process::id(),
            "song-select"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut bytes = vec![0xff, 0xfe];
        for unit in "#L1FILE bas.dtx\r\n#L2FILE adv.dtx\r\n#L3FILE ext.dtx\r\n#L4FILE mas.dtx\r\n"
            .encode_utf16()
        {
            bytes.extend(unit.to_le_bytes());
        }
        std::fs::write(root.join("set.def"), bytes).unwrap();

        let chart = |file: &str, level: u32| SongInfo {
            path: root.join(file),
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
            chart("ext.dtx", 77),
            chart("mas.dtx", 94),
            chart("bas.dtx", 355),
            chart("adv.dtx", 615),
        ];
        let mut sel = SongSelectSelection::default();
        sel.recompute(&all);
        let names: Vec<_> = sel.visible[0]
            .chart_indices
            .iter()
            .map(|&i| {
                all[i]
                    .path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(names, ["bas.dtx", "adv.dtx", "ext.dtx", "mas.dtx"]);
        let _ = std::fs::remove_dir_all(&root);
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
