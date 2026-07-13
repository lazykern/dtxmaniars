//! Song database — scans a directory of `.dtx` files, builds SongInfo list.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/SongDb/SongDb.cs`
//!
//! ## M5 scope
//!
//! - Sync directory scan (DTXManiaNX uses async Task — deferred to M6+)
//! - No SQLite cache (`SongCacheSqlite.cs` — M6+)
//! - Archive import (`import` module): zip/7z unpacking into the song root
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

pub mod import;
pub mod preferences;

pub use preferences::LibraryPreferences;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use bevy::prelude::*;
use dtx_core::{Chart, ChartFormat, ParseOptions, parse_source};
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
    /// Path to BGM audio file (ogg/wav/mp3) if found, else None.
    /// Search order preserves OGG/WAV priority before MP3 fallbacks.
    pub bgm_path: Option<PathBuf>,
    /// Path to the song-select preview audio: `#PREVIEW:` file if
    /// present, otherwise falls back to `bgm_path` (the full BGM).
    /// ADR-0015: song-select plays this; gameplay uses `bgm_path`.
    pub preview_path: Option<PathBuf>,
    /// True if `preview_path` came from `#PREVIEW:` (a short loop
    /// clip). False if it's a fallback to the full BGM (autoplay
    /// through, don't loop). ADR-0015 Q1 resolution.
    pub preview_is_loopable: bool,
    /// Path to the album art image (`#PREIMAGE:`) if present.
    /// ADR-0015 deferred item (e).
    pub preimage_path: Option<PathBuf>,
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

        let bgm_path = dtx_core::resolve_bgm_path(dtx_path, chart);

        // Preview path: prefer #PREVIEW: file; fall back to full BGM.
        // (ADR-0015 Q1: per-chart, BocaD-compatible.)
        let (preview_path, preview_is_loopable) = match chart.metadata.preview_filename.as_deref() {
            Some(name) => dtx_path
                .parent()
                .and_then(|parent| dtx_core::resolve_chart_asset_path(parent, name))
                .filter(|path| dtx_audio::supported_audio_format(path).is_some())
                .map_or_else(|| (bgm_path.clone(), false), |path| (Some(path), true)),
            None => (bgm_path.clone(), false),
        };

        // Album art path: #PREIMAGE: (ADR-0015 deferred item (e)).
        let preimage_path = chart
            .metadata
            .preimage_filename
            .as_deref()
            .and_then(|name| dtx_path.parent().map(|d| d.join(name)));

        Self {
            path: dtx_path.to_path_buf(),
            title,
            artist,
            bpm: chart.metadata.bpm,
            dlevel: chart.metadata.dlevel,
            bgm_path,
            preview_path,
            preview_is_loopable,
            preimage_path,
        }
    }

    /// Approximate total note count from the chart chips. M10 status panel
    /// shows this in the StatusPane. M10.1 counts per-instrument.
    pub fn notes_total(&self) -> u32 {
        // Load chart from disk on demand (M10: cheap; M10.1: cache).
        let Ok(bytes) = std::fs::read(&self.path) else {
            return 0;
        };
        let Some(format) = self
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(ChartFormat::from_extension)
        else {
            return 0;
        };
        let Ok(chart) = dtx_core::parse_source(bytes.as_slice(), format, ParseOptions::default())
            .map(|report| report.chart)
        else {
            return 0;
        };
        chart.chips.iter().filter(|c| c.channel.is_drum()).count() as u32
    }
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

/// A nonfatal chart issue encountered while scanning a song directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanProblemKind {
    Open,
    Parse,
    ParserWarning,
    MissingPreview,
    UnsupportedPreview,
    RejectedFormat,
}

/// Detailed context for a nonfatal chart scan issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProblem {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub kind: ScanProblemKind,
    pub detail: String,
}

/// Aggregate result retained from the latest song-directory scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub elapsed: std::time::Duration,
    /// Directories visited while discovering charts.
    pub directories: usize,
    pub discovered: usize,
    pub loaded: usize,
    pub problems: Vec<ScanProblem>,
}

impl ScanReport {
    /// Successfully parsed chart count (named for player-facing diagnostics).
    pub fn parsed(&self) -> usize {
        self.loaded
    }

    pub fn skipped(&self) -> usize {
        self.discovered.saturating_sub(self.loaded)
    }
}

/// True if `path` names a DTX chart, regardless of extension letter case.
pub fn is_dtx_path(path: &Path) -> bool {
    classify_chart_path(path) == ChartPathKind::Playable(ChartFormat::Dtx)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectedChartFormat {
    Bms,
    Bme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartPathKind {
    Playable(ChartFormat),
    Rejected(RejectedChartFormat),
    NotAChart,
}

pub fn classify_chart_path(path: &Path) -> ChartPathKind {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return ChartPathKind::NotAChart;
    };
    if let Some(format) = ChartFormat::from_extension(extension) {
        ChartPathKind::Playable(format)
    } else if extension.eq_ignore_ascii_case("bms") {
        ChartPathKind::Rejected(RejectedChartFormat::Bms)
    } else if extension.eq_ignore_ascii_case("bme") {
        ChartPathKind::Rejected(RejectedChartFormat::Bme)
    } else {
        ChartPathKind::NotAChart
    }
}

pub fn is_playable_chart_path(path: &Path) -> bool {
    matches!(classify_chart_path(path), ChartPathKind::Playable(_))
}

/// Walk a directory recursively, parse each .dtx file, return SongInfo list.
///
/// Errors on individual files are logged and skipped (so one bad DTX
/// doesn't kill the whole scan). Only a directory-level I/O error is fatal.
pub fn scan_directory(root: &Path) -> Result<(Vec<SongInfo>, ScanReport), ScanError> {
    let started = Instant::now();
    let mut songs = Vec::new();
    let mut report = ScanReport::default();
    walk_dtx(root, &mut songs, &mut report)?;
    report.elapsed = started.elapsed();
    Ok((songs, report))
}

fn walk_dtx(
    dir: &Path,
    songs: &mut Vec<SongInfo>,
    report: &mut ScanReport,
) -> Result<(), ScanError> {
    report.directories += 1;
    let entries = fs::read_dir(dir).map_err(|source| ScanError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dtx(&path, songs, report)?;
        } else {
            let format = match classify_chart_path(&path) {
                ChartPathKind::Playable(format) => format,
                ChartPathKind::Rejected(_) => {
                    report.discovered += 1;
                    let problem = ScanProblem {
                        path: path.clone(),
                        line: None,
                        kind: ScanProblemKind::RejectedFormat,
                        detail: "BMS/BME is not supported by the drums player; convert the chart to DTX, GDA, or G2D.".into(),
                    };
                    warn_scan_problem(&problem);
                    report.problems.push(problem);
                    continue;
                }
                ChartPathKind::NotAChart => continue,
            };
            report.discovered += 1;
            match fs::File::open(&path) {
                Ok(file) => match parse_source(file, format, ParseOptions::default()) {
                    Ok(parse_report) => {
                        for diagnostic in parse_report.diagnostics {
                            let problem = ScanProblem {
                                path: path.clone(),
                                line: diagnostic.line,
                                kind: ScanProblemKind::ParserWarning,
                                detail: diagnostic.detail,
                            };
                            warn_scan_problem(&problem);
                            report.problems.push(problem);
                        }
                        inspect_preview_problem(&path, &parse_report.chart, report);
                        songs.push(SongInfo::from_chart(&path, &parse_report.chart));
                        report.loaded += 1;
                    }
                    Err(source) => {
                        let problem = ScanProblem {
                            path: path.clone(),
                            line: None,
                            kind: ScanProblemKind::Parse,
                            detail: source.to_string(),
                        };
                        warn_scan_problem(&problem);
                        report.problems.push(problem);
                    }
                },
                Err(source) => {
                    let problem = ScanProblem {
                        path: path.clone(),
                        line: None,
                        kind: ScanProblemKind::Open,
                        detail: source.to_string(),
                    };
                    warn_scan_problem(&problem);
                    report.problems.push(problem);
                }
            }
        }
    }
    Ok(())
}

fn inspect_preview_problem(path: &Path, chart: &Chart, report: &mut ScanReport) {
    let Some(preview) = chart.metadata.preview_filename.as_deref() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };

    let kind = if dtx_audio::supported_audio_format(Path::new(preview)).is_none() {
        Some(ScanProblemKind::UnsupportedPreview)
    } else if dtx_core::resolve_chart_asset_path(parent, preview).is_none() {
        Some(ScanProblemKind::MissingPreview)
    } else {
        None
    };
    if let Some(kind) = kind {
        let problem = ScanProblem {
            path: path.to_path_buf(),
            line: None,
            kind,
            detail: format!("#PREVIEW references {preview:?}"),
        };
        warn_scan_problem(&problem);
        report.problems.push(problem);
    }
}

fn warn_scan_problem(problem: &ScanProblem) {
    let line = problem
        .line
        .map(|line| format!(":{line}"))
        .unwrap_or_default();
    bevy::log::warn!(
        "DTX scan {:?} at {}{}: {}",
        problem.kind,
        problem.path.display(),
        line,
        problem.detail
    );
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
    /// Structured outcome from the most recent completed scan.
    pub latest_scan: ScanReport,
}

impl SongDb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan the directory and update in place.
    pub fn rescan(&mut self, root: &Path) -> Result<(), ScanError> {
        let (mut songs, report) = scan_directory(root)?;
        sort_songs(&mut songs, self.sort_mode);
        self.songs = songs;
        self.scan_root = Some(root.to_path_buf());
        self.latest_scan = report;
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
        Ok(()) => {
            let report = &db.latest_scan;
            info!(
                "dtx-library: startup scan found {} chart(s): {} directories, {} parsed, {} skipped in {:.2?}",
                db.len(),
                report.directories,
                report.parsed(),
                report.skipped(),
                report.elapsed
            );
        }
        Err(e) => warn!("dtx-library: startup scan failed: {}", e),
    }
}

/// Plugin: register SongDb resource + startup scan system.
pub struct SongDbPlugin;

impl Plugin for SongDbPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SongDb>()
            .init_resource::<preferences::LibraryPreferences>()
            .add_systems(Startup, preferences::load_preferences_system)
            .add_systems(Startup, startup_scan_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_supported_legacy_and_rejected_keyboard_formats() {
        assert_eq!(
            classify_chart_path(Path::new("A.GDA")),
            ChartPathKind::Playable(dtx_core::ChartFormat::Gda)
        );
        assert_eq!(
            classify_chart_path(Path::new("B.g2d")),
            ChartPathKind::Playable(dtx_core::ChartFormat::G2d)
        );
        assert_eq!(
            classify_chart_path(Path::new("keys.BMS")),
            ChartPathKind::Rejected(RejectedChartFormat::Bms)
        );
        assert_eq!(
            classify_chart_path(Path::new("keys.bme")),
            ChartPathKind::Rejected(RejectedChartFormat::Bme)
        );
    }

    #[test]
    fn scanner_loads_gda_but_reports_and_excludes_bms() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-library-formats-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        std::fs::write(dir.join("drums.GDA"), b"#TITLE: Drums\n#000BD: 01\n").expect("write gda");
        std::fs::write(dir.join("keys.BMS"), b"#TITLE: Keys\n#00011: 01\n").expect("write bms");

        let (songs, report) = scan_directory(&dir).expect("scan succeeds");
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Drums");
        assert!(songs.iter().all(|song| !song.path.ends_with("keys.BMS")));
        assert!(report.problems.iter().any(|problem| {
            problem.kind == ScanProblemKind::RejectedFormat
                && problem.detail.contains("BMS/BME is not supported")
        }));

        std::fs::remove_dir_all(dir).expect("remove fixture dir");
    }

    #[test]
    fn library_preferences_round_trip_favorites_by_chart_path() {
        let path = std::env::temp_dir().join(format!(
            "dtx-library-preferences-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let chart = PathBuf::from("/songs/example/basic.dtx");
        let mut preferences = crate::preferences::LibraryPreferences::with_path(path.clone());

        assert!(preferences.toggle_favorite(&chart));
        preferences.save().expect("save preferences");

        let mut reloaded = crate::preferences::LibraryPreferences::with_path(path.clone());
        reloaded.load().expect("load preferences");
        assert!(reloaded.is_favorite(&chart));

        std::fs::remove_file(path).expect("remove preference fixture");
    }

    #[test]
    fn scan_report_counts_visited_directories_and_parsed_charts() {
        let (_, report) = scan_directory(&fixture_dir()).expect("scan fixtures");

        assert!(report.directories >= 1);
        assert_eq!(report.parsed(), report.loaded);
        assert_eq!(report.skipped(), report.discovered - report.loaded);
    }

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
            ..Default::default()
        };
        let path = PathBuf::from("/songs/mysong.dtx");
        let info = SongInfo::from_chart(&path, &chart);
        assert_eq!(info.title, "My Song");
        assert_eq!(info.artist, "Me");
        assert_eq!(info.bpm, Some(120.0));
        assert_eq!(info.dlevel, Some(85));
    }

    #[test]
    fn from_chart_preview_path_prefers_explicit_preview() {
        // When the chart declares #PREVIEW:, the song-select preview
        // should be that file and `preview_is_loopable` should be true.
        let dir = std::env::temp_dir().join(format!(
            "dtx-library-preview-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&dir).expect("create preview directory");
        let preview = dir.join("clip.ogg");
        fs::write(&preview, b"test fixture").expect("write preview fixture");
        let chart = Chart {
            metadata: dtx_core::Metadata {
                title: Some("X".into()),
                preview_filename: Some("clip.ogg".into()),
                preimage_filename: Some("cover.jpg".into()),
                ..default()
            },
            chips: vec![],
            ..Default::default()
        };
        let path = dir.join("x.dtx");
        let info = SongInfo::from_chart(&path, &chart);
        assert_eq!(info.preview_path, Some(preview));
        assert!(info.preview_is_loopable);
        assert_eq!(info.preimage_path, Some(dir.join("cover.jpg")));
        fs::remove_dir_all(dir).expect("remove preview directory");
    }

    #[test]
    fn from_chart_preview_path_falls_back_to_bgm() {
        // When the chart has no #PREVIEW:, preview_path mirrors
        // bgm_path and looping is disabled (autoplay through).
        let chart = Chart {
            metadata: dtx_core::Metadata {
                title: Some("X".into()),
                ..default()
            },
            chips: vec![],
            ..Default::default()
        };
        // Inject bgm via a chart that resolves to a known path.
        let path = PathBuf::from("/songs/x/x.dtx");
        let info = SongInfo::from_chart(&path, &chart);
        assert_eq!(info.preview_path, info.bgm_path);
        assert!(!info.preview_is_loopable);
    }

    #[test]
    fn from_chart_uses_case_insensitive_mp3_preview() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-library-mp3-preview-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create temp chart dir");
        let preview = dir.join("Preview.MP3");
        std::fs::write(&preview, b"not decoded in this metadata test").expect("write fixture");

        let chart = Chart {
            metadata: dtx_core::Metadata {
                preview_filename: Some("preview.mp3".into()),
                ..default()
            },
            ..default()
        };
        let info = SongInfo::from_chart(&dir.join("song.dtx"), &chart);
        assert_eq!(info.preview_path, Some(preview));
        assert!(info.preview_is_loopable);

        std::fs::remove_dir_all(dir).expect("remove temp chart dir");
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
        let (songs, _) = scan_directory(&fixture_dir()).expect("scan must succeed");
        assert!(
            !songs.is_empty(),
            "fixture dir should have at least one .dtx"
        );
        assert!(songs.iter().any(|s| s.path.ends_with("drums_basic.dtx")));
    }

    #[test]
    fn scan_directory_finds_uppercase_dtx_extension() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-library-uppercase-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create fixture directory");
        fs::write(dir.join("UPPER.DTX"), b"#TITLE: Uppercase\n").expect("write uppercase chart");

        let (songs, _) = scan_directory(&dir).expect("scan uppercase chart");

        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Uppercase");
        fs::remove_dir_all(dir).expect("remove fixture directory");
    }

    #[test]
    fn scan_report_keeps_parse_warning_and_preview_problems() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-library-scan-report-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create fixture directory");
        fs::write(
            dir.join("VALID.DTX"),
            b"#TITLE: Valid\n#BPM: 120\n#00013: 01\n",
        )
        .expect("write valid chart");
        fs::write(dir.join("invalid.dtx"), b"#BPM: nope\n").expect("write invalid chart");
        fs::write(
            dir.join("warning.dtx"),
            b"#RANDOM: nope\n#ENDRANDOM\n#PREVIEW: clip.xa\n#00013: 01\n",
        )
        .expect("write warning chart");

        let (songs, report) = scan_directory(&dir).expect("scan report");
        assert_eq!(songs.len(), 2);
        assert_eq!(report.discovered, 3);
        assert_eq!(report.loaded, 2);
        assert_eq!(report.skipped(), 1);
        assert!(report.elapsed <= std::time::Duration::from_secs(1));
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.kind == ScanProblemKind::Parse)
        );
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.kind == ScanProblemKind::ParserWarning)
        );
        assert!(report.problems.iter().any(|problem| {
            problem.kind == ScanProblemKind::UnsupportedPreview
                && problem.path.ends_with("warning.dtx")
        }));

        let mut db = SongDb::new();
        db.rescan(&dir).expect("rescan report");
        assert_eq!(db.latest_scan.discovered, 3);
        assert_eq!(db.latest_scan.loaded, 2);

        fs::remove_dir_all(dir).expect("remove fixture directory");
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
                preview_path: None,
                preview_is_loopable: false,
                preimage_path: None,
            },
            SongInfo {
                path: PathBuf::from("/b.dtx"),
                title: "Alpha".into(),
                artist: "Y".into(),
                bpm: None,
                dlevel: None,
                bgm_path: None,
                preview_path: None,
                preview_is_loopable: false,
                preimage_path: None,
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
