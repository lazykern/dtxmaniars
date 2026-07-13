//! End-to-end BGA integration tests — load real DTX fixture, verify BGA events.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use dtx_bga::chart::{timed_visual_events, visual_state_at, visual_state_at_with_motion};
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
    assert!(
        overlays.contains(&(BgaLayer::Layer1, 2)),
        "layer1 replaced with asset 2"
    );
    assert!(
        overlays.contains(&(BgaLayer::Layer3, 1)),
        "layer3 unchanged"
    );
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
        assert!(std::time::Instant::now() < deadline, "decoder timed out");
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

#[test]
fn visual_state_is_deterministic_at_pan_start_mid_end_and_after_seek() {
    let chart = parse(
        File::open("../dtx-core/tests/fixtures/compatibility/visual_pan_swap.dtx")
            .expect("fixture exists"),
    )
    .expect("parses");
    let events = timed_visual_events(&chart);

    let start = visual_state_at(&events, 2_000);
    let midpoint = visual_state_at(&events, 2_250);
    let end = visual_state_at(&events, 2_500);
    let after_backward_seek = visual_state_at(&events, 2_250);

    let start_geometry = start.layers[&BgaLayer::Layer1]
        .geometry
        .expect("pan geometry");
    assert_eq!(start_geometry.source.x, 0.0);
    assert_eq!(start_geometry.source.width, 100.0);
    let midpoint_geometry = midpoint.layers[&BgaLayer::Layer1]
        .geometry
        .expect("pan geometry");
    assert_eq!(midpoint_geometry.source.x, 5.0);
    assert_eq!(midpoint_geometry.source.width, 75.0);
    let end_geometry = end.layers[&BgaLayer::Layer1]
        .geometry
        .expect("pan geometry");
    assert_eq!(end_geometry.source.x, 10.0);
    assert_eq!(end_geometry.destination.x, 30.0);
    assert_eq!(midpoint, after_backward_seek);
}

#[test]
fn reduced_background_motion_resolves_pan_to_static_end_and_skips_movie() {
    let chart = parse(
        File::open("../dtx-core/tests/fixtures/compatibility/visual_pan_swap.dtx")
            .expect("fixture exists"),
    )
    .expect("parses");
    let events = timed_visual_events(&chart);
    let state = visual_state_at_with_motion(&events, 2_000, false);

    let geometry = state.layers[&BgaLayer::Layer1]
        .geometry
        .expect("pan geometry");
    assert_eq!(geometry.source.x, 10.0);
    assert_eq!(geometry.destination.x, 30.0);
    assert!(state.movie.is_none());
}

#[test]
fn swap_events_replace_each_target_scope() {
    let chart = parse(
        File::open("../dtx-core/tests/fixtures/compatibility/visual_pan_swap.dtx")
            .expect("fixture exists"),
    )
    .expect("parses");
    let state = visual_state_at(&timed_visual_events(&chart), 4_000);

    for layer in [
        BgaLayer::Layer1,
        BgaLayer::Layer2,
        BgaLayer::Layer3,
        BgaLayer::LayerN(4),
        BgaLayer::LayerN(5),
        BgaLayer::LayerN(6),
        BgaLayer::LayerN(7),
        BgaLayer::LayerN(8),
    ] {
        assert_eq!(state.layers[&layer].asset_id, 2);
        assert!(state.layers[&layer].geometry.is_none());
    }
}

#[test]
fn live_image_node_uses_mid_pan_crop_and_destination() {
    let dir = bga_fixture_dir();
    let chart = parse(
        b"#BPM: 120\n#BMP02: red.png\n#BGAPAN01: 02,100,100,50,50,0,0,10,10,20,20,30,30,96\n#00004: 01\n"
            .as_slice(),
    )
    .expect("parses");
    let res = ActiveChartRes::from_chart(&chart, Some(&dir.join("pan.dtx")));
    let mut app = headless_app();
    app.insert_resource(res);
    app.insert_resource(BgaClock { current_ms: 2_250 });
    app.update();

    let (_, node, image) = app
        .world_mut()
        .query::<(&BgaLayerOverlay, &Node, &ImageNode)>()
        .iter(app.world())
        .find(|(overlay, _, _)| overlay.layer == BgaLayer::Layer1)
        .expect("panned layer spawned");
    assert_eq!(node.left, Val::Px(25.0));
    assert_eq!(node.top, Val::Px(25.0));
    assert_eq!(node.width, Val::Px(75.0));
    let crop = image.rect.expect("source crop");
    assert_eq!(crop.min.x, 5.0);
    assert_eq!(crop.max.x, 80.0);
}
