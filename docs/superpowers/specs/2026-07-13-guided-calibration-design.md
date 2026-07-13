# Guided Calibration Design

Date: 2026-07-13
Status: approved through the program owner’s standing authorization
Scope: Cycle 3 — drums Customize surface

## Problem

The current Customize calibrator measures taps against the selected chart's BPM grid. Its result therefore depends on song content and playback state; it accepts every sample and reports only a median. It is not a reliable way to tell whether an input adjustment is trustworthy.

`InputHit` already carries a monotonic physical-input timestamp for both keyboard and MIDI. Calibration must consume that shared event, never `LaneHit`, because the latter also receives autoplay and has no physical input timestamp.

## Player experience

Selecting **Calibrate** starts a self-contained 120 BPM sequence: a short lead-in, then 16 evenly spaced audible clicks with a simultaneous visual beat pulse. The active chart, its BPM, and its timing lines do not influence this sequence. The player can use any mapped keyboard key or MIDI pad and taps to the clicks. Esc cancels at every stage.

After enough accepted taps, the overlay reports proposed input offset (the robust median signed error), accepted and rejected sample counts, median absolute deviation (spread), scheduler/frame observation spread, and a high/low confidence label with its reason.

Enter applies only a high-confidence proposal to the existing manual input offset. Low-confidence evidence is advisory: Enter keeps the current value and the overlay says to retry or use the existing arrow-key manual adjustment. The BGM adjustment remains untouched and is described as chart-audio alignment, not controller latency. No calibration result is persisted until the normal Customize close/save path runs.

## Timing model and limits

The click schedule and raw physical taps use `std::time::Instant`. A click is due every 500 ms; when an Update frame fires it, we record `actual - scheduled` as a scheduler observation. The visual pulse is driven by the same schedule. This gives one known synthetic sequence rather than a selected-song estimate.

The test estimates the player-visible, end-to-end relationship of the emitted audio click to input capture. It cannot independently identify audio-device buffer latency, display latency, controller latency, or human anticipation without a hardware loopback. The UI must state that scope plainly. High scheduler/frame jitter lowers confidence rather than silently changing the offset.

## Statistical policy

The pure `CalibrationReport::from_errors` receives signed millisecond errors. It first calculates their median, then rejects samples farther than 100 ms from that median. It calculates the proposal and spread from the accepted values. Confidence is high only when at least 12 values are accepted, at most one quarter of all values were rejected, accepted MAD is at most 20 ms, and the largest observed scheduler delay is at most 34 ms. Empty or weak evidence is never applicable. These conservative constants are local, named, and covered by unit tests.

The proposal replaces—not adds to—the current `input_offset_ms`, matching the existing judgment equation `(audio_ms - input_offset) - target`.

## Runtime state and recovery

Starting calibration snapshots autoplay, metronome, timing-line visibility, and the current input offset. It temporarily disables autoplay and suppresses chart-driven assumptions; cancellation, completion, performance exit, and a MIDI disconnect all restore that snapshot. MIDI disconnect is non-fatal: collected keyboard taps remain valid and the overlay reports that the player may continue with keyboard or reconnect the pad.

The system observes display update cadence only for confidence; it does not alter visual or BGM offsets. `BgmAdjustState` continues to be changed only by its existing controls.

## Non-goals

- Hardware loopback or automatic audio-device buffer probing.
- Automatic BGM adjustment or a visual-offset persistence field.
- Rebinding controls, modifying chart data, or changing normal judgment.
- Calibration during a normal scored run.
