# Practice UX v2 — Two-Tier HUD + Accuracy Ramp Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the pause-panel practice UX with a two-tier HUD (live quick tier + paused full HUD with mouse timeline) routed through a `PracticeAction` layer, and add the accuracy-gated rate ramp.

**Architecture:** All new code lives in `crates/gameplay-drums/src/practice/` (spec: `docs/superpowers/specs/2026-07-07-practice-ux-v2-design.md`). Keyboard input is translated by `actions.rs` into a `PracticeAction` Bevy message consumed by thin applier systems; the ramp protocol is a pure function (`ramp_step`) whose decisions a FixedUpdate system applies after `track_attempt_stats`. The full HUD is a fixed overlay (NOT a `dtx-layout` widget) that owns `PauseState::Paused` whenever a `PracticeSession` exists; `pause.rs` keeps suppressing its normal overlay. The engine layer (seek.rs, timeline.rs, ab_loop.rs, rate.rs, stats.rs) is unchanged, and the `SeekToChartTime { target_ms, snap, attempt_start_ms }` shape is FROZEN (editor plan 4 depends on it).

**Tech Stack:** Rust, Bevy 0.19 (`#[derive(Message)]` + `add_message`/`MessageReader`/`MessageWriter`; mouse idiom from `editor/drag.rs`: `Query<&Window>` + `window.cursor_position()` + `ButtonInput<MouseButton>`, node rects via `ComputedNode`+`GlobalTransform`), bevy_kira_audio 0.26 (rate path unchanged in `practice/rate.rs`), `dtx_ui::widget::density_strip`, `dtx_ui::theme::Theme`.

---

## Conventions (verified in codebase)

- Messages: `#[derive(Message)]`, registered with `app.add_message::<T>()` (see `events.rs`, `lib.rs:128-132`).
- Practice systems gate on `run_if(in_state(AppState::Performance))` + `run_if(resource_exists::<PracticeSession>)` (see `practice/mod.rs:27-33`).
- `apply_seek_system` runs in FixedUpdate even while paused (`lib.rs:155-160`); `start_pending_bgm` is Running-gated — paused seeks already defer audio. No seek-engine change needed.
- **rustfmt caution:** never run `cargo fmt --all` (formatter version drift reformats unrelated files). Use `cargo fmt -p gameplay-drums` at most, and only on files you touched.
- No AI co-authors in commits. Commit style: `feat(gameplay-drums): ...`.
- `lib.rs` is NOT modified by this plan (practice plugin already registered at `lib.rs:193`) — keeps the merge surface with the editor branch to `pause.rs` only.

## File structure

Create:
- `crates/gameplay-drums/src/practice/actions.rs` — `PracticeAction` message, `PracticeBindings` resource, keyboard→action emitter, quick-tier applier.
- `crates/gameplay-drums/src/practice/ramp.rs` — pure `ramp_step` protocol + `RampDecision`, `ramp_step_index`, ToggleRamp handler, FixedUpdate applier.
- `crates/gameplay-drums/src/practice/toast.rs` — `ToastQueue` resource (cap 4, 1.5 s life) + top-center toast UI system.
- `crates/gameplay-drums/src/practice/hud/mod.rs` — hud plugin aggregation, `format_chart_time` (moved from ui.rs).
- `crates/gameplay-drums/src/practice/hud/timeline_ui.rs` — pure cursor↔ms math, `drag_region`, `bar_number`, gesture state machine + mouse system.
- `crates/gameplay-drums/src/practice/hud/full_hud.rs` — paused-tier overlay: bottom density timeline, right rail, transport row, keyboard input.
- `crates/gameplay-drums/src/practice/hud/mini_strip.rs` — quick-tier thin loop strip (playhead + A/B fill, no density).
- `crates/gameplay-drums/src/practice/hud/chip.rs` — quick-tier top-right status chip.
- `crates/gameplay-drums/tests/practice_hud.rs` — headless HUD spawn/despawn + pause-suppression + transport-button tests.

Modify:
- `crates/gameplay-drums/src/practice/mod.rs` (lines 8-12 module decls, line 21-34 plugin) — register new modules/systems.
- `crates/gameplay-drums/src/practice/session.rs` (after line 121) — `RampConfig` + `RampState` fields on `PracticeSession`.
- `crates/gameplay-drums/src/practice/ui.rs` — pause panel removed in Task 4 (lines 237-497); file fully deleted in Task 8 (transport → `hud/mini_strip.rs`).
- `crates/gameplay-drums/src/pause.rs` (lines 19-21, 44-45, 109-117) — `PauseOverlay`, `PauseSelection`, `spawn_overlay` made `pub` so the suppression contract is testable from `tests/`.
- `crates/gameplay-drums/tests/practice_mode.rs` — action→seek wiring + ramp end-to-end tests appended.
- `crates/gameplay-drums/tests/fixed_update_schedule_ordering.rs` (lines 33-72) — mirror the new `apply_ramp` FixedUpdate edge.

Delete:
- `crates/gameplay-drums/src/practice/ui.rs` (Task 8, after both HUD tiers exist).

---

### Task 1: `actions.rs` — `PracticeAction` message + bindings

**Files:**
- Create: `crates/gameplay-drums/src/practice/actions.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs:8-12` (module decl), `:21-34` (plugin)
- Test: unit tests inside `actions.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/gameplay-drums/src/practice/actions.rs` with only the test module first:

```rust
//! Input→action indirection for practice mode.
//!
//! Keyboard (v2) is translated into `PracticeAction` messages; MIDI pad
//! combos / foot control later bind here without touching any consumer.

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::KeyCode;

    #[test]
    fn default_bindings_cover_spec_table() {
        let b = PracticeBindings::default();
        assert_eq!(
            action_for(&b, KeyCode::BracketLeft),
            Some(PracticeAction::SetLoopStart)
        );
        assert_eq!(
            action_for(&b, KeyCode::BracketRight),
            Some(PracticeAction::SetLoopEnd)
        );
        assert_eq!(
            action_for(&b, KeyCode::Backspace),
            Some(PracticeAction::ClearLoop)
        );
        assert_eq!(action_for(&b, KeyCode::Minus), Some(PracticeAction::RateDown));
        assert_eq!(action_for(&b, KeyCode::Equal), Some(PracticeAction::RateUp));
        assert_eq!(
            action_for(&b, KeyCode::KeyR),
            Some(PracticeAction::RestartLoop)
        );
        assert_eq!(action_for(&b, KeyCode::KeyT), Some(PracticeAction::ToggleRamp));
        assert_eq!(action_for(&b, KeyCode::Tab), Some(PracticeAction::OpenFullHud));
        assert_eq!(action_for(&b, KeyCode::KeyQ), None);
    }
}
```

Add `pub mod actions;` to the module list in `crates/gameplay-drums/src/practice/mod.rs` (alphabetical, before `ab_loop` is fine as `pub mod actions;` on line 8).

- [ ] **Step 2: Run it (expect FAIL)**

Run: `cargo test -p gameplay-drums --lib practice::actions`
Expected: FAIL to compile — `PracticeBindings`, `action_for`, `PracticeAction` not found.

- [ ] **Step 3: Implement the types + pure lookup + emitter**

Fill in `actions.rs` above the test module:

```rust
use bevy::prelude::*;
use game_shell::PauseState;

use super::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::{ChipTimeline, SnapDivisor};

/// One quick-tier practice action. All hotkeys (and later MIDI combos)
/// route through this so consumers never read raw input.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeAction {
    SetLoopStart,
    SetLoopEnd,
    ClearLoop,
    RateDown,
    RateUp,
    RestartLoop,
    ToggleRamp,
    OpenFullHud,
}

/// Key→action table. A resource so a future bindings UI / MIDI layer can
/// replace it wholesale.
#[derive(Resource, Debug, Clone)]
pub struct PracticeBindings(pub Vec<(KeyCode, PracticeAction)>);

impl Default for PracticeBindings {
    fn default() -> Self {
        Self(vec![
            (KeyCode::BracketLeft, PracticeAction::SetLoopStart),
            (KeyCode::BracketRight, PracticeAction::SetLoopEnd),
            (KeyCode::Backspace, PracticeAction::ClearLoop),
            (KeyCode::Minus, PracticeAction::RateDown),
            (KeyCode::Equal, PracticeAction::RateUp),
            (KeyCode::KeyR, PracticeAction::RestartLoop),
            (KeyCode::KeyT, PracticeAction::ToggleRamp),
            (KeyCode::Tab, PracticeAction::OpenFullHud),
        ])
    }
}

/// Pure: the action bound to `key`, if any.
pub fn action_for(bindings: &PracticeBindings, key: KeyCode) -> Option<PracticeAction> {
    bindings
        .0
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, a)| *a)
}

/// Quick tier only (Running): translate just-pressed keys into actions.
pub fn emit_practice_actions(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<PracticeBindings>,
    mut out: MessageWriter<PracticeAction>,
) {
    for key in keys.get_just_pressed() {
        if let Some(action) = action_for(&bindings, *key) {
            out.write(action);
        }
    }
}
```

- [ ] **Step 4: Run it (expect PASS)**

Run: `cargo test -p gameplay-drums --lib practice::actions`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/actions.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(gameplay-drums): PracticeAction message + default bindings"
```

---

### Task 2: `apply_practice_actions` — action semantics + wiring

**Files:**
- Modify: `crates/gameplay-drums/src/practice/actions.rs` (append system), `crates/gameplay-drums/src/practice/mod.rs:21-34` (plugin wiring)
- Test: `crates/gameplay-drums/tests/practice_mode.rs` (append)

- [ ] **Step 1: Write the failing integration tests**

Append to `crates/gameplay-drums/tests/practice_mode.rs`:

```rust
use gameplay_drums::practice::actions::{
    apply_practice_actions, emit_practice_actions, PracticeAction, PracticeBindings,
};

fn add_action_wiring(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<PracticeBindings>()
        .init_state::<game_shell::PauseState>()
        .add_message::<PracticeAction>()
        .add_systems(
            Update,
            (emit_practice_actions, apply_practice_actions)
                .chain()
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(resource_exists::<PracticeSession>),
        );
}

#[test]
fn bracket_key_sets_loop_start_snapped_to_bar() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(4_700));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::BracketLeft);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    let region = session.loop_region.expect("A marker set");
    assert_eq!(region.start_ms, 4_000, "A snaps down to the bar start");
}

#[test]
fn restart_key_seeks_to_loop_start() {
    let mut app = build_app();
    add_action_wiring(&mut app);
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
        clock.sync(Some(5_000));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(
        app.world().resource::<GameplayClock>().current_ms,
        2_000,
        "R restarts the loop at A"
    );
}
```

- [ ] **Step 2: Run them (expect FAIL)**

Run: `cargo test -p gameplay-drums --test practice_mode -- bracket_key restart_key`
Expected: FAIL to compile — `apply_practice_actions` not found.

- [ ] **Step 3: Implement the applier**

Append to `actions.rs` (below `emit_practice_actions`):

```rust
/// Apply quick-tier actions. `ToggleRamp` is intentionally not handled
/// here: `ramp::handle_toggle_ramp` consumes the same message stream
/// with its own `MessageReader` (multiple readers are independent).
pub fn apply_practice_actions(
    mut actions: MessageReader<PracticeAction>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    for action in actions.read() {
        match action {
            PracticeAction::SetLoopStart => {
                let ms = timeline.bar_start_before(clock.current_ms);
                session.set_loop_start(ms);
            }
            PracticeAction::SetLoopEnd => {
                let mut ms = timeline.bar_start_before(clock.current_ms);
                // Min region: one bar. B on/before A pushes one bar past A.
                if let Some(r) = session.loop_region {
                    if ms <= r.start_ms {
                        ms = timeline.snap_neighbor(r.start_ms, SnapDivisor::Bar, 1);
                    }
                }
                session.set_loop_end(ms);
            }
            PracticeAction::ClearLoop => session.loop_region = None,
            PracticeAction::RateDown => session.step_rate(-1),
            PracticeAction::RateUp => session.step_rate(1),
            PracticeAction::RestartLoop => {
                let intent = session
                    .loop_region
                    .map(|r| r.start_ms)
                    .unwrap_or(session.current_attempt.start_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
            }
            PracticeAction::OpenFullHud => next_pause.set(PauseState::Paused),
            PracticeAction::ToggleRamp => {}
        }
    }
}
```

Wire into `crates/gameplay-drums/src/practice/mod.rs` — inside `plugin`, before the `.add_plugins(...)` line:

```rust
    app.init_resource::<actions::PracticeBindings>()
        .add_message::<actions::PracticeAction>()
        .add_systems(
            Update,
            (actions::emit_practice_actions, actions::apply_practice_actions)
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        );
```

- [ ] **Step 4: Run them (expect PASS)**

Run: `cargo test -p gameplay-drums --test practice_mode`
Expected: all pass (old tests untouched, 2 new pass).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/actions.rs crates/gameplay-drums/src/practice/mod.rs crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(gameplay-drums): quick-tier practice hotkeys via PracticeAction"
```

---

### Task 3: `hud/timeline_ui.rs` — pure cursor↔ms math (TDD)

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/mod.rs`, `crates/gameplay-drums/src/practice/hud/timeline_ui.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs:8-12` (add `pub mod hud;`)
- Test: unit tests inside `timeline_ui.rs`

- [ ] **Step 1: Scaffold the hud module and write failing tests**

Create `crates/gameplay-drums/src/practice/hud/mod.rs`:

```rust
//! Two-tier practice HUD: quick tier (mini strip, chip, toasts) during
//! play, full HUD (timeline + right rail) while paused. Fixed overlay —
//! deliberately NOT a dtx-layout widget (no editor-pillar dependency).

pub mod timeline_ui;

use bevy::prelude::*;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

pub(super) fn plugin(_app: &mut App) {}
```

Add `pub mod hud;` to `practice/mod.rs` and `.add_plugins(hud::plugin)` won't be needed yet (empty plugin registered in Task 4).

Create `crates/gameplay-drums/src/practice/hud/timeline_ui.rs`:

```rust
//! Pure math for the full-HUD timeline: cursor x ↔ chart ms, drag→loop
//! region snapping, bar numbering, and the press/drag gesture machine.

use bevy::prelude::*;

use crate::practice::session::LoopRegion;
use crate::timeline::{ChipTimeline, SnapDivisor};

/// Cursor movement below this (logical px) between press and release is a
/// click (seek); above it the press becomes a loop drag.
pub const CLICK_SLOP_PX: f32 = 4.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms. 8 bars → end 16000ms.
    fn timeline() -> ChipTimeline {
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: (0..8)
                .map(|i| Chip::new(i, EChannel::BassDrum, 0.0))
                .collect(),
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 16_000)
    }

    #[test]
    fn cursor_to_ms_maps_strip_extent() {
        assert_eq!(cursor_to_ms(100.0, 100.0, 400.0, 16_000), 0);
        assert_eq!(cursor_to_ms(500.0, 100.0, 400.0, 16_000), 16_000);
        assert_eq!(cursor_to_ms(300.0, 100.0, 400.0, 16_000), 8_000);
        // Clamped outside the strip.
        assert_eq!(cursor_to_ms(0.0, 100.0, 400.0, 16_000), 0);
        assert_eq!(cursor_to_ms(900.0, 100.0, 400.0, 16_000), 16_000);
        // Degenerate inputs.
        assert_eq!(cursor_to_ms(300.0, 100.0, 0.0, 16_000), 0);
        assert_eq!(cursor_to_ms(300.0, 100.0, 400.0, 0), 0);
    }

    #[test]
    fn cursor_to_ms_round_trips_time_to_pct() {
        // dtx_ui::density_strip::time_to_pct is the inverse mapping used
        // to place markers; verify both directions agree.
        let ms = 6_000;
        let pct = dtx_ui::widget::density_strip::time_to_pct(ms, 16_000);
        let x = 100.0 + pct / 100.0 * 400.0;
        assert_eq!(cursor_to_ms(x, 100.0, 400.0, 16_000), ms);
    }

    #[test]
    fn drag_region_snaps_to_bars_and_orders_endpoints() {
        let tl = timeline();
        // Drag right→left across bars 2..4 (ms 4700 → 2100).
        let r = drag_region(&tl, 4_700, 2_100);
        assert_eq!(r.start_ms, 2_000);
        assert_eq!(r.end_ms, 4_000);
    }

    #[test]
    fn drag_region_shorter_than_one_bar_snaps_up() {
        let tl = timeline();
        let r = drag_region(&tl, 2_100, 2_300);
        assert_eq!(r.start_ms, 2_000);
        assert_eq!(r.end_ms, 4_000, "min region is one bar");
    }

    #[test]
    fn bar_number_is_one_based() {
        let tl = timeline();
        assert_eq!(bar_number(&tl.bar_ms, 0), 1);
        assert_eq!(bar_number(&tl.bar_ms, 1_999), 1);
        assert_eq!(bar_number(&tl.bar_ms, 2_000), 2);
        assert_eq!(bar_number(&tl.bar_ms, 5_000), 3);
    }

    #[test]
    fn gesture_click_seeks_drag_loops() {
        let idle = TimelineGesture::Idle;
        let press = GestureInput {
            just_pressed: true,
            pressed: true,
            inside_strip: true,
            cursor_x: 200.0,
            cursor_ms: 4_000,
        };
        let (g, fx) = advance_gesture(idle, press);
        assert_eq!(
            g,
            TimelineGesture::Pending {
                press_x: 200.0,
                press_ms: 4_000
            }
        );
        assert_eq!(fx, GestureEffect::None);

        // Release without movement → click seek.
        let release = GestureInput {
            just_pressed: false,
            pressed: false,
            inside_strip: true,
            cursor_x: 201.0,
            cursor_ms: 4_050,
        };
        let (g2, fx2) = advance_gesture(g, release);
        assert_eq!(g2, TimelineGesture::Idle);
        assert_eq!(fx2, GestureEffect::Seek { target_ms: 4_000 });

        // Move past the slop while held → loop drag.
        let drag = GestureInput {
            just_pressed: false,
            pressed: true,
            inside_strip: true,
            cursor_x: 240.0,
            cursor_ms: 6_500,
        };
        let (g3, fx3) = advance_gesture(g, drag);
        assert_eq!(g3, TimelineGesture::DragLoop { anchor_ms: 4_000 });
        assert_eq!(fx3, GestureEffect::LoopPreview { anchor_ms: 4_000 });

        // Release ends the drag (region was committed live).
        let (g4, fx4) = advance_gesture(g3, release);
        assert_eq!(g4, TimelineGesture::Idle);
        assert_eq!(fx4, GestureEffect::None);
    }

    #[test]
    fn gesture_press_outside_strip_is_ignored() {
        let press = GestureInput {
            just_pressed: true,
            pressed: true,
            inside_strip: false,
            cursor_x: 5.0,
            cursor_ms: 0,
        };
        let (g, fx) = advance_gesture(TimelineGesture::Idle, press);
        assert_eq!(g, TimelineGesture::Idle);
        assert_eq!(fx, GestureEffect::None);
    }
}
```

- [ ] **Step 2: Run them (expect FAIL)**

Run: `cargo test -p gameplay-drums --lib practice::hud::timeline_ui`
Expected: FAIL to compile — the functions/types don't exist yet.

- [ ] **Step 3: Implement the pure layer**

Add above the test module in `timeline_ui.rs`:

```rust
/// Map a cursor x (logical px) on a strip starting at `strip_min_x` with
/// `strip_width` px to chart ms over `[0, end_ms]`. Clamps to the strip.
pub fn cursor_to_ms(cursor_x: f32, strip_min_x: f32, strip_width: f32, end_ms: i64) -> i64 {
    if strip_width <= 0.0 || end_ms <= 0 {
        return 0;
    }
    let t = ((cursor_x - strip_min_x) / strip_width).clamp(0.0, 1.0) as f64;
    (t * end_ms as f64).round() as i64
}

/// Bar-snapped loop region for a drag between two chart times, in either
/// direction. Regions shorter than one bar snap up to exactly one bar.
pub fn drag_region(timeline: &ChipTimeline, anchor_ms: i64, cursor_ms: i64) -> LoopRegion {
    let (lo, hi) = if cursor_ms < anchor_ms {
        (cursor_ms, anchor_ms)
    } else {
        (anchor_ms, cursor_ms)
    };
    let start_ms = timeline.bar_start_before(lo);
    let mut end_ms = timeline.bar_start_before(hi);
    if end_ms <= start_ms {
        end_ms = timeline.snap_neighbor(start_ms, SnapDivisor::Bar, 1);
    }
    LoopRegion { start_ms, end_ms }
}

/// 1-based bar number containing `ms` (against `ChipTimeline::bar_ms`).
pub fn bar_number(bar_ms: &[i64], ms: i64) -> usize {
    match bar_ms.binary_search(&ms) {
        Ok(i) => i + 1,
        Err(0) => 1,
        Err(i) => i,
    }
}

/// Mouse gesture on the full-HUD timeline strip.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub enum TimelineGesture {
    #[default]
    Idle,
    /// Pressed inside the strip; not yet decided click vs drag.
    Pending { press_x: f32, press_ms: i64 },
    /// Dragging out a loop region anchored at the press point.
    DragLoop { anchor_ms: i64 },
}

/// One frame of mouse state, pre-resolved against the strip rect.
#[derive(Debug, Clone, Copy)]
pub struct GestureInput {
    pub just_pressed: bool,
    pub pressed: bool,
    pub inside_strip: bool,
    pub cursor_x: f32,
    pub cursor_ms: i64,
}

/// What the system should do this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEffect {
    None,
    /// Click released: seek (snap applied by the consumer via the seek
    /// message's `snap` field — SeekToChartTime shape is frozen).
    Seek { target_ms: i64 },
    /// Drag in progress: preview/commit `drag_region(anchor, cursor)`.
    LoopPreview { anchor_ms: i64 },
}

/// Pure gesture step: previous state + frame input → next state + effect.
pub fn advance_gesture(g: TimelineGesture, i: GestureInput) -> (TimelineGesture, GestureEffect) {
    match g {
        TimelineGesture::Idle => {
            if i.just_pressed && i.inside_strip {
                (
                    TimelineGesture::Pending {
                        press_x: i.cursor_x,
                        press_ms: i.cursor_ms,
                    },
                    GestureEffect::None,
                )
            } else {
                (TimelineGesture::Idle, GestureEffect::None)
            }
        }
        TimelineGesture::Pending { press_x, press_ms } => {
            if !i.pressed {
                (TimelineGesture::Idle, GestureEffect::Seek { target_ms: press_ms })
            } else if (i.cursor_x - press_x).abs() > CLICK_SLOP_PX {
                (
                    TimelineGesture::DragLoop { anchor_ms: press_ms },
                    GestureEffect::LoopPreview { anchor_ms: press_ms },
                )
            } else {
                (g, GestureEffect::None)
            }
        }
        TimelineGesture::DragLoop { anchor_ms } => {
            if i.pressed {
                (g, GestureEffect::LoopPreview { anchor_ms })
            } else {
                (TimelineGesture::Idle, GestureEffect::None)
            }
        }
    }
}
```

Add `pub mod timeline_ui;` already present in `hud/mod.rs` from Step 1.

- [ ] **Step 4: Run them (expect PASS)**

Run: `cargo test -p gameplay-drums --lib practice::hud`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(gameplay-drums): timeline cursor math + gesture machine (pure)"
```

---

### Task 4: `hud/full_hud.rs` — paused overlay replaces the pause panel

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- Create: `crates/gameplay-drums/tests/practice_hud.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mod.rs` (plugin body), `crates/gameplay-drums/src/practice/mod.rs:33` (add `hud::plugin`), `crates/gameplay-drums/src/practice/ui.rs` (delete lines 237-497: `PracticePanel` through `practice_panel_input` + its plugin registration lines 58-69; keep transport + `format_chart_time` for now), `crates/gameplay-drums/src/pause.rs:19-21,44-45,109` (make `PauseOverlay`, `PauseSelection`, `spawn_overlay` pub)

- [ ] **Step 1: Write the failing integration tests**

Create `crates/gameplay-drums/tests/practice_hud.rs`:

```rust
//! Headless HUD tests: full HUD spawn/despawn on pause, normal pause
//! overlay suppressed while a practice session exists.

use bevy::prelude::*;
use game_shell::{AppState, PauseState};
use gameplay_drums::practice::hud::full_hud::{
    despawn_full_hud, spawn_full_hud, FullHudRoot, RailSelection,
};
use gameplay_drums::practice::session::PracticeSession;
use gameplay_drums::resources::GameplayClock;
use gameplay_drums::timeline::ChipTimeline;

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_state::<PauseState>()
        .init_resource::<GameplayClock>()
        .init_resource::<ChipTimeline>()
        .init_resource::<RailSelection>()
        .init_resource::<gameplay_drums::practice::hud::full_hud::ExitArmed>()
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_full_hud.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), despawn_full_hud);
    app
}

fn set_paused(app: &mut App, paused: bool) {
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(if paused {
            PauseState::Paused
        } else {
            PauseState::Running
        });
    app.update();
}

#[test]
fn full_hud_spawns_on_pause_and_despawns_on_resume() {
    let mut app = build_app();
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "full HUD present while paused");

    set_paused(&mut app, false);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "full HUD gone after resume");
}

#[test]
fn full_hud_absent_without_practice_session() {
    let mut app = build_app();
    set_paused(&mut app, true);
    let count = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "normal pause never spawns the practice HUD");
}

#[test]
fn normal_pause_overlay_suppressed_in_practice() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .add_systems(OnEnter(PauseState::Paused), gameplay_drums::pause::spawn_overlay);
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(overlays, 0, "practice suppresses the normal pause overlay");
}
```

- [ ] **Step 2: Run them (expect FAIL)**

Run: `cargo test -p gameplay-drums --test practice_hud`
Expected: FAIL to compile — `full_hud` module and pub pause items missing.

- [ ] **Step 3: Make pause.rs testable and complete the suppression move**

In `crates/gameplay-drums/src/pause.rs`:

- Line 19-21: `struct PauseOverlay` → `pub struct PauseOverlay;` and update its doc comment to `/// Root marker for the normal pause overlay (practice suppresses it; the practice full HUD owns PauseState::Paused — see practice/hud/full_hud.rs).`
- Line 44-45: `struct PauseSelection(usize)` → `pub struct PauseSelection(pub usize);`
- Line 109: `fn spawn_overlay(` → `pub fn spawn_overlay(`

The early-return practice gates in `spawn_overlay` (line 114-116) and `pause_menu_input` (line 171-173) already exist and stay.

- [ ] **Step 4: Implement `full_hud.rs`**

Create `crates/gameplay-drums/src/practice/hud/full_hud.rs`:

```rust
//! Full practice HUD (paused tier), layout B "L-shape": bottom density
//! timeline (mouse scrub + drag loop; keyboard scrub kept) + right rail
//! (rate, snap, pre-roll, ramp config, attempt history, restart, exit).
//! Fixed overlay — not a dtx-layout widget.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use super::format_chart_time;
use crate::practice::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// Root marker for the full practice HUD.
#[derive(Component)]
pub struct FullHudRoot;

/// The bottom timeline strip (mouse hit-target; markers are children).
#[derive(Component)]
pub struct FullHudTimelineStrip;

#[derive(Component)]
pub struct HudPlayhead;
#[derive(Component)]
pub struct HudScrubCursor;
#[derive(Component)]
pub struct HudLoopFill;
#[derive(Component)]
pub struct HudTimeText;
#[derive(Component)]
pub struct AttemptHistoryText;

/// One selectable right-rail row.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum RailItem {
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

impl RailItem {
    pub const ORDER: [RailItem; 10] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::ExitPractice,
    ];
}

/// Currently highlighted rail row.
#[derive(Resource, Default)]
pub struct RailSelection(pub usize);

/// Exit needs a second Enter press (confirm); reset on selection move.
#[derive(Resource, Default)]
pub struct ExitArmed(pub bool);

pub fn rail_label(item: RailItem, session: &PracticeSession, exit_armed: bool) -> String {
    match item {
        RailItem::Resume => "Resume".into(),
        RailItem::Scrub => match session.scrub_cursor_ms {
            Some(ms) => format!("Scrub  ◀ {} ▶   (Enter: play here)", format_chart_time(ms)),
            None => "Scrub  ◀ ▶".into(),
        },
        RailItem::RestartSection => "Restart section".into(),
        RailItem::SetA => "Set A here".into(),
        RailItem::SetB => "Set B here".into(),
        RailItem::ClearLoop => "Clear loop".into(),
        RailItem::Rate => format!("Rate  ◀ x{:.2} ▶", session.rate),
        RailItem::Snap => format!("Snap  ◀ {} ▶", session.snap.label()),
        RailItem::Preroll => format!("Pre-roll  ◀ {} ▶", session.preroll.label()),
        RailItem::ExitPractice => {
            if exit_armed {
                "Exit practice — Enter again to confirm".into()
            } else {
                "Exit practice".into()
            }
        }
    }
}

pub fn attempt_history_text(session: &PracticeSession) -> String {
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in session.attempt_history.iter().enumerate().rev().take(8) {
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

pub fn spawn_full_hud(
    mut commands: Commands,
    mut selection: ResMut<RailSelection>,
    mut exit_armed: ResMut<ExitArmed>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
    timeline: Res<ChipTimeline>,
) {
    selection.0 = 0;
    exit_armed.0 = false;
    session.scrub_cursor_ms = Some(clock.current_ms);
    let theme = Theme::default();
    commands
        .spawn((
            FullHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            // Right rail.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(72.0),
                    width: Val::Px(340.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|rail| {
                rail.spawn((
                    Text::new("PRACTICE"),
                    Theme::title_font(),
                    TextColor(theme.text_primary),
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                ));
                for item in RailItem::ORDER {
                    rail.spawn((
                        item,
                        Text::new(rail_label(item, &session, false)),
                        Theme::hud_font(),
                        TextColor(theme.text_secondary),
                    ));
                }
                rail.spawn((
                    AttemptHistoryText,
                    Text::new(attempt_history_text(&session)),
                    Theme::label_font(),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
            });

            // Bottom timeline row: time text + density strip.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    height: Val::Px(72.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    padding: UiRect::horizontal(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|row| {
                row.spawn((
                    HudTimeText,
                    Text::new(format_chart_time(clock.current_ms)),
                    Theme::hud_font(),
                    TextColor(theme.text_primary),
                ));
                let strip = spawn_density_strip(row, &timeline.density, &theme);
                row.commands().entity(strip).insert(FullHudTimelineStrip);
                row.commands().entity(strip).with_children(|markers| {
                    // Bar ticks along the top edge.
                    for &bar in &timeline.bar_ms {
                        markers.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(time_to_pct(bar, timeline.end_ms)),
                                top: Val::Px(0.0),
                                width: Val::Px(1.0),
                                height: Val::Px(8.0),
                                ..default()
                            },
                            BackgroundColor(theme.text_secondary.with_alpha(0.6)),
                        ));
                    }
                    markers.spawn((
                        HudLoopFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Percent(0.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.25)),
                        Visibility::Hidden,
                    ));
                    markers.spawn((
                        HudPlayhead,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(theme.accent),
                    ));
                    markers.spawn((
                        HudScrubCursor,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        Visibility::Hidden,
                    ));
                });
            });
        });
}

pub fn despawn_full_hud(
    mut commands: Commands,
    roots: Query<Entity, With<FullHudRoot>>,
    mut session: Option<ResMut<PracticeSession>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if let Some(session) = session.as_mut() {
        session.scrub_cursor_ms = None;
    }
}

/// Keyboard nav for the rail (port of the v1 pause-panel input; the
/// v1 semantics for each row are unchanged).
#[allow(clippy::too_many_arguments)]
pub fn full_hud_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RailSelection>,
    mut exit_armed: ResMut<ExitArmed>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut requests: MessageWriter<TransitionRequest>,
    mut rows: Query<(&RailItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailItem>)>,
) {
    let count = RailItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
        exit_armed.0 = false;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
        exit_armed.0 = false;
    }
    let selected = RailItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        match selected {
            RailItem::Scrub => {
                let cur = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                session.scrub_cursor_ms = Some(timeline.snap_neighbor(cur, session.snap, dir));
            }
            RailItem::Rate => session.step_rate(dir),
            RailItem::Snap => session.snap = session.snap.next(),
            RailItem::Preroll => session.preroll = session.preroll.next(),
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match selected {
            RailItem::Resume => next_pause.set(PauseState::Running),
            RailItem::Scrub => {
                let intent = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            RailItem::RestartSection => {
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
            RailItem::SetA => {
                let ms =
                    timeline.bar_start_before(session.scrub_cursor_ms.unwrap_or(clock.current_ms));
                session.set_loop_start(ms);
            }
            RailItem::SetB => {
                let cursor = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                let mut ms = timeline.bar_start_before(cursor);
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
            RailItem::ClearLoop => session.loop_region = None,
            RailItem::Rate | RailItem::Snap | RailItem::Preroll => {}
            RailItem::ExitPractice => {
                if exit_armed.0 {
                    next_pause.set(PauseState::Running);
                    request_transition(&mut requests, AppState::SongSelect);
                } else {
                    exit_armed.0 = true;
                }
            }
        }
    }

    let theme = Theme::default();
    for (item, mut text, mut color) in &mut rows {
        text.0 = rail_label(*item, &session, exit_armed.0);
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

/// Reposition playhead / scrub cursor / loop fill each frame while open.
#[allow(clippy::type_complexity)]
pub fn update_full_hud_markers(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut time_text: Query<&mut Text, With<HudTimeText>>,
    mut markers: ParamSet<(
        Query<&mut Node, With<HudPlayhead>>,
        Query<(&mut Node, &mut Visibility), With<HudScrubCursor>>,
        Query<(&mut Node, &mut Visibility), With<HudLoopFill>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut t) = time_text.single_mut() {
        t.0 = format_chart_time(session.scrub_cursor_ms.unwrap_or(clock.current_ms));
    }
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
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match session.loop_region.filter(|r| r.end_ms != i64::MAX) {
            Some(r) => {
                let a = time_to_pct(r.start_ms, end);
                let b = time_to_pct(r.end_ms, end);
                node.left = Val::Percent(a);
                node.width = Val::Percent((b - a).max(0.0));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
```

Update `crates/gameplay-drums/src/practice/hud/mod.rs`:

```rust
pub mod full_hud;
pub mod timeline_ui;
```

and the plugin body:

```rust
pub(super) fn plugin(app: &mut App) {
    use game_shell::{AppState, PauseState};
    app.init_resource::<full_hud::RailSelection>()
        .init_resource::<full_hud::ExitArmed>()
        .add_systems(
            OnEnter(PauseState::Paused),
            full_hud::spawn_full_hud.run_if(resource_exists::<crate::practice::PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), full_hud::despawn_full_hud)
        .add_systems(
            Update,
            (full_hud::full_hud_input, full_hud::update_full_hud_markers)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<crate::practice::PracticeSession>),
        );
}
```

In `crates/gameplay-drums/src/practice/mod.rs`, add `hud::plugin` to the `add_plugins` tuple (line 33) and in `crates/gameplay-drums/src/practice/ui.rs` delete the pause-panel half: the plugin registrations at lines 58-69 (`init_resource::<PracticeSelection>` block) and everything from line 237 (`/// Root marker for the practice pause panel.`) to line 484 (end of `practice_panel_input`), plus now-unused imports (`request_transition`, `TransitionRequest`, `preroll_target`, `SeekToChartTime`). The transport strip (lines 26-235) and `format_chart_time` stay until Task 8.

- [ ] **Step 5: Run tests (expect PASS) and full suite**

Run: `cargo test -p gameplay-drums --test practice_hud && cargo test -p gameplay-drums --lib`
Expected: 3 HUD tests pass; lib tests pass (ui.rs `format_chart_time` test still present).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/practice crates/gameplay-drums/src/pause.rs crates/gameplay-drums/tests/practice_hud.rs
git commit -m "feat(gameplay-drums): full practice HUD replaces pause panel"
```

---

### Task 5: Timeline mouse — click seek + drag loop

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/timeline_ui.rs` (append system), `crates/gameplay-drums/src/practice/hud/mod.rs` (register)

- [ ] **Step 1: Implement the mouse system**

Append to `timeline_ui.rs` (above the tests module):

```rust
use crate::practice::session::PracticeSession;
use crate::seek::SeekToChartTime;

/// Logical-px rect of the timeline strip node (same math as
/// editor/picking.rs `node_rect`; duplicated to avoid coupling the
/// practice pillar to editor files).
fn strip_rect(node: &ComputedNode, gt: &GlobalTransform) -> Rect {
    let inv = node.inverse_scale_factor();
    let center = gt.translation().truncate() * inv;
    let size = node.size() * inv;
    Rect::from_center_size(center, size)
}

/// Mouse on the full-HUD timeline: press+release = seek (snapped via the
/// seek message's `snap` field), press+drag = select A/B region
/// (bar-snapped, min one bar, committed live while dragging).
pub fn timeline_mouse(
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    strips: Query<(&ComputedNode, &GlobalTransform), With<super::full_hud::FullHudTimelineStrip>>,
    mut gesture: ResMut<TimelineGesture>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        return;
    };
    let Ok((node, gt)) = strips.single() else {
        return;
    };
    let rect = strip_rect(node, gt);
    let cursor_ms = cursor_to_ms(pos.x, rect.min.x, rect.width(), timeline.end_ms);
    let input = GestureInput {
        just_pressed: buttons.just_pressed(MouseButton::Left),
        pressed: buttons.pressed(MouseButton::Left),
        inside_strip: rect.contains(pos),
        cursor_x: pos.x,
        cursor_ms,
    };
    let (next, effect) = advance_gesture(*gesture, input);
    *gesture = next;
    match effect {
        GestureEffect::None => {}
        GestureEffect::Seek { target_ms } => {
            let snapped = timeline.resolve_snap(target_ms, session.snap);
            session.scrub_cursor_ms = Some(snapped);
            seeks.write(SeekToChartTime {
                target_ms,
                snap: Some(session.snap),
                attempt_start_ms: None,
            });
        }
        GestureEffect::LoopPreview { anchor_ms } => {
            session.loop_region = Some(drag_region(&timeline, anchor_ms, cursor_ms));
        }
    }
}
```

- [ ] **Step 2: Register in `hud/mod.rs`**

Add to the hud plugin, alongside the paused-tier Update systems:

```rust
        .init_resource::<timeline_ui::TimelineGesture>()
```

and extend the paused Update tuple to:

```rust
            (
                timeline_ui::timeline_mouse,
                full_hud::full_hud_input,
                full_hud::update_full_hud_markers,
            )
                .chain()
```

(`timeline_mouse` runs before `full_hud_input` so a click's scrub-cursor update is repainted the same frame.)

- [ ] **Step 3: Verify build + tests**

Run: `cargo test -p gameplay-drums --lib practice::hud && cargo check -p gameplay-drums`
Expected: PASS / clean. (Gesture and mapping behavior is covered by Task 3's pure tests; the system is a thin adapter. Mouse feel is verified in Task 13.)

- [ ] **Step 4: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud
git commit -m "feat(gameplay-drums): mouse scrub + drag-loop on practice timeline"
```

---

### Task 6: Transport row — clickable prev bar / resume / next bar

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` (buttons + system), `crates/gameplay-drums/src/practice/hud/mod.rs` (register)
- Test: `crates/gameplay-drums/tests/practice_hud.rs` (append)

- [ ] **Step 1: Write the failing test**

Append to `tests/practice_hud.rs`:

```rust
use gameplay_drums::practice::hud::full_hud::{transport_buttons, TransportButton};

#[test]
fn next_bar_button_moves_scrub_cursor() {
    let mut app = build_app();
    app.add_systems(Update, transport_buttons);
    // 2 bars @ 120 BPM: bar starts at 0 and 2000.
    let chart = dtx_core::chart::Chart {
        metadata: dtx_core::chart::Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![dtx_core::chart::Chip::new(
            0,
            dtx_core::channel::EChannel::BassDrum,
            0.0,
        )],
        ..Default::default()
    };
    let bpm = gameplay_drums::judge::BpmChangeList::from_chart(&chart);
    let bar = gameplay_drums::judge::BarLengthChangeList::from_chart(&chart);
    app.world_mut().insert_resource(ChipTimeline::from_chart(
        &chart, &bpm, &bar, 0, 4_000,
    ));
    app.world_mut().insert_resource(PracticeSession {
        scrub_cursor_ms: Some(0),
        ..Default::default()
    });
    app.world_mut()
        .spawn((Interaction::Pressed, TransportButton::NextBar));
    app.update();
    assert_eq!(
        app.world()
            .resource::<PracticeSession>()
            .scrub_cursor_ms,
        Some(2_000),
        "next-bar button advances the scrub cursor one bar"
    );
}
```

- [ ] **Step 2: Run it (expect FAIL)**

Run: `cargo test -p gameplay-drums --test practice_hud -- next_bar_button`
Expected: FAIL to compile — `TransportButton` missing.

- [ ] **Step 3: Implement buttons + handler**

Append to `full_hud.rs`:

```rust
/// Clickable transport-row button.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum TransportButton {
    PrevBar,
    Resume,
    NextBar,
}

impl TransportButton {
    fn label(self) -> &'static str {
        match self {
            TransportButton::PrevBar => "|◀ bar",
            TransportButton::Resume => "▶ resume",
            TransportButton::NextBar => "bar ▶|",
        }
    }
}

pub fn transport_buttons(
    interactions: Query<(&Interaction, &TransportButton), Changed<Interaction>>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            TransportButton::PrevBar | TransportButton::NextBar => {
                let dir: i8 = if *button == TransportButton::NextBar { 1 } else { -1 };
                let cur = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                session.scrub_cursor_ms = Some(timeline.snap_neighbor(
                    cur,
                    crate::timeline::SnapDivisor::Bar,
                    dir,
                ));
            }
            TransportButton::Resume => next_pause.set(PauseState::Running),
        }
    }
}
```

In `spawn_full_hud`, inside the bottom timeline row's `with_children` closure, add the transport buttons before the time text:

```rust
                for button in [
                    TransportButton::PrevBar,
                    TransportButton::Resume,
                    TransportButton::NextBar,
                ] {
                    row.spawn((
                        button,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(button.label()),
                            Theme::label_font(),
                            TextColor(theme.text_primary),
                        ));
                    });
                }
```

Register `full_hud::transport_buttons` in the hud plugin's paused Update tuple (after `full_hud_input`).

- [ ] **Step 4: Run it (expect PASS)**

Run: `cargo test -p gameplay-drums --test practice_hud`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud crates/gameplay-drums/tests/practice_hud.rs
git commit -m "feat(gameplay-drums): clickable transport row in practice HUD"
```

---

### Task 7: `toast.rs` — toast queue + quick-tier feedback

**Files:**
- Create: `crates/gameplay-drums/src/practice/toast.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (module + registration), `crates/gameplay-drums/src/practice/actions.rs` (`apply_practice_actions` pushes toasts)
- Test: unit tests inside `toast.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/gameplay-drums/src/practice/toast.rs`:

```rust
//! Practice-local toast queue (spec: generalize only when a second
//! consumer appears). Newest at the bottom, cap 4, ~1.5 s life.

use bevy::prelude::*;
use dtx_ui::theme::Theme;

pub const TOAST_CAP: usize = 4;
pub const TOAST_SECS: f32 = 1.5;

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Debug, Default)]
pub struct ToastQueue(pub Vec<Toast>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_at_four_dropping_oldest() {
        let mut q = ToastQueue::default();
        for i in 0..6 {
            q.push(format!("t{i}"));
        }
        assert_eq!(q.0.len(), TOAST_CAP);
        assert_eq!(q.0[0].text, "t2", "oldest dropped first");
        assert_eq!(q.0[3].text, "t5");
    }

    #[test]
    fn tick_expires_old_toasts() {
        let mut q = ToastQueue::default();
        q.push("a");
        q.tick(1.0);
        q.push("b");
        q.tick(0.6); // a: 1.6s (dead), b: 0.6s (alive)
        assert_eq!(q.0.len(), 1);
        assert_eq!(q.0[0].text, "b");
    }
}
```

- [ ] **Step 2: Run them (expect FAIL)**

Run: `cargo test -p gameplay-drums --lib practice::toast`
Expected: FAIL to compile — `push`/`tick` missing. (Add `pub mod toast;` to `practice/mod.rs` first so the module compiles.)

- [ ] **Step 3: Implement queue + UI system**

Add between the type definitions and tests:

```rust
impl ToastQueue {
    pub fn push(&mut self, text: impl Into<String>) {
        self.0.push(Toast {
            text: text.into(),
            age: 0.0,
        });
        while self.0.len() > TOAST_CAP {
            self.0.remove(0);
        }
    }

    /// Age all toasts by `dt` seconds and drop expired ones.
    pub fn tick(&mut self, dt: f32) {
        for t in &mut self.0 {
            t.age += dt;
        }
        self.0.retain(|t| t.age < TOAST_SECS);
    }
}

#[derive(Component)]
pub struct ToastRoot;

/// Rebuild the top-center toast column each frame (≤4 small texts, so a
/// rebuild is cheaper than diffing).
pub fn toast_ui(
    time: Res<Time>,
    mut queue: ResMut<ToastQueue>,
    mut commands: Commands,
    roots: Query<Entity, With<ToastRoot>>,
) {
    queue.tick(time.delta_secs());
    for e in &roots {
        commands.entity(e).despawn();
    }
    if queue.0.is_empty() {
        return;
    }
    let theme = Theme::default();
    commands
        .spawn((
            ToastRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(56.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(1100),
        ))
        .with_children(|col| {
            for t in &queue.0 {
                col.spawn((
                    Text::new(t.text.clone()),
                    Theme::label_font(),
                    TextColor(theme.text_primary),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                        ..default()
                    },
                ));
            }
        });
}
```

Register in `practice/mod.rs` plugin:

```rust
    app.init_resource::<toast::ToastQueue>().add_systems(
        Update,
        toast::toast_ui
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
```

- [ ] **Step 4: Wire toasts into the action applier**

In `actions.rs`, add `use super::toast::ToastQueue;` and `use super::hud::timeline_ui::bar_number;`, add the parameter `mut toasts: ResMut<ToastQueue>,` to `apply_practice_actions`, and update the match arms that give feedback:

```rust
            PracticeAction::SetLoopStart => {
                let ms = timeline.bar_start_before(clock.current_ms);
                session.set_loop_start(ms);
                toasts.push(format!("A set @ bar {}", bar_number(&timeline.bar_ms, ms)));
            }
            PracticeAction::SetLoopEnd => {
                let mut ms = timeline.bar_start_before(clock.current_ms);
                if let Some(r) = session.loop_region {
                    if ms <= r.start_ms {
                        ms = timeline.snap_neighbor(r.start_ms, SnapDivisor::Bar, 1);
                    }
                }
                session.set_loop_end(ms);
                toasts.push(format!("B set @ bar {}", bar_number(&timeline.bar_ms, ms)));
            }
            PracticeAction::ClearLoop => {
                session.loop_region = None;
                toasts.push("loop cleared");
            }
            PracticeAction::RateDown => {
                session.step_rate(-1);
                toasts.push(format!("rate → {:.2}×", session.rate));
            }
            PracticeAction::RateUp => {
                session.step_rate(1);
                toasts.push(format!("rate → {:.2}×", session.rate));
            }
```

(`RestartLoop` / `OpenFullHud` / `ToggleRamp` arms unchanged; add `toasts.push("restart");` to `RestartLoop` after the seek write.)

Update `add_action_wiring` in `tests/practice_mode.rs` to also
`.init_resource::<gameplay_drums::practice::toast::ToastQueue>()`.

- [ ] **Step 5: Run everything (expect PASS)**

Run: `cargo test -p gameplay-drums --lib practice:: && cargo test -p gameplay-drums --test practice_mode`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/practice crates/gameplay-drums/tests/practice_mode.rs
git commit -m "feat(gameplay-drums): toast feedback for quick-tier practice actions"
```

---

### Task 8: `hud/mini_strip.rs` — quick-tier loop strip; delete `ui.rs`

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/mini_strip.rs`
- Delete: `crates/gameplay-drums/src/practice/ui.rs`
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (drop `pub mod ui;` + `ui::plugin`), `crates/gameplay-drums/src/practice/hud/mod.rs` (module + registration + move the `format_chart_time` unit test from ui.rs)

- [ ] **Step 1: Implement the mini strip**

Create `crates/gameplay-drums/src/practice/hud/mini_strip.rs`:

```rust
//! Quick-tier mini loop-strip: thin full-width bar at the bottom edge
//! showing playhead + armed A/B region over the full song extent.
//! Density is deliberately omitted at this size (spec §Quick tier).

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::time_to_pct;
use game_shell::AppState;

use crate::practice::session::PracticeSession;
use crate::resources::GameplayClock;
use crate::timeline::ChipTimeline;

#[derive(Component)]
pub struct MiniStripRoot;
#[derive(Component)]
pub struct MiniPlayhead;
#[derive(Component)]
pub struct MiniLoopFill;

pub fn spawn_mini_strip(mut commands: Commands) {
    let theme = Theme::default();
    commands
        .spawn((
            MiniStripRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(900),
        ))
        .with_children(|strip| {
            strip.spawn((
                MiniLoopFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.35)),
                Visibility::Hidden,
            ));
            strip.spawn((
                MiniPlayhead,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(theme.accent),
            ));
        });
}

pub fn despawn_mini_strip(mut commands: Commands, roots: Query<Entity, With<MiniStripRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
pub fn update_mini_strip(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut markers: ParamSet<(
        Query<&mut Node, With<MiniPlayhead>>,
        Query<(&mut Node, &mut Visibility), With<MiniLoopFill>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
        match session.loop_region.filter(|r| r.end_ms != i64::MAX) {
            Some(r) => {
                let a = time_to_pct(r.start_ms, end);
                let b = time_to_pct(r.end_ms, end);
                node.left = Val::Percent(a);
                node.width = Val::Percent((b - a).max(0.0));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_mini_strip
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_mini_strip)
    .add_systems(
        Update,
        update_mini_strip
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}
```

- [ ] **Step 2: Delete `ui.rs`, rehome its test**

- `git rm crates/gameplay-drums/src/practice/ui.rs`
- In `practice/mod.rs`: remove `pub mod ui;` and `ui::plugin` from the `add_plugins` tuple.
- In `hud/mod.rs`: add `pub mod mini_strip;`, call `mini_strip::plugin(app);` at the top of the hud plugin, and append the test moved from ui.rs:

```rust
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

- Run `grep -rn "practice::ui\|practice::ui::" crates/` — fix any straggler imports (expected: none; `format_chart_time` consumers already import from `practice::hud`).

- [ ] **Step 3: Run the whole crate's tests (expect PASS)**

Run: `cargo test -p gameplay-drums`
Expected: all pass; no references to the deleted module.

- [ ] **Step 4: Commit**

```bash
git add -A crates/gameplay-drums/src/practice
git commit -m "feat(gameplay-drums): quick-tier mini loop-strip, retire practice/ui.rs"
```

---

### Task 9: `hud/chip.rs` — quick-tier status chip

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/chip.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mod.rs` (module + plugin)
- Test: unit test inside `chip.rs`

- [ ] **Step 1: Write the failing test for the pure text builder**

Create `chip.rs` with the test first:

```rust
//! Quick-tier status chip (top-right): rate, loop bars, last accuracy.
//! The ramp segment is added by Task 12 once ramp state exists.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use game_shell::AppState;

use super::timeline_ui::bar_number;
use crate::practice::session::PracticeSession;
use crate::timeline::ChipTimeline;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{AttemptRecord, LoopRegion};

    #[test]
    fn chip_text_shows_rate_loop_and_last_accuracy() {
        let mut s = PracticeSession::default();
        let bar_ms = vec![0, 2_000, 4_000, 6_000, 8_000];
        assert_eq!(chip_text(&s, &bar_ms), "1.00×");

        s.rate = 0.85;
        s.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        s.attempt_history.push(AttemptRecord {
            start_ms: 2_000,
            end_ms: 6_000,
            rate: 0.85,
            counts: Default::default(),
            max_combo: 12,
            accuracy_pct: 94.2,
            mean_error_ms: -3.0,
        });
        assert_eq!(chip_text(&s, &bar_ms), "0.85× · loop 2–4 · 94%");
    }
}
```

- [ ] **Step 2: Run it (expect FAIL)**

Run: `cargo test -p gameplay-drums --lib practice::hud::chip`
Expected: FAIL to compile — `chip_text` missing. (Add `pub mod chip;` to `hud/mod.rs`.)

- [ ] **Step 3: Implement text builder + UI**

Add above the tests:

```rust
#[derive(Component)]
pub struct StatusChip;

/// Pure: chip contents from session state. `bar_ms` from `ChipTimeline`.
pub fn chip_text(session: &PracticeSession, bar_ms: &[i64]) -> String {
    let mut parts = vec![format!("{:.2}×", session.rate)];
    if let Some(r) = session.loop_region.filter(|r| r.end_ms != i64::MAX) {
        parts.push(format!(
            "loop {}–{}",
            bar_number(bar_ms, r.start_ms),
            bar_number(bar_ms, r.end_ms)
        ));
    }
    if let Some(last) = session.attempt_history.last() {
        parts.push(format!("{:.0}%", last.accuracy_pct));
    }
    parts.join(" · ")
}

pub fn spawn_chip(mut commands: Commands, session: Res<PracticeSession>, timeline: Res<ChipTimeline>) {
    let theme = Theme::default();
    commands.spawn((
        StatusChip,
        Text::new(chip_text(&session, &timeline.bar_ms)),
        Theme::label_font(),
        TextColor(theme.text_primary),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(12.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        GlobalZIndex(900),
    ));
}

pub fn despawn_chip(mut commands: Commands, chips: Query<Entity, With<StatusChip>>) {
    for e in &chips {
        commands.entity(e).despawn();
    }
}

pub fn update_chip(
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut chips: Query<&mut Text, With<StatusChip>>,
) {
    if let Ok(mut t) = chips.single_mut() {
        t.0 = chip_text(&session, &timeline.bar_ms);
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_chip
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_chip)
    .add_systems(
        Update,
        update_chip
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}
```

Call `chip::plugin(app);` in the hud plugin next to `mini_strip::plugin(app);`.

- [ ] **Step 4: Run it (expect PASS)**

Run: `cargo test -p gameplay-drums --lib practice::hud`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud
git commit -m "feat(gameplay-drums): quick-tier practice status chip"
```

---

### Task 10: Ramp data + pure `ramp_step` protocol (TDD)

**Files:**
- Modify: `crates/gameplay-drums/src/practice/session.rs` (types after line 52, fields at lines 97-121)
- Create: `crates/gameplay-drums/src/practice/ramp.rs` (pure layer only)
- Modify: `crates/gameplay-drums/src/practice/mod.rs` (add `pub mod ramp;`)
- Test: unit tests inside `ramp.rs`

- [ ] **Step 1: Add ramp state to the session**

In `session.rs`, after `preroll_target` (line 52), add:

```rust
pub const RAMP_START_DEFAULT: f32 = 0.70;
pub const RAMP_TARGET_DEFAULT: f32 = 1.00;
pub const RAMP_STEP_DEFAULT: f32 = 0.05;
pub const RAMP_THRESHOLD_DEFAULT: f32 = 90.0;

/// Accuracy-gated speed-ramp configuration (rail-editable).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampConfig {
    pub start_rate: f32,
    pub target_rate: f32,
    pub step: f32,
    pub threshold_pct: f32,
}

impl Default for RampConfig {
    fn default() -> Self {
        Self {
            start_rate: RAMP_START_DEFAULT,
            target_rate: RAMP_TARGET_DEFAULT,
            step: RAMP_STEP_DEFAULT,
            threshold_pct: RAMP_THRESHOLD_DEFAULT,
        }
    }
}

/// Live ramp state. `current_rate` mirrors `PracticeSession::rate` (the
/// applier re-adopts it each pass, so a manual rate nudge simply becomes
/// the ramp's current step).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampState {
    pub armed: bool,
    pub current_rate: f32,
    pub consecutive_fails: u8,
    /// Arming mid-loop rolls a stale pre-arm attempt on the next seek;
    /// the applier skips exactly one roll when this is set.
    pub skip_next_roll: bool,
}

impl Default for RampState {
    fn default() -> Self {
        Self {
            armed: false,
            current_rate: RAMP_START_DEFAULT,
            consecutive_fails: 0,
            skip_next_roll: false,
        }
    }
}
```

Add to `PracticeSession` (fields, then the same two lines in `Default::default()`):

```rust
    pub ramp_config: RampConfig,
    pub ramp: RampState,
```

```rust
            ramp_config: RampConfig::default(),
            ramp: RampState::default(),
```

- [ ] **Step 2: Write the failing table tests**

Create `crates/gameplay-drums/src/practice/ramp.rs`:

```rust
//! Accuracy-gated rate ramp (Rocksmith riff-repeater model). The
//! protocol is a pure function; systems only apply its decisions.

use bevy::prelude::*;

use super::session::{preroll_target, PracticeSession, RampConfig, RampState};
use super::toast::ToastQueue;
use crate::seek::SeekToChartTime;

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RampConfig {
        RampConfig::default() // 0.70 → 1.00, step 0.05, threshold 90%
    }

    fn state(rate: f32, fails: u8) -> RampState {
        RampState {
            armed: true,
            current_rate: rate,
            consecutive_fails: fails,
            skip_next_roll: false,
        }
    }

    #[test]
    fn clean_pass_steps_up() {
        let mut s = state(0.70, 0);
        let d = ramp_step(&cfg(), &mut s, 95.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.75 });
        assert!((s.current_rate - 0.75).abs() < 1e-6);
        assert_eq!(s.consecutive_fails, 0);
    }

    #[test]
    fn first_fail_holds() {
        let mut s = state(0.80, 0);
        let d = ramp_step(&cfg(), &mut s, 60.0);
        assert_eq!(d, RampDecision::Hold);
        assert_eq!(s.consecutive_fails, 1);
        assert!((s.current_rate - 0.80).abs() < 1e-6);
    }

    #[test]
    fn second_consecutive_fail_steps_down() {
        let mut s = state(0.80, 1);
        let d = ramp_step(&cfg(), &mut s, 60.0);
        assert_eq!(d, RampDecision::StepDown { new_rate: 0.75 });
        assert_eq!(s.consecutive_fails, 0, "fail counter resets after demotion");
    }

    #[test]
    fn step_down_floors_at_start_rate() {
        let mut s = state(0.70, 1);
        let d = ramp_step(&cfg(), &mut s, 0.0);
        assert_eq!(d, RampDecision::StepDown { new_rate: 0.70 });
    }

    #[test]
    fn pass_reaching_target_completes_and_disarms() {
        let mut s = state(0.95, 0);
        let d = ramp_step(&cfg(), &mut s, 92.0);
        assert_eq!(d, RampDecision::Complete { new_rate: 1.00 });
        assert!(!s.armed);
    }

    #[test]
    fn pass_resets_fail_counter() {
        let mut s = state(0.80, 1);
        let d = ramp_step(&cfg(), &mut s, 91.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.85 });
        assert_eq!(s.consecutive_fails, 0);
    }

    #[test]
    fn manual_nudge_adoption_steps_from_the_nudged_rate() {
        // A manual nudge to 0.90 mid-ramp becomes the current step.
        let mut s = state(0.75, 0);
        s.current_rate = 0.90; // applier does this from session.rate
        let d = ramp_step(&cfg(), &mut s, 95.0);
        assert_eq!(d, RampDecision::StepUp { new_rate: 0.95 });
    }

    #[test]
    fn step_index_display() {
        let c = cfg();
        assert_eq!(ramp_step_index(&c, 0.70), (0, 6));
        assert_eq!(ramp_step_index(&c, 0.85), (3, 6));
        assert_eq!(ramp_step_index(&c, 1.00), (6, 6));
    }
}
```

- [ ] **Step 3: Run them (expect FAIL)**

Run: `cargo test -p gameplay-drums --lib practice::ramp`
Expected: FAIL to compile — `ramp_step`, `RampDecision`, `ramp_step_index` missing. (Add `pub mod ramp;` to `practice/mod.rs`.)

- [ ] **Step 4: Implement the pure protocol**

Add above the tests in `ramp.rs`:

```rust
/// Outcome of one finished loop pass while the ramp is armed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampDecision {
    StepUp { new_rate: f32 },
    StepDown { new_rate: f32 },
    /// First fail at a step: keep the rate, remember the fail.
    Hold,
    /// Target reached: rate pinned to target, ramp disarms.
    Complete { new_rate: f32 },
}

/// Pure ramp protocol. Pass (accuracy ≥ threshold) → step up, completing
/// at the target. Two consecutive fails → step down once, floored at the
/// start rate.
pub fn ramp_step(cfg: &RampConfig, state: &mut RampState, accuracy_pct: f32) -> RampDecision {
    if accuracy_pct >= cfg.threshold_pct {
        state.consecutive_fails = 0;
        let next = (state.current_rate + cfg.step).min(cfg.target_rate);
        state.current_rate = next;
        if next >= cfg.target_rate - 1e-6 {
            state.armed = false;
            RampDecision::Complete {
                new_rate: cfg.target_rate,
            }
        } else {
            RampDecision::StepUp { new_rate: next }
        }
    } else {
        state.consecutive_fails += 1;
        if state.consecutive_fails >= 2 {
            state.consecutive_fails = 0;
            let next = (state.current_rate - cfg.step).max(cfg.start_rate);
            state.current_rate = next;
            RampDecision::StepDown { new_rate: next }
        } else {
            RampDecision::Hold
        }
    }
}

/// `(current, total)` step indices for display ("RAMP 3/6").
pub fn ramp_step_index(cfg: &RampConfig, rate: f32) -> (u32, u32) {
    if cfg.step <= 0.0 {
        return (0, 0);
    }
    let total = ((cfg.target_rate - cfg.start_rate) / cfg.step).round().max(0.0) as u32;
    let cur = (((rate - cfg.start_rate) / cfg.step).round() as i64).clamp(0, total as i64) as u32;
    (cur, total)
}
```

- [ ] **Step 5: Run them (expect PASS)**

Run: `cargo test -p gameplay-drums --lib practice::`
Expected: all pass (8 new ramp tests + existing).

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/practice/session.rs crates/gameplay-drums/src/practice/ramp.rs crates/gameplay-drums/src/practice/mod.rs
git commit -m "feat(gameplay-drums): pure accuracy-ramp protocol + session ramp state"
```

---

### Task 11: Ramp systems + schedule-ordering guard

**Files:**
- Modify: `crates/gameplay-drums/src/practice/ramp.rs` (systems + plugin), `crates/gameplay-drums/src/practice/mod.rs` (register `ramp::plugin`)
- Test: `crates/gameplay-drums/tests/fixed_update_schedule_ordering.rs:24-72`, `crates/gameplay-drums/tests/practice_mode.rs` (append)

- [ ] **Step 1: Extend the FixedUpdate ordering guard first**

In `tests/fixed_update_schedule_ordering.rs`: add a stub (after line 28) and mirror the new edge in `build_app`. Replace the non-cyclic branch (line 68):

```rust
fn ramp_apply_stub() {}
```

```rust
    } else {
        app.add_systems(
            FixedUpdate,
            (
                track_attempt_stub.after(judge_stub),
                ramp_apply_stub.after(track_attempt_stub),
            ),
        );
    }
```

Also update the file's doc comment mirror note: `practice/ramp.rs (apply_ramp: FixedUpdate, .after(track_attempt_stats))`.

Run: `cargo test -p gameplay-drums --test fixed_update_schedule_ordering`
Expected: PASS (stubs only — this pins the shape the real wiring must have).

- [ ] **Step 2: Write the failing integration tests**

Append to `tests/practice_mode.rs`:

```rust
use gameplay_drums::events::{JudgmentEvent, NoteMissed};
use gameplay_drums::practice::session::RampState;

fn add_ramp_wiring(app: &mut App) {
    app.add_message::<JudgmentEvent>()
        .add_message::<NoteMissed>()
        .init_resource::<gameplay_drums::practice::toast::ToastQueue>()
        .add_systems(
            Update,
            gameplay_drums::practice::ab_loop::loop_watcher
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            (
                gameplay_drums::practice::stats::track_attempt_stats,
                gameplay_drums::practice::ramp::apply_ramp,
            )
                .chain()
                .after(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        );
}

fn looped_session(rate: f32) -> PracticeSession {
    let mut s = PracticeSession {
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        preroll: gameplay_drums::practice::session::PrerollSetting::Off,
        rate,
        ..Default::default()
    };
    s.ramp.armed = true;
    s.ramp.current_rate = rate;
    s.current_attempt.start_ms = 2_000;
    s
}

/// Run the clock past B so the loop watcher rolls one attempt.
fn finish_loop_pass(app: &mut App, perfect_hits: u32) {
    for _ in 0..perfect_hits {
        app.world_mut()
            .resource_mut::<Messages<JudgmentEvent>>()
            .write(JudgmentEvent {
                lane: 3,
                kind: dtx_scoring::JudgmentKind::Perfect,
                delta_ms: 0,
                chip_idx: 0, // chip 0 sits at 2000ms — inside the loop
            });
    }
    if perfect_hits == 0 {
        app.world_mut()
            .resource_mut::<Messages<NoteMissed>>()
            .write(NoteMissed {
                lane: 3,
                audio_ms: 5_000,
            });
    }
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.update();
}

#[test]
fn ramp_steps_rate_up_after_clean_pass() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.70));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.75).abs() < 1e-6,
        "clean pass steps 0.70 → 0.75, got {}",
        session.rate
    );
    assert!(session.ramp.armed);
}

#[test]
fn two_failed_passes_step_rate_down() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.80));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 0); // fail #1 → hold
    assert!((app.world().resource::<PracticeSession>().rate - 0.80).abs() < 1e-6);
    finish_loop_pass(&mut app, 0); // fail #2 → step down
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.75).abs() < 1e-6,
        "second fail steps 0.80 → 0.75, got {}",
        session.rate
    );
}

#[test]
fn skip_next_roll_ignores_the_stale_pre_arm_attempt() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.ramp.skip_next_roll = true;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.rate - 0.70).abs() < 1e-6,
        "the roll right after arming must not step the ramp"
    );
    assert!(!session.ramp.skip_next_roll, "flag consumed");
}
```

Run: `cargo test -p gameplay-drums --test practice_mode -- ramp_ two_failed skip_next`
Expected: FAIL to compile — `apply_ramp` missing.

- [ ] **Step 3: Implement the systems**

Append to `ramp.rs`:

```rust
use crate::timeline::ChipTimeline;

/// Arm/disarm from `PracticeAction::ToggleRamp` (own reader; the quick
/// applier deliberately ignores this variant). Arming without an armed
/// A/B loop is an error toast + no-op. Arming resets the rate to the
/// configured start and restarts the loop so the first pass is clean.
pub fn handle_toggle_ramp(
    mut actions: MessageReader<super::actions::PracticeAction>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut toasts: ResMut<ToastQueue>,
) {
    for action in actions.read() {
        if *action != super::actions::PracticeAction::ToggleRamp {
            continue;
        }
        if session.ramp.armed {
            session.ramp.armed = false;
            toasts.push("ramp off");
            continue;
        }
        if !session.loop_armed() {
            toasts.push("ramp needs an A/B loop");
            continue;
        }
        let cfg = session.ramp_config;
        session.ramp = RampState {
            armed: true,
            current_rate: cfg.start_rate,
            consecutive_fails: 0,
            skip_next_roll: true,
        };
        session.rate = cfg.start_rate;
        let a_ms = session.loop_region.expect("loop_armed checked").start_ms;
        seeks.write(SeekToChartTime {
            target_ms: preroll_target(&timeline, session.preroll, a_ms),
            snap: None,
            attempt_start_ms: Some(a_ms),
        });
        toasts.push(format!("ramp armed @ {:.2}×", cfg.start_rate));
    }
}

/// Apply one ramp decision per finished loop pass. Runs after
/// `track_attempt_stats` (same tick as the loop's seek) so the finished
/// attempt is already in history. Re-adopts `session.rate` as the
/// current step first — a manual nudge simply moves the ramp.
pub fn apply_ramp(
    mut seeks: MessageReader<SeekToChartTime>,
    mut session: ResMut<PracticeSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    if seeks.read().last().is_none() {
        return;
    }
    if !session.ramp.armed {
        return;
    }
    if session.ramp.skip_next_roll {
        session.ramp.skip_next_roll = false;
        return;
    }
    let Some(region) = session.loop_region.filter(|r| r.end_ms != i64::MAX) else {
        return;
    };
    let Some(last) = session.attempt_history.last() else {
        return;
    };
    if last.start_ms != region.start_ms {
        return; // manual seek elsewhere, not a loop pass
    }
    session.ramp.current_rate = session.rate;
    let cfg = session.ramp_config;
    let accuracy = last.accuracy_pct;
    match ramp_step(&cfg, &mut session.ramp, accuracy) {
        RampDecision::StepUp { new_rate } => {
            session.rate = new_rate;
            toasts.push(format!("ramp: {new_rate:.2}×"));
        }
        RampDecision::StepDown { new_rate } => {
            session.rate = new_rate;
            toasts.push(format!("ramp: back to {new_rate:.2}×"));
        }
        RampDecision::Hold => toasts.push("ramp: one more fail steps down"),
        RampDecision::Complete { new_rate } => {
            session.rate = new_rate;
            toasts.push("ramp complete");
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    use game_shell::AppState;
    app.add_systems(
        Update,
        // Not Running-gated: the rail's ramp row (Task 12) toggles while
        // paused via the same message.
        handle_toggle_ramp
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(
        FixedUpdate,
        apply_ramp
            .after(crate::practice::stats::track_attempt_stats)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );
}
```

Register `ramp::plugin` in the `add_plugins` tuple in `practice/mod.rs` (line 33).

- [ ] **Step 4: Run everything (expect PASS)**

Run: `cargo test -p gameplay-drums --test practice_mode && cargo test -p gameplay-drums --test fixed_update_schedule_ordering && cargo test -p gameplay-drums --lib`
Expected: all pass — including the schedule guard, which now mirrors the real `apply_ramp` edge.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice crates/gameplay-drums/tests
git commit -m "feat(gameplay-drums): accuracy ramp systems + schedule ordering guard"
```

---

### Task 12: Rail ramp rows + chip ramp segment

**Files:**
- Modify: `crates/gameplay-drums/src/practice/hud/full_hud.rs` (`RailItem` + `rail_label` + `full_hud_input`), `crates/gameplay-drums/src/practice/hud/chip.rs` (`chip_text` + test)

- [ ] **Step 1: Write the failing chip test**

In `chip.rs` tests, append:

```rust
    #[test]
    fn chip_text_shows_ramp_segment_when_armed() {
        let mut s = PracticeSession::default();
        s.rate = 0.85;
        s.ramp.armed = true;
        let bar_ms = vec![0, 2_000];
        assert_eq!(chip_text(&s, &bar_ms), "0.85× · RAMP 3/6");
    }
```

Run: `cargo test -p gameplay-drums --lib practice::hud::chip -- ramp_segment`
Expected: FAIL — no ramp segment emitted.

- [ ] **Step 2: Implement the chip segment**

In `chip_text`, after the rate part:

```rust
    if session.ramp.armed {
        let (cur, total) =
            crate::practice::ramp::ramp_step_index(&session.ramp_config, session.rate);
        parts.push(format!("RAMP {cur}/{total}"));
    }
```

Run: `cargo test -p gameplay-drums --lib practice::hud::chip`
Expected: PASS.

- [ ] **Step 3: Add the ramp rows to the rail**

In `full_hud.rs`, extend `RailItem` (insert between `Preroll` and `ExitPractice`):

```rust
    RampArm,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
```

and the order array becomes:

```rust
    pub const ORDER: [RailItem; 15] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::RampArm,
        RailItem::RampStart,
        RailItem::RampTarget,
        RailItem::RampStep,
        RailItem::RampThreshold,
        RailItem::ExitPractice,
    ];
```

Extend `rail_label` with:

```rust
        RailItem::RampArm => {
            if session.ramp.armed {
                let (cur, total) = crate::practice::ramp::ramp_step_index(
                    &session.ramp_config,
                    session.rate,
                );
                format!("Ramp  ON  ({cur}/{total})")
            } else {
                "Ramp  off  (Enter: arm)".into()
            }
        }
        RailItem::RampStart => format!("Ramp start  ◀ x{:.2} ▶", session.ramp_config.start_rate),
        RailItem::RampTarget => format!("Ramp target  ◀ x{:.2} ▶", session.ramp_config.target_rate),
        RailItem::RampStep => format!("Ramp step  ◀ +{:.2} ▶", session.ramp_config.step),
        RailItem::RampThreshold => {
            format!("Ramp pass  ◀ ≥{:.0}% ▶", session.ramp_config.threshold_pct)
        }
```

In `full_hud_input`: add the parameter `mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,` (12 params total, under the ceiling). Extend the left/right block:

```rust
            RailItem::RampStart => {
                let c = &mut session.ramp_config;
                c.start_rate =
                    (c.start_rate + dir as f32 * 0.05).clamp(0.5, c.target_rate - 0.05);
            }
            RailItem::RampTarget => {
                let c = &mut session.ramp_config;
                c.target_rate =
                    (c.target_rate + dir as f32 * 0.05).clamp(c.start_rate + 0.05, 1.5);
            }
            RailItem::RampStep => {
                let c = &mut session.ramp_config;
                c.step = (c.step + dir as f32 * 0.05).clamp(0.05, 0.25);
            }
            RailItem::RampThreshold => {
                let c = &mut session.ramp_config;
                c.threshold_pct = (c.threshold_pct + dir as f32 * 5.0).clamp(50.0, 100.0);
            }
```

and the Enter block:

```rust
            RailItem::RampArm => {
                practice_actions.write(crate::practice::actions::PracticeAction::ToggleRamp);
            }
            RailItem::RampStart
            | RailItem::RampTarget
            | RailItem::RampStep
            | RailItem::RampThreshold => {}
```

- [ ] **Step 4: Run all tests (expect PASS)**

Run: `cargo test -p gameplay-drums`
Expected: all pass (`practice_hud.rs` tests are count-based, unaffected by the longer rail).

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/practice/hud
git commit -m "feat(gameplay-drums): ramp config rows in practice rail + chip readout"
```

---

### Task 13: Verification

**Files:** none (verification only)

- [ ] **Step 1: Full automated pass**

```bash
cargo test -p gameplay-drums
cargo clippy -p gameplay-drums -- -D warnings
cargo fmt -p gameplay-drums -- --check
```

Expected: all green. (Do NOT run `cargo fmt --all` — formatter version drift.)

- [ ] **Step 2: Manual checklist** (launch with `cargo run`, enter a song via Shift+Enter practice from song select)

- Quick tier: `[`/`]` set A/B with toasts naming the bar; `Backspace` clears; `-`/`=` nudge rate with toast; `R` restarts at A with pre-roll; mini strip shows playhead + green region; chip (top-right) tracks rate/loop/accuracy.
- Full HUD: `Esc`/`Tab` opens; gameplay dims; normal pause menu never appears; timeline click seeks (snapped, feels immediate); click-drag paints a bar-snapped loop live (min one bar); transport buttons work by mouse; keyboard rail nav (arrows/Enter) works; Exit requires a second Enter; `Esc` resumes.
- Ramp: `T` without a loop → error toast; with a loop → "ramp armed @ 0.70×", rate drops, loop restarts; clean passes (autoplay F1 if needed) step up with toasts; two sloppy passes step down; reaching 1.00× → "ramp complete", chip drops the RAMP segment; manual `=` nudges mid-ramp are adopted, not fought.
- Toasts: rapid keypresses never show more than 4; each fades after ~1.5 s.
- Normal (non-practice) play: pause menu unchanged, no strips/chip/toasts.

- [ ] **Step 3: Finish**

Use superpowers:finishing-a-development-branch (branch `feat/practice-ux-v2`; merge order note from the spec: editor branch merges first, rebase this branch after).
