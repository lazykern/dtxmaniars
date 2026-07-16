# PR1 completion + PR2 application-owned router — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Repo caveats:** never run bare `cargo fmt --all` (rustfmt version drift — format only files you touched via `rustfmt --edition 2021 <files>` or rely on `cargo fmt -- --check` for verification). Serialize Bevy-heavy builds. Subagents must run cargo inline and wait (no fire-and-forget). Package tests only, never full-workspace test.

**Goal:** Land PR1 (SystemVerb envelope sweep, already in working tree) and PR2 (game-shell context stack + centralized router + last-intentional-source), per `docs/superpowers/specs/2026-07-16-unified-input-settings-program-design.md`.

**Architecture:** `dtx-input` emits `SystemVerbHit { verb, source }` from profile bindings (keyboard translator + MIDI pump). `game-shell` owns a `NavContextStack`; a single router system gates hits by context scope, applies 80 ms debounce / 500 ms entry grace to MIDI, translates Left/Right→Decrease/Increase in edit contexts, stamps `coarse`, updates `LastIntentionalInputSource`, and emits `NavAction`. Screens stop reading raw keys for navigation and only push/pop contexts + consume `NavAction`.

**Tech Stack:** Rust (edition 2021), Bevy 0.19 messages/resources/states, toml-serialized profiles.

**Baseline:** main at `1a863ec`; PR1 sweep is uncommitted in the working tree and compiles except final verification.

---

### Task 1: Verify and commit the PR1 sweep

The working tree already contains: NavVerb→SystemVerb rename in all consumers, Practice-verb removal (Shift+Enter → coarse Confirm; FT → NextTab), `verb_for_lane` returning `SystemVerb`, updated tests.

**Files (already modified, verify only):**
- `crates/game-shell/src/navigation.rs`, `crates/game-shell/src/lib.rs`
- `crates/game-menu/src/{song_select,song_ready,song_loading,home}.rs`
- `crates/gameplay-drums/src/{menu_nav,pause}.rs`, `crates/gameplay-drums/src/editor/*.rs`, `crates/gameplay-drums/src/practice/hud/setup_controls.rs`
- `crates/game-results/src/input.rs`
- `crates/gameplay-drums/tests/{practice_mode,practice_hud}.rs`
- `crates/dtx-input/src/bindings.rs` (FT→NextTab default note addition)

- [ ] **Step 1: Compile everything swept**

Run: `cargo check -p dtx-input -p game-shell -p game-menu -p gameplay-drums -p game-results --all-targets 2>&1 | grep -E '^error' -A6`
Expected: no output (exit 0). Fix any residual errors — they will be missing `_ =>` catch-alls on now-16-variant `SystemVerb` matches, or duplicate `SystemVerb` imports (`game_shell::SystemVerb` re-exports `dtx_input::SystemVerb`; keep one).

- [ ] **Step 2: Run package tests**

Run, serially:
```sh
cargo test -p dtx-input --lib
cargo test -p game-shell --lib
cargo test -p game-menu --lib
cargo test -p game-results --lib
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test practice_mode
cargo test -p gameplay-drums --test practice_hud
cargo test -p gameplay-drums --test bindings_lane_pipeline
cargo check -p dtxmaniars-desktop
```
Expected: all pass. Known intentional behavior changes if a test still asserts them: FT pad no longer emits Practice anywhere; Song Select pad FT no longer opens Practice-mode Ready (Song Ready's internal mode toggle covers it); Results FT no longer jumps to practice.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all -- --check` — if only files you touched fail, format those files individually with `rustfmt --edition 2021 <file>`; do not reformat unrelated files.

- [ ] **Step 4: Commit**

```bash
git add crates/game-shell crates/game-menu crates/gameplay-drums crates/game-results crates/dtx-input
git commit -m "refactor(nav): NavAction carries SystemVerb; retire Practice verb

NavVerb is gone: the UI action envelope now carries the canonical
dtx_input::SystemVerb (Up/Down/Dec/Inc become NavigateUp/NavigateDown/
Decrease/Increase). Practice is no longer a shared semantic action:
Song Select's Shift+Enter accelerator rides Confirm with coarse=true,
Song Ready keeps its internal mode toggle, Results keeps Practice as a
verb-row choice, and the FT pad now maps to NextTab (still toggles the
Practice Setup surface focus / moves tabs)."
```

---

### Task 2: `SystemVerbHit` gains a device source

The router must know keyboard vs MIDI (debounce policy + source tracking).

**Files:**
- Modify: `crates/dtx-input/src/pump.rs` (message def + `poll_midi` + `consume_midi_events` tests)
- Modify: `crates/dtx-input/src/keyboard.rs` (`keyboard_system_verbs`)

- [ ] **Step 1: Write failing test** (in `pump.rs` tests)

```rust
#[test]
fn verb_hits_carry_their_device_source() {
    use crate::{BindSource, InputBindings, SystemVerb};
    let mut b = InputBindings::default();
    b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
    let resolver = BindResolver::from_bindings(&b);
    let mut last = LastMidiHit::default();
    let out = consume_midi_events(
        [crate::midi::MidiEvent::NoteOn { note: 37, velocity: 90, audio_ms: 0, captured_at: Instant::now() }],
        &resolver,
        &mut last,
    );
    assert_eq!(out.verbs, vec![(SystemVerb::Pause, VerbSource::Midi)]);
}
```

- [ ] **Step 2: Run** `cargo test -p dtx-input --lib pump` — expect FAIL (no `VerbSource`).

- [ ] **Step 3: Implement**

In `pump.rs`:
```rust
/// Which device fired a bound system verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbSource {
    /// Physical keyboard key.
    Keyboard,
    /// MIDI note (pad/zone).
    Midi,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemVerbHit {
    /// The verb that fired.
    pub verb: SystemVerb,
    /// The device that fired it.
    pub source: VerbSource,
}
```
`ConsumedMidi.verbs` becomes `Vec<(SystemVerb, VerbSource)>`; `consume_midi_events` pushes `(verb, VerbSource::Midi)`; `poll_midi` writes `SystemVerbHit { verb, source }`. In `keyboard.rs`, `keyboard_system_verbs` writes `SystemVerbHit { verb, source: VerbSource::Keyboard }`. Export `VerbSource` from `lib.rs` (`pub use pump::{.., VerbSource}`).

- [ ] **Step 4: Fix compile fallout** — grep `SystemVerbHit {` across `crates/` (pause.rs consumers construct/match it in tests) and add the `source` field. Run `cargo check -p dtx-input -p gameplay-drums --all-targets`.

- [ ] **Step 5: Run** `cargo test -p dtx-input --lib && cargo test -p gameplay-drums --lib` — expect PASS.

- [ ] **Step 6: Commit** `git commit -am "feat(input): SystemVerbHit carries its device source"`

---

### Task 3: Navigation module split + context stack

**Files:**
- Create: `crates/game-shell/src/navigation/mod.rs` (moves current `navigation.rs` content; keeps `NavGuard`, plugin, transitional pad mapper)
- Create: `crates/game-shell/src/navigation/context.rs`
- Create: `crates/game-shell/src/navigation/source.rs`
- Delete: `crates/game-shell/src/navigation.rs` (content redistributed)

- [ ] **Step 1: Write failing tests** (`context.rs`)

```rust
#[test]
fn top_of_stack_owns_input() {
    let mut s = NavContextStack::default();
    assert_eq!(s.top(), None);
    s.push(NavContext::Home);
    s.push(NavContext::ModalDialog);
    assert_eq!(s.top(), Some(NavContext::ModalDialog));
    s.pop(NavContext::ModalDialog);
    assert_eq!(s.top(), Some(NavContext::Home));
}

#[test]
fn pop_removes_the_named_context_even_if_not_top() {
    // Screens pop in OnExit; ordering vs overlays must not corrupt the stack.
    let mut s = NavContextStack::default();
    s.push(NavContext::SongSelectSongs);
    s.push(NavContext::ModalDialog);
    s.pop(NavContext::SongSelectSongs);
    assert_eq!(s.top(), Some(NavContext::ModalDialog));
}

#[test]
fn push_is_idempotent_per_context() {
    let mut s = NavContextStack::default();
    s.push(NavContext::Home);
    s.push(NavContext::Home);
    s.pop(NavContext::Home);
    assert_eq!(s.top(), None);
}
```

- [ ] **Step 2: Run** `cargo test -p game-shell --lib navigation::context` — FAIL.

- [ ] **Step 3: Implement**

```rust
/// Which UI surface owns semantic input. Top of the stack wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavContext {
    Home,
    SongSelectSongs,
    SongSelectDifficulty,
    SongReadyBrowse,
    SongReadyEdit,
    SongLoading,
    PracticeSetupSettings,
    PracticeSetupPreview,
    PauseMenu,
    Results,
    SettingsTabs,
    SettingsRows,
    SettingsEdit,
    ModalDialog,
    BindingCapture,
    LayoutEditor,
    LiveGameplay,
}

impl NavContext {
    /// Edit-type contexts translate NavigateLeft/Right into Decrease/Increase.
    pub fn is_edit(self) -> bool {
        matches!(self, NavContext::SongReadyEdit | NavContext::SettingsEdit
            | NavContext::PracticeSetupSettings)
    }
    /// Contexts that own raw input exclusively: no menu routing at all.
    pub fn exclusive(self) -> bool {
        matches!(self, NavContext::BindingCapture)
    }
}

/// Stack of active contexts; screens push in OnEnter/overlay-open and pop in
/// OnExit/overlay-close. `push` moves an already-present context to the top.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct NavContextStack(Vec<NavContext>);

impl NavContextStack {
    pub fn top(&self) -> Option<NavContext> { self.0.last().copied() }
    pub fn push(&mut self, ctx: NavContext) {
        self.0.retain(|c| *c != ctx);
        self.0.push(ctx);
    }
    pub fn pop(&mut self, ctx: NavContext) { self.0.retain(|c| *c != ctx); }
    pub fn clear(&mut self) { self.0.clear(); }
}
```
Old 7-variant `NavContext` (Title/SongSelect/Result/Paused/Editor/Loading/PracticeSetup) is deleted; `NavGuard`/`ActiveNavContext` keep compiling by switching to the new enum with mapping: Title→Home, SongSelect→SongSelectSongs, Result→Results, Paused→PauseMenu, Editor→LayoutEditor, Loading→SongLoading, PracticeSetup→PracticeSetupSettings. Update `gameplay-drums/src/menu_nav.rs` publisher and all `NavContext::X` references accordingly (grep `NavContext::`).

- [ ] **Step 4: source.rs**

```rust
/// Which device produced an intentional action. Replaces NavSource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource { Keyboard, Mouse, MidiKit, Gamepad }

/// Last accepted intentional input source. Plain resource: survives every
/// AppState transition. Pointer motion must never write it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastIntentionalInputSource(pub InputSource);
impl Default for LastIntentionalInputSource {
    fn default() -> Self { Self(InputSource::Keyboard) }
}

/// Screens report intentional mouse interactions (click/wheel/drag).
#[derive(Message, Debug, Clone, Copy)]
pub struct MouseIntent;

/// Accessibility: lock prompts to one source, or follow the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptSourcePreference {
    #[default]
    Automatic,
    Always(InputSource),
}
```
`PromptSourcePreference` persistence lands with the Settings draft (PR6); for now it is a plain resource registered by the plugin.

- [ ] **Step 5:** `mod.rs` re-exports everything old callers used: `pub use context::{NavContext, NavContextStack}; pub use source::{InputSource, LastIntentionalInputSource, MouseIntent, PromptSourcePreference};` plus existing `NavAction`, `NavGuard`, `NavSource` (transitional), `SystemVerb`, `MidiConnected`, `NavMapSet`, `ActiveNavContext`, `plugin`.

- [ ] **Step 6: Run** `cargo test -p game-shell --lib && cargo check -p game-menu -p gameplay-drums -p game-results --all-targets` — PASS.

- [ ] **Step 7: Commit** `git commit -am "refactor(shell): navigation module split; NavContextStack + InputSource"`

---

### Task 4: The router

**Files:**
- Create: `crates/game-shell/src/navigation/router.rs`
- Modify: `crates/game-shell/src/navigation/mod.rs` (plugin wiring; NavAction gains `repeated`)

- [ ] **Step 1: Failing tests** — pure reducer first:

```rust
/// Everything the router needs to decide one hit's fate. Pure.
#[test]
fn menu_verb_routes_only_when_a_menu_context_owns_input() {
    let mut g = NavGuard::default();
    let now = Instant::now();
    // LiveGameplay on top: menu verbs die, live verbs pass.
    assert_eq!(route(Some(NavContext::LiveGameplay), SystemVerb::NavigateUp,
        VerbSource::Midi, false, &mut g, now), Routed::Dropped);
    assert_eq!(route(Some(NavContext::LiveGameplay), SystemVerb::Pause,
        VerbSource::Midi, false, &mut g, now), Routed::Live(SystemVerb::Pause));
    // No context at all: everything menu-scope dies.
    assert_eq!(route(None, SystemVerb::Confirm, VerbSource::Keyboard, false,
        &mut g, now), Routed::Dropped);
}

#[test]
fn edit_context_translates_horizontal_navigation_to_adjustment() {
    let mut g = NavGuard::default();
    let now = Instant::now();
    let r = route(Some(NavContext::SongReadyEdit), SystemVerb::NavigateLeft,
        VerbSource::Keyboard, false, &mut g, now);
    assert_eq!(r, Routed::Menu(NavAction { verb: SystemVerb::Decrease,
        source: InputSource::Keyboard, coarse: false, repeated: false }));
}

#[test]
fn midi_hits_respect_grace_and_debounce_keyboard_does_not() {
    let mut g = NavGuard::default();
    let t0 = Instant::now();
    // Keyboard immediately after context entry: allowed.
    assert!(matches!(route(Some(NavContext::Home), SystemVerb::Confirm,
        VerbSource::Keyboard, false, &mut g, t0), Routed::Menu(_)));
    // MIDI inside the 500 ms grace: dropped.
    assert_eq!(route(Some(NavContext::Home), SystemVerb::Confirm,
        VerbSource::Midi, false, &mut g, t0), Routed::Dropped);
    let t1 = t0 + Duration::from_millis(600);
    assert!(matches!(route(Some(NavContext::Home), SystemVerb::Confirm,
        VerbSource::Midi, false, &mut g, t1), Routed::Menu(_)));
    // 40 ms later: debounced.
    assert_eq!(route(Some(NavContext::Home), SystemVerb::Confirm,
        VerbSource::Midi, false, &mut g, t1 + Duration::from_millis(40)),
        Routed::Dropped);
}

#[test]
fn exclusive_context_swallows_everything_menu() {
    let mut g = NavGuard::default();
    let now = Instant::now();
    assert_eq!(route(Some(NavContext::BindingCapture), SystemVerb::Back,
        VerbSource::Keyboard, false, &mut g, now), Routed::Dropped);
    // Live verbs still pass (pause key must work during capture? No —
    // capture owns raw input; RawInputOwned already silences the keyboard
    // translator, and a MIDI live verb mid-capture would fight the monitor):
    assert_eq!(route(Some(NavContext::BindingCapture), SystemVerb::Pause,
        VerbSource::Midi, false, &mut g, now), Routed::Dropped);
}

#[test]
fn shift_tab_becomes_previous_tab() {
    let mut g = NavGuard::default();
    let now = Instant::now();
    let r = route(Some(NavContext::SettingsTabs), SystemVerb::NextTab,
        VerbSource::Keyboard, true, &mut g, now);
    assert_eq!(r, Routed::Menu(NavAction { verb: SystemVerb::PreviousTab,
        source: InputSource::Keyboard, coarse: true, repeated: false }));
}
```

- [ ] **Step 2: Run** — FAIL (no `route`/`Routed`).

- [ ] **Step 3: Implement `router.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    Menu(NavAction),
    Live(SystemVerb),
    Dropped,
}

pub fn route(
    top: Option<NavContext>,
    verb: SystemVerb,
    source: VerbSource,
    coarse: bool,
    guard: &mut NavGuard,
    now: Instant,
) -> Routed {
    use dtx_input::bindings::VerbScope;
    if verb.activation_scope() == VerbScope::LiveSystem {
        // Live verbs bypass menu contexts, except exclusive capture.
        return match top {
            Some(ctx) if ctx.exclusive() => Routed::Dropped,
            _ => Routed::Live(verb),
        };
    }
    let Some(ctx) = top else { return Routed::Dropped };
    if ctx.exclusive() || ctx == NavContext::LiveGameplay {
        return Routed::Dropped;
    }
    if source == VerbSource::Midi && !guard.accept(now) {
        return Routed::Dropped;
    }
    let verb = match (ctx.is_edit(), verb, coarse) {
        (true, SystemVerb::NavigateLeft, _) => SystemVerb::Decrease,
        (true, SystemVerb::NavigateRight, _) => SystemVerb::Increase,
        (_, SystemVerb::NextTab, true) => SystemVerb::PreviousTab,
        (_, v, _) => v,
    };
    Routed::Menu(NavAction {
        verb,
        source: match source {
            VerbSource::Keyboard => InputSource::Keyboard,
            VerbSource::Midi => InputSource::MidiKit,
        },
        coarse,
        repeated: false, // key-repeat modeling deferred; always an initial press
    })
}
```
`NavGuard` gets `pub fn sync(&mut self, top: Option<NavContext>, now: Instant)` — calls `enter_context`/`clear_context` from the old API using the new enum. The Bevy system:

```rust
fn route_verbs(
    stack: Res<NavContextStack>,
    keys: Res<ButtonInput<KeyCode>>,
    mut guard: ResMut<NavGuard>,
    mut hits: MessageReader<dtx_input::SystemVerbHit>,
    mut last_source: ResMut<LastIntentionalInputSource>,
    mut mouse: MessageReader<MouseIntent>,
    mut menu_out: MessageWriter<NavAction>,
    mut live_out: MessageWriter<LiveVerb>,   // new message wrapping live verbs
) {
    let now = Instant::now();
    guard.sync(stack.top(), now);
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for hit in hits.read() {
        match route(stack.top(), hit.verb, hit.source, coarse, &mut guard, now) {
            Routed::Menu(action) => {
                last_source.0 = action.source;
                menu_out.write(action);
            }
            Routed::Live(verb) => {
                last_source.0 = match hit.source {
                    dtx_input::VerbSource::Keyboard => InputSource::Keyboard,
                    dtx_input::VerbSource::Midi => InputSource::MidiKit,
                };
                live_out.write(LiveVerb(verb));
            }
            Routed::Dropped => {}
        }
    }
    if mouse.read().next().is_some() {
        last_source.0 = InputSource::Mouse;
    }
}
```
`LiveVerb(pub SystemVerb)` is a game-shell message; gameplay-drums' pause/restart consumers switch from `SystemVerbHit` to `LiveVerb` (they keep their own state gating). `NavAction` gains `repeated: bool` — update every constructor (grep `NavAction {`).

- [ ] **Step 4: Wire plugin** — `plugin(app)` adds `route_verbs` in `Update` in a `NavRouterSet`, after dtx-input's pump ordering is not required (messages cross frames safely; keep `pad_nav_mapper` for now, it will be deleted in Task 6). Register `NavContextStack`, `LastIntentionalInputSource`, `PromptSourcePreference`, messages `MouseIntent`, `LiveVerb`.

- [ ] **Step 5: Run** `cargo test -p game-shell --lib` — PASS. `cargo check -p gameplay-drums -p game-menu -p game-results --all-targets`.

- [ ] **Step 6: Commit** `git commit -am "feat(shell): centralized semantic router with context stack"`

---

### Task 5: `keyboard_system_verbs` runs globally; keyboard defaults drive menus

**Files:**
- Modify: `crates/gameplay-drums/src/bindings.rs` (or wherever `keyboard_system_verbs` is scheduled — grep `keyboard_system_verbs`): remove the Performance-only run condition; run in `Update` always, still gated by `RawInputOwned`.
- Modify: `crates/gameplay-drums/src/pause.rs`: pause/restart consumers read `game_shell::LiveVerb` instead of `SystemVerbHit`.

- [ ] **Step 1:** Failing App test in `game-shell` (or adjust pause.rs unit tests): pressing bound Pause key with `PauseState::Running` still toggles pause via `LiveVerb` path.
- [ ] **Step 2:** Implement; keep velocity threshold/debounce semantics untouched (MIDI-side unchanged).
- [ ] **Step 3:** `cargo test -p gameplay-drums --lib && cargo test -p gameplay-drums --test practice_mode` — PASS.
- [ ] **Step 4:** Commit `git commit -am "refactor(input): keyboard verb translator runs globally; live verbs via router"`.

---

### Task 6: Migrate screens off raw keyboard navigation

One sub-task per screen; identical recipe: (a) `OnEnter`/`OnExit` push/pop the right `NavContext`s (replacing the `ActiveNavContext` publisher writes in `gameplay-drums/src/menu_nav.rs` and screen-local equivalents); (b) delete the screen's keyboard-emit system; (c) screen keeps consuming `NavAction`, now sourced from the router; (d) screen-local quirks stay screen-side; (e) every screen system that reacts to a click, wheel, or drag also writes one `MouseIntent` message (pointer-motion/hover systems must not).

Order (each ends with its package tests + a commit):

- [ ] **6a. Home** (`game-menu/src/home.rs`): push `NavContext::Home` OnEnter(Title). Delete home's raw arrow/Enter handling in favor of `NavAction` (it already consumes NavAction from pads; keyboard now arrives the same way). Commit `feat(menu): home navigates via router`.
- [ ] **6b. Song Select** (`song_select.rs`): push `SongSelectSongs`/`SongSelectDifficulty` as `SongSelectFocus` changes (system observing focus resource writes the stack). Delete `song_select_kb_emit` EXCEPT the two screen-local quirks, which become tiny raw-key systems that do NOT emit NavAction: (1) Esc-clears-search-first: a system that, when search is non-empty and Escape just pressed, clears the query and sets a one-frame `SearchEscConsumed` resource the NavAction consumer checks to skip one Back; (2) Shift+Enter accelerator: coarse Confirm already arrives via router (`coarse` stamped from Shift state). Tab sort-mode cycling moves to `NextTab` NavAction arm. Commit.
- [ ] **6c. Song Ready** (`song_ready.rs`): push `SongReadyBrowse` when layer != Closed, `SongReadyEdit` in edit layer (pop both on close). Router's edit translation now supplies Decrease/Increase from ←/→ and HT/LT; delete keyboard-vs-pad match arms that duplicate each other — consumers match on verb only, keeping pad-specific Up/Down-adjust behavior by matching `source == InputSource::MidiKit` where the approved UX differs. Commit.
- [ ] **6d. Song Loading** (`song_loading.rs`): push `SongLoading`; Back cancels (grace protects the confirming hit — covered by router test). Delete its raw Esc handling. Commit.
- [ ] **6e. Pause** (`pause.rs`): push `PauseMenu` while overlay open (normal + practice pause). Delete `pause_kb_emit`. Commit.
- [ ] **6f. Practice Setup** (`practice/hud/setup_controls.rs`): push `PracticeSetupSettings`/`PracticeSetupPreview` per `PracticeSurfaceFocus`/`PracticeTab`. Delete the raw-key `keyboard_actions` system; extend `nav_actions` (the existing NavAction→PracticeUiAction reducer) to cover the keyboard paths it loses: Tab→`NextTab` arm already toggles focus/moves tab; Space-as-play stays raw (text-entry-adjacent transport accelerator, screen-local). Run `cargo test -p gameplay-drums --test practice_hud` — the keyboard/pad parity tests are the acceptance gate. Commit.
- [ ] **6g. Results** (`game-results/src/input.rs`): push `Results`. Delete the raw-key folds (←/→/Enter/Esc) — router delivers NavigateLeft/Right which `reduce_result_nav` maps like Up/Down (add arms `SystemVerb::NavigateLeft => prev, NavigateRight => next`). `R`-retry stays raw (screen-local accelerator). Commit.
- [ ] **6h. Modal dialogs** (`editor/close_dialog.rs` + any `ModalDialog` surfaces): push `ModalDialog` while open. Commit.
- [ ] **6i. Layout editor** (`editor/keyboard_nav.rs`): push `LayoutEditor`; keep its local NavAction producer for editor-specific keys but delete overlapping arrow/Enter/Esc emission (router covers those). `BindingCapture` context pushed by `bindings_capture.rs` while capturing (alongside existing `RawInputOwned`). Commit.

Each sub-task's verification: the crate's `--lib` tests plus `cargo test -p game-shell --test all_stages_reachable` after 6a–6e.

---

### Task 7: Delete transitional pad path

**Files:**
- Modify: `crates/game-shell/src/navigation/mod.rs`: delete `pad_nav_mapper`, `verb_for_lane`, `ActiveNavContext`, `NavMapSet`, `NavSource` (grep-fix remaining references — `NavAction.source` is `InputSource` everywhere after Task 6).
- Modify: `crates/dtx-input/src/pump.rs`: stop emitting `PadNavHit`; delete the message + `nav_lanes` plumbing (menu navigation now flows exclusively through lane-shared menu-verb bindings → `SystemVerbHit`).
- Modify: `crates/gameplay-drums/src/menu_nav.rs`: reduce to gameplay-fact context pushes (LiveGameplay while judging active, PauseMenu, PracticeSetup*, SongLoading, LayoutEditor) — it writes `NavContextStack` directly (Game crate, allowed) in a system ordered before `NavRouterSet`.

- [ ] **Step 1:** Failing test: `grep -rn "PadNavHit" crates --include=*.rs` returns nothing after removal; functional test — App test in game-shell: virtual MIDI note 42 (HH, lane-shared NavigateUp) with `NavContextStack=[Home]` produces `NavAction{verb: NavigateUp, source: MidiKit}` after grace; same note with `[LiveGameplay]` produces none.
- [ ] **Step 2:** Implement deletions; run full swept-crate check + tests (same list as Task 1 Step 2 plus `cargo test -p game-shell --test all_stages_reachable`).
- [ ] **Step 3:** Commit `git commit -am "refactor(input): retire PadNavHit and hard-coded lane-to-verb mapping"`.

---

### Task 8: PR2 gates + memory note

- [ ] **Step 1:** Run the full changed-package gate:
```sh
cargo fmt --all -- --check
cargo test -p dtx-input --lib
cargo check -p dtx-input --features midi
cargo test -p game-shell --lib
cargo test -p game-shell --test all_stages_reachable
cargo test -p game-menu --lib
cargo test -p game-results --lib
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test practice_mode
cargo test -p gameplay-drums --test practice_hud
cargo test -p gameplay-drums --test bindings_lane_pipeline
cargo check -p dtxmaniars-desktop
cargo clippy --workspace --all-targets -- -D warnings
```
- [ ] **Step 2:** Manual smoke via BRP (see memory `brp-drive-customize`): launch desktop, keyboard-only walk Home→SongSelect→Ready→play→pause→results→home; verify arrows/Enter/Esc all work with `MidiConnected(false)`.
- [ ] **Step 3:** Record PR2 handoff notes (files changed, test results, deviations) in the PR description / session notes.
