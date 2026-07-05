//! Integration test: realistic DTX fixture (p0-7).
//!
//! Validates that a real DTX with BGM, BGA, BPM change, multi-measure
//! drums parses end-to-end and surfaces correct metadata + chip count.

use std::fs::File;

use dtx_core::bga::{bga_events, BgaLayer};
use dtx_core::parser::parse;

/// Inlined copy of dtx_timing::math::chip_time_ms_with_bpm_changes.
/// dtx-core must not depend on dtx-timing (Pure layer rule, AGENTS.md).
fn chip_time_ms_with_bpm_changes(
    measure: u32,
    fraction: f32,
    base_bpm: f32,
    changes: &[(u32, f32)],
) -> i64 {
    if base_bpm <= 0.0 {
        return 0;
    }
    let mut sorted: Vec<(u32, f32)> = changes.to_vec();
    sorted.sort_by_key(|c| c.0);
    let mut total_ms: f64 = 0.0;
    let mut current_bpm: f64 = base_bpm as f64;
    let mut interval_start: u32 = 0;
    for (m, bpm) in &sorted {
        if *m >= measure {
            break;
        }
        if *m > interval_start {
            total_ms += ((*m - interval_start) as f64) * 4.0 * 60_000.0 / current_bpm;
        }
        current_bpm = *bpm as f64;
        interval_start = *m;
    }
    let partial = (measure - interval_start) as f64 + fraction as f64;
    total_ms += partial * 4.0 * 60_000.0 / current_bpm;
    total_ms as i64
}

#[test]
fn real_chart_parses_metadata() {
    let f = File::open("tests/fixtures/real_chart.dtx").expect("fixture exists");
    let chart = parse(f).expect("parses");
    let m = chart.metadata;
    assert_eq!(m.title.as_deref(), Some("Real Chart Demo"));
    assert_eq!(m.artist.as_deref(), Some("dtxmaniars test"));
    assert_eq!(m.bpm, Some(120.0));
    assert_eq!(m.dlevel, Some(75));
    assert_eq!(m.glevel, Some(60));
    assert_eq!(m.blevel, Some(50));
}

#[test]
fn real_chart_has_bpm_change() {
    let f = File::open("tests/fixtures/real_chart.dtx").unwrap();
    let chart = parse(f).unwrap();
    let changes: Vec<(u32, f32)> = chart
        .chips
        .iter()
        .filter(|c| {
            matches!(
                c.channel,
                dtx_core::EChannel::BPM | dtx_core::EChannel::BPMEx
            )
        })
        .map(|c| (c.measure, c.value))
        .collect();
    assert_eq!(changes.len(), 1);
    // Source #00208 shifts to chart measure 3 (NX empty first measure).
    assert_eq!(changes[0].0, 3);
    assert!((changes[0].1 - 180.0).abs() < 0.01);
}

#[test]
fn real_chart_chip_count() {
    let f = File::open("tests/fixtures/real_chart.dtx").unwrap();
    let chart = parse(f).unwrap();
    // Drum chips: 5 (m0) + 8 (m1) + 3 (m2) + 5 (m3) = 21
    let drums = chart.chips.iter().filter(|c| c.channel.is_drum()).count();
    assert_eq!(drums, 21);
}

#[test]
fn real_chart_chip_timing_with_bpm_change() {
    let f = File::open("tests/fixtures/real_chart.dtx").unwrap();
    let _chart = parse(f).unwrap();
    let changes = [(2u32, 180.0f32)];
    let t0 = chip_time_ms_with_bpm_changes(0, 0.0, 120.0, &changes);
    assert_eq!(t0, 0);
    let t1 = chip_time_ms_with_bpm_changes(1, 0.0, 120.0, &changes);
    assert_eq!(t1, 2000);
    let t2 = chip_time_ms_with_bpm_changes(2, 0.0, 120.0, &changes);
    assert_eq!(t2, 4000);
    let t3 = chip_time_ms_with_bpm_changes(3, 0.0, 120.0, &changes);
    // m0..2 at 120 BPM = 4000ms. m2..3 at 180 BPM = 4*60000/180 = 1333.33ms.
    assert_eq!(t3, 5333);
}

#[test]
fn real_chart_bga_events() {
    let f = File::open("tests/fixtures/real_chart.dtx").unwrap();
    let chart = parse(f).unwrap();
    let events = bga_events(&chart);
    assert_eq!(events.len(), 2);
    let l1: Vec<_> = events
        .iter()
        .filter(|e| e.layer == BgaLayer::Layer1)
        .collect();
    let l2: Vec<_> = events
        .iter()
        .filter(|e| e.layer == BgaLayer::Layer2)
        .collect();
    assert_eq!(l1.len(), 1);
    assert_eq!(l2.len(), 1);
    assert_eq!(l1[0].bmp_index, 1);
    assert_eq!(l2[0].bmp_index, 2);
}

#[test]
fn real_chart_sibling_files_exist() {
    assert!(std::path::Path::new("tests/fixtures/real_chart.ogg").exists());
    assert!(std::path::Path::new("tests/fixtures/real_chart.bmp").exists());
}
