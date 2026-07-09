use dtx_core::parse_str;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity, SectionId};

fn parse(input: &str) -> dtx_core::Chart {
    parse_str(input).expect("fixture must parse")
}

#[test]
fn canonical_hash_ignores_metadata_and_comments() {
    let a = parse(
        r#"
#TITLE: Song A
#ARTIST: Alice
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
; changed comment
#ARTIST: Bob
#TITLE: Song A fixed title
#BPM: 120
#00111: 0100
"#,
    );

    assert_eq!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn canonical_hash_changes_when_note_moves() {
    let a = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
#BPM: 120
#00111: 0010
"#,
    );

    assert_ne!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn canonical_hash_changes_when_timing_changes() {
    let a = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
#BPM: 121
#00111: 0100
"#,
    );

    assert_ne!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn section_id_uses_canonical_chart_hash_and_bars() {
    let chart = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let section = SectionId::new(canonical_chart_hash(&chart), 4, 8);
    assert_eq!(section.bar_start, 4);
    assert_eq!(section.bar_end, 8);
    assert!(section.canonical_chart_hash.starts_with("dtx1:"));
}

#[test]
fn raw_file_hash_is_plain_sha256_hex() {
    let dir = std::env::temp_dir().join(format!("dtx_identity_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chart.dtx");
    std::fs::write(&path, b"#TITLE: X\n").unwrap();

    let hash = raw_file_sha256(&path).unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn chart_identity_keeps_aliases_unique() {
    let mut id = ChartIdentity::legacy_raw("abc".to_string());
    id.add_raw_alias("def".to_string());
    id.add_raw_alias("def".to_string());

    assert_eq!(id.canonical_hash, "legacy-raw:abc");
    assert_eq!(id.raw_sha256.as_deref(), Some("abc"));
    assert_eq!(id.raw_sha256_aliases, vec!["def"]);
}
