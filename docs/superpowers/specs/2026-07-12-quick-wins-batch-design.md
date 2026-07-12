# Quick-Wins Batch — Design

Date: 2026-07-12
Status: Approved by user (brainstorming session)
Sources: `docs/notes/2026-07-12-player-ux-audit.md` (findings F#), `docs/notes/2026-07-12-player-uxui-design-review.md` (findings U#)

## Goal

Close the highest-value/lowest-effort gaps from the UX audits in one batch: invisible gauge, silent saves, discarded animations, missing legends, small factual bugs, instant title quit, invisible search. Everything here is wiring, small behavior moves, or single-widget work — no screen rebuilds.

Out of scope (own future streams): results screen rebuild, practice full-HUD rail redesign, distant-kit pad grammar, design-token consolidation, Controls/Lanes reducer wiring.

## 1. Gauge widget (U1)

The stage gauge (`StageGauge`, mechanics-only today) becomes a visible HUD widget.

- New `WidgetKind::Gauge` registered like other HUD widgets (Widgets customize list, inspector, anchor/offset/scale/z, per-mode visibility).
- Placement default: horizontal bar spanning the top of the playfield (speaker-bar band), ref-px geometry derived from the playfield layout like other lane-strip-relative elements.
- Visuals: reuse `dtx-ui/src/widget/gauge_bar.rs` (dark `gauge_track`, `gauge_fill` green, OutQuad 150 ms fill ease). Additions:
  - threshold tick at the stage-failure point;
  - fill color switches to `judgment_miss` red while below the threshold.
- Data: reads `StageGauge` each frame.
- Defaults: `show_in_play = true`, `show_in_practice = false` (practice pins the gauge full; showing a frozen bar is noise).
- When Damage Level = None (gauge cannot fail), the widget still renders (it still moves); no special case.

## 2. Save visibility (F5, F6, U8)

### 2.1 Result persistence timing
- Move the persistence body of `save_result_then_despawn` (`game-results/src/lib.rs`) from `OnExit(AppState::Result)` to `OnEnter(AppState::Result)`. Despawn stays on exit.
- Practice runs remain never-persisted (existing guard).
- Results screen appends one bottom status line:
  - success: `saved ✓` in `clear_green`;
  - failure: `save failed — score kept this session only` in `judgment_miss` red.
- The line is part of the existing staggered reveal list (last row).

### 2.2 Editor save failures
- Settings-draft, widget-layout, and profile-registry write failures surface in the existing editor footer bar: the hover-desc slot shows the error in `chrome::ERR` red for ~4 s, then reverts. No new toast system.
- Existing dirty-dialog partial-failure behavior (dialog stays open with failed kinds) is unchanged; the footer message complements it.

### 2.3 Import toast colors
- Import notifications (`game-menu/src/import_ui.rs`) get per-outcome text color:
  - success → `clear_green`;
  - duplicate / filtered-selection-giveup → amber (`select_yellow`);
  - error / unsupported / no-charts / unsafe → `judgment_miss` red.
- Message text unchanged.

## 3. Feedback wiring (U5, U16, U17)

- Combo bounce: `sync_perf_combo` applies `ComboDisplay::scale()` to the combo number's transform (currently computed, never applied).
- Judgment popup: apply the computed grow-on-fade scale (currently bound to `_scale` and discarded) to the popup transform.
- New theme token `judgment_ok` = the score panel's Ok purple `srgb(0.75, 0.45, 0.95)`; `judgment_color()` maps the Ok/Poor label to it; score panel switches to the token (one color, two consumers).
- HitLine height: 3 px × scale at both spawn and layout sync (`hud.rs` spawn site currently 4 px).

## 4. Legends (F8, F12)

- Practice quick tier: one always-visible legend line, bottom-center above the mini strip, in `nav_legend` chip styling with keyboard glyphs:
  `[ ] loop · ⌫ clear · −/= tempo · R restart · T ramp · Tab menu`
  Visible only in practice quick tier (not in the full HUD, not in normal play).
- Title screen: spawn `nav_legend` with `BD start` when MIDI is connected (same gating as other screens).
- Loading screen: static keyboard hint `Esc cancel` (secondary text, near the status line). No pad verb — pad cancel belongs to the distant-kit stream.

## 5. Small bug sweep (U7, U11, F7, F28)

- Loading difficulty chip: use `theme.difficulty_color(selected_tier)` instead of the hardcoded `difficulty_color(2)` red (`song_loading.rs:419`).
- Album art on song select: delete the direct alpha-overwrite path (`song_select.rs:1787-1816` region); the `PreviewSwapEvent` crossfade in `album_art.rs` becomes the single driver.
- Controls `Reset tab` → segment-scoped:
  - visible label becomes `Reset keyboard` / `Reset MIDI` depending on the active segment;
  - keyboard reset clears only the keyboard binding map;
  - MIDI reset clears the MIDI binding map plus device fields (port, velocity threshold);
  - confirmation prompt states exactly what will be reset;
  - existing full-reset test updated to two segment tests.
- Fix stale comment `capture_modal.rs:161-163` (below-threshold hits ARE learnable; comment claims otherwise).

## 6. Esc-twice quit on title (F11)

- First `Esc` on title arms a ~2 s window and shows `press ESC again to quit` (amber, replacing/next to the footer `ESC QUIT` slot).
- Second `Esc` inside the window → `AppState::End` (existing exit flow).
- Any other input, or window expiry, disarms and clears the message.
- Footer static text stays `ESC QUIT` (the armed message is the dynamic part).

## 7. Search input box on song select (U4)

- Replace the bare `type to search…` string (top-right) with a real bordered field:
  - magnifier glyph + placeholder `type to search…` when empty;
  - `theme.accent` border + block caret (`█`, same idiom as profile name dialog) while the query is non-empty;
  - query text replaces placeholder.
- Input behavior unchanged (type-to-filter, Backspace deletes, 64-char cap, cleared on screen re-entry).
- New Esc rule: `Esc` with a non-empty query clears the query; `Esc` with an empty query returns to title. (Prevents accidental exit-to-title while correcting a search.)
- No mouse interaction on the field in this batch (song select remains keyboard-driven).

## Testing

Logic gets unit tests; visuals get a BRP smoke drive.

- Save-on-entry: entering Result persists; exiting does not double-persist; practice still skipped; failure path sets the failed-status for the results line.
- Segment-scoped reset: keyboard reset leaves MIDI map + device fields untouched; MIDI reset leaves keyboard map untouched and clears port/threshold.
- Esc-twice: armed window state machine (arm, fire, expire, disarm-on-other-input).
- Search Esc rule: non-empty query → cleared, state stays SongSelect; empty query → Title.
- Gauge widget: layout/visibility defaults (play on, practice off); threshold color switch at boundary.
- Visual verification (gauge, combo bounce, popup scale, legends, chip color, search box, import toast colors): launch via BRP smoke drive and observe.

## Error handling

- Result save failure: red status line on results (2.1); score for the session still shown from in-memory data.
- Editor save failure: red footer message (2.2); drafts stay dirty as today.
- No new failure modes introduced; all changes are UI-side or timing moves of existing persistence calls.

## Sequencing note

Items are independent; any order works. Suggested: 3 (feedback wiring, smallest) → 5 (bug sweep) → 2 (save visibility) → 1 (gauge) → 4 (legends) → 6/7 (title + search).
