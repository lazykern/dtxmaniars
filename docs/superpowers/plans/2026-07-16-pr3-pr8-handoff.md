# Handoff: unified input/settings program — PR3 through PR8

> Audience: an agent with **zero prior context**. Everything you need is in this
> file plus the documents it links. Read this top to bottom before coding.

## 0. Orientation (read these first, in order)

1. Root `AGENTS.md` — build/test workflow, crate layering, port-first rule, parallelism rules.
2. `docs/superpowers/specs/2026-07-16-unified-input-settings-program-design.md` — the approved program design (architecture decisions D1–D4, contexts, prompts, Settings IA, readiness, error handling). **This is the contract.**
3. `docs/superpowers/plans/2026-07-16-pr1-pr2-semantic-input-router.md` — the plan that was already executed (PR1+PR2). Useful to understand what exists.
4. `crates/<name>/AGENTS.md` for every crate you touch.
5. Design/decision docs: `docs/decisions/0005-crate-layering.md`, `0009-input-profiles-source-of-truth.md`, `0010-port-mechanics-redesign-ux.md`, `0014-outquint-screen-transitions.md`, and the specs under `docs/superpowers/specs/` dated 2026-07-13/14/15.

## 1. Current state (as of merge `4cd496a` on `main`, 2026-07-16)

PR0–PR2 of the program are **done and merged to main**. Baseline for new work = current `main`.

### What exists now

**`dtx-input` (Engine layer, edition 2021):**
- `SystemVerb` — the single canonical persisted non-lane action vocabulary, 16 variants:
  NavigateUp/Down/Left/Right, Confirm, Back, PreviousTab, NextTab, PreviousPage,
  NextPage, Decrease, Increase, Preview, OpenSystemMenu, Pause, Restart.
  Stable kebab-case keys via `key()`/`from_key()` (serde rename agrees); `label()`;
  `activation_scope() -> VerbScope { Menu, LiveSystem }`; `allows_lane_sharing()`
  (Menu=true, LiveSystem=false). `pause`/`restart` file keys are frozen (v1 files).
- Built-in defaults in `bindings.rs`: `default_menu_keyboard_sources()` (ArrowUp/Down/
  Left/Right, Enter, Escape, Tab=NextTab, PageUp/PageDown; PreviousTab derived from
  Shift+Tab by the router), `default_menu_midi_sources()` (drum convention: HH 42/46=Up,
  CY/RD 57/52/51/59=Down, HT 48/50=Left, LT 45/47=Right, BD 36/35=Confirm,
  SD 38/40=Back, FT 43/41=NextTab), composed by `default_system_bindings()` into
  `InputBindings::default().system`. **No default OpenSystemMenu note — never guess one.**
- `BindResolver` (`resolver.rs`): builds `key→[verb]`, `note→[verb]` tables.
  Live-system verbs lose lane ties (skipped with warn). Menu verbs may lane-share.
  A menu verb with **zero** bound sources falls back to the built-in keyboard defaults
  (migrated v1 profiles keep keyboard nav). Live verbs are never injected.
- Pump (`pump.rs`): emits `SystemVerbHit { verb, source: VerbSource::{Keyboard,Midi} }`
  after velocity threshold. `PadNavHit` **no longer exists**. `LastMidiHit`,
  `ResolvedInputHit`, `MidiConnected`, `RawInputOwned` unchanged.
- `keyboard.rs`: `keyboard_system_verbs` emits `SystemVerbHit{Keyboard}` from bound keys;
  silenced by `RawInputOwned`. Scheduled by gameplay-drums (`src/input.rs`) in
  **PreUpdate, unconditional** (all states).
- Profiles (`profiles.rs`): keyboard/MIDI registries persist `system` verb tables with
  stable keys; split/compose preserve all verbs; v1 files without `[system]` load fine
  (container serde(default) fills built-in menu defaults).

**`game-shell` (Game layer, edition 2024) — `src/navigation/`:**
- `context.rs`: `NavContext` (17 variants: Home, SongSelectSongs, SongSelectDifficulty,
  SongReadyBrowse, SongReadyEdit, SongLoading, PracticeSetupSettings, PracticeSetupPreview,
  PauseMenu, Results, SettingsTabs, SettingsRows, SettingsEdit, ModalDialog,
  BindingCapture, LayoutEditor, LiveGameplay). `is_edit()` = SongReadyEdit | SettingsEdit
  | PracticeSetupSettings | LayoutEditor (LayoutEditor is MIDI-parity only — see §2
  gotcha). `exclusive()` = BindingCapture. `NavContextStack`: `push` (idempotent,
  moves to top), `pop` (removes anywhere), `top`, `clear`.
- `source.rs`: `InputSource { Keyboard, Mouse, MidiKit, Gamepad }`,
  `LastIntentionalInputSource` (default Keyboard, plain resource, survives all state
  transitions), `MouseIntent` message (screens write it on click/wheel/drag — pointer
  motion never), `PromptSourcePreference { Automatic, Always(InputSource) }`
  (**not persisted yet** — PR6 wires it into the Settings draft/accessibility config).
- `router.rs`: pure `route(top, verb, source, coarse, guard, now) -> Routed
  {Menu(NavAction), Live(SystemVerb), Dropped}` + `route_verbs` system in `NavRouterSet`.
  Rules: live verbs pass everywhere except exclusive contexts; menu verbs need an
  owning non-LiveGameplay/non-exclusive context; MIDI hits ride NavGuard (500 ms
  entry grace on context change + 80 ms debounce) — keyboard exempt; edit contexts
  translate NavigateLeft/Right→Decrease/Increase; Shift is keyboard-only (`coarse`
  normalized false for MIDI); Shift+Tab→PreviousTab. `LiveVerb(SystemVerb)` message
  feeds live consumers. Router updates `LastIntentionalInputSource` on delivered
  actions and on `MouseIntent`.
  **Transitional:** keyboard menu delivery is skipped while stack top == LayoutEditor
  (the layout editor still runs its own keyboard→NavAction emitters). Removing this is
  PR8 work.
- `mod.rs`: `NavAction { verb: SystemVerb, source: InputSource, coarse: bool,
  repeated: bool }` (repeated always false today), `NavGuard` (+`sync`, test-only
  `force_ready`), sets `NavStackWriteSet` → `NavStackRefineSet` → (mirror deleted) →
  `NavRouterSet`. Plugin registers everything.
- Deleted (do not reintroduce): `NavVerb`, `NavSource`, `PadNavHit`, `verb_for_lane`,
  `ActiveNavContext`, `NavMapSet`, `mirror_stack_to_active`, per-screen keyboard emitters.

**Context stack population:**
- `gameplay-drums/src/menu_nav.rs`: transitional per-frame publisher `set_stack` writes
  exactly `[ctx]` or empty into `NavContextStack` (in `NavStackWriteSet`): Home,
  SongSelectSongs, Results, SongLoading, PauseMenu, LayoutEditor, PracticeSetupSettings,
  BindingCapture (capture/calibration), LiveGameplay (live play incl. practice Running).
- Refiners (in `NavStackRefineSet`): `game-menu/src/song_ready.rs` swaps top to
  SongReadyBrowse/SongReadyEdit while its layer is open; `gameplay-drums/practice/hud/
  setup_controls.rs` swaps to PracticeSetupPreview when preview/transport is focused.

**Screens** consume router `NavAction` only. Documented screen-local raw accelerators
(they do NOT emit NavAction): song_select Esc-clears-search (`search_esc_intercept` +
`SearchEscConsumed` one-Back skip), F1/F5/F7/Ctrl+R/Ctrl+0-4 hotkeys; results R/Tab/Space;
practice setup Space; pause raw-Esc toggle (coexists with router Back — both resolve to
the same state); layout editor full local keyboard nav (exempt).

**Live verbs:** `pause.rs` `system_verb_pause`/`system_verb_restart` consume `LiveVerb`,
run_if Performance, `.after(NavRouterSet)`. `OpenSystemMenu` opens the pause overlay when
Running and never un-pauses.

**Practice is not a verb.** Song Select Shift+Enter = Confirm+coarse → ReadyMode::Practice;
Song Ready has an internal Normal↔Practice mode toggle; Results has a Practice verb-row
entry. The old editor Controls tab lists live-system verbs only (`live_system_verbs()`
filter in `editor/controls_panel.rs`).

### Test counts at merge (all green)
dtx-input lib 128 · game-shell lib 34 + all_stages_reachable 4 · game-menu lib 90 ·
gameplay-drums lib 581 + practice_mode 98 + practice_hud 65 + bindings_lane_pipeline 1 ·
game-results lib 38 · `cargo check --workspace` · `cargo clippy --workspace --all-targets
-- -D warnings` · `cargo fmt --all -- --check`.

## 2. Hard-won gotchas (violate these and you will burn hours)

- **rustfmt drift:** never run bare `cargo fmt --all` (write mode). Verify with
  `cargo fmt --all -- --check`; format only files you touched with
  `rustfmt --edition <ed> <file>`. Editions vary per crate: game-shell/game-menu = 2024,
  dtx-input/gameplay-drums/dtx-config/dtx-ui = 2021 (check each Cargo.toml; workspace
  default is 2021).
- **Subagents + cargo:** long cargo builds get auto-backgrounded inside subagents and
  deadlock them. If orchestrating with subagents: they must poll the backgrounded output
  file to completion, or the controller runs builds inline.
- **CARGO_TARGET_DIR:** shared at `~/.cache/cargo-target`. Never create per-worktree
  target dirs (78 GB incident). Never `cargo test --workspace --all-targets` (RAM);
  package tests only.
- **Bevy 0.19 UI:** UI nodes use `UiGlobalTransform`, not `GlobalTransform` —
  a `&GlobalTransform` query silently matches nothing.
- **`LaneHit` has three producers** (MIDI, keyboard lane keys, autoplay). Anything that
  means "a human did a menu action" must consume router `NavAction`/`SystemVerbHit`,
  never `LaneHit`. The Customize surface forces autoplay ON — reading LaneHit there once
  let chart notes navigate and corrupt config.
- **Green unit tests ≠ schedule builds.** Use the ordering guard test / run the real app.
  BRP manual loop: build `cargo build -p dtxmaniars-desktop --features brp`, run the
  binary, drive with bevy-brp MCP tools (screenshot/send_keys). Binary name is
  `dtxmaniars`.
- **LayoutEditor double-delivery trap:** the router skips *keyboard* menu delivery when
  top == LayoutEditor because the editor has local emitters. If you migrate the editor
  (PR8), delete the skip in the same commit or arrows double-step. MIDI menu verbs DO
  route there already (that's how pads navigate it now), and LayoutEditor sits in
  `is_edit()` purely so MIDI HT/LT arrive as Decrease/Increase — if the editor migrates
  to explicit NavigateLeft/Right handling, remove it from `is_edit()` too.
- **One producer per input path.** Before adding any input handling, grep
  `MessageWriter<NavAction>` and `just_pressed` — the whole PR2 point is exactly one
  route per press. There are exactly-one-step regression tests in song_select.rs and
  game-results/input.rs; copy that pattern for new surfaces.
- **Transitions:** feature crates write `TransitionRequest` only, never
  `NextState<AppState>`. 300 ms OutQuint via the transition director (ADR-0014).
- **No `unwrap()` in `crates/*`** (tests may). No AI co-author trailers in commits.
- **A stray stash exists:** `stash@{0}: pre-merge-score-identity-…` is unrelated old
  work. Do not pop it. If conflict markers ever appear in dtx-core/dtx-timing files you
  didn't touch, someone popped it — `git reset --hard HEAD` and leave the stash alone.

## 3. Remaining work: PR3 → PR8

Each PR: separate branch off main, own plan (TDD for pure logic), package gates green
before merge, one logical commit per reviewable change. Full program requirements live
in the spec; below is the distilled task list with landed-state adjustments.

---

### PR3 — Dynamic prompt bar (`dtx-ui` + screens)

Refactor `crates/dtx-ui/src/widget/nav_legend.rs` (currently MIDI-connected-gated,
pad-specific) into a reusable prompt bar.

- Model: screens declare `PromptSpec { actions: Vec<PromptAction { verb: SystemVerb,
  text, enabled }> }` (shape may vary; screens declare semantics, never device strings).
- Renderer resolves: effective source = `PromptSourcePreference` (Always wins) else
  `LastIntentionalInputSource`; then formats each action from the **actual bindings**
  (`BindResolver` / active profiles — note dtx-ui must not depend on gameplay crates;
  dtx-ui may depend on dtx-input per layering, check `crates/dtx-ui/Cargo.toml` and
  AGENTS.md before wiring).
- Display: Keyboard → "Arrows Move · Enter Confirm · Esc Back" style from bound keys;
  MidiKit → lane-shared notes display the lane short name (HH/SD/BD…), spare notes a
  compact "N38" or profile label; Mouse → "Click Select · Wheel Scroll…".
  Unbound advertised verb → "Unbound" or omitted with Settings hint; never panic.
  Keyboard/mouse prompts must render with no MIDI connected.
- Migrate hard-coded footer/help text: Home, Song Select (the current bottom legend
  "←→ SELECT ↑↓ CHANGE ENTER READY…"), Song Ready, Practice Setup, practice pause,
  normal pause, Results, dialogs. Do not remove on-screen buttons mouse users need.
- Tests: automatic-vs-locked preference; no-MIDI shows keyboard prompts; configured
  bindings displayed; unbound safe; text-scale/accessibility behavior.
- Gates: `cargo test -p dtx-ui --lib` + game-shell/game-menu/gameplay-drums libs +
  desktop check.

### PR4 — Throne readiness + guided-test API (`dtx-input` + `game-shell`)

- Pure readiness (approved design D2 — struct, not enum):
  ```rust
  pub struct InputReadiness {
      pub keyboard_ready: bool, // true by construction; kept for UI truthfulness
      pub midi_gameplay: MidiGameplay,   // Ready | NoProfileLanes | DeviceUnavailable | NoDevice
      pub throne: ThroneStatus,          // Ready | Incomplete { missing: Vec<SystemVerb> } | NoMidi
  }
  pub fn assess(keyboard: &KeyboardProfile, midi: &MidiProfile, connected: bool) -> InputReadiness
  ```
  Throne-required verbs: NavigateUp/Down/Left/Right, Confirm, Back, OpenSystemMenu —
  all resolvable from the kit (MIDI sources, lane-shared fine) AND OpenSystemMenu valid
  + lane-exclusive. `ThroneStatus::Ready` requires a valid OpenSystemMenu binding —
  never claim it otherwise. No state blocks keyboard/mouse Play.
- Guided-test API (UI-agnostic, consumed by PR6 Kit page): list required throne verbs,
  current bound sources per verb, missing/conflicts, test-that-a-hit-resolves (feed
  `LastMidiHit`/`SystemVerbHit` and report which verb fired vs expected), clear/rebind
  a verb (respecting the resolver policy), active device/profile identity.
- `OpenSystemMenu` live wiring already exists (pause.rs). Keyboard Escape must keep
  working with no OpenSystemMenu binding (it does — Back/raw Esc paths).
- Tests: keyboard-only Ready with no MIDI; disconnect non-fatal; complete menu maps
  w/o OpenSystemMenu → Incomplete{missing:[OpenSystemMenu]}; spare-note OpenSystemMenu →
  Ready; lane-colliding OpenSystemMenu rejected; lane-sharing NavigateUp accepted;
  Pause/Restart round-trip unchanged.
- Gates: dtx-input lib, dtx-input --features midi check, game-shell lib.

### PR5 — `AppState::Settings` + `game-settings` crate shell

- Add `AppState::Settings` in `crates/game-shell/src/states.rs`; Home "Settings" →
  `TransitionRequest(Settings)`; Back → Home via the transition director. Settings must
  work with empty `SongDb` and no MIDI; never touches `EditorSession`/autoplay/
  Performance. (Home currently routes Settings through `crate::title::
  request_gameplay_settings` → chart-backed editor; leave that fn alive for the layout
  editor until PR8, but Home stops calling it for ordinary Settings **in this PR** —
  Definition of Done item 1.)
- New crate `crates/game-settings` (Game layer; deps dtx-config, dtx-input, dtx-ui,
  game-shell; register `GameSettingsPlugin` in `app/dtxmaniars-desktop/src/main.rs`;
  add to workspace members):
  ```
  src/lib.rs state.rs draft.rs navigation.rs search.rs ui.rs rows.rs
  src/pages/{mod,setup,controls,kit,audio,gameplay,visuals,library,accessibility}.rs
  ```
- Category bar: `SETUP | CONTROLS | KIT | AUDIO | GAMEPLAY | VISUALS | LIBRARY |
  ACCESSIBILITY` — one line at 1280×720 and 1920×1080, horizontal scroll below.
  Focus model Tabs→Rows→Edit; push/pop `SettingsTabs/SettingsRows/SettingsEdit` contexts
  (the router already translates Left/Right→Dec/Inc in SettingsEdit). Menu_nav's
  transitional publisher knows nothing about AppState::Settings — extend it (or better:
  publish from game-settings via a refiner, matching song_ready's pattern).
- Descriptor extraction (approved D4): move UI-independent config descriptors
  (id, label, description, keywords, kind, get/set/adjust/reset closures, clamps) from
  `gameplay-drums/src/editor/settings_data.rs` into Pure `dtx-config::settings`
  (Bevy-free!). Old editor + new Settings both consume them — one source of
  clamps/defaults, zero duplication.
- Row framework (`rows.rs`): Toggle, Enum selector, Numeric stepper, Slider, Action,
  Navigation link, Binding capture, Read-only status. Stable ID, label, value,
  description, enabled+disabled-reason, search keywords, validation, preview hook.
- Use the shared router + (PR3) prompt bar; mouse-clickable; focus never color-only.
- Tests: every category reachable; opens with empty SongDb + MidiConnected(false);
  Home no longer calls the chart-backed adapter for ordinary Settings; Back → Home;
  keyboard and MIDI category nav; mouse click changes focus and source; entities
  cleaned on exit (StateScoped); no Performance/EditorSession/autoplay activated.
- Gates: game-settings lib, game-shell lib, game-menu lib, desktop check.

### PR6 — Draft/Apply/Discard, search, Setup/Controls/Kit pages

- `SettingsDraft` (approved D3): `Domain<T> { entry: T, draft: T }` for
  `dtx_config::Config`, keyboard registry, MIDI registry, `PromptSourcePreference`
  (this is where the preference becomes persisted — decide its home: accessibility
  section of config.toml). Dirty = `entry != draft` per domain; UI shows summed count
  ("3 unsaved changes · Discard | Apply").
- Apply: validate all domains → ordered writes via existing atomic APIs
  (`dtx_persistence::replace_bytes`, `save_keyboard_registry`, `save_midi_registry`,
  config save) → per-domain failure keeps that domain dirty and reports domain+path;
  successes clear. Never claim full success after partial failure. Discard: draft←entry.
  Back with dirty → modal (ModalDialog context): Keep Editing / Discard / Apply.
- Search: typing while SettingsRows/Tabs owns input opens search (never during
  BindingCapture / text entry / modal). Results show label + category path + current
  value; selecting deep-links focus to the real row. Empty state handled.
- Setup page: renders PR4 `InputReadiness` + latency calibration status + library
  counts; actions: Test Controls, Complete Throne Controls, Calibrate Latency, Manage
  Song Library (links to pages/flows). No MIDI is a valid state.
- Controls page: keyboard gameplay binds, keyboard menu binds (the SystemVerb menu
  table!), Pause/Restart, mouse policy status, conflict indicators, reset/copy/save
  profile workflows (reuse `reduce_registry` actions). Keyboard controls never hidden
  inside Kit.
- Kit page: device/port picker (existing `match_midi_port` logic in editor
  controls_panel is the reference), active MIDI profile bar, velocity threshold, live
  note/velocity monitor (`LastMidiHit`), gameplay pad map, menu-verb binds
  (lane-sharing allowed + labeled), spare OpenSystemMenu capture (lane-exclusive,
  refusal names the owning lane), guided throne test (PR4 API), readiness card.
  Struck-note readout: note / velocity / gameplay owner / menu owner.
- Binding capture: exclusive context (push BindingCapture + set `RawInputOwned`);
  surrounding nav frozen; clear cancel; velocity monitor keeps updating (pump never
  checks RawInputOwned — by design); no double consumption.
- Tests: per-domain dirty; discard restores; apply persists + clears only on success;
  write-failure retains dirty (temp-dir induced failures); search by label/category/
  keyword; keyboard-only Setup reports Ready; guided test flags missing OpenSystemMenu;
  capture policies match resolver policy.

### PR7 — Audio, Gameplay, Visuals, Library, Accessibility pages

- Audio: volumes (master/BGM/drums per current schema), input offset, BGM offset,
  hit-sound options, calibration entry, audio tests. No invented device switching —
  disable with reason if the audio layer doesn't expose it.
- Gameplay: lane speed, tight mode, reverse, lane display, damage/fail behavior,
  fill-in, playback options — whatever `dtx-config::Config` actually persists today
  (enumerate from the extracted descriptors; do not invent). Optional compact non-judged
  note-motion preview (no score/judgment/practice attempt).
- Visuals: BGA/movie enable + opacity, lane/HUD presentation previewable without a
  chart, reduced-flash links, **Edit Full Layout** action → adapter: pick last-valid
  else first chart, set existing `EditorSession` resources, route SongLoading→
  Performance, return to **Settings** on exit (not Home/Results — the editor's exit
  path needs a `ReturnTo`-style mechanism; today it returns via title flow, check
  `request_gameplay_settings` + editor close path). Library empty → disabled row with
  "Add at least one playable chart to use the full layout editor."
- Library: watched/default folders (as currently architected in dtx-library), rescan,
  parsed/skipped/warning counts, scan diagnostics (move the dev-oriented scan-timing
  line out of Song Select's header here — don't delete it), import actions, discovery
  defaults where persisted.
- Accessibility: text scale, reduce motion, reduce flashes, background motion,
  prompt-source preference (the PR6 domain), focus support. Safe preview changes update
  the live `AccessibilityPolicy` immediately; persistence still via Apply/Discard.
- Tests: each row round-trips draft→persist; clamps match descriptor rules; preview
  never persists after Discard; Apply updates runtime policies; layout-editor disabled
  reason with empty library; EditorSession path when chart exists; editor exit returns
  to Settings; rescan errors reported without leaving Settings.

### PR8 — Home readiness panel, retirement, final cleanup, docs

- Home keeps Play/Settings/Exit; add compact readiness panel (from PR4 `assess`):
  keyboard/MIDI/throne/latency/library status lines, optional deep-links into Settings
  rows. Not new menu destinations. Truthful, never blocks Play.
- Retire old ordinary-Settings route: `request_gameplay_settings` renamed/reduced to
  the Full Layout Editor launcher only; ordinary Settings never emits "needs at least
  one available chart"; exactly one user-visible Settings system.
- Migrate the layout editor onto the router (delete its local keyboard emitters in
  `editor/keyboard_nav.rs` + panel emitters in lanes_panel/controls_panel, delete the
  router's LayoutEditor keyboard skip **in the same commit**, remove LayoutEditor from
  `is_edit()` if you add explicit NavigateLeft/Right arms). If genuinely impractical,
  document why and keep the skip — but the double-delivery invariant must hold.
- Replace menu_nav's transitional per-frame `set_stack` publisher with real per-screen
  push/pop where feasible (screens own OnEnter/OnExit); keep gameplay facts
  (LiveGameplay, PauseMenu, capture) published by gameplay-drums.
- Sweep stale references: grep docs/ + code comments for PadNavHit/NavVerb/NavSource/
  verb_for_lane (some doc lines may linger, e.g. old comments in game-results about
  "FT jumps to practice" — kill them).
- Docs: update `docs/roadmap.md`, `docs/player-guide.md`, `docs/data-and-persistence.md`
  (new binding keys + prompt preference), `docs/compatibility.md` (keyboard-only +
  MIDI-optional + readiness), and add an ADR for the unified SystemVerb vocabulary +
  Settings state boundary (next free number in docs/decisions/). Document that old
  profile files keep loading and MIDI is optional.

## 4. Verification matrix (run per-PR for touched crates; full set before final handoff)

```sh
cargo fmt --all -- --check
cargo test -p dtx-config --lib
cargo test -p dtx-input --lib
cargo check -p dtx-input --features midi
cargo test -p dtx-ui --lib
cargo test -p game-shell --lib
cargo test -p game-shell --test all_stages_reachable
cargo test -p game-menu --lib
cargo test -p game-results --lib
cargo test -p gameplay-drums --lib
cargo test -p gameplay-drums --test practice_mode
cargo test -p gameplay-drums --test practice_hud
cargo test -p gameplay-drums --test bindings_lane_pipeline
cargo test -p game-settings --lib          # once the crate exists
cargo check -p dtxmaniars-desktop
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 5. Manual QA matrix (record results at 1280×720 and 1920×1080)

Still fully open from the program (PR1/PR2 got only a keyboard smoke):

- **Keyboard only, no MIDI:** Home→Play→Song Select→Ready→normal play; Home→Settings
  with empty library; configure keyboard gameplay + navigation bindings; Practice Setup
  + run; pause/restart/results/return/exit; prompts switch to Keyboard on key press.
- **Mouse:** click all Home/Settings/Song Ready actions; wheel lists; drag sliders +
  practice loop/timeline; pointer motion alone never switches prompts; click/wheel/drag
  switches to Mouse.
- **Virtual MIDI:** lane-shared menu binds navigate outside live play; same notes judge
  during live play; OpenSystemMenu from a lane-free note; lane-colliding OpenSystemMenu
  refused; prompts show configured MIDI binds; 80 ms debounce + 500 ms grace effective.
- **Physical kit:** hot-plug/reconnect; velocity threshold; guided throne test; full
  throne navigation Home→Practice→back; pause/system menu mid-song; disconnect leaves
  keyboard/mouse recovery.
- **Settings:** zero songs; no MIDI; Apply/Discard on all page types; search+deep-link;
  layout editor handoff with/without charts; editor exit returns to Settings.
- **Practice regression:** every Practice request opens stopped Setup; preview
  non-judged; saved loops isolated by chart hash+difficulty; Wait/Ramp unchanged;
  practice Settings continue from pre-roll; ordinary Pause resumes exactly.

## 6. Definition of done (program-level, remaining items)

1. ~~SystemVerb sole vocabulary~~ ✅  2. ~~hard-coded lane mapping removed~~ ✅
3. ~~routing owned by game-shell~~ ✅  4. ~~menu binds configurable + lane-sharing policy~~ ✅
5. Home Settings → dedicated `AppState::Settings`, no chart dependency (PR5)
6. Dynamic prompts follow last intentional source + real bindings (PR3)
7. `OpenSystemMenu` required for Throne Ready; readiness truthful (PR4, PR8)
8. Settings: 8 pages, coherent Apply/Discard, search (PR5–7)
9. Autoplay editor reachable only via Visuals → Edit Full Layout (PR7–8)
10. Home readiness panel; keyboard/mouse complete without MIDI (PR8)
11. Existing Home/Song Select/Song Ready/Practice behavior intact (every PR)
12. Automated gates green + manual QA recorded (every PR / final)

## 7. Working agreements

- Branch per PR off `main`; merge only after gates + review. No AI co-author trailers.
- TDD for pure reducers, schema, routing, readiness, persistence. Bevy App tests for
  wiring. Copy the exactly-one-action test pattern for any new input surface.
- Serialize Bevy-heavy builds; parallelize only edits and small package checks.
- When unsure about Bevy APIs: `npx ctx7@latest docs /websites/rs_bevy "<question>"`.
- Report at each PR handoff: files changed, exact test results, manual checks done,
  compatibility notes, deviations + justification.
