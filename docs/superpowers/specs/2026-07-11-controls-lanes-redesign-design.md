# Controls & Lanes Tab Redesign

Date: 2026-07-11
Status: Approved

## Goal

Replace the unfinished Controls and Lanes tabs in the Customize surface with a finished, dual-input (mouse + keyboard/pad) UI. This design consumes the approved profile-management design (`2026-07-11-input-and-lane-profile-management-design.md`) — profile registries, draft/save model, and Keyboard | MIDI segmentation come from that spec; this spec defines the UI shape, interaction flows, visual system, and one new runtime capability (shared-source fan-out).

## Problems addressed

User pains, all confirmed:

1. **Looks bare** — raw chips and sliders float without hierarchy; panel reads as debug UI.
2. **Layout waste** — narrow crammed left strip, disconnected preview.
3. **Interaction clunky** — hidden footer hints, `<`/`>` steppers, `^`/`v` reorder arrows, opaque capture.
4. **Missing features** — no profile bar, no visible preset management, no visual lane preview linkage.

Plus one new requirement: **one input source may map to multiple channels** (e.g. one key fires both BD and LBD).

## Decisions

| Area | Decision |
|---|---|
| Paradigm | Hybrid: list panel left + manipulable playfield miniature right, selection synced both ways. |
| Controls structure | Segmented `Keyboard \| MIDI` per profile spec; one source visible at a time. |
| Lanes structure | Slim reorder rows + detail card for the selected lane. |
| Capture | Modal with arrived-input preview (key name / note + velocity) before commit. |
| Shared bindings | Keyboard **and** MIDI sources may map to multiple channels; runtime fans out one press to every owning channel. |
| Aesthetic | Quiet instrument: near-black, hairline cards, single blue accent; channel colors as small dots only. |
| Input priority | True dual: mouse and keyboard/pad both first-class. |

Rejected alternatives:

- **Polished panel with read-only preview** — cheapest, but wastes the miniature and fails the direct-manipulation expectation for lane widths.
- **Pure direct manipulation (no list)** — most game-like, but keyboard/pad-only navigation still needs a parallel path, and precise ops (exact widths, removing one note) get harder.
- **Merged keyboard+MIDI table** — everything visible at once but two profile pickers and two dirty states in one bar; diverges from the approved profile spec.
- **Inline listening-chip capture** — faster loop, but user chose modal for stray-hit safety and arrived-input preview.
- **Keyboard-only sharing** — user chose sharing for MIDI too; the capture modal absorbs the existing steal-confirm flow as one of two options.

## Structure & navigation

### Shared shell

Both tabs keep the existing left panel + shrunk playfield miniature (stage transform unchanged). New pinned **profile bar** at the top of the panel: context label, active profile dropdown, dirty dot (amber), `Save`, `Save As`. Content below as cards. Selection syncs both ways: selecting a row lights its lane in the preview; clicking a pad in the preview selects its row.

### Controls tab

- Segment selector `Keyboard | MIDI` sits under the profile bar. Each segment has its own profile registry and its own profile bar state (per profile spec).
- **Keyboard segment**: one "Pads" card — channel rows in `channels_in_display_order`: color dot, channel name, key chips, `+` capture affordance.
- **MIDI segment**: "Device" card (port dropdown with live status dot ● connected / ● disconnected, velocity-threshold slider with numeric value, Rescan) above the same "Pads" card with note chips.
- Chips whose source maps to multiple channels show a shared marker (⧉). Hovering or focusing a shared chip lights every owning lane in the preview.

### Lanes tab

- Profile bar hosts the preset dropdown: built-ins (Classic, NX Type-B, NX Type-D — immutable) plus user presets, then `Save` / `Save As`.
- **Slim rows**: drag handle, color dot, lane name, muted secondary-channel summary (`+HHO`). Mouse drags to reorder; keyboard/pad uses a move verb.
- **Detail card** below the list for the selected lane: width slider with numeric multiplier, channel chips with `+ add` (chooser lists unassigned channels), `hide lane`. Hidden lanes collect in a muted "Hidden" strip and can be restored.
- **Preview manipulation**: drag a pad horizontally to reorder; drag a pad edge to resize width. Detail card values update live. Both surfaces edit the same `LaneProfileDraft`.

### Focus model

Same descend/ascend pattern on both tabs. Controls: tab bar → segment selector → rows (chips and `+` actionable via Left/Right + Confirm within the focused row) — the existing `reduce_controls_nav` levels. Lanes: tab bar → rows → detail card; the Lanes reducer adds the `Detail` level with the same verb grammar. Pads navigate everything **except** while a capture modal is armed (pad hits are capture input there — existing `pad_excluded` contract). Footer keeps context verb hints.

## Editing flows

### Capture modal

`+` on a channel row (or Confirm verb on it) opens a centered modal over the dimmed panel; the preview stays visible with the target lane lit.

- Prompt: "Hit a pad for SD" / "Press a key for SD". Esc cancels. Modal never captures Esc, modifiers, or reserved keys (existing `is_reserved` rules).
- On input, the modal shows what arrived — key name, or MIDI note number + velocity — **before** commit. Enter (or hitting the same note again) commits; Esc discards.
- If the source is already owned by another channel, the modal offers **Add shared** (both channels keep it, chips marked ⧉) or **Move here** (steal — replaces the current `ConfirmMidiSteal` flow). Same options for keyboard and MIDI.
- Hits below the velocity threshold render grayed with "below threshold" — doubles as a threshold sanity check.
- MIDI capture keeps the strictly-new-NoteOn rule (`strictly_new_note`): stale hits predating arming are never learned.

### Shared-binding runtime fan-out

Today `lane_for_key` / `channel_for_note` dispatch to a single channel even though profile storage holds `Vec<EChannel>` per source. Change: dispatch fans out — one key press / NoteOn emits a `LaneHit` for **every** owning channel (BD+LBD chord from one key). Chip delete (×) removes only that channel's claim; other owners keep the source.

### Profiles & save

Per the profile spec: edits mutate a draft; dirty dot appears on the profile bar. `Save` writes the registry; `Save As` prompts for a name. Built-ins are immutable — editing one flips the flow to Save As. Switching profile/preset or closing the surface with a dirty draft routes through the existing dirty-close dialog (keep / discard). Lane preset switching previews instantly by replacing the draft; named presets are never silently overwritten (kills the current "editing renames you to Custom" behavior).

### Edge handling

- MIDI port vanishes mid-session → device card flips to ● disconnected; the profile stays active and editable (existing `PortMatch` contract, no profile switch).
- Removing the last chip on a channel is allowed; an unbound channel row gets a warning tint and "no binding" note.
- Lane width clamps to a minimum; zero-width lanes cannot be produced from either the slider or edge drag.

## Visual system

Quiet-instrument token set, defined once and shared by both tabs (constants join `editor/chrome.rs`):

- Panel near-black (`#12141a` family), cards one step lighter with hairline borders.
- Single blue accent for focus/selection; selected rows get a 2px inset left bar.
- Channel colors appear only as 9px rounded dots in rows and as lane colors in the preview — the panel stays monochrome otherwise.
- Uppercase letter-spaced micro-labels for card titles (DEVICE, PADS, HH LANE).
- Status dots: amber = dirty, green = connected, red = disconnected.
- Chips: dark fill, hairline border, ⧉ suffix when shared. Hover/focus states defined once in the shared kit.

## Architecture

Reshaping existing editor modules; no new crates beyond what the profile spec already adds.

```text
editor/
  panel_kit.rs        shared row/chip/card/profile-bar spawn helpers
  controls_panel.rs   segments + device card + pad rows (rendering moves here)
  lanes_panel.rs      slim rows + detail card + hidden strip
  bindings_capture.rs modal state machine: Armed -> Arrived -> commit/shared/steal
  bindings_spatial.rs preview linkage; pads become pick/drag targets on Lanes
```

- `bindings_panel.rs` (857 lines) splits into `controls_panel` rendering and `lanes_panel`; shared spawn helpers land in `panel_kit` (also serves future tabs).
- `bindings_capture.rs` keeps its pure-reducer style; the modal adds an `Arrived` step and the shared/steal branch.
- Preview manipulation extends existing picking + scene-space math (already unified); drag-reorder and edge-resize write to `LaneProfileDraft`.
- `dtx-input`: `lane_for_key` → `lanes_for_key` (and MIDI equivalent) returning all owners; gameplay dispatch emits one `LaneHit` per owner.
- Profile registries (`keyboard-profiles.toml`, `midi-profiles.toml`, `lane-profiles.toml` via `dtx-persistence`) come from the profile spec — consumed, not redesigned.

## Testing

Repo's existing pattern: pure reducers unit-tested —

- Navigation levels including the new `Detail` level, both tabs.
- Capture modal steps: arm, arrive, commit, cancel, shared vs steal, below-threshold, reserved-key refusal, stale-note refusal.
- Fan-out dispatch: one source, multiple owners → one `LaneHit` per owner; chip removal drops only that owner.
- Preset switch dirty rules: instant preview, dirty-dialog routing, built-in immutability forcing Save As.

BRP smoke for the visual layer: click pad → row selects; drag edge → width changes; capture modal shows arrived note.

## Out of scope

- Widgets tab (untouched).
- Profile import/export.
- Per-lane color editing.
