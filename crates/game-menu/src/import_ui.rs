//! Chart archive import UI: drag-and-drop + F6 file picker on the song
//! select screen. All real logic lives in `dtx_library::import`; this
//! module only moves paths to a worker thread and shows the outcome.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::FileDragAndDrop;

use dtx_library::import::{ImportError, ImportOutcome, import_archive};
use dtx_library::{SongDb, default_song_dir};
use dtx_ui::ThemeResource;
use dtx_ui::theme::Theme;
use game_shell::AppState;

type ImportResult = Result<ImportOutcome, ImportError>;

/// Channel between import worker threads and the poll system.
/// Receiver is not Sync, hence the Mutex (uncontended: single reader).
#[derive(Resource)]
struct ImportChannel {
    tx: Sender<ImportResult>,
    rx: Mutex<Receiver<ImportResult>>,
}

impl Default for ImportChannel {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }
}

/// How many outcome lines the toast shows at once (multi-import).
const TOAST_MAX_LINES: usize = 5;
/// Seconds the toast stays up after the latest outcome.
const TOAST_SECS: f64 = 5.0;

/// Recent import outcome lines, shown until `expires` (Time::elapsed secs).
/// Every new outcome appends a line and refreshes the timer, so a batch
/// of imports reads as one growing list instead of racing single toasts.
#[derive(Resource, Default)]
struct ImportToast {
    lines: Vec<String>,
    expires: f64,
}

/// Song folder name to move the wheel cursor to once it appears in the
/// visible list (after the post-import rescan recompute, next frame).
/// `frames_left` bounds the search so a filtered-out song can't leave a
/// stale pending jump forever.
#[derive(Resource, Default)]
struct PendingImportJump {
    name: Option<String>,
    frames_left: u32,
}

#[derive(Component)]
struct ToastNode;

pub fn plugin(app: &mut App) {
    app.init_resource::<ImportChannel>()
        .init_resource::<ImportToast>()
        .init_resource::<PendingImportJump>()
        .add_systems(OnEnter(AppState::SongSelect), spawn_toast_node)
        .add_systems(OnExit(AppState::SongSelect), despawn_toast_node)
        .add_systems(
            Update,
            (
                dropped_files,
                import_picker,
                poll_imports,
                jump_to_imported,
                update_toast,
            )
                .run_if(in_state(AppState::SongSelect)),
        );
}

/// One import = one short-lived thread. Extraction of a big pack takes
/// seconds; the UI must not block.
fn start_import(tx: &Sender<ImportResult>, path: PathBuf) {
    let tx = tx.clone();
    let root = default_song_dir();
    std::thread::spawn(move || {
        let _ = tx.send(import_archive(&path, &root));
    });
}

fn dropped_files(mut events: MessageReader<FileDragAndDrop>, channel: Res<ImportChannel>) {
    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            start_import(&channel.tx, path_buf.clone());
        }
    }
}

/// F6: native file picker. NonSendMarker pins this system to the main
/// thread — macOS requires dialogs there. The dialog blocks the frame
/// loop while open; acceptable for a modal picker.
fn import_picker(
    keys: Res<ButtonInput<KeyCode>>,
    channel: Res<ImportChannel>,
    _main_thread: NonSendMarker,
) {
    if !keys.just_pressed(KeyCode::F6) {
        return;
    }
    // rar is listed so picking one yields the "unsupported" toast instead
    // of the file being invisible and the user wondering why.
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("chart archives", &["zip", "7z", "rar"])
        .pick_files()
    else {
        return;
    };
    for path in paths {
        start_import(&channel.tx, path);
    }
}

fn poll_imports(
    channel: Res<ImportChannel>,
    mut db: ResMut<SongDb>,
    mut toast: ResMut<ImportToast>,
    mut jump: ResMut<PendingImportJump>,
    time: Res<Time>,
) {
    let rx = channel.rx.lock().expect("import channel poisoned");
    while let Ok(result) = rx.try_recv() {
        let text = match &result {
            Ok(outcome) => {
                if let Err(e) = db.rescan(&default_song_dir()) {
                    warn!("import: rescan failed: {e}");
                }
                jump.name = Some(outcome.dest_name.clone());
                jump.frames_left = 120;
                format!(
                    "imported \"{}\" ({} chart{})",
                    outcome.dest_name,
                    outcome.chart_count,
                    if outcome.chart_count == 1 { "" } else { "s" }
                )
            }
            Err(ImportError::UnsupportedFormat(f)) => {
                format!("unsupported: {f} — extract manually")
            }
            Err(ImportError::NoCharts) => "no charts found in archive".to_owned(),
            Err(ImportError::UnsafePath) => "archive rejected (unsafe paths)".to_owned(),
            Err(ImportError::AlreadyImported(name)) => {
                // Still jump: "where is it?" is the question the user is
                // actually asking when they re-import a song.
                jump.name = Some(name.clone());
                jump.frames_left = 120;
                format!("already imported: \"{name}\"")
            }
            Err(ImportError::Io(e)) => format!("import failed: {e}"),
        };
        info!("import: {text}");
        toast.lines.push(text);
        if toast.lines.len() > TOAST_MAX_LINES {
            let drop = toast.lines.len() - TOAST_MAX_LINES;
            toast.lines.drain(..drop);
        }
        toast.expires = time.elapsed_secs_f64() + TOAST_SECS;
    }
}

/// Does this visible folder live under `song_root/<name>/`?
/// Matches both a plain song folder and any song inside an imported
/// multi-song pack folder.
fn folder_under_import(view_folder: &Path, song_root: &Path, name: &str) -> bool {
    view_folder.starts_with(song_root.join(name))
}

/// Move the wheel cursor to the folder imported last. Runs every frame
/// while a jump is pending: the visible list only picks up the rescan on
/// the frame after `poll_imports`, so this waits for it to appear.
fn jump_to_imported(
    mut jump: ResMut<PendingImportJump>,
    mut selection: ResMut<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
) {
    let Some(name) = jump.name.as_deref() else {
        return;
    };
    let root = default_song_dir();
    if let Some(index) = selection_state
        .visible
        .iter()
        .position(|view| folder_under_import(&view.folder, &root, name))
    {
        selection.folder = index;
        selection.difficulty = 0;
        jump.name = None;
        return;
    }
    // Not visible (e.g. filtered out by an active search) — give up quietly.
    jump.frames_left = jump.frames_left.saturating_sub(1);
    if jump.frames_left == 0 {
        jump.name = None;
    }
}

fn spawn_toast_node(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands.spawn((
        ToastNode,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            top: Val::Px(80.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        // The song-select stage is a sibling full-screen root spawned in
        // the same OnEnter schedule with ambiguous order; without an
        // explicit z the stage can paint over the toast.
        GlobalZIndex(100),
        BackgroundColor(t.stage_panel_bg),
        Text::new(""),
        Theme::font(16.0),
        TextColor(t.text_secondary),
        Visibility::Hidden,
    ));
}

fn despawn_toast_node(mut commands: Commands, nodes: Query<Entity, With<ToastNode>>) {
    for entity in &nodes {
        commands.entity(entity).despawn();
    }
}

fn update_toast(
    mut toast: ResMut<ImportToast>,
    time: Res<Time>,
    mut nodes: Query<(&mut Text, &mut Visibility), With<ToastNode>>,
) {
    let expired = time.elapsed_secs_f64() > toast.expires;
    if expired && !toast.lines.is_empty() {
        toast.lines.clear();
    }
    for (mut text, mut visibility) in &mut nodes {
        if toast.lines.is_empty() {
            *visibility = Visibility::Hidden;
        } else {
            text.0 = toast.lines.join("\n");
            *visibility = Visibility::Visible;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_match_plain_and_pack() {
        let root = Path::new("/songs");
        // plain imported folder
        assert!(folder_under_import(
            Path::new("/songs/MySong"),
            root,
            "MySong"
        ));
        // song inside an imported multi-song pack
        assert!(folder_under_import(
            Path::new("/songs/Pack Vol.1/Song A"),
            root,
            "Pack Vol.1"
        ));
        // different folder with a shared prefix must NOT match
        assert!(!folder_under_import(
            Path::new("/songs/MySong 2"),
            root,
            "MySong"
        ));
        assert!(!folder_under_import(
            Path::new("/songs/Other"),
            root,
            "MySong"
        ));
    }
}
