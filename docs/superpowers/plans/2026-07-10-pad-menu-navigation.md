# Pad Menu Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drum pads navigate every menu (title, song select, pause, results, settings overlay) through a semantic `NavAction` layer; settings rows get bigger targets.

**Architecture:** A `NavAction` message type lives in `game-shell` (every UI crate already depends on it). `gameplay-drums` maps resolved drum `LaneHit`s to `NavAction`s (it owns lane semantics, bindings, and the editor's capture/calibration suspension states) and relaxes MIDI polling so pads work outside `AppState::Performance`. Each screen keeps its own keyboard system but routes verbs through `NavAction` so pads and keyboard share one consumption path.

**Tech Stack:** Rust, Bevy 0.19 (`Message`/`MessageReader`/`MessageWriter`, `add_message`), existing crates: `game-shell`, `gameplay-drums`, `game-menu`, `game-results`, `dtx-ui`.

**Spec:** `docs/superpowers/specs/2026-07-10-pad-menu-navigation-design.md`

**Repo rules:**
- NEVER run `cargo fmt --all` (local rustfmt reformats unrelated files). Format only files you touched, e.g. `rustfmt crates/game-shell/src/nav.rs`.
- Green unit tests don't prove the FixedUpdate schedule builds. `cargo test --workspace` includes the schedule/ordering guard tests — run it at the end of every task.
- No AI co-authors in commits.

**Key existing types (verified):**
- `gameplay_drums::events::LaneHit { lane: u8, audio_ms: i64 }` — resolved drum hits (crate-local type, NOT `dtx_input::events::LaneHit`).
- `gameplay_drums::lane_map::LANE_ORDER` — lane 0=HiHatClose, 1=Snare, 2=BassDrum, 3=HighTom, 4=LowTom, 5=FloorTom, 6=Cymbal, 7=HiHatOpen, 8=RideCymbal, 9=LeftCymbal, 10=LeftPedal, 11=LeftBassDrum.
- `gameplay_drums::editor`: `EditorOpen` resource + `pub fn editor_open(open: Res<EditorOpen>) -> bool` (mod.rs:185), `tabs::ActiveTab(pub CustomizeTab)`, `tabs::ConfigDraft(pub dtx_config::Config)`, `keyboard_nav::FocusedRow(pub usize)`, `settings_data::settings_items(tab)` rows with `(item.adjust)(&mut draft.0, delta)`, `bindings_capture::CaptureState` (Idle/Capturing/ConfirmSteal), `calibration::CalibrationState` (Idle/Collecting/Done).
- `game_shell::{AppState, PauseState, CustomizeTab}`; `CustomizeTab::is_settings()`.
- MIDI: `gameplay-drums/src/lib.rs` `midi_consumer` module — `connect_midi` (OnEnter Performance + LiveBindings change), `drain_real_midi`, `poll_midi` (all `run_if(in_state(AppState::Performance))`; `poll_midi` also early-returns on empty chart / unready clock). Velocity threshold applied inside `poll_midi` via `BindResolver`.
- Song select: `game-menu/src/song_select.rs` `song_select_navigation` (line ~1207), `Selection { folder, difficulty }`, `SongSelectSelection`, `PracticeIntent`.
- Pause: `gameplay-drums/src/pause.rs` `pause_menu_input`, `PauseSelection`, `PauseItem::ORDER`, Esc `toggle_pause`.
- Results: `game-results/src/lib.rs` `result_input` (line ~270).
- Title: `game-menu/src/title.rs` `title_input` (Enter advances).

---

### Task 1: `NavAction` message + `MidiConnected` resource in game-shell

**Files:**
- Create: `crates/game-shell/src/nav.rs`
- Modify: `crates/game-shell/src/lib.rs` (add `pub mod nav;` + re-export + plugin registration)

- [ ] **Step 1: Write the failing test** — at the bottom of the new `crates/game-shell/src/nav.rs` (file won't compile until Step 3 adds the types; that IS the failing state):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_action_is_copy_and_comparable() {
        let a = NavAction { verb: NavVerb::Up, source: NavSource::Pad, coarse: false };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn midi_connected_defaults_false() {
        assert!(!MidiConnected::default().0);
    }
}
```

- [ ] **Step 2: Write the module** — full content of `crates/game-shell/src/nav.rs` above the tests:

```rust
//! Semantic menu-navigation actions shared by all UI crates.
//!
//! Producers: per-screen keyboard systems and the gameplay-drums pad mapper.
//! Consumers: song select, title, pause menu, results, settings overlay.

use bevy::prelude::*;

/// What the input means, not what produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavVerb {
    /// Move focus up / previous. Pads: HH.
    Up,
    /// Move focus down / next. Pads: CY/RD.
    Down,
    /// Enter / select / apply. Pads: BD.
    Confirm,
    /// Back out / cancel. Pads: SD.
    Back,
    /// Decrement focused value (keyboard Left; pads reuse Up in adjust mode).
    Dec,
    /// Increment focused value (keyboard Right; pads reuse Down in adjust mode).
    Inc,
    /// Start practice mode (keyboard Shift+Enter; pads FT at difficulty level).
    Practice,
}

/// Which device produced the action. Consumers may branch on this: keyboard
/// keeps its flat navigation model, pads use the two-level model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSource {
    Keyboard,
    Pad,
}

/// One navigation action. Screens consume these instead of raw keys/pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct NavAction {
    pub verb: NavVerb,
    pub source: NavSource,
    /// Shift held (keyboard only) — consumers multiply steps by 10.
    pub coarse: bool,
}

/// True while a real MIDI device is connected. Written by gameplay-drums'
/// `connect_midi`; read by legend bars (hidden when false).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MidiConnected(pub bool);

pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>().init_resource::<MidiConnected>();
}
```

- [ ] **Step 3: Wire into game-shell lib** — in `crates/game-shell/src/lib.rs` add `pub mod nav;` next to the other module declarations, re-export `pub use nav::{MidiConnected, NavAction, NavSource, NavVerb};`, and register `nav::plugin` where the crate's plugin builds the app (find the existing `impl Plugin`/plugin fn in lib.rs and add `nav::plugin(app);`). If game-shell has no plugin that gameplay crates install, register instead in the app assembly that already adds game-shell state types (grep `init_state::<AppState>` to find it) — put `game_shell::nav::plugin` beside it.

- [ ] **Step 4: Run tests**

Run: `cargo test -p game-shell nav`
Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/game-shell/src/nav.rs crates/game-shell/src/lib.rs
git commit -m "feat(nav): NavAction message + MidiConnected resource in game-shell"
```

---

### Task 2: Pad → NavAction mapper in gameplay-drums

**Files:**
- Create: `crates/gameplay-drums/src/menu_nav.rs`
- Modify: `crates/gameplay-drums/src/lib.rs` (add `pub mod menu_nav;`, install plugin fn beside the other `plugin(app)` calls)

- [ ] **Step 1: Write the failing tests** — bottom of new `crates/gameplay-drums/src/menu_nav.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use game_shell::NavVerb;

    #[test]
    fn lane_verbs_follow_gitadora_convention() {
        assert_eq!(verb_for_lane(0), Some(NavVerb::Up)); // HH close
        assert_eq!(verb_for_lane(7), Some(NavVerb::Up)); // HH open
        assert_eq!(verb_for_lane(6), Some(NavVerb::Down)); // CY
        assert_eq!(verb_for_lane(8), Some(NavVerb::Down)); // RD
        assert_eq!(verb_for_lane(2), Some(NavVerb::Confirm)); // BD
        assert_eq!(verb_for_lane(1), Some(NavVerb::Back)); // SD
        assert_eq!(verb_for_lane(5), Some(NavVerb::Practice)); // FT
        assert_eq!(verb_for_lane(3), None); // HT unmapped
        assert_eq!(verb_for_lane(10), None); // LP unmapped
    }

    #[test]
    fn guard_enforces_grace_then_debounce() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        // Context just entered: everything inside grace window is rejected.
        g.enter_context(NavContext::SongSelect, t0);
        assert!(!g.accept(t0 + std::time::Duration::from_millis(100)));
        // Past grace: first hit accepted.
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        // 40ms later: debounced.
        assert!(!g.accept(t1 + std::time::Duration::from_millis(40)));
        // 100ms later: accepted.
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
    }

    #[test]
    fn guard_resets_grace_on_context_change() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelect, t0);
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        // Same context re-asserted: no grace reset.
        g.enter_context(NavContext::SongSelect, t1);
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
        // New context: grace applies again.
        g.enter_context(NavContext::Result, t1 + std::time::Duration::from_millis(200));
        assert!(!g.accept(t1 + std::time::Duration::from_millis(300)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gameplay-drums menu_nav`
Expected: compile FAIL — `verb_for_lane`, `NavGuard`, `NavContext` not defined.

- [ ] **Step 3: Write the implementation** — top of `crates/gameplay-drums/src/menu_nav.rs`:

```rust
//! Drum pads → `NavAction`: pads navigate menus (spec
//! 2026-07-10-pad-menu-navigation). Consumes this crate's resolved `LaneHit`s
//! (velocity threshold + bindings already applied by the producers) and emits
//! `game_shell::NavAction { source: Pad }` while a menu context is active.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_shell::{AppState, NavAction, NavSource, NavVerb, PauseState};

use crate::events::LaneHit;

/// Minimum gap between accepted pad nav actions (double-trigger/flam guard).
const DEBOUNCE: Duration = Duration::from_millis(80);
/// Pad nav ignored for this long after entering a screen/context.
const ENTER_GRACE: Duration = Duration::from_millis(500);

/// Which menu surface pads are currently navigating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavContext {
    Title,
    SongSelect,
    Result,
    Paused,
    Editor,
}

/// GITADORA-ish convention. Lane ids per `crate::lane_map::LANE_ORDER`.
pub(crate) fn verb_for_lane(lane: u8) -> Option<NavVerb> {
    match lane {
        0 | 7 => Some(NavVerb::Up),      // HiHatClose / HiHatOpen
        6 | 8 => Some(NavVerb::Down),    // Cymbal / RideCymbal
        2 => Some(NavVerb::Confirm),     // BassDrum
        1 => Some(NavVerb::Back),        // Snare
        5 => Some(NavVerb::Practice),    // FloorTom
        _ => None,
    }
}

/// Debounce + screen-enter grace bookkeeping.
#[derive(Resource, Debug, Default)]
pub struct NavGuard {
    context: Option<NavContext>,
    entered_at: Option<Instant>,
    last_accept: Option<Instant>,
}

impl NavGuard {
    /// Record the active context; resets the grace window on change.
    pub fn enter_context(&mut self, ctx: NavContext, now: Instant) {
        if self.context != Some(ctx) {
            self.context = Some(ctx);
            self.entered_at = Some(now);
            self.last_accept = None;
        }
    }

    pub fn clear_context(&mut self) {
        self.context = None;
    }

    /// True if a pad hit at `now` may become a NavAction.
    pub fn accept(&mut self, now: Instant) -> bool {
        let Some(entered) = self.entered_at else {
            return false;
        };
        if now.saturating_duration_since(entered) < ENTER_GRACE {
            return false;
        }
        if let Some(last) = self.last_accept
            && now.saturating_duration_since(last) < DEBOUNCE
        {
            return false;
        }
        self.last_accept = Some(now);
        true
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NavGuard>()
        .add_systems(Update, pad_nav_mapper);
}

/// Compute the active context. `None` = pads are gameplay input (or a
/// capture/calibration overlay owns raw hits) — no nav.
#[allow(clippy::too_many_arguments)]
fn active_context(
    app_state: &AppState,
    pause: &PauseState,
    editor_open: bool,
    capture_armed: bool,
    calibrating: bool,
) -> Option<NavContext> {
    if capture_armed || calibrating {
        return None;
    }
    match app_state {
        AppState::Title => Some(NavContext::Title),
        AppState::SongSelect => Some(NavContext::SongSelect),
        AppState::Result => Some(NavContext::Result),
        AppState::Performance => {
            if editor_open {
                Some(NavContext::Editor)
            } else if *pause == PauseState::Paused {
                Some(NavContext::Paused)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn pad_nav_mapper(
    app_state: Res<State<AppState>>,
    pause: Res<State<PauseState>>,
    editor_open: Res<crate::editor::EditorOpen>,
    capture: Res<crate::editor::bindings_capture::CaptureState>,
    calibration: Res<crate::editor::calibration::CalibrationState>,
    mut hits: MessageReader<LaneHit>,
    mut guard: ResMut<NavGuard>,
    mut out: MessageWriter<NavAction>,
) {
    let now = Instant::now();
    let ctx = active_context(
        app_state.get(),
        pause.get(),
        crate::editor::editor_open_flag(&editor_open),
        !matches!(*capture, crate::editor::bindings_capture::CaptureState::Idle),
        !matches!(
            *calibration,
            crate::editor::calibration::CalibrationState::Idle
        ),
    );
    let Some(ctx) = ctx else {
        guard.clear_context();
        hits.clear();
        return;
    };
    guard.enter_context(ctx, now);
    for hit in hits.read() {
        let Some(verb) = verb_for_lane(hit.lane) else {
            continue;
        };
        if !guard.accept(now) {
            continue;
        }
        out.write(NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        });
    }
}
```

Notes for this step:
- `AppState` variant names: verify against `crates/game-shell/src/states.rs` (e.g. `Result` vs `Results`, `Title`) and adjust the two `match`es.
- `editor_open_flag`: `EditorOpen`'s field is private to the editor module's design; add a tiny accessor in `crates/gameplay-drums/src/editor/mod.rs` next to `editor_open` (line 185):

```rust
/// Plain-bool view of `EditorOpen` for callers outside a run-condition.
pub fn editor_open_flag(open: &EditorOpen) -> bool {
    open.0
}
```

(If `EditorOpen`'s field is already `pub`, skip the helper and use `editor_open.0` directly.)
- `CaptureState` / `CalibrationState` visibility: both are `pub` types in `pub` modules (`bindings_capture.rs:28`, `calibration.rs:40`); if the modules are `pub(crate)`, that's fine — `menu_nav` is in the same crate.
- If `PauseState` derives `PartialEq` you can compare directly; otherwise use `matches!`.

- [ ] **Step 4: Wire the plugin** — in `crates/gameplay-drums/src/lib.rs`, add `pub mod menu_nav;` beside the other module declarations and call `menu_nav::plugin(app);` where sibling modules (e.g. `input::plugin`, `pause`) are installed.

- [ ] **Step 5: Run tests**

Run: `cargo test -p gameplay-drums menu_nav`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/menu_nav.rs crates/gameplay-drums/src/lib.rs crates/gameplay-drums/src/editor/mod.rs
git commit -m "feat(nav): map drum LaneHits to NavAction in menu contexts"
```

---

### Task 3: MIDI alive outside Performance + `MidiConnected`

**Files:**
- Modify: `crates/gameplay-drums/src/lib.rs` (`midi_consumer` module, lines ~333–457)

- [ ] **Step 1: Write the failing test** — in the `tests` section of the `midi_consumer` area (or the crate's existing test module for `poll_midi` logic, if any). The chart/clock guards are what we're relaxing, so test the extracted stamp helper:

```rust
#[test]
fn menu_hits_are_stamped_even_without_clock() {
    // clock not ready → fall back to the event's own audio_ms (0 for real devices)
    assert_eq!(super::midi_consumer::stamp_audio_ms(None, 123), 123);
    assert_eq!(super::midi_consumer::stamp_audio_ms(Some(5000), 0), 5000);
    // event-provided stamp wins when present
    assert_eq!(super::midi_consumer::stamp_audio_ms(Some(5000), 123), 123);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gameplay-drums stamp_audio_ms`
Expected: compile FAIL — `stamp_audio_ms` not defined.

- [ ] **Step 3: Implement.** Inside `midi_consumer`:

(a) Extract the stamp decision (replaces the inline `if audio_ms != 0` in `poll_midi`):

```rust
/// Timestamp for an emitted LaneHit: the event's own stamp if it has one,
/// else the gameplay clock, else 0 (menus don't care about timing).
pub(crate) fn stamp_audio_ms(clock_ms: Option<i64>, event_ms: i64) -> i64 {
    if event_ms != 0 {
        event_ms
    } else {
        clock_ms.unwrap_or(0)
    }
}
```

(b) Relax `poll_midi` guards — delete these two early returns:

```rust
        if chart.chart.chips.is_empty() {
            return;
        }
        if !clock.is_ready() {
            return;
        }
```

and change the emit to:

```rust
            hits.write(LaneHit {
                lane,
                audio_ms: stamp_audio_ms(
                    clock.is_ready().then(|| clock.current_ms),
                    audio_ms,
                ),
            });
```

(`ActiveChart` may stay as a system param if other logic uses it; if not, drop the param.)

(c) Remove the `Performance` gating so MIDI works in menus. In `midi_consumer::plugin`:

```rust
        app.init_resource::<LastMidiHit>()
            .add_systems(FixedUpdate, poll_midi.in_set(super::DrumsSets::Input));

        #[cfg(feature = "midi")]
        {
            app.insert_non_send(MidiConnection::default())
                .add_systems(Startup, connect_midi)
                .add_systems(
                    Update,
                    connect_midi.run_if(resource_changed::<crate::bindings::LiveBindings>),
                )
                .add_systems(
                    FixedUpdate,
                    drain_real_midi
                        .in_set(super::DrumsSets::Input)
                        .before(poll_midi),
                );
        }
```

Check: `DrumsSets::Input` set may itself be configured with a Performance run condition on the whole set (grep `DrumsSets::Input` in lib.rs / orchestrator.rs). If the set is state-gated, schedule `poll_midi`/`drain_real_midi` in plain `FixedUpdate` with `.before(...)` ordering instead of the set.

(d) Write `MidiConnected` in `connect_midi` — after the existing `match` arms:

```rust
    fn connect_midi(
        mut conn: NonSendMut<MidiConnection>,
        live: Res<crate::bindings::LiveBindings>,
        mut connected: ResMut<game_shell::MidiConnected>,
    ) {
        // ... existing body ...
        // In the Ok arm add:  connected.0 = true;
        // In the Err arm add: connected.0 = false;
    }
```

(e) `LiveBindings` must exist at `Startup` — grep where `LiveBindings` is initialized (`init_resource` or `insert_resource`); if it only appears on Performance enter, move its load to `Startup` (it reads the persisted bindings file, safe at boot).

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: all PASS including `stamp_audio_ms` and existing midi tests.

- [ ] **Step 5: Manual smoke** — `cargo run --features midi` (or the project's normal run profile), sit on song select, hit a pad: no crash, log shows `MIDI connected:` at startup. (Nav won't do anything yet — consumers come later.)

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/lib.rs
git commit -m "feat(midi): poll MIDI in all states; expose MidiConnected"
```

---

### Task 4: Settings overlay — NavAction consumer with Rail/Rows/Adjust levels

**Files:**
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs` (keyboard emits NavAction; consumer owns all mutation)
- Modify: `crates/gameplay-drums/src/editor/ui.rs` (close path reusable by pads)

- [ ] **Step 1: Write the failing tests** — append to `keyboard_nav.rs` tests:

```rust
    use game_shell::{CustomizeTab, NavSource, NavVerb};

    #[test]
    fn pad_level_transitions() {
        // Rail --Confirm--> Rows --Confirm--> Adjust --Back--> Rows --Back--> Rail
        let mut lvl = NavLevel::Rail;
        lvl = next_level_on_confirm(lvl, CustomizeTab::Gameplay);
        assert!(matches!(lvl, NavLevel::Rows));
        lvl = next_level_on_confirm(lvl, CustomizeTab::Gameplay);
        assert!(matches!(lvl, NavLevel::Adjust));
        lvl = next_level_on_back(lvl);
        assert!(matches!(lvl, NavLevel::Rows));
        lvl = next_level_on_back(lvl);
        assert!(matches!(lvl, NavLevel::Rail));
    }

    #[test]
    fn excluded_tabs_refuse_row_entry() {
        assert!(matches!(
            next_level_on_confirm(NavLevel::Rail, CustomizeTab::Bindings),
            NavLevel::Rail
        ));
        assert!(matches!(
            next_level_on_confirm(NavLevel::Rail, CustomizeTab::Widgets),
            NavLevel::Rail
        ));
    }

    #[test]
    fn pad_verbs_in_adjust_mode_map_to_steps() {
        assert_eq!(adjust_delta(NavVerb::Up, NavSource::Pad), Some(-1));
        assert_eq!(adjust_delta(NavVerb::Down, NavSource::Pad), Some(1));
        assert_eq!(adjust_delta(NavVerb::Dec, NavSource::Keyboard), Some(-1));
        assert_eq!(adjust_delta(NavVerb::Inc, NavSource::Keyboard), Some(1));
        assert_eq!(adjust_delta(NavVerb::Confirm, NavSource::Pad), None);
    }
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p gameplay-drums keyboard_nav`
Expected: compile FAIL — `NavLevel`, `next_level_on_confirm`, `next_level_on_back`, `adjust_delta` undefined.

- [ ] **Step 3: Implement the level state + pure helpers** in `keyboard_nav.rs`:

```rust
use game_shell::{CustomizeTab, NavAction, NavSource, NavVerb};

/// Pad navigation level inside the Customize surface. Keyboard stays flat
/// (rows + direct Dec/Inc) and ignores this except for the focus ring.
#[derive(Resource, Default)]
pub enum NavLevel {
    /// HH/CY switch tabs, BD enters the tab, SD closes the overlay.
    #[default]
    Rail,
    /// HH/CY move row focus, BD enters adjust, SD returns to the rail.
    Rows,
    /// HH = −1, CY = +1, BD keeps the value, SD reverts to `saved`.
    Adjust {
        /// Full config snapshot taken on adjust-entry; SD restores it.
        saved: Box<dtx_config::Config>,
    },
}

/// Tabs whose CONTENT pads cannot navigate (pointer/capture surfaces).
pub fn pad_excluded(tab: CustomizeTab) -> bool {
    matches!(tab, CustomizeTab::Bindings | CustomizeTab::Widgets)
}

fn next_level_on_confirm(level: NavLevel, tab: CustomizeTab) -> NavLevel {
    match level {
        NavLevel::Rail if pad_excluded(tab) || !tab.is_settings() => NavLevel::Rail,
        NavLevel::Rail => NavLevel::Rows,
        NavLevel::Rows => NavLevel::Adjust {
            saved: Box::new(dtx_config::Config::default()), // caller overwrites with real draft
        },
        adjust @ NavLevel::Adjust { .. } => adjust,
    }
}

fn next_level_on_back(level: NavLevel) -> NavLevel {
    match level {
        NavLevel::Adjust { .. } => NavLevel::Rows,
        NavLevel::Rows => NavLevel::Rail,
        NavLevel::Rail => NavLevel::Rail,
    }
}

/// Delta a verb applies to the focused row, if any.
fn adjust_delta(verb: NavVerb, source: NavSource) -> Option<i32> {
    match (verb, source) {
        (NavVerb::Up, NavSource::Pad) | (NavVerb::Dec, _) => Some(-1),
        (NavVerb::Down, NavSource::Pad) | (NavVerb::Inc, _) => Some(1),
        _ => None,
    }
}
```

Note: `next_level_on_confirm` test uses `matches!(lvl, NavLevel::Adjust)` — write it as `NavLevel::Adjust { .. }` in the test if the compiler complains. The `Config::default()` placeholder inside the helper exists only so the pure function is testable; the system snapshot-overwrites it (Step 4). If `dtx_config::Config` doesn't impl `Default`, change the helper to return a marker (`NavLevel::Rows` → caller constructs `Adjust` itself) and adapt the test to assert on a `should_enter_adjust(level) -> bool` helper instead.

Also verify the non-settings tab `Lanes`: `CustomizeTab::Lanes.is_settings()` is false, so `Rail → Confirm` on Lanes stays at Rail in v1 (Lanes panel pad nav is not in the spec's settings grammar; the spec's "Lanes stays pad-navigable" applies to rail scrolling only).

- [ ] **Step 4: Split the keyboard system + add the consumer.** Replace `settings_keyboard_nav`'s arrow-handling with emission, and add one consumer that owns ALL mutation:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FocusedRow>()
        .init_resource::<NavLevel>()
        .add_systems(
            Update,
            (keyboard_emit_nav, settings_nav_consumer)
                .chain()
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        );
}

fn keyboard_emit_nav(
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<super::tabs::ActiveTab>,
    mut out: MessageWriter<NavAction>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        return; // perf hotkeys / save
    }
    // Tab switching stays a raw keyboard affordance (PageUp/PageDown).
    if keys.just_pressed(KeyCode::PageDown) {
        active.0 = active.0.next();
        return;
    } else if keys.just_pressed(KeyCode::PageUp) {
        active.0 = active.0.prev();
        return;
    }
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut emit = |verb| {
        out.write(NavAction { verb, source: NavSource::Keyboard, coarse });
    };
    if keys.just_pressed(KeyCode::ArrowDown) {
        emit(NavVerb::Down);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        emit(NavVerb::Up);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        emit(NavVerb::Inc);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        emit(NavVerb::Dec);
    }
}

#[allow(clippy::too_many_arguments)]
fn settings_nav_consumer(
    mut actions: MessageReader<NavAction>,
    mut active: ResMut<super::tabs::ActiveTab>,
    mut focused: ResMut<FocusedRow>,
    mut level: ResMut<NavLevel>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
    mut close: MessageWriter<super::EditorCloseRequest>,
) {
    if active.is_changed() {
        focused.0 = 0;
        *level = NavLevel::Rail;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for action in actions.read() {
        match action.source {
            // Keyboard: flat model, unchanged behavior.
            NavSource::Keyboard => {
                if !active.0.is_settings() || items.is_empty() {
                    continue;
                }
                let reps = if action.coarse { 10 } else { 1 };
                match action.verb {
                    NavVerb::Down => focused.0 = (focused.0 + 1).min(items.len() - 1),
                    NavVerb::Up => focused.0 = focused.0.saturating_sub(1),
                    v => {
                        if let (Some(delta), Some(item)) =
                            (adjust_delta(v, NavSource::Keyboard), items.get(focused.0))
                        {
                            for _ in 0..reps {
                                (item.adjust)(&mut draft.0, delta);
                            }
                        }
                    }
                }
            }
            // Pads: Rail / Rows / Adjust.
            NavSource::Pad => match &mut *level {
                NavLevel::Rail => match action.verb {
                    NavVerb::Up => active.0 = active.0.prev(),
                    NavVerb::Down => active.0 = active.0.next(),
                    NavVerb::Confirm => {
                        if active.0.is_settings() && !pad_excluded(active.0) && !items.is_empty() {
                            focused.0 = 0;
                            *level = NavLevel::Rows;
                        }
                    }
                    NavVerb::Back => {
                        close.write(super::EditorCloseRequest);
                    }
                    _ => {}
                },
                NavLevel::Rows => match action.verb {
                    NavVerb::Up => focused.0 = focused.0.saturating_sub(1),
                    NavVerb::Down => {
                        focused.0 = (focused.0 + 1).min(items.len().saturating_sub(1))
                    }
                    NavVerb::Confirm => {
                        *level = NavLevel::Adjust { saved: Box::new(draft.0.clone()) };
                    }
                    NavVerb::Back => *level = NavLevel::Rail,
                    _ => {}
                },
                NavLevel::Adjust { saved } => match action.verb {
                    NavVerb::Confirm => *level = NavLevel::Rows,
                    NavVerb::Back => {
                        draft.0 = (**saved).clone();
                        *level = NavLevel::Rows;
                    }
                    v => {
                        if let (Some(delta), Some(item)) =
                            (adjust_delta(v, NavSource::Pad), items.get(focused.0))
                        {
                            (item.adjust)(&mut draft.0, delta);
                        }
                    }
                },
            },
        }
    }
}
```

Note the tab-change reset (`active.is_changed()`) replicates the old behavior — it also fires when pads switch tabs, which is correct.

- [ ] **Step 5: `EditorCloseRequest`.** In `crates/gameplay-drums/src/editor/mod.rs` add:

```rust
/// Request to close the overlay through the same save-on-close path as Esc.
#[derive(Debug, Clone, Copy, Message)]
pub struct EditorCloseRequest;
```

Register `app.add_message::<EditorCloseRequest>()` in the editor plugin. Then in `ui.rs` `close_on_escape` (line ~214), extend the trigger: the system currently keys off `keys.just_pressed(KeyCode::Escape)`; add a `mut reqs: MessageReader<EditorCloseRequest>` param and treat `reqs.read().next().is_some()` as an Esc-equivalent (same widget-deselect ordering, same `save_draft_on_close` flow — do NOT duplicate the close logic).

- [ ] **Step 6: Run tests**

Run: `cargo test -p gameplay-drums`
Expected: new keyboard_nav tests PASS, existing tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/gameplay-drums/src/editor/keyboard_nav.rs crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/editor/ui.rs
git commit -m "feat(editor): pad Rail/Rows/Adjust navigation via NavAction"
```

---

### Task 5: Focus/adjust rings + bigger settings rows

**Files:**
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (`spawn_settings_block` ~line 664; sizing)
- Modify: `crates/gameplay-drums/src/editor/keyboard_nav.rs` (ring update system)

- [ ] **Step 1: Tag rows.** In `spawn_settings_block`, on the entity spawned per settings row (the row `Node` that contains label + control), insert a marker component with its index:

```rust
/// Index of a settings row inside the active tab, for focus-ring rendering.
#[derive(Component)]
pub struct SettingsRowIndex(pub usize);
```

(Declare it in `panel.rs`, `pub` so keyboard_nav can query it.) The row spawn loop already iterates `settings_items(tab)` with an index — insert `SettingsRowIndex(i)` there.

- [ ] **Step 2: Ring system.** In `keyboard_nav.rs`:

```rust
const FOCUS_RING: Color = Color::srgb(0.89, 0.20, 0.20);
const ADJUST_RING: Color = Color::srgb(0.16, 0.62, 0.36);

fn update_focus_rings(
    focused: Res<FocusedRow>,
    level: Res<NavLevel>,
    mut rows: Query<(&super::panel::SettingsRowIndex, &mut Outline)>,
) {
    for (row, mut outline) in &mut rows {
        if row.0 == focused.0 {
            outline.width = Val::Px(3.0);
            outline.color = match *level {
                NavLevel::Adjust { .. } => ADJUST_RING,
                _ => FOCUS_RING,
            };
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}
```

Add `Outline::default()` to the row bundle in `spawn_settings_block` so the query matches. Register the system in the same `Update` tuple as the consumer (after it): `(keyboard_emit_nav, settings_nav_consumer, update_focus_rings).chain()`.

- [ ] **Step 3: Bigger rows.** In `panel.rs`, locate the row/label/value/stepper style values inside `spawn_settings_block` and scale: row vertical padding → `Val::Px(14.0)` (from whatever small value is there), label + value font size → 18.0 (if currently ~13–14), stepper button padding → `UiRect::axes(Val::Px(14.0), Val::Px(6.0))`, row `margin.bottom` → `Val::Px(7.0)`. Keep group-label styling as is. If sizes come from shared `Theme` constants used by other surfaces, introduce local constants in panel.rs instead of editing the shared theme:

```rust
const ROW_PAD_V: f32 = 14.0;
const ROW_FONT: f32 = 18.0;
const STEP_PAD_H: f32 = 14.0;
```

- [ ] **Step 4: In adjust mode, stepper glyphs swap `<`/`>` → `−`/`+`.** The stepper buttons are spawned in `spawn_settings_block` and clicked via `handle_settings_adjust` (panel.rs:1267). Tag the two glyph `Text` entities per row (`StepperGlyph { row: usize, dir: i8 }` component) and add a small system beside `update_focus_rings` that sets the text to `−`/`+` when `level` is `Adjust` for that row and `<`/`>` otherwise.

- [ ] **Step 5: Build + visual check**

Run: `cargo build -p gameplay-drums` → clean.
Run the game, F2 → settings tab: rows visibly bigger, keyboard arrows move a red ring.

- [ ] **Step 6: Commit**

```bash
git add crates/gameplay-drums/src/editor/panel.rs crates/gameplay-drums/src/editor/keyboard_nav.rs
git commit -m "feat(editor): focus/adjust rings and larger settings rows"
```

---

### Task 6: Nav legend widget (dtx-ui) + editor legend + excluded-tab hint

**Files:**
- Create: `crates/dtx-ui/src/widget/nav_legend.rs`
- Modify: `crates/dtx-ui/src/widget/mod.rs` (or wherever sibling widgets are declared — match `song_wheel.rs`'s registration)
- Modify: `crates/gameplay-drums/src/editor/panel.rs` (legend at panel bottom, hint banner)

- [ ] **Step 1: Widget.** `crates/dtx-ui/src/widget/nav_legend.rs`:

```rust
//! Bottom legend bar showing pad verbs, e.g. "HH up · CY down · BD adjust".

use bevy::prelude::*;

/// Marker for legend bars so surfaces can despawn/rebuild them.
#[derive(Component)]
pub struct NavLegend;

/// Spawn a legend as the last child of `parent`.
/// `items` = [("HH", "up"), ("CY", "down"), ...].
pub fn spawn_nav_legend(parent: &mut ChildSpawnerCommands, items: &[(&str, &str)]) {
    parent
        .spawn((
            NavLegend,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .with_children(|bar| {
            for (pad, verb) in items {
                bar.spawn((
                    Text::new(format!("{pad} {verb}")),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
            }
        });
}
```

(`ChildSpawnerCommands` is the Bevy 0.19 child-builder type — confirm the exact name used by `with_children` closures elsewhere in this repo, e.g. in `pause.rs:142`, and match it.)

- [ ] **Step 2: Editor legend.** In `panel.rs`, after the tab content in the left panel root, spawn the legend when `game_shell::MidiConnected` is true. Legend text depends on `NavLevel`:
  - Rail: `[("HH","prev tab"),("CY","next tab"),("BD","enter"),("SD","close")]`
  - Rows: `[("HH","up"),("CY","down"),("BD","adjust"),("SD","tabs")]`
  - Adjust: `[("HH","−"),("CY","+"),("BD","confirm"),("SD","cancel")]`

Implement as: a system `update_editor_legend` (in panel.rs, registered in the editor plugin `Update` set, `run_if(super::editor_open)`) that despawns any `NavLegend` child of the panel and respawns it when `NavLevel` changed or `MidiConnected` changed. Rebuild-on-change is fine at this frequency.

- [ ] **Step 3: Excluded-tab hint.** In `rebuild_left_content` (panel.rs:176), for `CustomizeTab::Bindings` and `CustomizeTab::Widgets`, when `MidiConnected.0` is true prepend a hint row above the tab content:

```rust
    parent.spawn((
        Text::new("keyboard/mouse required — pads: SD to go back"),
        TextFont { font_size = 12.0, ..default() },
        TextColor(Color::srgba(1.0, 0.8, 0.3, 0.9)),
        Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
    ));
```

(Fix the struct-literal syntax to match the repo's `TextFont` usage.)

- [ ] **Step 4: Build + visual check**

Run: `cargo build --workspace`
Run game with MIDI device: legend visible at panel bottom; without device: absent.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-ui/src/widget/nav_legend.rs crates/dtx-ui/src/widget/mod.rs crates/gameplay-drums/src/editor/panel.rs
git commit -m "feat(ui): pad nav legend widget; editor legend + excluded-tab hint"
```

---

### Task 7: Song select + title pad navigation

**Files:**
- Modify: `crates/game-menu/src/song_select.rs` (`song_select_navigation` ~1207)
- Modify: `crates/game-menu/src/title.rs` (`title_input` ~114)

- [ ] **Step 1: Write the failing test** — pure wheel-level transition logic, in `song_select.rs` tests:

```rust
    #[test]
    fn pad_wheel_levels() {
        use game_shell::NavVerb;
        let mut lvl = PadWheelLevel::Wheel;
        lvl = lvl.on_verb(NavVerb::Confirm);
        assert_eq!(lvl, PadWheelLevel::Difficulty);
        lvl = lvl.on_verb(NavVerb::Back);
        assert_eq!(lvl, PadWheelLevel::Wheel);
        // Back at wheel level handled by consumer (exit to Title), level unchanged.
        assert_eq!(PadWheelLevel::Wheel.on_verb(NavVerb::Back), PadWheelLevel::Wheel);
    }
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p game-menu pad_wheel`
Expected: compile FAIL — `PadWheelLevel` undefined.

- [ ] **Step 3: Implement.**

```rust
/// Pad two-level song select: wheel (folders) then difficulty.
/// Keyboard stays flat and never touches this.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PadWheelLevel {
    #[default]
    Wheel,
    Difficulty,
}

impl PadWheelLevel {
    fn on_verb(self, verb: game_shell::NavVerb) -> Self {
        use game_shell::NavVerb;
        match (self, verb) {
            (PadWheelLevel::Wheel, NavVerb::Confirm) => PadWheelLevel::Difficulty,
            (PadWheelLevel::Difficulty, NavVerb::Back) => PadWheelLevel::Wheel,
            (lvl, _) => lvl,
        }
    }
}
```

`init_resource::<PadWheelLevel>()` in the song-select plugin; reset to `Wheel` in the `OnEnter(AppState::SongSelect)` spawn system.

- [ ] **Step 4: Split `song_select_navigation`.** Keep everything raw-key EXCEPT the six nav verbs, which now emit; add a consumer that performs them. The consumer contains the movement/start/back code moved verbatim out of the old system:

```rust
fn song_select_kb_emit(
    keys: Res<ButtonInput<KeyCode>>,
    mut out: MessageWriter<NavAction>,
) {
    use game_shell::{NavSource, NavVerb};
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut emit = |verb| {
        out.write(NavAction { verb, source: NavSource::Keyboard, coarse: false });
    };
    if keys.just_pressed(KeyCode::ArrowDown) {
        emit(NavVerb::Down);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        emit(NavVerb::Up);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        emit(NavVerb::Inc);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        emit(NavVerb::Dec);
    } else if keys.just_pressed(KeyCode::Enter) {
        emit(if shift { NavVerb::Practice } else { NavVerb::Confirm });
    } else if keys.just_pressed(KeyCode::Escape) {
        emit(NavVerb::Back);
    }
}
```

Consumer (replaces the arrow/Enter/Esc arms of `song_select_navigation`; Tab/F1/F5 stay in the old system, which loses its arrow/Enter/Esc arms):

```rust
#[allow(clippy::too_many_arguments)]
fn song_select_nav_consumer(
    mut actions: MessageReader<NavAction>,
    mut level: ResMut<PadWheelLevel>,
    mut db: ResMut<SongDb>,
    mut selection: ResMut<Selection>,
    mut selection_state: ResMut<SongSelectSelection>,
    mut selected_song: ResMut<SelectedSong>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_intent: ResMut<PracticeIntent>,
) {
    use game_shell::{NavSource, NavVerb};
    if selection_state.visible.is_empty() {
        return;
    }
    for action in actions.read() {
        // Effective axis: keyboard flat (Up/Down = folder, Dec/Inc = difficulty);
        // pads two-level (Up/Down = whichever level is active).
        let (folder_move, diff_move) = match (action.source, *level, action.verb) {
            (_, _, NavVerb::Dec) => (0, -1),
            (_, _, NavVerb::Inc) => (0, 1),
            (NavSource::Keyboard, _, NavVerb::Up) => (-1, 0),
            (NavSource::Keyboard, _, NavVerb::Down) => (1, 0),
            (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Up) => (-1, 0),
            (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Down) => (1, 0),
            (NavSource::Pad, PadWheelLevel::Difficulty, NavVerb::Up) => (0, -1),
            (NavSource::Pad, PadWheelLevel::Difficulty, NavVerb::Down) => (0, 1),
            _ => (0, 0),
        };
        if folder_move != 0 {
            let max = selection_state.visible.len() - 1;
            selection.folder = if folder_move > 0 {
                (selection.folder + 1).min(max)
            } else {
                selection.folder.saturating_sub(1)
            };
            selection.clamp_to_visible(&selection_state);
        }
        if diff_move != 0 {
            if diff_move > 0 {
                if let Some(folder) = selection_state.visible.get(selection.folder) {
                    let count = folder.difficulty_count();
                    if count > 0 {
                        selection.difficulty =
                            (selection.difficulty + 1).min((count - 1) as u8);
                    }
                }
            } else {
                selection.difficulty = selection.difficulty.saturating_sub(1);
            }
        }
        let start = |practice: bool,
                     selected_song: &mut SelectedSong,
                     practice_intent: &mut PracticeIntent,
                     requests: &mut MessageWriter<TransitionRequest>| {
            if let Some(chart_idx) = selection.chart_index(&selection_state)
                && let Some(song) = db.songs.get(chart_idx)
            {
                practice_intent.0 = practice;
                selected_song.0 = Some(song.path.clone());
                request_transition(requests, AppState::SongLoading);
            }
        };
        match (action.source, *level, action.verb) {
            (NavSource::Keyboard, _, NavVerb::Confirm)
            | (NavSource::Pad, PadWheelLevel::Difficulty, NavVerb::Confirm) => {
                start(false, &mut selected_song, &mut practice_intent, &mut requests);
            }
            (_, _, NavVerb::Practice)
                if action.source == NavSource::Keyboard
                    || *level == PadWheelLevel::Difficulty =>
            {
                start(true, &mut selected_song, &mut practice_intent, &mut requests);
            }
            (NavSource::Keyboard, _, NavVerb::Back)
            | (NavSource::Pad, PadWheelLevel::Wheel, NavVerb::Back) => {
                request_transition(&mut requests, AppState::Title);
            }
            _ => {}
        }
        if action.source == NavSource::Pad {
            *level = level.on_verb(action.verb);
        }
    }
    let _ = &mut db; // db used by `start` closure via capture; drop lint if unused otherwise
}
```

The closure-borrow shape above will fight the borrow checker (closure captures `selection`/`db` while params are borrowed) — if it does, inline the `start` body at both call sites instead of a closure. Correctness over cleverness; the inlined code is the exact Enter-branch body of the old `song_select_navigation`.

Register: `(song_select_kb_emit, song_select_nav_consumer).chain()` in `Update`, `run_if(in_state(AppState::SongSelect))`, replacing `song_select_navigation`'s registration (the residual raw system with Tab/F1/F5 keeps its old registration).

- [ ] **Step 5: Difficulty-level visual cue + legend.** When `PadWheelLevel::Difficulty`, the existing difficulty grid highlight already shows the cursor; add the legend bar (Task 6 widget) at the bottom of song select's root when `MidiConnected.0`:
  - Wheel: `[("HH","up"),("CY","down"),("BD","difficulty"),("SD","title")]`
  - Difficulty: `[("HH","prev diff"),("CY","next diff"),("BD","play"),("FT","practice"),("SD","songs")]`

Same despawn/respawn-on-change pattern as the editor legend, keyed on `PadWheelLevel` + `MidiConnected`.

- [ ] **Step 6: Title.** In `title_input`, add `mut actions: MessageReader<NavAction>` and treat any `NavVerb::Confirm` action as the existing Enter branch (extract the Enter-branch body into a local fn or just `let confirm = keys.just_pressed(KeyCode::Enter) || actions.read().any(|a| a.verb == game_shell::NavVerb::Confirm);`).

- [ ] **Step 7: Run tests**

Run: `cargo test -p game-menu`
Expected: PASS including `pad_wheel_levels`.

- [ ] **Step 8: Commit**

```bash
git add crates/game-menu/src/song_select.rs crates/game-menu/src/title.rs
git commit -m "feat(menu): pad navigation for song select and title"
```

---

### Task 8: Pause menu + results via NavAction

**Files:**
- Modify: `crates/gameplay-drums/src/pause.rs` (`pause_menu_input` ~170)
- Modify: `crates/game-results/src/lib.rs` (`result_input` ~270)

- [ ] **Step 1: Pause.** Split like the others. `pause_kb_emit` (new, same `run_if(in_state(PauseState::Paused))`):

```rust
fn pause_kb_emit(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<NavAction>) {
    use game_shell::{NavAction, NavSource, NavVerb};
    let mut emit = |verb| {
        out.write(NavAction { verb, source: NavSource::Keyboard, coarse: false });
    };
    if keys.just_pressed(KeyCode::ArrowDown) {
        emit(NavVerb::Down);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        emit(NavVerb::Up);
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        emit(NavVerb::Confirm);
    }
}
```

`pause_menu_input` becomes the consumer: replace its `keys` param with `mut actions: MessageReader<NavAction>`, and its three key checks with:

```rust
    for action in actions.read() {
        match action.verb {
            game_shell::NavVerb::Down => selection.0 = (selection.0 + 1) % count,
            game_shell::NavVerb::Up => selection.0 = (selection.0 + count - 1) % count,
            game_shell::NavVerb::Confirm => { /* existing match on selected */ }
            game_shell::NavVerb::Back => next_pause.set(PauseState::Running), // SD resumes
            _ => {}
        }
    }
```

(Keep the row-recolor block outside the loop, exactly as today. `toggle_pause`'s Esc handling is untouched — kb Esc still toggles; pad SD resumes via `Back`.)

Register `(pause_kb_emit, pause_menu_input).chain()` under the existing `run_if(in_state(PauseState::Paused))`.

- [ ] **Step 2: Results.** `result_input` gains NavAction:

```rust
fn result_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<NavAction>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    use game_shell::NavVerb;
    let pad = actions
        .read()
        .any(|a| matches!(a.verb, NavVerb::Confirm | NavVerb::Back));
    if pad || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}
```

(Keyboard stays raw here — two keys, no selection state; routing them through NavAction adds nothing. The mapper's 500 ms grace already protects against the last drum note skipping results.)

- [ ] **Step 3: Legend bars.** Pause overlay (`spawn_overlay` in pause.rs) and results (`spawn_result` in game-results) get the Task 6 legend as a bottom child when `MidiConnected.0`:
  - Pause: `[("HH","up"),("CY","down"),("BD","select"),("SD","resume")]`
  - Results: `[("BD","continue")]`

Both spawn once (these overlays are static) — pass `Res<MidiConnected>` into the spawn systems.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gameplay-drums && cargo test -p game-results`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gameplay-drums/src/pause.rs crates/game-results/src/lib.rs
git commit -m "feat(nav): pause and results consume NavAction; pad legends"
```

---

### Task 9: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Workspace tests** (includes the FixedUpdate ordering/schedule guard — required, green unit tests alone don't prove the schedule builds):

Run: `cargo test --workspace`
Expected: all PASS.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets`
Expected: no new warnings.

- [ ] **Step 3: Manual BRP session** (see memory note "brp drive customize" — launch `dtxmaniars` via `mcp__bevy-brp__brp_launch`, drive with screenshots). MIDI hits can be simulated by pushing into `VirtualSource` via BRP if no device attached. Checklist:
  1. Title: BD → song select.
  2. Song select: HH/CY scroll folders; BD → difficulty; HH/CY cycle difficulty; SD → back to wheel; BD → song starts. Legend text updates per level.
  3. Last note of a song → results NOT skipped (grace works).
  4. Results: BD → song select.
  5. Pause: Esc → HH/CY move selection, SD resumes.
  6. F2 overlay: rail HH/CY tab-cycle; BD into Gameplay; CY to Input Offset; BD adjust (green ring, − / + glyphs); CY ×3 → value +3 live; SD reverts; BD keeps; SD SD → rail → overlay closes and saves.
  7. Bindings tab focused via pads: hint banner shows, BD does nothing, SD returns.
  8. Arm binding capture with keyboard → pad hits do NOT navigate. Same during calibration overlay.
  9. Unplug/no MIDI: no legends anywhere, keyboard/mouse identical to pre-change behavior.

- [ ] **Step 4: Final commit** (any fixups), then update the layout-editor/customize memory files if editor surface behavior notes changed.

---

## Self-Review Notes (already applied)

- Spec coverage: NavAction layer (T1–T2), MIDI-in-menus prerequisite the spec implies (T3), settings grammar + rings + big rows (T4–T5), legends + excluded tabs (T6), song select/title (T7), pause/results (T8), edge cases: velocity (free via poll_midi), debounce+grace (T2), capture/calibration suspension (T2), no-MIDI hidden legends (T6–T8), tests incl. schedule guard (T9).
- Known deliberate deviation: results/title keyboard keys stay raw (two stateless keys); spec's "consumers consume only NavAction" is honored for all stateful navigation.
- `Lanes` tab: pad-reachable on the rail but its panel content is not row-navigable in v1 (it's not a `settings_items` tab); spec's grammar only covers settings rows.
