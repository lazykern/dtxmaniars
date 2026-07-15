# Difficulty Vertical-Order Navigation — Design

## Goal

Make vertical difficulty navigation follow the visible difficulty rail in every
menu that exposes it: Up selects the row above and Down selects the row below.

## Decision

Difficulty rails render the highest available ordinal at the top and the
lowest at the bottom. `Selection.difficulty` remains an ascending ordinal, so
input adapters must translate visual direction to ordinal direction:

| Input | Visible movement | Ordinal change |
| --- | --- | --- |
| Up / upward wheel | one row up | `+1` |
| Down / downward wheel | one row down | `-1` |

Apply this mapping in Song Select and Song Ready. Leave difficulty ordering,
selection persistence, direct row clicks, bounds clamping, shared navigation
types, and pad card traversal unchanged.

## Error handling and testing

At the top and bottom of a rail, movement remains clamped. Add focused unit
tests proving that keyboard Up/Down emits the correct Ready delta and that the
shared difficulty step helper moves the ordinal in the displayed direction.
Run the `game-menu` library tests, formatting, and a package check before
handoff.
