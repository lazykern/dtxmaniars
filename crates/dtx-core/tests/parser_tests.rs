use dtx_core::{parse, parse_with_options, EChannel, ParseOptions};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

#[test]
fn parse_minimal_fixture() {
    let path = fixture("minimal.dtx");
    let chart = parse(BufReader::new(File::open(&path).unwrap())).unwrap();
    assert_eq!(chart.metadata.title.as_deref(), Some("Minimal Test"));
    assert_eq!(chart.metadata.artist.as_deref(), Some("dtxmaniars"));
    assert_eq!(chart.metadata.bpm, Some(120.0));
    assert_eq!(chart.metadata.dlevel, Some(30));
}

#[test]
fn parse_drums_basic_fixture() {
    let path = fixture("drums_basic.dtx");
    let chart = parse(BufReader::new(File::open(&path).unwrap())).unwrap();
    assert_eq!(chart.metadata.title.as_deref(), Some("Drums Basic"));
    assert_eq!(chart.metadata.bpm, Some(150.0));

    let drums: Vec<_> = chart.drum_chips().collect();
    assert!(!drums.is_empty(), "expected at least one drum chip");

    // M2 contract: every chip's channel is a known drum channel.
    for c in &drums {
        assert!(
            c.channel.is_drum(),
            "non-drum channel leaked: {:?}",
            c.channel
        );
    }

    // Sanity: BD appears at measure 1.
    let bd = drums.iter().find(|c| c.channel == EChannel::BassDrum);
    assert!(bd.is_some());
}

#[test]
fn visual_sequences_preserve_asset_id_and_fraction() {
    let src = b"#TITLE: Visual\n#BMP01: first.png\n#BMP02: second.png\n#AVI03: movie.avi\n#00004: 0102\n#00154: 0003\n";
    let chart = parse(&src[..]).expect("visual chart parses");

    let images: Vec<_> = chart
        .chips
        .iter()
        .filter(|chip| chip.channel == EChannel::BGALayer1)
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!((images[0].wav_slot, images[0].value), (1, 0.0));
    assert_eq!((images[1].wav_slot, images[1].value), (2, 0.5));

    let movie = chart
        .chips
        .iter()
        .find(|chip| chip.channel == EChannel::Movie)
        .expect("movie chip");
    assert_eq!(movie.wav_slot, 3);
    assert_eq!(movie.value, 0.5);

    let events = dtx_core::bga::bga_events(&chart);
    assert_eq!(events[0].bmp_index, 1);
    assert_eq!(events[1].bmp_index, 2);
    assert_eq!(events[1].fraction, 0.5);
}

#[test]
fn conditional_fixture_selects_reproducible_branches() {
    let parse_seed = |seed| {
        parse_with_options(
            BufReader::new(File::open(fixture("conditional_branches.dtx")).unwrap()),
            ParseOptions { random_seed: seed },
        )
        .expect("conditional fixture parses")
        .chart
    };

    let one = parse_seed(0);
    assert!(one
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::HiHatClose));
    assert!(!one
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::BassDrum));
    assert_eq!(one.assets.wav.get(1), Some("branch-one.wav"));
    assert_eq!(one.assets.wav.get(2), None);

    let two = parse_seed(1);
    assert!(two
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::BassDrum));
    assert!(!two
        .chips
        .iter()
        .any(|chip| chip.channel == EChannel::HiHatClose));
    assert_eq!(two.assets.wav.get(1), None);
    assert_eq!(two.assets.wav.get(2), Some("branch-two.wav"));
}

#[test]
fn conditional_nested_fixture_keeps_one_inner_branch() {
    let report = parse_with_options(
        BufReader::new(File::open(fixture("conditional_nested.dtx")).unwrap()),
        ParseOptions { random_seed: 1 },
    )
    .expect("nested conditional fixture parses");

    assert!(report.warnings.is_empty());
    assert_eq!(report.chart.drum_chips().count(), 1);
}
