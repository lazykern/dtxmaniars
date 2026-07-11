# Real BGA Image Playback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the colored BGA placeholder rectangles with the chart's actual image assets (`#BMPxx` files), preserving BocuD event and layer semantics, with an honest on-screen fallback for movie channels. No video decode (explicit roadmap non-goal).

**Architecture:** Three gaps to bridge, in order: (1) the parser stores BGA chip lines as one whole-line decimal float, so image ids >9 and intra-measure positions are lost — reuse the note-channel measure-slot base36 parsing for BGA channels; (2) `ActiveChartRes` carries only bpm+events — thread the already-parsed `#BMPxx` registry and the chart's source directory through `song_loading.rs`; (3) `spawn_overlay` draws colored rects and never despawns them — swap to `ImageNode` with per-layer replacement and correct z-order (Layer3 fullscreen behind Layer1/2).

**Tech Stack:** Bevy 0.19 UI (`ImageNode`, `AssetServer`), existing `dtx-core` registries. Possible `bmp` feature flag on bevy for `.bmp` decode.

**Source basis (verified 2026-07-11):**
- `crates/dtx-bga/src/lib.rs` (337 lines): `BgaPlayer` (:34-46), `BgaLayerOverlay { layer, bmp_index }` (:60-66), plugin (:80), `tick_bga_player` (:90-127, walks `ActiveChartRes.events` against `dtx_timing::AudioClock`), `activate_event` (:130-148, movies → `movies_skipped += 1`), `spawn_overlay` (:150-169, colored `Node` + `BackgroundColor`, `ZIndex(-1)`), `overlay_geometry` (:173-191, placeholder colors, 1280×720 reference px). Overlays accumulate — nothing despawns prior overlays per layer; cleanup only `OnExit(Performance)` (`song_loading.rs:133-142`).
- `ActiveChartRes { bpm, events }` (:195-201) — no assets, no source dir.
- Consumer wiring: dtx-bga plugin registered in `game-menu` (`crates/game-menu/src/lib.rs:25`); `poll_chart_parse` (`crates/game-menu/src/song_loading.rs:183-276`) inserts `ActiveChartRes` at :227-233; `drums_chart.source_path` and `chart.assets.bmp` are in scope there but not forwarded.
- Registries already parsed: `BmpRegistry` (`crates/dtx-core/src/assets.rs:74-110`, `by_id: HashMap<u32,String>`); `chart.assets.process_line` wired at `parser.rs:95-97`; test `parser.rs:531` proves `#BMP01: bg.bmp` → `bmp.get(1) == Some("bg.bmp")`.
- **Parser defect:** BGA/movie chip lines parsed at `crates/dtx-core/src/parser.rs:308-326` as `value.parse::<f32>()` (whole line = one decimal float). Note channels use measure-slot base36 at `parser.rs:339-353`. Consequences: `BgaEvent.fraction` hardcoded `0.0` (`bga.rs:128`), `bmp_index = c.value as u32` misresolves multi-slot lines and base36 ids > 9.
- `BgaEvent { measure, layer, bmp_index, fraction }` (`bga.rs:96-105`), `approx_ms(bpm)` (:110-114), `bga_events(chart)` (:118-134). `BgaLayer::{Layer1,Layer2,Layer3,LayerN,Movie,MovieFull}`, channels 4/7/0x55/0x56-0x60/0x54/0x5A (`bga.rs:27-58`, `channel.rs`).
- Image-loading conventions: `asset_server.load(path.to_string_lossy().to_string())` (album art, `song_select.rs:950,1653-1667`); `UnapprovedPathMode::Allow` set in `main.rs:55-58` so absolute chart paths load; load-state polling precedent `song_loading.rs:295-304`.
- Fixtures: `crates/dtx-core/tests/fixtures/bga_basic.dtx` has BGA chips but NO `#BMP` defs and no image files. `dtx-bga/tests/integration_bga.rs` (5 tests) loads it.
- Not confirmed: bevy `bmp` image feature enabled (registry values include `.bmp`).

**Non-goals:** video decode (movie channels get an honest fallback), BGAPAN pan/opacity animation, `bg_alpha`/`movie_alpha` dimming (that hook belongs to the accessibility plan).

---

### Task 1: Parse BGA channels as measure-slot base36 sequences

**Files:**
- Modify: `crates/dtx-core/src/parser.rs:308-326`
- Modify: `crates/dtx-core/src/bga.rs` (fraction + id fidelity)

- [ ] **Step 1: Understand the two existing paths (read before editing)**

Read `parser.rs:300-360`. The note-channel path (:339-353) splits the object line into 2-char base36 cells, emitting one `Chip` per non-zero cell with an intra-measure position. Confirm exactly how a `Chip` records intra-measure position (field or via value encoding) — the BGA fix must produce chips the same way, with `value` = the base36 id as f32. Record the mechanism in the commit message.

- [ ] **Step 2: Write the failing tests**

In `parser.rs` tests (near the existing `#BMP01: bg.bmp` test at :521-531):

```rust
#[test]
fn bga_channel_parses_base36_slots() {
    let src = "#TITLE: t\n#BMP01: a.png\n#BMP0A: b.png\n#00004: 01000A00\n";
    let chart = parse(src.as_bytes()).unwrap();
    let events = bga_events(&chart);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].bmp_index, 1);
    assert_eq!(events[0].fraction, 0.0);
    assert_eq!(events[1].bmp_index, 10); // base36 "0A"
    assert_eq!(events[1].fraction, 0.5); // slot 2 of 4
}

#[test]
fn bga_zero_slots_emit_no_events() {
    let src = "#TITLE: t\n#00055: 0000\n";
    let chart = parse(src.as_bytes()).unwrap();
    assert!(bga_events(&chart).is_empty());
}
```

(Adjust the import path for `bga_events` — it lives in `dtx_core::bga`.)

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p dtx-core -j 2 bga_channel`
Expected: FAIL — single-float parsing produces 1 event with `bmp_index = 1000` (or similar garbage).

- [ ] **Step 4: Implement**

In `parser.rs:308-326`, route the BGA/movie channels (4, 7, 0x54, 0x55, 0x56..=0x60, 0x5A) through the same slot-parsing helper the note channels use (:339-353) instead of `value.parse::<f32>()`. If the helper is note-specific, extract the shared part into a fn `parse_object_row(measure, channel, line) -> Vec<Chip>` used by both. Keep single-float parsing for the channels that genuinely are scalars (BPM channel 8, bar-length channel 2 — check the match arms; do NOT touch them).

In `bga.rs:118-134` (`bga_events`): take `fraction` from the chip's intra-measure position (Step 1's mechanism) instead of the hardcoded `0.0` (:128); `bmp_index` remains `c.value as u32`, which is now the true base36 id.

- [ ] **Step 5: Run the full dtx-core suite**

Run: `cargo test -p dtx-core -j 2`
Expected: PASS. Watch specifically for `bga_basic.dtx`-derived assertions in `dtx-bga` (`../dtx-core/tests/fixtures`): its lines are single-slot (`#00054: 1`), so event counts stay 5 — but run `cargo test -p dtx-bga -j 2` too and fix `approx_ms`/ordering expectations if fraction changes timing (`bga_fixture_bpm_timing_120` asserts measure-21 → 42000 ms; single-slot lines have fraction 0, so it must still pass unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-core crates/dtx-bga
git commit -m "fix(dtx-core): parse BGA channels as base36 measure slots"
```

---

### Task 2: Thread the BMP registry and source directory into dtx-bga

**Files:**
- Modify: `crates/dtx-bga/src/lib.rs` (`ActiveChartRes`, :195-201)
- Modify: `crates/game-menu/src/song_loading.rs` (:227-233)

- [ ] **Step 1: Write the failing test**

In `dtx-bga/src/lib.rs` tests:

```rust
#[test]
fn active_chart_res_resolves_bmp_paths() {
    let mut bmp = std::collections::HashMap::new();
    bmp.insert(1, "bg.png".to_string());
    let res = ActiveChartRes {
        bpm: 120.0,
        events: vec![],
        bmp,
        source_dir: std::path::PathBuf::from("/songs/foo"),
    };
    assert_eq!(
        res.image_path(1),
        Some(std::path::PathBuf::from("/songs/foo/bg.png"))
    );
    assert_eq!(res.image_path(99), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dtx-bga -j 2 resolves_bmp`
Expected: FAIL — fields/method missing.

- [ ] **Step 3: Implement**

Extend `ActiveChartRes`:

```rust
#[derive(Resource, Default)]
pub struct ActiveChartRes {
    pub bpm: f32,
    pub events: Vec<BgaEvent>,
    /// #BMPxx id -> filename, from the chart's DtxAssets.
    pub bmp: std::collections::HashMap<u32, String>,
    /// Directory of the .dtx file; BMP filenames resolve relative to it.
    pub source_dir: std::path::PathBuf,
}

impl ActiveChartRes {
    pub fn image_path(&self, bmp_index: u32) -> Option<std::path::PathBuf> {
        self.bmp.get(&bmp_index).map(|f| self.source_dir.join(f))
    }
}
```

In `song_loading.rs` `poll_chart_parse` (:227-233), populate the new fields. The `BmpRegistry` internals may be private — check `crates/dtx-core/src/assets.rs:74-110`; if `by_id` isn't public, add an accessor `pub fn iter(&self) -> impl Iterator<Item = (u32, &str)>` to `BmpRegistry` and build the HashMap from it:

```rust
commands.insert_resource(ActiveChartRes {
    bpm: chart.metadata.bpm.unwrap_or(120.0),
    events,
    bmp: chart.assets.bmp.iter().map(|(id, f)| (id, f.to_string())).collect(),
    source_dir: drums_chart
        .source_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default(),
});
```

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p dtx-bga -p dtx-core -j 2 && cargo check -p game-menu -j 2`
Expected: PASS / clean (existing `active_chart_res_default_empty_events` test needs the new `Default` derive or field updates).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-bga crates/game-menu crates/dtx-core
git commit -m "feat(bga): thread BMP registry and chart source dir into ActiveChartRes"
```

---

### Task 3: Render real images with per-layer replacement and correct z-order

**Files:**
- Modify: `crates/dtx-bga/src/lib.rs` (`activate_event` :130-148, `spawn_overlay` :150-169, `overlay_geometry` :173-191)

- [ ] **Step 1: Write the failing tests for the pure geometry/z rules**

```rust
#[test]
fn layer3_sits_behind_foreground_layers() {
    assert!(layer_z(BgaLayer::Layer3) < layer_z(BgaLayer::Layer1));
    assert!(layer_z(BgaLayer::Layer3) < layer_z(BgaLayer::Layer2));
    assert!(layer_z(BgaLayer::Layer1) < 0); // everything stays behind HUD
}

#[test]
fn geometry_keeps_bocud_regions() {
    let (x, y, w, h) = layer_rect(BgaLayer::Layer3);
    assert_eq!((x, y, w, h), (0.0, 0.0, 1280.0, 720.0)); // fullscreen
    let (_, _, w1, h1) = layer_rect(BgaLayer::Layer1);
    assert_eq!((w1, h1), (320.0, 240.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-bga -j 2 layer_`
Expected: FAIL — `layer_z`/`layer_rect` not found.

- [ ] **Step 3: Implement**

Split `overlay_geometry` into two pure fns and delete the placeholder colors:

```rust
/// UI z within the BGA band: Layer3 (fullscreen background) behind the
/// small foreground layers, everything behind the HUD (>= 0).
pub fn layer_z(layer: BgaLayer) -> i32 {
    match layer {
        BgaLayer::Layer3 => -3,
        _ => -2,
    }
}

/// BocuD screen regions in 1280x720 reference px (unchanged from the
/// placeholder implementation — keep the exact values currently in
/// overlay_geometry for Layer1/Layer2/LayerN).
pub fn layer_rect(layer: BgaLayer) -> (f32, f32, f32, f32) {
    match layer {
        BgaLayer::Layer1 => (0.0, 0.0, 320.0, 240.0),
        BgaLayer::Layer2 => (0.0, 240.0, 320.0, 240.0),
        BgaLayer::Layer3 => (0.0, 0.0, 1280.0, 720.0),
        // LayerN arms: copy the existing per-layer positions verbatim
        // Movie arms return (0.0, 0.0, 0.0, 0.0) — never spawned as images
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}
```

Rewrite `spawn_overlay` to take the resolved image path and use `ImageNode`:

```rust
fn spawn_overlay(
    commands: &mut Commands,
    asset_server: &AssetServer,
    layer: BgaLayer,
    bmp_index: u32,
    image_path: &std::path::Path,
) {
    let (x, y, w, h) = layer_rect(layer);
    commands.spawn((
        BgaLayerOverlay { layer, bmp_index },
        ImageNode::new(asset_server.load(image_path.to_string_lossy().to_string())),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        ZIndex(layer_z(layer)),
    ));
}
```

Rewrite `activate_event` semantics (BocuD: a new event on a layer REPLACES that layer's image):

```rust
fn activate_event(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player: &mut BgaPlayer,
    chart: &ActiveChartRes,
    existing: &Query<(Entity, &BgaLayerOverlay)>,
    ev: &BgaEvent,
) {
    if ev.layer.is_movie() {
        player.movies_skipped += 1;
        return; // honest fallback handled by Task 4
    }
    // Replace: despawn any overlay already on this layer.
    for (entity, overlay) in existing.iter() {
        if overlay.layer == ev.layer {
            commands.entity(entity).despawn();
        }
    }
    let Some(path) = chart.image_path(ev.bmp_index) else {
        player.layers.insert(ev.layer, BgaLayerState::Ended);
        bevy::log::warn!("BGA: #BMP{:02} undefined for {:?}", ev.bmp_index, ev.layer);
        return;
    };
    player.layers.insert(ev.layer, BgaLayerState::Displaying);
    player.activations += 1;
    spawn_overlay(commands, asset_server, ev.layer, ev.bmp_index, &path);
}
```

`tick_bga_player` gains `Res<AssetServer>`, `Res<ActiveChartRes>` (already there), and the `Query<(Entity, &BgaLayerOverlay)>`; delete the no-op cleanup loop (:114-124) — replacement now happens in `activate_event`.

Missing files on disk (registry names a file that doesn't exist) load as a broken handle and render nothing visible — acceptable v1; log via the existing load-state polling pattern only if trivial, otherwise skip (roadmap: skip + log, no crash).

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p dtx-bga -j 2 && cargo check -p game-menu -j 2`
Expected: PASS / clean. Delete the dead `overlay_geometry` tests (`bga_layer_overlay_geometry_unique_per_layer`, `bga_movie_overlay_geometry_is_zero`) — replaced by `layer_z`/`layer_rect` tests.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-bga
git commit -m "feat(bga): render real BMP images with per-layer replacement and z-order"
```

---

### Task 4: Honest movie fallback

Roadmap failure handling: "Show an honest fallback for unsupported BGA video."

**Files:**
- Modify: `crates/dtx-bga/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn movie_fallback_text_appears_once() {
    assert_eq!(movie_fallback_text(0), None);
    assert_eq!(
        movie_fallback_text(1).as_deref(),
        Some("BGA video not supported — showing images only")
    );
    assert_eq!(movie_fallback_text(3).as_deref(), movie_fallback_text(1).as_deref());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dtx-bga -j 2 movie_fallback`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
pub fn movie_fallback_text(movies_skipped: u32) -> Option<&'static str> {
    (movies_skipped > 0).then_some("BGA video not supported — showing images only")
}
```

Add a small system `show_movie_fallback` (Update, same plugin): when `movie_fallback_text(player.movies_skipped)` is `Some` and no marker entity exists, spawn a one-line `Text` (12 px, `theme.text_secondary`-style dim white `srgba(1.,1.,1.,0.4)`, absolute bottom-left `left: Px(8.), bottom: Px(8.)`, `ZIndex(-1)`) with marker component `MovieFallbackLabel`; include `MovieFallbackLabel` entities in the `OnExit(Performance)` cleanup in `song_loading.rs:133-142` (add it to the despawn query, or give the label the `BgaLayerOverlay` component with `layer: Movie` so the existing cleanup catches it — prefer the marker + explicit cleanup for clarity).

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p dtx-bga -j 2 && cargo check -p game-menu -j 2`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-bga crates/game-menu
git commit -m "feat(bga): honest on-screen fallback for movie channels"
```

---

### Task 5: BMP decode support + fixtures with real assets

**Files:**
- Modify: root `Cargo.toml` (bevy features) — only if needed
- Create: `crates/dtx-core/tests/fixtures/bga_images.dtx`
- Create: `crates/dtx-core/tests/fixtures/bga_red.png`, `crates/dtx-core/tests/fixtures/bga_blue.png`
- Modify: `crates/dtx-bga/tests/integration_bga.rs`

- [ ] **Step 1: Check bevy image-format features**

Run: `grep -n 'features' Cargo.toml | head` then inspect the workspace `bevy` dependency's feature list, and `cargo metadata --format-version 1 | python3 -c "import json,sys; m=json.load(sys.stdin); print([f for p in m['packages'] if p['name']=='bevy' for f in p['features'] if f in ('bmp','png','jpeg')])"`
PNG is on by default with `bevy_image`; if `bmp` is absent from resolved features, add `"bmp"` (and `"jpeg"` if missing — registry values in the wild use `.bmp`/`.jpg`) to the workspace bevy features. If they're already active, skip the Cargo change.

- [ ] **Step 2: Create fixture assets**

Generate two 8×8 PNGs (no binary blobs in the plan — generate deterministically):

```bash
python3 - <<'EOF'
import struct, zlib
def png(path, rgb):
    raw = b''.join(b'\x00' + bytes(rgb)*8 for _ in range(8))
    def chunk(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c))
    open(path,'wb').write(
        b'\x89PNG\r\n\x1a\n'
        + chunk(b'IHDR', struct.pack('>IIBBBBB', 8, 8, 8, 2, 0, 0, 0))
        + chunk(b'IDAT', zlib.compress(raw))
        + chunk(b'IEND', b''))
png('crates/dtx-core/tests/fixtures/bga_red.png', (255,0,0))
png('crates/dtx-core/tests/fixtures/bga_blue.png', (0,0,255))
EOF
```

`crates/dtx-core/tests/fixtures/bga_images.dtx`:

```
#TITLE: BGA images fixture
#BPM: 120
#BMP01: bga_red.png
#BMP02: bga_blue.png

#00055: 01
#00104: 02
#00255: 02
```

(Layer3 image at measure 0, Layer1 at measure 1, Layer3 replaced at measure 2. `#BMP03` deliberately absent so a missing-id case can be added later without a new fixture.)

- [ ] **Step 3: Write integration tests**

Append to `crates/dtx-bga/tests/integration_bga.rs`:

```rust
#[test]
fn bga_images_fixture_resolves_paths() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dtx-core/tests/fixtures/bga_images.dtx");
    let file = std::fs::File::open(&path).unwrap();
    let chart = dtx_core::parser::parse(file).unwrap();
    let events = dtx_core::bga::bga_events(&chart);
    assert_eq!(events.len(), 3);
    assert_eq!(chart.assets.bmp.get(1), Some("bga_red.png"));
    // every event's id resolves in the registry, and the file exists on disk
    let dir = path.parent().unwrap();
    for ev in &events {
        let name = chart.assets.bmp.get(ev.bmp_index).expect("id in registry");
        assert!(dir.join(name).exists(), "{name} missing");
    }
}
```

(Match `BmpRegistry::get`'s actual return type — `Option<&String>` vs `Option<&str>` — check `assets.rs:74-110`.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-bga -p dtx-core -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-core/tests/fixtures crates/dtx-bga/tests Cargo.toml
git commit -m "test(bga): fixture chart with real image assets; enable bmp decode"
```

---

### Task 6: Manual verification

- [ ] **Step 1: Real chart run (bevy-brp)**

Launch `dtxmaniars` with a real DTX song that has BGA images (or point `DTX_SONG_DIR` at a folder containing the fixture). Play; screenshot at a known BGA measure:
- Layer3 image fills the background behind the HUD; Layer1/2 images sit in their BocuD regions above it.
- A later event on the same layer REPLACES the image (no stacking).
- A chart with movie channels shows the "BGA video not supported" line, once.
- A chart with a `#BMPxx` naming a missing file plays without crashing.

- [ ] **Step 2: Full sweep + scoped fmt**

Run: `cargo test -p dtx-core -p dtx-bga -p game-menu -j 2`
Then scoped `cargo fmt -p dtx-bga -p dtx-core -- <changed files>`.

---

## Failure-handling mapping (roadmap)

- "Show an honest fallback for unsupported BGA video" → Task 4.
- "Fixture charts for BGA layers, missing assets" → Task 5 (+ missing-id warn path in Task 3).
- "Skip malformed charts…log paths and reasons" → warn logs in Task 3 for undefined ids.

## Deferred (explicitly)

- Video decode (roadmap non-goal; separate future work).
- BGAPAN animation, opacity, `Cueing` state semantics.
- `bg_alpha`/`movie_alpha` dimming — consumed by the accessibility plan (`2026-07-11-shared-control-accessibility.md`), which will multiply into the `ImageNode` color.
