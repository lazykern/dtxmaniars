//! Integration tests for archive import (pure logic, no Bevy).

use std::fs;
use std::path::{Path, PathBuf};

use dtx_library::import::{ImportError, import_archive};

/// Fresh, empty scratch dir under the system temp dir, unique per test name.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dtx-import-test-{}-{}", std::process::id(), name));
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
