# Practice Count-In Metronome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Synthesized count-in click + on-screen beat countdown during the practice pre-roll window.

**Architecture:** A pure schedule function (`build_preroll_schedule`) computes click times from `ChipTimeline.beat_ms` on every practice seek; a FixedUpdate system fires clicks as the gameplay clock crosses them and drives a countdown display near the quick-tier mini strip. Click samples are sine bursts synthesized at startup into `Assets<bevy_kira_audio::AudioSource>` — no asset files.

**Tech Stack:** Bevy 0.19, bevy_kira_audio 0.26 (kira 0.12.1 re-exported via `bevy_kira_audio::prelude::{StaticSoundData, StaticSoundSettings, Frame}`).

**Spec:** `docs/superpowers/specs/2026-07-11-practice-count-in-metronome-design.md`

**Build notes (repo conventions):**
- Never run bare `cargo fmt --all` (formatter version drift). Format only files you touched: `rustfmt --edition 2021 <files>` if needed.
- Test command: `cargo test -p gameplay-drums`.
- After wiring systems, ALWAYS run the schedule guard: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering` and `cargo test -p gameplay-drums --test practice_hud real_hud_plugin_schedule_builds_headlessly`. Green unit tests do NOT prove the real FixedUpdate schedule builds.

---

## File Structure

- Create: `crates/gameplay-drums/src/practice/metronome.rs` — pure schedule + synth + systems + plugin
- Modify: `crates/gameplay-drums/src/practice/session.rs` — `metronome: bool` on `PracticeTransport`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` — register `metronome::plugin`
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` — `RailItem::Metronome` row
- Countdown UI lives in `metronome.rs` (small; splitting it out would be a one-system file)

---

### Task 1: Pure click schedule

**Files:**
- Create: `crates/gameplay-drums/src/practice/metronome.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (add `pub mod metronome;`)

- [ ] **Step 1: Write the failing tests**

Create `crates/gameplay-drums/src/practice/metronome.rs`:

```rust
//! Count-in metronome: click schedule computed at seek time, fired as
//! the clock crosses each beat, plus the quick-tier countdown number.
//! Spec: docs/superpowers/specs/2026-07-11-practice-count-in-metronome-design.md

use super::session::{preroll_target, PrerollSetting};
use crate::timeline::ChipTimeline;

/// One scheduled count-in click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    /// Chart time the click fires at (a `ChipTimeline.beat_ms` line).
    pub at_ms: i64,
    /// First click of the schedule is accented.
    pub accent: bool,
    /// Number shown by the countdown UI ("4 3 2 1"): clicks left
    /// including this one.
    pub beats_remaining: u8,
}

/// Click times for one pre-roll window, sorted ascending.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClickSchedule {
    pub clicks: Vec<Click>,
}

/// Clicks on beat-grid lines in `[preroll_target, intent_ms)`. No click
/// at `intent_ms` itself — the music entry is the implicit "1".
pub fn build_preroll_schedule(
    timeline: &ChipTimeline,
    preroll: PrerollSetting,
    intent_ms: i64,
) -> ClickSchedule {
    let window_start = preroll_target(timeline, preroll, intent_ms);
    if window_start >= intent_ms {
        return ClickSchedule::default();
    }
    let beats: Vec<i64> = timeline
        .beat_ms
        .iter()
        .copied()
        .filter(|&ms| ms >= window_start && ms < intent_ms)
        .collect();
    let n = beats.len();
    ClickSchedule {
        clicks: beats
            .into_iter()
            .enumerate()
            .map(|(i, at_ms)| Click {
                at_ms,
                accent: i == 0,
                beats_remaining: (n - i) as u8,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms, beat = 500ms. Chart spans 4 bars.
    fn timeline() -> ChipTimeline {
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(3, EChannel::Snare, 0.75), // keeps 4 bars of lines
            ],
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 8_000)
    }

    #[test]
    fn one_bar_preroll_yields_four_clicks_counting_down() {
        let tl = timeline();
        // Intent at bar 2 start (4000ms): window = [2000, 4000).
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![2_000, 2_500, 3_000, 3_500]);
        let remaining: Vec<u8> = s.clicks.iter().map(|c| c.beats_remaining).collect();
        assert_eq!(remaining, vec![4, 3, 2, 1]);
    }

    #[test]
    fn only_first_click_is_accented() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks[0].accent);
        assert!(s.clicks[1..].iter().all(|c| !c.accent));
    }

    #[test]
    fn no_click_at_intent_itself() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks.iter().all(|c| c.at_ms < 4_000));
    }

    #[test]
    fn seconds_preroll_takes_beats_inside_window() {
        let tl = timeline();
        // 1.2s window before 4000ms: [2800, 4000) → beats 3000, 3500.
        let s = build_preroll_schedule(&tl, PrerollSetting::Seconds(1.2), 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![3_000, 3_500]);
        assert_eq!(s.clicks[0].beats_remaining, 2);
    }

    #[test]
    fn off_preroll_yields_empty_schedule() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::Off, 4_000);
        assert!(s.clicks.is_empty());
    }

    #[test]
    fn intent_at_chart_start_yields_empty_schedule() {
        let tl = timeline();
        // preroll_target clamps to 0 == intent → empty window.
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 0);
        assert!(s.clicks.is_empty());
    }
}
```

Add to `crates/gameplay-drums/src/practice/mod.rs` after `pub mod hud;`:

```rust
pub mod metronome;
```

- [ ] **Step 2: Run tests to verify they pass** (pure fn implemented with the tests in one step — file is new, so the interesting failure mode is compile errors)

Run: `cargo test -p gameplay-drums metronome`
Expected: 6 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/metronome.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(practice): pure count-in click schedule"
```

---

### Task 2: Metronome toggle on the transport

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs:150-172` (`PracticeTransport` + `Default`)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `session.rs`:

```rust
#[test]
fn metronome_defaults_on() {
    let s = PracticeSession::default();
    assert!(s.transport.metronome);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums metronome_defaults_on`
Expected: FAIL — no field `metronome`.

- [ ] **Step 3: Add the field**

In `PracticeTransport` (session.rs), after `pub preroll: PrerollSetting,`:

```rust
    /// Count-in click during pre-roll (spec: count-in metronome).
    pub metronome: bool,
```

In `impl Default for PracticeTransport`, after `preroll: PrerollSetting::OneBar,`:

```rust
            metronome: true,
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums session`
Expected: PASS (including new test).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/session.rs
git commit -m "feat(practice): metronome toggle on transport (default on)"
```

---

### Task 3: Synthesized click sounds

**Files:**
- Modify: `crates/gameplay-drums/src/practice/metronome.rs`

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `metronome.rs`:

```rust
    #[test]
    fn synth_click_is_short_and_non_silent() {
        let frames = synth_click_frames(2_000.0, 44_100);
        // ~30ms at 44.1kHz ≈ 1323 frames.
        assert!((1_300..=1_350).contains(&frames.len()));
        assert!(frames.iter().any(|f| f.left.abs() > 0.05));
        // Exponential decay: tail quieter than head.
        assert!(frames[frames.len() - 1].left.abs() < frames[10].left.abs());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums synth_click`
Expected: FAIL — `synth_click_frames` not found.

- [ ] **Step 3: Implement synth + startup system**

Add to `metronome.rs` (top-level, after `build_preroll_schedule`):

```rust
use bevy::prelude::*;
use bevy_kira_audio::prelude::{Frame, StaticSoundData, StaticSoundSettings};
use bevy_kira_audio::AudioSource as KiraAudioSource;

const CLICK_SAMPLE_RATE: u32 = 44_100;
const CLICK_LEN_S: f32 = 0.03;
const ACCENT_HZ: f32 = 2_000.0;
const TICK_HZ: f32 = 1_000.0;

/// ~30ms sine burst with exponential decay (pure; unit-tested).
pub fn synth_click_frames(freq_hz: f32, sample_rate: u32) -> Vec<Frame> {
    let n = (CLICK_LEN_S * sample_rate as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let env = (-t * 150.0).exp();
            Frame::from_mono((t * freq_hz * std::f32::consts::TAU).sin() * env * 0.8)
        })
        .collect()
}

fn click_source(freq_hz: f32) -> KiraAudioSource {
    KiraAudioSource {
        sound: StaticSoundData {
            sample_rate: CLICK_SAMPLE_RATE,
            frames: synth_click_frames(freq_hz, CLICK_SAMPLE_RATE).into(),
            settings: StaticSoundSettings::default(),
            slice: None,
        },
    }
}

/// Handles to the two synthesized click samples.
#[derive(Resource, Default)]
pub struct MetronomeSounds {
    pub accent: Handle<KiraAudioSource>,
    pub tick: Handle<KiraAudioSource>,
}

/// Build the click samples once per Performance enter (practice only —
/// the plugin gates on `PracticeSession`).
pub fn build_metronome_sounds(
    mut sounds: ResMut<MetronomeSounds>,
    mut sources: ResMut<Assets<KiraAudioSource>>,
) {
    sounds.accent = sources.add(click_source(ACCENT_HZ));
    sounds.tick = sources.add(click_source(TICK_HZ));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums metronome`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/metronome.rs
git commit -m "feat(practice): synthesize metronome click samples"
```

---

### Task 4: Schedule rebuild + click firing systems

**Files:**
- Modify: `crates/gameplay-drums/src/practice/metronome.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (register plugin)

- [ ] **Step 1: Write the failing test** (pure cursor logic; the systems are thin wrappers)

Append to the `tests` module:

```rust
    #[test]
    fn due_clicks_advance_cursor_and_stop_at_clock() {
        let s = ClickSchedule {
            clicks: vec![
                Click { at_ms: 2_000, accent: true, beats_remaining: 2 },
                Click { at_ms: 2_500, accent: false, beats_remaining: 1 },
            ],
        };
        let mut cursor = 0usize;
        let first: Vec<Click> = due_clicks(&s, &mut cursor, 2_010).collect();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].at_ms, 2_000);
        assert_eq!(cursor, 1);
        // Nothing new until the clock reaches the next click.
        assert_eq!(due_clicks(&s, &mut cursor, 2_499).count(), 0);
        assert_eq!(due_clicks(&s, &mut cursor, 2_500).count(), 1);
        assert_eq!(cursor, 2);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums due_clicks`
Expected: FAIL — `due_clicks` not found.

- [ ] **Step 3: Implement resources + systems + plugin**

Add to `metronome.rs`:

```rust
use crate::resources::{DrumAudioSettings, GameplayClock};
use crate::seek::SeekToChartTime;
use bevy_kira_audio::prelude::Audio;
use game_shell::{AppState, PauseState};

use super::session::PracticeSession;

/// The schedule for the current pre-roll, plus how far it has fired.
#[derive(Resource, Debug, Default)]
pub struct ActiveClickSchedule {
    pub schedule: ClickSchedule,
    pub cursor: usize,
}

/// What the countdown UI shows right now.
#[derive(Resource, Debug, Default)]
pub struct CountdownDisplay {
    /// `Some((beats_remaining, accent, wall_seconds_shown_at))`.
    pub current: Option<(u8, bool, f64)>,
}

/// Clicks whose time has come at `clock_ms`, advancing `cursor` past them.
pub fn due_clicks<'a>(
    schedule: &'a ClickSchedule,
    cursor: &'a mut usize,
    clock_ms: i64,
) -> impl Iterator<Item = Click> + 'a {
    std::iter::from_fn(move || {
        let click = schedule.clicks.get(*cursor)?;
        if click.at_ms <= clock_ms {
            *cursor += 1;
            Some(*click)
        } else {
            None
        }
    })
}

/// Rebuild the schedule whenever a practice seek lands. Runs after
/// `apply_seek_system` so it sees the same coalesced last-seek.
pub fn rebuild_click_schedule(
    mut seeks: MessageReader<SeekToChartTime>,
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    mut active: ResMut<ActiveClickSchedule>,
    mut display: ResMut<CountdownDisplay>,
) {
    let Some(seek) = seeks.read().last().copied() else {
        return;
    };
    display.current = None;
    if !session.transport.metronome {
        *active = ActiveClickSchedule::default();
        return;
    }
    let intent = seek.attempt_start_ms.unwrap_or(seek.target_ms);
    active.schedule = build_preroll_schedule(&timeline, session.transport.preroll, intent);
    active.cursor = 0;
}

/// Fire due clicks: play the sample, update the countdown display.
pub fn fire_clicks(
    clock: Res<GameplayClock>,
    sounds: Res<MetronomeSounds>,
    audio: Res<Audio>,
    settings: Res<DrumAudioSettings>,
    time: Res<Time>,
    mut active: ResMut<ActiveClickSchedule>,
    mut display: ResMut<CountdownDisplay>,
) {
    if !clock.is_ready() {
        return;
    }
    let ActiveClickSchedule { schedule, cursor } = &mut *active;
    let mut last = None;
    for click in due_clicks(schedule, cursor, clock.current_ms) {
        let source = if click.accent {
            sounds.accent.clone()
        } else {
            sounds.tick.clone()
        };
        dtx_audio::play_sfx_handle(&audio, source, 100, 0, settings.master_volume, 1.0);
        last = Some(click);
    }
    if let Some(click) = last {
        display.current = Some((click.beats_remaining, click.accent, time.elapsed_secs_f64()));
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<MetronomeSounds>()
        .init_resource::<ActiveClickSchedule>()
        .init_resource::<CountdownDisplay>()
        .add_systems(
            OnEnter(AppState::Performance),
            build_metronome_sounds.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            FixedUpdate,
            (
                rebuild_click_schedule.after(crate::seek::apply_seek_system),
                fire_clicks.after(crate::DrumsSets::ClockSync),
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        );
}
```

Note: `crate::DrumsSets` — check the actual visibility of `DrumsSets` in `crates/gameplay-drums/src/lib.rs` (it is defined there; if it is private, make it `pub(crate)`).

Register in `practice/mod.rs`: in the `.add_plugins((...))` list at the bottom of `plugin`, add `metronome::plugin,` after `hud::plugin,`.

- [ ] **Step 4: Run unit tests + schedule guards**

Run: `cargo test -p gameplay-drums metronome`
Expected: 8 passed.
Run: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering && cargo test -p gameplay-drums --test practice_hud`
Expected: PASS — the real plugin schedule still builds.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/metronome.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(practice): fire count-in clicks from the seek-built schedule"
```

---

### Task 5: Countdown UI at the mini strip

**Files:**
- Modify: `crates/gameplay-drums/src/practice/metronome.rs`

- [ ] **Step 1: Implement spawn/update/despawn (UI-only; covered by the headless schedule guard, not unit tests)**

Add to `metronome.rs`:

```rust
use dtx_ui::theme::Theme;

#[derive(Component)]
pub struct CountdownText;

const COUNTDOWN_FADE_S: f64 = 0.4;

pub fn spawn_countdown(mut commands: Commands) {
    let theme = Theme::default();
    commands.spawn((
        CountdownText,
        Text::new(""),
        Theme::title_font(),
        TextColor(theme.text_primary),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Percent(50.0),
            ..default()
        },
        GlobalZIndex(crate::ui_z::PRACTICE),
        Visibility::Hidden,
    ));
}

pub fn despawn_countdown(mut commands: Commands, q: Query<Entity, With<CountdownText>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn update_countdown(
    display: Res<CountdownDisplay>,
    time: Res<Time>,
    mut q: Query<(&mut Text, &mut TextColor, &mut Visibility), With<CountdownText>>,
) {
    let Ok((mut text, mut color, mut vis)) = q.single_mut() else {
        return;
    };
    match display.current {
        Some((n, accent, shown_at)) => {
            let age = time.elapsed_secs_f64() - shown_at;
            if age > COUNTDOWN_FADE_S {
                *vis = Visibility::Hidden;
                return;
            }
            text.0 = n.to_string();
            let theme = Theme::default();
            let base = if accent { theme.accent } else { theme.text_primary };
            color.0 = base.with_alpha(1.0 - (age / COUNTDOWN_FADE_S) as f32);
            *vis = Visibility::Visible;
        }
        None => *vis = Visibility::Hidden,
    }
}
```

Extend `plugin` in `metronome.rs` (same fn, add these):

```rust
        .add_systems(
            OnEnter(AppState::Performance),
            spawn_countdown.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(AppState::Performance), despawn_countdown)
        .add_systems(
            Update,
            update_countdown
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        )
```

Follow the existing pattern in `practice/hud/mini_strip.rs` if signatures drift.

- [ ] **Step 2: Run schedule guards**

Run: `cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --test fixed_update_schedule_ordering`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/src/practice/metronome.rs
git commit -m "feat(practice): count-in countdown number at mini strip"
```

---

### Task 6: Rail row (Count-in on/off)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs:38-147` (enum, ORDER, label), `~:277-283` (header indices), `~:506-579` (Enter handler)

- [ ] **Step 1: Write the failing test**

Append to `full_hud.rs` tests (find the existing `#[cfg(test)] mod tests`; create one if the file has none — it may live in `tests/practice_hud.rs` instead; put it wherever `rail_label` is already tested, otherwise add here):

```rust
    #[test]
    fn metronome_rail_label_reflects_toggle() {
        let mut s = crate::practice::session::PracticeSession::default();
        assert_eq!(rail_label(RailItem::Metronome, &s, false), "Count-in  on");
        s.transport.metronome = false;
        assert_eq!(rail_label(RailItem::Metronome, &s, false), "Count-in  off");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums metronome_rail_label`
Expected: FAIL — no variant `Metronome`.

- [ ] **Step 3: Implement**

1. Add `Metronome,` to `enum RailItem` after `Preroll,`.
2. In `RailItem::ORDER`, insert `RailItem::Metronome,` after `RailItem::Preroll,` and bump the array length `[RailItem; 16]` → `[RailItem; 17]`.
3. **Header indices shift**: in the spawn loop (`full_hud.rs` around line 278), `Metronome` lands at index 6, so update the match: `0 => Some("TRANSPORT"), 7 => Some("LOOP"), 10 => Some("TRAINER")`.
4. `rail_label`: add

```rust
        RailItem::Metronome => format!(
            "Count-in  {}",
            if session.transport.metronome { "on" } else { "off" }
        ),
```

5. Enter handler (the `match selected` under `keys.just_pressed(KeyCode::Enter)`): add

```rust
            RailItem::Metronome => {
                session.transport.metronome = !session.transport.metronome;
            }
```

6. Left/right handler: nothing (toggle is Enter-only, like `RampArm`). If the compiler demands exhaustiveness in the arrow-key `match selected`, it falls into the existing `_ => {}` arm.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: full crate green (322+ tests, plus new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud/full_hud.rs
git commit -m "feat(practice): count-in rail toggle"
```

---

### Task 7: Verification

- [ ] **Step 1: Full suite + guards**

Run: `cargo test -p gameplay-drums`
Expected: all green.

- [ ] **Step 2: Manual check (if a display is available)**

Launch the game, enter practice (SHIFT+ENTER on song select), set a loop, let it wrap: pre-roll should tick 4-3-2-1 with a higher-pitched first click and a fading number above the mini strip. Toggle "Count-in off" in the full HUD rail (Tab) and confirm silence. `PrerollSetting::Off` must also produce no clicks.

- [ ] **Step 3: Final commit if any fixups**
