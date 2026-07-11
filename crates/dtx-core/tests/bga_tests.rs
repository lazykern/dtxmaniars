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

    // DTXManiaNX inserts one empty measure before chart data.
    // Layer1 image at source measure 0 → chart measure 1
    assert!(events
        .iter()
        .any(|e| e.measure == 1 && e.layer == BgaLayer::Layer1 && e.bmp_index == 1));

    // Movie at source measure 0 → chart measure 1, referencing #AVI03
    assert!(events
        .iter()
        .any(|e| e.measure == 1 && e.layer == BgaLayer::Movie && e.bmp_index == 3));

    // BGALayer3 (fullscreen) at source measure 0 → chart measure 1
    assert!(events
        .iter()
        .any(|e| e.measure == 1 && e.layer == BgaLayer::Layer3 && e.bmp_index == 1));

    // Movie at source measure 20 → chart measure 21
    assert!(events
        .iter()
        .any(|e| e.measure == 21 && e.layer == BgaLayer::Movie));

    // BGALayer3 (fullscreen) at source measure 20 with bmp_index 2 → chart measure 21
    assert!(events
        .iter()
        .any(|e| e.measure == 21 && e.layer == BgaLayer::Layer3 && e.bmp_index == 2));

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
    // Source measure 0 → chart measure 1 → 2000ms,
    // source measure 20 → chart measure 21 → 42000ms at 120 BPM.
    let at_one = events.iter().filter(|e| e.measure == 1).count();
    let at_21 = events.iter().filter(|e| e.measure == 21).count();
    assert_eq!(at_one, 3);
    assert_eq!(at_21, 2);

    // Spot-check timing
    let event_at_21 = events.iter().find(|e| e.measure == 21).unwrap();
    assert_eq!(event_at_21.approx_ms(bpm), 42000);
}
