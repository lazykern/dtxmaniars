# DTXManiaRS Player UX & Behavioral Audit

Date: 2026-07-12
Status: Evidence-backed audit. No redesign performed.
Inputs:
- `docs/notes/2026-07-11-player-manual-current-behavior.md` (factual baseline)
- `docs/notes/2026-07-11-player-user-stories.md`
- Source verification pass (three targeted sweeps over `gameplay-drums`, `game-menu`, `game-shell`, `game-results`, `dtx-input`, `dtx-layout`, `dtx-persistence`)
- 13 live 1920×1080 screenshots (title, song select, gameplay, pause, practice full HUD, all seven Customize tabs) — partially closes the manual's "unverified visually" gap

Companion: `2026-07-12-player-uxui-design-review.md` — per-screen visual/interaction design review (typography, color, motion, affordances) with 20 additional UI findings (U1–U20), including the highest-severity one found anywhere: no gauge is rendered during normal play while stage failure defaults on.

Verification result up front: **every questionable manual claim checked against source was confirmed.** The manual is a reliable baseline. The audit below therefore treats manual statements corroborated by source as Confirmed. Two source-level discrepancies were found (stale comment in `capture_modal.rs:161-163`; Esc-precedence ordering in `ui.rs:283` vs the documented hierarchy) — both are code-hygiene notes, not player-visible contradictions.

---

## 1. Executive assessment per player type

### Player 1 — Keyboard and mouse

**Overall: functional and efficient once learned; discoverability and persistence confidence are the weak axes.**

The complete loop (launch → browse → play → pause → results → practice → customize → quit) is reachable without ever leaving the desk. Navigation is keyboard-first with hidden-shortcut density typical of rhythm games (`Tab` sort, `F5`/`F6`, `Shift+Enter`, practice `[ ] - = R T`). The bottom hint bar on song select (screenshot evidence) advertises the main verbs, but practice quick keys have **zero** on-screen representation (`practice/actions.rs:37-46`; only post-hoc toasts exist). The two most dangerous behaviors for this persona are silent: result persistence happens only on leaving Results (`game-results/src/lib.rs:45`), and settings/layout/profile save failures produce log lines, never UI (`tabs.rs:72-74`, `save.rs:59,74`). Plain arrow keys mutate scroll speed and input offset during live play and persist within ~200 ms (`perf_hotkeys.rs:197-223, 34`).

### Player 2 — MIDI kit, computer nearby

**Overall: best-served persona. Kit covers the frequent loop; the computer absorbs everything else at natural break points — except pausing, which breaks posture mid-performance.**

Two-level pad navigation (wheel/difficulty) with debounce and entry-grace is solid and legend-supported when MIDI is connected (`song_select.rs:1536-1587`, `menu_nav.rs:22-24`). Pause menu, results, and four Customize settings tabs are pad-operable. The single mid-performance handoff — reaching for `Esc` to pause — is exactly the moment when hands hold sticks. Setup tasks (binding capture, threshold, port, lanes) are well-instrumented at the computer: velocity meter, below-threshold amber, shared-binding markers, live lane highlight.

### Player 3 — MIDI kit, computer out of reach

**Overall: the persona the current build cannot honestly support for a full session. Four hard blockers, all confirmed in source.**

The kit can run the happy path (title → browse → difficulty → play/practice → results → title). Everything off the happy path requires the computer: cannot pause a running song (`pause.rs:79-90` + `menu_nav.rs:116-124` returns no pad context during live play), cannot cancel loading (`song_loading.rs:477-495`), cannot control or exit practice (all practice input is `KeyCode`-only, `actions.rs:56-66`, `full_hud.rs:491-547`), cannot quit the app (`title.rs:124-127` reads only pad Confirm). The floor-tom practice entry is a trap for this persona: it starts an infinite whole-song loop (`ab_loop.rs:35-67`) that the kit can neither shape nor leave. Title and loading screens show no pad legend at all (`title.rs`, `song_loading.rs` spawn no `nav_legend`), so the session's first and most anxious moments are also its least guided.

---

## 2. End-to-end journey maps

Column key — **See**: what must be noticed. **Know**: what must be understood/remembered. **Do**: physical actions (K=keyboard, M=mouse, P=pad). **Wait**: imposed delays. **Switch**: device/posture change. **Recover**: cancel/undo path.

### 2.1 Keyboard/mouse player

| Journey | See | Know | Do | Wait | Switch | Recover |
|---|---|---|---|---|---|---|
| First launch | Splash → title, `PRESS ENTER`, tiny footer `F1/F2/ESC` | Title is not clickable despite button-styled `PRESS ENTER` | K: `Enter` | ~0.5 s splash | — | `Esc` = quit (immediate, no confirm) |
| Empty library + import | Empty-state hints (F5/F6/drop) | RAR shown in picker but rejected; search can hide the import | K: `F6`; M: pick files, or drag-drop | Extract + rescan | K→M | Notification per outcome; clear search manually if filtered |
| Title → song select | Song wheel, bottom hint bar | Wheel is keyboard-only | K: `Enter` | fade | — | `Esc` back to title |
| Browse/search/sort/difficulty | Wheel, difficulty slots, density graph, history, `SORT:` chip | `Tab`=sort, typing=search (no visible search box until typed), `←/→`=difficulty | K: arrows, type, `Tab` | — | — | `Backspace` deletes search char; search clears on re-entry |
| Start + cancel loading | Progress/status text | `Esc` cancels | K: `Enter`, `Esc` to cancel | Parse+audio prep | — | Failure auto-returns, unexplained |
| Normal performance | Chips, judgment, gauge, HUD | Arrow keys are LIVE (speed/offset); `F11` overlay | K: drum keys | — | — | `Esc` pause |
| Pause/retry/quit/fail/results | PAUSED menu; banner; results | Banner auto-advances 1.6 s; rank ≠ score-only | K: arrows + `Enter`/`Space`; `Esc` resumes | 1.6 s banner unless skipped | — | Full: resume/retry/quit |
| Leave results (save) | Results panel; **no save indicator** | Save fires only on LEAVING results | K: `Enter`/`Esc` | — | — | None: close-on-results loses play; save failure silent |
| Practice enter/control/exit | Playfield + mini loop strip (no text) | `[ ] - = R T Tab` from memory; whole-song loops forever; tempo changes pitch | K: quick keys; full HUD: arrows+`Enter`; M: timeline seek/drag | Pre-roll/count-in | K↔M optional | `Backspace` clears loop; Exit = rail row 18, Enter twice |
| Binding setup (KB+MIDI) | Controls tab, segment toggle, chips, `no binding` amber rows | Two independent profiles (Keyboard vs MIDI); reserved keys ignored during capture | M: click segment/`+`/Confirm; K: press key; P: hit pad (MIDI capture) | — | M→K→P during MIDI capture | `Esc` cancels capture only; chip `x` removes one claim |
| MIDI port/threshold/verify | Device card, port, velocity meter, amber below-threshold | Threshold blocks gameplay, not capture | M: port arrows, Rescan, threshold ± (1 per click) | reconnect attempts | M+P | Rescan; threshold is click-per-unit (0–127!) |
| Conflicts/shared/removal | `Add shared`/`Move here` in modal; shared markers | Shared key fires ALL owners at runtime | M/K: choose + Confirm | — | — | Cancel in modal |
| Profile management (KB/MIDI/Lanes) | Profile bar, amber dirty dot, overflow `…` | Built-ins immutable; Save on dirty built-in silently becomes a named copy | M: Save/Save As/…; K: type names | — | — | Dirty guard on switch/close; Revert via `…` |
| Lane editing | Rows, detail card, preview pads, `Hidden` strip | Rows select-only; **preview pads are the drag surface**; hidden lanes still judged | M: drag pad body (reorder) / edge (resize), click chips | — | M only | `Ctrl+Z`/`Ctrl+Y`; final lane can't be hidden |
| Settings tabs + widgets | Tab strips, steppers/sliders, footer hints | Settings save on close; layout via `Ctrl+S`; both fail silently | K: PgUp/Dn, arrows; M: click/drag; widgets: M select, K nudge | — | K↔M | `RESET TAB` / `Reset Widget`; undo for layout only |
| Close Customize (dirty states) | `Cancel / Discard all / Save all` dialog | `Enter`=Save, `Esc`=Cancel, **Discard is mouse-only** | K or M | — | K→M for Discard | Failed save keeps dialog with only failed kinds |
| Quit app | `Thanks for playing` | Title `Esc` exits with no confirmation | K: `Esc` on title | ~1 s | — | None once pressed |

### 2.2 Nearby-MIDI player (deltas from 2.1)

| Journey | Kit portion | Computer portion | Switch cost |
|---|---|---|---|
| Title → play | P: BD advance; HH/CY browse; BD difficulty; HH/CY choose; BD start (legend shown when connected) | — | None |
| Search/sort/import/rescan | None | K/M as 2.1 | Natural-break switch |
| Cancel loading | None | K: `Esc` | Mid-wait reach |
| Pause during play | **None** | K: `Esc` | **Mid-performance reach, sticks in hand** |
| Operate open pause | P: HH up / CY down / BD select / SD resume (legend shown) | K fallback | None once open |
| Results | P: BD/SD leave | — | None |
| Practice | P: FT starts | K/M for A/B, tempo, restart, HUD, exit | Kit→desk each adjustment; resume requires rail/transport, then back to kit |
| Settings after open | P: 4 tabs navigable (HH/CY/BD/SD grammar, legend shown) | K: `F1` to open; M for Controls/Lanes/Widgets | Open requires desk |
| Calibration | P: 12 taps | M: click `Calibrate`; K: `Enter`/`Esc` apply/cancel | Desk→kit→desk sandwich |
| Binding capture | P: hit pad, same-note reconfirm | M: `+`, shared choice, Confirm | Desk-centric with pad moments |

### 2.3 Distant-MIDI player (kit-only phase)

| Journey | Kit action | Outcome | Reachability |
|---|---|---|---|
| Title advance | BD | Song select | ✅ (no legend on title, though) |
| Browse/difficulty/start | HH/CY, BD, FT | Full happy path | ✅ |
| Correct wrong song | SD | Back a level | ✅ |
| Cancel loading | — | none exists | ❌ **Blocker** |
| Pause running song | — | none exists | ❌ **Blocker** |
| Operate pause (if opened by someone) | HH/CY/BD/SD | Works | ✅ conditional |
| Skip clear/fail banner | — | wait 1.6 s | ⚠️ auto-advance saves it |
| Leave results | BD/SD | Save attempt triggered | ✅ (silent on failure) |
| Practice: shape/exit | — | infinite loop, no exit | ❌ **Blocker** |
| Search/sort/import | — | none | ❌ setup-time only |
| Quit app | — | none | ❌ **Blocker** |

---

## 3. Cross-persona reachability matrix (source-verified)

Legend: ✅ direct · 🔁 fallback device · ❌ unreachable · 🛠 setup-time only

| Action | KB/M | Nearby MIDI | Distant MIDI | Evidence |
|---|---|---|---|---|
| Advance title | ✅ | ✅ kit | ✅ kit | `title.rs:123-150` |
| Browse songs / difficulty | ✅ | ✅ kit | ✅ kit | `song_select.rs:1657-1740` |
| Search / sort / rescan / import | ✅ | 🔁 | ❌ | `song_select.rs:1590-1625`, `import_ui.rs:103-125` |
| Start play / practice | ✅ | ✅ kit | ✅ kit | `song_select.rs:1704-1716` |
| Cancel loading | ✅ | 🔁 | ❌ | `song_loading.rs:477-495` |
| Open pause during play | ✅ | 🔁 | ❌ | `pause.rs:79-90`, `menu_nav.rs:116-124` |
| Operate open pause | ✅ | ✅ kit | ✅ kit | `pause.rs:238-276` |
| Skip clear/fail banner | ✅ | 🔁 | ❌ (wait) | `stage_end.rs:143-153` |
| Leave results → save attempt | ✅ | ✅ kit | ✅ kit | `game-results/src/lib.rs:281-295` |
| Confirm save success | ❌ | ❌ | ❌ | no UI in any save path |
| Practice A/B / tempo / restart / HUD / exit | ✅ | 🔁 | ❌ | `actions.rs:37-66`, `full_hud.rs:491-547` |
| Calibrate | ✅ | 🔁+pad taps | 🛠 | manual §14; start is mouse-click |
| Gameplay/Audio/Drums/System (editor open) | ✅ | ✅ pads | 🛠 | `keyboard_nav.rs:32-39` |
| Controls / Lanes / Widgets content | ✅ (mouse-led) | 🔁 | 🛠 | `pad_can_enter` exclusion, ibid. |
| Lane reorder/resize | M-only drag | 🔁 | 🛠 | `lane_drag.rs:130-186`; reducer unwired `lane_drag.rs:4` |
| Dirty-dialog Discard | M-only | M-only | 🛠 | `close_dialog.rs:169-178` |
| Quit from title | ✅ | 🔁 | ❌ | `title.rs:124-127` |

---

## 4. Findings ordered by severity

Format per finding: severity · personas · journey/step · required action · current behavior · consequence · evidence · research question · direction.

### Blockers (all distant-MIDI)

**F1 — Cannot pause a running song from the kit.**
Journey: performance → interruption. Required action: none exists; player must leave the throne for keyboard `Esc`. Behavior: pad→nav mapper yields no context during `Performance`+`Running` (`menu_nav.rs:116-124`, test `:236-256`); pause toggle reads only `KeyCode::Escape` (`pause.rs:79-90`). Consequence: interruptions force abandoning posture or letting the song run unattended; misses accumulate against a default-on failure rule. Research question: which pad gesture (double-hit? held pad? rim chord?) can open pause with an acceptably low false-trigger rate under real playing? Direction: reserve a deliberate low-false-positive kit gesture for pause; do not ship one without hardware testing.

**F2 — Practice is a kit trap: FT enters an infinite loop the kit cannot shape or leave.**
Journey: practice, every step after entry. Behavior: FT at difficulty level starts practice (`song_select.rs:1704-1716`); all practice actions are `KeyCode`-only (`actions.rs:56-66`); full HUD rail is keyboard-only with double-press exit (`full_hud.rs:491-547, 620-627`); whole-song span loops forever (`ab_loop.rs:35-67`). Consequence: distant player must walk to the computer or kill the app externally; likely learns to avoid practice entirely (hypothesis H4). Evidence chain fully confirmed. Research question: what minimal practice verb set (restart, tempo ±, exit) do drummers actually need mid-session from the throne? Direction: extend the existing HH/CY/BD/SD grammar into the practice-paused state at minimum (exit + resume), before attempting full rail parity.

**F3 — Cannot quit the application from the kit.**
Journey: session end. Behavior: title consumes only pad `Confirm` (`title.rs:124-127`); quit is keyboard `Esc` → `AppState::End` → `process::exit` (`title.rs:135`, `end.rs:49-56`). Consequence: kit-only session cannot be completed; contrast with F11 (keyboard quit is arguably *too* easy). Research question: is a pad quit needed, or is "leave it on title" acceptable for the persona? Direction: candidate SD-hold or SD at title = quit-with-confirm; pairs naturally with fixing F11.

**F4 — Cannot cancel loading from the kit.**
Journey: start → wrong chart. Behavior: `watch_cancel_key` reads only `Escape` (`song_loading.rs:477-495`); `SongLoading` maps to no pad context (`menu_nav.rs` test `:287-296`). Consequence: a mis-start forces waiting through the load and then quitting via pause — except F1 means the kit can't open pause either; the player must play or fail out. Research question: how often do distant players mis-start (two-level nav should make it rare)? Direction: SD during loading = cancel; consistent with SD-as-back everywhere else.

### High

**F5 — Result persistence is invisible and timing-fragile.**
Personas: all. Journey: results → leave. Behavior: save runs only in `OnExit(AppState::Result)` (`game-results/src/lib.rs:45, 297-377`); nothing on entry; no success/failure UI; write failure only logs. Consequence: closing the app while admiring a score silently loses it (hypothesis H5); a failed write is indistinguishable from success. Evidence: confirmed. Research question: do players linger on results and close from there? Direction: persist on results *entry* (or immediately at stage end), keep exit as navigation only; surface failure as a toast. This is the single highest-value persistence-confidence fix.

**F6 — Every non-profile save path fails silently.**
Personas: all. Journeys: closing Customize (settings draft), `Ctrl+S`/close (widget layout). Behavior: failures produce `error!`/`warn!` logs only (`tabs.rs:72-74`, `mod.rs:194-196`, `save.rs:59, 74`); session keeps showing edited values. Consequence: player believes changes persisted; discovers loss next launch with no cause visible. Note the asymmetry: *profile* saves get a retry dialog (strength S4), settings/layout get nothing. Research question: none needed — behavior is confirmed and consequence is mechanical. Direction: reuse the existing toast/notification channel for failed writes.

**F7 — Controls `Reset tab` silently wipes both segments plus device fields.**
Personas: KB/M, nearby MIDI. Journey: binding setup → reset. Behavior: reset assigns `InputBindings::default()` wholesale — keyboard map, MIDI map, port, threshold (`bindings_panel.rs:680-682`, test `:1057-1067`) — while the UI shows only one segment. Consequence: a player resetting "the keyboard tab" destroys their MIDI mapping and device tuning; confirmation dialog exists but its scope statement is the button label. Research question: do players read `Reset tab` as segment-scoped? (Almost certainly — the segment toggle dominates the panel.) Direction: scope reset to the active segment, or rename + enumerate what will be lost in the confirm prompt.

**F8 — Practice quick controls are memory-only.**
Personas: KB/M, nearby MIDI. Journey: practice while playing. Behavior: `[ ] - = R T Tab` bindings exist solely in code (`actions.rs:37-46`); the quick-tier HUD is a textless mini strip (`mini_strip.rs`); toasts confirm actions only *after* the key is pressed. Consequence: first-time players get a looping song with no visible controls; the feature set (A/B, ramp, wait) is undiscoverable without the manual. Research question: which subset belongs on-screen vs. behind the full HUD? Direction: one legend line in practice quick mode, mirroring the pause-menu legend pattern that already exists.

**F9 — Corrupted profile registry recovers silently; the recovery dialog is dead code.**
Personas: all. Journey: launch with damaged data. Behavior: `open_corrupt_reset`/`RegistryHealth` never called in production (`profile_dialog_ui.rs:73-74` carries the deferral comment); corruption logs `error!` and falls back to read-only built-ins (`bindings.rs:170-173`). Consequence: player's custom profiles vanish from the selector with zero explanation; user cannot distinguish data loss from a UI change; Save paths error confusingly against `ReadOnlyBuiltins`. Research question: none — wiring gap is confirmed. Direction: wire startup detection to the already-built dialog.

**F10 — Play Speed is a desync hazard presented as an ordinary setting.**
Personas: all. Journey: Gameplay tab. Behavior: `play_speed` compresses chart time only (`resources.rs:229-261`, `dtx-core/src/timing.rs:29-53`); audio plays at native rate; no warning in the row UI (screenshot: plain `< 1.00x >` stepper; footer shows only generic hover text). Consequence: any value ≠1.0 silently desynchronizes chart from music — the one thing a rhythm game must not do — and the player will blame their timing or the chart. Research question: is chart-only speed useful to anyone as-is? Direction: warn inline, or gate the row until audio time-scaling exists.

### Medium

**F11 — Title `Esc` exits the app instantly, against the game's own Esc grammar.**
Everywhere else `Esc` means back/cancel/close; from song select it returns to title, so two reflexive `Esc` presses from song select terminate the program (`title.rs:135`, `end.rs:49-56`, no confirm). Nothing is lost (saves are elsewhere), but session termination as a reachable reflex misfire is a consistency/error-prevention defect. Direction: confirm step or hold-to-quit; solves F3 jointly if pad-reachable.

**F12 — Title and loading screens show no pad legend.**
`nav_legend` is spawned on song select, pause, results, editor — but not title or loading (verified by absence). The distant player's first action (BD) and first dead-end (loading) are unguided. Direction: extend the existing legend widget; trivial consistency win.

**F13 — Live arrow keys during performance mutate persisted settings.**
Plain `↑/↓/←/→` change scroll speed and input offset mid-song and persist within ~200 ms (`perf_hotkeys.rs:197-223, 34, 240-251`). Convenient for veterans; an accidental brush permanently shifts calibration with only a transient on-screen readout. Research question: accidental-hit frequency on real keyboards. Direction: keep, but consider requiring a modifier for offset (the calibration-adjacent one), or an undo toast.

**F14 — Dirty-close `Discard` is mouse-only.**
`Enter`=Save, `Esc`=Cancel, but Discard has no key and no focus traversal (`profile_state.rs:394-402`, `close_dialog.rs:169-178`). A keyboard-flow player who wants to throw away edits must reach for the mouse — inverted friction (destructive action *harder* is defensible, but full keyboard users are stranded, and the pad-navigation story stops at this dialog too). Direction: `←/→` focus movement + `Enter`, keeping Save as default focus.

**F15 — Practice full HUD renders over other HUD elements with no clipping/reflow.**
No clip or layout negotiation exists; overlap is z-order only (`full_hud.rs:262-287`). Screenshot confirms rail text colliding with the Now Playing card and value rows wrapping mid-token (`Ramp start ◄x0.70 ►` across two lines) at 1080p. The most information-dense surface in the game is its least legible. Direction: give the rail an opaque panel and reserve its column; audit at 1080p.

**F16 — Import success can be visually silent under active search.**
Import succeeds, rescan runs, but an active search filter can hide the imported folder and the auto-select gives up without a message (manual §6, confirmed pipeline). Player conclusion: "import failed." Direction: when auto-select fails due to filter, extend the existing notification: "imported — clear search to view."

**F17 — Primary menus are keyboard-only while the title renders a button-styled `PRESS ENTER`.**
Screenshot shows a filled, glowing button — the strongest click affordance in the game — that does nothing on click (`title.rs` has no `Interaction` handlers). Same for song wheel rows, pause rows, results. Hypothesis H1 (first-time users click first) is near-certain but unmeasured. Direction: either accept clicks on the few obvious targets or de-button the visual.

**F18 — Controls/Lanes/Widgets keyboard+pad navigation exists as reducers but is unwired.**
`reduce_controls_nav` (`controls_panel.rs:115`) and `reduce_lanes_nav` (`lanes_panel.rs:102`) are test-only; `pad_can_enter` excludes these tabs (`keyboard_nav.rs:32-39`). Lane reorder/resize is preview-drag-only (`lane_drag.rs:130-186`), rows are select-only. Consequence: mouse is mandatory for input/lane setup (LANE-3); also an accessibility ceiling. Direction: finish the deferred wiring — the hard part (reducers + tests) already exists.

**F19 — Score-history load failure yields silently empty history.**
Manual §22, uncontested. Player cannot distinguish "no plays" from "history unreadable." Direction: one-line degraded-state notice on song select when the store failed to load.

**F20 — Esc semantics fork inside practice.**
In normal play `Esc` = pause menu with Resume/Retry/Quit; in practice `Esc` = full HUD where "quit" is rail row 18 with a double-press (`hud/mod.rs:32-33`, `full_hud.rs:620-627`). Same key, same phase of play, different mental model — and no Retry/Quit-to-songs verbs at all in practice. Direction: align exit affordances across the two pause surfaces.

**F21 — Threshold and offset controls are hostile at range extremes.**
Velocity threshold moves 1 unit per click over 0–127 (`Controls-tab mouse actions`, manual §18); widget offsets accept extreme values (screenshot shows `offset x -6666` accepted). Clamping/step behavior at extremes unverified. Research gap + friction. Direction: drag/type entry for threshold; verify offset clamps.

### Low

**F22 — No pad skip for the clear/fail banner** (`stage_end.rs:143-153`); auto-advance in 1.6 s caps the cost. Direction: accept BD, trivially consistent with results.
**F23 — Stage failure rule is on by default with no settings row** (manual §14); player can't see or change the rule that ends their song. Direction: add the row.
**F24 — BGA placeholders can read as intended chart art** (manual §21). Direction: subtle "no media" treatment instead of colored blocks.
**F25 — Immediate lane hit on binding confirm** (`bindings_capture.rs:293-310`) — deliberate verification affordance, but it fires sound/visuals uncommanded; startle vs. utility unknown. Keep + observe.
**F26 — Calibration can only be started by mouse click** (no keyboard activation of `Calibrate`); apply/cancel are keyboard-only (`Enter`/`Esc`) — a K→M→K sandwich inside one flow.
**F27 — Dirty state is an amber dot; below-threshold is amber fill** — meaning carried by color alone in at least two places (manual §18). Accessibility: pair with a glyph/text.
**F28 — Stale code comment**: `capture_modal.rs:161-163` claims below-threshold hits never reach capture; they do (`lib.rs:583-588`, `strictly_new_note`). Player-facing behavior is correct; fix the comment before it misleads a future change.

### Strengths (behavior already serving players well)

- **S1 — Pad menu grammar with debounce + entry grace** (80 ms / 500 ms, `menu_nav.rs:22-24`): the enter-hit can't double-fire into the next screen. Two-level wheel prevents accidental starts.
- **S2 — Contextual pad legends** on song select, pause, results, editor, shown only when MIDI is connected — right pattern, needs only wider coverage (F12).
- **S3 — Dirty-close protection with Save as default** (`Enter`=Save), and **failed saves keep the dialog with only failed kinds pending** (`profile_state.rs:352-359, 406-423`) — genuinely good partial-failure semantics.
- **S4 — Built-in immutability + auto-named copy on save** (`mod.rs:441-458`): impossible to destroy factory defaults.
- **S5 — Import pipeline**: unsafe-path rejection, duplicate avoidance, wrapper-folder handling, rescan + cursor jump, per-outcome notifications.
- **S6 — Capture machine**: stale-hit rejection, same-note reconfirm, different-note replace without reopening, below-threshold learnable-but-gameplay-gated, live velocity meter — CTRL-1/2/3 are substantially satisfied at the computer.
- **S7 — Lane model safety**: hidden lanes stay judgeable (`lane_edit.rs:94-116`), last lane can't be hidden, one undo stack spans lanes+widgets, merge chooser excludes other primaries.
- **S8 — Practice training semantics**: ramp step-down on two fails, disarm on manual interference, wait/ramp exclusivity, gauge pinned, never pollutes score history.
- **S9 — Perf-hotkey persistence** debounced + flushed on exit — changes survive without a save ritual (double-edged; see F13).
- **S10 — Remembered song/difficulty selection** and live search make the returning-player loop fast.

---

## 5. Device/posture-switch inventory

| # | Switch | Trigger | Persona cost (KB/M · Nearby · Distant) |
|---|---|---|---|
| 1 | Kit → keyboard, mid-performance | Pause (`Esc`) | n/a · **high (sticks in hand)** · **impossible → blocker** |
| 2 | Kit → keyboard, mid-wait | Cancel loading | n/a · medium · **blocker** |
| 3 | Kit → keyboard/mouse | Practice A/B, tempo, restart, HUD, exit | n/a · medium, repeated per adjustment · **blocker** |
| 4 | Kit → keyboard | Search / sort / rescan / import | n/a · low (natural break) · unavailable |
| 5 | Kit → keyboard | Open Customize (`F1`) | n/a · low (setup) · setup-only |
| 6 | Keyboard → mouse | Discard in dirty dialog | low but mandatory · same · setup-only |
| 7 | Keyboard → mouse | Start Calibrate; Reset tab; profile bar; all of Lanes/Widgets | low, frequent in setup · same · setup-only |
| 8 | Mouse → keyboard | Calibrate apply/cancel; profile name entry | low · low · setup-only |
| 9 | Desk → kit → desk | Calibration sandwich (click → 12 pad taps → Enter) | n/a · medium · setup-only |
| 10 | Kit → keyboard | Quit from title | n/a · low · **blocker** |

Pattern: for the nearby persona, all switches except #1 land at natural breaks — consistent with hypothesis H2. #1 is the outlier and the priority. For keyboard users the churn is #6–8: single-flow K↔M sandwiches inside Customize.

---

## 6. Highest-risk behavioral hypotheses

Ranked by (likelihood × damage), building on H1–H7 of the stories doc:

1. **H5+ (save loss will actually happen).** Results is exactly where players linger — screenshotting, recording. Save-on-exit means the celebration posture is the data-loss posture. Compounded by silent write failure. *Risk: real data loss + trust collapse.*
2. **H4+ (practice trap teaches avoidance).** Distant players who once get stuck in an unexitable pitch-shifted loop will not try practice again — the feature built for drummers becomes drummer-hostile. *Risk: core feature abandonment.*
3. **H8 (new): practice features go unfound entirely.** With zero on-screen hints (F8), keyboard players may never learn A/B/ramp/wait exist; practice reads as "the mode where the song repeats." *Risk: invisible feature investment.*
4. **H9 (new): Reset tab causes real mapping loss.** A player resetting the keyboard segment wipes hours of MIDI threshold/mapping work; recovery requires re-capture. *Risk: rage event, distinct from silent failures because the player did it "to themselves."*
5. **H1 (click-first on title).** `PRESS ENTER` styled as a button nearly guarantees first-click failure; cost is seconds, but it is the first impression.
6. **H6 (calibration trust).** Median-of-12 with no spread indication; pad strikes have higher variance than key taps.
7. **H7 (throne-distance readability).** Screenshots show legends/footers in small type at 1080p; at 2–3 m the pad legends — the distant persona's lifeline — are likely subreadable. Untestable from source; must be observed.

---

## 7. Prioritized usability-testing plan

Reuses sessions A–E from the stories doc, reordered by risk and sharpened:

1. **P1 — Distant-kit interruption & practice trap (Session C).** Instrument time-to-first-blocked-attempt, first pad tried for pause, whether the participant leaves the throne or abandons. Add: does the participant believe pause exists but is hidden? Feeds F1–F4 gesture design.
2. **P2 — Results-save awareness (fold into A & E).** After a play, ask "is your score saved?" while still on Results; then have them close the app from Results in a sacrificial run. Directly tests H5 before/after any F5 fix.
3. **P3 — Practice discoverability, keyboard (Session A step 9 expanded).** Give the task "practice bars 12–16 at 0.75×" with no coaching; measure discovery of `[`/`]`/`-` or full HUD, and Exit comprehension (double-press). Tests F8, F20, H8.
4. **P4 — Controls comprehension (new session).** Tasks: swap a MIDI note, create a shared key, reset only keyboard bindings (observe F7 damage), close with dirty edits choosing Discard (observe F14 mouse reach), pull the MIDI cable mid-session. Tests F7, F14, S6 comprehension, CTRL-1..4.
5. **P5 — First-contact click behavior (Session A steps 1–3).** Screen-record mouse; count clicks on title button/wheel/pause rows. Tests H1/F17.
6. **P6 — Throne-distance readability (Session D step 4).** Legends, difficulty labels, judgment popup, pause menu at 2.5 m and 3.5 m on a TV. Tests H7; gates any distant-persona investment.
7. **P7 — Calibration confidence (Sessions A/B step comparison).** Keyboard vs pad sample spread; ask whether the suggested offset is trusted and which direction they think it shifts. Tests H6, KM-F4.

---

## 8. Improvement opportunities (direction only, no screen design)

### 8.1 Small interaction/content corrections
- Persist results on entry/stage-end; toast on save failure (F5).
- Failure toasts for settings/layout/profile writes via existing notification channel (F6).
- One-line quick-practice legend; reuse `nav_legend` pattern (F8).
- Pad legend on title + loading (F12).
- BD skips banner; SD cancels loading (F22, F4-cheap-part).
- Keyboard focus for Discard (F14).
- Scope or relabel Controls reset + enumerate losses in confirm (F7).
- Quit confirmation / hold-Esc on title (F11).
- "Imported — clear search to view" notification variant (F16).
- Inline warning on Play Speed ≠ 1.0 (F10 stopgap).
- Stage-failure settings row (F23); degraded-history notice (F19); fix stale capture comment (F28); pair color signals with glyphs (F27).

### 8.2 Workflow restructuring
- Unify pause semantics across normal play and practice: one Esc surface with Resume/Retry/Quit(+practice verbs) (F20).
- Results screen: add Retry / Practice-this handoff so failure recovery doesn't require the full wheel round-trip (distant failure story).
- Calibration flow operable end-to-end from one device (F26).
- Practice exit: replace double-Enter-on-row-18 with the unified pause surface.

### 8.3 Features required for reachability (distant persona)
- Kit gesture to open pause during play (F1) — the keystone; unblocks retry/quit mid-song.
- Kit-reachable practice minimum set: exit, restart, tempo (F2), plausibly by extending the HH/CY/BD/SD grammar inside practice-paused.
- Kit cancel on loading (F4) and kit quit path (F3).
- Wire the existing `reduce_controls_nav` / `reduce_lanes_nav` reducers to panels and pads (F18) — implementation exists, only integration is missing.

### 8.4 Needs player research before redesign
- The pause gesture itself: any pad input during live play risks false triggers; choose from P1 + hardware testing, not from the armchair.
- Distant readability targets (P6) before resizing anything.
- Whether immediate-hit-on-confirm (F25) reads as verification or defect.
- Calibration confidence presentation (spread/confidence vs. bare median) (P7).
- Whether BD's meaning shift between wheel and difficulty levels confuses (H3).
- Threshold-setting ergonomics with real soft-hit workflows (F21).

---

## 9. Conclusion — change classification

**No change needed** — import pipeline (S5); pad menu grammar + debounce (S1); pause-menu pad operation; profile immutability/copy model (S4); dirty-guard save semantics (S3); capture machine core (S6); lane-model safety rules (S7); practice training semantics (S8); scoring/judgment feedback loop.

**Refinement** — save-failure visibility (F5/F6 toasts); legends coverage (F8, F12); banner/loading pad verbs (F22, F4); Discard keyboard access (F14); Reset-tab scope (F7); title quit guard (F11); import-under-search notice (F16); Play Speed warning (F10); accessibility pairings (F27); small-content items F19/F23/F24/F28.

**Partial redesign** — results persistence timing and results-screen verbs (F5 + retry handoff); practice pause/exit surface and full-HUD layout (F2-keyboard-side, F15, F20); Controls/Lanes/Widgets input wiring — finishing the deferred reducer integration is closer to completion than redesign, but the interaction model needs a pass once keyboard/pad paths exist (F18); calibration flow (F26).

**Fundamental redesign** — the distant-kit session model. F1–F4 are not four patches; they are one missing design layer: a kit-input grammar for *interrupting and steering* a session, not just navigating menus. It must be designed against false triggers with hardware in the loop (P1, P6 first). Until then, the honest posture is the one the manual already takes: the distant persona is Limited, and the title/first-run experience should say so before the player sits down (DM-F3).

### Priority order if work started tomorrow
1. F5 (results persistence) — data loss, all personas, small fix.
2. F6 (save-failure toasts) — trust, all personas, small fix.
3. F8 + F12 (legends) — discoverability, near-free given `nav_legend` exists.
4. F7 (reset scope) — destructive mislabel, small fix.
5. P1 research → F1 gesture → F2/F3/F4 grammar extension — the distant-persona arc, gated on research.
6. F18 (wire reducers) — unlocks keyboard/pad parity for setup surfaces.
