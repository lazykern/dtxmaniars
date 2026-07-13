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
    pub formats: ChartFormatCounts,
    pub rejected: Vec<ImportChartDiagnostic>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChartFormatCounts {
    pub dtx: usize,
    pub gda: usize,
    pub g2d: usize,
}

impl ChartFormatCounts {
    pub const fn total(self) -> usize {
        self.dtx + self.gda + self.g2d
    }

    fn record(&mut self, format: dtx_core::ChartFormat) {
        match format {
            dtx_core::ChartFormat::Dtx => self.dtx += 1,
            dtx_core::ChartFormat::Gda => self.gda += 1,
            dtx_core::ChartFormat::G2d => self.g2d += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportChartDiagnostic {
    pub path: PathBuf,
    pub detail: String,
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

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Import a chart archive into `song_root`. See module docs for the flow.
pub fn import_archive(archive: &Path, song_root: &Path) -> Result<ImportOutcome, ImportError> {
    let format = detect_format(archive)?;
    fs::create_dir_all(song_root)?;
    clean_stale_temps(song_root);

    let temp = song_root.join(format!(
        ".import-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&temp)?;

    let result = (|| {
        match format {
            Format::Zip => extract_zip(archive, &temp)?,
            Format::SevenZ => extract_7z(archive, &temp)?,
        }

        let (content, wrapper_name) = collapse_wrappers(temp.clone())?;
        let (formats, rejected) = count_chart_formats(&content)?;
        let chart_count = formats.total();
        if chart_count == 0 {
            return Err(ImportError::NoCharts);
        }

        let dest_name = wrapper_name.unwrap_or_else(|| {
            archive
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "imported".to_owned())
        });
        let dest = song_root.join(&dest_name);
        if dest.exists() {
            return Err(ImportError::AlreadyImported(dest_name));
        }
        fs::rename(&content, &dest)?;
        Ok(ImportOutcome {
            dest_name,
            chart_count,
            formats,
            rejected,
        })
    })();

    // Content was renamed out on success; on failure this removes the
    // partial extraction. Either way the temp dir must not survive.
    let _ = fs::remove_dir_all(&temp);
    result
}

/// Remove leftover temp dirs from crashed imports by OTHER processes.
/// Our own live temps are excluded by pid so concurrent imports in this
/// process don't delete each other's work.
fn clean_stale_temps(song_root: &Path) {
    let own_prefix = format!(".import-{}-", std::process::id());
    let Ok(entries) = fs::read_dir(song_root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".import-") && !name.starts_with(&own_prefix) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), ImportError> {
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(io::Error::other)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(io::Error::other)?;
        let name = decode_name(entry.name_raw());
        let Some(rel) = sanitize(&name)? else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut f = fs::File::create(&out)?;
            io::copy(&mut entry, &mut f)?;
        }
    }
    Ok(())
}

fn extract_7z(archive: &Path, dest: &Path) -> Result<(), ImportError> {
    let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
        .map_err(io::Error::other)?;
    // The closure can only return the crate's error type, so ImportError
    // is smuggled out through this slot and iteration stopped early.
    let mut failed: Option<ImportError> = None;
    reader
        .for_each_entries(|entry, r| {
            let rel = match sanitize(entry.name()) {
                Ok(Some(rel)) => rel,
                Ok(None) => return Ok(true),
                Err(e) => {
                    failed = Some(e);
                    return Ok(false);
                }
            };
            let out = dest.join(rel);
            let io_result = (|| -> io::Result<()> {
                if entry.is_directory() {
                    fs::create_dir_all(&out)?;
                } else {
                    if let Some(parent) = out.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut f = fs::File::create(&out)?;
                    io::copy(r, &mut f)?;
                }
                Ok(())
            })();
            if let Err(e) = io_result {
                failed = Some(ImportError::Io(e));
                return Ok(false);
            }
            Ok(true)
        })
        .map_err(io::Error::other)?;
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Descend through redundant single-directory wrappers.
/// Returns the innermost content dir and the name of the last wrapper
/// entered (None if the archive root already held loose content).
fn collapse_wrappers(mut dir: PathBuf) -> io::Result<(PathBuf, Option<String>)> {
    let mut wrapper_name = None;
    loop {
        let entries: Vec<_> = fs::read_dir(&dir)?.flatten().collect();
        if entries.len() == 1 && entries[0].path().is_dir() {
            wrapper_name = Some(entries[0].file_name().to_string_lossy().into_owned());
            dir = entries[0].path();
        } else {
            return Ok((dir, wrapper_name));
        }
    }
}

/// Count `.dtx` files recursively. Shares the scanner's case-insensitive
/// extension rule so archive counts match a later rescan.
fn count_chart_formats(dir: &Path) -> io::Result<(ChartFormatCounts, Vec<ImportChartDiagnostic>)> {
    let mut formats = ChartFormatCounts::default();
    let mut rejected = Vec::new();
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let (nested_formats, mut nested_rejected) = count_chart_formats(&path)?;
            formats.dtx += nested_formats.dtx;
            formats.gda += nested_formats.gda;
            formats.g2d += nested_formats.g2d;
            rejected.append(&mut nested_rejected);
        } else {
            match crate::classify_chart_path(&path) {
                crate::ChartPathKind::Playable(format) => formats.record(format),
                crate::ChartPathKind::Rejected(_) => rejected.push(ImportChartDiagnostic {
                    path,
                    detail:
                        "BMS/BME is not supported by the drums player; convert to DTX, GDA, or G2D."
                            .into(),
                }),
                crate::ChartPathKind::NotAChart => {}
            }
        }
    }
    Ok((formats, rejected))
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
