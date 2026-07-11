//! End-to-end BGA integration tests — load real DTX fixture, verify BGA events.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use dtx_bga::{ActiveChartRes, BgaClock, BgaLayerOverlay, MovieWorker};
use dtx_core::bga::{bga_events, BgaLayer};
use dtx_core::parser::parse;
use std::fs::File;
use std::path::PathBuf;

fn bga_fixture_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p
}

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Image>();
    app.add_plugins(dtx_bga::plugin);
    app
}

#[test]
fn static_image_event_replaces_only_target_layer() {
    let dir = bga_fixture_dir();
    let chart = parse(File::open(dir.join("visual.dtx")).expect("visual fixture")).expect("parse");
    let res = ActiveChartRes::from_chart(&chart, Some(&dir.join("visual.dtx")));

    let mut app = headless_app();
    app.insert_resource(res);
    app.insert_resource(BgaClock { current_ms: 2000 });
    app.update();

    let overlays: Vec<(BgaLayer, u32)> = app
        .world_mut()
        .query::<(&BgaLayerOverlay, &ImageNode)>()
        .iter(app.world())
        .map(|(o, _)| (o.layer, o.asset_id))
        .collect();
    assert_eq!(overlays.len(), 2, "layer1 + layer3 at 2000ms");
    assert!(overlays.contains(&(BgaLayer::Layer1, 1)));
    assert!(overlays.contains(&(BgaLayer::Layer3, 1)));

    // Advance past the second Layer1 event: only Layer1 is replaced.
    app.insert_resource(BgaClock { current_ms: 4000 });
    app.update();

    let overlays: Vec<(BgaLayer, u32)> = app
        .world_mut()
        .query::<(&BgaLayerOverlay, &ImageNode)>()
        .iter(app.world())
        .map(|(o, _)| (o.layer, o.asset_id))
        .collect();
    assert_eq!(overlays.len(), 2, "still one entity per layer");
    assert!(overlays.contains(&(BgaLayer::Layer1, 2)), "layer1 replaced with asset 2");
    assert!(overlays.contains(&(BgaLayer::Layer3, 1)), "layer3 unchanged");
}

#[test]
fn movie_worker_decodes_tiny_avi() {
    let path = bga_fixture_dir().join("tiny.avi");
    let mut worker = MovieWorker::spawn(path);
    worker.set_target_ms(900);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let frame = loop {
        if let Some(frame) = worker.newest_due_frame(900) {
            break frame;
        }
        if let Some(err) = worker.take_error() {
            panic!("decoder error: {err}");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "decoder timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!((frame.width, frame.height), (16, 16));
    assert_eq!(frame.rgba.len(), 16 * 16 * 4);
    worker.stop();
}

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
