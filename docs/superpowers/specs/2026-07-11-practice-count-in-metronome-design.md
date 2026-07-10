# Practice count-in metronome — design

Date: 2026-07-11
Status: approved (brainstorm)
Scope: pre-roll only. No through-loop click (future extension).

## Problem

Practice pre-roll (`PrerollSetting::OneBar` / `Seconds`) rewinds silently:
the drummer gets ready-time but no tempo reference and no cue for when the
attempt actually starts. A count-in click plus a visual countdown fixes the
"silent rewind" feel and primes tempo before every attempt/loop lap.

## Decisions

- **Scope:** click + countdown during the pre-roll window only.
- **Click sound:** synthesized in code at startup (no asset files).
- **Visual:** beat numbers ("4 3 2 1") at the quick-tier loop strip.
- **Trigger architecture:** beat schedule computed at seek time, consumed by
  a clock-crossing system (approach A; kira-clock scheduling and per-frame
  beat derivation rejected — more plumbing / awkward countdown for no
  audible gain over ~one-frame jitter).

## 1. Engine: schedule + synth

New `crates/gameplay-drums/src/practice/metronome.rs`, engine layer, pure:

```rust
pub struct Click {
    pub at_ms: i64,
    pub accent: bool,
    pub beats_remaining: u8,
}

pub struct ClickSchedule {
    pub clicks: Vec<Click>,
}

pub fn build_preroll_schedule(
    timeline: &ChipTimeline,
    preroll: PrerollSetting,
    intent_ms: i64,
) -> ClickSchedule
```

Rules:

- Clicks land on timeline beat-grid lines in `[preroll_target, intent_ms)`.
  No click at `intent_ms` itself — the music entry is the implicit "1".
- `beats_remaining` counts down toward the intent point; it is the number
  the UI displays.
- First click of the schedule is accented.
- `PrerollSetting::OneBar` in 4/4 yields 4 clicks. `Seconds(s)` yields
  whatever beat lines fall inside the window (the first may be a partial
  beat — acceptable). `Off` yields an empty schedule.
- BPM changes come free: the beat grid is derived from the timeline.
- Tempo/rate scaling comes free: the song clock already runs at
  `effective_tempo()`; schedule times are chart-time ms like everything
  else.

Click synthesis (startup, once):

- Two short sine bursts (~30 ms, exponential decay): accent ≈ 2 kHz,
  regular ≈ 1 kHz.
- Raw frames → `StaticSoundData` → added programmatically to
  `Assets<AudioSource>` (bevy_kira_audio). No files shipped.

## 2. Systems flow

```
practice seek (restart, click-seek, loop wrap, Set A, ...)
        │  preroll_target() already computes the seek point
        ▼
rebuild ClickSchedule resource (same site where seek intent is known)
        ▼
Update system: while next click.at_ms <= clock_ms:
    play click sample; set CountdownDisplay { number, shown_at }
        ▼
pause  → clock frozen → no clicks fire
re-seek → schedule rebuilt → no stale clicks
```

- Loop wrap goes through the seek engine, so the schedule rebuilds every
  lap automatically.
- Metronome toggle off → rebuild skipped (schedule left empty).

## 3. UI: countdown at loop strip

- Beats-remaining number rendered next to the quick-tier loop strip /
  status chip. One-beat lifetime, quick fade per click.
- Driven entirely by `CountdownDisplay` state written by the click system;
  the UI holds no timing logic.
- Accent click rendered slightly larger/highlighted.
- Nothing shown when preroll is `Off` or the metronome is toggled off.

## 4. Settings

- Single toggle `metronome: bool` on the practice transport, default on.
- Rail row in the TRANSPORT group: "Count-in: on/off".
- Persistence follows whatever the sibling transport settings do today
  (session-only if they are session-only).

## 5. Testing

Unit tests on `build_preroll_schedule`:

- 4/4 `OneBar` → 4 clicks, first accented, `beats_remaining` 4→1.
- Bar containing a BPM change → click times follow the grid.
- `Seconds(2.0)` → partial-beat window handled.
- `Off` → empty schedule.
- Intent at chart start → window clamped at 0 ms.
- No click at `intent_ms` itself.

Synth: deterministic buffer length / non-silence test.

Schedule-build guard: run the existing FixedUpdate ordering guard test —
green unit tests alone do not prove the real plugin schedule builds.

## Out of scope

- Click through the loop body (future toggle; schedule fn generalizes).
- Metronome outside practice mode.
- User-configurable click pitch/volume.
