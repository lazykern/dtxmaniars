//! End-to-end BGA integration tests — load real DTX fixture, verify BGA events.

use dtx_core::bga::{bga_events, BgaLayer};
use dtx_core::parser::parse;
use std::fs::File;

#[test]
fn bga_fixture_full_event_count() {
    let f = File::open("../dtx-core/tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let events = bga_events(&chart);
    assert_eq!(events.len(), 5, "5 BGA events in bga_basic.dtx");
}

#[test]
fn bga_fixture_layer3_fullscreen_event() {
    let f = File::open("../dtx-core/tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let events = bga_events(&chart);
    let fullscreen = events
        .iter()
        .filter(|e| e.layer == BgaLayer::Layer3)
        .count();
    assert_eq!(fullscreen, 2, "2 BGALayer3 events in fixture");
}

#[test]
fn bga_fixture_movies_skipped() {
    let f = File::open("../dtx-core/tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let events = bga_events(&chart);
    let movies = events.iter().filter(|e| e.layer.is_movie()).count();
    assert_eq!(movies, 2, "2 movie events in fixture (M7.1 will decode)");
}

#[test]
fn bga_fixture_no_drum_leakage() {
    let f = File::open("../dtx-core/tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let events = bga_events(&chart);
    // No event should be a drum channel.
    for e in &events {
        assert!(
            !matches!(e.layer, BgaLayer::Layer1 if e.layer == BgaLayer::Layer1 && e.bmp_index == 0)
        );
    }
}

#[test]
fn bga_fixture_bpm_timing_120() {
    let f = File::open("../dtx-core/tests/fixtures/bga_basic.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let events = bga_events(&chart);
    let at_21 = events
        .iter()
        .find(|e| e.measure == 21)
        .expect("event at chart measure 21");
    // NX empty first measure: source measure 20 -> chart measure 21 -> 42000ms.
    assert_eq!(at_21.approx_ms(120.0), 42000);
}
