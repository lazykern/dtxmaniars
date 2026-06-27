//! Song database — scans a directory of `.dtx` files, builds SongInfo list.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/SongDb/SongDb.cs`
//!
//! ## M5 scope
//!
//! - Sync directory scan (DTXManiaNX uses async Task — deferred to M6+)
//! - No SQLite cache (`SongCacheSqlite.cs` — M6+)
//! - No zip unpacking
//! - No folder/box tree (`SongNode.cs` — M6+)
//! - 3 sort modes: Default, ByTitle, ByArtist
//! - BGM detection: try `<dtx>.ogg` and `1.ogg` in same dir
//!
//! ## Phase 0 p0-3 additions
//!
//! - `default_song_dir()` resolves `DTX_SONG_DIR` env var or fixture fallback.
//! - `startup_scan_system()` runs at app boot, populates SongDb before
//!   SongSelect is reached (avoids empty first frame).
//! - `refresh_song_db()`: re-scan from the active root path.
//!
//! ponytail: stdlib `walkdir` (or manual recursion) + dtx_core::parse. No async
//! machinery until we have 1000s of charts.

use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use dtx_core::{Chart, parse};
use thiserror::Error;

/// One chart (= one .dtx file in M5; M6+ supports multi-chart songs).
#[derive(Debug, Clone, PartialEq)]
pub struct SongInfo {
    /// Path to the .dtx file.
    pub path: PathBuf,
    /// Title (from #TITLE) or filename stem if missing.
    pub title: String,
    /// Artist (from #ARTIST) or "Unknown".
    pub artist: String,
    /// BPM (from #BPM) or None.
    pub bpm: Option<f32>,
    /// Drums difficulty level (from #DLEVEL) or None.
    pub dlevel: Option<u32>,
    /// Path to BGM audio file (ogg/wav) if found, else None.
    /// Search order: `<dtx_stem>.ogg`, `1.ogg`, `<dtx_stem>.wav`, `1.wav`.
    pub bgm_path: Option<PathBuf>,
}

impl SongInfo {
    /// Build SongInfo from a parsed Chart + the dtx file path.
    pub fn from_chart(dtx_path: &Path, chart: &Chart) -> Self {
        let title = chart.metadata.title.clone().unwrap_or_else(|| {
            dtx_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });
        let artist = chart
            .metadata
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            path: dtx_path.to_path_buf(),
            title,
            artist,
            bpm: chart.metadata.bpm,
            dlevel: chart.metadata.dlevel,
            bgm_path: find_bgm(dtx_path),
        }
    }

    /// Approximate total note count from the chart chips. M10 status panel
    /// shows this in the StatusPane. M10.1 counts per-instrument.
    pub fn notes_total(&self) -> u32 {
        // Load chart from disk on demand (M10: cheap; M10.1: cache).
        let Ok(bytes) = std::fs::read(&self.path) else {
            return 0;
        };
        let Ok(chart) = dtx_core::parse(bytes.as_slice()) else {
            return 0;
        };
        chart.chips.iter().filter(|c| c.channel.is_drum()).count() as u32
    }
}

/// Find a BGM audio file near the given .dtx file. Returns the first match.
fn find_bgm(dtx_path: &Path) -> Option<PathBuf> {
    let parent = dtx_path.parent()?;
    let stem = dtx_path.file_stem()?.to_str()?;

    for ext in &["ogg", "wav"] {
        // <dtx_stem>.<ext>
        let p = parent.join(format!("{stem}.{ext}"));
        if p.exists() {
            return Some(p);
        }
        // 1.<ext>  (DTXmania convention for #BGM: 1)
        let p = parent.join(format!("1.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("I/O error reading directory {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Parse error in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: dtx_core::DtxError,
    },
}

/// Walk a directory recursively, parse each .dtx file, return SongInfo list.
///
/// Errors on individual files are logged and skipped (so one bad DTX
/// doesn't kill the whole scan). Only a directory-level I/O error is fatal.
pub fn scan_directory(root: &Path) -> Result<Vec<SongInfo>, ScanError> {
    let mut songs = Vec::new();
    walk_dtx(root, &mut songs)?;
    Ok(songs)
}

fn walk_dtx(dir: &Path, songs: &mut Vec<SongInfo>) -> Result<(), ScanError> {
    let entries = fs::read_dir(dir).map_err(|source| ScanError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dtx(&path, songs)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("dtx") {
            match fs::File::open(&path) {
                Ok(file) => match parse(file) {
                    Ok(chart) => songs.push(SongInfo::from_chart(&path, &chart)),
                    Err(source) => {
                        bevy::log::warn!("DTX parse failed for {}: {}", path.display(), source);
                    }
                },
                Err(source) => {
                    bevy::log::warn!("DTX open failed for {}: {}", path.display(), source);
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortMode {
    /// File-system order (DTXManiaNX SortDefault).
    #[default]
    Default,
    /// Alphabetical by title.
    ByTitle,
    /// Alphabetical by artist.
    ByArtist,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::ByTitle,
            Self::ByTitle => Self::ByArtist,
            Self::ByArtist => Self::Default,
        }
    }
}

/// Sort the song list in place using the given mode.
pub fn sort_songs(songs: &mut [SongInfo], mode: SortMode) {
    match mode {
        SortMode::Default => {} // preserve current order
        SortMode::ByTitle => songs.sort_by(|a, b| a.title.cmp(&b.title)),
        SortMode::ByArtist => songs.sort_by(|a, b| a.artist.cmp(&b.artist)),
    }
}

/// Game-wide song database. Resource.
#[derive(Resource, Debug, Default, Clone)]
pub struct SongDb {
    pub songs: Vec<SongInfo>,
    pub sort_mode: SortMode,
    /// Root directory that was scanned (for re-scan).
    pub scan_root: Option<PathBuf>,
}

impl SongDb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan the directory and update in place.
    pub fn rescan(&mut self, root: &Path) -> Result<(), ScanError> {
        let mut songs = scan_directory(root)?;
        sort_songs(&mut songs, self.sort_mode);
        self.songs = songs;
        self.scan_root = Some(root.to_path_buf());
        Ok(())
    }

    /// Re-scan using the previously-set `scan_root`. Returns Err if no root set.
    pub fn refresh(&mut self) -> Result<(), ScanError> {
        let root = self.scan_root.clone().ok_or_else(|| ScanError::Io {
            path: PathBuf::from("(no scan_root)"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "SongDb.scan_root not set"),
        })?;
        self.rescan(&root)
    }

    /// Cycle to the next sort mode and re-sort.
    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.next();
        sort_songs(&mut self.songs, self.sort_mode);
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&SongInfo> {
        self.songs.get(index)
    }
}

/// Default directory to scan. Priority:
/// 1. `DTX_SONG_DIR` env var (explicit override)
/// 2. `$XDG_CONFIG_HOME/dtxmaniars/` (XDG) — standard DTXMania layout
/// 3. `$HOME/.config/dtxmaniars/` (XDG fallback)
/// 4. Bundled test fixtures (dev/headless fallback)
///
/// The XDG path is the standard DTXMania layout: each song gets its own
/// subfolder under the root (e.g. `~/.config/dtxmaniars/Song A/bsc.dtx`).
/// The scanner recurses, so this single root finds everything.
///
/// Returns the path. Does NOT create it — callers should `create_dir_all`
/// if they want to ensure the dir exists.
pub fn default_song_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DTX_SONG_DIR") {
        return PathBuf::from(p);
    }
    if let Some(dir) = user_data_dir() {
        return dir;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dtx-core")
        .join("tests")
        .join("fixtures")
}

/// XDG-style user data directory: `$XDG_CONFIG_HOME/dtxmaniars/` or
/// `$HOME/.config/dtxmaniars/`. Returns `None` if neither env var is set
/// (e.g. exotic environments without HOME).
pub fn user_data_dir() -> Option<PathBuf> {
    data_dir_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

/// Compute the user data dir from explicit env var values (testable
/// without unsafe env mutation).
fn data_dir_from(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(xdg) = xdg {
        let mut p = PathBuf::from(xdg);
        p.push("dtxmaniars");
        return Some(p);
    }
    if let Some(home) = home {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("dtxmaniars");
        return Some(p);
    }
    None
}

/// Bevy system: run at app startup (before any AppState transition).
/// Populates `SongDb` with all charts in the default scan dir.
pub fn startup_scan_system(mut db: ResMut<SongDb>) {
    let dir = default_song_dir();
    info!("dtx-library: startup scan {}", dir.display());
    // Ensure the dir exists (no-op if already there, or a fixture path).
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(
            "dtx-library: could not create scan dir {}: {}",
            dir.display(),
            e
        );
    }
    match db.rescan(&dir) {
        Ok(()) => info!("dtx-library: found {} song(s)", db.len()),
        Err(e) => warn!("dtx-library: startup scan failed: {}", e),
    }
}

/// Plugin: register SongDb resource + startup scan system.
pub struct SongDbPlugin;

impl Plugin for SongDbPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SongDb>()
            .add_systems(Startup, startup_scan_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dtx-core")
            .join("tests")
            .join("fixtures")
    }

    #[test]
    fn from_chart_uses_metadata_when_present() {
        let chart = Chart {
            metadata: dtx_core::Metadata {
                title: Some("My Song".into()),
                artist: Some("Me".into()),
                bpm: Some(120.0),
                dlevel: Some(85),
                ..default()
            },
            chips: vec![],
        };
        let path = PathBuf::from("/songs/mysong.dtx");
        let info = SongInfo::from_chart(&path, &chart);
        assert_eq!(info.title, "My Song");
        assert_eq!(info.artist, "Me");
        assert_eq!(info.bpm, Some(120.0));
        assert_eq!(info.dlevel, Some(85));
    }

    #[test]
    fn from_chart_falls_back_to_filename_stem() {
        let chart = Chart::default();
        let path = PathBuf::from("/songs/cool_song.dtx");
        let info = SongInfo::from_chart(&path, &chart);
        assert_eq!(info.title, "cool_song");
        assert_eq!(info.artist, "Unknown");
    }

    #[test]
    fn scan_directory_finds_drums_basic_fixture() {
        let songs = scan_directory(&fixture_dir()).expect("scan must succeed");
        assert!(
            !songs.is_empty(),
            "fixture dir should have at least one .dtx"
        );
        assert!(songs.iter().any(|s| s.path.ends_with("drums_basic.dtx")));
    }

    #[test]
    fn sort_by_title_alphabetical() {
        let mut songs = vec![
            SongInfo {
                path: PathBuf::from("/a.dtx"),
                title: "Charlie".into(),
                artist: "X".into(),
                bpm: None,
                dlevel: None,
                bgm_path: None,
            },
            SongInfo {
                path: PathBuf::from("/b.dtx"),
                title: "Alpha".into(),
                artist: "Y".into(),
                bpm: None,
                dlevel: None,
                bgm_path: None,
            },
        ];
        sort_songs(&mut songs, SortMode::ByTitle);
        assert_eq!(songs[0].title, "Alpha");
        assert_eq!(songs[1].title, "Charlie");
    }

    #[test]
    fn sort_mode_cycles_through_three() {
        let m = SortMode::Default;
        assert_eq!(m.next(), SortMode::ByTitle);
        assert_eq!(SortMode::ByTitle.next(), SortMode::ByArtist);
        assert_eq!(SortMode::ByArtist.next(), SortMode::Default);
    }

    #[test]
    fn song_db_starts_empty() {
        let db = SongDb::new();
        assert!(db.is_empty());
        assert_eq!(db.sort_mode, SortMode::Default);
    }

    #[test]
    fn song_db_rescan_populates() {
        let mut db = SongDb::new();
        db.rescan(&fixture_dir()).expect("scan must succeed");
        assert!(!db.is_empty());
        assert!(db.scan_root.is_some());
    }

    #[test]
    fn song_db_refresh_uses_existing_root() {
        let mut db = SongDb::new();
        db.rescan(&fixture_dir()).unwrap();
        let before = db.len();
        assert!(before > 0);
        db.refresh().expect("refresh must succeed");
        assert_eq!(db.len(), before);
    }

    #[test]
    fn song_db_refresh_no_root_errors() {
        let mut db = SongDb::new();
        assert!(db.refresh().is_err());
    }

    #[test]
    fn song_db_cycle_sort_resorts() {
        let mut db = SongDb::new();
        db.rescan(&fixture_dir()).unwrap();
        let before = db.songs.iter().map(|s| s.title.clone()).collect::<Vec<_>>();
        db.cycle_sort(); // Default → ByTitle
        let after = db.songs.iter().map(|s| s.title.clone()).collect::<Vec<_>>();
        assert_eq!(before.len(), after.len());
    }

    #[test]
    fn default_song_dir_returns_existing_path() {
        let p = default_song_dir();
        assert!(p.exists(), "default dir should exist: {:?}", p);
    }

    #[test]
    fn data_dir_from_xdg_takes_priority() {
        let xdg = std::ffi::OsString::from("/tmp/xdg_test");
        let home = std::ffi::OsString::from("/tmp/fakehome");
        let p = data_dir_from(Some(xdg), Some(home)).expect("should resolve");
        assert_eq!(p, PathBuf::from("/tmp/xdg_test/dtxmaniars"));
    }

    #[test]
    fn data_dir_from_falls_back_to_home() {
        let home = std::ffi::OsString::from("/tmp/fakehome");
        let p = data_dir_from(None, Some(home)).expect("should resolve via HOME");
        assert_eq!(p, PathBuf::from("/tmp/fakehome/.config/dtxmaniars"));
    }

    #[test]
    fn data_dir_from_none_without_either() {
        let p = data_dir_from(None, None);
        assert_eq!(p, None);
    }

    #[test]
    fn user_data_dir_returns_current_path() {
        // No env manipulation — just verify the function returns *some*
        // valid PathBuf given the current process env.
        if std::env::var_os("XDG_CONFIG_HOME").is_some() || std::env::var_os("HOME").is_some() {
            let p = user_data_dir().expect("should resolve on a normal system");
            assert!(p.ends_with("dtxmaniars"));
        }
    }

    #[test]
    fn startup_scan_populates_empty_db() {
        let mut world = World::new();
        world.init_resource::<SongDb>();
        let mut db = world.resource_mut::<SongDb>();
        // Empty before scan.
        assert!(db.is_empty());
        // Direct call (no Bevy scheduler). Scan fixtures explicitly so
        // this test does not depend on the user's home dir having charts.
        let dir = fixture_dir();
        db.rescan(&dir).unwrap();
        assert!(!db.is_empty());
    }
}

pub mod song_db_sub_acts;
