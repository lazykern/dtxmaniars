# Cycle 7 Extended DTX and Media Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make supported DTX, GDA, and G2D drum charts faithful through discovery, parsing, loading, play, seek/restart, media diagnostics, and documentation while explicitly rejecting BMS/BME and unrecoverable XA charts.

**Architecture:** `dtx-core` records source provenance and normalizes every playable format into one `Chart`; format-specific syntax stops at parser front ends. The library and asset loader classify formats explicitly and carry structured diagnostics into Song Loading. Gameplay consumes hidden/system/mixer events without treating them as notes, while `dtx-bga` reconstructs deterministic visual state from typed swap/pan events.

**Tech Stack:** Rust 1.95+, Bevy 0.19, existing `encoding_rs`, `dtx-core`, `dtx-library`, `dtx-assets`, `dtx-audio`, `dtx-bga`, `dtx-timing`, `game-menu`, and `gameplay-drums`.

## Global Constraints

- Product scope is drums; guitar/bass expansion and keyboard-oriented BMS/BME gameplay are excluded.
- Supported means discovery, parse, normalization, required media, play, seek/restart, and diagnostics all pass a fixture contract.
- Unknown required gameplay structures reject before Performance; optional failures are Degraded with Warning and name recovery.
- `.dtx`, `.gda`, and `.g2d` discovery is case-insensitive.
- `.bms` and `.bme` are detected and explicitly rejected; they never appear as zero-note playable songs.
- Hidden drum chips are timed, invisible, non-judgeable, and excluded from note count, combo, gauge, score, density, and analysis.
- MIDI chorus is a timed no-op; system events never enter judgment routing.
- XA decoding, native libraries, vendored converters, and automatic conversion are excluded.
- XA fallback order is same-stem OGG, WAV, then MP3, case-insensitively.
- Pitch-preserving time stretch is not exposed; current rate changes continue to change pitch.
- `references/` is read-only and every NX-derived behavior cites `references/DTXmaniaNX/`.
- Do not add a song cache, folder tree, unsafe code, or CI/CD changes.

---

## File structure

- Create `crates/dtx-core/src/format.rs`: source/rejected format classification and normalized load entry point.
- Create `crates/dtx-core/src/legacy_gda.rs`: GDA/G2D channel-name front end.
- Create `crates/dtx-core/src/diagnostic.rs`: structured parser compatibility warnings.
- Modify `crates/dtx-core/src/chart.rs`, `parser.rs`, `channel.rs`, `assets.rs`, `bga.rs`, and `lib.rs`: provenance, normalized levels/aliases, channels, and pan definitions.
- Add `crates/dtx-core/tests/fixtures/compatibility/`: paired DTX/GDA/G2D, malformed, hidden/system, pan/swap, and level fixtures.
- Modify `crates/dtx-core/tests/compatibility_matrix.rs`: end-to-end parser matrix.
- Modify `crates/dtx-library/src/lib.rs` and `import.rs`: format discovery/rejection and archive reporting.
- Modify `crates/dtx-library/tests/import.rs`: mixed-format archive outcomes.
- Modify `crates/dtx-assets/src/lib.rs`: path-aware format load API.
- Modify `crates/game-menu/src/song_loading.rs`: supported/degraded/rejected loading contract.
- Create `crates/gameplay-drums/src/system_events.rs`: hidden/no-op/click/first-sound consumption.
- Create `crates/gameplay-drums/src/mixer_events.rs`: mixer eligibility and seek rebuild.
- Modify `crates/gameplay-drums/src/sound_bank.rs`, `timeline.rs`, `lanes.rs`, `score.rs`, `results_analysis.rs`, `seek.rs`, and `lib.rs`: exclusion/routing/audio recovery.
- Modify `crates/dtx-bga/src/chart.rs` and `lib.rs`: typed replace/swap/pan events and deterministic reconstruction.
- Modify `crates/dtx-bga/tests/integration_bga.rs`: start/mid/end/seek geometry assertions.
- Create or modify `docs/compatibility.md`: exact Supported / Degraded with Warning / Rejected with Recovery guide.

## Reference evidence to read before implementation

- `references/DTXmaniaNX/DTXMania/Score,Song/EChannel.cs`: exact channel values.
- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1198-1232`: GDA/G2D conversion table.
- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:4788-4884`: metadata aliases and level/decimal processing.
- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:5149-5459` and `:5679-5989`: AVIPAN/BGAPAN argument parsing.
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:3084-3122`: hidden drums and MIDI chorus consumption.
- `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:3358-3365`: BGA swaps.
- `references/DTXmaniaNX/FDK/Sound/Cxa.cs`: native XA dependency that is deliberately not ported.

### Task 1: Add chart provenance, normalized drum levels, aliases, and diagnostics

**Files:**
- Create: `crates/dtx-core/src/format.rs`
- Create: `crates/dtx-core/src/diagnostic.rs`
- Modify: `crates/dtx-core/src/chart.rs`
- Modify: `crates/dtx-core/src/parser.rs`
- Modify: `crates/dtx-core/src/lib.rs`
- Modify: `crates/dtx-core/tests/parser_tests.rs`
- Modify: `crates/dtx-core/tests/compatibility_matrix.rs`

**Interfaces:**
- Consumes: existing DTX decode/conditional parser and `ParseOptions`.
- Produces: `ChartFormat`, `ChartLevel`, `DiagnosticKind`, `ChartDiagnostic`, `parse_source`, and explicit metadata alias fields.

- [ ] **Step 1: Write failing normalization tests**

```rust
#[test]
fn playlevel_packed_level_and_dlvdec_follow_nx_order() {
    let chart = parse_str("#PLAYLEVEL: 355\n#DLVDEC: 7\n#HIDDENLEVEL: ON\n#WALL: wall.png\n").unwrap();
    assert_eq!(chart.format, ChartFormat::Dtx);
    assert_eq!(chart.metadata.drum_level, Some(ChartLevel { tenths: 35, hundredths: 7 }));
    assert!((chart.metadata.drum_level.unwrap().display() - 3.57).abs() < 0.001);
    assert!(chart.metadata.hidden_level);
    assert_eq!(chart.metadata.background.as_deref(), Some("wall.png"));
}

#[test]
fn packed_levels_clamp_exactly_like_nx() {
    assert_eq!(ChartLevel::from_raw(1_999), ChartLevel { tenths: 100, hundredths: 0 });
    assert_eq!(ChartLevel::from_raw(77), ChartLevel { tenths: 77, hundredths: 0 });
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core --lib`

Expected: FAIL because provenance, normalized level, and aliases are absent.

- [ ] **Step 3: Implement the core model and path-independent parser API**

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChartFormat { #[default] Dtx, Gda, G2d }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartLevel { pub tenths: u16, pub hundredths: u8 }

pub struct ChartDiagnostic {
    pub line: Option<usize>,
    pub kind: DiagnosticKind,
    pub detail: String,
    pub recovery: Option<String>,
}

pub enum DiagnosticKind {
    Conditional,
    UnknownOptional,
    UnsupportedChannel,
    MalformedVisual,
}

impl ChartLevel {
    pub fn from_raw(raw: u32) -> Self {
        let raw = raw.clamp(0, 1000) as u16;
        if raw >= 100 {
            Self { tenths: raw / 10, hundredths: (raw % 10) as u8 }
        } else {
            Self { tenths: raw, hundredths: 0 }
        }
    }
    pub fn with_decimal(mut self, value: i32) -> Self {
        self.hundredths = value.clamp(0, 10) as u8;
        self
    }
    pub fn display(self) -> f32 { self.tenths as f32 / 10.0 + self.hundredths as f32 / 100.0 }
}
```

Replace raw `Metadata::dlevel` consumption with `drum_level`; parse DLEVEL and PLAYLEVEL identically and apply DLVDEC in input order. Retain guitar/bass raw/decimal directives only as provenance fields. Add `hidden_level`, `background`, and `background_gr`; WALL writes `background`. `Chart` gets `format`. `ParseReport` carries `Vec<ChartDiagnostic>`, converting current conditional warnings without losing line/kind/detail and retaining unknown optional directives with recovery text. Existing `parse` and `parse_str` remain DTX convenience APIs; `parse_source(reader, format, options)` is the shared entry point.

- [ ] **Step 4: Migrate level consumers and commit**

Update Song Select, loading, Results, skill calculation, and persistence callers to use `ChartLevel::display()`. Verify canonical identity ignores source format and alias spelling when normalized gameplay is equal.

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core -p dtx-library -p game-menu -p game-results`

Expected: PASS.

```bash
git add crates/dtx-core/src/format.rs crates/dtx-core/src/diagnostic.rs crates/dtx-core/src/chart.rs crates/dtx-core/src/parser.rs crates/dtx-core/src/lib.rs crates/dtx-core/tests/parser_tests.rs crates/dtx-core/tests/compatibility_matrix.rs crates/dtx-library/src/lib.rs crates/game-menu/src crates/game-results/src
git commit -m "feat: normalize chart format and drum levels"
```

### Task 2: Normalize GDA and G2D through a dedicated front end

**Files:**
- Create: `crates/dtx-core/src/legacy_gda.rs`
- Modify: `crates/dtx-core/src/format.rs`
- Modify: `crates/dtx-core/src/parser.rs`
- Modify: `crates/dtx-core/src/lib.rs`
- Create: `crates/dtx-core/tests/fixtures/compatibility/equivalent.dtx`
- Create: `crates/dtx-core/tests/fixtures/compatibility/equivalent.gda`
- Create: `crates/dtx-core/tests/fixtures/compatibility/equivalent.g2d`
- Create: `crates/dtx-core/tests/fixtures/compatibility/malformed.gda`
- Modify: `crates/dtx-core/tests/compatibility_matrix.rs`

**Interfaces:**
- Consumes: `parse_source`, decoded active lines, and the existing DTX chip parser.
- Produces: `normalize_gda_head(&str) -> Result<Option<EChannel>, LegacyChannelError>` and equivalent normalized charts.

- [ ] **Step 1: Write failing paired-fixture tests**

```rust
#[test]
fn dtx_gda_and_g2d_normalize_to_equal_drum_gameplay() {
    let dtx = load_fixture("equivalent.dtx", ChartFormat::Dtx);
    let gda = load_fixture("equivalent.gda", ChartFormat::Gda);
    let g2d = load_fixture("equivalent.g2d", ChartFormat::G2d);
    assert_eq!(gameplay_signature(&dtx), gameplay_signature(&gda));
    assert_eq!(gameplay_signature(&dtx), gameplay_signature(&g2d));
    assert_eq!(dtx.drum_chips().count(), gda.drum_chips().count());
}

#[test]
fn malformed_gda_channel_has_line_diagnostic() {
    let report = load_fixture_report("malformed.gda", ChartFormat::Gda);
    assert!(report.diagnostics.iter().any(|d| d.line == Some(4) && d.kind == DiagnosticKind::UnsupportedChannel));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core gda`

Expected: FAIL because the legacy front end and fixtures are absent.

- [ ] **Step 3: Implement the exact drums-relevant NX conversion table**

```rust
pub fn normalize_gda_head(head: &str) -> Result<Option<EChannel>, LegacyChannelError> {
    let upper = head.to_ascii_uppercase();
    if let Some(channel) = legacy_se_channel(&upper) { return Ok(Some(channel)); }
    Ok(Some(match upper.as_str() {
        "TC" => EChannel::BPM,
        "BL" => EChannel::BarLength,
        "FI" => EChannel::FillIn,
        "HH" => EChannel::HiHatClose,
        "SD" => EChannel::Snare,
        "BD" => EChannel::BassDrum,
        "HT" => EChannel::HighTom,
        "LT" => EChannel::LowTom,
        "CY" => EChannel::Cymbal,
        "GS" | "DS" => return Ok(None),
        other => return Err(LegacyChannelError::Unsupported(other.to_owned())),
    }))
}
```

Map legacy SE strings `01` through `20` to SE01 through SE32. Keep guitar/bass channel syntax as a structured unsupported warning in the drums product rather than spreading format branches into gameplay. Reuse common metadata, encoding, conditional, asset, BPM, bar-length, and chip-data parsing after head normalization.

- [ ] **Step 4: Verify encoding/timing/media equivalence and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core --test compatibility_matrix`

Expected: PASS for lanes, BGM, BPM/bar changes, case-insensitive assets, UTF-8, Shift-JIS, malformed input, and DTX equivalence.

```bash
git add crates/dtx-core/src/legacy_gda.rs crates/dtx-core/src/format.rs crates/dtx-core/src/parser.rs crates/dtx-core/src/lib.rs crates/dtx-core/tests/fixtures/compatibility crates/dtx-core/tests/compatibility_matrix.rs
git commit -m "feat: normalize gda and g2d drum charts"
```

### Task 3: Classify discovery, import, and load outcomes

**Files:**
- Modify: `crates/dtx-library/src/lib.rs`
- Modify: `crates/dtx-library/src/import.rs`
- Modify: `crates/dtx-library/tests/import.rs`
- Modify: `crates/dtx-assets/src/lib.rs`
- Modify: `crates/game-menu/src/song_loading.rs`

**Interfaces:**
- Consumes: `ChartFormat`, `parse_source`, current scan/load reports, and archive extraction.
- Produces: `classify_chart_path`, `RejectedChartFormat`, `load_chart_report`, and `LoadSupport`.

- [ ] **Step 1: Write failing discovery and rejection tests**

```rust
#[test]
fn scanner_discovers_supported_legacy_formats_and_rejects_bms_bme() {
    assert_eq!(classify_chart_path(Path::new("A.GDA")), ChartPathKind::Playable(ChartFormat::Gda));
    assert_eq!(classify_chart_path(Path::new("B.g2d")), ChartPathKind::Playable(ChartFormat::G2d));
    assert_eq!(classify_chart_path(Path::new("keys.BMS")), ChartPathKind::Rejected(RejectedChartFormat::Bms));
    assert_eq!(classify_chart_path(Path::new("keys.bme")), ChartPathKind::Rejected(RejectedChartFormat::Bme));
}

#[test]
fn bms_is_reported_but_never_inserted_as_a_song() {
    let (songs, report) = scan_directory(&mixed_fixture_dir()).unwrap();
    assert!(songs.iter().all(|song| song.path.extension().unwrap() != "bms"));
    assert!(report.problems.iter().any(|p| p.detail.contains("BMS/BME is not supported by the drums player")));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-library -p dtx-assets`

Expected: FAIL because only DTX is classified.

- [ ] **Step 3: Implement one path-aware classification and load route**

```rust
pub enum ChartPathKind {
    Playable(ChartFormat),
    Rejected(RejectedChartFormat),
    NotAChart,
}

pub enum LoadSupport {
    Supported,
    Degraded { problems: Vec<LoadProblem> },
    Rejected { problems: Vec<LoadProblem> },
}

pub fn load_chart_report(path: &Path) -> Result<ParseReport, LoadError> {
    let format = classify_playable_path(path).ok_or_else(|| LoadError::UnsupportedFormat(path.to_path_buf()))?;
    let file = fs::File::open(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_source(file, format, ParseOptions::default()).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}
```

Replace `is_dtx_path`, `load_dtx_report`, and archive `.dtx` counts with the classifier while retaining compatibility wrappers for callers that truly require DTX. Scan reports include rejected BMS/BME problems but `loaded` counts playable charts only. Archive import succeeds when at least one playable chart exists and returns per-format counts plus rejected chart diagnostics. Song Loading displays Supported, Degraded with Warning, or Rejected with Recovery using the exact product copy. It groups duplicate player-facing problems by kind/path while logging every original diagnostic with path, line, kind, detail, and recovery.

- [ ] **Step 4: Verify scan/import/loading paths and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-library -p dtx-assets -p game-menu format`

Expected: PASS, including uppercase extensions and BMS/BME archive reporting.

```bash
git add crates/dtx-library/src/lib.rs crates/dtx-library/src/import.rs crates/dtx-library/tests/import.rs crates/dtx-assets/src/lib.rs crates/game-menu/src/song_loading.rs
git commit -m "feat: classify supported and rejected chart formats"
```

### Task 4: Port hidden and remaining system channels without scoring pollution

**Files:**
- Modify: `crates/dtx-core/src/channel.rs`
- Modify: `crates/dtx-core/src/chart.rs`
- Modify: `crates/dtx-core/src/parser.rs`
- Create: `crates/dtx-core/tests/fixtures/compatibility/system_channels.dtx`
- Modify: `crates/dtx-core/tests/compatibility_matrix.rs`
- Create: `crates/gameplay-drums/src/system_events.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/lanes.rs`
- Modify: `crates/gameplay-drums/src/timeline.rs`
- Modify: `crates/gameplay-drums/src/score.rs`
- Modify: `crates/gameplay-drums/src/results_analysis.rs`
- Modify: `crates/gameplay-drums/src/hit_sound.rs`

**Interfaces:**
- Consumes: parsed chips, chart timing, lane/sound mapping, and gameplay statistics.
- Produces: hidden channels 0x31–0x3C, `MIDIChorus`, `FillIn`, `Click`, `FirstSoundChip`, and classification helpers.

- [ ] **Step 1: Write failing channel and gameplay-exclusion tests**

```rust
#[test]
fn hidden_channels_map_to_sound_lanes_but_are_not_notes() {
    assert_eq!(EChannel::from_byte(0x31), Some(EChannel::HiHatCloseHidden));
    assert_eq!(EChannel::HiHatCloseHidden.hidden_sound_lane(), Some(EChannel::HiHatClose));
    assert!(EChannel::HiHatCloseHidden.is_hidden_drum());
    assert!(!EChannel::HiHatCloseHidden.is_drum());
}

#[test]
fn system_fixture_never_changes_scoring_totals() {
    let chart = system_fixture();
    assert_eq!(chart.drum_chips().count(), 1);
    let totals = gameplay_totals(&chart);
    assert_eq!((totals.notes, totals.combo, totals.analysis_events), (1, 1, 1));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core -p gameplay-drums`

Expected: FAIL because the channels and system-event consumer are absent.

- [ ] **Step 3: Add exact channel values and a system-event scheduler**

Add 0x31–0x3C hidden drums, 0x52 MIDIChorus, 0x53 FillIn, 0xEC Click, and 0xED FirstSoundChip to `EChannel`. Keep existing 0x1F DrumsFillin distinct. `system_events` advances a cursor in chart-time order: hidden chips are never spawned/judged and expire without a miss; when needed for empty-hit/sound state they update the matching visible lane's sound template. MIDIChorus is consume-only. FillIn retains section-marker behavior. Click/FirstSound are timed system sounds and never enter lane routing.

- [ ] **Step 4: Verify every statistics surface and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core -p gameplay-drums`

Expected: PASS for note count, combo, gauge, score, density, weakest-lane/section analysis, sound state, and seek expiration.

```bash
git add crates/dtx-core/src/channel.rs crates/dtx-core/src/chart.rs crates/dtx-core/src/parser.rs crates/dtx-core/tests/fixtures/compatibility/system_channels.dtx crates/dtx-core/tests/compatibility_matrix.rs crates/gameplay-drums/src/system_events.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/lanes.rs crates/gameplay-drums/src/timeline.rs crates/gameplay-drums/src/score.rs crates/gameplay-drums/src/results_analysis.rs crates/gameplay-drums/src/hit_sound.rs
git commit -m "feat: consume hidden and system chart events"
```

### Task 5: Implement mixer add/remove lifetime and seek reconstruction

**Files:**
- Modify: `crates/dtx-core/src/channel.rs`
- Create: `crates/gameplay-drums/src/mixer_events.rs`
- Modify: `crates/gameplay-drums/src/lib.rs`
- Modify: `crates/gameplay-drums/src/sound_bank.rs`
- Modify: `crates/gameplay-drums/src/bgm_scheduler.rs`
- Modify: `crates/gameplay-drums/src/se_scheduler.rs`
- Modify: `crates/gameplay-drums/src/hit_sound.rs`
- Modify: `crates/gameplay-drums/src/seek.rs`

**Interfaces:**
- Consumes: mixer add/remove chips (0xEE/0xEF), `ChartSoundBank`, and chart/practice seek time.
- Produces: `MixerEligibility`, `MixerEventCursor`, `rebuild_mixer_at`, and `is_slot_eligible`.

- [ ] **Step 1: Write failing pure mixer tests**

```rust
#[test]
fn repeated_add_remove_is_idempotent_and_seek_rebuilds_state() {
    let events = [add(1_000, 5), add(1_000, 5), remove(2_000, 5), add(3_000, 7)];
    let at_1500 = rebuild_mixer_at(&events, 1_500);
    assert!(at_1500.is_slot_eligible(5));
    let at_2500 = rebuild_mixer_at(&events, 2_500);
    assert!(!at_2500.is_slot_eligible(5));
    assert!(!at_2500.is_slot_eligible(7));
}

#[test]
fn remove_does_not_request_a_choke_for_an_active_voice() {
    assert_eq!(apply_mixer_event(&mut eligibility(), remove(2_000, 5)), MixerAction::EligibilityOnly);
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums mixer_events`

Expected: FAIL because the mixer model is absent.

- [ ] **Step 3: Implement bounded eligibility over the existing sound bank**

Add `MixerAdd = 0xEE` and `MixerRemove = 0xEF`. If a chart has no mixer events, every registered slot is eligible, preserving existing behavior. Otherwise rebuild the eligible set from events at or before time zero and advance each event once in forward play. Schedulers consult `is_slot_eligible` before starting a new sound. Removal never stops an existing audio instance. Backward seek, practice restart, and loop seek call `rebuild_mixer_at(target_ms)`. Missing slots emit one grouped optional-audio diagnostic.

- [ ] **Step 4: Verify forward/seek/restart behavior and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums mixer`

Expected: PASS for repeated events, normal play, backward seek, practice restart, and missing slots.

```bash
git add crates/dtx-core/src/channel.rs crates/gameplay-drums/src/mixer_events.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/sound_bank.rs crates/gameplay-drums/src/bgm_scheduler.rs crates/gameplay-drums/src/se_scheduler.rs crates/gameplay-drums/src/hit_sound.rs crates/gameplay-drums/src/seek.rs
git commit -m "feat: rebuild mixer eligibility across seeks"
```

### Task 6: Parse and render BGA swaps and pan animation deterministically

**Files:**
- Modify: `crates/dtx-core/src/channel.rs`
- Modify: `crates/dtx-core/src/assets.rs`
- Modify: `crates/dtx-core/src/bga.rs`
- Modify: `crates/dtx-core/src/parser.rs`
- Create: `crates/dtx-core/tests/fixtures/compatibility/visual_pan_swap.dtx`
- Modify: `crates/dtx-core/tests/bga_tests.rs`
- Modify: `crates/dtx-bga/src/chart.rs`
- Modify: `crates/dtx-bga/src/lib.rs`
- Modify: `crates/dtx-bga/tests/integration_bga.rs`

**Interfaces:**
- Consumes: BGAPAN/AVIPAN definitions, swap channels, chart timing, `BgaClock`, and Cycle 6 `BgaSettings::motion_enabled`.
- Produces: `PanDefinition`, `VisualGeometry`, `VisualEventKind`, `TimedVisualEvent`, and `visual_state_at`.

- [ ] **Step 1: Write failing parser/interpolation tests**

```rust
#[test]
fn pan_definition_requires_asset_plus_thirteen_numeric_fields() {
    let report = parse_report("#BGAPAN01: 02,100,100,50,50,0,0,10,10,20,20,30,30,96\n");
    let pan = report.chart.assets.bga_pan.get(1).unwrap();
    assert_eq!(pan.asset_slot, 2);
    assert_eq!(pan.duration_ticks, 96);
}

#[test]
fn visual_state_is_deterministic_at_start_mid_end_and_after_seek() {
    let events = pan_swap_events();
    assert_eq!(visual_state_at(&events, 1_000).geometry, start_geometry());
    assert_eq!(visual_state_at(&events, 1_500).geometry, midpoint_geometry());
    assert_eq!(visual_state_at(&events, 2_000).geometry, end_geometry());
    assert_eq!(visual_state_at(&events, 1_500), visual_state_after_backward_seek(&events, 1_500));
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core -p dtx-bga`

Expected: FAIL because pan registries and typed visual operations are absent.

- [ ] **Step 3: Implement parser definitions and chart-time visual operations**

```rust
pub enum VisualEventKind {
    Replace { layer: BgaLayer, asset_slot: u32 },
    Swap { layer: BgaLayer, source_layer: BgaLayer },
    ImagePan { layer: BgaLayer, definition: PanDefinition },
    MoviePan { definition: PanDefinition },
}

pub struct PanDefinition {
    pub asset_slot: u32,
    pub source_start: PixelRect,
    pub source_end: PixelRect,
    pub destination_start: PixelRect,
    pub destination_end: PixelRect,
    pub duration_ticks: u32,
}
```

Parse the full NX argument order and replace duplicate ids with the later definition. Malformed arity/numbers produce line diagnostics. Add swap channels 0xC4, 0xC7, 0xD5–0xD9, and 0xE0. Convert duration ticks to chart milliseconds at event normalization, interpolate in chart time, clamp source rectangles to media bounds and destinations to the stage safe area, and apply zero duration immediately. `visual_state_at` replays replace/swap/pan operations through the target time; reduced Background Motion resolves to the latest static end state without starting a movie. Invalid optional visual events are logged and skipped without changing the gameplay timeline.

- [ ] **Step 4: Verify render and seek fixtures and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core --test bga_tests && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-bga --test integration_bga`

Expected: PASS for start/mid/end/zero duration, layer swaps, normal play, backward seek, restart, and reduced background motion.

```bash
git add crates/dtx-core/src/channel.rs crates/dtx-core/src/assets.rs crates/dtx-core/src/bga.rs crates/dtx-core/src/parser.rs crates/dtx-core/tests/fixtures/compatibility/visual_pan_swap.dtx crates/dtx-core/tests/bga_tests.rs crates/dtx-bga/src/chart.rs crates/dtx-bga/src/lib.rs crates/dtx-bga/tests/integration_bga.rs
git commit -m "feat: render deterministic bga pan and swaps"
```

### Task 7: Add XA fallback and faithful load rejection

**Files:**
- Modify: `crates/gameplay-drums/src/sound_bank.rs`
- Modify: `crates/game-menu/src/song_loading.rs`
- Modify: `crates/dtx-library/src/import.rs`
- Modify: `crates/dtx-library/tests/import.rs`
- Create: `crates/gameplay-drums/tests/fixtures/xa/required_bgm.dtx`
- Create: `crates/gameplay-drums/tests/fixtures/xa/optional_se.dtx`
- Modify: `crates/gameplay-drums/tests/play_chart.rs`

**Interfaces:**
- Consumes: registered WAV slots, chart usage classification, case-insensitive asset resolution, and load diagnostics.
- Produces: `AudioResolution`, `AudioRequirement`, `ChartAudioReport`, `resolve_chart_audio`, XA substitution warnings, and required-BGM rejection.

- [ ] **Step 1: Write failing resolution tests**

```rust
#[test]
fn xa_fallback_uses_ogg_then_wav_then_mp3_case_insensitively() {
    let dir = xa_fixture_dir_with(["music.MP3", "music.WAV", "music.OGG"]);
    assert_eq!(resolve_chart_audio(&dir, "MUSIC.xa"), AudioResolution::Substituted(dir.join("music.OGG")));
}

#[test]
fn missing_required_xa_bgm_rejects_but_optional_se_degrades() {
    let required = preflight(required_xa_bgm());
    assert_eq!(required.required_failures.len(), 1);
    let optional = preflight(optional_xa_se());
    assert_eq!(optional.warnings.len(), 1);
    assert!(optional.required_failures.is_empty());
}
```

- [ ] **Step 2: Verify failure**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums -p game-menu xa`

Expected: FAIL because XA is only classified unsupported.

- [ ] **Step 3: Implement substitution and usage-sensitive policy**

```rust
pub enum AudioResolution {
    Native(PathBuf),
    Substituted(PathBuf),
    Missing,
    Unsupported,
}

pub enum AudioRequirement { RequiredBgm, Optional }

pub struct AudioSubstitution {
    pub slot: u32,
    pub requested: PathBuf,
    pub resolved: PathBuf,
}

pub struct ChartAudioReport {
    pub substitutions: Vec<AudioSubstitution>,
    pub warnings: Vec<PreloadIssue>,
    pub required_failures: Vec<PreloadIssue>,
}
```

Resolve native supported assets first. For `.xa`, search the chart directory by case-insensitive same stem in OGG/WAV/MP3 order. Record substitutions in `ChartAudioReport`. Classify each use as `RequiredBgm` or `Optional`. If an unresolved XA slot is used by BGM, append a required failure; Song Loading maps a nonempty required-failure list to `LoadSupport::Rejected`. If used only by SE/preview/optional media, append a warning and Song Loading maps it to `LoadSupport::Degraded`. Each issue names slot/path/conversion guidance. Archive import returns the same diagnostics but never runs a converter.

- [ ] **Step 4: Verify fallback priority and commit**

Run: `cargo fmt --all && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-library -p gameplay-drums -p game-menu xa`

Expected: PASS for uppercase extensions, all fallback combinations, required rejection, optional degradation, and archive reporting.

```bash
git add crates/gameplay-drums/src/sound_bank.rs crates/game-menu/src/song_loading.rs crates/dtx-library/src/import.rs crates/dtx-library/tests/import.rs crates/gameplay-drums/tests/fixtures/xa crates/gameplay-drums/tests/play_chart.rs
git commit -m "feat: recover or reject xa media explicitly"
```

### Task 8: Publish an executable compatibility matrix and rate-mode truth

**Files:**
- Modify: `crates/dtx-core/tests/compatibility_matrix.rs`
- Modify: `crates/gameplay-drums/tests/play_chart.rs`
- Create or Modify: `docs/compatibility.md`
- Modify: `docs/notes/2026-07-13-game-improvement-program.md`

**Interfaces:**
- Consumes: every fixture and support state introduced in Tasks 1–7.
- Produces: one table whose discovery/parse/load/play/render/diagnostic expectations match executable outcomes.

- [ ] **Step 1: Add matrix rows that fail if any support layer drifts**

```rust
struct CompatibilityCase {
    path: &'static str,
    discovery: ExpectedDiscovery,
    parse: ExpectedParse,
    load: ExpectedLoad,
    play: ExpectedPlay,
    diagnostic: Option<&'static str>,
}

#[test]
fn declared_compatibility_cases_match_end_to_end_outcomes() {
    for case in COMPATIBILITY_CASES { assert_case(case); }
}
```

Rows cover levels/aliases, every added channel, hidden exclusions, MIDI/fill-in, mixer seeks, BGA pan/swap, GDA/G2D equivalence, BMS/BME rejection, XA priority/policy, encodings, filename case, BPM/bar changes, and mixed media.

- [ ] **Step 2: Verify matrix failure before documentation edits**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core --test compatibility_matrix && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p gameplay-drums --test play_chart`

Expected: PASS only after all executable rows exist; any unsupported claim remains a named failure.

- [ ] **Step 3: Write the compatibility guide from the matrix vocabulary**

Document DTX/GDA/G2D as Supported only for the passing drum contract. Document optional media substitutions as Degraded with Warning. Document BMS/BME and XA BGM without fallback as Rejected with Recovery. State that speed changes currently alter pitch and that no pitch-preserving mode is offered. Do not call extension discovery alone support.

- [ ] **Step 4: Run Cycle 7 gates and commit completion**

Run: `cargo fmt --all -- --check && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test -p dtx-core -p dtx-library -p dtx-assets -p dtx-audio -p dtx-bga -p game-menu -p gameplay-drums && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy -p dtx-core -p dtx-library -p dtx-assets -p dtx-audio -p dtx-bga -p game-menu -p gameplay-drums --all-targets -- -D warnings`

Expected: PASS.

```bash
git add crates/dtx-core/tests/compatibility_matrix.rs crates/gameplay-drums/tests/play_chart.rs docs/compatibility.md docs/notes/2026-07-13-game-improvement-program.md
git commit -m "docs: publish verified extended compatibility"
```

### Task 9: Run workspace verification

**Files:**
- No source changes unless a gate exposes a Cycle 7 regression.

- [ ] **Step 1: Run workspace gates**

Run: `CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo check --workspace && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo clippy --workspace --all-targets -- -D warnings && CARGO_TARGET_DIR=/home/lazykern/lab/dtxmaniars/target cargo test --workspace --lib`

Expected: PASS.

- [ ] **Step 2: Verify repository scope**

Run: `git diff --check && git status --short && git log --oneline -10`

Expected: no whitespace errors, no reference-tree changes, no native XA library/converter, no cache/folder hierarchy, and no CI/CD file changes.

## Plan self-review

The nine tasks cover provenance, normalized aliases/levels, dedicated GDA/G2D parsing, format discovery/import/loading, explicit BMS/BME rejection, every approved hidden/system/mixer/swap channel, pan geometry and seek reconstruction, XA fallback/rejection, structured diagnostics, playback-rate truth, fixture-backed documentation, and package/workspace gates. Types are introduced before library/gameplay consumers. BMS/BME gameplay, XA decode/conversion, pitch-preserving stretch, guitar/bass expansion, cache/folder hierarchy, unsafe code, reference edits, and CI/CD remain excluded.
