use std::path::PathBuf;

use dtx_core::{
    parse, parse_source, parse_str, parse_with_options, ChartFormat, DiagnosticKind, EChannel,
    ParseOptions,
};

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

fn legacy_fixture(name: &str, format: ChartFormat) -> dtx_core::ParseReport {
    parse_source(
        std::fs::File::open(fixture(name)).expect("compatibility fixture exists"),
        format,
        ParseOptions { random_seed: 0 },
    )
    .expect("legacy chart parses")
}

fn gameplay_signature(chart: &dtx_core::Chart) -> Vec<(u32, u8, u32, u32)> {
    chart
        .chips
        .iter()
        .filter(|chip| {
            chip.channel.is_drum() || matches!(chip.channel, EChannel::BPM | EChannel::BarLength)
        })
        .map(|chip| {
            (
                chip.measure,
                chip.channel as u8,
                chip.value.to_bits(),
                chip.wav_slot,
            )
        })
        .collect()
}

#[test]
fn dtx_gda_and_g2d_normalize_to_equal_drum_gameplay() {
    let dtx = legacy_fixture("compatibility/equivalent.dtx", ChartFormat::Dtx);
    let gda = legacy_fixture("compatibility/equivalent.gda", ChartFormat::Gda);
    let g2d = legacy_fixture("compatibility/equivalent.g2d", ChartFormat::G2d);
    assert_eq!(
        gameplay_signature(&dtx.chart),
        gameplay_signature(&gda.chart)
    );
    assert_eq!(
        gameplay_signature(&dtx.chart),
        gameplay_signature(&g2d.chart)
    );
    assert_eq!(
        dtx.chart.drum_chips().count(),
        gda.chart.drum_chips().count()
    );
}

#[test]
fn malformed_gda_channel_has_line_diagnostic() {
    let report = legacy_fixture("compatibility/malformed.gda", ChartFormat::Gda);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.line == Some(4) && diagnostic.kind == DiagnosticKind::UnsupportedChannel
    }));
}
