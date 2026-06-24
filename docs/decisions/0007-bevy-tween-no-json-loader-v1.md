# 0007: `bevy_tween` for animations, no JSON keyframe loader in v1

Status: accepted
Date: 2026-06-23

## Context

BocuD's fork added a runtime-loadable keyframe animation framework (JSON-driven)
so skin authors can ship custom animations without recompiling Rust.

We have two distinct needs:
- **Engine-internal tweens** (lane flashes, judgement pop, transitions): always in Rust.
- **Skin-extensible animations** (skins add custom judgement FX, menu transitions): need a loader.

## Decision

- **v1 (M0–M5):** use `bevy_tween` with Rust-defined tweens. Cubic-bezier easing via
  `CubicSegment::new_bezier_easing`.
- **M5+ (skin system lands):** add a thin JSON keyframe loader on top of `bevy_tween`
  if skin authors request it.

## Consequences

- Faster shipping — no loader to maintain until someone needs it.
- Skin extensibility deferred; artists can't yet customize animations.
- Loader, when added, is a thin layer (`JsonKeyframe → bevy_tween::Tween`).
- YAGNI applies: build the loader when there's a concrete use case.

## Alternatives considered

- **JSON loader from M0:** speculative; doubles animation system surface.
- **Hand-rolled easing in `dtx-ui`:** reinvents `bevy_tween`.

## Reference files

- `references/DTXmaniaNX-BocuD/README.md` — BocuD's animation framework section