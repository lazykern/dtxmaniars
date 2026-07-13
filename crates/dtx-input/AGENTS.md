# crates/dtx-input

Engine-layer input vocabulary and persistence for keyboard and MIDI sources.
Gameplay crates own the mapping from emitted raw sources to their visual/judge
lanes.

## Current contract

- `LaneHit`, `LaneHitKind`, and opaque `LaneId` are shared event types.
- `InputBindings` covers 12 drum channels plus the independent Pause and
  Restart system verbs. A lane source may be shared by lanes, but a lane-owned
  source cannot also fire a system verb.
- Defaults match the reference keyboard layout and General MIDI percussion
  notes. System verbs are intentionally unbound.
- `keyboard-profiles.toml` and `midi-profiles.toml` are separate version-1,
  atomically replaced registries with built-ins, user profiles, copy/rename,
  migration, backup, and reset behavior.
- `bindings.toml` is the version-1 legacy migration source.
- The optional `midi` feature enables real `midir` ports. `MidiSource` and
  `VirtualSource` remain available for hardware-free tests; the desktop binary
  enables MIDI by default.

## Ownership boundary

`dtx-core` and configuration/persistence crates are Pure dependencies. This
crate must not depend on Game crates or decide drum grouping, judgment, menus,
or hardware-specific pad meaning beyond stored note bindings. Input profiles
are the source of truth under
[ADR-0009](../../docs/decisions/0009-input-profiles-source-of-truth.md).

## Reference evidence

- `references/DTXmaniaNX/FDK/Input/` — device/input architecture
- `references/DTXmaniaNX/DTXMania/Core/Config/CConfigIni.cs` — legacy key assignments

## Verify

```sh
cargo test -p dtx-input --lib
cargo check -p dtx-input
cargo check -p dtx-input --features midi
```

The feature build is mechanical evidence only; real port enumeration and pad
NoteOn behavior require a manual hardware check.
