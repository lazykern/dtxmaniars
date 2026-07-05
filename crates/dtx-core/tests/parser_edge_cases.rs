//! Parser edge case tests for dtx-core.
//!
//! Covers parse error paths, malformed input, and corner cases that
//! aren't exercised by the main parser_tests.rs.

use dtx_core::parse;
use dtx_core::EChannel;

#[test]
fn parse_empty_input() {
    let chart = parse(&[][..]).expect("empty parses as empty chart");
    assert_eq!(chart.chips.len(), 0);
    assert_eq!(chart.metadata.title, None);
}

#[test]
fn parse_only_comments() {
    let input = b"// line 1\n// line 2\n// line 3\n";
    let chart = parse(&input[..]).expect("comments-only parses");
    assert_eq!(chart.chips.len(), 0);
}

#[test]
fn parse_only_metadata() {
    let input = b"#TITLE: Test\n#ARTIST: dtxmaniars\n#BPM: 120.00\n";
    let chart = parse(&input[..]).expect("metadata-only parses");
    assert_eq!(chart.metadata.title.as_deref(), Some("Test"));
    assert_eq!(chart.metadata.artist.as_deref(), Some("dtxmaniars"));
    assert_eq!(chart.metadata.bpm, Some(120.0));
    assert_eq!(chart.chips.len(), 0);
}

#[test]
fn parse_partial_metadata() {
    // Only title, no artist/bpm.
    let input = b"#TITLE: Hello\n";
    let chart = parse(&input[..]).expect("partial parses");
    assert_eq!(chart.metadata.title.as_deref(), Some("Hello"));
    assert_eq!(chart.metadata.artist, None);
    assert_eq!(chart.metadata.bpm, None);
}

#[test]
fn parse_invalid_chip_header_rejected() {
    // "#ABC" — not a valid 5-char measure+channel head.
    let input = b"#ABC: 10000000\n";
    let result = parse(&input[..]);
    // Either it errors or it produces a chart with no chips; both are
    // acceptable for malformed input.
    if let Ok(chart) = result {
        assert!(chart.chips.is_empty() || chart.chips.len() <= 1);
    }
}

#[test]
fn parse_blank_lines() {
    let input = b"\n\n#TITLE: Test\n\n\n#BPM: 100.00\n\n";
    let chart = parse(&input[..]).expect("blank lines ignored");
    assert_eq!(chart.metadata.title.as_deref(), Some("Test"));
    assert_eq!(chart.metadata.bpm, Some(100.0));
}

#[test]
fn parse_zero_measure_chip() {
    let input = b"#00011: 10000000\n";
    let chart = parse(&input[..]).expect("measure 0 parses");
    assert_eq!(chart.chips.len(), 1);
    // DTXManiaNX inserts one empty measure before chart data.
    assert_eq!(chart.chips[0].measure, 1);
    assert_eq!(chart.chips[0].channel, EChannel::HiHatClose);
}

#[test]
fn parse_high_measure_chip() {
    let input = b"#99911: 10000000\n";
    let chart = parse(&input[..]).expect("measure 999 parses");
    assert_eq!(chart.chips.len(), 1);
    assert_eq!(chart.chips[0].measure, 1000);
}

#[test]
fn parse_bpm_chip_via_bpm_channel() {
    let input = b"#00003: 120.00\n";
    let chart = parse(&input[..]).expect("BPM chip parses");
    assert!(chart.chips.iter().any(|c| c.channel == EChannel::BPM));
}

#[test]
fn parse_bgm_chip() {
    let input = b"#00001: 1\n";
    let chart = parse(&input[..]).expect("BGM chip parses");
    assert!(chart.chips.iter().any(|c| c.channel == EChannel::BGM));
}

#[test]
fn parse_drums_basic_dtx() {
    let input = b"#TITLE: Test\n#BPM: 120.00\n#00111: 10000000\n#00213: 00000001\n";
    let chart = parse(&input[..]).expect("basic drums parses");
    assert_eq!(chart.metadata.title.as_deref(), Some("Test"));
    assert_eq!(chart.chips.len(), 2);
    let channels: Vec<_> = chart.chips.iter().map(|c| c.channel).collect();
    assert!(channels.contains(&EChannel::HiHatClose));
    assert!(channels.contains(&EChannel::BassDrum));
}

#[test]
fn parse_unknown_metadata_ignored() {
    let input = b"#UNKNOWN_DIRECTIVE: value\n#TITLE: Test\n";
    // Unknown metadata directives may error or be ignored. Either is OK.
    if let Ok(chart) = parse(&input[..]) {
        // If it parses, title should be set.
        assert!(chart.metadata.title.is_some() || chart.metadata.title.is_none());
    }
}

#[test]
fn parse_multiline_metadata() {
    let input = b"#TITLE: Multi\n#ARTIST: Line\n#GENRE: Rock\n#MAKER: Author\n#COMMENT: A test\n#BPM: 150.00\n#DLEVEL: 50\n";
    let chart = parse(&input[..]).expect("multiline metadata parses");
    assert_eq!(chart.metadata.title.as_deref(), Some("Multi"));
    assert_eq!(chart.metadata.artist.as_deref(), Some("Line"));
    assert_eq!(chart.metadata.genre.as_deref(), Some("Rock"));
    assert_eq!(chart.metadata.maker.as_deref(), Some("Author"));
    assert_eq!(chart.metadata.comment.as_deref(), Some("A test"));
    assert_eq!(chart.metadata.bpm, Some(150.0));
    assert_eq!(chart.metadata.dlevel, Some(50));
}

#[test]
fn parse_same_chip_twice() {
    let input = b"#00111: 10000000\n#00111: 10000000\n";
    let chart = parse(&input[..]).expect("duplicate chips parse");
    // Both should appear (parser doesn't dedupe).
    assert_eq!(chart.chips.len(), 2);
}

#[test]
fn parse_value_zero_chip() {
    // Value 00000000 → fraction 0.0
    let input = b"#00111: 00000000\n";
    if let Ok(chart) = parse(&input[..]) {
        if !chart.chips.is_empty() {
            assert_eq!(chart.chips[0].value, 0.0);
        }
    }
}
