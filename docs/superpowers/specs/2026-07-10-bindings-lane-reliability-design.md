# Bindings and Lane Reliability Design

Date: 2026-07-10

## Goal

Make binding capture, persistence, lane-preset presentation, and gameplay routing
behave as one verifiable workflow without coupling display layout to judgment
mechanics.

## Decision

Keep the existing two-axis model:

```text
bindings.toml -> EChannel -> logical DrumPad -> judgment
                       |
layout.toml  -> EChannel -> display column -> notes and hit feedback
```

Bindings continue to target drum channels. Lane arrangements continue to map
those channels to display columns. Changing Classic, NX Type-B, NX Type-D, or a
custom arrangement must never change which chart channels a physical input can
judge.

Two alternatives were rejected:

- Binding directly to display columns would make a visual preset change gameplay
  mechanics and would make merged lanes ambiguous.
- Adding a new binding/layout adapter layer would duplicate the existing
  `EChannel` boundary without resolving the concrete defects.

## MIDI capture

The MIDI consumer must drain every queued event whenever it runs. Every NoteOn
must update `LastMidiHit`, even when the chart is empty or the gameplay clock is
not ready, so device selection, the velocity meter, and binding capture remain
chart-independent.

Gameplay `LaneHit` emission remains gated by all existing gameplay requirements:
the event must exceed the configured threshold, resolve to a bound channel, the
chart must contain chips, and the gameplay clock must be ready. Events observed
while those requirements are false are consumed for editor feedback and are not
replayed later.

## Binding capture feedback

Completing a keyboard or MIDI capture must provide feedback for the newly bound
channel in the same editor interaction. The capture system will emit a logical
`LaneHit` for the target channel after installing the binding. Existing gameplay
input may also observe a previously bound keyboard key earlier in the frame; the
capture path must avoid presenting that old mapping as the successful result.

Judgment and scoring remain disabled while Customize is open, so this feedback
only drives lane flash and the editor hit voice.

## Binding panel order and grouping

The Bindings panel must follow the active arrangement from left to right. Each
display lane forms a group whose primary channel appears first, followed by any
secondary channels mapped to the same display lane in canonical logical-pad
order. Every bindable channel appears exactly once.

Changing a lane preset or editing a custom arrangement rebuilds this order
immediately. The order is presentation-only; serialization remains channel-keyed
and stable.

## Reset and modified state

The Bindings panel gets the same recovery affordance as settings tabs:

- `RESET TAB` restores `InputBindings::default()` after an explicit confirmation.
- A modified indicator is shown when live bindings differ from defaults.
- Reset increments `BindingsRev`, immediately rebuilds `BindResolver`, and is
  persisted by the existing close/save path.
- Canceling confirmation leaves bindings unchanged.

The reset includes keyboard bindings, MIDI note bindings, selected MIDI port, and
velocity threshold.

## Custom-layout validation

`LanesSection::resolve` drops duplicate lane IDs after the first occurrence and
emits a warning. This preserves the user's first declared position while
preventing ambiguous lookup and empty duplicate columns. Existing unknown-ID,
width-clamping, and missing-channel repair behavior remains unchanged.

Editor-generated layouts already contain unique IDs; this rule hardens manual or
future-version input.

## Verification

Unit tests cover:

- MIDI draining and `LastMidiHit` updates without chart/clock readiness;
- no stale gameplay hit after a gated MIDI event;
- arrangement-derived binding order for Classic, NX Type-B, and NX Type-D;
- merged-lane primary/secondary grouping and exactly-once channel coverage;
- reset confirmation, cancel, modified state, and default restoration;
- duplicate custom lane-ID removal with first occurrence preserved;
- post-capture feedback targeting the newly bound channel.

A headless integration test covers the complete boundary:

```text
persisted binding -> resolver -> input event -> logical LaneHit/judgment
                  -> active arrangement display-column lookup
```

It runs representative keyboard and MIDI inputs through all three named lane
presets. The existing focused suites and `cargo check --workspace` remain the
final regression gate.

## Scope exclusions

- Binding presets beyond the current defaults
- Per-device MIDI note maps
- Hi-hat pedal control-change interpretation
- Changing BocuD judgment groups or default input assignments
- Redesigning the lane editor beyond binding-list ordering and reset recovery
