# Practice Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Practice mode v1 — seek anywhere via scrub bar, A/B loop, playback rate, per-attempt section stats — built on a shared seek-engine primitive.

**Architecture:** Approach A from the approved spec (`docs/superpowers/specs/2026-07-06-practice-mode-design.md`): a `PracticeSession` resource flags the existing gameplay stage into practice; a public `SeekToChartTime` Bevy message is the single seek entry point consumed by one engine-side system that reseeds skip-sets, despawns notes, restarts BGM at offset, and jumps `GameplayClock`. Practice UI/loop/stats live in a new `practice/` module and only send messages / read resources.

**Tech Stack:** Rust, Bevy 0.19 (messages via `add_message`/`MessageReader`), bevy_kira_audio 0.26 (static sounds; `start_from`, per-instance and per-channel `set_playback_rate`), existing crates: gameplay-drums, dtx-core, dtx-timing, dtx-audio, dtx-ui, game-shell, game-menu, game-results.

---

## File Structure

Create:
- `crates/gameplay-drums/src/timeline.rs` — `ChipTimeline` resource: per-chip times (both timebases), timing-line times, bar/beat snap points, BGM chip list, density buckets, snap resolution. Pure data, one build on stage enter.
- `crates/gameplay-drums/src/seek.rs` — `SeekToChartTime` message, skip-set seeding (pure fn), `apply_seek_system`, `PendingBgmStart` + `start_pending_bgm`.
- `crates/gameplay-drums/src/practice/mod.rs` — practice plugin, session insert/remove, gauge freeze.
- `crates/gameplay-drums/src/practice/session.rs` — `PracticeSession`, `LoopRegion`, `PrerollSetting`, `AttemptStats`, `AttemptRecord`, rate stepping.
- `crates/gameplay-drums/src/practice/ab_loop.rs` — loop watcher.
- `crates/gameplay-drums/src/practice/stats.rs` — attempt lifecycle + judgment accumulation.
- `crates/gameplay-drums/src/practice/rate.rs` — audio-rate application.
- `crates/gameplay-drums/src/practice/ui.rs` — transport strip + practice pause panel.
- `crates/dtx-ui/src/widget/density_strip.rs` — reusable time-density widget helpers.
- `crates/gameplay-drums/tests/practice_mode.rs` — integration tests.

Modify:
- `crates/gameplay-drums/src/resources.rs` — `GameplayClock::seek`, `AudioRate` resource.
- `crates/gameplay-drums/src/lib.rs` — module decls, resource/message registration, system wiring, rate-scaled clock tick.
- `crates/gameplay-drums/src/orchestrator.rs` — end-of-stage suppression inside an active loop region.
- `crates/gameplay-drums/src/pause.rs` — normal pause overlay/input skipped in practice.
- `crates/game-shell/src/states.rs` + `crates/game-shell/src/lib.rs` — `PracticeIntent` resource.
- `crates/game-menu/src/song_select.rs` — Shift+Enter practice entry.
- `crates/game-results/src/lib.rs` — skip score persistence in practice.
- `crates/dtx-ui/src/widget/mod.rs` — register `density_strip`.

Conventions to follow (verified in codebase):
- Messages: `#[derive(Message)]` + `app.add_message::<T>()` + `MessageReader`/`MessageWriter` (see `crates/gameplay-drums/src/events.rs` usage in `lib.rs:107-110`).
- System ordering: `DrumsSets` (`lib.rs:64-70`), enter chain wrapped in `orchestrator::DrumsEnterSet`.
- Keep Bevy system parameter counts ≤ ~16 (trait-solver ceiling documented at `orchestrator.rs:75-81`); use `#[derive(SystemParam)]` bundles where needed.
- No AI co-authors in commits. Commit messages follow repo style (`feat(gameplay-drums): ...`).

---

### Task 1: `GameplayClock::seek`

**Files:**
- Modify: `crates/gameplay-drums/src/resources.rs` (impl `GameplayClock`, after `sync` at ~line 544)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `resources.rs`:

```rust
#[test]
fn seek_snaps_all_clock_fields() {
    let mut clock = GameplayClock::default();
    clock.start_wall_clock();
    clock.tick(1.0 / 60.0, Some(0));
    for _ in 0..30 {
        clock.tick(1.0 / 60.0, None);
    }

    clock.seek(42_000);

    assert_eq!(clock.current_ms, 42_000);
    assert!((clock.visual_ms() - 42_000.0).abs() < f64::EPSILON);
    assert!((clock.prev_visual_ms() - 42_000.0).abs() < f64::EPSILON);
}

#[test]
fn seek_rearms_first_audio_snap() {
    let mut clock = GameplayClock::default();
    clock.start_audio_required();
    clock.tick(1.0 / 60.0, Some(10_000));
    assert!(!clock.is_waiting_for_audio());

    clock.seek(42_000);

    // Next measured position snaps the clock (no bounded drift fight),
    // exactly like the first observation after stage start.
    clock.tick(1.0 / 60.0, Some(42_180));
    assert_eq!(clock.current_ms, 42_180);
}

#[test]
fn seek_on_unstarted_clock_is_noop() {
    let mut clock = GameplayClock::default();
    clock.seek(5_000);
    assert_eq!(clock.current_ms, 0);
    assert!(!clock.is_started());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums seek_ -- --nocapture`
Expected: FAIL with "no method named `seek` found"

- [ ] **Step 3: Implement `seek`**

Add to `impl GameplayClock` directly after `sync`:

```rust
    /// Jump the clock to `ms` and keep running. Re-arms the first-audio
    /// snap (`audio_synced = false`) so the next measured BGM position —
    /// from the restarted stream — snaps the clock instead of being
    /// dragged in by the bounded drift corrector.
    pub fn seek(&mut self, ms: i64) {
        if !self.started {
            return;
        }
        self.current_ms = ms;
        self.audio_ms = ms as f64;
        self.visual_ms = ms as f64;
        self.prev_visual_ms = ms as f64;
        self.audio_synced = false;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gameplay-drums seek_`
Expected: 3 passed

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/resources.rs
git commit -m "feat(gameplay-drums): GameplayClock::seek jump primitive"
```

---

### Task 2: `PracticeIntent` resource in game-shell

**Files:**
- Modify: `crates/game-shell/src/states.rs` (append), `crates/game-shell/src/lib.rs:12,20-22`

- [ ] **Step 1: Add the resource**

Append to `crates/game-shell/src/states.rs`:

```rust
/// Set by song select when the player chooses Practice instead of a
/// normal play; read on Performance enter to insert the practice
/// session. Lives in game-shell so game-menu doesn't need gameplay
/// internals to request practice.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PracticeIntent(pub bool);
```

- [ ] **Step 2: Export + init**

In `crates/game-shell/src/lib.rs` change the re-export line and plugin body:

```rust
pub use states::{despawn_stage, AppState, EGameMode, PauseState, PracticeIntent, StageEntity};
```

and inside `GameShellPlugin::build`, after `.init_state::<PauseState>()`:

```rust
            .init_resource::<states::PracticeIntent>()
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p game-shell`
Expected: clean

- [ ] **Step 4: Commit**

```bash
git add crates/game-shell/src/states.rs crates/game-shell/src/lib.rs
git commit -m "feat(game-shell): PracticeIntent resource"
```

---

### Task 3: `ChipTimeline` resource

**Files:**
- Create: `crates/gameplay-drums/src/timeline.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module decl + init + build system)

Purpose: one precomputed, binary-searchable view of the chart used by seek seeding, snap resolution, loop watcher, attempt stats, and the density strip.

- [ ] **Step 1: Write the failing tests**

Create `crates/gameplay-drums/src/timeline.rs` with the tests first (module skeleton so the file parses):

```rust
//! Precomputed chart timeline for seek/practice: per-chip times in both
//! timebases, timing-line times, snap points, BGM chip list, density.

use bevy::prelude::*;
use dtx_core::beat_lines::TimingLineKind;
use dtx_core::{Chart, EChannel};
use dtx_timing::math::ChartTiming;

use crate::judge::{
    auto_chip_target_ms, chip_target_ms, BarLengthChangeList, BpmChangeList,
};
use crate::lane_map::lane_of;

pub const DENSITY_BUCKETS: usize = 128;

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::assets::DtxAssets;
    use dtx_core::chart::{Chip, Metadata};

    // 120 BPM, 4/4: one measure = 2000ms, one beat = 500ms.
    fn test_chart() -> Chart {
        let mut assets = DtxAssets::default();
        assets.wav.insert(1, "bgm_a.ogg".into());
        assets.wav.insert(2, "bgm_b.ogg".into());
        assets.wav.insert(3, "snare.ogg".into());
        Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),   // slice A @ 0ms
                Chip::with_wav(4, EChannel::BGM, 0.0, 2),   // slice B @ 8000ms
                Chip::new(0, EChannel::BassDrum, 0.0),      // 0ms
                Chip::new(1, EChannel::Snare, 0.5),         // 3000ms
                Chip::new(6, EChannel::BassDrum, 0.0),      // 12000ms
            ],
            assets,
            ..Default::default()
        }
    }

    fn build(chart: &Chart) -> ChipTimeline {
        let bpm = BpmChangeList::from_chart(chart);
        let bar = BarLengthChangeList::from_chart(chart);
        ChipTimeline::from_chart(chart, &bpm, &bar, 0, 14_000)
    }

    #[test]
    fn entries_sorted_and_indexable() {
        let chart = test_chart();
        let tl = build(&chart);
        assert_eq!(tl.entries.len(), chart.chips.len());
        assert!(tl.entries.windows(2).all(|w| w[0].judge_ms <= w[1].judge_ms));
        // idx→time lookup covers every chip.
        assert_eq!(tl.judge_ms_by_idx.len(), chart.chips.len());
        assert_eq!(tl.judge_ms_by_idx[3], 3000);
    }

    #[test]
    fn bar_snap_floors_to_bar_start() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(3_100, SnapDivisor::Bar), 2_000);
        assert_eq!(tl.resolve_snap(1_999, SnapDivisor::Bar), 0);
    }

    #[test]
    fn beat_snap_floors_to_beat() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(3_100, SnapDivisor::Beat), 3_000);
        assert_eq!(tl.resolve_snap(2_600, SnapDivisor::Beat), 2_500);
    }

    #[test]
    fn snap_clamps_into_chart_range() {
        let tl = build(&test_chart());
        assert_eq!(tl.resolve_snap(-500, SnapDivisor::Bar), 0);
        assert!(tl.resolve_snap(99_999, SnapDivisor::Bar) <= tl.end_ms);
    }

    #[test]
    fn governing_bgm_chip_picks_last_at_or_before() {
        let tl = build(&test_chart());
        assert_eq!(tl.governing_bgm_chip(0), Some((0, 0)));
        assert_eq!(tl.governing_bgm_chip(7_999), Some((0, 0)));
        assert_eq!(tl.governing_bgm_chip(8_000), Some((1, 8_000)));
        assert_eq!(tl.governing_bgm_chip(12_000), Some((1, 8_000)));
    }

    #[test]
    fn governing_bgm_chip_none_before_first() {
        let mut chart = test_chart();
        // Move both BGM chips later than 0.
        chart.chips[0] = dtx_core::Chip::with_wav(2, EChannel::BGM, 0.0, 1);
        let tl = build(&chart);
        assert_eq!(tl.governing_bgm_chip(1_000), None);
    }

    #[test]
    fn snap_neighbor_steps_between_points() {
        let tl = build(&test_chart());
        assert_eq!(tl.snap_neighbor(2_000, SnapDivisor::Bar, 1), 4_000);
        assert_eq!(tl.snap_neighbor(2_000, SnapDivisor::Bar, -1), 0);
        assert_eq!(tl.snap_neighbor(0, SnapDivisor::Bar, -1), 0);
    }

    #[test]
    fn density_counts_only_drum_lanes() {
        let tl = build(&test_chart());
        let total: f32 = tl.density.iter().sum();
        assert!(total > 0.0);
        let max = tl.density.iter().cloned().fold(0.0_f32, f32::max);
        assert!((max - 1.0).abs() < 1e-6, "density must be normalized to 1.0");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums timeline`
Expected: FAIL to compile (types missing)

- [ ] **Step 3: Implement**

Add above the tests module:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineEntry {
    pub chip_idx: usize,
    pub channel: EChannel,
    /// Judgement timebase (`chip_target_ms`, no BGM adjust).
    pub judge_ms: i64,
    /// Auto-scheduler timebase (`auto_chip_target_ms`, BGM adjust applied).
    pub auto_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapDivisor {
    #[default]
    Bar,
    Beat,
    Quarter,
}

impl SnapDivisor {
    pub fn label(self) -> &'static str {
        match self {
            SnapDivisor::Bar => "Bar",
            SnapDivisor::Beat => "Beat",
            SnapDivisor::Quarter => "1/2 beat",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SnapDivisor::Bar => SnapDivisor::Beat,
            SnapDivisor::Beat => SnapDivisor::Quarter,
            SnapDivisor::Quarter => SnapDivisor::Bar,
        }
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ChipTimeline {
    /// One entry per chart chip, sorted by `judge_ms`.
    pub entries: Vec<TimelineEntry>,
    /// `judge_ms` indexed by chip index (unsorted chart order).
    pub judge_ms_by_idx: Vec<i64>,
    /// Times of `dtx_core::expand_timing_lines` output, parallel to
    /// `TimingLineList.lines` (same expansion, same order).
    pub timing_line_ms: Vec<i64>,
    /// Bar-line times, sorted ascending.
    pub bar_ms: Vec<i64>,
    /// Bar + beat line times merged, sorted ascending.
    pub beat_ms: Vec<i64>,
    /// BGM chips with audio: `(chip_idx, auto_ms)`, sorted by `auto_ms`.
    pub bgm_chips: Vec<(usize, i64)>,
    /// Chart end incl. tail (mirrors `DrumsStageCompletion::chart_end_ms`).
    pub end_ms: i64,
    /// Normalized drum-chip density over `[0, end_ms]`.
    pub density: [f32; DENSITY_BUCKETS],
}

impl ChipTimeline {
    pub fn from_chart(
        chart: &Chart,
        bpm_changes: &BpmChangeList,
        bar_changes: &BarLengthChangeList,
        bgm_adjust_ms: i32,
        end_ms: i64,
    ) -> Self {
        let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
        let timing = ChartTiming {
            bpm_changes: &bpm_changes.changes,
            bar_changes: &bar_changes.changes,
        };

        let mut entries: Vec<TimelineEntry> = chart
            .chips
            .iter()
            .enumerate()
            .map(|(idx, chip)| TimelineEntry {
                chip_idx: idx,
                channel: chip.channel,
                judge_ms: chip_target_ms(chip, base_bpm, timing),
                auto_ms: auto_chip_target_ms(chip, base_bpm, timing, bgm_adjust_ms),
            })
            .collect();
        let mut judge_ms_by_idx = vec![0i64; chart.chips.len()];
        for e in &entries {
            judge_ms_by_idx[e.chip_idx] = e.judge_ms;
        }
        entries.sort_by_key(|e| e.judge_ms);

        let lines = dtx_core::expand_timing_lines(chart);
        let mut timing_line_ms = Vec::with_capacity(lines.len());
        let mut bar_ms = Vec::new();
        let mut beat_only_ms = Vec::new();
        for line in &lines {
            let (measure, fraction) =
                dtx_core::beat_lines::tick_to_measure_fraction(line.tick);
            let ms = dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(
                measure, fraction, base_bpm, timing,
            );
            timing_line_ms.push(ms);
            match line.kind {
                TimingLineKind::Bar => bar_ms.push(ms),
                TimingLineKind::Beat => beat_only_ms.push(ms),
            }
        }
        bar_ms.sort_unstable();
        bar_ms.dedup();
        let mut beat_ms: Vec<i64> =
            bar_ms.iter().copied().chain(beat_only_ms).collect();
        beat_ms.sort_unstable();
        beat_ms.dedup();

        let mut bgm_chips: Vec<(usize, i64)> = entries
            .iter()
            .filter(|e| {
                e.channel == EChannel::BGM
                    && chart.chips[e.chip_idx].wav_slot != 0
            })
            .map(|e| (e.chip_idx, e.auto_ms))
            .collect();
        bgm_chips.sort_by_key(|&(_, ms)| ms);

        let mut density = [0.0_f32; DENSITY_BUCKETS];
        if end_ms > 0 {
            for e in &entries {
                if lane_of(e.channel).is_none() {
                    continue;
                }
                let slot = ((e.judge_ms.max(0) as f64 / end_ms as f64)
                    * DENSITY_BUCKETS as f64) as usize;
                density[slot.min(DENSITY_BUCKETS - 1)] += 1.0;
            }
            let max = density.iter().cloned().fold(0.0_f32, f32::max);
            if max > 0.0 {
                for d in &mut density {
                    *d /= max;
                }
            }
        }

        Self {
            entries,
            judge_ms_by_idx,
            timing_line_ms,
            bar_ms,
            beat_ms,
            bgm_chips,
            end_ms,
            density,
        }
    }

    fn snap_points(&self, snap: SnapDivisor) -> Vec<i64> {
        match snap {
            SnapDivisor::Bar => self.bar_ms.clone(),
            SnapDivisor::Beat => self.beat_ms.clone(),
            SnapDivisor::Quarter => {
                let mut pts = self.beat_ms.clone();
                for w in self.beat_ms.windows(2) {
                    pts.push(w[0] + (w[1] - w[0]) / 2);
                }
                pts.sort_unstable();
                pts.dedup();
                pts
            }
        }
    }

    /// Floor `target_ms` to the nearest snap point at or before it,
    /// clamped into `[first point, end_ms]`.
    pub fn resolve_snap(&self, target_ms: i64, snap: SnapDivisor) -> i64 {
        let pts = self.snap_points(snap);
        if pts.is_empty() {
            return target_ms.clamp(0, self.end_ms.max(0));
        }
        let clamped = target_ms.clamp(pts[0], self.end_ms.max(pts[0]));
        match pts.binary_search(&clamped) {
            Ok(i) => pts[i],
            Err(0) => pts[0],
            Err(i) => pts[i - 1],
        }
    }

    /// Next (`dir > 0`) or previous (`dir < 0`) snap point from `ms`.
    /// Saturates at the ends.
    pub fn snap_neighbor(&self, ms: i64, snap: SnapDivisor, dir: i8) -> i64 {
        let pts = self.snap_points(snap);
        if pts.is_empty() {
            return ms;
        }
        let cur = self.resolve_snap(ms, snap);
        let i = pts.binary_search(&cur).unwrap_or_else(|e| e.min(pts.len() - 1));
        let j = if dir > 0 {
            (i + 1).min(pts.len() - 1)
        } else {
            i.saturating_sub(1)
        };
        pts[j]
    }

    /// Start of the bar at or before `ms` (pre-roll anchor).
    pub fn bar_start_before(&self, ms: i64) -> i64 {
        self.resolve_snap(ms, SnapDivisor::Bar)
    }

    /// The BGM chip whose stream should be playing at chart time
    /// `target_ms`: the last chip with `auto_ms <= target_ms`.
    pub fn governing_bgm_chip(&self, target_ms: i64) -> Option<(usize, i64)> {
        self.bgm_chips
            .iter()
            .take_while(|&&(_, ms)| ms <= target_ms)
            .last()
            .copied()
    }
}

/// Build the timeline on Performance enter. Ordered after the drums
/// enter chain so BPM/bar lists and `chart_end_ms` are already derived.
pub fn build_chip_timeline(
    chart: Res<crate::resources::ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<crate::resources::BgmAdjustState>,
    completion: Res<crate::orchestrator::DrumsStageCompletion>,
    mut timeline: ResMut<ChipTimeline>,
) {
    *timeline = ChipTimeline::from_chart(
        &chart.chart,
        &bpm_changes,
        &bar_changes,
        bgm_adjust.total_ms(),
        completion.chart_end_ms,
    );
}
```

Note: `tick_to_measure_fraction` is already imported by `crates/gameplay-drums/src/beat_lines.rs:7` from `dtx_core::beat_lines` — same function, fully qualified here.

- [ ] **Step 4: Register in lib.rs**

In `crates/gameplay-drums/src/lib.rs`: add `pub mod timeline;` to the module list, add `.init_resource::<timeline::ChipTimeline>()` next to the other init_resources, and add:

```rust
    .add_systems(
        OnEnter(game_shell::AppState::Performance),
        timeline::build_chip_timeline.after(orchestrator::DrumsEnterSet),
    )
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums timeline`
Expected: 8 passed

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/timeline.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(gameplay-drums): ChipTimeline resource with snap + density + BGM slice lookup"
```

---

### Task 4: Seek engine — `SeekToChartTime` + `apply_seek_system`

**Files:**
- Create: `crates/gameplay-drums/src/seek.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module + message + resource + system wiring)

- [ ] **Step 1: Write the failing tests (pure seeding fn)**

Create `crates/gameplay-drums/src/seek.rs`:

```rust
//! Seek engine op: position playback at an arbitrary chart time.
//!
//! `SeekToChartTime` is the ONLY entry point to seeking. One system
//! (`apply_seek_system`) owns the ordering: stop audio → reseed
//! skip-sets → despawn notes → queue BGM restart → jump the clock.
//! Consumers: practice UI, A/B loop watcher, (later) live preview and
//! trainers.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::AudioSource as KiraAudioSource;
use game_shell::PauseState;

use crate::bgm_scheduler::chip_wav_path;
use crate::components::NoteVisual;
use crate::judge::JudgedChips;
use crate::resources::{
    ActiveChart, ActiveDrumSounds, DrumAudioSettings, GameStartMs, GameplayClock,
    TimingLineCrossed,
};
use crate::timeline::{ChipTimeline, SnapDivisor};

/// Request to jump playback to a chart time.
#[derive(Message, Debug, Clone, Copy)]
pub struct SeekToChartTime {
    /// Requested chart time (ms). Snapped by the engine when `snap` is set.
    pub target_ms: i64,
    /// Snap the target down to this grid before applying.
    pub snap: Option<SnapDivisor>,
    /// Chart time the *attempt* conceptually starts at (e.g. the A-marker
    /// when `target_ms` includes pre-roll). Consumers that track section
    /// stats read this; `None` means "same as the applied target".
    pub attempt_start_ms: Option<i64>,
}

/// BGM restart queued by a seek; started by [`start_pending_bgm`] on the
/// next running tick. Deferring the start (instead of playing inside the
/// seek) keeps paused-seek correct: audio only starts once unpaused.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingBgmStart(pub Option<PendingBgm>);

#[derive(Debug, Clone)]
pub struct PendingBgm {
    /// WAV slot to fetch from the sound bank; 0 = load `path` directly.
    pub wav_slot: u32,
    pub path: String,
    pub start_seconds: f64,
}

/// Rebuild all skip-sets for playback positioned at `target_ms`.
///
/// - `judged`: every chip strictly before the target in the judgement
///   timebase (playable or not — mirrors what a played-through stage
///   would contain, so spawner/judge/miss/autoplay all skip them).
/// - `played_bgm`: BGM chips at or before the target in the auto
///   timebase (the governing chip is restarted manually by the caller).
/// - `played_se`: SE chips strictly before the target (auto timebase).
/// - `crossed`: timing lines strictly before the target.
pub fn seed_skip_sets(
    timeline: &ChipTimeline,
    target_ms: i64,
    judged: &mut HashSet<usize>,
    played_bgm: &mut HashSet<usize>,
    played_se: &mut HashSet<usize>,
    crossed: &mut HashSet<usize>,
) {
    judged.clear();
    played_bgm.clear();
    played_se.clear();
    crossed.clear();
    for e in &timeline.entries {
        if e.judge_ms < target_ms {
            judged.insert(e.chip_idx);
        }
        match e.channel {
            dtx_core::EChannel::BGM => {
                if e.auto_ms <= target_ms {
                    played_bgm.insert(e.chip_idx);
                }
            }
            dtx_core::EChannel::SE01
            | dtx_core::EChannel::SE02
            | dtx_core::EChannel::SE03
            | dtx_core::EChannel::SE04
            | dtx_core::EChannel::SE05 => {
                if e.auto_ms < target_ms {
                    played_se.insert(e.chip_idx);
                }
            }
            _ => {}
        }
    }
    for (i, &ms) in timeline.timing_line_ms.iter().enumerate() {
        if ms < target_ms {
            crossed.insert(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::assets::DtxAssets;
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM: measure = 2000ms.
    fn chart() -> Chart {
        let mut assets = DtxAssets::default();
        assets.wav.insert(1, "bgm.ogg".into());
        assets.wav.insert(2, "se.ogg".into());
        Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1), // 0ms
                Chip::new(0, EChannel::BassDrum, 0.0),    // 0ms
                Chip::new(1, EChannel::Snare, 0.0),       // 2000ms
                Chip::with_wav(2, EChannel::SE01, 0.0, 2), // 4000ms
                Chip::new(3, EChannel::BassDrum, 0.0),    // 6000ms
            ],
            assets,
            ..Default::default()
        }
    }

    fn timeline() -> ChipTimeline {
        let c = chart();
        let bpm = BpmChangeList::from_chart(&c);
        let bar = BarLengthChangeList::from_chart(&c);
        ChipTimeline::from_chart(&c, &bpm, &bar, 0, 8_000)
    }

    fn seeded(target: i64) -> (HashSet<usize>, HashSet<usize>, HashSet<usize>, HashSet<usize>) {
        let tl = timeline();
        let (mut j, mut b, mut s, mut c) =
            (HashSet::new(), HashSet::new(), HashSet::new(), HashSet::new());
        seed_skip_sets(&tl, target, &mut j, &mut b, &mut s, &mut c);
        (j, b, s, c)
    }

    #[test]
    fn forward_seek_marks_everything_before_target() {
        let (j, b, s, _) = seeded(5_000);
        assert!(j.contains(&1) && j.contains(&2), "drum chips before target judged");
        assert!(!j.contains(&4), "chip after target stays live");
        assert!(b.contains(&0), "bgm chip at 0 marked played");
        assert!(s.contains(&3), "se chip before target marked played");
    }

    #[test]
    fn backward_seek_to_zero_clears_everything() {
        let (j, b, s, c) = seeded(0);
        assert!(j.is_empty());
        assert!(b.contains(&0), "bgm chip exactly at target is governing → marked");
        assert!(s.is_empty());
        assert!(c.is_empty());
    }

    #[test]
    fn sets_are_rebuilt_not_patched() {
        let tl = timeline();
        let mut j: HashSet<usize> = (0..100).collect();
        let (mut b, mut s, mut c) = (HashSet::new(), HashSet::new(), HashSet::new());
        seed_skip_sets(&tl, 1_000, &mut j, &mut b, &mut s, &mut c);
        assert!(j.len() <= tl.entries.len(), "stale indices must be gone");
        assert!(!j.contains(&99));
    }

    #[test]
    fn timing_lines_before_target_marked_crossed() {
        let tl = timeline();
        let (_, _, _, c) = seeded(2_001);
        // Lines at 0..=2000 crossed, later ones not.
        assert!(!c.is_empty());
        for &i in &c {
            assert!(tl.timing_line_ms[i] < 2_001);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify pure-fn tests pass and the rest fails to compile**

Run: `cargo test -p gameplay-drums seek::tests`
Expected: compile error until `lib.rs` declares the module — add `pub mod seek;` to `lib.rs` first, then run again. The 4 tests pass (system not yet written).

- [ ] **Step 3: Implement the seek system**

Append to `seek.rs` (above the tests module):

```rust
/// Audio-side parameters for the seek system, bundled to stay under
/// Bevy's system-param ceiling (see orchestrator.rs:75-81).
#[derive(SystemParam)]
pub struct SeekAudio<'w> {
    pub audio: Res<'w, Audio>,
    pub settings: Res<'w, DrumAudioSettings>,
    pub sound_bank: Res<'w, dtx_audio::ChartSoundBank>,
    pub sources: Res<'w, Assets<KiraAudioSource>>,
    pub bgm: ResMut<'w, dtx_audio::BgmHandle>,
    pub instances: ResMut<'w, Assets<AudioInstance>>,
    pub polyphony: ResMut<'w, dtx_audio::DrumPolyphony>,
    pub active: ResMut<'w, ActiveDrumSounds>,
    pub pending: ResMut<'w, PendingBgmStart>,
}

/// Skip-set + clock parameters for the seek system.
#[derive(SystemParam)]
pub struct SeekState<'w> {
    pub judged: ResMut<'w, JudgedChips>,
    pub played_bgm: ResMut<'w, crate::bgm_scheduler::PlayedBgmChips>,
    pub played_se: ResMut<'w, crate::se_scheduler::PlayedSeChips>,
    pub crossed: ResMut<'w, TimingLineCrossed>,
    pub start_ms: ResMut<'w, GameStartMs>,
    pub clock: ResMut<'w, GameplayClock>,
}

pub fn apply_seek_system(
    mut seeks: MessageReader<SeekToChartTime>,
    timeline: Res<ChipTimeline>,
    chart: Res<ActiveChart>,
    mut audio: SeekAudio,
    mut state: SeekState,
    notes: Query<Entity, With<NoteVisual>>,
    mut commands: Commands,
) {
    // Coalesce: only the last seek this tick wins.
    let Some(seek) = seeks.read().last().copied() else {
        return;
    };
    if !state.clock.is_started() || timeline.entries.is_empty() {
        return;
    }

    let resolved = match seek.snap {
        Some(snap) => timeline.resolve_snap(seek.target_ms, snap),
        None => seek.target_ms.clamp(0, timeline.end_ms.max(0)),
    };

    // 1. Stop everything currently sounding (layers, HH, stick SE, drums).
    audio.active.stop_all(&mut audio.instances);
    dtx_audio::stop_polyphony(&mut audio.instances, &audio.polyphony);

    // 2. Rebuild skip-sets from scratch.
    seed_skip_sets(
        &timeline,
        resolved,
        &mut state.judged.0,
        &mut state.played_bgm.0,
        &mut state.played_se.0,
        &mut state.crossed.0,
    );

    // 3. Despawn live notes; the spawner refills from the new `now`.
    for entity in &notes {
        commands.entity(entity).despawn();
    }

    // 4. Queue the BGM restart (started by `start_pending_bgm` while
    //    running — a paused seek must not emit audio).
    audio.pending.0 = None;
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    match timeline.governing_bgm_chip(resolved) {
        Some((idx, chip_ms)) => {
            state.start_ms.0 = chip_ms;
            let start_seconds = (resolved - chip_ms).max(0) as f64 / 1000.0;
            let wav_slot = chart.chart.chips[idx].wav_slot;
            let within_slice = audio
                .sound_bank
                .get(wav_slot)
                .and_then(|s| audio.sources.get(&s.handle))
                .map(|src| start_seconds < src.sound.duration().as_secs_f64())
                // Duration unknown (asset still decoding): try anyway.
                .unwrap_or(true);
            dtx_audio::stop_bgm(&audio.audio, &mut audio.bgm, &mut audio.instances);
            if within_slice {
                if let Some(path) = chip_wav_path(&chart.chart, idx, source_dir) {
                    audio.pending.0 = Some(PendingBgm {
                        wav_slot,
                        path,
                        start_seconds,
                    });
                }
            }
            // else: seek landed past the governing slice's audio — stay
            // silent; the next BGM chip schedules normally.
        }
        None => {
            dtx_audio::stop_bgm(&audio.audio, &mut audio.bgm, &mut audio.instances);
            if !crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart) {
                // Whole-file fallback BGM (no BGM chips): stream position 0
                // is chart time 0.
                state.start_ms.0 = 0;
                if let Some(source_path) = chart.source_path.as_ref() {
                    if let Some(bgm_path) =
                        dtx_core::resolve_bgm_path(source_path, &chart.chart)
                    {
                        audio.pending.0 = Some(PendingBgm {
                            wav_slot: 0,
                            path: bgm_path.to_string_lossy().to_string(),
                            start_seconds: resolved.max(0) as f64 / 1000.0,
                        });
                    }
                }
            } else {
                // Target is before the first BGM chip: leave it unplayed;
                // bgm_scheduler starts it on time. Restore enter-time
                // GameStartMs (first BGM chip's chart time).
                if let Some(&(_, first_ms)) = timeline.bgm_chips.first() {
                    state.start_ms.0 = first_ms;
                }
            }
        }
    }

    // 5. Jump the clock last; next measured BGM position re-snaps it.
    state.clock.seek(resolved);
    info!(
        "seek: target={} resolved={} (snap {:?})",
        seek.target_ms, resolved, seek.snap
    );
}

/// Start a queued BGM restart. Runs only while unpaused so a seek made
/// from the pause menu starts audio exactly on resume.
pub fn start_pending_bgm(
    mut pending: ResMut<PendingBgmStart>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    settings: Res<DrumAudioSettings>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let Some(p) = pending.0.take() else {
        return;
    };
    if let Some(sound) = sound_bank.get(p.wav_slot) {
        dtx_audio::play_bgm_handle_with_mix_from_seconds(
            &audio,
            &mut instances,
            &mut bgm,
            sound.handle.clone(),
            &sound.path.to_string_lossy(),
            sound.volume,
            sound.pan,
            settings.master_volume,
            p.start_seconds,
            0,
        );
    } else {
        dtx_audio::play_bgm_from_seconds(
            &audio,
            &asset_server,
            &mut bgm,
            &mut instances,
            &p.path,
            p.start_seconds,
            0,
        );
    }
}
```

Note the unused-import cleanup: `PauseState` is not needed in this file after the pending-BGM design (pausedness is expressed by scheduling `start_pending_bgm` under `run_if(in_state(PauseState::Running))`). Remove the import if the compiler flags it.

- [ ] **Step 4: Wire into `lib.rs`**

In `crates/gameplay-drums/src/lib.rs`:
- module list: `pub mod seek;` (alphabetical near `score`).
- registration: `.add_message::<seek::SeekToChartTime>()` next to the other messages, `.init_resource::<seek::PendingBgmStart>()` next to the other resources.
- systems (immediately after the existing `sync_gameplay_clock` add_systems block):

```rust
    .add_systems(
        FixedUpdate,
        seek::apply_seek_system
            .before(dtx_timing::update_audio_clock_system)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        FixedUpdate,
        seek::start_pending_bgm
            .after(seek::apply_seek_system)
            .before(dtx_timing::update_audio_clock_system)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running)),
    )
```

Ordering rationale (documented in the seek module header): the seek runs before the audio-clock read so `AudioClock` never feeds a stale pre-seek position into `sync_gameplay_clock` in the same tick, and before `DrumsSets::Judge` (ClockSync precedes Judge) so the reseeded `JudgedChips` wins against `despawn_missed_notes_system` — no phantom miss burst.

- [ ] **Step 5: Run tests + check**

Run: `cargo test -p gameplay-drums seek && cargo check -p gameplay-drums`
Expected: 4 passed, clean check

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/seek.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(gameplay-drums): SeekToChartTime engine op with skip-set reseeding and deferred BGM restart"
```

---

### Task 5: Practice session types + lifecycle

**Files:**
- Create: `crates/gameplay-drums/src/practice/session.rs`, `crates/gameplay-drums/src/practice/mod.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module + plugin)

- [ ] **Step 1: Write the failing tests**

Create `crates/gameplay-drums/src/practice/session.rs`:

```rust
//! Practice session state: loop region, rate, snap, pre-roll, attempts.

use bevy::prelude::*;

use crate::resources::JudgmentCounts;
use crate::timeline::{ChipTimeline, SnapDivisor};

pub const RATE_MIN: f32 = 0.5;
pub const RATE_MAX: f32 = 1.5;
pub const RATE_STEP: f32 = 0.05;
pub const MAX_ATTEMPT_HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopRegion {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrerollSetting {
    OneBar,
    Seconds(f32),
    Off,
}

impl PrerollSetting {
    pub fn label(self) -> String {
        match self {
            PrerollSetting::OneBar => "1 bar".into(),
            PrerollSetting::Seconds(s) => format!("{s:.0}s"),
            PrerollSetting::Off => "off".into(),
        }
    }

    pub fn next(self) -> Self {
        match self {
            PrerollSetting::OneBar => PrerollSetting::Seconds(2.0),
            PrerollSetting::Seconds(_) => PrerollSetting::Off,
            PrerollSetting::Off => PrerollSetting::OneBar,
        }
    }
}

/// Resolve the actual seek target for an intended attempt start:
/// back off by the configured pre-roll so the drummer gets ready-time.
pub fn preroll_target(
    timeline: &ChipTimeline,
    preroll: PrerollSetting,
    intent_ms: i64,
) -> i64 {
    match preroll {
        PrerollSetting::Off => intent_ms,
        PrerollSetting::Seconds(s) => (intent_ms - (s * 1000.0) as i64).max(0),
        PrerollSetting::OneBar => {
            timeline.bar_start_before((intent_ms - 1).max(0))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AttemptStats {
    /// Attempt span start (the intent, not the pre-roll point). Chips
    /// judged before this are pre-roll and excluded.
    pub start_ms: i64,
    pub counts: JudgmentCounts,
    pub combo: u32,
    pub max_combo: u32,
    pub error_sum_ms: i64,
    pub error_count: u32,
}

impl AttemptStats {
    pub fn accuracy_pct(&self) -> f32 {
        self.counts.achievement_pct()
    }

    pub fn mean_error_ms(&self) -> f32 {
        if self.error_count == 0 {
            0.0
        } else {
            self.error_sum_ms as f32 / self.error_count as f32
        }
    }

    pub fn has_data(&self) -> bool {
        self.counts.total() > 0
    }
}

#[derive(Debug, Clone)]
pub struct AttemptRecord {
    pub start_ms: i64,
    pub end_ms: i64,
    pub rate: f32,
    pub counts: JudgmentCounts,
    pub max_combo: u32,
    pub accuracy_pct: f32,
    pub mean_error_ms: f32,
}

/// Present only while the stage runs in practice mode. Absence = normal
/// play with zero behavior change.
#[derive(Resource, Debug, Clone)]
pub struct PracticeSession {
    pub loop_region: Option<LoopRegion>,
    pub rate: f32,
    pub snap: SnapDivisor,
    pub preroll: PrerollSetting,
    pub current_attempt: AttemptStats,
    pub attempt_history: Vec<AttemptRecord>,
    /// Scrub cursor while paused (chart ms). None = cursor at playhead.
    pub scrub_cursor_ms: Option<i64>,
}

impl Default for PracticeSession {
    fn default() -> Self {
        Self {
            loop_region: None,
            rate: 1.0,
            snap: SnapDivisor::Bar,
            preroll: PrerollSetting::OneBar,
            current_attempt: AttemptStats::default(),
            attempt_history: Vec::new(),
            scrub_cursor_ms: None,
        }
    }
}

impl PracticeSession {
    /// Step the rate by `dir` in RATE_STEP increments, clamped and
    /// quantized so repeated stepping never accumulates float error.
    pub fn step_rate(&mut self, dir: i8) {
        let steps = (self.rate / RATE_STEP).round() as i32 + dir as i32;
        self.rate = (steps as f32 * RATE_STEP).clamp(RATE_MIN, RATE_MAX);
    }

    /// Finalize the running attempt into history (skipped when it saw no
    /// judgements) and start a fresh one at `next_start_ms`.
    pub fn roll_attempt(&mut self, end_ms: i64, next_start_ms: i64) {
        if self.current_attempt.has_data() {
            let a = &self.current_attempt;
            self.attempt_history.push(AttemptRecord {
                start_ms: a.start_ms,
                end_ms,
                rate: self.rate,
                counts: a.counts,
                max_combo: a.max_combo,
                accuracy_pct: a.accuracy_pct(),
                mean_error_ms: a.mean_error_ms(),
            });
            if self.attempt_history.len() > MAX_ATTEMPT_HISTORY {
                self.attempt_history.remove(0);
            }
        }
        self.current_attempt = AttemptStats {
            start_ms: next_start_ms,
            ..Default::default()
        };
    }

    /// Set the A marker; keeps the region valid (swap, min length is
    /// enforced by the caller against bar data).
    pub fn set_loop_start(&mut self, ms: i64) {
        let end = self.loop_region.map(|r| r.end_ms);
        self.loop_region = Some(match end {
            Some(e) if e > ms => LoopRegion { start_ms: ms, end_ms: e },
            _ => LoopRegion { start_ms: ms, end_ms: i64::MAX },
        });
    }

    pub fn set_loop_end(&mut self, ms: i64) {
        let start = self.loop_region.map(|r| r.start_ms).unwrap_or(0);
        self.loop_region = Some(if ms > start {
            LoopRegion { start_ms: start, end_ms: ms }
        } else {
            // B placed before A: swap.
            LoopRegion { start_ms: ms, end_ms: start.max(ms + 1) }
        });
    }

    /// True when a bounded loop region is armed.
    pub fn loop_armed(&self) -> bool {
        self.loop_region.is_some_and(|r| r.end_ms != i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_step_quantized_and_clamped() {
        let mut s = PracticeSession::default();
        s.step_rate(-1);
        assert!((s.rate - 0.95).abs() < 1e-6);
        for _ in 0..40 {
            s.step_rate(-1);
        }
        assert!((s.rate - RATE_MIN).abs() < 1e-6);
        for _ in 0..40 {
            s.step_rate(1);
        }
        assert!((s.rate - RATE_MAX).abs() < 1e-6);
    }

    #[test]
    fn roll_attempt_records_history_and_resets() {
        let mut s = PracticeSession::default();
        s.current_attempt.start_ms = 4_000;
        s.current_attempt.counts.perfect = 10;
        s.current_attempt.max_combo = 10;
        s.roll_attempt(8_000, 4_000);
        assert_eq!(s.attempt_history.len(), 1);
        assert_eq!(s.attempt_history[0].start_ms, 4_000);
        assert_eq!(s.attempt_history[0].end_ms, 8_000);
        assert!(!s.current_attempt.has_data());
        assert_eq!(s.current_attempt.start_ms, 4_000);
    }

    #[test]
    fn empty_attempt_not_recorded() {
        let mut s = PracticeSession::default();
        s.roll_attempt(1_000, 2_000);
        assert!(s.attempt_history.is_empty());
    }

    #[test]
    fn history_capped() {
        let mut s = PracticeSession::default();
        for i in 0..(MAX_ATTEMPT_HISTORY + 5) {
            s.current_attempt.counts.perfect = 1;
            s.roll_attempt(i as i64, 0);
        }
        assert_eq!(s.attempt_history.len(), MAX_ATTEMPT_HISTORY);
    }

    #[test]
    fn loop_markers_swap_when_inverted() {
        let mut s = PracticeSession::default();
        s.set_loop_start(4_000);
        s.set_loop_end(2_000);
        let r = s.loop_region.unwrap();
        assert!(r.start_ms < r.end_ms);
        assert_eq!(r.start_ms, 2_000);
    }

    #[test]
    fn loop_not_armed_until_both_markers() {
        let mut s = PracticeSession::default();
        assert!(!s.loop_armed());
        s.set_loop_start(2_000);
        assert!(!s.loop_armed());
        s.set_loop_end(4_000);
        assert!(s.loop_armed());
    }

    #[test]
    fn mean_error_signed() {
        let mut a = AttemptStats::default();
        a.error_sum_ms = -30;
        a.error_count = 10;
        assert!((a.mean_error_ms() + 3.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Create the plugin module**

Create `crates/gameplay-drums/src/practice/mod.rs`:

```rust
//! Practice mode: seek/scrub, A/B loop, playback rate, attempt stats.
//!
//! `PracticeSession` present = practice; absent = normal play with zero
//! behavior change. Inserted on Performance enter when
//! `game_shell::PracticeIntent` is set, removed on returning to song
//! select (it must survive StageClear/Result so the save gate sees it).

pub mod ab_loop;
pub mod rate;
pub mod session;
pub mod stats;
pub mod ui;

use bevy::prelude::*;
use game_shell::{AppState, PracticeIntent};

pub use session::PracticeSession;

use crate::gauge::StageGauge;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        enter_practice_session.before(crate::orchestrator::DrumsEnterSet),
    )
    .add_systems(OnEnter(AppState::SongSelect), remove_practice_session)
    .add_systems(
        FixedUpdate,
        freeze_gauge_in_practice
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_plugins((ab_loop::plugin, rate::plugin, stats::plugin, ui::plugin));
}

fn enter_practice_session(intent: Res<PracticeIntent>, mut commands: Commands) {
    if intent.0 {
        commands.insert_resource(PracticeSession::default());
    } else {
        commands.remove_resource::<PracticeSession>();
    }
}

fn remove_practice_session(mut commands: Commands) {
    commands.remove_resource::<PracticeSession>();
}

/// Gauge is meaningless in practice: pin it full so it can never fail
/// the stage and the HUD reads as neutral.
fn freeze_gauge_in_practice(mut gauge: ResMut<StageGauge>) {
    gauge.value = 1.0;
    gauge.failed = false;
}
```

Note: `StageGauge.value` is clamped display-side by `pct()` (`gauge.rs:111-113`); `GAUGE_MAX` is 1.0 (`starts_at_two_thirds` test shows value is a 0..1 fraction) — pin to `1.0` directly.

For this task, create placeholder submodules so the crate compiles (each will be filled by its own task):

`crates/gameplay-drums/src/practice/ab_loop.rs`, `rate.rs`, `stats.rs`, `ui.rs`, each initially:

```rust
use bevy::prelude::*;

pub(super) fn plugin(_app: &mut App) {}
```

- [ ] **Step 3: Register in lib.rs**

In `crates/gameplay-drums/src/lib.rs`: add `pub mod practice;` to the module list and `practice::plugin,` to the second `add_plugins` tuple (after `stage_end::plugin`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums practice::session && cargo check -p gameplay-drums`
Expected: 7 passed, clean

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice crates/gameplay-drums/src/lib.rs
git commit -m "feat(gameplay-drums): PracticeSession state + lifecycle plugin"
```

---

### Task 6: Practice gates — end-of-stage suppression + no score save

**Files:**
- Modify: `crates/gameplay-drums/src/orchestrator.rs:386-429` (`detect_end_of_stage`)
- Modify: `crates/game-results/src/lib.rs:235` (`save_result_then_despawn`)
- Test: `crates/gameplay-drums/tests/practice_mode.rs` (new)

- [ ] **Step 1: Write the failing integration test**

Create `crates/gameplay-drums/tests/practice_mode.rs`:

```rust
//! Integration tests for practice mode: seek, gates, loop.

use bevy::prelude::*;
use dtx_audio::BgmHandle;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip, Metadata};
use game_shell::AppState;
use gameplay_drums::components::LastJudgment;
use gameplay_drums::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use gameplay_drums::orchestrator::{
    detect_end_of_stage, enter_derive_from_chart, enter_reset_run_state,
    enter_seed_bgm_state, DrumsStageCompletion,
};
use gameplay_drums::practice::session::{LoopRegion, PracticeSession};
use gameplay_drums::resources::{
    ActiveChart, BgmAdjustState, Combo, GameStartMs, GameplayClock, JudgmentCounts,
    Score,
};
use gameplay_drums::timeline::build_chip_timeline;

fn chart_with_measures(n: u32) -> Chart {
    let chips: Vec<Chip> = (0..n)
        .map(|i| Chip::new(i, EChannel::BassDrum, 1.0))
        .collect();
    Chart {
        metadata: Metadata::default(),
        chips,
        ..Default::default()
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        bevy_kira_audio::AudioPlugin,
    ))
    .init_state::<AppState>()
    .init_resource::<DrumsStageCompletion>()
    .init_resource::<GameplayClock>()
    .init_resource::<ActiveChart>()
    .init_resource::<Score>()
    .init_resource::<gameplay_drums::resources::DrumScoring>()
    .init_resource::<Combo>()
    .init_resource::<JudgmentCounts>()
    .init_resource::<gameplay_drums::resources::DrumGameplaySettings>()
    .init_resource::<gameplay_drums::resources::DrumAudioSettings>()
    .init_resource::<JudgedChips>()
    .init_resource::<LastJudgment>()
    .init_resource::<GameStartMs>()
    .init_resource::<BgmAdjustState>()
    .init_resource::<BpmChangeList>()
    .init_resource::<BarLengthChangeList>()
    .init_resource::<BgmHandle>()
    .init_resource::<dtx_audio::ChartSoundBank>()
    .init_resource::<dtx_audio::DrumPolyphony>()
    .init_resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
    .init_resource::<gameplay_drums::bgm_scheduler::PrimaryBgmChip>()
    .init_resource::<gameplay_drums::bgm_scheduler::BgmRecoveryState>()
    .init_resource::<gameplay_drums::resources::CurrentEmptyHitTemplates>()
    .init_resource::<gameplay_drums::resources::ActiveDrumSounds>()
    .init_resource::<gameplay_drums::se_scheduler::PlayedSeChips>()
    .init_resource::<gameplay_drums::resources::FastSlowCount>()
    .init_resource::<gameplay_drums::resources::SkillValue>()
    .init_resource::<gameplay_drums::derived::ChartDerived>()
    .init_resource::<gameplay_drums::resources::TimingLineCrossed>()
    .init_resource::<gameplay_drums::timeline::ChipTimeline>()
    .init_resource::<gameplay_drums::seek::PendingBgmStart>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_systems(
        OnEnter(AppState::Performance),
        (
            enter_reset_run_state,
            enter_derive_from_chart,
            enter_seed_bgm_state,
            build_chip_timeline,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            gameplay_drums::seek::apply_seek_system,
            detect_end_of_stage,
        )
            .chain()
            .run_if(in_state(AppState::Performance)),
    );
    app
}

fn enter_performance(app: &mut App, chart: Chart) {
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
}

#[test]
fn active_loop_region_suppresses_end_of_stage() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 0,
            end_ms: 2_000,
        }),
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(50_000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        !completion.end_requested,
        "active A/B loop must suppress end-of-stage"
    );
}

#[test]
fn cleared_loop_region_restores_end_of_stage() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    app.world_mut()
        .insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(50_000));
    }
    app.update();
    let completion = app.world().resource::<DrumsStageCompletion>();
    assert!(
        completion.end_requested,
        "practice without a loop region ends the stage normally"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode`
Expected: `active_loop_region_suppresses_end_of_stage` FAILS (end_requested is true)

- [ ] **Step 3: Gate `detect_end_of_stage`**

In `crates/gameplay-drums/src/orchestrator.rs`, add a parameter to `detect_end_of_stage` and an early return after the `end_requested` check:

```rust
pub fn detect_end_of_stage(
    clock: Res<GameplayClock>,
    mut completion: ResMut<DrumsStageCompletion>,
    _chart: Res<ActiveChart>,
    mut score: ResMut<Score>,
    mut scoring: ResMut<DrumScoring>,
    counts: Res<JudgmentCounts>,
    _combo: Res<Combo>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    if completion.end_requested {
        return;
    }
    // An armed A/B loop owns the stage end: the loop watcher seeks back
    // before the chart end is ever reached "for real".
    if practice.as_ref().is_some_and(|s| s.loop_region.is_some()) {
        return;
    }
    // ... rest unchanged ...
```

- [ ] **Step 4: Gate the score save**

In `crates/game-results/src/lib.rs`, change `save_result_then_despawn`:

```rust
fn save_result_then_despawn(
    commands: Commands,
    practice: Option<Res<gameplay_drums::practice::PracticeSession>>,
    score: Res<Score>,
    // ... existing params unchanged ...
) {
    // Practice runs are never persisted (no ScoreStore entry, no
    // score.ini update) — only the UI teardown happens.
    if practice.is_some() {
        despawn_stage::<ResultEntity>(commands, query);
        return;
    }
    // ... rest unchanged ...
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums --test practice_mode && cargo check -p game-results`
Expected: 2 passed, clean

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/orchestrator.rs crates/game-results/src/lib.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(practice): gate end-of-stage and score persistence in practice mode"
```

---

### Task 7: Playback rate — audio + clock scaling

**Files:**
- Modify: `crates/gameplay-drums/src/resources.rs` (add `AudioRate`), `crates/gameplay-drums/src/lib.rs` (`sync_gameplay_clock` + init)
- Modify: `crates/gameplay-drums/src/practice/rate.rs`

Model (from spec): audio plays at rate `r` (pitch shifts, v1-accepted); the gameplay clock advances at `r × dt` so chart-time math, judge windows (chart-ms), and scroll need no changes. Keysounds/SE follow via a channel-wide rate so every one-shot in the main channel — and the restarted BGM after a seek — inherits it.

- [ ] **Step 1: Add `AudioRate` + scale the clock tick**

In `crates/gameplay-drums/src/resources.rs` (near `ScrollSettings`):

```rust
/// Audio playback rate (1.0 = native). Practice rate control writes it;
/// the gameplay clock advances by `dt * rate` so chart-time math and
/// judge windows stay in chart-ms. Distinct from
/// `ScrollSettings::play_speed` (the DTXManiaNX chart-time compressor).
#[derive(Resource, Debug, Clone, Copy)]
pub struct AudioRate(pub f64);

impl Default for AudioRate {
    fn default() -> Self {
        Self(1.0)
    }
}
```

In `crates/gameplay-drums/src/lib.rs`, `.init_resource::<resources::AudioRate>()`, and change `sync_gameplay_clock`:

```rust
fn sync_gameplay_clock(
    audio_clock: Res<dtx_timing::AudioClock>,
    start_ms: Res<resources::GameStartMs>,
    rate: Res<resources::AudioRate>,
    time: Res<Time<Fixed>>,
    mut gameplay_clock: ResMut<resources::GameplayClock>,
) {
    let chart_ms = audio_clock
        .current_ms
        .map(|pos| start_ms.0.saturating_add(pos));
    gameplay_clock.tick(time.delta_secs_f64() * rate.0, chart_ms);
}
```

- [ ] **Step 2: Implement the rate applier**

Replace `crates/gameplay-drums/src/practice/rate.rs`:

```rust
//! Applies the practice rate to audio (BGM instance + main channel) and
//! to the gameplay clock via `AudioRate`.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::AppState;

use super::PracticeSession;
use crate::resources::AudioRate;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        apply_practice_rate
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), reset_audio_rate);
}

fn apply_practice_rate(
    session: Res<PracticeSession>,
    mut rate: ResMut<AudioRate>,
    audio: Res<Audio>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut applied: Local<f64>,
) {
    let target = session.rate as f64;
    // Local starts at 0.0, so the first run always applies (incl. 1.0).
    if (*applied - target).abs() < 1e-9 {
        return;
    }
    *applied = target;
    rate.0 = target;
    // Channel-wide: retunes currently playing sounds and is inherited by
    // future plays in the main channel (keysounds, SE, restarted BGM).
    audio.set_playback_rate(target);
    // Belt and braces for the tracked BGM instance (immediate tween).
    if let Some(handle) = &bgm.instance {
        if let Some(mut inst) = instances.get_mut(handle) {
            inst.set_playback_rate(target, AudioTween::default());
        }
    }
}

fn reset_audio_rate(mut rate: ResMut<AudioRate>, audio: Res<Audio>) {
    rate.0 = 1.0;
    audio.set_playback_rate(1.0);
}
```

- [ ] **Step 3: Add a clock-scaling unit test**

In `resources.rs` tests:

```rust
#[test]
fn rate_default_is_native() {
    let r = AudioRate::default();
    assert!((r.0 - 1.0).abs() < f64::EPSILON);
}
```

(The dt-scaling itself is `tick(dt * rate)` — covered by existing `tick_wall_clock_advances_by_dt` semantics; the multiplication site is one expression in `sync_gameplay_clock`.)

- [ ] **Step 4: Check + manual verification note**

Run: `cargo test -p gameplay-drums rate_default && cargo check -p gameplay-drums`
Expected: pass, clean.

MANUAL CHECK (flag for human review at the end of the plan): confirm in-game that after `audio.set_playback_rate(0.75)` newly played keysounds inherit the channel rate (bevy_kira_audio channel state applies settings to future plays — verify audibly; if not, fall back to passing the rate into `play_sfx_handle`/`play_drum_hit_handle` as a follow-up).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/resources.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/practice/rate.rs
git commit -m "feat(practice): playback rate applied to audio channel and gameplay clock"
```

---

### Task 8: A/B loop watcher

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ab_loop.rs`
- Test: `crates/gameplay-drums/tests/practice_mode.rs` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/practice_mode.rs`:

```rust
#[test]
fn loop_watcher_seeks_back_to_region_start() {
    let mut app = build_app();
    // Register the watcher in front of the seek system.
    app.add_systems(
        Update,
        gameplay_drums::practice::ab_loop::loop_watcher
            .before(gameplay_drums::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance)),
    );
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        preroll: gameplay_drums::practice::session::PrerollSetting::Off,
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(6_100));
    }
    app.update();
    let clock = app.world().resource::<GameplayClock>();
    assert_eq!(
        clock.current_ms, 2_000,
        "past region end the clock must land back on A"
    );
    // Chips before A are seeded as judged; the chip after A is live.
    let judged = &app.world().resource::<JudgedChips>().0;
    assert!(judged.contains(&0), "chip at 1000ms (before A) seeded");
    assert!(!judged.contains(&2), "chip at 5000ms (inside region) live");
}
```

(Chart timing: `chart_with_measures` puts `Chip::new(i, BassDrum, 1.0)` — chip value 1.0 clamps to the end of measure `i`; with the default 120 BPM metadata each measure is 2000ms, so chip 0 ≈ 1994ms, chip 1 ≈ 3994ms, chip 2 ≈ 5994ms. Adjust the two `judged` assertions if actual values differ — the invariant is: chips with `judge_ms < 2000` seeded, chips with `judge_ms >= 2000` live.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p gameplay-drums --test practice_mode loop_watcher`
Expected: FAIL to compile (`loop_watcher` missing)

- [ ] **Step 3: Implement**

Replace `crates/gameplay-drums/src/practice/ab_loop.rs`:

```rust
//! A/B loop: when the clock passes B, seek back to A (with pre-roll).

use bevy::prelude::*;
use game_shell::{AppState, PauseState};

use super::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        loop_watcher
            .before(crate::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}

pub fn loop_watcher(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    if !clock.is_ready() {
        return;
    }
    let Some(region) = session.loop_region else {
        return;
    };
    if region.end_ms == i64::MAX {
        return; // only A set — not armed yet
    }
    if clock.current_ms >= region.end_ms {
        let target = preroll_target(&timeline, session.preroll, region.start_ms);
        seeks.write(SeekToChartTime {
            target_ms: target,
            snap: None,
            attempt_start_ms: Some(region.start_ms),
        });
    }
}
```

Note the test registers `loop_watcher` in `Update` manually because the test app has no FixedUpdate pump; production wiring uses FixedUpdate via the plugin. Both consume the same logic.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums --test practice_mode`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/ab_loop.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(practice): A/B loop watcher seeks back through the shared seek op"
```

---

### Task 9: Attempt stats collection

**Files:**
- Modify: `crates/gameplay-drums/src/practice/stats.rs`
- Test: unit tests in the same file

- [ ] **Step 1: Write the failing tests**

Replace `crates/gameplay-drums/src/practice/stats.rs`:

```rust
//! Per-attempt section stats: accumulate judgements between seeks.
//!
//! An attempt spans seek-to-seek. On each `SeekToChartTime` the running
//! attempt is finalized into history (ordered before the seek applies,
//! so `clock.current_ms` is still the pre-seek time) and a fresh one
//! starts at the seek's `attempt_start_ms`. Pre-roll chips (judged
//! before the attempt start in chart time) are excluded.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use game_shell::AppState;

use super::session::PracticeSession;
use crate::events::{JudgmentEvent, NoteMissed};
use crate::resources::{Combo, GameplayClock};
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        track_attempt_stats
            .after(crate::judge::judge_lane_hit_system)
            .before(crate::seek::apply_seek_system)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}

/// Fold one judgement into the attempt (pure; unit-tested).
pub fn apply_judgment(
    attempt: &mut super::session::AttemptStats,
    kind: JudgmentKind,
    delta_ms: i64,
) {
    match kind {
        JudgmentKind::Perfect => attempt.counts.perfect += 1,
        JudgmentKind::Great => attempt.counts.great += 1,
        JudgmentKind::Good => attempt.counts.good += 1,
        JudgmentKind::Poor => attempt.counts.ok += 1,
        JudgmentKind::Miss => attempt.counts.miss += 1,
    }
    if kind == JudgmentKind::Miss {
        attempt.combo = 0;
    } else {
        attempt.combo += 1;
        attempt.max_combo = attempt.max_combo.max(attempt.combo);
        attempt.error_sum_ms += delta_ms;
        attempt.error_count += 1;
    }
}

pub fn track_attempt_stats(
    mut judgments: MessageReader<JudgmentEvent>,
    mut missed: MessageReader<NoteMissed>,
    mut seeks: MessageReader<SeekToChartTime>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut session: ResMut<PracticeSession>,
    mut combo: ResMut<Combo>,
) {
    for ev in judgments.read() {
        let judge_ms = timeline
            .judge_ms_by_idx
            .get(ev.chip_idx)
            .copied()
            .unwrap_or(i64::MIN);
        if judge_ms < session.current_attempt.start_ms {
            continue; // pre-roll chip: audible feedback only
        }
        apply_judgment(&mut session.current_attempt, ev.kind, ev.delta_ms);
    }
    for _ in missed.read() {
        // NoteMissed carries no chip index; pre-roll chips are seeded as
        // judged by the seek, so any miss here belongs to the attempt.
        session.current_attempt.counts.miss += 1;
        session.current_attempt.combo = 0;
    }
    if let Some(seek) = seeks.read().last() {
        let end_ms = clock.current_ms; // pre-seek (ordered before apply)
        let next_start = seek.attempt_start_ms.unwrap_or(seek.target_ms);
        session.roll_attempt(end_ms, next_start);
        // Fresh attempt = fresh visible combo.
        combo.current = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::AttemptStats;

    #[test]
    fn hits_accumulate_counts_combo_and_error() {
        let mut a = AttemptStats::default();
        apply_judgment(&mut a, JudgmentKind::Perfect, -5);
        apply_judgment(&mut a, JudgmentKind::Great, 20);
        apply_judgment(&mut a, JudgmentKind::Perfect, -15);
        assert_eq!(a.counts.perfect, 2);
        assert_eq!(a.counts.great, 1);
        assert_eq!(a.max_combo, 3);
        assert_eq!(a.error_count, 3);
        assert_eq!(a.error_sum_ms, 0);
    }

    #[test]
    fn miss_resets_combo_and_skips_error() {
        let mut a = AttemptStats::default();
        apply_judgment(&mut a, JudgmentKind::Perfect, 0);
        apply_judgment(&mut a, JudgmentKind::Miss, 400);
        apply_judgment(&mut a, JudgmentKind::Perfect, 0);
        assert_eq!(a.counts.miss, 1);
        assert_eq!(a.combo, 1);
        assert_eq!(a.max_combo, 1);
        assert_eq!(a.error_count, 2, "miss delta must not pollute mean error");
    }
}
```

Variant names verified against `dtx_scoring::JudgmentKind` (Perfect/Great/Good/Poor/Miss — `Poor` maps onto the `JudgmentCounts.ok` counter, matching the existing score system's convention).

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums practice::stats`
Expected: 2 passed

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/stats.rs
git commit -m "feat(practice): per-attempt stats with pre-roll exclusion"
```

---

### Task 10: Density strip widget (dtx-ui)

**Files:**
- Create: `crates/dtx-ui/src/widget/density_strip.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs` (add `pub mod density_strip;`)

- [ ] **Step 1: Write the failing tests + implementation**

Create `crates/dtx-ui/src/widget/density_strip.rs`:

```rust
//! Horizontal time-density strip: N bars over song length, plus
//! percent-positioning helpers for playhead / A / B markers.
//!
//! Practice transport uses it; any time-indexed overview can reuse it.
//! (Distinct from `density_graph`, which is per-lane, not time-indexed.)

use bevy::prelude::*;

use crate::theme::Theme;

/// Marker for the strip container (relative-positioned).
#[derive(Component)]
pub struct DensityStrip;

/// One density bar; index into the samples array.
#[derive(Component)]
pub struct DensityBar(pub usize);

/// Bar height as percent of strip height for a normalized sample.
pub fn bar_height_pct(v: f32) -> f32 {
    8.0 + v.clamp(0.0, 1.0) * 92.0
}

/// Left position (percent) for a chart time on a strip of `end_ms` length.
pub fn time_to_pct(ms: i64, end_ms: i64) -> f32 {
    if end_ms <= 0 {
        0.0
    } else {
        (ms.clamp(0, end_ms) as f64 / end_ms as f64 * 100.0) as f32
    }
}

/// Spawn the strip with one bar per sample as a child of `parent`.
/// Returns the strip entity so callers can attach marker children.
pub fn spawn_density_strip(
    parent: &mut ChildSpawnerCommands,
    samples: &[f32],
    theme: &Theme,
) -> Entity {
    let mut strip = parent.spawn((
        DensityStrip,
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(1.0),
            ..default()
        },
    ));
    strip.with_children(|bars| {
        for (i, &v) in samples.iter().enumerate() {
            bars.spawn((
                DensityBar(i),
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(bar_height_pct(v)),
                    ..default()
                },
                BackgroundColor(theme.text_secondary.with_alpha(0.55)),
            ));
        }
    });
    strip.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_height_has_floor_and_ceiling() {
        assert!((bar_height_pct(0.0) - 8.0).abs() < 1e-6);
        assert!((bar_height_pct(1.0) - 100.0).abs() < 1e-6);
        assert!((bar_height_pct(5.0) - 100.0).abs() < 1e-6);
    }

    #[test]
    fn time_to_pct_clamps_and_scales() {
        assert_eq!(time_to_pct(0, 10_000), 0.0);
        assert!((time_to_pct(5_000, 10_000) - 50.0).abs() < 1e-4);
        assert_eq!(time_to_pct(-100, 10_000), 0.0);
        assert!((time_to_pct(99_999, 10_000) - 100.0).abs() < 1e-4);
        assert_eq!(time_to_pct(500, 0), 0.0);
    }
}
```

If `ChildSpawnerCommands` is not the child-builder type this Bevy version uses, mirror the exact type from an existing spawner in `crates/dtx-ui/src/widget/difficulty_grid.rs` (the closure parameter of `with_children`). If `Theme` lacks `text_secondary`, use the field the pause overlay uses (`crates/gameplay-drums/src/pause.rs:141`).

- [ ] **Step 2: Register + run**

Add `pub mod density_strip;` to `crates/dtx-ui/src/widget/mod.rs`.

Run: `cargo test -p dtx-ui density_strip`
Expected: 2 passed

- [ ] **Step 3: Commit**

```bash
git add crates/dtx-ui/src/widget/density_strip.rs crates/dtx-ui/src/widget/mod.rs
git commit -m "feat(dtx-ui): time-density strip widget"
```

---

### Task 11: Transport strip UI

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ui.rs`

Persistent bottom strip: `time · BPM · density strip (playhead + A/B markers) · rate · attempt`. Display-only while running; scrub interaction comes with the pause panel (Task 12).

- [ ] **Step 1: Implement (helpers test-first)**

Replace `crates/gameplay-drums/src/practice/ui.rs`:

```rust
//! Practice UI: persistent transport strip + practice pause panel.
//!
//! v1 layout contract (spec §UI): every element here is a discrete,
//! self-contained UI entity — no tendrils into hud.rs internals — so the
//! future layout-editor widget registry can absorb them as widgets.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use super::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

#[derive(Component)]
struct TransportRoot;
#[derive(Component)]
struct TransportTimeText;
#[derive(Component)]
struct TransportRateText;
#[derive(Component)]
struct TransportAttemptText;
#[derive(Component)]
struct PlayheadMarker;
#[derive(Component)]
struct ScrubCursorMarker;
#[derive(Component)]
struct LoopMarkerA;
#[derive(Component)]
struct LoopMarkerB;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_transport
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_transport)
    .add_systems(
        Update,
        (update_transport_texts, update_transport_markers)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
    // Pause panel systems are added in Task 12.
}

fn marker_node(left_pct: f32, width_px: f32, color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(left_pct),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(width_px),
            ..default()
        },
        BackgroundColor(color),
    )
}

fn spawn_transport(
    mut commands: Commands,
    timeline: Res<ChipTimeline>,
    chart: Res<crate::resources::ActiveChart>,
) {
    let theme = Theme::default();
    let bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    commands
        .spawn((
            TransportRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(900),
        ))
        .with_children(|root| {
            root.spawn((
                TransportTimeText,
                Text::new("0:00.0"),
                Theme::label_font(),
                TextColor(theme.text_primary),
            ));
            root.spawn((
                Text::new(format!("{bpm:.0} BPM")),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
            let strip = spawn_density_strip(root, &timeline.density, &theme);
            root.commands().entity(strip).with_children(|markers| {
                markers.spawn((
                    PlayheadMarker,
                    marker_node(0.0, 2.0, theme.accent),
                ));
                markers.spawn((
                    ScrubCursorMarker,
                    marker_node(0.0, 2.0, Color::WHITE),
                    Visibility::Hidden,
                ));
                markers.spawn((
                    LoopMarkerA,
                    marker_node(0.0, 2.0, Color::srgb(0.3, 0.9, 0.5)),
                    Visibility::Hidden,
                ));
                markers.spawn((
                    LoopMarkerB,
                    marker_node(0.0, 2.0, Color::srgb(0.95, 0.5, 0.3)),
                    Visibility::Hidden,
                ));
            });
            root.spawn((
                TransportRateText,
                Text::new("x1.00"),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
            root.spawn((
                TransportAttemptText,
                Text::new(""),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
        });
}

fn despawn_transport(mut commands: Commands, roots: Query<Entity, With<TransportRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn update_transport_texts(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TransportTimeText>>,
        Query<&mut Text, With<TransportRateText>>,
        Query<&mut Text, With<TransportAttemptText>>,
    )>,
) {
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format_chart_time(clock.current_ms);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("x{:.2}", session.rate);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        let n = session.attempt_history.len() + 1;
        let a = &session.current_attempt;
        t.0 = if a.has_data() {
            format!("attempt #{n}  {:.1}%", a.accuracy_pct())
        } else {
            format!("attempt #{n}")
        };
    }
}

#[allow(clippy::type_complexity)]
fn update_transport_markers(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut markers: ParamSet<(
        Query<&mut Node, With<PlayheadMarker>>,
        Query<(&mut Node, &mut Visibility), With<ScrubCursorMarker>>,
        Query<(&mut Node, &mut Visibility), With<LoopMarkerA>>,
        Query<(&mut Node, &mut Visibility), With<LoopMarkerB>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
        match session.scrub_cursor_ms {
            Some(ms) => {
                node.left = Val::Percent(time_to_pct(ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    let region = session.loop_region;
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match region {
            Some(r) => {
                node.left = Val::Percent(time_to_pct(r.start_ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = markers.p3().single_mut() {
        match region.filter(|r| r.end_ms != i64::MAX) {
            Some(r) => {
                node.left = Val::Percent(time_to_pct(r.end_ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_time_formats_minutes_seconds_tenths() {
        assert_eq!(format_chart_time(0), "0:00.0");
        assert_eq!(format_chart_time(83_450), "1:23.4");
        assert_eq!(format_chart_time(-50), "0:00.0");
    }
}
```

API-drift notes for the executor: `root.commands()` inside `with_children` may need to be restructured (spawn markers by passing the strip entity and using `commands.entity(strip).with_children(...)` after the outer `with_children` closure ends). `single_mut()` returning `Result` matches this Bevy version's query API as used elsewhere (`hud_root.single()` at `scroll.rs:99`); mirror the local convention if it differs.

- [ ] **Step 2: Run tests + check**

Run: `cargo test -p gameplay-drums format_chart_time && cargo check -p gameplay-drums`
Expected: 1 passed, clean

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/ui.rs
git commit -m "feat(practice): persistent transport strip with density, playhead, loop markers"
```

---

### Task 12: Practice pause panel + scrub interaction

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ui.rs` (extend)
- Modify: `crates/gameplay-drums/src/pause.rs` (skip normal overlay/input in practice)

- [ ] **Step 1: Suppress the normal pause overlay in practice**

In `crates/gameplay-drums/src/pause.rs`:

```rust
fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
) {
    if practice.is_some() {
        return; // practice pause panel owns the overlay
    }
    // ... existing body unchanged ...
```

and

```rust
fn pause_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    // ... existing params ...
) {
    if practice.is_some() {
        return;
    }
    // ... existing body unchanged ...
```

- [ ] **Step 2: Add the practice panel**

Append to `crates/gameplay-drums/src/practice/ui.rs`:

```rust
#[derive(Component)]
struct PracticePanel;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum PracticeItem {
    Resume,
    Scrub,
    RestartSection,
    SetA,
    SetB,
    ClearLoop,
    Rate,
    Snap,
    Preroll,
    ExitPractice,
}

impl PracticeItem {
    const ORDER: [PracticeItem; 10] = [
        PracticeItem::Resume,
        PracticeItem::Scrub,
        PracticeItem::RestartSection,
        PracticeItem::SetA,
        PracticeItem::SetB,
        PracticeItem::ClearLoop,
        PracticeItem::Rate,
        PracticeItem::Snap,
        PracticeItem::Preroll,
        PracticeItem::ExitPractice,
    ];
}

#[derive(Resource, Default)]
struct PracticeSelection(usize);

#[derive(Component)]
struct AttemptHistoryText;

fn practice_item_label(item: PracticeItem, session: &PracticeSession) -> String {
    match item {
        PracticeItem::Resume => "Resume".into(),
        PracticeItem::Scrub => match session.scrub_cursor_ms {
            Some(ms) => format!("Scrub  ◀ {} ▶   (Enter: play here)", format_chart_time(ms)),
            None => "Scrub  ◀ ▶".into(),
        },
        PracticeItem::RestartSection => "Restart section".into(),
        PracticeItem::SetA => "Set A here".into(),
        PracticeItem::SetB => "Set B here".into(),
        PracticeItem::ClearLoop => "Clear loop".into(),
        PracticeItem::Rate => format!("Rate  ◀ x{:.2} ▶", session.rate),
        PracticeItem::Snap => format!("Snap  ◀ {} ▶", session.snap.label()),
        PracticeItem::Preroll => format!("Pre-roll  ◀ {} ▶", session.preroll.label()),
        PracticeItem::ExitPractice => "Exit practice".into(),
    }
}

fn attempt_history_text(session: &PracticeSession) -> String {
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in session.attempt_history.iter().enumerate().rev().take(5) {
        lines.push(format!(
            "#{}  {:.1}%  {:+.0}ms  x{:.2}",
            i + 1,
            a.accuracy_pct,
            a.mean_error_ms,
            a.rate
        ));
    }
    lines.join("\n")
}

fn spawn_practice_panel(
    mut commands: Commands,
    mut selection: ResMut<PracticeSelection>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
) {
    selection.0 = 0;
    session.scrub_cursor_ms = Some(clock.current_ms);
    let theme = Theme::default();
    commands
        .spawn((
            PracticePanel,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("PRACTICE"),
                Theme::title_font(),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(18.0)),
                    ..default()
                },
            ));
            for item in PracticeItem::ORDER {
                root.spawn((
                    item,
                    Text::new(practice_item_label(item, &session)),
                    Theme::hud_font(),
                    TextColor(theme.text_secondary),
                ));
            }
            root.spawn((
                AttemptHistoryText,
                Text::new(attempt_history_text(&session)),
                Theme::label_font(),
                TextColor(theme.text_secondary),
                Node {
                    margin: UiRect::top(Val::Px(18.0)),
                    ..default()
                },
            ));
        });
}

fn despawn_practice_panel(
    mut commands: Commands,
    panels: Query<Entity, With<PracticePanel>>,
    mut session: Option<ResMut<PracticeSession>>,
) {
    for e in &panels {
        commands.entity(e).despawn();
    }
    if let Some(session) = session.as_mut() {
        session.scrub_cursor_ms = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn practice_panel_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<PracticeSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut requests: MessageWriter<TransitionRequest>,
    mut rows: Query<(&PracticeItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<PracticeItem>)>,
) {
    let count = PracticeItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
    }
    let selected = PracticeItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        match selected {
            PracticeItem::Scrub => {
                let cur = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                session.scrub_cursor_ms =
                    Some(timeline.snap_neighbor(cur, session.snap, dir));
            }
            PracticeItem::Rate => session.step_rate(dir),
            PracticeItem::Snap => session.snap = session.snap.next(),
            PracticeItem::Preroll => session.preroll = session.preroll.next(),
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match selected {
            PracticeItem::Resume => next_pause.set(PauseState::Running),
            PracticeItem::Scrub => {
                let intent = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            PracticeItem::RestartSection => {
                let intent = session
                    .loop_region
                    .map(|r| r.start_ms)
                    .unwrap_or(session.current_attempt.start_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            PracticeItem::SetA => {
                let ms = timeline.bar_start_before(
                    session.scrub_cursor_ms.unwrap_or(clock.current_ms),
                );
                session.set_loop_start(ms);
            }
            PracticeItem::SetB => {
                let cursor = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                let mut ms = timeline.bar_start_before(cursor);
                // Min region: one bar. If B lands on/before A, push B one
                // bar past A.
                if let Some(r) = session.loop_region {
                    if ms <= r.start_ms {
                        ms = timeline.snap_neighbor(
                            r.start_ms,
                            crate::timeline::SnapDivisor::Bar,
                            1,
                        );
                    }
                }
                session.set_loop_end(ms);
            }
            PracticeItem::ClearLoop => session.loop_region = None,
            PracticeItem::Rate | PracticeItem::Snap | PracticeItem::Preroll => {}
            PracticeItem::ExitPractice => {
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongSelect);
            }
        }
    }

    // Repaint rows every frame (labels are cheap, list is 10 rows).
    let theme = Theme::default();
    for (item, mut text, mut color) in &mut rows {
        text.0 = practice_item_label(*item, &session);
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }
    if let Ok(mut t) = history.single_mut() {
        t.0 = attempt_history_text(&session);
    }
}
```

And extend the `plugin` fn in the same file (replace the pause-panel comment):

```rust
    app.init_resource::<PracticeSelection>()
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_practice_panel.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), despawn_practice_panel)
        .add_systems(
            Update,
            practice_panel_input
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<PracticeSession>),
        );
```

- [ ] **Step 3: Check + tests**

Run: `cargo check -p gameplay-drums && cargo test -p gameplay-drums`
Expected: clean, all pass

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/practice/ui.rs crates/gameplay-drums/src/pause.rs
git commit -m "feat(practice): practice pause panel with scrub, A/B, rate, snap, pre-roll"
```

---

### Task 13: Song-select entry (Shift+Enter = Practice)

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (the Enter branch near line 1242 — the system containing `request_transition(&mut requests, AppState::SongLoading)`)

- [ ] **Step 1: Set the intent on selection**

Add `mut practice_intent: ResMut<game_shell::PracticeIntent>,` to that system's parameters, then change the Enter branch:

```rust
    } else if keys.just_pressed(KeyCode::Enter) {
        if let Some(chart_idx) = selection.chart_index(&selection_state)
            && let Some(song) = db.songs.get(chart_idx)
        {
            let practice = keys.pressed(KeyCode::ShiftLeft)
                || keys.pressed(KeyCode::ShiftRight);
            practice_intent.0 = practice;
            info!(
                "SongSelect: selected {} ({}){}",
                song.title,
                SongFolderView::difficulty_label(selection.difficulty),
                if practice { " [practice]" } else { "" }
            );
            selected_song.0 = Some(song.path.clone());
            request_transition(&mut requests, AppState::SongLoading);
        }
    }
```

If the system is already at the Bevy param ceiling, wrap the new param plus `selected_song`/`requests` in a `#[derive(SystemParam)]` struct following the pattern in `seek.rs`.

- [ ] **Step 2: Check**

Run: `cargo check -p game-menu`
Expected: clean

- [ ] **Step 3: Commit**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "feat(song-select): Shift+Enter starts the chart in practice mode"
```

---

### Task 14: Seek integration tests + full verification

**Files:**
- Modify: `crates/gameplay-drums/tests/practice_mode.rs` (extend)

- [ ] **Step 1: Add seek round-trip tests**

Append to `tests/practice_mode.rs`:

```rust
use gameplay_drums::seek::SeekToChartTime;
use gameplay_drums::se_scheduler::PlayedSeChips;

fn send_seek(app: &mut App, target_ms: i64) {
    app.world_mut()
        .resource_mut::<Messages<SeekToChartTime>>()
        .write(SeekToChartTime {
            target_ms,
            snap: None,
            attempt_start_ms: None,
        });
}

#[test]
fn forward_seek_seeds_skip_sets_and_jumps_clock() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();

    assert_eq!(
        app.world().resource::<GameplayClock>().current_ms,
        9_000
    );
    let judged = &app.world().resource::<JudgedChips>().0;
    // Chips 0..=3 land before 9000ms at default 120 BPM (measure=2000ms).
    assert!(judged.contains(&0) && judged.contains(&3));
    assert!(!judged.contains(&4), "chips past target stay live");
}

#[test]
fn backward_seek_prunes_skip_sets() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();
    send_seek(&mut app, 0);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 0);
    assert!(
        app.world().resource::<JudgedChips>().0.is_empty(),
        "backward seek must un-mark judged chips"
    );
    assert!(app.world().resource::<PlayedSeChips>().0.is_empty());
}

#[test]
fn seek_is_inert_without_practice_in_normal_play() {
    // Regression guard: with no PracticeSession and no seek messages,
    // a normal stage run is untouched by the new systems.
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(10_000));
    }
    app.update();
    assert!(
        app.world()
            .resource::<DrumsStageCompletion>()
            .end_requested,
        "normal end-of-stage unchanged"
    );
}
```

If `Messages<T>` is not the resource name for writing messages from the world in this Bevy version, mirror how existing tests write messages (see `lane_hit_count` in `autoplay.rs` tests which reads via `Messages<LaneHit>` — writing uses the same resource's `.write(...)`).

- [ ] **Step 2: Full verification**

Run:
```
cargo test -p gameplay-drums
cargo test -p dtx-ui
cargo check --workspace
cargo fmt --all -- --check
```
Expected: all green. Fix any fmt drift with `cargo fmt --all`.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/tests/practice_mode.rs
git commit -m "test(practice): seek round-trip and normal-play regression coverage"
```

---

## Manual verification checklist (human, post-implementation)

1. Song select → Shift+Enter on a chart with a single BGM → transport strip visible, plays normally.
2. Esc → practice panel; Scrub ←/→ moves cursor by bar; Enter → resumes ~1 bar before the cursor with BGM audible at the right position.
3. Set A / Set B → markers on the strip; loop plays and snaps back with pre-roll; attempt counter increments; accuracy per attempt in panel history.
4. Rate 0.75 → audio slower and lower-pitched, keysounds match BGM pitch, notes scroll slower, judging feels consistent. **Confirms the channel-rate inheritance assumption from Task 7.**
5. Clear loop → play to the end → StageClear → Result shows, but no new entry in the score store and `<chart>.score.ini` mtime unchanged.
6. Normal play (plain Enter) → everything behaves exactly as before (no strip, pause menu unchanged, score saves).
7. Sliced-BGM chart (multiple BGM chips): seek into the middle → correct slice plays from the correct offset.
8. No-BGM chart: seek works, clock free-runs, no audio errors in the log.

## Spec-coverage self-review (done while writing)

- Seek op steps 1-8 (spec §Seek Op) → Tasks 1, 3, 4 (snap, stop audio, reseed, despawn, restart-at-offset via governing chip incl. sliced-BGM generalization, clock seek + re-armed audio snap, pre-roll via `preroll_target`, stale-position avoided by always restarting on a fresh instance).
- Session state & A/B (spec §Session State) → Tasks 5, 8.
- Per-attempt stats + pre-roll exclusion + gauge freeze → Tasks 5, 9.
- Rate semantics → Task 7.
- UI: transport strip, markers, panel, layering contract (self-contained entities) → Tasks 10-12.
- Entry + no-save + end-of-stage suppression → Tasks 6, 13.
- Testing section of spec → Tasks 6, 8, 14 (+ unit tests throughout). Audio-rate audibility and sliced-BGM behavior are manual checks (no audible assertions in CI).
- Known deviation: spec's `SnapDivisor::Quarter` is implemented as half-beat subdivision (beat midpoints); scrub-by-mouse is deferred to a follow-up (keyboard scrub ships; strip is display-only while running either way).
