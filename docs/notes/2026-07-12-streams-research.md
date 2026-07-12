# Remaining-Streams Research — 2026-07-12

Pre-design research for the five remaining work streams, in execution order:
results screen rebuild → practice rail → reducer wiring → design tokens →
distant-kit pad grammar. All line refs verified against post-quick-wins main
(`58c2fa0`). Each stream still gets its own brainstorm → spec → plan →
implementation cycle; this doc is the shared factual base.

---

## Stream 1 — Results screen rebuild

### Current state (`crates/game-results/src/lib.rs`)

- Plugin: `OnEnter(Result)` runs `(save_result, spawn_result).chain()`;
  `OnExit` despawns; `Update` runs `(result_input, animate_staggered_reveal)`
  (`lib.rs:52-60`).
- `SaveStatus { Practice, Saved, Failed }` set by `save_result`
  (`lib.rs:36-42, 327-408`) — keep this behavior verbatim in the rebuild.
- 15 stat rows + conditional save-status row, all `Theme::label_font()` (18px)
  white, alignment via space-padding inside format strings
  (`lib.rs:161-208, 261`). Constants `STAGGER_MS=120`, `FADE_DURATION_MS=300`.
- `animate_staggered_reveal` is a pure linear alpha fade — no easing, no
  transform (`lib.rs:295-309`).
- `result_input` (`lib.rs:311-325`): pad Confirm/Back or Esc/Enter → SongSelect.
  Single verb.

### Data available at results time

| Datum | Source |
|---|---|
| `Score(u64)` | `gameplay-drums/src/resources.rs:54` |
| `Combo { current, max }` | `resources.rs:83-86` |
| `JudgmentCounts` + `total()`, `perfect_pct()`, `achievement_pct()` | `resources.rs:147-189` |
| `ActiveChart.metadata()` (title/artist/dlevel) | `resources.rs:31-49` |
| `DrumScoring.total_notes` | `resources.rs:63-65` |
| `Rank` SS..E, `from_bocud_counts` (XG: P%·0.85 + G%·0.35 + combo%·0.15; thresholds 95/80/73/63/53/45) | `dtx-scoring/src/lib.rs:74-153` |
| `LastStageOutcome { cleared }` | `gameplay-drums/src/stage_end.rs` |

### Key feasibility findings

- **Retry is trivial.** `SelectedSong` (`song_select.rs:89`) and
  `PracticeIntent` (`game-shell/states.rs:101`) are plain resources with no
  `OnExit` clears — both survive into Result. Retry =
  `request_transition(AppState::SongLoading)`; SongLoading re-reads
  `SelectedSong` and relaunches identically. Pause-menu Retry already does
  exactly this (`pause.rs:266-270`).
- **Practice handoff is trivial.** Set `PracticeIntent.0 = true` then
  transition to SongLoading; `enter_practice_session`
  (`practice/mod.rs:70-83`) derives `PracticeSession` on Performance enter.
- **Pad verbs are free.** All five NavVerbs already reach `result_input`
  (`menu_nav.rs:115, 133-166`); only Confirm/Back consumed today. Up/Down can
  drive verb selection, FT (`Practice`) can map to practice handoff.
- **OutQuint reveal mechanism exists.** `EnterChoreo::slide(offset, delay_ms,
  duration_ms)` defaults to OutQuint; `enter_choreo_system` drives
  `UiTransform.translation` and self-removes (`dtx-ui/src/motion.rs:127-183`).
  Concrete stagger usage: `title.rs:95,119`, `song_select.rs:780-1004`
  (increasing delay per row = stagger). `ScalarTween` (`tween.rs:7-60`)
  available for alpha.
- Theme has full judgment palette incl. new `judgment_ok` purple,
  `judgment_color(label)` mapper, `difficulty_color`, `clear_green`
  (`theme.rs:47-51, 70-79, 117-124`).
- Legend pattern: `spawn_nav_legend(parent, &theme, &[(pad, verb)])` gated on
  `Option<Res<MidiConnected>>.is_some_and(|m| m.0)` (`nav_legend.rs:17`,
  current usage `game-results/lib.rs:288-292`).
- **No NX reference recoverable**: `references/` is empty; `CStageResult`
  exists only as doc-comment citations. Rebuild is free to follow ADR-0014
  osu-inspired direction without port-parity constraints (results is
  presentation, not mechanics).

### Design surface

Hierarchy (rank headline, score sub-headline, judgment table), judgment-colored
rows, real column layout (two-node rows, not space padding), OutQuint staggered
slide+fade, skip-reveal on first input, verb row (Continue / Retry / Practice)
with keyboard + pad selection, keep SaveStatus line and BD legend.

---

## Stream 2 — Practice full-HUD rail redesign + pause unification

### Current state

- `practice/hud/mod.rs:23-49`: full HUD spawns `OnEnter(PauseState::Paused)`
  gated `resource_exists::<PracticeSession>`; update chain
  `timeline_mouse → full_hud_input → transport_buttons →
  update_full_hud_markers`.
- Rail (`full_hud.rs:273-345`): absolute right, **`width: Val::Px(340.0)`**
  (`:279`, inline literal), 18 `RailItem` rows (`:39-82`) each
  `Text + hud_font()` (32px), no wrap/clip/width, no Button/Interaction.
  Headers TRANSPORT/LOOP/TRAINER injected at idx 0/7/10 (`:299-304`).
  Selected row = color-only accent swap (`full_hud_input` re-labels every
  frame, `:641-655`).
- Exit = rail row 18 double-Enter via `ExitArmed` (`:630-637`).
- Bottom 72px timeline row (`:348-448`): three `TransportButton`s (already
  Button+Interaction, `:200-245`), time text, density strip with playhead /
  loop fill / scrub cursor.
- Root hardcodes `GlobalZIndex(1000)` instead of `ui_z::PRACTICE_FULL_HUD`
  (`:269`).
- Esc and Tab are identical in practice: both set `PauseState::Paused` →
  full HUD (`pause.rs:79-90` + suppression `:142-144, 231-234`;
  `actions.rs:140`).

### Pause overlay already has what unification needs

`pause.rs`: Resume/Retry/Quit rows (`:26-42`), keyboard emit (`:203-219`),
NavAction consumer with full pad grammar HH/CY/BD/SD + legend when MIDI
connected (`:181-192, 221-277`). SD = resume. Retry → SongLoading, Quit →
SongSelect. Practice currently suppresses all of it. Unification =
**un-suppress + extend**, not build-new. Note pause rows are Text-only (no
mouse); rail redesign wants clickable — decide one interaction model for both.

### Layout primitives for scale-awareness

- `PlayfieldLayout`: `scale = (w/1280).min(h/720)`, `origin`,
  `px(ref) = ref*scale`, `ref_hud_right_x()` (`layout.rs:57, 83-85, 174-176`);
  rebuilt on resize (`:212-228`).
- `HudRefRect` component stores ref-px rect, `apply(scale, origin, node)`;
  `scaled_font(scale, ref_size)` (`dtx-ui/src/widget/hud_ref.rs:23-37`);
  reapplied on resize in `hud.rs:393-411`. `now_playing.rs` is the reference
  implementation (320×72 ref-px at `ref_hud_right_x()`).
- `spawn_full_hud` does not query `PlayfieldLayout` today — adding it is the
  whole conversion prerequisite.
- Full HUD deliberately not a `WidgetKind` (fixed overlay by design,
  `hud/mod.rs:1-3`); keep it that way.

### Reusable control kit

`dtx-ui/src/widget/controls.rs`: `spawn_slider` / `spawn_stepper` /
`spawn_toggle` with `Slider`/`Stepper`/`Toggle` + `ControlValue`/`ControlBool`,
driven by `Changed<Interaction>` systems (`:56-193`). Maps directly onto rail's
`◀ value ▶` rows. Caveat: kit is fixed-px (`SLIDER_WIDTH=110`).

### Per-row value map (what each rail row mutates)

`PracticeSession { transport, trainer, current_attempt, attempt_history,
lane_diag }` (`session.rs:203-211`):

| Rows | State | Mutation |
|---|---|---|
| Rate | `transport.user_tempo` | `step_user_tempo(dir)` clamp 0.5–1.5 step 0.05 (`session.rs:234-237`) |
| Snap / Pre-roll | `transport.snap` / `preroll` | `.next()` |
| Set A / Set B / Clear | `transport.loop_region` | `set_loop_start/end`, `clear_loop` (clears diag, disarms ramp) |
| Metronome / WaitMode | bools | direct flip (`full_hud.rs:604-624`) |
| RampStart/Target/Step/Threshold/Streak | `trainer.ramp_config` | direct writes + `ramp::clamp_to_config` (`full_hud.rs:519-544`) |
| RampArm | `trainer.ramp` | via `PracticeAction::ToggleRamp` message |
| Scrub | `transport.scrub_cursor_ms` | timeline gestures + arrows |

### Sizing math (why current rail fails)

Window: 1280×720 windowed or borderless monitor-res
(`app/dtxmaniars-desktop/src/main.rs:28-47`), no min size. At 1080p
scale≈1.5: ref-px Now-Playing renders ~480px wide, rail flat 340 screen-px →
collision. At 720p: 18 rows × 32px + 3 headers + gaps ≈ 800px into 648px
(`bottom:72`) with `justify_content: Center` → overflow, no scroll/clip.

### Design surface

Ref-px rail (or restructured panel) via `HudRefRect` + `scaled_font`; type
hierarchy (headers vs rows vs values); mouse rows (Interaction) reusing
controls.rs steppers/toggles; unify pause: Esc in practice → pause surface
with Resume/Retry/Quit + practice verbs, exit-practice lives there (kills
double-Enter rail exit and the F20 Esc fork); Tab may keep full rail. Fix z
literal.

---

## Stream 3 — Reducer wiring (Controls / Lanes / Widgets / dialogs)

### Verified: still unwired

Only callers of `reduce_controls_nav` / `reduce_lanes_nav` are their own
tests. `ControlsFocus` is registered (`editor/mod.rs:114`) and already in
`LeftPanelSig` (`panel.rs:252-264`) but never mutated — sits at `TabBar`
forever. `LanesFocus` (`lanes_panel.rs:67`) is never `init_resource`'d — fully
dead.

### The reducers

- `reduce_controls_nav(focus, segment, verb) -> (ControlsFocus,
  ControlsSegment)` (`controls_panel.rs:115-136`). Pure. TabBar ↔
  SegmentSelector ↔ Rows; Inc/Dec toggles Keyboard/Midi segment.
  **Gap: no row cursor** — Rows-level movement not modeled.
- `reduce_lanes_nav(focus, selected, lane_count, verb, coarse) ->
  (LanesFocus, usize, LanesNavEffect)` (`lanes_panel.rs:102-176`).
  `LanesNavEffect::{None, Reorder{index,dir}, AdjustWidth{index,dir}}` —
  caller applies mutations. `coarse` (shift / pad coarse) repurposed as move
  modifier: coarse+Up/Down reorders. Both reducers well-tested
  (`controls_panel.rs:241-284`, `lanes_panel.rs:744-820`).

### Nav flow today

`keyboard_emit_nav` (arrows/Enter/PageUp/Dn, shift=coarse,
`keyboard_nav.rs:141-176`) and `menu_nav.rs:162` (pads) write `NavAction`;
single consumer `settings_nav_consumer` (`keyboard_nav.rs:178-288`) drives
`FocusedRow`/`NavLevel` for `is_settings()` tabs only. `pad_can_enter`
excludes Controls+Widgets (`:32-39`); Lanes isn't `is_settings()` so neither
keyboard nor pads can descend into it via the generic path. Separate
`MessageReader` instances each get their own copy — a parallel Controls/Lanes
consumer coexists fine with the generic one.

### Wiring requirements

- New consumer system(s) gated `Performance` + `editor_open` +
  `profile_dialog_closed`, acting only when `active.0` matches the tab.
- Controls driver must own what the reducer doesn't: stepping
  `bindings_capture::SelectedChannel` (`bindings_capture.rs:85`), Confirm →
  `CaptureState::Keyboard/Midi(ch)` (capture already has full
  keyboard+mouse parity via shared `arrived_step` reducer,
  `capture_modal.rs:416-446`), delete binding, Device-card steppers (port
  cycle, velocity threshold) on MIDI segment.
- Lanes driver: `init_resource::<LanesFocus>`; apply `Reorder` via the
  `move_lane_to` walk, `AdjustWidth` via `set_lane_width` (clamped
  MIN/MAX_LANE_WIDTH); undo via `UndoStack::push` same as mouse paths
  (`lane_drag.rs` one-snapshot-per-gesture pattern, `:257-266`).
- Repaint: `ControlsFocus`/`ControlsSegment`/`SelectedLane`/`Lanes` mutations
  already re-trigger `rebuild_left_content` via `LeftPanelSig`; **`LanesFocus`
  must be added to the sig** (`panel.rs:252-264`) plus a
  `resource_changed::<LanesFocus>` run condition (`panel.rs:127` pattern).
- Mouse parity to preserve: `EDGE_GRAB=6`, `CLICK_SLOP=3`, select-on-press
  without undo snapshot, nearest-center `drop_index`
  (`lane_drag.rs:32-71, 126-186`).

### Widgets tab + dialogs

- Widgets keyboard nudge already exists but bypasses NavAction
  (`drag.rs:281-322`, direct KeyCode, 1 ref-px / shift 8). Missing: keyboard
  selection cycling (Alt+click only today) and keyboard inspector steppers.
- Discard dialog (`close_dialog.rs`): Cancel/DiscardAll/SaveAll buttons,
  `Interaction::Pressed` only, zero KeyCode handling; layout already carries
  `default_focus` + `destructive` indices used only for coloring
  (`:142-147`) — natural hook for Enter-confirm/Esc-cancel + arrow traversal.
- Second mouse-only dirty dialog: `profile_dialog_ui.rs` `DialogButton::Dirty`
  (`:299, 474-500`). Name dialog already has full keyboard (`:354-420`).

---

## Stream 4 — Design-token consolidation

### Four competing accents

1. `theme.accent` cyan #00d4aa (`theme.rs:44`) — HUD widgets, controls kit.
2. Menus: `select_yellow` #ffcc00 as de-facto accent — 20 sites across 8 files
   (song_select, title, song_loading, stage_panel, import_ui, …).
3. Editor: `chrome::ACCENT` #5b8cff blue (`editor/chrome.rs:20`) + full private
   palette (PANEL_BG/CARD_BG/CHIP_BG/DIRTY/OK/ERR/WARN_TINT).
4. Selection box: local gold `srgb(1.0,0.75,0.1)` (`selection_box.rs:43`).

Plus **red FOCUS_RING** `srgb(0.89,0.20,0.20)` (`editor/panel.rs:63`) vs green
ADJUST_RING — red-as-focus collides with red-as-error (chrome ERR, judgment
miss, destructive buttons).

### Hardcoded color census

111 `Color::srgb(a)` occurrences in 39 files outside theme.rs/chrome.rs.
Notables: theme.accent cyan re-derived by hand in `controls.rs:324`; 9×
repeated `srgb(0.14,0.14,0.18)` in panel.rs; dialog scrim `srgba(0,0,0,~0.72)`
duplicated in 6+ files with 8 alpha variants — no scrim token; gauge zone
colors local to `gauge.rs:121-132`.

### Type-scale census

Explicit px counts: 9→6, 10→19, 11→20, **12→32**, 13→11, 14→6, 15→3, 16→4,
20→7, 22→2, 24→1, 26→1, 28→3, 34→1, 56→2; helpers 48/32/18/16. 77 sites at
≤12px (critical-text promotion candidates), densest in editor panels
(bindings_panel, lanes_panel, panel.rs, panel_kit) and menu chips. A 5-step
scale must absorb ~21 distinct sizes.

### Dead paths (verified)

| Path | Status |
|---|---|
| `gradient_background_bundle` (`theme.rs:81`) | Dead — zero callers; also misnamed (returns flat bg_bottom) |
| `bg_top` (`theme.rs:41`) | **Not dead** — one consumer, `game-menu/src/startup.rs:29` |
| `ParallaxInfo` (`parallax.rs:21`) | Dead in production — system registered (`lib.rs:112`) but component spawned only in its own test |
| `bevy_tweening::TweeningPlugin` (`lib.rs:103`) | Dead — registration is the only API use; drop plugin + Cargo dep (`dtx-ui/Cargo.toml:17`) |
| FiraMono path: `DEFAULT_FONT_PATH`, `load_font_handle`, `default_text_font`, `pt_to_px`, `absolute_label`, `*_PT` consts (`lib.rs:27-71`) | Dead — zero external callers; everything renders `FontSource::SansSerif` |

### Two toast systems

| | practice/toast.rs | game-menu/import_ui.rs |
|---|---|---|
| Position | top-center, top:56 | top-right, right:24 top:80 |
| Lifetime / cap | 1.5s / 4 | 5s / 5 lines |
| Tone | none | Success/Warn/Error → theme colors |
| Font / bg | 18px / black@0.65 | 16px / stage_panel_bg |
| Z | ui_z::TOAST=1100 | raw `GlobalZIndex(100)` (unregistered) |
| Render | despawn+respawn every frame | rebuild on `is_changed()` |

Unified primitive belongs in dtx-ui (cross-crate consumers).

### Coordinate systems

Ref-px (1280×720 × scale): playfield (`layout.rs`), frame chrome, HUD widgets
via `HudRefRect`, song-select stage. Fixed screen-px: editor chrome
(TAB_BAR_HEIGHT=64, LEFT_PANEL_WIDTH=480, INSPECTOR_WIDTH=240), panel row
metrics, selection-box handles, practice rail 340 (inline), controls kit
SLIDER_WIDTH=110. Split is roughly editor-chrome vs gameplay/menu. Decision
needed: bless the split explicitly (editor = desktop-tool screen-px, game
surfaces = ref-px) or converge; stream 2 already pushes practice HUD to
ref-px.

### Z registry

`ui_z.rs`: PRACTICE 900 · PRACTICE_FULL_HUD/PAUSE 1000 · STAGE_END/TOAST 1100
· PREVIEW_SCRIM 1500 · STAGE_OUTLINE 1900 · BIND_OVERLAY 1910 · EDITOR_CHROME
2000 · SNAP_GUIDES 2050 · HOVER_OUTLINE 2100 · ANCHOR_VIZ 2150 · SELECTION_BOX
2200 · EDITOR_MODAL 2300. game-menu toast (z=100) and full-HUD literal 1000
sit outside it; registry is gameplay-drums-local — cross-crate story needed if
toasts unify.

---

## Stream 5 — Distant-kit pad grammar (research-gated)

### Grammar + contexts today

`verb_for_lane` (`menu_nav.rs:42-51`): HH(0|7)=Up, CY/RD(6|8)=Down,
BD(2)=Confirm, SD(1)=Back, FT(5)=Practice; lanes 3,4,9,10,11 unmapped.
`active_context` (`:102-127`): Title/SongSelect/Result always;
Performance → Editor or Paused only; **Performance+Running, SongLoading,
StageClear/Failed, Startup, End → None** (hits drained unacted).
Guards: DEBOUNCE 80ms, ENTER_GRACE 500ms (`:21-24`).

### MIDI pipeline capabilities

- `MidiEvent::NoteOn { note, velocity, audio_ms }` / `NoteOff { note,
  audio_ms }` / `ControlChange` (`dtx-input/src/midi.rs:77-104`). Channel
  nibble discarded at parse (`:108-128`). Velocity-0 NoteOn → NoteOff.
- Consumer `consume_midi_events` (`gameplay-drums/lib.rs:566-605`) matches
  **NoteOn only — NoteOff and CC silently dropped** (`:576-583`). Velocity
  threshold gate at `:590`. Emits `InputHit` (scoring, only when
  gameplay-ready) + `PadNavHit { lane }` (always) per accepted NoteOn.
- `PadNavHit` carries lane only — no velocity, no timestamp (`lib.rs:518-522`).
- Only per-hit wall-clock retained: global `LastMidiHit { note, velocity,
  below_threshold, at }` overwritten each NoteOn (`lib.rs:422-428`).

Gesture feasibility:

| Gesture | Feasible today? | Needs |
|---|---|---|
| Velocity-conditioned | Yes | nothing (velocity on NoteOn) |
| Double-hit within N ms | Plumbing yes | new per-lane recent-hit ring (audio_ms monotonic in-run) |
| Two-pad chord | Plumbing yes | grouping in drained `buf` (whole FIFO drained per FixedUpdate tick) or audio_ms delta window |
| Held pad (duration) | **No** | consume NoteOff + pair by note number; all raw material parsed, only consumer drops it |

Detector placement: `DrumsSets::Input` before `Judge` (set order
`lib.rs:152-161`), beside `consume_midi_events`; `PadNavHit` split is the
precedent for peeling non-scoring signals off the NoteOn stream.

### False-trigger surface

All 12 lanes are scoring lanes (`lane_map.rs:18-31`); no free pad. Judge
consumes every above-threshold NoteOn as `InputHit`, whiffs become `EmptyHit`
(`judge.rs:154-180`) — an accidental gesture during play costs judgment AND
triggers the gesture. Any live-play gesture must be improbable under real
drumming (hold and specific chord are the plausible candidates; single/double
hits are normal playing vocabulary).

### Keyboard-only chokepoints (full list)

| Chokepoint | Input | Ref |
|---|---|---|
| Pause open | Esc only | `pause.rs:79-90` |
| Loading cancel | Esc only (Parsing/LoadingAudio phases) | `song_loading.rs:483-501` |
| Title quit | Esc×2 (`QuitArm`) | `title.rs:207-211, 17-43` |
| Practice quick controls | `[ ] Bksp - = R T Tab` | `actions.rs:35-66` |
| Practice full-HUD nav/exit | arrows/Enter, double-Enter exit | `full_hud.rs:468-639` |
| Perf hotkeys (speed/offset) | arrows | `perf_hotkeys.rs:184-238` |

Already pad-reachable: title start (BD), results leave (BD/SD), pause menu
once open (HH/CY/BD/SD, SD=resume — `pause.rs:181-277`). The pause grammar is
the pattern to extend.

### Session C protocol (`docs/notes/2026-07-11-player-user-stories.md:978-1001`)

Seven-step pads-only run; steps 3,4,6,7,8 are designed to hit the blockers
(cancel load, pause during play, shape practice, exit practice, quit). Record
first attempted pad, time-to-blocker, throne-leave, hidden-vs-unavailable
belief. Maps 1:1 to F1–F4.

### Audit directions (F1–F4, `2026-07-12-player-ux-audit.md:128-140`)

- F1 pause-from-kit: reserve deliberate low-false-positive gesture; **do not
  ship without hardware testing**.
- F2 practice trap: minimum first step = extend HH/CY/BD/SD grammar into
  practice-paused state (exit + resume) before full rail parity — stream 2's
  pause unification delivers the surface for free.
- F3 quit: SD-hold or SD-at-title with confirm; pairs with existing QuitArm.
- F4 loading cancel: SD during loading = cancel (consistent SD-as-back);
  lowest-risk item — loading is not live play, false-trigger surface near
  zero.

### Sequencing note

F4 (loading cancel) and F2-minimum (pads in practice-paused surface) need no
gesture research — they extend the existing menu grammar into two more
contexts. Only F1 (live-play interrupt) and F3 (quit gesture, if hold-based)
are hardware-research-gated. Stream can split: ship grammar extensions,
gate gesture detector on Session C.

---

## Cross-stream dependencies

- Stream 2 (pause unification) creates the practice-paused pad surface that
  stream 5's F2-minimum needs — do stream 2 first (order already correct).
- Stream 4's type scale and accent decision should land before or with
  stream 1/2 visual work ideally, but streams 1–2 can proceed with current
  theme tokens and be swept later; token stream explicitly includes a sweep.
- Stream 3 and stream 5 both write NavAction consumers; no conflict
  (different contexts: editor vs play states).
- Hold-gesture (stream 5) requires dtx-input consumer change (NoteOff) —
  isolated from all other streams.
