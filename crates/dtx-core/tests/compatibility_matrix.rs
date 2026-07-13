use std::path::PathBuf;

use dtx_core::{
    parse, parse_source, parse_str, parse_with_options, ChartFormat, ChartLevel, DiagnosticKind,
    EChannel, ParseOptions,
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

#[derive(Debug, Clone, Copy)]
struct ParserCompatibilityCase {
    fixture: &'static str,
    format: ChartFormat,
    expected_drum_notes: usize,
    required_channels: &'static [EChannel],
    expected_diagnostic: Option<DiagnosticKind>,
}

const SYSTEM_CHANNELS: &[EChannel] = &[
    EChannel::HiHatCloseHidden,
    EChannel::SnareHidden,
    EChannel::BassDrumHidden,
    EChannel::HighTomHidden,
    EChannel::LowTomHidden,
    EChannel::CymbalHidden,
    EChannel::FloorTomHidden,
    EChannel::HiHatOpenHidden,
    EChannel::RideCymbalHidden,
    EChannel::LeftCymbalHidden,
    EChannel::LeftPedalHidden,
    EChannel::LeftBassDrumHidden,
    EChannel::MIDIChorus,
    EChannel::FillIn,
    EChannel::Click,
    EChannel::FirstSoundChip,
    EChannel::MixerAdd,
    EChannel::MixerRemove,
];

const VISUAL_SWAP_CHANNELS: &[EChannel] = &[
    EChannel::BGALayer1Swap,
    EChannel::BGALayer2Swap,
    EChannel::BGALayer3Swap,
    EChannel::BGALayer4Swap,
    EChannel::BGALayer5Swap,
    EChannel::BGALayer6Swap,
    EChannel::BGALayer7Swap,
    EChannel::BGALayer8Swap,
];

const PARSER_COMPATIBILITY_CASES: &[ParserCompatibilityCase] = &[
    ParserCompatibilityCase {
        fixture: "compatibility/equivalent.dtx",
        format: ChartFormat::Dtx,
        expected_drum_notes: 3,
        required_channels: &[],
        expected_diagnostic: None,
    },
    ParserCompatibilityCase {
        fixture: "compatibility/equivalent.gda",
        format: ChartFormat::Gda,
        expected_drum_notes: 3,
        required_channels: &[],
        expected_diagnostic: None,
    },
    ParserCompatibilityCase {
        fixture: "compatibility/equivalent.g2d",
        format: ChartFormat::G2d,
        expected_drum_notes: 3,
        required_channels: &[],
        expected_diagnostic: None,
    },
    ParserCompatibilityCase {
        fixture: "compatibility/system_channels.dtx",
        format: ChartFormat::Dtx,
        expected_drum_notes: 1,
        required_channels: SYSTEM_CHANNELS,
        expected_diagnostic: None,
    },
    ParserCompatibilityCase {
        fixture: "compatibility/visual_pan_swap.dtx",
        format: ChartFormat::Dtx,
        expected_drum_notes: 0,
        required_channels: VISUAL_SWAP_CHANNELS,
        expected_diagnostic: Some(DiagnosticKind::MalformedVisual),
    },
    ParserCompatibilityCase {
        fixture: "compatibility/malformed.gda",
        format: ChartFormat::Gda,
        expected_drum_notes: 0,
        required_channels: &[],
        expected_diagnostic: Some(DiagnosticKind::UnsupportedChannel),
    },
];

#[test]
fn declared_parser_compatibility_cases_match_executable_outcomes() {
    for case in PARSER_COMPATIBILITY_CASES {
        let fixture_path = PathBuf::from(case.fixture);
        let extension = fixture_path
            .extension()
            .and_then(|extension| extension.to_str())
            .expect("compatibility fixture has an extension");
        assert_eq!(
            ChartFormat::from_extension(extension),
            Some(case.format),
            "discovery front-end drifted for {}",
            case.fixture
        );
        let report = legacy_fixture(case.fixture, case.format);
        assert_eq!(
            report.chart.drum_chips().count(),
            case.expected_drum_notes,
            "drum contract drifted for {}",
            case.fixture
        );
        for channel in case.required_channels {
            assert!(
                report
                    .chart
                    .chips
                    .iter()
                    .any(|chip| chip.channel == *channel),
                "{} lost channel {channel:?}",
                case.fixture
            );
        }
        match case.expected_diagnostic {
            Some(kind) => assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.kind == kind),
                "{} lost its {kind:?} diagnostic",
                case.fixture
            ),
            None => assert!(
                report.diagnostics.is_empty(),
                "{} unexpectedly degraded: {:?}",
                case.fixture,
                report.diagnostics
            ),
        }
    }
}

#[test]
fn declared_format_boundary_rejects_bms_and_bme() {
    for extension in ["bms", "BMS", "bme", "BME"] {
        assert_eq!(ChartFormat::from_extension(extension), None);
    }
}

#[test]
fn metadata_aliases_and_mixed_timing_media_remain_in_the_matrix() {
    let levels =
        parse_str("#PLAYLEVEL: 355\n#DLVDEC: 7\n#00012: 01\n").expect("level aliases parse");
    assert_eq!(
        levels.metadata.drum_level,
        Some(ChartLevel {
            tenths: 35,
            hundredths: 7,
        })
    );

    let mixed = parse_str(
        "#BPM: 120\n#BPM01: 180\n#WAV01: bgm.mp3\n#WAV02: hit.ogg\n#00001: 01\n#00002: 0.5\n#00008: 01\n#00112: 02\n",
    )
    .expect("mixed timing/media chart parses");
    assert_eq!(mixed.assets.wav.get(1), Some("bgm.mp3"));
    assert_eq!(mixed.assets.wav.get(2), Some("hit.ogg"));
    assert!(mixed
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::BPMEx));
    assert!(mixed
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::BarLength));
    assert_eq!(mixed.drum_chips().count(), 1);
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

#[test]
fn hidden_and_system_channels_never_become_drum_notes() {
    let report = legacy_fixture("compatibility/system_channels.dtx", ChartFormat::Dtx);
    assert_eq!(report.chart.drum_chips().count(), 1);
    for channel in SYSTEM_CHANNELS {
        assert!(report
            .chart
            .chips
            .iter()
            .any(|chip| chip.channel == *channel));
    }
}
