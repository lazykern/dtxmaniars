//! DTX asset loading (Engine layer).
//!
//! Loads `.dtx` files from disk into [`dtx_core::Chart`] values and caches them.
//!
//! ## Reference
//! - `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs` (272KB) — DTX parser
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines)
//!
//! ## Scope (M4)
//!
//! File-based loading + in-memory cache. bevy AssetLoader integration lands in M5+.
//!
//! ponytail: stdlib `fs::read` + dtx_core::parse — no need for bevy's AssetLoader
//! machinery until we have 100s of DTX files to manage (M5+).

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use dtx_core::{Chart, ChartFormat, DtxError, ParseOptions, ParseReport, parse_source};

/// Load a DTX file from disk and parse it into a [`Chart`].
///
/// Errors:
/// - I/O errors (file not found, permission denied, etc.)
/// - Parse errors (malformed DTX) — see [`DtxError`]
pub fn load_dtx(path: &Path) -> Result<Chart, LoadError> {
    load_dtx_report(path).map(|report| report.chart)
}

/// Load a DTX file while retaining recoverable parser warnings.
pub fn load_dtx_report(path: &Path) -> Result<ParseReport, LoadError> {
    load_report_as(path, ChartFormat::Dtx)
}

/// Load any supported drums chart using its case-insensitive path extension.
pub fn load_chart_report(path: &Path) -> Result<ParseReport, LoadError> {
    let format = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(ChartFormat::from_extension)
        .ok_or_else(|| LoadError::UnsupportedFormat(path.to_path_buf()))?;
    load_report_as(path, format)
}

pub fn load_chart(path: &Path) -> Result<Chart, LoadError> {
    load_chart_report(path).map(|report| report.chart)
}

fn load_report_as(path: &Path, format: ChartFormat) -> Result<ParseReport, LoadError> {
    let file = fs::File::open(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_source(file, format, ParseOptions::default()).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Combined error type for DTX loading: I/O + parse.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("unsupported chart format: {0}")]
    UnsupportedFormat(PathBuf),
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("DTX parse error in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: DtxError,
    },
}

/// In-memory cache of loaded DTX charts. Resource for game-wide access.
///
/// M4: simple HashMap. M5+ will integrate bevy AssetLoader.
#[derive(Resource, Default, Debug)]
pub struct DtxCache {
    by_path: HashMap<PathBuf, Chart>,
}

impl DtxCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load (if not cached) and return a reference to the chart.
    pub fn get_or_load(&mut self, path: &Path) -> Result<&Chart, LoadError> {
        if !self.by_path.contains_key(path) {
            let path_buf = path.to_path_buf();
            let chart = load_chart(&path_buf)?;
            self.by_path.insert(path_buf, chart);
        }
        Ok(self.by_path.get(path).expect("just inserted"))
    }

    /// Direct insert (for tests).
    #[cfg(test)]
    pub fn insert(&mut self, path: PathBuf, chart: Chart) {
        self.by_path.insert(path, chart);
    }

    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }
}

/// Plugin: register the [`DtxCache`] resource. No systems (M4).
pub struct DtxAssetsPlugin;

impl Plugin for DtxAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DtxCache>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        // CARGO_MANIFEST_DIR/../dtx-core/tests/fixtures/drums_basic.dtx
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dtx-core")
            .join("tests")
            .join("fixtures")
            .join("drums_basic.dtx")
    }

    #[test]
    fn load_dtx_parses_drums_basic_fixture() {
        let chart = load_dtx(&fixture_path()).expect("fixture must load");
        assert!(!chart.chips.is_empty(), "fixture should have chips");
        assert!(chart.metadata.bpm.is_some(), "fixture should have BPM");
    }

    #[test]
    fn load_dtx_report_preserves_parser_warnings() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-assets-report-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        let path = dir.join("warning.dtx");
        std::fs::write(&path, b"#RANDOM: nope\n#00013: 01\n").expect("write chart");

        let report = load_dtx_report(&path).expect("chart must load");
        assert_eq!(report.chart.drum_chips().count(), 1);
        assert!(!report.warnings.is_empty());

        std::fs::remove_dir_all(dir).expect("remove fixture dir");
    }

    #[test]
    fn path_aware_loader_uses_legacy_format_and_rejects_bms() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-assets-format-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        let gda = dir.join("legacy.GDA");
        std::fs::write(&gda, b"#000BD: 01\n").expect("write gda");
        assert_eq!(
            load_chart_report(&gda).expect("gda loads").chart.format,
            dtx_core::ChartFormat::Gda
        );
        let bms = dir.join("keys.bms");
        std::fs::write(&bms, b"#00011: 01\n").expect("write bms");
        assert!(matches!(
            load_chart_report(&bms),
            Err(LoadError::UnsupportedFormat(_))
        ));
        std::fs::remove_dir_all(dir).expect("remove fixture dir");
    }

    #[test]
    fn load_dtx_errors_on_missing_file() {
        let path = PathBuf::from("/nonexistent/path/to/missing.dtx");
        let err = load_dtx(&path).expect_err("missing file must error");
        assert!(matches!(err, LoadError::Io { .. }));
    }

    #[test]
    fn cache_returns_same_chart_on_second_call() {
        let mut cache = DtxCache::new();
        let p = fixture_path();
        let first_len = cache.get_or_load(&p).unwrap().chips.len();
        let second_len = cache.get_or_load(&p).unwrap().chips.len();
        assert_eq!(first_len, second_len, "cache must return same chart");
        assert_eq!(cache.len(), 1, "cache should have one entry");
    }

    #[test]
    fn cache_misses_then_loads() {
        let mut cache = DtxCache::new();
        assert!(cache.is_empty());
        let _ = cache.get_or_load(&fixture_path()).unwrap();
        assert_eq!(cache.len(), 1);
    }
}
