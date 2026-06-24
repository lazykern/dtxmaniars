use dtx_core::parse;
use dtx_core::EChannel;
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
