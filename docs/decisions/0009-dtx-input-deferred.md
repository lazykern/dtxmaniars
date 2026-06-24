# 0009: dtx-input crate deferred — keyboard in gameplay-drums for M2

Status: accepted
Date: 2026-06-23

## Context

ARCHITECTURE.md lists `dtx-input` (Engine) as the keyboard/MIDI/pad mapping
crate. M2 (gameplay-drums) needed keyboard input to be playable. Options:

1. Create `dtx-input` crate now, even though only keyboard is used.
2. Put keyboard input directly in `gameplay-drums` (Game layer); extract to
   `dtx-input` later when MIDI/pad lands.

## Decision

Put keyboard → LaneHit directly in `gameplay-drums/src/input.rs`. Defer
`dtx-input` crate to M6 (when MIDI/pad input lands alongside guitar/bass).

YAGNI: a single-file keyboard mapper doesn't need its own crate.

## Consequences

- One less crate to maintain.
- When MIDI lands (M6), extract `gameplay-drums/src/input.rs` →
  `crates/dtx-input/src/keyboard.rs` + new `midi.rs`. Game crate then consumes
  `dtx-input::LaneHit` instead of defining its own.
- The keyboard mapping currently uses digits 1–9 by default. LaneMap is a
  resource so rebinding is trivial later.

## Alternatives considered

- **Create dtx-input now:** premature; nothing to put in it except keyboard,
  which is 30 lines of code.
- **Make dtx-input a module of gameplay-drums:** wrong layer (Engine belongs
  in a standalone crate for use by other game modes).