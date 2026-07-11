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

/// Decode an archive entry name: UTF-8 if valid, else Shift-JIS
/// (Japanese chart packs commonly use it).
fn decode_name(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            let (s, _, _) = encoding_rs::SHIFT_JIS.decode(raw);
            s.into_owned()
        }
    }
}

/// Turn an untrusted entry name into a safe path relative to the extract dir.
/// zip-slip guard: `..` and drive prefixes are hard errors; absolute paths are
/// made relative. `Ok(None)` = nothing to write (pure directory marker).
fn sanitize(name: &str) -> Result<Option<PathBuf>, ImportError> {
    let name = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(ImportError::UnsafePath),
            p if p.contains(':') => return Err(ImportError::UnsafePath),
            p => out.push(p),
        }
    }
    if out.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_plain_path() {
        assert_eq!(
            sanitize("Song A/bsc.dtx").unwrap(),
            Some(PathBuf::from("Song A/bsc.dtx"))
        );
    }

    #[test]
    fn sanitize_normalizes_backslashes() {
        // Some Windows zippers write backslash separators.
        assert_eq!(
            sanitize(r"Song A\bsc.dtx").unwrap(),
            Some(PathBuf::from("Song A/bsc.dtx"))
        );
    }

    #[test]
    fn sanitize_rejects_parent_traversal() {
        assert!(matches!(
            sanitize("../evil.dtx"),
            Err(ImportError::UnsafePath)
        ));
        assert!(matches!(
            sanitize("a/../../evil"),
            Err(ImportError::UnsafePath)
        ));
    }

    #[test]
    fn sanitize_rejects_drive_prefix() {
        assert!(matches!(
            sanitize("C:/evil.dtx"),
            Err(ImportError::UnsafePath)
        ));
    }

    #[test]
    fn sanitize_defuses_absolute_path() {
        // Leading slash is stripped; result stays relative to dest.
        assert_eq!(
            sanitize("/etc/passwd").unwrap(),
            Some(PathBuf::from("etc/passwd"))
        );
    }

    #[test]
    fn sanitize_skips_empty_names() {
        // Pure directory markers like "dir/" leave nothing after the split.
        assert_eq!(sanitize("").unwrap(), None);
        assert_eq!(sanitize("/").unwrap(), None);
    }

    #[test]
    fn decode_name_utf8_passthrough() {
        assert_eq!(decode_name("曲/bsc.dtx".as_bytes()), "曲/bsc.dtx");
    }

    #[test]
    fn decode_name_shift_jis_fallback() {
        // "テスト" in Shift-JIS (invalid as UTF-8).
        let mut raw = b"\x83\x65\x83\x58\x83\x67".to_vec();
        raw.extend_from_slice(b"/test.dtx");
        assert_eq!(decode_name(&raw), "テスト/test.dtx");
    }
}
