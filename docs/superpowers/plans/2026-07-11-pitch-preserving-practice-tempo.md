# Pitch-Preserving Practice Tempo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Practice tempo below 1.0× keeps the song's pitch. Current behavior (kira `set_playback_rate`) resamples — slower = lower-pitched. Roadmap constraint: keep the practice UX and domain model untouched; only the audio application changes; do not redesign practice.

**Architecture:** kira (via bevy_kira_audio 0.26) has no time-stretch. Realtime stretch inside kira would require a custom `Sound` implementation reaching past bevy_kira_audio's abstraction — high risk. Instead: **offline pre-stretch**. Practice tempo is quantized (0.5..1.5 in 0.05 steps → at most 21 values, and sessions typically use 3-5 of them); when the tempo changes, a background task renders the BGM through Signalsmith Stretch into a new buffer, cached per (chart, tempo); playback swaps to the stretched sound at rate 1.0. Until a render finishes — and as a permanent fallback if the spike fails — the existing pitch-shifting rate path plays (graceful, current behavior). The gameplay clock keeps its contract: it advances `dt × tempo` in chart-ms; only the audio-position→chart-ms mapping gains a `× tempo` factor for stretched playback.

**Tech Stack:** candidate crates (Task 1 decides): `signalsmith-stretch` or `ssstretch` (both bind the C++ Signalsmith Stretch library — high quality, faster than realtime; C++ toolchain required at build), `timestretch` (pure Rust, quality unproven). Decoding: the BGM is ogg — decode via `lewton` or `symphonia` (spike picks; kira itself uses symphonia internally, so `symphonia` with ogg/vorbis features adds no new native deps).

**Source basis (verified 2026-07-11):**
- Pitch shift lives ONLY in `crates/gameplay-drums/src/practice/rate.rs:38-42` (`audio.set_playback_rate(target)` channel-wide + BGM instance tween). Domain model (`PracticeSession::effective_tempo`, session.rs:214-222) and clock coupling (`AudioRate` consumed by `sync_gameplay_clock`, lib.rs:286-299: `tick(dt * rate, start_ms + audio_position)`) are backend-agnostic.
- BGM playback: `dtx_audio::play_bgm*` / `play_bgm_from_seconds*` (`crates/dtx-audio/src/lib.rs:268-442`), `BgmHandle.instance`; position via `position_ms` (:631-640). bevy_kira_audio 0.26, feature `ogg` (workspace Cargo.toml:60).
- Practice tempo quantization: `RATE_MIN=0.5, RATE_MAX=1.5, RATE_STEP=0.05` (`practice/session.rs:8-10`); `step_user_tempo` quantizes (:256-259).
- Seek/loop: practice seeks constantly (`SeekToChartTime`, A/B loops) — the stretched path must support `play_bgm_from_seconds`-style starts (stretched-time seconds = chart seconds / tempo).
- v1 accepted pitch-shift documented at `docs/superpowers/specs/2026-07-07-practice-ux-v2-design.md:32` and practice plan Task 7.

**Decision gate:** Task 1 is a spike with kill criteria. If it fails, STOP after Task 1 and record why in the spike doc — the roadmap explicitly conditions this feature on "a suitable audio backend proves viable". Everything after Task 1 assumes the spike passed.

---

### Task 1: Viability spike (throwaway binary, keep the numbers)

**Files:**
- Create: `tools/stretch-spike/` (new bin crate, NOT added to default workspace CI groups; delete or keep as a tools/ utility after the decision)
- Create: `docs/superpowers/specs/2026-07-11-stretch-spike-result.md`

- [ ] **Step 1: Scaffold**

```bash
cargo new tools/stretch-spike --bin
```

Add to its Cargo.toml (NOT workspace-wide):

```toml
[dependencies]
signalsmith-stretch = "*"   # pin to latest on crates.io at execution time
symphonia = { version = "*", features = ["ogg", "vorbis"] }
hound = "*"                  # wav out for listening
```

(Workspace members glob `tools/*` auto-includes it — that's fine; it stays out of the CI test groups.)

- [ ] **Step 2: Implement the spike**

The bin takes `<input.ogg> <tempo>`: decode ogg → f32 interleaved stereo; run Signalsmith Stretch at time-ratio `1/tempo` (0.7 tempo = 1.43× longer) with pitch ratio 1.0; write `out.wav`; print decode ms, stretch ms, output duration, peak RSS if easy. Consult the crate's docs for the exact API (`npx ctx7@latest library "signalsmith-stretch"` or docs.rs) — do not guess the call signatures.

- [ ] **Step 3: Measure against the kill criteria**

Run on a real ~4-minute BGM ogg at tempos 0.7 and 0.9. Record in `docs/superpowers/specs/2026-07-11-stretch-spike-result.md`:

| Criterion | Threshold | Measured |
|---|---|---|
| Stretch wall time (4-min song) | < 3 s release build | |
| Artifacts (listen: drums transient smear?) | usable for practice | |
| Build impact (C++ toolchain OK on target/CI?) | builds on x86_64-linux + CI image | |
| Memory (decoded+stretched buffers) | < 300 MB peak | |

Listen check is subjective — the executor plays both wavs and notes transient quality (drum practice tolerates mild smear; melody pitch must be stable).

- [ ] **Step 4: Decide and record**

PASS → continue to Task 2 with the chosen crate pinned. FAIL (any criterion) → try `ssstretch`, then `timestretch`; if all fail, write the numbers + verdict in the spike doc, commit it, and STOP (fallback pitch-shift remains shipped behavior; roadmap explicitly allows this outcome).

- [ ] **Step 5: Commit**

```bash
git add tools/stretch-spike docs/superpowers/specs/2026-07-11-stretch-spike-result.md
git commit -m "spike: signalsmith time-stretch viability measurements"
```

---

### Task 2: Stretched-BGM rendering + cache in dtx-audio

**Files:**
- Create: `crates/dtx-audio/src/stretch.rs`
- Modify: `crates/dtx-audio/src/lib.rs` (module + re-exports)
- Modify: `crates/dtx-audio/Cargo.toml` (feature-gated dep)

- [ ] **Step 1: Feature-gate the dependency**

```toml
[features]
stretch = ["dep:signalsmith-stretch", "dep:symphonia"]

[dependencies]
signalsmith-stretch = { version = "<pinned from spike>", optional = true }
symphonia = { version = "<pinned>", features = ["ogg", "vorbis"], optional = true }
```

Enable `stretch` from `gameplay-drums`'s dtx-audio dependency (and transitively the app). Feature-gating keeps the C++ build out of minimal builds and gives a clean kill switch.

- [ ] **Step 2: Write the pure core, test-first**

`stretch.rs` — separate rendering (pure, testable) from Bevy plumbing:

```rust
/// Key for the per-session stretch cache. Tempo quantized to the practice
/// step grid so 0.8500001 and 0.85 share an entry.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct StretchKey {
    pub bgm_path: PathBuf,
    pub tempo_centi: u32, // tempo * 100 rounded; 50..=150
}

pub fn quantize_tempo(tempo: f32) -> u32 {
    ((tempo * 100.0).round() as u32).clamp(50, 150)
}

/// Decode + stretch. Runs on AsyncComputeTaskPool, never on the main thread.
/// Returns interleaved f32 stereo at the source sample rate.
pub fn render_stretched(bgm_path: &Path, tempo: f32) -> Result<StretchedPcm, StretchError> { ... }

pub struct StretchedPcm {
    pub sample_rate: u32,
    pub frames: Vec<[f32; 2]>,
}
```

Tests (in-file): `quantize_tempo` grid/clamps; `render_stretched` against a generated fixture — synthesize a 2-second 440 Hz sine ogg? Generating ogg in-test is heavy; instead make `render_stretched` take a decoded buffer internally (`stretch_pcm(pcm: &StretchedPcm, tempo: f32) -> StretchedPcm`) and unit-test THAT: a 1000-frame buffer at tempo 0.5 yields ~2000±32 frames and preserves dominant frequency (assert via zero-crossing count within 5%). Decode stays a thin untested wrapper (exercised by the spike + manual).

- [ ] **Step 3: Bevy-side cache + task plumbing**

Resource + system in `stretch.rs`, following the async-task pattern from `chart_stats.rs:76-124` (`AsyncComputeTaskPool` + poll system):

```rust
#[derive(Resource, Default)]
pub struct StretchCache {
    /// Finished renders, as kira-playable audio sources.
    ready: HashMap<StretchKey, Handle<AudioSource>>,
    in_flight: Option<(StretchKey, Task<Result<StretchedPcm, StretchError>>)>,
}
```

Poll system converts finished `StretchedPcm` into a `bevy_kira_audio::AudioSource` — check what bevy_kira_audio 0.26 exposes for raw-PCM sources (`AudioSource { sound: StaticSoundData }`; kira's `StaticSoundData::from_frames`-style constructor — consult docs via ctx7 at execution, API varies by kira version). Insert into `Assets<AudioSource>` and store the handle. Cap the cache at 4 entries (LRU by insertion order) — practice hops between few tempos; stretched 4-min stereo f32 ≈ 80 MB/entry.

- [ ] **Step 4: Run tests**

Run: `cargo test -p dtx-audio --features stretch -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-audio
git commit -m "feat(dtx-audio): offline time-stretch rendering with per-tempo cache"
```

---

### Task 3: Swap practice playback onto stretched audio (with live fallback)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/rate.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (`sync_gameplay_clock`, :286-299)

- [ ] **Step 1: Playback-mode resource, test-first**

```rust
/// How practice tempo is currently realized. Drives both the audio swap and
/// the clock's position mapping.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub enum TempoPlayback {
    #[default]
    Native,                    // tempo 1.0, original BGM
    PitchShift { rate: f64 },  // fallback: kira playback rate (current behavior)
    Stretched { tempo: f64 },  // stretched buffer at rate 1.0
}

/// Chart-ms measured from the raw audio position, given the playback mode.
pub fn measured_chart_ms(audio_position_ms: f64, start_ms: f64, mode: TempoPlayback) -> f64 {
    match mode {
        TempoPlayback::Native | TempoPlayback::PitchShift { .. } => start_ms + audio_position_ms,
        // stretched audio runs 1/tempo longer: position × tempo = chart time
        TempoPlayback::Stretched { tempo } => start_ms + audio_position_ms * tempo,
    }
}

#[test]
fn stretched_position_maps_back_to_chart_time() {
    // at tempo 0.8 the stretched file reaches chart-second 8 after 10 s of audio
    assert!((measured_chart_ms(10_000.0, 0.0, TempoPlayback::Stretched { tempo: 0.8 }) - 8_000.0).abs() < 1e-6);
    assert_eq!(measured_chart_ms(5_000.0, 1_000.0, TempoPlayback::Native), 6_000.0);
}
```

(PitchShift mode needs no mapping: kira's `position` is source-time, which IS chart time on the original file — that's how today's code works; keep it.)

- [ ] **Step 2: Selection logic in the rate system**

Extend the rate-apply system (`rate.rs:38-42`; if the play-speed-contract plan landed, this is `apply_playback_rate` — integrate there, `target_rate` remains the single tempo source):

- Desired tempo `t` = current target rate.
- If `t == 1.0` → Native: play original BGM at rate 1.0.
- Else if `StretchCache` has `ready[StretchKey(bgm, quantize_tempo(t))]` → swap the BGM instance to the stretched source (stop current instance, `play_bgm_from_seconds(stretched_handle, chart_seconds / t)` so position continuity holds), set `TempoPlayback::Stretched`, playback rate 1.0.
- Else → request a render (`StretchCache.request(key)`) and meanwhile apply the EXISTING pitch-shift path, `TempoPlayback::PitchShift`.
- When a requested render completes while still at that tempo, the next run of this system performs the swap (seamless-enough: practice loops restart constantly; swap also happens naturally on the next loop seek).

Seeks: practice's `apply_seek_system` restarts BGM at a chart position — thread `TempoPlayback` in so stretched playback seeks to `chart_seconds / tempo` (grep where `play_bgm_from_seconds` is invoked on seek; single call site expected in the orchestrator/seek path).

Keysounds/SE stay on the pitch-shift channel rate in Stretched mode? NO — in Stretched mode set channel rate to 1.0 and accept native-pitch keysounds (they're one-shot hits; stretching them is meaningless). Only the BGM was ever the pitch problem.

- [ ] **Step 3: Clock mapping**

In `sync_gameplay_clock` (lib.rs:286-299), replace the direct `start_ms + audio_position` with `measured_chart_ms(audio_position, start_ms, *tempo_playback)`. The `dt × AudioRate` free-run advance is ALREADY in chart-ms and stays untouched (in Stretched mode `AudioRate` still equals the tempo — chart-time advances at tempo × wall speed in both modes; only the measurement mapping differs).

- [ ] **Step 4: Tests**

- Pure: Step 1 tests.
- Headless (`tests/practice_mode.rs`): with `TempoPlayback::Stretched { tempo: 0.8 }` inserted and a synthetic audio position fed through the existing `clock.sync`-style helper, assert `GameplayClock.current_ms` lands on chart-time (×0.8). Mirror how existing tests inject positions.

Run: `cargo test -p gameplay-drums -j 2` (incl. schedule guard).
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums
git commit -m "feat(practice): pitch-preserving tempo via stretched BGM with pitch-shift fallback"
```

---

### Task 4: Manual A/B verification

- [ ] **Step 1:** Practice a melodic chart at 0.7×. First loop may be pitch-shifted (render in flight); within a few seconds/next loop the pitch corrects to native while speed stays 0.7×. Notes stay in sync across a full loop (no drift — the clock mapping test made this likely; ears confirm).
- [ ] **Step 2:** Step tempo 0.7 → 0.75 → 0.8 rapidly: no crash, fallback bridges gaps, cache holds recent tempos so stepping BACK is instant.
- [ ] **Step 3:** Ramp mode graduation to 1.0×: Native path resumes (original file, rate 1.0).
- [ ] **Step 4:** A/B loop seeks land at the right musical position in stretched mode.
- [ ] **Step 5:** Build without the feature (`cargo check -p gameplay-drums --no-default-features` as applicable): pitch-shift-only behavior intact.

---

## Constraint compliance (roadmap)

- "Keep current practice UX and domain model" → zero changes to `PracticeSession`/transport/ramp/HUD; only `rate.rs` + audio plumbing + one clock mapping fn.
- "when a suitable audio backend proves viable" → Task 1 decision gate with recorded numbers; failing the gate ships nothing and costs one spike doc.
- "Do not redesign practice again" → honored; tempo keys/ramp/loops untouched.

## Research sources

- signalsmith-stretch bindings: https://lib.rs/crates/signalsmith-stretch
- ssstretch bindings: https://github.com/bmisiak/ssstretch
- Upstream C++ library: https://github.com/Signalsmith-Audio/signalsmith-stretch
- Pure-Rust alternative: https://lib.rs/crates/timestretch
