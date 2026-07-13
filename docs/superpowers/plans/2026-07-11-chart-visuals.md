# Chart Visuals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render chart-authored BGA images and AVI movies behind drum gameplay, wire the four existing visual settings, and show `#PREIMAGE` in the Now Playing card.

**Architecture:** `dtx-core` preserves visual asset IDs and fractional chip timing. `dtx-bga` resolves visual assets, schedules static layers against a gameplay-clock bridge, and decodes movies on a bounded worker through `video-rs`. `gameplay-drums` supplies authoritative chart time, live settings, and performance cover art.

**Tech Stack:** Rust 1.95+, Bevy 0.19, `video-rs` 0.11, system FFmpeg libraries, std threads/channels, existing `dtx-core` timing math.

## Global Constraints

- Read `AGENTS.md`, `crates/dtx-core/AGENTS.md`, `crates/dtx-bga/AGENTS.md`, and `crates/gameplay-drums/AGENTS.md` before implementation.
- Cite `references/DTXmaniaNX/...:L<line>` in commits for ported chart behavior.
- Keep `references/` read-only.
- Keep `dtx-core` Bevy-free. Game crates may depend on Engine crates; Engine crates may depend on Pure crates.
- Keep `unsafe_code = "forbid"` in workspace code. Unsafe internals may remain inside third-party decoder crates.
- Do not use `unwrap()` in `crates/*`.
- Decode video off Bevy's main thread. Keep at most two decoded frames buffered.
- Ignore movie audio. Existing chart BGM and keysounds remain authoritative.
- Render movies aspect-fit behind lanes and HUD.
- Implement only direct BMP/BGA image display and AVI movie playback. Leave `BGAPAN`, `AVIPAN`, windowed movies, hardware zero-copy, and FFmpeg bundling out of scope.
- Preserve serialized config field names.
- Use TDD: write each behavior test, run it red, then add minimum implementation.
- Serialize Bevy-heavy builds. Do not clean shared Cargo target.

---

## File Structure

### Create

- `crates/dtx-bga/src/chart.rs` — resolve chart visual assets and produce BPM-aware timed events.
- `crates/dtx-bga/src/video.rs` — `video-rs` worker, bounded frame queue, seek/backpressure logic.
- `crates/dtx-bga/tests/fixtures/tiny.avi` — committed three-frame decoder fixture.
- `crates/dtx-bga/tests/fixtures/red.png` — committed static image fixture.

### Modify

- `Cargo.toml` — workspace `video-rs` dependency.
- `Cargo.lock` — resolved decoder dependencies.
- `.github/workflows/ci.yml` — FFmpeg development packages.
- `crates/dtx-core/src/assets.rs` — generic case-insensitive chart-asset resolver.
- `crates/dtx-core/src/parser.rs` — standard visual chip sequence parsing.
- `crates/dtx-core/src/bga.rs` — use `wav_slot` and fractional `value` in `BgaEvent`.
- `crates/dtx-core/tests/parser_tests.rs` — visual sequence regression coverage.
- `crates/dtx-core/tests/fixtures/bga_basic.dtx` — valid BMP/AVI directives and timed sequences.
- `crates/dtx-audio/src/lib.rs` — reuse generic chart-asset resolver.
- `crates/dtx-bga/Cargo.toml` — `dtx-config` and `video-rs` dependencies.
- `crates/dtx-bga/src/lib.rs` — plugin, resources, static image/movie rendering, cleanup.
- `crates/dtx-bga/tests/integration_bga.rs` — prepared-event and decoder integration.
- `crates/dtx-config/src/lib.rs` — correct reversed BGA/Movie field comments.
- `crates/game-menu/src/song_loading.rs` — publish prepared chart visuals.
- `crates/gameplay-drums/Cargo.toml` — depend on `dtx-bga`.
- `crates/gameplay-drums/src/lib.rs` — saved settings apply and clock bridge registration.
- `crates/gameplay-drums/src/hud.rs` — performance cover synchronization.
- `crates/gameplay-drums/src/editor/settings_data.rs` — four visual controls.
- `crates/gameplay-drums/src/editor/tabs.rs` — live visual settings apply.
- `crates/dtx-ui/src/widget/now_playing.rs` — image-capable art tile.
- `crates/dtx-bga/AGENTS.md` — replace M7 placeholder/deferred description with implemented M7.1 behavior.

---

### Task 1: Preserve visual chip timing and share path resolution

**Files:**
- Modify: `crates/dtx-core/src/parser.rs`
- Modify: `crates/dtx-core/src/bga.rs`
- Modify: `crates/dtx-core/src/assets.rs`
- Modify: `crates/dtx-core/tests/parser_tests.rs`
- Modify: `crates/dtx-core/tests/fixtures/bga_basic.dtx`
- Modify: `crates/dtx-audio/src/lib.rs`

**References:**
- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1296-1476`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfBGA.cs:61-96`

**Interfaces:**
- Produces: `dtx_core::resolve_chart_asset_path(chart_dir: &Path, filename: &str) -> Option<PathBuf>`.
- Produces: `BgaEvent { measure, layer, bmp_index, fraction }` from `Chip.wav_slot` and `Chip.value`.
- Preserves: `dtx_audio::resolve_chart_audio_path(...) -> PathBuf` public behavior.

- [ ] **Step 1: Write failing parser tests**

Add tests that parse real two-character sequences:

```rust
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
}
```

Add a `bga_events` assertion:

```rust
let events = dtx_core::bga::bga_events(&chart);
assert_eq!(events[0].bmp_index, 1);
assert_eq!(events[1].bmp_index, 2);
assert_eq!(events[1].fraction, 0.5);
```

- [ ] **Step 2: Run tests red**

Run:

```sh
cargo test -p dtx-core visual_sequences_preserve_asset_id_and_fraction -- --exact
```

Expected: FAIL because visual channels currently parse the full value as one decimal number and set fraction to zero.

- [ ] **Step 3: Remove the visual-channel decimal special case**

Delete the branch in `parse_chip_line` that parses BGA/Movie `value` directly as `f32`. Let the existing generic two-character chip parser create `Chip::with_wav(measure, channel, fraction, slot)` for visual channels.

Update `bga_events`:

```rust
Some(BgaEvent {
    measure: c.measure,
    layer,
    bmp_index: c.wav_slot,
    fraction: c.value,
})
```

Sort by measure and fraction while preserving source order for equal timestamps:

```rust
events.sort_by(|a, b| {
    a.measure
        .cmp(&b.measure)
        .then_with(|| a.fraction.total_cmp(&b.fraction))
});
```

Update `bga_basic.dtx` with valid declarations and sequences:

```text
#BMP01: red.png
#BMP02: blue.png
#AVI03: tiny.avi
#00004: 01
#00054: 03
#00055: 01
#02054: 03
#02055: 02
```

- [ ] **Step 4: Add failing chart-asset resolver tests**

Add to `assets.rs` tests:

```rust
#[test]
fn resolve_chart_asset_path_matches_case_insensitively() {
    let dir = std::env::temp_dir().join(format!(
        "dtx-core-visual-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&dir).expect("create temp chart dir");
    std::fs::write(dir.join("Jacket.PNG"), b"x").expect("write fixture");

    assert_eq!(
        resolve_chart_asset_path(&dir, "jacket.png"),
        Some(dir.join("Jacket.PNG"))
    );

    std::fs::remove_dir_all(dir).expect("remove temp chart dir");
}
```

- [ ] **Step 5: Run resolver test red**

Run:

```sh
cargo test -p dtx-core resolve_chart_asset_path_matches_case_insensitively -- --exact
```

Expected: FAIL because `resolve_chart_asset_path` does not exist.

- [ ] **Step 6: Implement generic resolver and reuse it from audio**

Add:

```rust
pub fn resolve_chart_asset_path(chart_dir: &Path, filename: &str) -> Option<PathBuf> {
    let direct = chart_dir.join(filename);
    if direct.is_file() {
        return Some(direct);
    }
    let wanted = Path::new(filename).file_name()?.to_str()?;
    std::fs::read_dir(chart_dir)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        })
        .map(|entry| entry.path())
}
```

Re-export it from `dtx-core/src/lib.rs`. Keep audio's fallback behavior:

```rust
pub fn resolve_chart_audio_path(chart_dir: &Path, filename: &str) -> PathBuf {
    dtx_core::resolve_chart_asset_path(chart_dir, filename)
        .unwrap_or_else(|| chart_dir.join(filename))
}
```

- [ ] **Step 7: Run package tests**

Run:

```sh
cargo test -p dtx-core
cargo test -p dtx-audio --lib
```

Expected: PASS.

- [ ] **Step 8: Commit**

```sh
git add crates/dtx-core crates/dtx-audio/src/lib.rs
git commit -m "fix(core): preserve chart visual timing"
```

Commit body must cite `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1296-1476`.

---

### Task 2: Prepare BPM-aware visual events and render BMP layers

**Files:**
- Create: `crates/dtx-bga/src/chart.rs`
- Modify: `crates/dtx-bga/src/lib.rs`
- Modify: `crates/dtx-bga/Cargo.toml`
- Modify: `crates/dtx-bga/tests/integration_bga.rs`
- Create: `crates/dtx-bga/tests/fixtures/red.png`
- Modify: `crates/game-menu/src/song_loading.rs`

**Interfaces:**
- Consumes: `dtx_core::resolve_chart_asset_path` and corrected `BgaEvent`.
- Produces: `TimedVisualEvent { target_ms: i64, layer: BgaLayer, asset_id: u32 }`.
- Produces: `ActiveChartRes::from_chart(chart: &Chart, source_path: Option<&Path>) -> Self`.
- Produces: `BgaClock { current_ms: i64 }` and static image layer entities.

- [ ] **Step 1: Write failing prepared-chart tests**

Add tests:

```rust
#[test]
fn active_chart_res_resolves_assets_and_bpm_aware_times() {
    let dir = fixture_dir();
    let chart = dtx_core::parser::parse(
        std::fs::File::open(dir.join("visual.dtx")).expect("fixture chart"),
    )
    .expect("parse fixture");
    let prepared = ActiveChartRes::from_chart(&chart, Some(&dir.join("visual.dtx")));

    assert_eq!(prepared.events[0].target_ms, 0);
    assert_eq!(prepared.events[0].asset_id, 1);
    assert_eq!(prepared.bmp_paths.get(&1), Some(&dir.join("red.png")));
}
```

Add a chart with a BPM change and assert the later visual event uses `chip_time_ms_with_bpm_and_bar_changes`, not constant-BPM `approx_ms`.

- [ ] **Step 2: Run prepared-chart tests red**

Run:

```sh
cargo test -p dtx-bga active_chart_res_resolves_assets_and_bpm_aware_times -- --exact
```

Expected: FAIL because `TimedVisualEvent` and `ActiveChartRes::from_chart` do not exist.

- [ ] **Step 3: Implement `chart.rs`**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedVisualEvent {
    pub target_ms: i64,
    pub layer: BgaLayer,
    pub asset_id: u32,
}

pub fn timed_visual_events(chart: &Chart) -> Vec<TimedVisualEvent> {
    let mut bpm_changes: Vec<BpmChange> = chart
        .chips
        .iter()
        .filter(|chip| matches!(chip.channel, EChannel::BPM | EChannel::BPMEx))
        .map(|chip| BpmChange { measure: chip.measure, bpm: chip.value })
        .collect();
    bpm_changes.sort_by_key(|change| change.measure);

    let mut bar_changes: Vec<BarLengthChange> = chart
        .chips
        .iter()
        .filter(|chip| chip.channel == EChannel::BarLength)
        .map(|chip| BarLengthChange { measure: chip.measure, ratio: chip.value })
        .collect();
    bar_changes.sort_by_key(|change| change.measure);

    let timing = ChartTiming {
        bpm_changes: &bpm_changes,
        bar_changes: &bar_changes,
    };
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);

    chart
        .chips
        .iter()
        .filter_map(|chip| {
            let layer = BgaLayer::from_channel(chip.channel)?;
            Some(TimedVisualEvent {
                target_ms: chip_time_ms_with_bpm_and_bar_changes(
                    chip.measure,
                    chip.value,
                    base_bpm,
                    timing,
                ),
                layer,
                asset_id: chip.wav_slot,
            })
        })
        .collect()
}
```

Build BMP/AVI maps with `resolve_chart_asset_path`. Missing files stay absent from maps and produce no panic.

- [ ] **Step 4: Replace SongLoading's literal resource construction**

Use:

```rust
commands.insert_resource(ActiveChartRes::from_chart(&chart, path.as_deref()));
```

Remove the old `{ bpm, events }` construction.

- [ ] **Step 5: Write failing static layer tests**

Use a headless `App` to insert `ActiveChartRes`, `BgaClock`, and `AssetServer`, then advance the clock. Assert one `BgaLayerOverlay` with an `ImageNode` exists, a later event replaces only the same layer, and another layer remains.

- [ ] **Step 6: Run static layer tests red**

Run:

```sh
cargo test -p dtx-bga static_image_event_replaces_only_target_layer -- --exact
```

Expected: FAIL because the plugin still spawns colored placeholders.

- [ ] **Step 7: Implement static image layers**

Change `BgaLayerOverlay` to include the resolved asset ID. Query overlays by component and despawn only the entity for the incoming layer. Spawn:

```rust
commands.spawn((
    BgaLayerOverlay {
        layer: event.layer,
        asset_id: event.asset_id,
    },
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    },
    ImageNode {
        image: asset_server.load(path.to_string_lossy().to_string()),
        color: Color::WHITE.with_alpha(settings.image_alpha),
        ..default()
    },
    ZIndex(-100),
));
```

Do not create an entity when the asset map lacks the ID. Track warned `(layer, asset_id)` pairs so a missing file logs once.

- [ ] **Step 8: Run package tests**

```sh
cargo test -p dtx-bga
cargo check -p game-menu
```

Expected: PASS.

- [ ] **Step 9: Commit**

```sh
git add crates/dtx-bga crates/game-menu/src/song_loading.rs
git commit -m "feat(bga): render timed chart images"
```

Commit body must cite `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfBGA.cs:61-96`.

---

### Task 3: Wire the gameplay clock and four visual settings

**Files:**
- Modify: `crates/dtx-config/src/lib.rs`
- Modify: `crates/dtx-bga/Cargo.toml`
- Modify: `crates/dtx-bga/src/lib.rs`
- Modify: `crates/gameplay-drums/Cargo.toml`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/editor/settings_data.rs`
- Modify: `crates/gameplay-drums/src/editor/tabs.rs`

**Interfaces:**
- Produces: `BgaSettings { images_enabled, movie_enabled, image_alpha, movie_alpha }`.
- Produces: `impl From<&dtx_config::SystemConfig> for BgaSettings`.
- Produces: `sync_bga_clock(GameplayClock -> BgaClock)`.

- [ ] **Step 1: Write failing settings conversion tests**

```rust
#[test]
fn bga_settings_map_existing_config_fields() {
    let system = dtx_config::SystemConfig {
        bga_enabled: false,
        movie_enabled: true,
        bg_alpha: 128,
        movie_alpha: 64,
        ..Default::default()
    };
    let settings = BgaSettings::from(&system);
    assert!(!settings.images_enabled);
    assert!(settings.movie_enabled);
    assert!((settings.image_alpha - 128.0 / 255.0).abs() < f32::EPSILON);
    assert!((settings.movie_alpha - 64.0 / 255.0).abs() < f32::EPSILON);
}
```

- [ ] **Step 2: Run settings test red**

```sh
cargo test -p dtx-bga bga_settings_map_existing_config_fields -- --exact
```

Expected: FAIL because `BgaSettings` does not exist.

- [ ] **Step 3: Implement settings resource**

```rust
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct BgaSettings {
    pub images_enabled: bool,
    pub movie_enabled: bool,
    pub image_alpha: f32,
    pub movie_alpha: f32,
}

impl From<&dtx_config::SystemConfig> for BgaSettings {
    fn from(value: &dtx_config::SystemConfig) -> Self {
        Self {
            images_enabled: value.bga_enabled,
            movie_enabled: value.movie_enabled,
            image_alpha: value.bg_alpha as f32 / 255.0,
            movie_alpha: value.movie_alpha as f32 / 255.0,
        }
    }
}
```

Initialize it from `dtx_config::load(&dtx_config::default_path()).system`. Correct the reversed field comments in `dtx-config`; do not rename fields.

- [ ] **Step 4: Write failing settings-row tests**

Assert System settings contain `BGA Images`, `Chart Movie`, `BGA Opacity`, and `Movie Opacity`; toggles mutate the expected booleans; sliders clamp and round to `u8`.

- [ ] **Step 5: Run settings-row tests red**

```sh
cargo test -p gameplay-drums editor::settings_data --lib
```

Expected: FAIL because the rows are absent.

- [ ] **Step 6: Add four System settings rows**

For opacity setters, use:

```rust
set: |c, v| c.system.bg_alpha = v.round().clamp(0.0, 255.0) as u8,
```

Use slider range `0.0..=255.0`, step `1.0`, and percentage display:

```rust
value: |c| format!("{}%", (c.system.bg_alpha as f32 / 255.0 * 100.0).round() as i32),
```

Mirror this for movie opacity.

- [ ] **Step 7: Add clock bridge and live apply**

Add `dtx-bga` to `gameplay-drums`. Register:

```rust
fn sync_bga_clock(
    gameplay: Res<resources::GameplayClock>,
    mut visuals: ResMut<dtx_bga::BgaClock>,
) {
    visuals.current_ms = gameplay.current_ms;
}
```

Run it in `Update` while `AppState::Performance`, before `dtx_bga` consumes the clock if ordering is available; otherwise move BGA consumption into an exported `SystemSet` and order against it.

Extend `apply_config_on_enter` and `apply_draft_live`:

```rust
*bga_settings = dtx_bga::BgaSettings::from(&cfg.system);
```

and:

```rust
*bga_settings = dtx_bga::BgaSettings::from(&draft.0.system);
```

When images are disabled, hide image overlays immediately. When re-enabled, rebuild active layers at current `BgaClock` time. Apply alpha changes to existing nodes without respawning them.

- [ ] **Step 8: Run package tests**

```sh
cargo test -p dtx-config --lib
cargo test -p dtx-bga
cargo test -p gameplay-drums --lib
```

Expected: PASS.

- [ ] **Step 9: Commit**

```sh
git add crates/dtx-config crates/dtx-bga crates/gameplay-drums
git commit -m "feat(settings): wire chart visual controls"
```

---

### Task 4: Show `#PREIMAGE` in the performance Now Playing card

**Files:**
- Modify: `crates/dtx-ui/src/widget/now_playing.rs`
- Modify: `crates/gameplay-drums/src/hud.rs`

**References:**
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/InfoBox.cs:20-34`

**Interfaces:**
- Consumes: `ActiveChart.chart.metadata.preimage_filename` and `ActiveChart.source_path`.
- Produces: `performance_preimage_path(chart: &ActiveChart) -> Option<PathBuf>`.

- [ ] **Step 1: Write failing path tests**

```rust
#[test]
fn performance_preimage_resolves_case_insensitively() {
    let dir = unique_temp_dir("performance-cover");
    std::fs::create_dir_all(&dir).expect("create cover dir");
    std::fs::write(dir.join("Cover.PNG"), b"image").expect("write cover");
    let chart = ActiveChart::new(
        Chart {
            metadata: Metadata {
                preimage_filename: Some("cover.png".into()),
                ..Default::default()
            },
            ..Default::default()
        },
        Some(dir.join("song.dtx")),
    );
    assert_eq!(performance_preimage_path(&chart), Some(dir.join("Cover.PNG")));
    std::fs::remove_dir_all(dir).expect("remove cover dir");
}
```

Add missing metadata and missing file cases returning `None`.

- [ ] **Step 2: Run cover tests red**

```sh
cargo test -p gameplay-drums performance_preimage --lib
```

Expected: FAIL because helper does not exist.

- [ ] **Step 3: Make Now Playing art image-capable**

Attach `ImageNode::default()` to `NowPlayingArt`. Keep its `BackgroundColor` neutral tile. The 60 by 60 node remains unchanged so layout/widget persistence does not move.

- [ ] **Step 4: Implement cover synchronization**

Extend `sync_now_playing` with:

```rust
mut q_art: Query<
    (&mut ImageNode, &mut BackgroundColor),
    With<now_playing::NowPlayingArt>,
>,
asset_server: Res<AssetServer>,
```

For a valid path:

```rust
image.image = asset_server.load(path.to_string_lossy().to_string());
image.color = Color::WHITE;
bg.0 = bg.0.with_alpha(0.0);
```

For fallback:

```rust
image.image = Handle::default();
image.color = image.color.with_alpha(0.0);
bg.0 = Color::srgb(0.15, 0.15, 0.2);
```

Use `dtx_core::resolve_chart_asset_path` in `performance_preimage_path`.

- [ ] **Step 5: Run package tests**

```sh
cargo test -p gameplay-drums --lib
cargo check -p dtx-ui
```

Expected: PASS.

- [ ] **Step 6: Commit**

```sh
git add crates/dtx-ui/src/widget/now_playing.rs crates/gameplay-drums/src/hud.rs
git commit -m "feat(hud): show chart cover in now playing"
```

Commit body must cite `references/DTXmaniaNX/DTXMania/Stage/06.Performance/InfoBox.cs:20-34`.

---

### Task 5: Add the bounded `video-rs` decoder worker

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/dtx-bga/Cargo.toml`
- Create: `crates/dtx-bga/src/video.rs`
- Create: `crates/dtx-bga/tests/fixtures/tiny.avi`
- Modify: `crates/dtx-bga/tests/integration_bga.rs`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `DecodedFrame { timestamp_ms, width, height, rgba }`.
- Produces: `MovieWorker::spawn(path: PathBuf) -> Self`.
- Produces: `MovieWorker::set_target_ms(&self, target_ms: i64)`.
- Produces: `MovieWorker::newest_due_frame(&self, target_ms: i64) -> Option<DecodedFrame>`.
- Produces: `MovieWorker::take_error(&self) -> Option<String>`.

- [ ] **Step 1: Add dependency and CI packages**

Add:

```toml
video-rs = { version = "0.11", features = ["ndarray"] }
```

Add `video-rs = { workspace = true }` to `dtx-bga`.

Extend the existing one-line apt install with:

```text
libavcodec-dev libavformat-dev libavutil-dev libswscale-dev
```

Do not split the apt command across YAML lines; the workflow comment records why.

- [ ] **Step 2: Generate committed decoder fixture**

Run:

```sh
mkdir -p crates/dtx-bga/tests/fixtures
ffmpeg -y -f lavfi -i color=c=red:s=16x16:r=3:d=1 -an -c:v mjpeg crates/dtx-bga/tests/fixtures/tiny.avi
```

Verify:

```sh
ffprobe -v error -select_streams v:0 -show_entries stream=width,height,nb_frames -of default=nw=1 crates/dtx-bga/tests/fixtures/tiny.avi
```

Expected: width 16, height 16, three frames.

- [ ] **Step 3: Write failing bounded queue tests**

Test pure queue behavior with synthetic frames:

```rust
#[test]
fn frame_queue_never_exceeds_two_and_returns_newest_due() {
    let queue = FrameQueue::default();
    queue.push(frame(0));
    queue.push(frame(333));
    queue.push(frame(666));
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.newest_due(700).map(|f| f.timestamp_ms), Some(666));
}
```

Add a test that future frames remain queued when target time is earlier.

- [ ] **Step 4: Run queue tests red**

```sh
cargo test -p dtx-bga frame_queue_never_exceeds_two_and_returns_newest_due -- --exact
```

Expected: FAIL because `FrameQueue` does not exist.

- [ ] **Step 5: Implement frame queue**

Use `Arc<Mutex<VecDeque<DecodedFrame>>>`. `push` removes the oldest frame while length is two. `newest_due` removes all due frames and returns the newest, leaving future frames in place. Handle poisoned locks by returning an empty result and recording an error instead of panicking.

Core shape:

```rust
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub timestamp_ms: i64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Default)]
struct FrameQueue(Arc<Mutex<VecDeque<DecodedFrame>>>);
```

- [ ] **Step 6: Write failing real decoder test**

```rust
#[test]
fn movie_worker_decodes_tiny_avi() {
    let path = fixture_dir().join("tiny.avi");
    let mut worker = MovieWorker::spawn(path);
    worker.set_target_ms(900);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let frame = loop {
        if let Some(frame) = worker.newest_due_frame(900) {
            break frame;
        }
        assert!(std::time::Instant::now() < deadline, "decoder timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!((frame.width, frame.height), (16, 16));
    assert_eq!(frame.rgba.len(), 16 * 16 * 4);
    worker.stop();
}
```

- [ ] **Step 7: Run decoder test red**

```sh
cargo test -p dtx-bga movie_worker_decodes_tiny_avi -- --exact
```

Expected: FAIL because `MovieWorker` does not exist.

- [ ] **Step 8: Implement worker**

Initialize `video-rs` once with `OnceLock<Result<(), String>>`. Spawn one std thread per active movie. Store target time in `Arc<AtomicI64>`, stop state in `Arc<AtomicBool>`, decoded frames in `FrameQueue`, and first error in `Arc<Mutex<Option<String>>>`.

Worker loop:

```rust
while !stop.load(Ordering::Acquire) {
    let target = target_ms.load(Ordering::Acquire).max(0);
    if target + 100 < last_timestamp || target > last_timestamp + 500 {
        if decoder.seek(target).is_err() {
            record_error("movie seek failed");
            break;
        }
        queue.clear();
    }
    if last_timestamp > target + 100 {
        std::thread::sleep(Duration::from_millis(2));
        continue;
    }
    match decoder.decode() {
        Ok((timestamp, rgb)) => {
            let rgba = rgb_to_rgba(rgb.as_slice().unwrap_or(&[]), width, height);
            last_timestamp = (timestamp.as_secs_f64() * 1000.0).round() as i64;
            queue.push(DecodedFrame { timestamp_ms: last_timestamp, width, height, rgba });
        }
        Err(error) if is_decode_exhausted(&error) => break,
        Err(error) => {
            record_error(format!("movie decode failed: {error}"));
            break;
        }
    }
}
```

Do not call ndarray `unwrap`; convert non-contiguous arrays by iterating pixels when `as_slice()` returns `None`. `Drop` calls `stop`, takes the `JoinHandle`, and joins it without panicking if the worker already failed.

- [ ] **Step 9: Run decoder tests and check**

```sh
cargo test -p dtx-bga movie_worker -- --nocapture
cargo check -p dtx-bga
```

Expected: PASS with no warnings.

- [ ] **Step 10: Commit**

```sh
git add Cargo.toml Cargo.lock .github/workflows/ci.yml crates/dtx-bga
git commit -m "feat(bga): decode chart movies with ffmpeg"
```

---

### Task 6: Render and synchronize fullscreen chart movies

**Files:**
- Modify: `crates/dtx-bga/src/lib.rs`
- Modify: `crates/dtx-bga/src/video.rs`
- Modify: `crates/dtx-bga/tests/integration_bga.rs`
- Modify: `crates/game-menu/src/song_loading.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`

**References:**
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:16-34`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:202-225`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:266-285`
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:319-332`

**Interfaces:**
- Consumes: `TimedVisualEvent`, `ActiveChartRes.avi_paths`, `BgaClock`, `BgaSettings`, and `MovieWorker`.
- Produces: one reusable movie `Handle<Image>` and one `MovieOverlay` entity.
- Produces: `clear_visuals` cleanup system callable from `game-menu` on Performance exit.

- [ ] **Step 1: Write failing clock-discontinuity tests**

```rust
#[test]
fn clock_discontinuity_detects_seek_not_normal_frame() {
    assert!(!clock_discontinuity(1_000, 1_016));
    assert!(clock_discontinuity(5_000, 2_000));
    assert!(clock_discontinuity(1_000, 5_000));
}
```

Add a test that rebuilding at a target chooses the last image event per layer and the last movie event at or before the target.

- [ ] **Step 2: Run discontinuity tests red**

```sh
cargo test -p dtx-bga clock_discontinuity_detects_seek_not_normal_frame -- --exact
```

Expected: FAIL because discontinuity/rebuild helpers do not exist.

- [ ] **Step 3: Implement seek-aware event cursor**

Treat backward time or a forward jump over 250 ms as a discontinuity. Rebuild static layer state from the latest image event per layer at `now_ms`. Select the latest movie event at or before `now_ms`, start that asset, and set worker target to `now_ms - event.target_ms`.

Normal forward frames continue processing due events from `next_event_idx` without rescanning the chart.

- [ ] **Step 4: Write failing texture upload tests**

Test a pure helper validates `rgba.len() == width * height * 4`, creates a new texture when dimensions change, and mutates existing `Image.data` when dimensions match. Invalid frame lengths return an error and keep the old texture.

- [ ] **Step 5: Run texture tests red**

```sh
cargo test -p dtx-bga movie_texture -- --nocapture
```

Expected: FAIL because movie texture helpers do not exist.

- [ ] **Step 6: Implement reusable dynamic texture**

Create Bevy images with:

```rust
Image::new(
    Extent3d {
        width: frame.width,
        height: frame.height,
        depth_or_array_layers: 1,
    },
    TextureDimension::D2,
    frame.rgba.clone(),
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
)
```

For matching dimensions, set `image.data = Some(frame.rgba)` and let Bevy upload the changed asset. For changed dimensions, replace the handle with `images.add(new_image)`.

Spawn one fullscreen movie node:

```rust
commands.spawn((
    MovieOverlay,
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    },
    ImageNode {
        image: texture,
        color: Color::WHITE.with_alpha(settings.movie_alpha),
        ..default()
    },
    ZIndex(-110),
));
```

Use image aspect-ratio sizing or a centered child node whose dimensions are calculated from viewport and frame aspect ratio. Do not stretch video.

- [ ] **Step 7: Integrate movie event playback**

On a due movie event:

- stop prior worker;
- find `avi_paths[event.asset_id]`;
- spawn `MovieWorker`;
- store event start time;
- set desired movie time from `BgaClock`;
- update texture with the newest due frame;
- report decoder errors once;
- hide the node when `movie_enabled` is false;
- resume at current chart time when re-enabled.

Drift correction threshold remains 100 ms, matching NX.

- [ ] **Step 8: Implement cleanup**

Export:

```rust
pub fn clear_visuals(
    mut commands: Commands,
    mut player: ResMut<BgaPlayer>,
    overlays: Query<Entity, Or<(With<BgaLayerOverlay>, With<MovieOverlay>)>>,
) {
    player.stop_movie();
    player.reset();
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}
```

Register it in `game-menu`'s existing `OnExit(AppState::Performance)` cleanup chain. Keep repeated calls safe.

- [ ] **Step 9: Run integration tests**

```sh
cargo test -p dtx-bga -- --nocapture
cargo test -p gameplay-drums --lib
cargo check -p game-menu
```

Expected: PASS. Decoder fixture opens, static BGA still works, and cleanup leaves no worker or overlay.

- [ ] **Step 10: Commit**

```sh
git add crates/dtx-bga crates/game-menu/src/song_loading.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(bga): sync fullscreen chart movies"
```

Commit body must cite `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:266-285` and `:319-332`.

---

### Task 7: Update crate contract and run final verification

**Files:**
- Modify: `crates/dtx-bga/AGENTS.md`
- Verify: all files changed in Tasks 1-6

**Interfaces:**
- Consumes: completed feature.
- Produces: accurate crate documentation and merge evidence.

- [ ] **Step 1: Update `dtx-bga/AGENTS.md`**

Replace placeholder/deferred M7 text with:

- real `#BMP` image rendering;
- `video-rs`/FFmpeg movie decoding;
- gameplay-clock synchronization;
- pause/seek/cleanup behavior;
- four config controls;
- deferred `BGAPAN`, `AVIPAN`, movie audio, and zero-copy decoding.

Keep reference file list unchanged.

- [ ] **Step 2: Run formatting**

```sh
cargo fmt --all -- --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, inspect the diff, then rerun the check.

- [ ] **Step 3: Run changed-package tests**

```sh
cargo test -p dtx-core
cargo test -p dtx-audio --lib
cargo test -p dtx-bga -- --nocapture
cargo test -p gameplay-drums --lib
cargo test -p game-menu --lib
```

Expected: PASS.

- [ ] **Step 4: Run workspace gates**

```sh
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS with no warnings.

- [ ] **Step 5: Run manual chart verification**

Start desktop:

```sh
cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking
```

Verify against a chart containing `#PREIMAGE`, `#BMP`, and `#AVI`:

- cover appears in top-right card;
- movie aspect-fits behind lanes and HUD;
- BGA Images and Chart Movie toggles apply live;
- both opacity sliders apply live;
- pause freezes frame;
- practice seek resumes correct movie time;
- missing visual files log once and gameplay continues.

Capture one screenshot showing cover and movie/BGA behind gameplay.

- [ ] **Step 6: Commit documentation**

```sh
git add crates/dtx-bga/AGENTS.md
git commit -m "docs(bga): document chart visual playback"
```

- [ ] **Step 7: Review branch diff**

```sh
git status --short
git diff --check main...HEAD
git diff --stat main...HEAD
git log --oneline main..HEAD
```

Expected: clean feature worktree, no whitespace errors, only chart-visual files and fixtures changed.

- [ ] **Step 8: Merge to main after review passes**

From the main worktree, preserve unrelated user changes. Confirm no changed main-worktree file would be overwritten, then run:

```sh
git merge --no-ff feat/chart-visuals
```

After merge:

```sh
git log -1 --oneline
git status --short
```

Expected: merge commit on `main`; pre-existing unrelated changes remain untouched.
