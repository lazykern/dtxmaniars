# Chart Archive Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users import downloaded `.zip` / `.7z` chart archives into the song library via drag-and-drop or an F6 file picker, cross-platform.

**Architecture:** A pure, Bevy-free `import_archive()` function in `dtx-library` does format detection, safe extraction to a temp dir inside the song root, wrapper-collapse placement, and validation. A thin Bevy module in `game-menu` wires drag-drop events, the F6 picker (rfd), a background import thread, a result toast, and rescan-on-success.

**Tech Stack:** Rust 2024, Bevy 0.19 (Messages, not Events), `zip`, `sevenz-rust2`, `encoding_rs`, `rfd`. Spec: `docs/superpowers/specs/2026-07-11-chart-archive-import-design.md`.

**Key constraints (why the code looks the way it does):**
- Temp extraction dir MUST live inside `song_root` — `fs::rename` fails across filesystems (e.g. tmpfs `/tmp` → disk `~/.config`).
- The scanner (`walk_dtx` in `crates/dtx-library/src/lib.rs:151`) matches extension `== Some("dtx")` (lowercase, case-sensitive). Chart counting must use the same rule.
- Archives are untrusted downloads: every entry name is sanitized (zip-slip guard) before any write.
- Japanese packs often store zip entry names in Shift-JIS, not UTF-8.
- macOS requires native file dialogs on the main thread → picker system takes `NonSendMarker`.
- Repo convention: library logic is Bevy-free and unit-tested; Bevy glue is thin and untested (see commit `dd1facf`).

---

## File Structure

- Create: `crates/dtx-library/src/import.rs` — all import logic (pure, no Bevy)
- Create: `crates/dtx-library/tests/import.rs` — integration tests
- Create: `crates/game-menu/src/import_ui.rs` — Bevy glue (drag-drop, F6 picker, worker thread, toast)
- Modify: `crates/dtx-library/src/lib.rs` — add `pub mod import;`
- Modify: `crates/dtx-library/Cargo.toml` — add deps
- Modify: `crates/game-menu/Cargo.toml` — add `rfd`
- Modify: `crates/game-menu/src/lib.rs` — register `import_ui::plugin`
- Modify: `crates/game-menu/src/song_select.rs` — legend entry + no-songs hint text

---

### Task 1: Dependencies

**Files:**
- Modify: `crates/dtx-library/Cargo.toml`
- Modify: `crates/game-menu/Cargo.toml`

- [ ] **Step 1: Add extraction deps to dtx-library**

In `crates/dtx-library/Cargo.toml` under `[dependencies]` add:

```toml
zip = "5"
sevenz-rust2 = "0.21"
encoding_rs = "0.8"
```

And add a dev-dependencies section (for building test fixtures):

```toml
[dev-dependencies]
crc32fast = "1"
sevenz-rust2 = { version = "0.21", features = ["compress"] }
```

Note: if `zip = "5"` fails to resolve, run `cargo add zip -p dtx-library` and take whatever latest stable cargo picks (the APIs used — `ZipArchive::new`, `by_index`, `name_raw`, `is_dir` — are stable across recent majors). Do NOT take a pre-release (e.g. `9.0.0-pre2`).

- [ ] **Step 2: Add rfd to game-menu**

In `crates/game-menu/Cargo.toml` under `[dependencies]` add:

```toml
rfd = "0.17"
```

- [ ] **Step 3: Verify it builds**

Run: `cargo check -p dtx-library -p game-menu`
Expected: clean check (downloads new crates first time).

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-library/Cargo.toml crates/game-menu/Cargo.toml Cargo.lock
git commit -m "chore(library): add zip, 7z, encoding, dialog deps for import"
```

---

### Task 2: Format detection + error/outcome types

**Files:**
- Create: `crates/dtx-library/src/import.rs`
- Modify: `crates/dtx-library/src/lib.rs` (add `pub mod import;`)
- Create: `crates/dtx-library/tests/import.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/dtx-library/tests/import.rs`:

```rust
//! Integration tests for archive import (pure logic, no Bevy).

use std::fs;
use std::path::{Path, PathBuf};

use dtx_library::import::{import_archive, ImportError};

/// Fresh, empty scratch dir under the system temp dir, unique per test name.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("dtx-import-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn rejects_rar_by_magic() {
    let dir = test_dir("rar");
    let archive = dir.join("pack.zip"); // wrong extension on purpose: magic wins
    write_file(&archive, b"Rar!\x1a\x07\x00rest-of-file");
    let root = dir.join("songs");
    match import_archive(&archive, &root) {
        Err(ImportError::UnsupportedFormat(f)) => assert_eq!(f, "rar"),
        other => panic!("expected UnsupportedFormat(rar), got {other:?}"),
    }
}

#[test]
fn rejects_unknown_bytes() {
    let dir = test_dir("unknown");
    let archive = dir.join("pack.zip");
    write_file(&archive, b"not an archive at all");
    let root = dir.join("songs");
    assert!(matches!(
        import_archive(&archive, &root),
        Err(ImportError::UnsupportedFormat(_))
    ));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-library --test import`
Expected: compile error — `dtx_library::import` does not exist.

- [ ] **Step 3: Create the module skeleton**

Add to `crates/dtx-library/src/lib.rs` (near the top, after the crate docs):

```rust
pub mod import;
```

Create `crates/dtx-library/src/import.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-library --test import`
Expected: both tests PASS (they only exercise `detect_format` via the public fn, which errors before the `todo!`).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library/src/import.rs crates/dtx-library/src/lib.rs crates/dtx-library/tests/import.rs
git commit -m "feat(library): detect archive format by magic bytes"
```

---

### Task 3: Entry-name sanitizing + Shift-JIS decoding

**Files:**
- Modify: `crates/dtx-library/src/import.rs`

These are internal helpers; test them as unit tests inside the module (they never touch the filesystem).

- [ ] **Step 1: Write failing unit tests**

Append to `crates/dtx-library/src/import.rs`:

```rust
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
        assert!(matches!(sanitize("../evil.dtx"), Err(ImportError::UnsafePath)));
        assert!(matches!(sanitize("a/../../evil"), Err(ImportError::UnsafePath)));
    }

    #[test]
    fn sanitize_rejects_drive_prefix() {
        assert!(matches!(sanitize("C:/evil.dtx"), Err(ImportError::UnsafePath)));
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-library --lib import`
Expected: compile error — `sanitize` / `decode_name` not defined.

- [ ] **Step 3: Implement the helpers**

Add to `crates/dtx-library/src/import.rs` (above the test module):

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-library --lib import`
Expected: all 8 unit tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library/src/import.rs
git commit -m "feat(library): sanitize archive entry paths, decode Shift-JIS names"
```

---

### Task 4: Zip extraction

**Files:**
- Modify: `crates/dtx-library/src/import.rs`
- Modify: `crates/dtx-library/tests/import.rs`

- [ ] **Step 1: Add zip-building test helpers + failing tests**

The integration tests need two zip builders: a normal one (via the `zip` crate) and a raw one that can write arbitrary (non-UTF-8, unsafe) entry name bytes, which the `zip` writer API refuses. Append to `crates/dtx-library/tests/import.rs`:

```rust
use std::io::Write;

/// Build a zip at `path` from (entry-name, bytes) pairs using the zip crate.
fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for (name, data) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap();
}

fn push_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Handcraft a stored (uncompressed) zip with RAW entry-name bytes.
/// Needed because the zip crate's writer only accepts &str names, but we
/// must test Shift-JIS and `../` names as they appear in the wild.
fn make_raw_zip(path: &Path, entries: &[(&[u8], &[u8])]) {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut offsets = Vec::new();
    for (name, data) in entries {
        offsets.push(out.len() as u32);
        let crc = crc32fast::hash(data);
        // Local file header
        push_u32(&mut out, 0x0403_4b50);
        push_u16(&mut out, 20); // version needed
        push_u16(&mut out, 0); // flags (bit 11 unset: name is NOT declared UTF-8)
        push_u16(&mut out, 0); // method: stored
        push_u16(&mut out, 0); // mod time
        push_u16(&mut out, 0); // mod date
        push_u32(&mut out, crc);
        push_u32(&mut out, data.len() as u32); // compressed size
        push_u32(&mut out, data.len() as u32); // uncompressed size
        push_u16(&mut out, name.len() as u16);
        push_u16(&mut out, 0); // extra len
        out.extend_from_slice(name);
        out.extend_from_slice(data);
    }
    for (i, (name, data)) in entries.iter().enumerate() {
        let crc = crc32fast::hash(data);
        // Central directory header
        push_u32(&mut central, 0x0201_4b50);
        push_u16(&mut central, 20); // version made by
        push_u16(&mut central, 20); // version needed
        push_u16(&mut central, 0); // flags
        push_u16(&mut central, 0); // method
        push_u16(&mut central, 0); // mod time
        push_u16(&mut central, 0); // mod date
        push_u32(&mut central, crc);
        push_u32(&mut central, data.len() as u32);
        push_u32(&mut central, data.len() as u32);
        push_u16(&mut central, name.len() as u16);
        push_u16(&mut central, 0); // extra len
        push_u16(&mut central, 0); // comment len
        push_u16(&mut central, 0); // disk number
        push_u16(&mut central, 0); // internal attrs
        push_u32(&mut central, 0); // external attrs
        push_u32(&mut central, offsets[i]);
        central.extend_from_slice(name);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    // End of central directory
    push_u32(&mut out, 0x0605_4b50);
    push_u16(&mut out, 0); // disk
    push_u16(&mut out, 0); // cd start disk
    push_u16(&mut out, entries.len() as u16);
    push_u16(&mut out, entries.len() as u16);
    push_u32(&mut out, central.len() as u32);
    push_u32(&mut out, cd_offset);
    push_u16(&mut out, 0); // comment len
    fs::write(path, out).unwrap();
}
```

Then the extraction tests (spec test cases 1–6 for zip):

```rust
const DTX: &[u8] = b"#TITLE: test\n";

#[test]
fn zip_single_wrapper_no_double_nest() {
    let dir = test_dir("wrapper");
    let archive = dir.join("Zattou Bokura no Machi.zip");
    make_zip(
        &archive,
        &[
            ("Zattou Bokura no Machi/mas.dtx", DTX),
            ("Zattou Bokura no Machi/036_Kick.ogg", b"ogg"),
        ],
    );
    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "Zattou Bokura no Machi");
    assert_eq!(out.chart_count, 1);
    assert!(root.join("Zattou Bokura no Machi/mas.dtx").is_file());
    // no double nest:
    assert!(!root
        .join("Zattou Bokura no Machi/Zattou Bokura no Machi")
        .exists());
}

#[test]
fn zip_bare_files_get_archive_name_folder() {
    let dir = test_dir("bare");
    let archive = dir.join("MySong.zip");
    make_zip(&archive, &[("mas.dtx", DTX), ("kick.ogg", b"ogg")]);
    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "MySong");
    assert!(root.join("MySong/mas.dtx").is_file());
}

#[test]
fn zip_wrapper_with_multiple_songs_kept_together() {
    let dir = test_dir("pack");
    let archive = dir.join("BigPack.zip");
    make_zip(
        &archive,
        &[
            ("Pack Vol.1/Song A/bsc.dtx", DTX),
            ("Pack Vol.1/Song B/bsc.dtx", DTX),
        ],
    );
    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "Pack Vol.1");
    assert_eq!(out.chart_count, 2);
    assert!(root.join("Pack Vol.1/Song A/bsc.dtx").is_file());
    assert!(root.join("Pack Vol.1/Song B/bsc.dtx").is_file());
}

#[test]
fn zip_multiple_root_dirs_get_archive_name_folder() {
    let dir = test_dir("multiroot");
    let archive = dir.join("TwoSongs.zip");
    make_zip(
        &archive,
        &[("Song A/bsc.dtx", DTX), ("Song B/bsc.dtx", DTX)],
    );
    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "TwoSongs");
    assert_eq!(out.chart_count, 2);
    assert!(root.join("TwoSongs/Song A/bsc.dtx").is_file());
}

#[test]
fn zip_slip_rejected_nothing_written() {
    let dir = test_dir("slip");
    let archive = dir.join("evil.zip");
    make_raw_zip(&archive, &[(b"../evil.dtx".as_slice(), DTX)]);
    let root = dir.join("songs");
    assert!(matches!(
        import_archive(&archive, &root),
        Err(ImportError::UnsafePath)
    ));
    assert!(!dir.join("evil.dtx").exists());
    // song_root holds no leftovers (temp cleaned up):
    let leftovers: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "leftovers: {leftovers:?}");
}

#[test]
fn zip_shift_jis_names_decoded() {
    let dir = test_dir("sjis");
    let archive = dir.join("jp.zip");
    // "テスト" in Shift-JIS as the wrapper dir name.
    let mut name = b"\x83\x65\x83\x58\x83\x67".to_vec();
    name.extend_from_slice(b"/mas.dtx");
    make_raw_zip(&archive, &[(name.as_slice(), DTX)]);
    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "テスト");
    assert!(root.join("テスト/mas.dtx").is_file());
}

#[test]
fn no_charts_rejected() {
    let dir = test_dir("nocharts");
    let archive = dir.join("photos.zip");
    make_zip(&archive, &[("vacation/img.png", b"png")]);
    let root = dir.join("songs");
    assert!(matches!(
        import_archive(&archive, &root),
        Err(ImportError::NoCharts)
    ));
    let leftovers: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert!(leftovers.is_empty());
}

#[test]
fn second_import_same_name_skipped() {
    let dir = test_dir("dup");
    let archive = dir.join("MySong.zip");
    make_zip(&archive, &[("MySong/mas.dtx", DTX)]);
    let root = dir.join("songs");
    import_archive(&archive, &root).unwrap();
    match import_archive(&archive, &root) {
        Err(ImportError::AlreadyImported(name)) => assert_eq!(name, "MySong"),
        other => panic!("expected AlreadyImported, got {other:?}"),
    }
    // original untouched, exactly one visible folder in root:
    assert!(root.join("MySong/mas.dtx").is_file());
    let visible: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert_eq!(visible.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-library --test import`
Expected: new tests FAIL (panic on the `todo!` in `import_archive`). The two Task-2 tests still pass.

- [ ] **Step 3: Implement zip extraction, placement, and the full `import_archive`**

Replace the `import_archive` stub in `crates/dtx-library/src/import.rs` and add the helpers:

```rust
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
        let chart_count = count_dtx(&content)?;
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
    let mut reader =
        sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
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

/// Count `.dtx` files recursively. Same extension rule as the scanner's
/// `walk_dtx` (lowercase, case-sensitive) so the count matches what a
/// rescan will actually pick up.
fn count_dtx(dir: &Path) -> io::Result<usize> {
    let mut n = 0;
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            n += count_dtx(&path)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("dtx") {
            n += 1;
        }
    }
    Ok(n)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-library --test import`
Expected: all 10 integration tests PASS.

Also run: `cargo test -p dtx-library --lib import`
Expected: 8 unit tests still PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library/src/import.rs crates/dtx-library/tests/import.rs
git commit -m "feat(library): import zip chart archives with wrapper collapse"
```

---

### Task 5: 7z extraction test

`extract_7z` was implemented in Task 4 (it shares all logic with zip except the reader loop); this task proves it works end-to-end with a real 7z file.

**Files:**
- Modify: `crates/dtx-library/tests/import.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/dtx-library/tests/import.rs`:

```rust
#[test]
fn sevenz_wrapper_imported() {
    let dir = test_dir("sevenz");
    // Build a source tree, then compress it with the crate's own writer
    // (dev-dependency has the `compress` feature).
    let src = dir.join("src/My7zSong");
    write_file(&src.join("mas.dtx"), DTX);
    write_file(&src.join("kick.ogg"), b"ogg");
    let archive = dir.join("My7zSong.7z");
    sevenz_rust2::compress_to_path(dir.join("src"), &archive).unwrap();

    let root = dir.join("songs");
    let out = import_archive(&archive, &root).unwrap();
    assert_eq!(out.dest_name, "My7zSong");
    assert_eq!(out.chart_count, 1);
    assert!(root.join("My7zSong/mas.dtx").is_file());
}
```

Note: if `compress_to_path(dir.join("src"), ...)` produces entry names that include or exclude the top-level dir differently than expected, adjust the assertion to whatever single wrapper collapse yields — the invariant to keep is: one wrapper dir in, `dest_name` == that wrapper, chart inside it. Check with `fs::read_dir` debugging if needed.

- [ ] **Step 2: Run test to verify current state**

Run: `cargo test -p dtx-library --test import sevenz`
Expected: PASS if Task 4's `extract_7z` is correct; if it FAILS, fix `extract_7z` (not the test) until green.

- [ ] **Step 3: Commit**

```bash
git add crates/dtx-library/tests/import.rs
git commit -m "test(library): cover 7z archive import end-to-end"
```

---

### Task 6: Bevy glue — worker, drag-drop, F6 picker, toast

**Files:**
- Create: `crates/game-menu/src/import_ui.rs`
- Modify: `crates/game-menu/src/lib.rs` (register module + plugin)

No tests (thin Bevy glue, repo convention). Verified by compile + the manual check in Task 7.

- [ ] **Step 1: Find how song_select's plugin is registered**

Run: `grep -n "song_select" crates/game-menu/src/lib.rs`
Note the `mod song_select;` line and where `song_select::plugin` is added to the app — `import_ui` gets registered the same way, right next to it.

Also confirm the exact import paths used by `song_select.rs` for `AppState`, `Theme`, and `SongDb`:
Run: `sed -n '1,40p' crates/game-menu/src/song_select.rs`
Use the same paths in `import_ui.rs`.

- [ ] **Step 2: Create `crates/game-menu/src/import_ui.rs`**

Adjust the `use` lines to match what Step 1 found (the code below assumes the same imports song_select uses):

```rust
//! Chart archive import UI: drag-and-drop + F6 file picker on the song
//! select screen. All real logic lives in `dtx_library::import`; this
//! module only moves paths to a worker thread and shows the outcome.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;

use dtx_library::import::{import_archive, ImportError, ImportOutcome};
use dtx_library::{default_song_dir, SongDb};
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

fn spawn_toast_node(mut commands: Commands, theme: Res<Theme>) {
    commands.spawn((
        ToastNode,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            top: Val::Px(80.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(theme.stage_panel_bg),
        Text::new(""),
        Theme::font(16.0),
        TextColor(theme.text_secondary),
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
```

Adaptation notes for the executor (verify against the codebase, don't guess):
- `MessageReader<FileDragAndDrop>`: this repo is Bevy 0.19 where events are Messages (see `MessageReader<NavAction>` at `crates/game-menu/src/song_select.rs:1519`). If `FileDragAndDrop` is not a Message in 0.19, check `bevy::window` docs for the 0.19 reading pattern and adapt.
- `NonSendMarker`: import from wherever Bevy 0.19 exposes it (`bevy::ecs::system::NonSendMarker` or prelude). If absent, an `Option<NonSend<()>>` parameter achieves the same main-thread pinning.
- `Theme` fields (`stage_panel_bg`, `text_secondary`) and `Theme::font` are used exactly as in `spawn_wheel_rows` (`crates/game-menu/src/song_select.rs:908-915`) — copy from there if the names differ.
- `despawn()`: if the repo uses a `despawn_stage`-style helper or `despawn_recursive`, match local convention.

- [ ] **Step 3: Register the module**

In `crates/game-menu/src/lib.rs`, next to the `song_select` module declaration and plugin registration found in Step 1, add:

```rust
mod import_ui;
```

and register `import_ui::plugin` in the same place `song_select::plugin` is registered (same `add_plugins`/function-call style).

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p game-menu`
Expected: clean. Fix any import-path mismatches per the adaptation notes.

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/import_ui.rs crates/game-menu/src/lib.rs
git commit -m "feat(menu): import chart archives via drag-drop and F6 picker"
```

---

### Task 7: Discoverability text + final verification

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (two text tweaks)

- [ ] **Step 1: Add F6 to the nav legend**

At `crates/game-menu/src/song_select.rs:858` (search for `"F5 RESCAN"`), the legend items list contains `("F5 RESCAN", false)`. Add immediately after it:

```rust
("F6 IMPORT", false),
```

- [ ] **Step 2: Mention import in the no-songs message**

At `crates/game-menu/src/song_select.rs:910` (search for `no songs found`), change the format string:

```rust
Text::new(format!(
    "no songs found — put song folders in {}\npress F5 to rescan, F6 to import an archive, or drop a .zip here",
    dtx_library::default_song_dir().display()
)),
```

- [ ] **Step 3: Full workspace check**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets 2>&1 | tail -20 && cargo test --workspace 2>&1 | tail -20`
Expected: fmt clean, no new clippy warnings, all tests pass.

- [ ] **Step 4: Manual verification (use the superpowers:verification-before-completion skill)**

1. Build a real test zip: `cd /tmp && mkdir -p "Test Song" && printf '#TITLE: Manual Test\n' > "Test Song/mas.dtx" && zip -r test-import.zip "Test Song"`
2. Run the game: `cargo run -p dtxmaniars-desktop` (or the repo's usual run command).
3. On song select, press **F6**, pick `/tmp/test-import.zip` → expect toast `imported "Test Song" (1 chart)` and the song appears in the wheel.
4. Press F6 and import the same file again → expect toast `already imported: "Test Song"`.
5. Drag the zip from a file manager onto the window → same already-imported toast (proves drag-drop path).
6. Clean up: remove `Test Song` from the song dir.

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(menu): surface F6 import in legend and empty-library hint"
```

---

## Spec coverage checklist

- Magic-byte detection (zip/7z/rar-reject) → Task 2
- Temp-dir extraction, never pollute library → Task 4 (`import_archive` closure + cleanup)
- zip-slip guard → Task 3 (`sanitize`) + Task 4 test
- Shift-JIS filename fallback → Task 3 (`decode_name`) + Task 4 test
- Wrapper-collapse placement, all 4 archive shapes → Task 4
- ≥1 `.dtx` validation (`NoCharts`) → Task 4
- Collision → `AlreadyImported`, never overwrite → Task 4
- Drag & drop → Task 6 (`dropped_files`)
- F6 picker (rfd, multi-select, zip/7z filter) → Task 6 (`import_picker`)
- Background thread + channel polling → Task 6 (`start_import` / `poll_imports`)
- Rescan on success + all toast messages → Task 6 (`poll_imports`)
- All 8 spec test cases → Tasks 2–5
