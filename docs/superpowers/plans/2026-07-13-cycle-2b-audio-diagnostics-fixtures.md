# Cycle 2B Audio, Diagnostics, and Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play MP3 chart audio, report scan/load problems honestly, and lock compatibility behavior with a representative fixture matrix.

**Architecture:** Enable the backend's existing MP3 loader and centralize supported-audio classification in `dtx-audio`. Extend library scans with a retained structured report and render only a compact summary in Song Select. Change chart-audio preloading to return asset identities plus missing/unsupported issues, allowing SongLoading to distinguish parser, media, and decoder failures while keeping optional media nonfatal.

**Tech Stack:** Rust 1.95, Bevy 0.19 AssetServer, bevy_kira_audio 0.26 MP3/Ogg/Wav loaders, ffmpeg-generated test fixtures, Cargo integration tests.

## Global Constraints

- Complete Cycle 0 and Cycle 2A before this plan.
- Preserve existing OGG/WAV priority and behavior.
- MP3 supports preview, explicit `#WAVxx`, `#BGMWAV`, fallback BGM, layers, and drum samples.
- XA is not decoded in this cycle; it must be reported as unsupported and never submitted to the decoder.
- Parse failure and zero playable drum chips are fatal.
- Missing, unsupported, or undecodable audio and missing visual media are visible nonfatal warnings.
- Detailed paths remain in logs; a full in-game problem browser is out of scope.
- Do not change CI/CD configuration or workflows.

---

## File map

- Modify: `Cargo.toml` — enable the existing `mp3` backend feature
- Modify if dependency resolution changes: `Cargo.lock`
- Modify: `crates/dtx-core/src/assets.rs` — MP3 fallback and case-insensitive nested resolution
- Modify: `crates/dtx-library/src/lib.rs` — MP3 preview/BGM docs and structured scan reports
- Modify: `crates/dtx-audio/src/lib.rs` — supported-format classifier
- Create: `crates/dtx-audio/tests/mp3_decode.rs` — real MP3 loader integration
- Create: `crates/dtx-core/tests/fixtures/compat-tone.mp3` — generated 250 ms tone
- Create: `crates/dtx-core/tests/fixtures/compat-tone.wav` — generated 250 ms tone
- Create: `crates/dtx-core/tests/fixtures/mp3_audio.dtx` — MP3 BGM/drum/SE fixture
- Modify: `crates/gameplay-drums/src/sound_bank.rs` — structured preload batch
- Modify: `crates/game-menu/src/song_loading.rs` — structured load diagnostics and readable delays
- Modify: `crates/game-menu/src/song_select.rs` — compact scan problem summary
- Modify: `crates/dtx-assets/src/lib.rs` — load parse reports from disk
- Create: `crates/dtx-core/tests/compatibility_matrix.rs` — encoding/media/malformed matrix
- Create: `crates/dtx-timing/tests/compatibility_timing.rs` — variable BPM/bar timing matrix

### Task 1: Enable and prove MP3 decoding and resolution

**Files:**

- Modify: `Cargo.toml:40-45`
- Modify: `Cargo.lock`
- Modify: `crates/dtx-audio/src/lib.rs`
- Modify: `crates/dtx-core/src/assets.rs:380-465`
- Modify: `crates/dtx-library/src/lib.rs:35-95`
- Create: `crates/dtx-audio/tests/mp3_decode.rs`
- Create: `crates/dtx-core/tests/fixtures/compat-tone.mp3`
- Create: `crates/dtx-core/tests/fixtures/compat-tone.wav`
- Create: `crates/dtx-core/tests/fixtures/mp3_audio.dtx`

**Interfaces:**

- Produces: `dtx_audio::AudioFormat::{Ogg, Wav, Mp3}`
- Produces: `dtx_audio::supported_audio_format(&Path) -> Option<AudioFormat>`
- Extends: `dtx_core::resolve_bgm_path` with MP3 candidates

- [ ] **Step 1: Generate deterministic audio fixtures**

Run:

```bash
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:duration=0.25 -ac 1 -ar 22050 -q:a 9 -map_metadata -1 -write_xing 0 -y crates/dtx-core/tests/fixtures/compat-tone.mp3
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:duration=0.25 -ac 1 -ar 22050 -map_metadata -1 -y crates/dtx-core/tests/fixtures/compat-tone.wav
```

Expected: both files exist and `file` identifies MPEG Layer III and RIFF/WAVE data respectively.

Create `mp3_audio.dtx` with:

```text
#TITLE: MP3 Compatibility
#ARTIST: dtxmaniars
#BPM: 120
#WAV01: compat-tone.mp3
#WAV02: compat-tone.mp3
#BGMWAV: 01
#00001: 01
#00113: 02
#00192: 02
```

- [ ] **Step 2: Write the failing MP3 asset-loader test**

Create `crates/dtx-audio/tests/mp3_decode.rs`:

```rust
use bevy::asset::{AssetApp, AssetPlugin, LoadState};
use bevy::prelude::*;
use bevy_kira_audio::source::mp3_loader::Mp3Loader;
use bevy_kira_audio::AudioSource;

#[test]
fn mp3_fixture_decodes_through_bevy_asset_loader() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<AudioSource>();
    app.register_asset_loader(Mp3Loader::default());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dtx-core/tests/fixtures/compat-tone.mp3");
    let handle: Handle<AudioSource> = app
        .world()
        .resource::<AssetServer>()
        .load_builder()
        .override_unapproved()
        .load(path.to_string_lossy().into_owned());

    for _ in 0..200 {
        app.update();
        match app.world().resource::<AssetServer>().get_load_state(handle.id()) {
            Some(LoadState::Loaded) => return,
            Some(LoadState::Failed(error)) => panic!("MP3 decode failed: {error:?}"),
            _ => std::thread::sleep(std::time::Duration::from_millis(1)),
        }
    }
    panic!("MP3 fixture did not reach a terminal load state");
}
```

- [ ] **Step 3: Run and observe the disabled-feature failure**

Run:

```bash
cargo test -p dtx-audio --test mp3_decode -- --nocapture
```

Expected: compile failure because `mp3_loader` is feature-gated.

- [ ] **Step 4: Enable MP3 and add format classification**

Change the workspace dependency to:

```toml
bevy_kira_audio = { version = "0.26", default-features = false, features = ["ogg", "wav", "mp3"] }
```

Add to `dtx-audio/src/lib.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Ogg,
    Wav,
    Mp3,
}

pub fn supported_audio_format(path: &Path) -> Option<AudioFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ogg" => Some(AudioFormat::Ogg),
        "wav" => Some(AudioFormat::Wav),
        "mp3" => Some(AudioFormat::Mp3),
        _ => None,
    }
}
```

Add unit assertions for lowercase, uppercase, mixed-case extensions, XA rejection, and extensionless rejection.

- [ ] **Step 5: Extend BGM and preview resolution without changing old priority**

In `resolve_bgm_path`, keep all current OGG/WAV candidates first, then add:

```rust
for name in ["drums.mp3", "bgm_d.mp3", "bgm.mp3", "1.mp3"] {
    if let Some(path) = resolve_chart_asset_path(parent, name) {
        return Some(path);
    }
}
```

Use `resolve_chart_asset_path` for explicit `#BGMWAV`, preview, and fallback candidates, and extend the stem loop to `&["ogg", "wav", "mp3"]`. Enhance `resolve_chart_asset_path` to normalize `\` to `/` and resolve every nested component case-insensitively, returning `None` if any component is missing.

In `SongInfo::from_chart`, resolve `#PREVIEW` case-insensitively and accept only OGG/WAV/MP3 extensions; missing or unsupported previews fall back to the resolved BGM. Update comments to list MP3.

- [ ] **Step 6: Run MP3 and resolver tests**

Run:

```bash
cargo test -p dtx-audio --test mp3_decode -- --nocapture
cargo test -p dtx-audio supported_audio_format -- --nocapture
cargo test -p dtx-core assets::tests -- --nocapture
cargo test -p dtx-library --test bgm_preview -- --nocapture
```

Expected: the real MP3 reaches `LoadState::Loaded`; OGG/WAV tests remain unchanged; MP3 fallback and nested mixed-case resolution pass.

- [ ] **Step 7: Commit MP3 support**

```bash
git add Cargo.toml Cargo.lock crates/dtx-audio/src/lib.rs crates/dtx-audio/tests/mp3_decode.rs crates/dtx-core/src/assets.rs crates/dtx-core/tests/fixtures/compat-tone.mp3 crates/dtx-core/tests/fixtures/compat-tone.wav crates/dtx-core/tests/fixtures/mp3_audio.dtx crates/dtx-library/src/lib.rs
git commit -m "feat: support MP3 chart audio"
```

### Task 2: Retain structured library scan reports and show a compact summary

**Files:**

- Modify: `crates/dtx-library/src/lib.rs:130-245`
- Modify: `crates/game-menu/src/song_select.rs:450-600,760-850,1620-1650`

**Interfaces:**

- Produces: `ScanProblemKind::{Open, Parse, ParserWarning, MissingPreview, UnsupportedPreview}`
- Produces: `ScanProblem { path, line, kind, detail }`
- Produces: `ScanReport { elapsed, discovered, loaded, problems }`
- Changes: `scan_directory(&Path) -> Result<(Vec<SongInfo>, ScanReport), ScanError>`
- Extends: `SongDb::latest_scan: ScanReport`

- [ ] **Step 1: Add failing scan-report tests**

Add tests that build a temp directory with one valid `.DTX`, one invalid-BPM `.dtx`, one valid chart with a malformed conditional warning, and one chart with `#PREVIEW: clip.xa`. Assert discovered/loaded/skipped counts, problem kinds/paths, nonzero-or-valid elapsed duration, and that `SongDb::rescan` retains the report.

- [ ] **Step 2: Run the focused test and observe missing report types**

Run:

```bash
cargo test -p dtx-library scan_report -- --nocapture
```

Expected: compile failure because the report types and field do not exist.

- [ ] **Step 3: Implement plain report data and aggregation**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanProblemKind {
    Open,
    Parse,
    ParserWarning,
    MissingPreview,
    UnsupportedPreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProblem {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub kind: ScanProblemKind,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub elapsed: std::time::Duration,
    pub discovered: usize,
    pub loaded: usize,
    pub problems: Vec<ScanProblem>,
}

impl ScanReport {
    pub fn skipped(&self) -> usize {
        self.discovered.saturating_sub(self.loaded)
    }
}
```

Measure the outer scan with `Instant::now()`. Increment `discovered` for every case-insensitive DTX path. Parse through `dtx_core::parse_with_options`; append every parser warning with its line and debug/detail text. Inspect explicit `#PREVIEW` paths before building `SongInfo`: record `UnsupportedPreview` for extensions other than OGG/WAV/MP3, record `MissingPreview` when the resolved file does not exist, and let `SongInfo` fall back to BGM in both cases. Convert file-open and parse errors into problems and continue. Log every problem with its full path. Return only directory-level read failures as `ScanError`.

Change `SongDb::rescan` to sort returned songs, assign `latest_scan`, and preserve the existing `Result<(), ScanError>` API.

- [ ] **Step 4: Render and update only the compact Song Select summary**

Add a `ScanProblemSummary` marker and pure formatter:

```rust
fn scan_problem_summary(report: &dtx_library::ScanReport) -> Option<String> {
    if report.skipped() > 0 {
        Some(format!("{} charts skipped — see log", report.skipped()))
    } else if !report.problems.is_empty() {
        Some(format!("{} chart warnings — see log", report.problems.len()))
    } else {
        None
    }
}
```

Spawn the summary as small amber text beneath the search/sort top bar. Add an update system that reacts to `SongDb` changes after startup, F5 rescan, or import rescan; hide the node when the formatter returns `None`. Add pure formatter tests for clean, skipped, and warning-only reports.

- [ ] **Step 5: Run library and Song Select tests**

Run:

```bash
cargo test -p dtx-library -- --nocapture
cargo test -p game-menu scan_problem_summary -- --nocapture
```

Expected: scan metrics/problems and all three UI strings pass; detailed paths appear only in logs.

- [ ] **Step 6: Commit scan diagnostics**

```bash
git add crates/dtx-library/src/lib.rs crates/game-menu/src/song_select.rs
git commit -m "feat: report chart scan problems"
```

### Task 3: Return structured chart-audio preload outcomes

**Files:**

- Modify: `crates/gameplay-drums/src/sound_bank.rs`
- Test: `crates/gameplay-drums/src/sound_bank.rs`

**Interfaces:**

- Produces: `PreloadIssueKind::{Missing, Unsupported}`
- Produces: `PreloadIssue { slot, path, kind }`
- Produces: `PreloadedAudio { slot, path, handle }`
- Produces: `PreloadBatch { assets, issues }`
- Produces: `preload_slots_report(...) -> PreloadBatch`
- Preserves temporarily: `preload_slots(...) -> Vec<Handle<KiraAudioSource>>`

- [ ] **Step 1: Add failing pure preflight tests**

Extract and test a pure preflight helper using a temp directory. Cover: existing `.mp3` returns a load candidate, missing `.ogg` returns `Missing`, existing `.xa` returns `Unsupported`, and nested mixed-case `.WAV` resolves successfully.

- [ ] **Step 2: Run and observe missing outcome types**

Run:

```bash
cargo test -p gameplay-drums --lib sound_bank::tests::preflight -- --nocapture
```

Expected: compile failure because the preflight/outcome types do not exist.

- [ ] **Step 3: Implement structured preloading**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadIssueKind { Missing, Unsupported }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadIssue {
    pub slot: u32,
    pub path: std::path::PathBuf,
    pub kind: PreloadIssueKind,
}

#[derive(Debug, Clone)]
pub struct PreloadedAudio {
    pub slot: u32,
    pub path: std::path::PathBuf,
    pub handle: Handle<KiraAudioSource>,
}

#[derive(Debug, Default)]
pub struct PreloadBatch {
    pub assets: Vec<PreloadedAudio>,
    pub issues: Vec<PreloadIssue>,
}
```

For each requested slot, resolve the path, reject nonexistent files as `Missing`, reject `supported_audio_format(path).is_none()` as `Unsupported`, and call `preload_chart_sound` only for supported existing files. Implement this as `preload_slots_report`. Keep `preload_slots` as a compatibility wrapper that returns only report asset handles so the workspace compiles between Tasks 3 and 4. Update `preload_chart_sounds` to consume the report, return `batch.assets.len()`, and log every safety-net issue on Performance entry.

- [ ] **Step 4: Update SE32 and existing preload tests**

Keep existing slot-collection tests. Add a chart using SE32/MP3 and assert it enters `assets`; add XA and missing files and assert neither gets an AssetServer handle.

- [ ] **Step 5: Run sound-bank tests and commit**

Run:

```bash
cargo test -p gameplay-drums --lib sound_bank -- --nocapture
```

Expected: all collection/preflight/batch tests pass.

```bash
git add crates/gameplay-drums/src/sound_bank.rs
git commit -m "refactor: report chart audio preload issues"
```

### Task 4: Distinguish fatal and nonfatal SongLoading problems

**Files:**

- Modify: `crates/dtx-assets/src/lib.rs`
- Modify: `crates/game-menu/src/song_loading.rs`

**Interfaces:**

- Produces: `dtx_assets::load_dtx_report(&Path) -> Result<ParseReport, LoadError>`
- Produces: `LoadProblemKind::{ParserWarning, MissingAudio, UnsupportedAudio, DecoderFailure, MissingVisual}`
- Produces: `LoadProblem`, `LoadDiagnostics`, and delayed transition timestamps
- Consumes: `PreloadBatch` from Task 3

- [ ] **Step 1: Add failing load-policy tests**

Add pure tests for:

```rust
assert!(load_failure_for(&Chart::default()).is_some());
assert!(load_failure_for(&chart_with_one_drum_chip()).is_none());
assert_eq!(failure_hold_seconds(), 2.5);
assert_eq!(warning_hold_seconds(&[one_warning]), 0.75);
assert_eq!(warning_hold_seconds(&[]), 0.0);
```

Add a status formatter test asserting fatal text includes the actual parse/no-chip reason and warning text includes the warning count.

- [ ] **Step 2: Run and observe missing policy functions/types**

Run:

```bash
cargo test -p game-menu song_loading::tests::load_ -- --nocapture
```

Expected: compile failure for the missing diagnostics/policy interfaces.

- [ ] **Step 3: Preserve parser warnings through dtx-assets**

Add `load_dtx_report` that opens the file and calls `dtx_core::parse_with_options`. Implement existing `load_dtx` as:

```rust
pub fn load_dtx(path: &Path) -> Result<Chart, LoadError> {
    load_dtx_report(path).map(|report| report.chart)
}
```

Change `ChartParseTask` to `Task<Result<dtx_core::ParseReport, String>>` and call `load_dtx_report`.

- [ ] **Step 4: Add structured diagnostics and readable transition delays**

Add plain data:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadProblemKind { ParserWarning, MissingAudio, UnsupportedAudio, DecoderFailure, MissingVisual }

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadProblem {
    kind: LoadProblemKind,
    path: Option<std::path::PathBuf>,
    detail: String,
}

#[derive(Resource, Debug, Default)]
struct LoadDiagnostics {
    fatal: Option<String>,
    warnings: Vec<LoadProblem>,
}

#[derive(Resource, Debug, Default)]
struct AdvanceNotBefore(f64);
```

Reset both resources in `start_load`. On parse success, convert parser warnings, then reject a chart with `chart.drum_chips().next().is_none()` using fatal text `selected conditional branch contains no playable drum chips` and set the return deadline to `now + 2.5` seconds.

On the parse task's `Err(error)` branch, store the complete error string in
`LoadDiagnostics::fatal`, enter `LoadPhase::Failed`, and set the same `now +
2.5` deadline. Cancellation remains immediate and does not masquerade as a
parse failure.

Call `preload_slots_report` and convert `PreloadIssueKind::Missing/Unsupported` into load warnings. Store `PreloadedAudio` path/slot/handle in `RequiredAudio`; do not submit XA. Compare `ActiveChartRes`'s resolved BMP/AVI maps with referenced registry IDs and append `MissingVisual` warnings for unresolved optional media. After migrating SongLoading, remove the temporary `preload_slots` wrapper and update any remaining gameplay caller to the report API in the same commit.

During `wait_for_audio`, inspect each handle once. Loaded handles become terminal; failed handles append one `DecoderFailure` with the asset path and become terminal. When all handles are terminal, enter Ready and set `AdvanceNotBefore` to `now + 0.75` only if warnings exist.

In `advance_when_loaded`, refuse transitions until `Time::elapsed_secs_f64()` reaches the deadline. Fatal then returns to Song Select; Ready proceeds to Performance. Update the visible status to show the fatal reason or `ready — N media warnings; continuing…`.

- [ ] **Step 5: Run SongLoading and asset tests**

Run:

```bash
cargo test -p dtx-assets -- --nocapture
cargo test -p game-menu song_loading -- --nocapture
cargo check -p game-menu
```

Expected: parse reports retain warnings; no-chip is fatal; missing/XA/decoder/visual issues are nonfatal and individually classified; status/delay tests pass.

- [ ] **Step 6: Commit loading diagnostics**

```bash
git add crates/dtx-assets/src/lib.rs crates/game-menu/src/song_loading.rs crates/gameplay-drums/src/sound_bank.rs
git commit -m "feat: surface chart loading diagnostics"
```

### Task 5: Add the compatibility matrix integration test

**Files:**

- Create: `crates/dtx-core/tests/compatibility_matrix.rs`
- Create: `crates/dtx-timing/tests/compatibility_timing.rs`
- Reuse: `crates/dtx-core/tests/fixtures/conditional_branches.dtx`
- Reuse: `crates/dtx-core/tests/fixtures/conditional_nested.dtx`
- Reuse: `crates/dtx-core/tests/fixtures/mp3_audio.dtx`
- Reuse: `crates/dtx-core/tests/fixtures/real_chart.ogg`
- Reuse: `crates/dtx-core/tests/fixtures/compat-tone.wav`

**Interfaces:**

- Consumes: all Cycle 2A/2B compatibility interfaces
- Produces: named matrices covering encodings, conditionals, timing, formats, high SE, missing assets, and malformed input

- [ ] **Step 1: Create table-driven parser cases**

In `compatibility_matrix.rs`, define named byte cases for UTF-8, Shift-JIS (`encoding_rs::SHIFT_JIS.encode`), UTF-16LE BOM, and UTF-16BE BOM. Each contains a Japanese/non-ASCII title plus one drum chip; parse and assert title/chip preservation.

Add tests that:

- parse both conditional fixtures with explicit seeds and assert one branch;
- parse `mp3_audio.dtx` and assert BGM, drum, and SE32 reference the MP3 slot;
- resolve `.ogg`, `.wav`, and `.mp3` fixture paths;
- parse missing-asset references without dropping chips;
- parse malformed conditional input into warnings without panic.

In `crates/dtx-timing/tests/compatibility_timing.rs`, use this exact test:

```rust
#[test]
fn parsed_bpm_and_bar_changes_keep_expected_timeline() {
    let chart = dtx_core::parse_str(
        "#BPM: 120\n#BPM01: 240\n#00002: 0.5\n#00008: 01\n\
         #00013: 01\n#00113: 01\n#00213: 01\n",
    )
    .expect("timing fixture parses");
    let bpm = dtx_core::timing::bpm_changes_from_chart(&chart);
    let bars = dtx_core::timing::bar_changes_from_chart(&chart);
    let timing = dtx_timing::math::ChartTiming {
        bpm_changes: &bpm,
        bar_changes: &bars,
    };
    let times: Vec<_> = chart
        .drum_chips()
        .map(|chip| dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(
            chip.measure,
            chip.value,
            120.0,
            timing,
        ))
        .collect();
    assert_eq!(times, vec![2_000, 2_500, 3_000]);
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
}
```

The expected values include the parser's NX-compatible leading empty measure:
2,000 ms at 120 BPM, followed by half-length measures at 240 BPM (500 ms each).
Keeping this test in `dtx-timing` also verifies its public math re-export.

- [ ] **Step 2: Run the matrix and fix only compatibility defects it exposes**

Run:

```bash
cargo test -p dtx-core --test compatibility_matrix -- --nocapture
cargo test -p dtx-timing --test compatibility_timing -- --nocapture
```

Expected: every named matrix case passes. Any failure must be fixed in the owning Task 1-4 file and covered by the failing case before proceeding.

- [ ] **Step 3: Commit the matrix**

```bash
git add crates/dtx-core/tests/compatibility_matrix.rs crates/dtx-timing/tests/compatibility_timing.rs
git commit -m "test: add DTX compatibility matrix"
```

### Task 6: Run the combined Cycles 0-2 gate

**Files:**

- Test: all affected crates and workspace

**Interfaces:**

- Consumes: Cycle 0, Cycle 1, Cycle 2A, and Tasks 1-5
- Produces: fully verified first program increment

- [ ] **Step 1: Run focused packages**

Run:

```bash
cargo test -p dtx-core
cargo test -p dtx-assets
cargo test -p dtx-library
cargo test -p dtx-audio
cargo test -p gameplay-drums
cargo test -p game-shell
cargo test -p game-results
cargo test -p game-menu
```

Expected: every package exits 0.

- [ ] **Step 2: Run workspace release gates**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
```

Expected: every command exits 0.

- [ ] **Step 3: Audit final scope and behavior**

Run:

```bash
git diff --check
git status --short
git log --oneline --decorate -15
```

Expected: no uncommitted changes, logical commits for every task, no `references/` changes, and no CI/CD changes.
