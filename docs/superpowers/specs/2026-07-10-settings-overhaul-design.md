# Settings Overhaul — Design

Date: 2026-07-10
Branch: `feat/settings-overhaul`
Scope: the Customize surface (in-Performance settings/layout overlay, entered F1/F2). All settings live in `crates/gameplay-drums/src/editor/` + `crates/dtx-config/`.

## Motivation

An in-game audit of the Customize surface (all 7 tabs, real screenshots via BRP) surfaced correctness bugs, a silent data-loss path, and settings that persist but do nothing until the next song. This overhaul fixes the bugs and closes the "looks broken" gaps. BGA/movie/dark/reverse/fill-in toggles are explicitly **out of scope** — those config fields have no runtime system behind them (they are Phase-1 placeholders); exposing a toggle without a renderer/behavior would be lying UI.

## The two persistence + apply contracts (current state)

```
                    save-on-close        apply-while-open
config.toml   ──────  auto (Esc)   ──────  PARTIAL  (only scroll/offsets/volumes)
bindings file ──────  auto (Esc)   ──────  live
layout.toml   ──────  MANUAL Ctrl+S ─────  live (direct mutation)   ← data loss on Esc
```

Two problems visible here: layout edits are lost on Esc (item 1), and config apply-while-open is partial (item 3).

Discovered anchor: `apply_config_on_enter` (`crates/gameplay-drums/src/lib.rs:232`) already maps the *entire* `Config` into live resources on Performance enter (`ScrollSettings`, `DrumAudioSettings`, `StageGauge.damage_level`, `InputOffsetMs`, `BgmAdjustState`, `ShowPerfInfo`, `MetronomeEnabled`, `ShowTimingLines`), and `load_drum_audio_settings` (lib.rs:265) maps `DrumGameplaySettings.config` + `DrumPolyphony`. Every setting therefore already has a live resource that is consumed each frame/hit. Live-apply = push the draft into that same set of resources while the surface is open, instead of only the four fields `apply_draft_live` currently touches (`editor/tabs.rs:69`).

## Work items

### 1. Layout auto-save on close (data-loss fix)

Layout (`layout.toml`, Lanes/Widgets tabs) must persist on close like config + bindings do, not only on Ctrl+S.

- Add `save_layout_on_close` in `editor/save.rs`: mirrors `tabs::save_draft_on_close` — fires when `EditorOpen` flips true→false while in Performance (Esc route). Builds `layout_file_from(&layouts, &lanes)` and writes `dtx_layout::default_path()`.
- Add layout + config + bindings save into `mod.rs::close_editor_on_exit` (the OnExit(Performance) route — song-ended-mid-edit). Currently NO store fires on that route, so config/bindings are also lost there; close all three gaps at once. Idempotent double-write (Esc route saves twice) is harmless — same bytes.
- Keep Ctrl+S as an explicit early save.

### 2. Drums tab value-text overflow (visual bug)

Long stepper values ("All Separate") wrap to a second line and overlap the next row (`editor/panel.rs` stepper branch, `SettingValueText` `min_width: 60px`, no line-break control).

- Give the stepper value `Text` a `TextLayout` with `LineBreak::NoWrap` so it never wraps vertically (overflows horizontally into its own reserved column instead).
- Widen the stepper value `min_width` 60→96 and reduce `< >` button horizontal padding 6→5 to buy room in the ~200px panel.
- Apply the same `NoWrap` to slider value text for consistency.
- Verify on the Drums tab via BRP screenshot; iterate widths if any row still collides.

### 3. Live-apply every setting

Expand `editor/tabs.rs::apply_draft_live` to push the full draft into the live resources that `apply_config_on_enter` + `load_drum_audio_settings` already drive:

| Setting | Live resource / action | Consumer already reads it |
|---|---|---|
| scroll_speed | `ScrollSettings::from_scroll_speed` | yes (existing) |
| play_speed | `ScrollSettings.play_speed = play_speed_multiplier(raw)` | judge/scroll chart-time |
| input_offset_ms | `InputOffsetMs.0` | yes (existing) |
| bgm_adjust_ms | `BgmAdjustState.common_ms` | yes (existing) |
| audio (5) | `DrumAudioSettings` (+ BGM volume/stop) | yes (existing) |
| damage_level | `StageGauge.damage_level = map_damage_level(..)` | gauge on miss |
| show_perf_info | `ShowPerfInfo.0` | bar-measure number labels near timing lines (not a true FPS overlay) |
| metronome | `MetronomeEnabled.0` | beat-line tick |
| lane_display | `ShowTimingLines.0 = lane_display.shows_timing_lines()` | beat-line render |
| drums.* | `DrumGameplaySettings.config` + recompute `.groups` via `EffectiveGroups::from_config(&config, &presence)` | hit_sound/judge |
| polyphonic_sounds | `DrumPolyphony::set_voices(n)` | hit_sound voices |
| vsync | mutate primary `Window.present_mode` (`AutoVsync`/`AutoNoVsync`) | winit |

- Make `map_damage_level` `pub(crate)` in lib.rs so `apply_draft_live` (same crate) can reuse it (no duplicate mapping table).
- VSync is currently unwired **even at startup** (main.rs only logs it; `present_mode` uses Bevy's default). Wire it in two places: (a) set `present_mode` from `cfg.system.vsync` on the initial `Window` in `app/dtxmaniars-desktop/src/main.rs`, and (b) live-mutate it in `apply_draft_live`. `bevy_framepace` caps frame *rate* independently; `present_mode` still governs vsync/tearing. Note in a code comment that framepace pacing is orthogonal.
- play_speed live-applies via `ScrollSettings.play_speed`, which rescales **visual note spawn only**. Judgment and audio use unscaled time (pre-existing decoupling, `resources.rs:230-231`), so changing play_speed mid-chart visually desyncs notes from the judge line/audio until re-enter. Live-apply matches the resource's semantics; BRP-verify whether the mid-loop visual is acceptable, and if it reads as broken, fall back to leaving play_speed apply-on-re-enter (the one exception). Not expanding audio-rate here.
- Remove the early-return in `apply_draft_live` that short-circuits when audio is unchanged (it would skip the new non-audio writes). Restructure so every field writes each run; keep the "only touch BGM volume/stop when audio actually changed" guard local to the audio block.

Result: no "applies next song" badges needed — everything is genuinely live.

### 4. Offset range widening + tap-test calibration

**Range + granularity** (`crates/dtx-config/src/lib.rs`):
- `INPUT_OFFSET_CLAMP_MS` and `BGM_ADJUST_CLAMP_MS` 99 → 300.
- Offset rows step 1 ms per adjust unit (was 10): change the two offset `SettingItem.adjust` closures to `± 1 * d` and their slider `step` 10.0 → 1.0 (`editor/settings_data.rs`). Fine control by default; coarse via Shift (below).

**Shift = coarse** (`editor/keyboard_nav.rs`): when Shift is held, repeat the focused row's `adjust(dir)` 10×. Generic across rows (offsets → 10 ms, scroll → 5.0, enum cycles → 10 steps). Footer documents "Shift = coarse".

**Tap-test overlay** — measures input+audio latency and suggests `input_offset_ms`. New module `editor/calibration.rs`:
- `CalibrationState` resource: `Idle` | `Collecting { samples: Vec<i32> }` | `Done { median: i32 }`.
- Entry: a "Calibrate" button in the Gameplay settings block header (spawned next to RESET TAB, Gameplay tab only). Press → `Collecting`, force `MetronomeEnabled.0 = true` AND `ShowTimingLines.0 = true` (the tick only fires on timing-line crossings, so both are required — see runtime trace), remembering both prior flags to restore on exit.
- Sampling: on each drum hit while `Collecting`, read the raw chart clock `GameplayClock.current_ms` (BEFORE input offset — we are measuring raw latency), compute signed error to the nearest quarter-beat on the chart BPM grid (`error_ms(now, bpm, first_beat)` pure fn), push to `samples`. Ignore samples with |error| > half a beat (mishits).
- After ≥ 12 samples → `Done { median }`. Overlay shows live count "Tap to the beat (n/12)" then "Suggested {+X} ms — Enter apply · Esc cancel".
- Enter → `input_offset_ms = -median` (clamped), restore metronome flag, `Idle`. Esc → restore + `Idle` (Esc is intercepted while calibrating so it does not close the surface).
- Pure functions (`error_ms`, `median`, `suggested_offset`) unit-tested; the wiring is BRP-verified for the UI flow (overlay appears, count increments on simulated hits, suggestion renders). Real-latency accuracy can't be asserted headlessly — documented.
- This is the highest-risk item and lands as the **last commit** so the six solid items are never blocked by it.

### 5. Keyboard tab switching

- Add `CustomizeTab::next()` / `prev()` cycling `CustomizeTab::ALL` (`crates/game-shell/src/states.rs`).
- `editor/keyboard_nav.rs` (or ui.rs): `PageDown` → `active.0 = active.0.next()`, `PageUp` → `.prev()`. Works on all 7 tabs (not just settings). Do not fire while bindings capture is active or Ctrl held.
- Footer legend gains "PgUp/Dn tab".

### 6. Delete dead `game-menu/src/config.rs`

1143-line orphan: references a non-existent `AppState::Config` variant and is not declared as a module in `game-menu/src/lib.rs`. `rm` the file. Confirm via grep that nothing references `menu::config` / `game_menu::config` / `mod config`.

### 7. Bindings tab polish

- Long MIDI port name wraps to 2 lines (`editor/bindings_panel.rs`, `port_display_label` + the port `Text` `max_width: 150`). Truncate the display string to ~22 chars with a trailing "…" and add `LineBreak::NoWrap`.
- Examine `editor/bindings_spatial.rs`: the selected channel's bind chips render as clipped, overlapping text on the preview pad (screenshot: "Space Convert N36 N35 N72" jammed at the BD pad). Fix legibility (clamp count shown / reposition / smaller readable stack) or suppress when it can't fit. Decide during implementation after reading the module.
- The velocity meter under "Velocity threshold" is **intended** (fill + threshold tick) — audit re-check confirms it is not a stray fragment; no change.

### Micro-fixes
- `dtx-config/src/lib.rs:195` doc comment "0.5..4.0" → "0.5..9.0" (matches the actual clamp).
- Footer hint "Hover a setting for details." starts at x=16px, hidden behind the left rail. Offset the footer description text start past `RAIL_WIDTH + LEFT_PANEL_WIDTH` so it sits in the preview area (`editor/footer.rs`).

## Testing

- Unit: calibration `error_ms` / `median` / `suggested_offset`; `CustomizeTab::next/prev` cycling; offset clamp at ±300; `map_damage_level` reused (compile-level).
- Existing settings_data round-trip tests stay green; update the scroll-speed range test if it asserts the old clamp.
- **Schedule-ordering guard**: green unit tests do not prove the real FixedUpdate schedule builds (known trap — memory `tests-skip-real-plugin-schedule`). Run the ordering guard test / a real launch.
- **BRP screenshot verification** per changed tab: Drums (no overflow), Gameplay (Calibrate button + tap overlay flow), footer hint visible, PgUp/Dn switches tabs, Bindings port name single-line. Coordinate scale varies per monitor (memory `brp-drive-customize`) — one test click to confirm mapping first.
- Manual data-loss check: edit a lane width, Esc, re-open → change persisted.

## Commit sequence (one branch, `feat/settings-overhaul`)

1. Delete dead `config.rs` (item 6) — isolated, safe first.
2. Micro-fixes (doc comment, footer hint) — trivial.
3. Drums/slider value overflow (item 2).
4. Layout auto-save on close (item 1).
5. Live-apply every setting + VSync wiring (item 3).
6. Keyboard tab switching (item 5).
7. Offset range widening + Shift-coarse (item 4a).
8. Bindings polish (item 7).
9. Tap-test calibration (item 4b) — last, highest risk.

No AI co-author in commits.

## Out of scope (explicit)
- Dark Mode, Reverse, Fill-in, Stage Failed, BGA, Movie, BG Alpha toggles — no runtime system exists; would be lying UI. Config fields remain unwired placeholders.
- Gamepad navigation — no input infra.
- play_speed → audio playback rate coupling — pre-existing gap, unchanged.
- Config/bindings save on the song-ended-OnExit route is fixed as a side effect of item 1's `close_editor_on_exit` change.
