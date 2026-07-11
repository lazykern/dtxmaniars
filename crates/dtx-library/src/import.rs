//! Chart archive import: extract a downloaded `.zip`/`.7z` into the song
//! library. Pure std + archive crates — no Bevy, fully unit-testable.
//!
//! Flow (see docs/superpowers/specs/2026-07-11-chart-archive-import-design.md):
//! 1. Detect format by magic bytes (extension lies).
//! 2. Extract to a temp dir INSIDE song_root (rename across filesystems fails).
//! 3. Sanitize every entry path (archives are untrusted downloads).
//! 4. Collapse redundant single-dir wrappers, require >= 1 `.dtx`.
//! 5. Move into `song_root/<name>/`; existing name = AlreadyImported, never overwrite.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use thiserror::Error;

#[derive(Debug)]
pub struct ImportOutcome {
    /// Final folder name placed in `song_root`.
    pub dest_name: String,
    /// Number of `.dtx` charts found under it.
    pub chart_count: usize,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("unsupported archive format: {0}")]
    UnsupportedFormat(String),
    #[error("archive contains unsafe entry paths")]
    UnsafePath,
    #[error("no .dtx charts found in archive")]
    NoCharts,
    #[error("already imported: {0}")]
    AlreadyImported(String),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Zip,
    SevenZ,
}

/// Sniff the archive format from magic bytes. Extension is ignored —
/// downloads are frequently misnamed.
fn detect_format(archive: &Path) -> Result<Format, ImportError> {
    let mut head = [0u8; 6];
    let mut f = fs::File::open(archive)?;
    let n = f.read(&mut head)?;
    let head = &head[..n];
    if head.starts_with(b"PK\x03\x04") {
        Ok(Format::Zip)
    } else if head.starts_with(b"7z\xBC\xAF\x27\x1C") {
        Ok(Format::SevenZ)
    } else if head.starts_with(b"Rar!") {
        Err(ImportError::UnsupportedFormat("rar".into()))
    } else {
        Err(ImportError::UnsupportedFormat("unknown".into()))
    }
}

/// Import a chart archive into `song_root`. See module docs for the flow.
pub fn import_archive(archive: &Path, song_root: &Path) -> Result<ImportOutcome, ImportError> {
    let _format = detect_format(archive)?;
    let _ = song_root;
    todo!("extraction lands in later tasks")
}
