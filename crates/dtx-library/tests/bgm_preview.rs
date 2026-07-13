//! Integration test: SongDb + BGM preview wiring.
//!
//! Verifies that when SongDb scans a directory containing a DTX + matching
//! OGG, the SongInfo has `bgm_path` set. The full play_bgm call requires
//! a Bevy audio backend and is covered manually in the binary.

use std::path::PathBuf;

use dtx_library::{SongInfo, scan_directory};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dtx-core")
        .join("tests")
        .join("fixtures")
}

#[test]
fn scan_finds_with_bgm_dtx() {
    let (songs, _) = scan_directory(&fixture_dir()).expect("scan must succeed");
    let bgm_song = songs
        .iter()
        .find(|s| s.title == "BGM Test")
        .expect("BGM Test fixture should be in scan results");
    assert_eq!(bgm_song.artist, "Test");
    assert_eq!(bgm_song.bpm, Some(120.0));
    assert_eq!(bgm_song.dlevel, Some(50));
}

#[test]
fn bgm_path_resolves_for_with_bgm() {
    let (songs, _) = scan_directory(&fixture_dir()).expect("scan must succeed");
    let bgm_song = songs
        .iter()
        .find(|s| s.title == "BGM Test")
        .expect("BGM Test fixture should be scanned");
    let bgm = bgm_song
        .bgm_path
        .as_ref()
        .expect("with_bgm.dtx should have bgm_path set (matches with_bgm.ogg)");
    assert!(bgm.exists(), "BGM file should exist: {:?}", bgm);
    assert!(bgm.extension().is_some());
}

#[test]
fn other_dtx_have_no_bgm() {
    let (songs, _) = scan_directory(&fixture_dir()).expect("scan must succeed");
    let minimal = songs
        .iter()
        .find(|s| s.title == "Minimal Test")
        .expect("minimal.dtx fixture");
    assert!(
        minimal.bgm_path.is_none(),
        "minimal.dtx has no .ogg sibling, bgm_path must be None"
    );
}

#[test]
fn song_info_clone_for_bgm_preview() {
    // Verifies SongInfo is Clone-able for the bgm_preview_on_change Local cache.
    let (songs, _) = scan_directory(&fixture_dir()).unwrap();
    let song: SongInfo = songs
        .iter()
        .find(|s| s.title == "BGM Test")
        .unwrap()
        .clone();
    let bgm = song.bgm_path.as_ref().unwrap().clone();
    let _copy: SongInfo = song.clone(); // ensure clone works
    assert!(bgm.exists());
}
