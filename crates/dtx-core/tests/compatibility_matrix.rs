use std::path::PathBuf;

use dtx_core::{parse, parse_str, parse_with_options, EChannel, ParseOptions};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn parser_preserves_titles_and_drum_chips_across_legacy_encodings() {
    let text = "#TITLE: テスト\n#BPM: 120\n#00013: 01\n";
    let utf8 = parse(text.as_bytes()).expect("utf-8 parses");
    let (shift_jis, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    let shift_jis = parse(shift_jis.as_ref()).expect("shift-jis parses");
    let mut utf16le = vec![0xff, 0xfe];
    utf16le.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let utf16le = parse(utf16le.as_slice()).expect("utf-16le parses");
    let mut utf16be = vec![0xfe, 0xff];
    utf16be.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
    let utf16be = parse(utf16be.as_slice()).expect("utf-16be parses");
    for chart in [utf8, shift_jis, utf16le, utf16be] {
        assert_eq!(chart.metadata.title.as_deref(), Some("テスト"));
        assert_eq!(chart.drum_chips().count(), 1);
    }
}

#[test]
fn fixtures_cover_conditionals_mp3_high_se_and_missing_assets() {
    for name in ["conditional_branches.dtx", "conditional_nested.dtx"] {
        let bytes = std::fs::read(fixture(name)).expect("fixture exists");
        let report = parse_with_options(bytes.as_slice(), ParseOptions { random_seed: 0 })
            .expect("conditional fixture parses");
        let repeated = parse_with_options(bytes.as_slice(), ParseOptions { random_seed: 0 })
            .expect("conditional fixture parses repeatedly");
        assert_eq!(report.chart.chips, repeated.chart.chips);
    }
    let mp3 = parse(std::fs::File::open(fixture("mp3_audio.dtx")).expect("mp3 chart"))
        .expect("mp3 chart parses");
    assert_eq!(mp3.assets.wav.get(1), Some("compat-tone.mp3"));
    assert!(mp3.chips.iter().any(|chip| chip.channel == EChannel::SE32));
    let missing = parse_str("#WAV01: absent.wav\n#00013: 01\n").expect("missing asset parses");
    assert_eq!(missing.drum_chips().count(), 1);
}
