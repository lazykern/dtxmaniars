//! Integration tests for BGA chip detection from DTX files.

use dtx_core::bga::{bga_events, BgaLayer};
use dtx_core::parser::parse;
use std::fs::File;

#[test]
fn parse_bga_fixture_extracts_image_and_movie_chips() {
    let f = File::open("tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");

    let events = bga_events(&chart);
    assert_eq!(events.len(), 5, "fixture has 5 BGA chips");

    // Layer1 image at measure 0
    assert!(events
        .iter()
        .any(|e| e.measure == 0 && e.layer == BgaLayer::Layer1 && e.bmp_index == 1));

    // Movie at measure 0
    assert!(events
        .iter()
        .any(|e| e.measure == 0 && e.layer == BgaLayer::Movie && e.bmp_index == 1));

    // BGALayer3 (fullscreen) at measure 0
    assert!(events
        .iter()
        .any(|e| e.measure == 0 && e.layer == BgaLayer::Layer3 && e.bmp_index == 1));

    // Movie at measure 20
    assert!(events
        .iter()
        .any(|e| e.measure == 20 && e.layer == BgaLayer::Movie));

    // BGALayer3 (fullscreen) at measure 20 with bmp_index 2
    assert!(events
        .iter()
        .any(|e| e.measure == 20 && e.layer == BgaLayer::Layer3 && e.bmp_index == 2));

    // Drum chips should NOT appear in BGA events.
    assert!(!events
        .iter()
        .any(|e| matches!(e.layer, BgaLayer::Movie) && e.bmp_index == 0 && e.measure < 4));
}

#[test]
fn bga_events_timing_with_fixture_bpm() {
    let f = File::open("tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");

    let events = bga_events(&chart);
    let bpm = 120.0;
    // Measure 0 → 0ms, measure 20 → 40000ms (at 120 BPM = 2000ms/measure)
    let at_zero = events.iter().filter(|e| e.measure == 0).count();
    let at_20 = events.iter().filter(|e| e.measure == 20).count();
    assert_eq!(at_zero, 3);
    assert_eq!(at_20, 2);

    // Spot-check timing
    let event_at_20 = events.iter().find(|e| e.measure == 20).unwrap();
    assert_eq!(event_at_20.approx_ms(bpm), 40000);
}
