# crates/dtx-input

Engine-layer crate. Keyboard + MIDI input sources for all gameplay modes.

## Reference / context

- **ADR-0009** (this crate's originating decision): `docs/decisions/0009-dtx-input-deferred.md`
- BocuD input: `references/DTXmaniaNX-BocuD/DTXMania/Input/` (no consolidated
  file — M6c ports the Rust API only; full port of CInputManager.cs is M6.1)

## Why now (M6c)

ADR-0009 deferred this crate from M2 to M6. M6c extracts
`gameplay-drums/src/input.rs` (keyboard → LaneHit) here and adds a MIDI
abstraction. Per ADR-0009 the gameplay crate consumes `dtx-input::LaneHit`
instead of defining its own.

## MIDI

The `midi` feature enables the `midir` crate. We ship it as optional so the
default build stays small (no libasound/libcoremidi headers required). The
`MidiSource` trait is always available; the real-device impl requires the
feature.

`VirtualSource` (in this crate) provides an in-memory event queue used by
tests. It is the verification vehicle for "virtual device smoke test" in
M6c's task contract.

## MIDI drum note mapping (5-pin standard)

```
36 = BD          42 = CH (closed HH)  46 = HH open
38 = SD          49 = HH (closed alt) 51 = RD
```

These are user-rebindable later via `dtx-config`. M6c ships hardcoded defaults.

## Layer

Engine. Sits between dtx-core (Engine) and gameplay-* (Game) crates.