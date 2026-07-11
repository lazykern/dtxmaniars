# Guided Calibration and Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the existing input-offset tap test into one guided flow that shows the active MIDI device, sample spread, estimated offset with a confidence rating, measured frame rate, and frame-time spikes — while keeping manual adjustment and refusing to apply a low-confidence estimate.

**Architecture:** Everything builds on the shipped tap test in `crates/gameplay-drums/src/editor/calibration.rs` (state machine `Idle → Collecting → Done`, 12 samples, median, Enter-gated apply into `ConfigDraft`). We add pure statistics (spread + confidence), a device-name resource (currently the connected port name is logged and dropped), a frame-time monitor resource, disconnect-resilient sample collection, and a richer overlay. No new screens or states — the flow stays inside the Customize surface's Gameplay tab.

**Tech Stack:** Bevy 0.19 (existing), no new dependencies.

**Source basis (verified 2026-07-11):**
- Tap test: `crates/gameplay-drums/src/editor/calibration.rs` (257 lines). Pure fns `error_ms` (:7), `median` (:21), `suggested_offset` (:34). `CalibrationState` (:39) = `Idle | Collecting { samples: Vec<i32>, prev_metronome, prev_timing_lines, prev_autoplay } | Done { median: i32, prev_* }`. `TARGET_SAMPLES = 12` (:58). Systems: `start_calibration` (:81), `collect_taps` (:101, reads `LaneHit`, beat from `chart.chart.metadata.bpm` default 120), `confirm_or_cancel` (:143, Enter writes `draft.0.gameplay.input_offset_ms`), `render_overlay` (:188). Entry: `CalibrateButton` in `editor/panel.rs:1457`.
- Offsets: `GameplayConfig::input_offset_ms` / `bgm_adjust_ms` (i32 ms, `crates/dtx-config/src/lib.rs:205,208`), clamps ±300 (:158-159). Consumed at `judge.rs:110` / `judge.rs:162`.
- MIDI: `RealMidiSource::connect(port_filter) -> Result<(Self, String), String>` returns the port name (`crates/dtx-input/src/midi.rs:162`); `connect_midi` at `crates/gameplay-drums/src/lib.rs:364` logs the name at :375 and drops it. Only `game_shell::MidiConnected(pub bool)` exists (`crates/game-shell/src/nav.rs:50`).
- Frame diagnostics: none in source. `bevy_framepace` registered unconfigured (`app/dtxmaniars-desktop/src/main.rs:60`). No `FrameTimeDiagnosticsPlugin`, no `DiagnosticsStore` use. `ShowPerfInfo` toggle exists without a renderer.
- Stats helpers: only `median` (calibration.rs:21) and `mean_*` in practice. No stddev anywhere.
- Test conventions: pure fns in `#[cfg(test)]` at file bottom; headless Bevy App tests in `crates/gameplay-drums/tests/`; real tap accuracy is not headlessly assertable — manual check via bevy-brp MCP (launch, F1 → surface, screenshot).

**Explicit non-goals:** auto-applying any estimate (Enter stays the only apply path); a separate calibration AppState; audio-latency measurement hardware loopback (audio timing stays the manual BGM Offset slider, now explained in the flow); video-offset as a separate stored value.

---

### Task 1: Pure statistics — spread and confidence

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests` in `calibration.rs`:

```rust
#[test]
fn spread_of_identical_samples_is_zero() {
    assert_eq!(spread_ms(&[10, 10, 10, 10]), 0.0);
}

#[test]
fn spread_is_sample_stddev() {
    // samples 2,4,4,4,5,5,7,9: mean 5, variance 32/7, stddev ~2.138
    let s = spread_ms(&[2, 4, 4, 4, 5, 5, 7, 9]);
    assert!((s - 2.138).abs() < 0.01, "got {s}");
}

#[test]
fn spread_of_short_input_is_zero() {
    assert_eq!(spread_ms(&[]), 0.0);
    assert_eq!(spread_ms(&[5]), 0.0);
}

#[test]
fn confidence_tiers() {
    assert_eq!(confidence(12, 8.0), Confidence::High);
    assert_eq!(confidence(12, 20.0), Confidence::Medium);
    assert_eq!(confidence(12, 40.0), Confidence::Low);
    // too few samples is always Low regardless of spread
    assert_eq!(confidence(5, 2.0), Confidence::Low);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums --lib editor::calibration -j 2`
Expected: FAIL — `spread_ms`, `confidence`, `Confidence` not found.

- [ ] **Step 3: Implement**

Add near the top of `calibration.rs` (after `suggested_offset`):

```rust
/// Sample standard deviation of tap errors, in ms. 0.0 for fewer than 2 samples.
pub fn spread_ms(samples: &[i32]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let n = samples.len() as f64;
    let mean = samples.iter().map(|&s| f64::from(s)).sum::<f64>() / n;
    let var = samples
        .iter()
        .map(|&s| (f64::from(s) - mean).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    var.sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// High: full sample count and tight spread. Low estimates must never be applied.
pub fn confidence(sample_count: usize, spread: f64) -> Confidence {
    if sample_count < TARGET_SAMPLES {
        return Confidence::Low;
    }
    if spread <= 12.0 {
        Confidence::High
    } else if spread <= 30.0 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}
```

Thresholds rationale (comment-worthy in code): YARG documents 20–40 ms as the range players start perceiving; a spread ≤12 ms means the median is trustworthy to well under that, ≤30 ms is usable with a warning, above that the median is noise.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gameplay-drums --lib editor::calibration -j 2`
Expected: PASS (new tests + the existing `error_ms`/`median`/`suggested_offset` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat(calibration): pure spread and confidence estimation"
```

---

### Task 2: Carry spread + confidence through the state machine; gate apply on confidence

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`

- [ ] **Step 1: Extend `CalibrationState::Done`**

Change the `Done` variant (currently `{ median: i32, prev_metronome, prev_timing_lines, prev_autoplay }`) to:

```rust
Done {
    median: i32,
    spread: f64,
    confidence: Confidence,
    prev_metronome: bool,
    prev_timing_lines: bool,
    prev_autoplay: bool,
},
```

- [ ] **Step 2: Compute them in `collect_taps`**

At the ≥ `TARGET_SAMPLES` transition (calibration.rs:101 system, where `median` is computed today):

```rust
let med = median(&samples);
let spread = spread_ms(&samples);
let conf = confidence(samples.len(), spread);
*state = CalibrationState::Done {
    median: med,
    spread,
    confidence: conf,
    prev_metronome,
    prev_timing_lines,
    prev_autoplay,
};
```

- [ ] **Step 3: Gate Enter in `confirm_or_cancel`**

In the `Done` arm (calibration.rs:143), wrap the existing apply:

```rust
if keys.just_pressed(KeyCode::Enter) {
    if confidence == Confidence::Low {
        // Refuse: low-confidence estimates are never applied (roadmap trust rule).
        // Leave state in Done; the overlay explains and offers R to retry.
        return;
    }
    draft.0.gameplay.input_offset_ms =
        suggested_offset(median, dtx_config::INPUT_OFFSET_CLAMP_MS);
    // ... existing restore of prev_* and reset to Idle unchanged
}
```

Add an `R` key handler in the `Done` arm that returns to `Collecting` with EMPTY samples (fresh run), keeping the saved `prev_*` values:

```rust
if keys.just_pressed(KeyCode::KeyR) {
    *state = CalibrationState::Collecting {
        samples: Vec::new(),
        prev_metronome,
        prev_timing_lines,
        prev_autoplay,
    };
    return;
}
```

- [ ] **Step 4: Write the state-machine tests**

These are pure-ish; follow the existing convention of testing decision logic via extracted helpers. Add a pure decision fn plus tests:

```rust
/// What Enter should do in the Done state.
pub fn apply_allowed(confidence: Confidence) -> bool {
    confidence != Confidence::Low
}

#[test]
fn low_confidence_blocks_apply() {
    assert!(!apply_allowed(Confidence::Low));
    assert!(apply_allowed(Confidence::Medium));
    assert!(apply_allowed(Confidence::High));
}
```

Use `apply_allowed(confidence)` in the Enter handler above instead of the inline `==` check.

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p gameplay-drums --lib editor::calibration -j 2 && cargo check -p gameplay-drums -j 2`
Expected: PASS / clean build (compiler errors will point at every `Done { .. }` pattern that needs the new fields — fix all, including `render_overlay`; a temporary `spread: 0.0, confidence: Confidence::Low` placeholder in `render_overlay` is fine until Task 5 rewrites it).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat(calibration): confidence-gated apply, retry key, spread in Done state"
```

---

### Task 3: Active MIDI device name resource

**Files:**
- Modify: `crates/game-shell/src/nav.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (`connect_midi`, ~:364-399)

- [ ] **Step 1: Write the failing test**

In `crates/game-shell/src/nav.rs` tests (existing mod at :59):

```rust
#[test]
fn midi_device_name_defaults_none() {
    assert_eq!(MidiDeviceName::default().0, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p game-shell -j 2 midi_device_name`
Expected: FAIL — `MidiDeviceName` not found.

- [ ] **Step 3: Implement the resource**

In `nav.rs`, next to `MidiConnected` (:50):

```rust
/// Name of the connected MIDI input port, when one is connected.
#[derive(Resource, Debug, Default, Clone)]
pub struct MidiDeviceName(pub Option<String>);
```

Register it in `nav::plugin` alongside `MidiConnected` (`app.init_resource::<MidiDeviceName>();`) and re-export from `game-shell/src/lib.rs` next to the `MidiConnected` re-export (:14).

- [ ] **Step 4: Populate it on connect/disconnect**

In `crates/gameplay-drums/src/lib.rs` `connect_midi` (:364): add `mut device_name: ResMut<game_shell::MidiDeviceName>` to the system params. Where the success path currently does `info!("MIDI connected: {name}")` and `connected.0 = true` (:375), add `device_name.0 = Some(name.clone());`. On the failure/disconnect path where `connected.0 = false` is set, add `device_name.0 = None;`.

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p game-shell -j 2 && cargo check -p gameplay-drums --features midi -j 2 && cargo check -p gameplay-drums --no-default-features -j 2`
Expected: PASS / clean. The no-default-features check matters: `connect_midi` is `#[cfg(feature = "midi")]`; the resource itself must not be feature-gated (keyboard-only builds show "keyboard" in the overlay).

- [ ] **Step 6: Commit**

```bash
git add crates/game-shell/src/nav.rs crates/game-shell/src/lib.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(midi): expose connected device name as a resource"
```

---

### Task 4: Frame-time monitor (refresh estimate + spike counter)

**Files:**
- Create: `crates/gameplay-drums/src/frame_stats.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (module + plugin registration)

- [ ] **Step 1: Write the failing tests**

`crates/gameplay-drums/src/frame_stats.rs`:

```rust
//! Rolling frame-time statistics for the calibration/diagnostics overlay.
//! Pure ring-buffer math; fed by `Time::delta` every frame.

use bevy::prelude::*;

const WINDOW: usize = 240; // ~4s at 60 Hz, ~1.7s at 144 Hz
/// A frame counts as a spike when it takes over 1.5x the median frame time.
const SPIKE_FACTOR: f64 = 1.5;

#[derive(Resource, Debug, Default)]
pub struct FrameStats {
    deltas_ms: Vec<f64>,
    cursor: usize,
    pub spikes_in_window: u32,
}

impl FrameStats {
    pub fn push(&mut self, delta_ms: f64) {
        if delta_ms <= 0.0 {
            return;
        }
        if self.deltas_ms.len() < WINDOW {
            self.deltas_ms.push(delta_ms);
        } else {
            self.deltas_ms[self.cursor] = delta_ms;
            self.cursor = (self.cursor + 1) % WINDOW;
        }
        self.recount();
    }

    /// Estimated display rate in Hz from the median frame time (vsync makes
    /// the median track the refresh interval).
    pub fn refresh_estimate_hz(&self) -> Option<f64> {
        let med = self.median_ms()?;
        Some(1000.0 / med)
    }

    pub fn median_ms(&self) -> Option<f64> {
        if self.deltas_ms.len() < 30 {
            return None; // not enough data to say anything
        }
        let mut v = self.deltas_ms.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Some(v[v.len() / 2])
    }

    fn recount(&mut self) {
        let Some(med) = self.median_ms() else {
            self.spikes_in_window = 0;
            return;
        };
        self.spikes_in_window = self
            .deltas_ms
            .iter()
            .filter(|&&d| d > med * SPIKE_FACTOR)
            .count() as u32;
    }
}

pub fn sample_frame_time(time: Res<Time>, mut stats: ResMut<FrameStats>) {
    stats.push(time.delta_secs_f64() * 1000.0);
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FrameStats>()
        .add_systems(Update, sample_frame_time);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_estimate_from_steady_60hz() {
        let mut s = FrameStats::default();
        for _ in 0..120 {
            s.push(16.6667);
        }
        let hz = s.refresh_estimate_hz().unwrap();
        assert!((hz - 60.0).abs() < 0.5, "got {hz}");
        assert_eq!(s.spikes_in_window, 0);
    }

    #[test]
    fn spikes_counted_against_median() {
        let mut s = FrameStats::default();
        for i in 0..120 {
            // every 20th frame takes 50ms instead of ~16.7ms
            s.push(if i % 20 == 0 { 50.0 } else { 16.7 });
        }
        assert_eq!(s.spikes_in_window, 6);
        // median still reflects the steady rate
        assert!((s.refresh_estimate_hz().unwrap() - 59.9).abs() < 1.0);
    }

    #[test]
    fn no_estimate_before_enough_samples() {
        let mut s = FrameStats::default();
        for _ in 0..10 {
            s.push(16.7);
        }
        assert!(s.refresh_estimate_hz().is_none());
    }

    #[test]
    fn window_rolls_over() {
        let mut s = FrameStats::default();
        for _ in 0..WINDOW {
            s.push(33.3); // fill with 30 Hz
        }
        for _ in 0..WINDOW {
            s.push(16.7); // fully replace with 60 Hz
        }
        assert!((s.refresh_estimate_hz().unwrap() - 59.9).abs() < 1.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (module not registered)**

Add `pub mod frame_stats;` to `crates/gameplay-drums/src/lib.rs` (with the other `pub mod` lines near the top), then:

Run: `cargo test -p gameplay-drums --lib frame_stats -j 2`
Expected: PASS if implementation matches (this file is written test-first as one unit; if any test fails, fix the implementation, not the test).

- [ ] **Step 3: Register the plugin**

In `crates/gameplay-drums/src/lib.rs`, inside the main plugin build where other sub-plugins register (near `editor` plugin registration), add:

```rust
app.add_plugins(frame_stats::plugin);
```

Keep it unconditional (Update-schedule, one push per frame — negligible; it must run before the user opens calibration so the window is already warm).

- [ ] **Step 4: Verify the schedule still builds**

Run: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering -j 2`
Expected: PASS (guard test — green unit tests alone do not prove the schedule builds).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/frame_stats.rs crates/gameplay-drums/src/lib.rs
git commit -m "feat(diagnostics): rolling frame-time stats with refresh estimate and spike counter"
```

---

### Task 5: Guided overlay — device, spread, confidence, refresh, spikes, audio-offset pointer

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs` (`render_overlay`, :188)

- [ ] **Step 1: Extract a pure text builder with tests**

The overlay is a single centered `Text`; keep it that way but build the string in a pure fn:

```rust
pub struct OverlayFacts<'a> {
    pub device: Option<&'a str>,
    pub refresh_hz: Option<f64>,
    pub spikes: u32,
}

pub fn overlay_text(state: &CalibrationState, facts: &OverlayFacts) -> Option<String> {
    let device_line = match facts.device {
        Some(d) => format!("Input: {d} (MIDI)"),
        None => "Input: keyboard".to_string(),
    };
    let frame_line = match facts.refresh_hz {
        Some(hz) => format!("Display: ~{hz:.0} Hz · {} frame spikes recently", facts.spikes),
        None => "Display: measuring…".to_string(),
    };
    match state {
        CalibrationState::Idle => None,
        CalibrationState::Collecting { samples, .. } => Some(format!(
            "CALIBRATION — tap any pad to the metronome\n{device_line}\n{frame_line}\n\
             Samples: {}/{}\nEsc cancel",
            samples.len(),
            TARGET_SAMPLES
        )),
        CalibrationState::Done {
            median,
            spread,
            confidence,
            ..
        } => {
            let conf_line = match confidence {
                Confidence::High => "Confidence: HIGH",
                Confidence::Medium => "Confidence: MEDIUM — consider one more run",
                Confidence::Low => "Confidence: LOW — result NOT applicable, press R to retry",
            };
            let action_line = if apply_allowed(*confidence) {
                "Enter apply · R retry · Esc cancel"
            } else {
                "R retry · Esc cancel"
            };
            Some(format!(
                "CALIBRATION RESULT\n{device_line}\n{frame_line}\n\
                 Suggested input offset: {median:+} ms (spread ±{spread:.0} ms)\n{conf_line}\n\
                 Audio feels late/early? Adjust BGM Offset in the Gameplay tab.\n{action_line}"
            ))
        }
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    fn done(conf: Confidence) -> CalibrationState {
        CalibrationState::Done {
            median: -12,
            spread: 8.0,
            confidence: conf,
            prev_metronome: false,
            prev_timing_lines: false,
            prev_autoplay: false,
        }
    }

    #[test]
    fn idle_renders_nothing() {
        let facts = OverlayFacts { device: None, refresh_hz: None, spikes: 0 };
        assert!(overlay_text(&CalibrationState::Idle, &facts).is_none());
    }

    #[test]
    fn low_confidence_hides_apply_hint() {
        let facts = OverlayFacts { device: Some("TD-17"), refresh_hz: Some(144.0), spikes: 2 };
        let t = overlay_text(&done(Confidence::Low), &facts).unwrap();
        assert!(t.contains("NOT applicable"));
        assert!(!t.contains("Enter apply"));
        assert!(t.contains("TD-17"));
        assert!(t.contains("144 Hz"));
    }

    #[test]
    fn high_confidence_shows_apply_and_signed_offset() {
        let facts = OverlayFacts { device: None, refresh_hz: Some(60.0), spikes: 0 };
        let t = overlay_text(&done(Confidence::High), &facts).unwrap();
        assert!(t.contains("Enter apply"));
        assert!(t.contains("-12 ms"));
        assert!(t.contains("keyboard"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gameplay-drums --lib editor::calibration -j 2`
Expected: PASS.

- [ ] **Step 3: Rewire `render_overlay`**

Change `render_overlay` to gather `Res<game_shell::MidiDeviceName>`, `Res<game_shell::MidiConnected>`, `Res<crate::frame_stats::FrameStats>`, build `OverlayFacts` (`device: connected.0.then(|| name.0.as_deref()).flatten()`), call `overlay_text`, and set/despawn the overlay `Text` accordingly. Keep the existing spawn/despawn structure and styling; only the string source changes.

- [ ] **Step 4: Build + schedule guard**

Run: `cargo check -p gameplay-drums -j 2 && cargo test -p gameplay-drums --test fixed_update_schedule_ordering -j 2`
Expected: clean / PASS.

- [ ] **Step 5: Manual check (bevy-brp)**

Launch `dtxmaniars` from the repo, enter a song, open the Customize surface, Gameplay tab → Calibrate. Screenshot: overlay shows device line, display line with a plausible Hz, sample counter. Tap to 12; screenshot the result panel; verify Enter applies only on Medium/High.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat(calibration): guided overlay with device, spread, confidence, frame diagnostics"
```

---

### Task 6: Preserve samples across device disconnect

Roadmap failure-handling rule: "Preserve calibration samples across device disconnect and ask the player to retry."

**Files:**
- Modify: `crates/gameplay-drums/src/editor/calibration.rs`

- [ ] **Step 1: Extend the Collecting state with a paused flag**

Add a field to `Collecting`:

```rust
Collecting {
    samples: Vec<i32>,
    paused_for_device: bool,
    prev_metronome: bool,
    prev_timing_lines: bool,
    prev_autoplay: bool,
},
```

(Compiler drives the pattern fixes; `start_calibration` initializes `paused_for_device: false`.)

- [ ] **Step 2: Watch for disconnect**

New system in `calibration.rs`, registered in the calibration plugin (same gating as `collect_taps`):

```rust
/// Pause collection when the MIDI device drops mid-run; samples are kept.
/// Keyboard-only runs (device was never connected) are unaffected.
pub fn watch_device_drop(
    connected: Res<game_shell::MidiConnected>,
    mut state: ResMut<CalibrationState>,
) {
    if !connected.is_changed() {
        return;
    }
    if let CalibrationState::Collecting { paused_for_device, samples, .. } = &mut *state {
        if !connected.0 && !samples.is_empty() {
            *paused_for_device = true;
        } else if connected.0 {
            *paused_for_device = false;
        }
    }
}
```

In `collect_taps`, early-return when `paused_for_device` is true (taps from a flapping device must not pollute the run). In `overlay_text`'s `Collecting` arm, when paused render:
`"Device disconnected — samples kept ({}/{}). Reconnect to continue, Esc to cancel."`

- [ ] **Step 3: Tests**

```rust
#[test]
fn overlay_explains_device_drop() {
    let state = CalibrationState::Collecting {
        samples: vec![5, -3, 8],
        paused_for_device: true,
        prev_metronome: false,
        prev_timing_lines: false,
        prev_autoplay: false,
    };
    let facts = OverlayFacts { device: None, refresh_hz: None, spikes: 0 };
    let t = overlay_text(&state, &facts).unwrap();
    assert!(t.contains("samples kept (3/12)"));
}
```

Plus a headless App test in `crates/gameplay-drums/tests/` is NOT needed here — `watch_device_drop` is trivial and `MidiConnected` flapping is manual-matrix territory (roadmap verification names two MIDI modules).

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p gameplay-drums --lib editor::calibration -j 2 && cargo check -p gameplay-drums -j 2`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/editor/calibration.rs
git commit -m "feat(calibration): keep samples across MIDI disconnect, prompt to resume"
```

---

## Verification (whole plan)

1. `cargo test -p gameplay-drums -p game-shell -j 2` green, including the schedule-ordering guard.
2. Manual (bevy-brp): calibrate with keyboard → overlay says "Input: keyboard"; result shows signed offset + spread; Low confidence refuses Enter and offers R.
3. Manual with a MIDI module: device name appears; unplug mid-run → "samples kept" message; replug → collection resumes.
4. Display line shows ~60/~120/~144 Hz matching the test display; a deliberately induced stutter (drag another window) increments the spike counter.
5. `cargo fmt -p gameplay-drums -- src/editor/calibration.rs src/frame_stats.rs` and scoped clippy clean.

## Success-check mapping (roadmap)

- "Calibration preserves manual control and rejects low-confidence auto-apply" → Tasks 2, 5.
- "Preserve calibration samples across device disconnect" → Task 6.
- "frame-time overruns are visible" → Task 4-5 (spike counter in the flow).
