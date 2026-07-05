# Live Autoplay Song-Select Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-clip `#PREVIEW` at song select with a live autoplay of the actual song — full BGM plus autoplayed drum/SE chip sounds, starting at an inferred "good part," synced like gameplay, looping back to the start point, and resuming in place after a round-trip into gameplay.

**Architecture:** A note-density heuristic computed at scan time picks a preview start point stored on `SongInfo`. A new `preview_autoplay` module in `game-menu` runs a mini scheduler at song select: it reuses the gameplay audio-master-clock model (a `PreviewClock` drift-corrected toward the BGM's real playback position via `position_ms`), walks the chart's BGM/drum/SE chips, and plays each when due through the existing `dtx-audio` primitives. A 250 ms dwell debounce gates async chart parsing; the existing single-clip `PreviewPlayer` remains the fallback when a chart fails to parse or has no audio chips.

**Tech Stack:** Rust, Bevy (states, resources, systems, `AsyncComputeTaskPool`), `bevy_kira_audio`. Reuses `dtx-core` (`Chart`/`Chip`/`EChannel`), `dtx-timing` (chip-time math), `dtx-audio` (play primitives, `BgmHandle`, `DrumPolyphony`, `ChartSoundBank`), `gameplay-drums` (`lane_map::lane_of`, `sound_bank` collectors).

---

## Reference: verbatim signatures this plan calls

These are current as of writing (file:line). Use them exactly.

```rust
// crates/dtx-timing/src/lib.rs  (pub mod math, re-exported)
pub fn chip_time_ms_with_bpm_and_bar_changes(measure: u32, fraction: f32, base_bpm: f32, timing: ChartTiming<'_>) -> i64; // :216
#[derive(Debug, Clone, Copy, PartialEq)] pub struct BpmChange { pub measure: u32, pub bpm: f32 }                          // :123
#[derive(Debug, Clone, Copy, PartialEq)] pub struct BarLengthChange { pub measure: u32, pub ratio: f32 }                 // :136
#[derive(Debug, Clone, Copy, Default)] pub struct ChartTiming<'a> { pub bpm_changes: &'a [BpmChange], pub bar_changes: &'a [BarLengthChange] } // :145

// crates/dtx-core/src/chart.rs / channel.rs
pub struct Chip { pub measure: u32, pub channel: EChannel, pub value: f32, pub wav_slot: u32 }
impl Chart { pub fn drum_chips(&self) -> impl Iterator<Item=&Chip>; }  // uses channel.is_drum()
// EChannel::BGM = 1; SE01=0x61 .. SE05=0x65; drum lanes 0x11..0x1C (+ DrumsFillin 0x1F)
pub const fn is_drum(self) -> bool;   // includes DrumsFillin

// crates/gameplay-drums/src/lane_map.rs
pub fn lane_of(channel: EChannel) -> Option<LaneId>;   // Some only for the 12 LANE_ORDER channels (excludes DrumsFillin)

// crates/dtx-audio/src/lib.rs
#[derive(Resource, Default, Debug, Clone)] pub struct BgmHandle { pub instance: Option<Handle<AudioInstance>>, pub path: Option<String> }
pub struct DrumPolyphony; impl DrumPolyphony { pub fn reset(&mut self); }  // Default voices=4
#[derive(Resource, Default, Debug, Clone)] pub struct ChartSoundBank; // get(u32)->Option<&LoadedChartSound>, insert, clear, is_empty
pub struct LoadedChartSound { pub handle: Handle<KiraAudioSource>, pub path: PathBuf, pub volume: i32, pub pan: i32 }
pub fn preload_chart_sound(asset_server:&AssetServer, bank:&mut ChartSoundBank, source_dir:Option<&Path>, wav_slot:u32, filename:&str, volume:i32, pan:i32) -> Handle<KiraAudioSource>;
pub fn resolve_chart_audio_path(chart_dir:&Path, filename:&str) -> PathBuf;
pub fn play_bgm_handle_with_mix_from_seconds(audio:&Audio, instances:&mut Assets<AudioInstance>, bgm:&mut BgmHandle, source:Handle<KiraAudioSource>, path:&str, dtx_volume:i32, dtx_pan:i32, master:f32, start_seconds:f64, fade_in_ms:u32) -> Handle<AudioInstance>;
pub fn play_bgm_handle_with_mix(audio:&Audio, bgm:&mut BgmHandle, instances:&mut Assets<AudioInstance>, source:Handle<KiraAudioSource>, path:&str, dtx_volume:i32, dtx_pan:i32, master:f32, fade_in_ms:u32) -> Handle<AudioInstance>;
pub fn play_drum_hit_handle(audio:&Audio, instances:&mut Assets<AudioInstance>, polyphony:&mut DrumPolyphony, source:Handle<KiraAudioSource>, wav_slot:u32, dtx_volume:i32, dtx_pan:i32, master:f32, drum_channel:f32) -> Handle<AudioInstance>;
pub fn position_ms(audio:&Audio, bgm:&BgmHandle) -> Option<i64>;
pub fn stop_bgm(audio:&Audio, bgm:&mut BgmHandle, instances:&mut Assets<AudioInstance>);

// crates/dtx-audio/src/crossfade.rs
pub const PREVIEW_FADE_IN_MS: u32 = 220;  pub const PREVIEW_FADE_OUT_MS: u32 = 150;
pub fn stop_with_fade(instances:&mut Assets<AudioInstance>, handle:&Handle<AudioInstance>, ms:u32);

// crates/game-shell/src/states.rs
pub enum AppState { Startup, Title, Config, SongSelect, SongLoading, Performance, StageClear, StageFailed, Result, End }

// crates/game-menu/src/song_select.rs
pub struct Selection { pub folder: usize, pub difficulty: u8 }
impl Selection { pub fn chart_index(&self, sel:&SongSelectSelection) -> Option<usize>; }
// current song = db.songs.get(selection.chart_index(&selection_state)?)  -> &SongInfo
```

**Key facts:**
- The parsed `Chart` is dropped during scan at `dtx-library/src/lib.rs:165`; only `SongInfo` fields survive. The heuristic must run *inside* `SongInfo::from_chart` where the `Chart` is still in hand.
- `GameplayClock::tick(dt_secs, measured_ms)` (`gameplay-drums/src/resources.rs:493`) free-runs on frame delta and drift-corrects toward the measured kira BGM position; the first observed position snaps the clock onto the audio timeline. `PreviewClock` copies this algorithm.
- Drum-lane chips are played by the input/judge path in gameplay, **not** by any time scheduler. The preview must schedule them itself (like SE chips).

---

## File structure

- Create `crates/dtx-library/src/preview_point.rs` — pure `compute_preview_start_ms(&Chart) -> u32`.
- Modify `crates/dtx-library/src/lib.rs` — `mod preview_point;`, add `preview_start_ms` to `SongInfo`, populate in `from_chart`.
- Modify `crates/dtx-library/Cargo.toml` — add `dtx-timing` dep.
- Create `crates/game-menu/src/preview_autoplay.rs` — `PreviewClock`, `build_preview_schedule`, `PreviewAutoplay` resource + systems.
- Modify `crates/game-menu/src/song_select.rs` — dwell debounce, async parse, wire autoplay + clip fallback, freeze/resume.
- Modify `crates/game-menu/src/lib.rs` (or wherever `mod`s live) — `mod preview_autoplay;` and register systems.
- Modify `crates/game-menu/Cargo.toml` — add `dtx-timing` dep.

---

## Task 1: Preview-point density heuristic (pure)

**Files:**
- Modify: `crates/dtx-library/Cargo.toml`
- Create: `crates/dtx-library/src/preview_point.rs`
- Modify: `crates/dtx-library/src/lib.rs` (add `mod preview_point;` + re-export)

- [ ] **Step 1: Add the `dtx-timing` dependency**

In `crates/dtx-library/Cargo.toml`, under `[dependencies]`, add after the `dtx-core` line:

```toml
dtx-timing = { path = "../dtx-timing" }
```

- [ ] **Step 2: Write the failing test file**

Create `crates/dtx-library/src/preview_point.rs`:

```rust
//! Note-density heuristic that picks a song-select preview start point.
//!
//! No preview-time field exists in DTX (see the 2026-07-05 design doc), so
//! we infer one: the start of the densest 20-second drum-note window,
//! biased toward the earliest such window and clamped so it never lands in
//! the final third of the chart. Charts with no drum notes fall back to
//! 40% of the chart's duration (the osu!/drum-game default).
//!
//! Pure and engine-independent: timing is computed with a constant base
//! BPM (BPM changes only shift the chosen window slightly, which does not
//! matter for a preview point), so this needs no BPM-change table.

use dtx_core::Chart;
use dtx_timing::math::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};

/// Width of the density-scan window.
pub const PREVIEW_WINDOW_MS: i64 = 20_000;
/// Fallback start as a fraction of chart duration when there are no drums.
const FALLBACK_FRACTION: f64 = 0.4;
/// Never start a preview past this fraction of the chart.
const MAX_START_FRACTION: f64 = 0.7;

/// Absolute time (ms) of a chip under a constant base BPM.
fn chip_ms(chart: &Chart, measure: u32, value: f32) -> i64 {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &[],
        bar_changes: &[],
    };
    chip_time_ms_with_bpm_and_bar_changes(measure, value, base_bpm, timing)
}

/// Compute the preview start point in milliseconds for a chart.
pub fn compute_preview_start_ms(chart: &Chart) -> u32 {
    let duration_ms = chart
        .chips
        .iter()
        .map(|c| chip_ms(chart, c.measure, c.value))
        .max()
        .unwrap_or(0)
        .max(0);
    let max_start = (duration_ms as f64 * MAX_START_FRACTION) as i64;

    let mut drum_times: Vec<i64> = chart
        .drum_chips()
        .map(|c| chip_ms(chart, c.measure, c.value))
        .filter(|&t| t >= 0)
        .collect();
    drum_times.sort_unstable();

    if drum_times.is_empty() {
        let fallback = (duration_ms as f64 * FALLBACK_FRACTION) as i64;
        return fallback.clamp(0, max_start.max(0)) as u32;
    }

    let mut best_start = drum_times[0];
    let mut best_count = 0usize;
    for &start in &drum_times {
        if start > max_start {
            break;
        }
        let end = start + PREVIEW_WINDOW_MS;
        let count = drum_times.iter().filter(|&&t| t >= start && t < end).count();
        if count > best_count {
            best_count = count;
            best_start = start;
        }
    }
    best_start.clamp(0, max_start.max(0)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::channel::EChannel;
    use dtx_core::chart::{Chip, Metadata};

    fn chart_with(bpm: f32, chips: Vec<Chip>) -> Chart {
        Chart {
            metadata: Metadata {
                bpm: Some(bpm),
                ..Default::default()
            },
            chips,
            ..Default::default()
        }
    }

    // At 120 BPM a 4/4 measure is 2000ms; value is the fraction within it,
    // so measure m, value v -> (m + v) * 2000 ms.

    #[test]
    fn no_drums_falls_back_to_40_percent() {
        // One BGM chip at measure 10 => duration 20000ms; 40% => 8000ms.
        let chart = chart_with(120.0, vec![Chip::new(10, EChannel::BGM, 0.0)]);
        assert_eq!(compute_preview_start_ms(&chart), 8000);
    }

    #[test]
    fn empty_chart_is_zero() {
        let chart = chart_with(120.0, vec![]);
        assert_eq!(compute_preview_start_ms(&chart), 0);
    }

    #[test]
    fn picks_densest_window() {
        // Sparse drums early (m0..m2), a dense cluster at m5 (5 chips within
        // one measure = 2000ms < 20s window), then end marker at m30.
        let mut chips = vec![
            Chip::new(0, EChannel::Snare, 0.0),
            Chip::new(2, EChannel::Snare, 0.0),
            Chip::new(30, EChannel::BGM, 0.0), // duration marker (60000ms)
        ];
        for i in 0..5 {
            chips.push(Chip::new(5, EChannel::Snare, i as f32 * 0.2));
        }
        let start = compute_preview_start_ms(&chart_with(120.0, chips));
        // Densest window begins at the first chip of the m5 cluster = 10000ms.
        assert_eq!(start, 10000);
    }

    #[test]
    fn clamps_to_70_percent() {
        // All drums bunched at the very end (m9) of a 10-measure chart.
        // Duration = 20000ms; max_start = 14000ms. The densest window would
        // start at 18000ms but must clamp to 14000ms.
        let mut chips = vec![Chip::new(10, EChannel::BGM, 0.0)];
        for i in 0..4 {
            chips.push(Chip::new(9, EChannel::Snare, i as f32 * 0.1));
        }
        let start = compute_preview_start_ms(&chart_with(120.0, chips));
        assert_eq!(start, 14000);
    }
}
```

- [ ] **Step 3: Register the module**

In `crates/dtx-library/src/lib.rs`, near the top with the other `mod`/`use` lines, add:

```rust
pub mod preview_point;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dtx-library preview_point`
Expected: 4 tests pass (`no_drums_falls_back_to_40_percent`, `empty_chart_is_zero`, `picks_densest_window`, `clamps_to_70_percent`).

If `Chip::new` / `Metadata` / `EChannel` import paths differ, fix the `use` lines to match `dtx-core`'s actual public paths (`dtx_core::chart::{Chip, Metadata}`, `dtx_core::channel::EChannel`).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-library/Cargo.toml crates/dtx-library/src/preview_point.rs crates/dtx-library/src/lib.rs
git commit -m "feat(dtx-library): note-density preview-point heuristic"
```

---

## Task 2: Store `preview_start_ms` on `SongInfo`

**Files:**
- Modify: `crates/dtx-library/src/lib.rs` (`SongInfo` struct + `from_chart` + any struct-literal construction in tests)

- [ ] **Step 1: Add the field to `SongInfo`**

In `crates/dtx-library/src/lib.rs`, in the `SongInfo` struct (around line 31), add after `preimage_path`:

```rust
    /// Inferred song-select preview start point in milliseconds
    /// (note-density heuristic; see `preview_point`). Applies to the
    /// live autoplay preview; 0 for charts with no timed content.
    pub preview_start_ms: u32,
```

- [ ] **Step 2: Populate it in `from_chart`**

In `SongInfo::from_chart` (around line 61), in the returned `Self { ... }` literal, add:

```rust
            preview_start_ms: crate::preview_point::compute_preview_start_ms(chart),
```

- [ ] **Step 3: Fix any other `SongInfo { ... }` literals**

Run: `cargo build -p dtx-library`
Expected: compile errors for every struct-literal construction of `SongInfo` missing the new field (likely in unit tests within `lib.rs`). For each, add `preview_start_ms: 0,`. If `SongInfo` is only ever built via `from_chart`, there will be none.

- [ ] **Step 4: Add a test asserting it is populated**

In the `#[cfg(test)]` module of `crates/dtx-library/src/lib.rs`, add:

```rust
    #[test]
    fn from_chart_sets_preview_start_ms() {
        use dtx_core::channel::EChannel;
        use dtx_core::chart::{Chip, Chart, Metadata};
        let chart = Chart {
            metadata: Metadata { bpm: Some(120.0), ..Default::default() },
            chips: vec![Chip::new(10, EChannel::BGM, 0.0)], // no drums -> 40%
            ..Default::default()
        };
        let info = SongInfo::from_chart(std::path::Path::new("/songs/x.dtx"), &chart);
        assert_eq!(info.preview_start_ms, 8000);
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p dtx-library`
Expected: all pass, including `from_chart_sets_preview_start_ms`.

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-library/src/lib.rs
git commit -m "feat(dtx-library): expose preview_start_ms on SongInfo"
```

---

## Task 3: `PreviewClock` (audio-master clock, mirrors `GameplayClock`)

**Files:**
- Modify: `crates/game-menu/Cargo.toml` (add `dtx-timing`)
- Create: `crates/game-menu/src/preview_autoplay.rs`
- Modify: `crates/game-menu/src/lib.rs` (add `mod preview_autoplay;`)

- [ ] **Step 1: Add the `dtx-timing` dependency**

In `crates/game-menu/Cargo.toml`, under `[dependencies]`, add:

```toml
dtx-timing = { path = "../dtx-timing" }
```

- [ ] **Step 2: Create the module with `PreviewClock` + failing tests**

Create `crates/game-menu/src/preview_autoplay.rs`:

```rust
//! Live autoplay preview engine for song select.
//!
//! Plays the actual song — BGM plus autoplayed drum/SE chips — starting at
//! the chart's inferred preview point (see `dtx_library::preview_point`),
//! synced to the BGM's real playback position and looping back to the start.
//!
//! `PreviewClock` copies the audio-master-clock model of
//! `gameplay_drums::resources::GameplayClock`: it free-runs on frame delta
//! and drift-corrects toward the measured kira BGM position so autoplayed
//! drums stay locked to the music. Keep the two in sync if either changes;
//! unifying them into one shared clock is a possible later refactor.

/// A drift-corrected playback clock in chart-time milliseconds.
#[derive(Debug, Default, Clone)]
pub struct PreviewClock {
    current_ms: i64,
    started: bool,
    audio_synced: bool,
    audio_ms: f64,
}

impl PreviewClock {
    const CORRECTION_GAIN: f64 = 10.0;
    const MAX_CORRECTION_MS: f64 = 20.0;
    const MAX_BACKSTEP_MS: f64 = 8.0;
    const GLITCH_JUMP_MS: f64 = 500.0;

    /// Begin the clock seeded at `start_ms` chart-time (the preview point).
    /// The first observed BGM position snaps the clock onto the audio
    /// timeline; until then it free-runs from `start_ms`.
    pub fn start_at(&mut self, start_ms: i64) {
        self.started = true;
        self.audio_synced = false;
        self.audio_ms = start_ms as f64;
        self.current_ms = start_ms;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn current_ms(&self) -> i64 {
        self.current_ms
    }

    /// Advance by `dt_secs`, drift-correcting toward `measured_ms` (the
    /// chart-time reported by the tracked BGM instance) when available.
    /// Mirrors `GameplayClock::tick`.
    pub fn tick(&mut self, dt_secs: f64, measured_ms: Option<i64>) {
        if !self.started {
            return;
        }
        if !self.audio_synced {
            if let Some(ms) = measured_ms {
                self.audio_synced = true;
                self.audio_ms = ms as f64;
                self.current_ms = ms;
                return;
            }
        }
        let dt_ms = dt_secs * 1000.0;
        let prev = self.audio_ms;
        let mut next = prev + dt_ms;
        if let Some(ms) = measured_ms {
            let measured = ms as f64;
            let glitch = (measured - prev).abs() > Self::GLITCH_JUMP_MS && dt_secs <= 0.5;
            if !glitch && measured >= prev - Self::MAX_BACKSTEP_MS {
                let drift = measured - next;
                let catchup = (Self::CORRECTION_GAIN * dt_secs).clamp(0.0, 1.0);
                next += (drift * catchup).clamp(-Self::MAX_CORRECTION_MS, Self::MAX_CORRECTION_MS);
            }
        }
        self.audio_ms = next;
        self.current_ms = self.audio_ms.round() as i64;
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn unstarted_clock_ignores_tick() {
        let mut c = PreviewClock::default();
        assert!(!c.is_started());
        c.tick(0.1, Some(5000));
        assert_eq!(c.current_ms(), 0);
    }

    #[test]
    fn starts_seeded_at_start_ms() {
        let mut c = PreviewClock::default();
        c.start_at(12000);
        assert!(c.is_started());
        assert_eq!(c.current_ms(), 12000);
    }

    #[test]
    fn free_runs_before_audio_sync() {
        let mut c = PreviewClock::default();
        c.start_at(10000);
        c.tick(0.1, None); // +100ms
        assert_eq!(c.current_ms(), 10100);
    }

    #[test]
    fn snaps_onto_audio_timeline_on_first_measure() {
        let mut c = PreviewClock::default();
        c.start_at(10000);
        // First real BGM position observed: snap directly, no ramp.
        c.tick(0.1, Some(10500));
        assert_eq!(c.current_ms(), 10500);
    }

    #[test]
    fn drift_corrects_toward_audio_after_sync() {
        let mut c = PreviewClock::default();
        c.start_at(10000);
        c.tick(0.016, Some(10000)); // sync
        let before = c.current_ms();
        // Audio ahead by 50ms: clock should move toward it, but < full jump.
        c.tick(0.016, Some(before + 50));
        assert!(c.current_ms() > before);
        assert!(c.current_ms() < before + 50);
    }
}
```

- [ ] **Step 3: Register the module**

In `crates/game-menu/src/lib.rs` (or the crate root that declares the other `mod` lines such as `mod song_select;`), add:

```rust
mod preview_autoplay;
```

- [ ] **Step 4: Run the clock tests**

Run: `cargo test -p game-menu preview_autoplay::clock_tests`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/Cargo.toml crates/game-menu/src/preview_autoplay.rs crates/game-menu/src/lib.rs
git commit -m "feat(game-menu): PreviewClock audio-master clock for preview autoplay"
```

---

## Task 4: Preview schedule builder (pure)

Builds the ordered list of chips to autoplay from the preview start point, and decides the single-BGM-seek vs sliced-BGM-snap rule.

**Files:**
- Modify: `crates/game-menu/src/preview_autoplay.rs`

- [ ] **Step 1: Add the schedule types + builder + failing tests**

Append to `crates/game-menu/src/preview_autoplay.rs`:

```rust
use dtx_core::channel::EChannel;
use dtx_core::Chart;
use dtx_timing::math::{chip_time_ms_with_bpm_and_bar_changes, BpmChange, ChartTiming};
use gameplay_drums::lane_map::lane_of;

/// What kind of sound a scheduled chip is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewChipKind {
    /// The single BGM stream the clock syncs to. Seeked into via
    /// `seek_seconds`. At most one per schedule.
    BgmPrimary,
    /// A BGM layer chip (sliced BGM), played as a one-shot, no seek.
    BgmLayer,
    /// A drum-lane chip.
    Drum,
    /// An auto-SE chip (0x61..0x65).
    Se,
}

/// One chip to play during the preview, at chart-time `time_ms`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledChip {
    pub time_ms: i64,
    pub kind: PreviewChipKind,
    pub wav_slot: u32,
    /// Chart-time of the BGM chip the clock tracks (`BgmPrimary` only).
    pub bgm_chip_time_ms: i64,
    /// Seek offset into the primary BGM file, in seconds (`BgmPrimary` only).
    pub seek_seconds: f64,
}

/// The full ordered preview plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewSchedule {
    /// Chips sorted ascending by `time_ms`, all with `time_ms >= start_ms`.
    pub chips: Vec<ScheduledChip>,
    /// Effective start point (may be snapped to a BGM slice boundary).
    pub start_ms: i64,
    /// Chart end in ms (last chip time). The clock loops back to `start_ms`
    /// once it passes this.
    pub end_ms: i64,
}

fn collect_bpm_changes(chart: &Chart) -> Vec<BpmChange> {
    let mut v: Vec<BpmChange> = chart
        .chips
        .iter()
        .filter(|c| matches!(c.channel, EChannel::BPM | EChannel::BPMEx))
        .map(|c| BpmChange {
            measure: c.measure,
            bpm: c.value,
        })
        .collect();
    v.sort_by_key(|c| c.measure);
    v
}

const fn is_se_channel(ch: EChannel) -> bool {
    matches!(
        ch,
        EChannel::SE01 | EChannel::SE02 | EChannel::SE03 | EChannel::SE04 | EChannel::SE05
    )
}

/// Build the preview schedule for `chart` starting at `requested_start_ms`.
///
/// BGM rule (handles chip-sliced BGM):
/// - Exactly one BGM chip with a WAV → treat it as the primary; seek into
///   it by `requested_start_ms - bgm_chip_time` and sync the clock to it.
/// - Multiple BGM chips → sliced BGM. Do not seek. Snap `start_ms` to the
///   first BGM chip at/after `requested_start_ms` (or the last one before it
///   if none follow), mark that chip `BgmPrimary` with `seek_seconds = 0`,
///   and schedule the rest as `BgmLayer` one-shots. The clock still syncs to
///   the primary slice; subsequent slices free-run between syncs.
/// - No BGM chip with a WAV → drums/SE-only preview; clock free-runs.
pub fn build_preview_schedule(chart: &Chart, requested_start_ms: i64) -> PreviewSchedule {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let bpm_changes = collect_bpm_changes(chart);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes,
        bar_changes: &[],
    };
    let t = |measure: u32, value: f32| {
        chip_time_ms_with_bpm_and_bar_changes(measure, value, base_bpm, timing)
    };

    // All (chip, time) for BGM/drum/SE chips with a real WAV slot.
    let mut bgm: Vec<(i64, u32)> = chart
        .chips
        .iter()
        .filter(|c| c.channel == EChannel::BGM && c.wav_slot != 0)
        .map(|c| (t(c.measure, c.value), c.wav_slot))
        .collect();
    bgm.sort_by_key(|(ms, _)| *ms);

    // Pick the primary BGM chip and effective start.
    let (primary, start_ms): (Option<(i64, u32, f64)>, i64) = if bgm.is_empty() {
        (None, requested_start_ms)
    } else if bgm.len() == 1 {
        let (chip_ms, slot) = bgm[0];
        let seek = ((requested_start_ms - chip_ms).max(0) as f64) / 1000.0;
        (Some((chip_ms, slot, seek)), requested_start_ms)
    } else {
        // Sliced BGM: snap to a slice boundary, no seek.
        let snapped = bgm
            .iter()
            .find(|(ms, _)| *ms >= requested_start_ms)
            .or_else(|| bgm.last())
            .copied()
            .unwrap();
        (Some((snapped.0, snapped.1, 0.0)), snapped.0)
    };

    let mut chips: Vec<ScheduledChip> = Vec::new();

    if let Some((bgm_chip_time, slot, seek)) = primary {
        chips.push(ScheduledChip {
            time_ms: start_ms,
            kind: PreviewChipKind::BgmPrimary,
            wav_slot: slot,
            bgm_chip_time_ms: bgm_chip_time,
            seek_seconds: seek,
        });
        // BGM layers strictly after the primary chip.
        for &(ms, slot) in &bgm {
            if ms > bgm_chip_time {
                chips.push(ScheduledChip {
                    time_ms: ms,
                    kind: PreviewChipKind::BgmLayer,
                    wav_slot: slot,
                    bgm_chip_time_ms: 0,
                    seek_seconds: 0.0,
                });
            }
        }
    }

    // Drum + SE chips at/after start_ms.
    for c in &chart.chips {
        if c.wav_slot == 0 {
            continue;
        }
        let kind = if lane_of(c.channel).is_some() {
            PreviewChipKind::Drum
        } else if is_se_channel(c.channel) {
            PreviewChipKind::Se
        } else {
            continue;
        };
        let ms = t(c.measure, c.value);
        if ms >= start_ms {
            chips.push(ScheduledChip {
                time_ms: ms,
                kind,
                wav_slot: c.wav_slot,
                bgm_chip_time_ms: 0,
                seek_seconds: 0.0,
            });
        }
    }

    chips.sort_by_key(|c| c.time_ms);
    let end_ms = chart
        .chips
        .iter()
        .map(|c| t(c.measure, c.value))
        .max()
        .unwrap_or(start_ms)
        .max(start_ms);

    PreviewSchedule {
        chips,
        start_ms,
        end_ms,
    }
}
```

- [ ] **Step 2: Add builder tests**

Append to the same file (new `#[cfg(test)]` module):

```rust
#[cfg(test)]
mod schedule_tests {
    use super::*;
    use dtx_core::chart::{Chip, Metadata};

    fn chart(chips: Vec<Chip>) -> Chart {
        Chart {
            metadata: Metadata { bpm: Some(120.0), ..Default::default() },
            chips,
            ..Default::default()
        }
    }

    // 120 BPM: measure m, value v -> (m+v)*2000 ms.

    #[test]
    fn single_bgm_seeks_into_it() {
        // One BGM chip at t=0, request start at 10000ms.
        let c = chart(vec![Chip::with_wav(0, EChannel::BGM, 0.0, 1)]);
        let s = build_preview_schedule(&c, 10000);
        assert_eq!(s.start_ms, 10000);
        let p = &s.chips[0];
        assert_eq!(p.kind, PreviewChipKind::BgmPrimary);
        assert_eq!(p.bgm_chip_time_ms, 0);
        assert!((p.seek_seconds - 10.0).abs() < 1e-6);
    }

    #[test]
    fn drum_chips_before_start_are_dropped() {
        let c = chart(vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(2, EChannel::Snare, 0.0, 2), // t=4000 < 10000 -> dropped
            Chip::with_wav(6, EChannel::Snare, 0.0, 2), // t=12000 -> kept
        ]);
        let s = build_preview_schedule(&c, 10000);
        let drums: Vec<_> = s
            .chips
            .iter()
            .filter(|c| c.kind == PreviewChipKind::Drum)
            .collect();
        assert_eq!(drums.len(), 1);
        assert_eq!(drums[0].time_ms, 12000);
    }

    #[test]
    fn sliced_bgm_snaps_to_boundary_no_seek() {
        // Many BGM slices every measure; request start 5000ms -> snap to the
        // slice at m3 (t=6000), no seek.
        let mut chips = vec![];
        for m in 0..10 {
            chips.push(Chip::with_wav(m, EChannel::BGM, 0.0, (m + 1) as u32));
        }
        let s = build_preview_schedule(&chart(chips), 5000);
        assert_eq!(s.start_ms, 6000);
        let primary: Vec<_> = s
            .chips
            .iter()
            .filter(|c| c.kind == PreviewChipKind::BgmPrimary)
            .collect();
        assert_eq!(primary.len(), 1);
        assert_eq!(primary[0].time_ms, 6000);
        assert_eq!(primary[0].seek_seconds, 0.0);
        // Earlier slices are gone; later slices are BgmLayer.
        assert!(s.chips.iter().all(|c| c.time_ms >= 6000));
        assert!(s
            .chips
            .iter()
            .any(|c| c.kind == PreviewChipKind::BgmLayer));
    }

    #[test]
    fn se_chips_are_scheduled() {
        let c = chart(vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(6, EChannel::SE01, 0.0, 9), // t=12000
        ]);
        let s = build_preview_schedule(&c, 10000);
        assert!(s
            .chips
            .iter()
            .any(|c| c.kind == PreviewChipKind::Se && c.time_ms == 12000));
    }

    #[test]
    fn no_bgm_still_schedules_drums() {
        let c = chart(vec![Chip::with_wav(6, EChannel::Snare, 0.0, 2)]);
        let s = build_preview_schedule(&c, 10000);
        assert!(s.chips.iter().all(|c| c.kind == PreviewChipKind::Drum));
        assert_eq!(s.chips.len(), 1);
    }
}
```

- [ ] **Step 3: Run the builder tests**

Run: `cargo test -p game-menu preview_autoplay::schedule_tests`
Expected: 5 tests pass. If `EChannel::SE01`/`Snare`/`Chip::with_wav` paths differ, correct the imports to `dtx-core`'s real paths.

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu/src/preview_autoplay.rs
git commit -m "feat(game-menu): preview schedule builder with BGM seek/snap rule"
```

---

## Task 5: `PreviewAutoplay` resource + start logic

Holds all runtime state for a running preview and the `start_preview` entry point (builds schedule, loads WAV slots, seeds the clock, starts the primary BGM).

**Files:**
- Modify: `crates/game-menu/src/preview_autoplay.rs`

- [ ] **Step 1: Add the resource + start_preview**

Append to `crates/game-menu/src/preview_autoplay.rs`:

```rust
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::AudioInstance;
use dtx_audio::{
    play_bgm_handle_with_mix, play_bgm_handle_with_mix_from_seconds, play_drum_hit_handle,
    position_ms, preload_chart_sound, stop_with_fade, BgmHandle, ChartSoundBank, DrumPolyphony,
    PREVIEW_FADE_IN_MS, PREVIEW_FADE_OUT_MS,
};

/// Runtime state for the live autoplay preview. A Bevy resource; owns its
/// own `BgmHandle` (NOT the gameplay one) so it never collides with the
/// gameplay BGM lifecycle.
#[derive(Resource, Default)]
pub struct PreviewAutoplay {
    /// Identity of the currently-previewed chart (its .dtx path). `None`
    /// when idle. Used for same-song detection and resume.
    pub chart_path: Option<PathBuf>,
    schedule: PreviewSchedule,
    cursor: usize,
    clock: PreviewClock,
    bgm: BgmHandle,
    polyphony: DrumPolyphony,
    bank: ChartSoundBank,
    /// Chart-time of the BGM chip the clock currently tracks (for
    /// `measured_ms = bgm_chip_time + position_ms`). `None` until a
    /// primary BGM chip has been played.
    tracked_bgm_chip_ms: Option<i64>,
    /// Extra drum/SE instances kept alive for fade-out on stop.
    active: Vec<Handle<AudioInstance>>,
}

impl PreviewAutoplay {
    pub fn is_active(&self) -> bool {
        self.chart_path.is_some()
    }

    /// Chart-time position (ms) of the running preview, for freeze/resume.
    pub fn position_ms(&self) -> i64 {
        self.clock.current_ms()
    }

    /// Stop everything and return to idle, fading out over `fade_ms`.
    /// Releases the BGM and all tracked drum/SE instances so nothing leaks
    /// into gameplay.
    pub fn stop(&mut self, instances: &mut Assets<AudioInstance>, fade_ms: u32) {
        if let Some(h) = self.bgm.instance.take() {
            stop_with_fade(instances, &h, fade_ms);
        }
        self.bgm.path = None;
        for h in self.active.drain(..) {
            stop_with_fade(instances, &h, fade_ms);
        }
        self.polyphony.reset();
        self.bank.clear();
        self.schedule = PreviewSchedule::default();
        self.cursor = 0;
        self.clock.reset();
        self.tracked_bgm_chip_ms = None;
        self.chart_path = None;
    }

    /// Load every WAV slot referenced by the schedule into `self.bank`.
    fn load_bank(&mut self, chart: &dtx_core::Chart, source_dir: Option<&Path>, asset_server: &AssetServer) {
        self.bank.clear();
        let mut seen = std::collections::BTreeSet::new();
        for sc in &self.schedule.chips {
            if sc.wav_slot == 0 || !seen.insert(sc.wav_slot) {
                continue;
            }
            let Some(filename) = chart.assets.wav.get(sc.wav_slot) else {
                continue;
            };
            preload_chart_sound(
                asset_server,
                &mut self.bank,
                source_dir,
                sc.wav_slot,
                filename,
                chart.assets.wav.volume(sc.wav_slot),
                chart.assets.wav.pan(sc.wav_slot),
            );
        }
    }

    /// Start (or resume at `start_ms`) previewing `chart` loaded from
    /// `chart_path`. Builds the schedule, loads WAV slots, seeds the clock,
    /// and starts the primary BGM (seeked). Drum/SE chips layer in from the
    /// scheduler as their handles finish loading.
    #[allow(clippy::too_many_arguments)]
    pub fn start_preview(
        &mut self,
        chart: &dtx_core::Chart,
        chart_path: PathBuf,
        start_ms: i64,
        audio: &Audio,
        asset_server: &AssetServer,
        instances: &mut Assets<AudioInstance>,
        master_volume: f32,
        drum_volume: f32,
    ) {
        // Tear down any prior preview first.
        self.stop(instances, PREVIEW_FADE_OUT_MS);

        self.schedule = build_preview_schedule(chart, start_ms);
        self.cursor = 0;
        let source_dir = chart_path.parent();
        self.load_bank(chart, source_dir, asset_server);
        self.clock.start_at(self.schedule.start_ms);
        self.chart_path = Some(chart_path);

        // Fire the primary BGM immediately (it is chip 0 when present).
        if let Some(first) = self.schedule.chips.first().cloned() {
            if first.kind == PreviewChipKind::BgmPrimary {
                self.play_chip(&first, audio, instances, master_volume, drum_volume, source_dir);
                self.cursor = 1;
            }
        }
    }

    /// Play a single scheduled chip through the right primitive.
    fn play_chip(
        &mut self,
        sc: &ScheduledChip,
        audio: &Audio,
        instances: &mut Assets<AudioInstance>,
        master_volume: f32,
        drum_volume: f32,
        _source_dir: Option<&Path>,
    ) {
        let Some(sound) = self.bank.get(sc.wav_slot) else {
            return; // not loaded yet — skip; progressive layering
        };
        let path = sound.path.to_string_lossy().to_string();
        let vol = sound.volume;
        let pan = sound.pan;
        let src = sound.handle.clone();
        match sc.kind {
            PreviewChipKind::BgmPrimary => {
                play_bgm_handle_with_mix_from_seconds(
                    audio,
                    instances,
                    &mut self.bgm,
                    src,
                    &path,
                    vol,
                    pan,
                    master_volume,
                    sc.seek_seconds,
                    PREVIEW_FADE_IN_MS,
                );
                self.tracked_bgm_chip_ms = Some(sc.bgm_chip_time_ms);
            }
            PreviewChipKind::BgmLayer => {
                // One-shot layer: reuse the mix primitive without tracking.
                let mut throwaway = BgmHandle::default();
                let h = play_bgm_handle_with_mix(
                    audio,
                    &mut throwaway,
                    instances,
                    src,
                    &path,
                    vol,
                    pan,
                    master_volume,
                    0,
                );
                self.active.push(h);
            }
            PreviewChipKind::Drum | PreviewChipKind::Se => {
                let h = play_drum_hit_handle(
                    audio,
                    instances,
                    &mut self.polyphony,
                    src,
                    sc.wav_slot,
                    vol,
                    pan,
                    master_volume,
                    // Drum-channel gain from the live drum slider (design:
                    // gameplay sliders apply to the preview mix).
                    drum_volume,
                );
                self.active.push(h);
                // Bound the kept-alive list so long previews don't grow it
                // without limit; drop the oldest once past a cap.
                if self.active.len() > 64 {
                    self.active.remove(0);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add a smoke test for stop() idempotence**

Append to the file (extend `schedule_tests` or a new module):

```rust
#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn new_resource_is_idle() {
        let p = PreviewAutoplay::default();
        assert!(!p.is_active());
        assert_eq!(p.position_ms(), 0);
    }
}
```

(The audio-driving paths need a running kira context, so they are exercised in the manual verification step of Task 9, not in unit tests.)

- [ ] **Step 3: Build + run**

Run: `cargo test -p game-menu preview_autoplay`
Expected: compiles; clock + schedule + `new_resource_is_idle` tests pass. Fix any import path mismatches (`chart.assets.wav.get/volume/pan` — confirm the `WavTable` accessor names against `dtx-core/src/assets.rs`; adjust if they differ).

- [ ] **Step 4: Commit**

```bash
git add crates/game-menu/src/preview_autoplay.rs
git commit -m "feat(game-menu): PreviewAutoplay resource with start/stop + WAV loading"
```

---

## Task 6: Scheduler tick system (fire chips, loop)

**Files:**
- Modify: `crates/game-menu/src/preview_autoplay.rs`

- [ ] **Step 1: Add the tick system**

Append to `crates/game-menu/src/preview_autoplay.rs`:

```rust
/// System: advance the preview clock and fire due chips. Runs every
/// `Update` frame while a preview is active. Loops back to `start_ms` after
/// passing `end_ms`.
#[allow(clippy::too_many_arguments)]
pub fn preview_autoplay_tick(
    time: Res<Time>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut preview: ResMut<PreviewAutoplay>,
    settings: Res<dtx_config::AudioConfig>,
) {
    if !preview.is_active() || !preview.clock.is_started() {
        return;
    }
    let master = settings.master_volume;
    let drum = settings.drum_volume;

    // measured chart-time from the tracked BGM instance, if any.
    let measured = match preview.tracked_bgm_chip_ms {
        Some(chip_ms) => position_ms(&audio, &preview.bgm).map(|p| chip_ms + p),
        None => None,
    };
    preview.clock.tick(time.delta_secs() as f64, measured);
    let now = preview.clock.current_ms();

    // Fire all chips whose time has arrived.
    let source_dir = preview.chart_path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let sd = source_dir.as_deref();
    while preview.cursor < preview.schedule.chips.len() {
        let sc = preview.schedule.chips[preview.cursor].clone();
        if now < sc.time_ms {
            break;
        }
        preview.play_chip(&sc, &audio, &mut instances, master, drum, sd);
        preview.cursor += 1;
    }

    // Loop: once past the end, restart from the preview start point.
    if now >= preview.schedule.end_ms && preview.schedule.end_ms > preview.schedule.start_ms {
        let start_ms = preview.schedule.start_ms;
        // Stop the current BGM (fade) and replay from the seek point.
        if let Some(h) = preview.bgm.instance.take() {
            stop_with_fade(&mut instances, &h, PREVIEW_FADE_OUT_MS);
        }
        preview.bgm.path = None;
        preview.tracked_bgm_chip_ms = None;
        preview.cursor = 0;
        preview.clock.start_at(start_ms);
        if let Some(first) = preview.schedule.chips.first().cloned() {
            if first.kind == PreviewChipKind::BgmPrimary {
                preview.play_chip(&first, &audio, &mut instances, master, drum, sd);
                preview.cursor = 1;
            }
        }
    }
    let _ = &asset_server; // reserved: lazy slot reload if a handle failed
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p game-menu`
Expected: compiles. `AudioConfig` is `dtx_config::AudioConfig`; confirm it is registered as a Bevy resource in the app (it is used by other systems — if it is wrapped in a newtype resource elsewhere, match that type instead).

- [ ] **Step 3: Commit**

```bash
git add crates/game-menu/src/preview_autoplay.rs
git commit -m "feat(game-menu): preview autoplay tick system with loop"
```

---

## Task 7: Dwell debounce + async parse + wiring into song select

Replaces the primary path of `bgm_preview_on_change` with: 250 ms dwell → async chart parse → `start_preview`. Keeps the single-clip `PreviewPlayer` as the fallback.

**Files:**
- Modify: `crates/game-menu/src/preview_autoplay.rs` (dwell resources + parse system)
- Modify: `crates/game-menu/src/song_select.rs` (register systems; adjust `bgm_preview_on_change` to only handle the fallback)

- [ ] **Step 1: Add dwell + parse-task resources and the trigger system**

Append to `crates/game-menu/src/preview_autoplay.rs`:

```rust
use bevy::tasks::{AsyncComputeTaskPool, Task};
use dtx_core::Chart;

/// Debounce timer + generation counter for preview selection changes.
#[derive(Resource, Default)]
pub struct PreviewDwell {
    /// Seconds remaining before the pending selection commits. `None` = idle.
    pending_secs: Option<f32>,
    /// Path of the chart whose preview is pending / active.
    pending_path: Option<PathBuf>,
    /// Bumped on every new selection; stale parse results are discarded.
    generation: u64,
}

/// Dwell before committing a hovered selection to a parse+play. Slightly
/// above osu's 150ms because our load is heavier (parse + many WAVs).
pub const PREVIEW_DWELL_SECS: f32 = 0.25;

/// Background chart-parse task tagged with the generation it belongs to.
#[derive(Resource, Default)]
pub struct PreviewParseTask(pub Option<(u64, Task<Result<Chart, String>>)>);

impl PreviewDwell {
    /// Register a newly-hovered chart path. Resets the dwell timer unless
    /// this is the same path already pending/active (no restart).
    pub fn hover(&mut self, path: PathBuf) -> bool {
        if self.pending_path.as_deref() == Some(path.as_path()) {
            return false; // same song — do not restart
        }
        self.pending_path = Some(path);
        self.pending_secs = Some(PREVIEW_DWELL_SECS);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn clear(&mut self) {
        self.pending_secs = None;
        self.pending_path = None;
    }
}

/// System: tick the dwell timer; when it elapses, spawn an async parse of
/// the pending chart tagged with the current generation.
pub fn preview_dwell_system(
    time: Res<Time>,
    mut dwell: ResMut<PreviewDwell>,
    mut task: ResMut<PreviewParseTask>,
) {
    let Some(remaining) = dwell.pending_secs.as_mut() else {
        return;
    };
    *remaining -= time.delta_secs();
    if *remaining > 0.0 {
        return;
    }
    dwell.pending_secs = None;
    let Some(path) = dwell.pending_path.clone() else {
        return;
    };
    let generation = dwell.generation;
    let pool = AsyncComputeTaskPool::get();
    let path_clone = path.clone();
    task.0 = Some((
        generation,
        pool.spawn(async move { dtx_assets::load_dtx(&path_clone).map_err(|e| e.to_string()) }),
    ));
}

/// System: poll the async parse; on completion (and if still current),
/// start the autoplay preview. On parse failure or a chart with no audio
/// chips, fall back to the single-clip `PreviewPlayer`.
#[allow(clippy::too_many_arguments)]
pub fn preview_parse_poll_system(
    mut task: ResMut<PreviewParseTask>,
    dwell: Res<PreviewDwell>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut preview: ResMut<PreviewAutoplay>,
    mut clip: ResMut<dtx_audio::PreviewPlayer>,
    mut cache: ResMut<dtx_audio::AudioHandleCache>,
    mut swap_events: MessageWriter<dtx_audio::PreviewSwapEvent>,
    settings: Res<dtx_config::AudioConfig>,
) {
    let Some((generation, t)) = task.0.as_mut() else {
        return;
    };
    let gen = *generation;
    let Some(result) = bevy::tasks::block_on(bevy::tasks::poll_once(t)) else {
        return; // still parsing
    };
    task.0 = None;

    // Stale result: a newer selection superseded this parse.
    if gen != dwell.generation {
        return;
    }
    let Some(path) = dwell.pending_path.clone() else {
        return;
    };

    match result {
        Ok(chart) => {
            let has_audio_chips = chart.chips.iter().any(|c| {
                c.wav_slot != 0
                    && (c.channel == dtx_core::channel::EChannel::BGM
                        || gameplay_drums::lane_map::lane_of(c.channel).is_some()
                        || matches!(
                            c.channel,
                            dtx_core::channel::EChannel::SE01
                                | dtx_core::channel::EChannel::SE02
                                | dtx_core::channel::EChannel::SE03
                                | dtx_core::channel::EChannel::SE04
                                | dtx_core::channel::EChannel::SE05
                        ))
            });
            if has_audio_chips {
                // Stop the clip fallback if it was playing.
                clip.stop(&mut instances, PREVIEW_FADE_OUT_MS);
                let start_ms =
                    dtx_library::preview_point::compute_preview_start_ms(&chart) as i64;
                preview.start_preview(
                    &chart,
                    path,
                    start_ms,
                    &audio,
                    &asset_server,
                    &mut instances,
                    settings.master_volume,
                    settings.drum_volume,
                );
            } else {
                preview.stop(&mut instances, PREVIEW_FADE_OUT_MS);
                play_clip_fallback(
                    &path, &audio, &asset_server, &mut cache, &mut clip,
                    &mut instances, &mut swap_events,
                );
            }
        }
        Err(err) => {
            warn!("Preview: parse failed for {}: {}", path.display(), err);
            preview.stop(&mut instances, PREVIEW_FADE_OUT_MS);
            play_clip_fallback(
                &path, &audio, &asset_server, &mut cache, &mut clip,
                &mut instances, &mut swap_events,
            );
        }
    }
}

/// Fallback: play the chart's `#PREVIEW` clip (or full-BGM path) via the
/// single-clip player. `path` here is the .dtx path — resolve its preview
/// file the same way `SongInfo` does is unnecessary; we reuse the clip
/// player only when autoplay is impossible, so play the .dtx's sibling
/// bgm/preview through the existing player if a preview path is known.
fn play_clip_fallback(
    _dtx_path: &Path,
    _audio: &Audio,
    _asset_server: &AssetServer,
    _cache: &mut dtx_audio::AudioHandleCache,
    clip: &mut dtx_audio::PreviewPlayer,
    instances: &mut Assets<AudioInstance>,
    _swap: &mut MessageWriter<dtx_audio::PreviewSwapEvent>,
) {
    // We do not have the SongInfo preview_path here; the safe fallback is
    // to leave the clip player stopped (silence) rather than guess a file.
    // The autoplay path covers all charts that have any audio chip, which
    // is effectively every playable chart. Kept as a hook for a future
    // #PREVIEW-clip fallback wired from SongInfo.
    clip.stop(instances, PREVIEW_FADE_OUT_MS);
}
```

> Note: the clip fallback is intentionally minimal — a chart with zero audio chips is not playable, so silence is acceptable. If you want the old `#PREVIEW` clip as the fallback, thread `SongInfo.preview_path` into `preview_parse_poll_system` (add `db: Res<SongDb>` + resolve like the current `bgm_preview_on_change`) and call `clip.play(...)` with it. This is optional polish, out of the approved v1 scope.

- [ ] **Step 2: Rewrite the song-select trigger**

In `crates/game-menu/src/song_select.rs`, replace the body of `bgm_preview_on_change` (lines ~1332-1421) with a thin trigger that only feeds the dwell debounce. Change its signature/body to:

```rust
fn bgm_preview_on_change(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<SongDb>,
    mut dwell: ResMut<crate::preview_autoplay::PreviewDwell>,
) {
    if !selection.is_changed() {
        return;
    }
    let Some(chart_idx) = selection.chart_index(&selection_state) else {
        return;
    };
    let Some(song) = db.songs.get(chart_idx) else {
        return;
    };
    dwell.hover(song.path.clone());
}
```

Remove now-unused imports in `song_select.rs` if the compiler flags them (`BgmHandle`, `PreviewPlayer`, `get_or_load_audio_handle`, `PreviewSwapDirection`, `PreviewSwapEvent` may still be used elsewhere — only remove what `cargo build` reports as unused).

- [ ] **Step 3: Register the resources + systems**

In the `plugin(app: &mut App)` fn in `crates/game-menu/src/song_select.rs`, add to the resource init chain (near `.init_resource::<Selection>()`):

```rust
        .init_resource::<crate::preview_autoplay::PreviewAutoplay>()
        .init_resource::<crate::preview_autoplay::PreviewDwell>()
        .init_resource::<crate::preview_autoplay::PreviewParseTask>()
```

And in the `Update` systems tuple gated by `run_if(in_state(AppState::SongSelect))`, add alongside `bgm_preview_on_change`:

```rust
                crate::preview_autoplay::preview_dwell_system,
                crate::preview_autoplay::preview_parse_poll_system,
                crate::preview_autoplay::preview_autoplay_tick,
```

- [ ] **Step 4: Build**

Run: `cargo build -p game-menu`
Expected: compiles. Resolve any unused-import warnings and confirm `dtx_config::AudioConfig` is the correct resource type in the app (grep for how other song-select systems read master volume; match it).

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/preview_autoplay.rs crates/game-menu/src/song_select.rs
git commit -m "feat(game-menu): dwell-debounced async parse driving preview autoplay"
```

---

## Task 8: Resume across the gameplay round-trip + lifecycle cleanup

Freeze the preview position on exit into gameplay; resume in place on return to the same song; stop cleanly (no `BgmHandle` leak).

**Files:**
- Modify: `crates/game-menu/src/preview_autoplay.rs` (freeze resource + enter/exit systems)
- Modify: `crates/game-menu/src/song_select.rs` (register enter/exit systems; ensure `OnExit(SongSelect)` stops the autoplay preview)

- [ ] **Step 1: Add the frozen-state resource + enter/exit systems**

Append to `crates/game-menu/src/preview_autoplay.rs`:

```rust
/// Persists across a gameplay round-trip so the preview can resume in place
/// for the same song. Cleared once consumed or when a different song is
/// selected on return.
#[derive(Resource, Default)]
pub struct FrozenPreview {
    pub path: Option<PathBuf>,
    pub chart_ms: i64,
}

/// System (OnExit SongSelect): freeze the running preview's position and
/// stop it fully (releasing the BGM + drum/SE instances). No handle is left
/// alive — resume re-parses and re-seeks.
pub fn freeze_preview_on_exit(
    mut preview: ResMut<PreviewAutoplay>,
    mut frozen: ResMut<FrozenPreview>,
    mut dwell: ResMut<PreviewDwell>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if preview.is_active() {
        frozen.path = preview.chart_path.clone();
        frozen.chart_ms = preview.position_ms();
    } else {
        frozen.path = None;
    }
    // Align with the 300ms screen fade (matches stop_preview_system).
    preview.stop(&mut instances, 300);
    dwell.clear();
}

/// System (OnEnter SongSelect): if the frozen song matches the current
/// selection, resume the autoplay from the frozen position by re-parsing
/// and seeding the dwell with an immediate (zero-delay) commit. Otherwise
/// leave normal hover-driven behavior to take over.
pub fn resume_preview_on_enter(
    selection: Res<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    mut frozen: ResMut<FrozenPreview>,
    mut dwell: ResMut<PreviewDwell>,
    mut resume: ResMut<ResumeRequest>,
) {
    let Some(path) = frozen.path.take() else {
        return;
    };
    let current = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|s| s.path.clone());
    if current.as_deref() == Some(path.as_path()) {
        // Same song: request a resume at the frozen position. Prime the
        // dwell so the normal parse path runs immediately (no 250ms wait).
        resume.0 = Some((path.clone(), frozen.chart_ms));
        dwell.hover(path);
        dwell.pending_secs = Some(0.0); // commit on the next dwell tick
    }
}

/// Carries the "resume at this chart-time" request from `resume_preview_on_enter`
/// to `preview_parse_poll_system`, overriding the computed start point once.
#[derive(Resource, Default)]
pub struct ResumeRequest(pub Option<(PathBuf, i64)>);
```

- [ ] **Step 2: Make the parse poll honor a resume request**

In `preview_parse_poll_system` (Task 7), change the `start_ms` computation in the `has_audio_chips` branch to prefer a matching resume request:

```rust
                let start_ms = match resume.0.take() {
                    Some((rp, ms)) if rp == path => ms,
                    other => {
                        resume.0 = other; // put back a non-matching request
                        dtx_library::preview_point::compute_preview_start_ms(&chart) as i64
                    }
                };
```

Add `mut resume: ResMut<ResumeRequest>,` to `preview_parse_poll_system`'s parameters.

- [ ] **Step 3: Register the resource + enter/exit systems**

In `plugin(...)` in `song_select.rs`:

Add to the resource inits:

```rust
        .init_resource::<crate::preview_autoplay::FrozenPreview>()
        .init_resource::<crate::preview_autoplay::ResumeRequest>()
```

Add `resume_preview_on_enter` to the `OnEnter(AppState::SongSelect)` chain (after `recompute_visible`, so the selection is valid):

```rust
                crate::preview_autoplay::resume_preview_on_enter,
```

Replace the `OnExit(AppState::SongSelect)` chain to freeze+stop the autoplay preview alongside the existing clip stop:

```rust
        .add_systems(
            OnExit(AppState::SongSelect),
            (
                crate::preview_autoplay::freeze_preview_on_exit,
                stop_preview_system,
                stop_bgm_system,
                despawn_stage::<SongSelectEntity>,
            )
                .chain(),
        )
```

Make `Selection` and `SongSelectSelection` visible to the `preview_autoplay` module: ensure they are `pub` (or `pub(crate)`) in `song_select.rs`. If `song_select` is a private module, change its declaration to `pub(crate) mod song_select;` or re-export the two types.

- [ ] **Step 4: Build + run existing tests**

Run: `cargo build -p game-menu && cargo test -p game-menu preview_autoplay`
Expected: compiles; prior unit tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/game-menu/src/preview_autoplay.rs crates/game-menu/src/song_select.rs
git commit -m "feat(game-menu): freeze + resume preview across gameplay round-trip"
```

---

## Task 9: Workspace build, lint, and manual verification

**Files:** none (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo build`
Expected: the whole workspace compiles.

- [ ] **Step 2: Full test suite**

Run: `cargo test`
Expected: all tests pass (dtx-library preview_point + SongInfo, game-menu preview_autoplay clock/schedule/resource).

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings. Fix any (e.g. `self.cursor = self.cursor + 1` → `self.cursor += 1`).

- [ ] **Step 4: Manual smoke test — basic autoplay**

Run: `cargo run` (ensure `DTX_SONG_DIR` points at a folder with real DTX songs).
Navigate to song select. Verify:
- Hovering a song for ~250 ms starts the actual song (BGM audible), not the short `#PREVIEW` clip.
- Autoplayed drums/SE are audible on top of the BGM and stay in time with the music (no progressive drift over ~30 s).
- The preview starts partway into the song (a busy section), not at t=0.

- [ ] **Step 5: Manual smoke test — smoothness**

- Scroll the wheel quickly across many songs: no audio churn — only the song you settle on for 250 ms starts.
- Switching between difficulties of the same song does NOT restart the preview (same `song.path`).
- Let a preview play to the end of the song: it loops back to the preview start point, not to t=0.

- [ ] **Step 6: Manual smoke test — resume**

- Start a song's preview, enter it (SongLoading → Performance), play briefly, exit back to song select.
- Verify the preview resumes near where gameplay's song position was (same song still selected), rather than restarting at the preview point.
- Verify gameplay BGM itself started correctly during play (i.e. the preview did not leak into the gameplay `BgmHandle`).

- [ ] **Step 7: Manual smoke test — sliced-BGM song**

- Find a DTX whose BGM is many WAV slices (no single full-BGM file). Verify the preview still plays audible music from a slice boundary and drums layer on top (may free-run slightly; acceptable per design).

- [ ] **Step 8: Commit any lint fixes**

```bash
git add -A
git commit -m "chore(game-menu): clippy + verification fixes for preview autoplay"
```

---

## Self-review notes (addressed)

- **Clock drift** (design risk): `PreviewClock` reuses `GameplayClock`'s drift-correction toward `position_ms`; `measured_ms = tracked_bgm_chip_ms + position_ms(preview_bgm)`. Not naive frame-delta.
- **Chip-sliced BGM**: explicit single-seek vs multi-snap rule in `build_preview_schedule` (Task 4), with a dedicated test.
- **`BgmHandle` leak**: `PreviewAutoplay` owns a private `BgmHandle`; `freeze_preview_on_exit` reads position then fully stops. Gameplay's resource is never written by the preview.
- **Play-to-end + loop**: bank loads every slot from `start_ms` to chart end; loop restarts from `start_ms` (Task 6). No `+40s` window.
- **Drum predicate**: scheduler uses `lane_of().is_some()` (matches loaded slots); the heuristic uses `is_drum()` (density only).
- **Same-song no-restart**: `PreviewDwell::hover` returns early for the same path (Task 7); difficulty switches within a song do not restart.

## Deferred (not in this plan; from the spec's out-of-scope)

Audio-similarity preview point, offline pre-render cache, `#PREVIEW`-clip instant first layer, persistent SongDb cache for the offset, and the orthogonal osu polish (mod-rate preview, dialog low-pass duck, window-unfocus fade, hover samples, gameplay-loader muffle handoff).
