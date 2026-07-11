//! Chart archive import UI: drag-and-drop + F6 file picker on the song
//! select screen. All real logic lives in `dtx_library::import`; this
//! module only moves paths to a worker thread and shows the outcome.

use std::path::PathBuf;
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

/// Last import outcome message, shown until `expires` (Time::elapsed secs).
#[derive(Resource, Default)]
struct ImportToast {
    text: String,
    expires: f64,
}

#[derive(Component)]
struct ToastNode;

pub fn plugin(app: &mut App) {
    app.init_resource::<ImportChannel>()
        .init_resource::<ImportToast>()
        .add_systems(OnEnter(AppState::SongSelect), spawn_toast_node)
        .add_systems(OnExit(AppState::SongSelect), despawn_toast_node)
        .add_systems(
            Update,
            (dropped_files, import_picker, poll_imports, update_toast)
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
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("chart archives", &["zip", "7z"])
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
    time: Res<Time>,
) {
    let rx = channel.rx.lock().expect("import channel poisoned");
    while let Ok(result) = rx.try_recv() {
        let text = match &result {
            Ok(outcome) => {
                if let Err(e) = db.rescan(&default_song_dir()) {
                    warn!("import: rescan failed: {e}");
                }
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
                format!("already imported: \"{name}\"")
            }
            Err(ImportError::Io(e)) => format!("import failed: {e}"),
        };
        info!("import: {text}");
        toast.text = text;
        toast.expires = time.elapsed_secs_f64() + 4.0;
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
    toast: Res<ImportToast>,
    time: Res<Time>,
    mut nodes: Query<(&mut Text, &mut Visibility), With<ToastNode>>,
) {
    for (mut text, mut visibility) in &mut nodes {
        if toast.text.is_empty() || time.elapsed_secs_f64() > toast.expires {
            *visibility = Visibility::Hidden;
        } else {
            text.0 = toast.text.clone();
            *visibility = Visibility::Visible;
        }
    }
}
