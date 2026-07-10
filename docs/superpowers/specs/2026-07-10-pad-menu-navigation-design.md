# Pad Menu Navigation — Design

Date: 2026-07-10
Scope: drum-pad-driven navigation for all menus (song select, pause, results, settings overlay) plus bigger settings-row targets. Widgets tab stays keyboard/mouse-only for movement/drag work; a guided binding setup wizard is a **separate future spec**.

## Motivation

Every menu today assumes keyboard/mouse. A drummer sitting behind the kit cannot pick a song, retry, or tweak input offset without walking to the keyboard. Goal: full session — pick song, play, adjust hot settings, retry — pads only. Secondary goal: bigger settings controls that are readable and clickable from a couple of meters away, which also helps mouse users.

## Architecture: semantic `NavAction` layer

```
keyboard keys ──┐
                ├──> nav_mapper ──> NavAction events ──> consumers
drum hits ──────┘    (one system,      Up / Down /        ├─ song select
(resolved channel     gated by         Confirm / Back /   ├─ pause menu
 events, post-        MenuContext)     Dec / Inc          ├─ results screen
 bindings)                                                └─ settings overlay
```

- New event enum `NavAction`: `Up, Down, Confirm, Back, Dec, Inc`, each carrying a `coarse: bool` modifier (Shift on keyboard = ±10; pads never emit coarse — repeated hits substitute).
- `MenuContext` (state/resource) gates the mapper: active in Title, SongSelect, Result, pause overlay, and the F1/F2 settings overlay; inactive during live Performance.
- The mapper consumes the same **resolved channel** of a hit that gameplay uses (after bindings resolution), so nav works with any MIDI note layout the user bound.
- Existing keyboard nav paths (`editor/keyboard_nav.rs`, `song_select.rs` arrows, `pause.rs`, `game-results`) are refactored to emit/consume `NavAction`. Keyboard behavior is unchanged; this is a pure indirection step.
- Screens consume only `NavAction`, never raw keys or pads (existing kb-only extras — Tab sort, search, F5, Shift+Enter — stay on their raw-key paths).

### Fixed pad mapping (GITADORA/DTXMania convention)

One table constant, not scattered:

| Pad | Nav meaning |
|-----|-------------|
| HH  | Up / Dec (−) |
| CY / RD | Down / Inc (+) |
| BD  | Confirm |
| SD  | Back / Cancel |
| FT  | Practice-start (song select difficulty level only) |

## Settings overlay (F1/F2 Customize surface)

Overall layout unchanged: left panel (tab rail + rows), live stage preview right, save-on-close. Changes are inside the left panel.

### Two-level focus-then-adjust grammar

```
TAB RAIL              ROW LIST                 ADJUST MODE
HH/CY = prev/next tab HH/CY = row up/down      HH = − , CY = +
BD = enter tab   ──>  BD = enter adjust  ──>   BD = confirm (keep value)
SD = close overlay    SD = back to tab rail    SD = cancel (revert row to
                                               value on adjust-entry)
```

- Focused row: red focus ring. Adjust mode: green ring; stepper `< >` glyphs swap to `− +`.
- Adjust-mode edits apply live via the existing `apply_draft_live` path; SD cancel restores the row's pre-adjust value.
- Sliders step by their existing step size per hit; a drum roll on CY/HH is the natural fast-scrub.
- Focused row auto-scrolls into view.
- Keyboard/mouse flows unchanged (arrows/PageUp/PageDown/clicks as today), but keyboard focus drives the same focus ring.

### Bigger rows everywhere (mockup option B)

Taller rows, larger label/value text, fat stepper buttons — the whole panel, not just the focused row (no layout jump while scrolling). Cost: fewer rows per screen, panel scrolls more. Benefits mouse users too.

### Excluded tabs

- **Widgets** and **Bindings** content is not pad-navigable. Pad nav can still scroll past them on the tab rail; entering shows a "keyboard/mouse required" hint banner and only `SD` (back to rail / close) works from pads.
- Rationale: widget movement is inherently pointer work; bindings capture consumes MIDI hits and pad nav there is a footgun. Pad-friendly binding = future setup wizard spec (DTXMania-style hit-prompted capture).

## Song select, pause, results

**Song select** — same two-level grammar:

```
WHEEL LEVEL                      DIFFICULTY LEVEL
HH/CY = folder up/down           HH/CY = difficulty prev/next
BD = confirm → difficulty  ──>   BD = start song
SD = back to Title               SD = back to wheel
                                 FT = start Practice mode
```

Keyboard keeps its flat model (Up/Down folders, Left/Right difficulty, Enter play, Shift+Enter practice) — the mapper feeds pads the two-level path without changing kb semantics.

**Pause menu**: HH/CY = item up/down, BD = confirm, SD = resume (back). Pads reach the pause menu only while paused, so no gameplay conflict.

**Results**: BD or SD → song select (kb Esc/Enter unchanged).

**Title**: BD = advance to song select (whatever kb confirm does today), so `SD` backing out of the wheel doesn't strand a pads-only player.

**Legend bar**: every pad-navigable screen shows a bottom legend with the current verb set (e.g. `HH up · CY down · BD adjust · SD back`), updating per nav level. Hidden when no MIDI device is connected.

## Edge cases

- **Velocity**: nav respects the same velocity threshold as gameplay; ghost hits / mesh crosstalk don't navigate.
- **Double-trigger/flam guard**: minimum ~80 ms gap between accepted pad-sourced `NavAction`s; when two pads land in one frame (flam), the first event wins.
- **Screen-enter grace**: pad nav ignored for ~500 ms after each screen/state entry — the last note of a song can't skip the results screen.
- **Capture/calibration suspension**: pad nav fully suspends while bindings capture is armed or the calibration tap-test overlay is open (both sample raw hits).
- **No MIDI device**: zero behavior change; legends hidden.
- **Keyboard + pads together**: both emit `NavAction`; no arbitration needed.

## Feedback

- Pad hits in menus keep playing their normal kit sound — natural confirmation. No new SFX in v1.
- Visual state carried by focus/adjust rings and legend bars.

## Testing

- Unit: mapper table (channel → `NavAction` per context and nav level), exclusion gating (Widgets/Bindings/capture/calibration), debounce, screen-enter grace — all headless (events in, events out).
- Refactor safety: keyboard-nav behavior tests assert the `NavAction` consumption path end-to-end so the indirection step is provably behavior-neutral.
- Schedule safety: FixedUpdate ordering guard test covers the new systems (green unit tests alone don't prove the schedule builds).
- Manual: BRP-driven session — launch game, inject events, screenshot each nav state.

## Implementation phases

1. `NavAction` layer + keyboard refactor (no behavior change).
2. Settings overlay: pad nav (focus-then-adjust) + bigger rows + excluded-tab hints + legend.
3. Song select / pause / results pad nav + legends + grace/debounce guards.
4. *(separate future spec)* guided binding setup wizard.

## Out of scope

- Widget moving/resizing via pads (explicitly excluded by design).
- Binding configuration via pads (future wizard spec).
- Gamepad support (the `NavAction` layer makes it cheap later, but it is not in this spec).
- New menu SFX.
