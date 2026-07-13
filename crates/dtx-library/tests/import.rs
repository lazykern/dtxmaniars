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

use std::io::Write;

/// Build a zip at `path` from (entry-name, bytes) pairs using the zip crate.
fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
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
    assert!(
        !root
            .join("Zattou Bokura no Machi/Zattou Bokura no Machi")
            .exists()
    );
}

#[test]
fn zip_uppercase_dtx_counts_as_chart() {
    let dir = test_dir("uppercase");
    let archive = dir.join("Uppercase.zip");
    make_zip(&archive, &[("Song/MASTER.DTX", DTX)]);
    let root = dir.join("songs");

    let out = import_archive(&archive, &root).expect("uppercase chart imports");

    assert_eq!(out.chart_count, 1);
    assert!(root.join("Song/MASTER.DTX").is_file());
}

#[test]
fn mixed_format_archive_reports_playable_and_rejected_counts() {
    let dir = test_dir("mixed-formats");
    let archive = dir.join("Mixed.zip");
    make_zip(
        &archive,
        &[
            ("Pack/basic.dtx", DTX),
            ("Pack/advanced.GDA", DTX),
            ("Pack/legacy.g2d", DTX),
            ("Pack/keys.bms", DTX),
        ],
    );
    let out = import_archive(&archive, &dir.join("songs")).expect("playable charts import");
    assert_eq!(out.chart_count, 3);
    assert_eq!(out.formats.dtx, 1);
    assert_eq!(out.formats.gda, 1);
    assert_eq!(out.formats.g2d, 1);
    assert_eq!(out.rejected.len(), 1);
    assert!(out.rejected[0].detail.contains("BMS/BME is not supported"));
}

#[test]
fn archive_reports_xa_substitution_without_running_a_converter() {
    let dir = test_dir("xa-substitution");
    let archive = dir.join("XaFallback.zip");
    make_zip(
        &archive,
        &[
            (
                "Song/chart.dtx",
                b"#WAV01: MUSIC.xa\n#00001: 01\n#00012: 01\n",
            ),
            ("Song/music.OGG", b"ogg"),
        ],
    );

    let out = import_archive(&archive, &dir.join("songs")).expect("XA chart imports");

    assert_eq!(out.media_diagnostics.len(), 1);
    assert!(out.media_diagnostics[0].detail.contains("substituted"));
    assert!(out.media_diagnostics[0].detail.contains("music.OGG"));
}

#[test]
fn archive_reports_unresolved_required_xa_with_recovery_guidance() {
    let dir = test_dir("xa-required");
    let archive = dir.join("XaRequired.zip");
    make_zip(
        &archive,
        &[(
            "Song/chart.dtx",
            b"#WAV01: music.xa\n#00001: 01\n#00012: 01\n",
        )],
    );

    let out = import_archive(&archive, &dir.join("songs")).expect("archive still imports");

    assert_eq!(out.media_diagnostics.len(), 1);
    assert!(out.media_diagnostics[0].detail.contains("required BGM"));
    assert!(out.media_diagnostics[0].detail.contains("OGG, WAV, or MP3"));
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
