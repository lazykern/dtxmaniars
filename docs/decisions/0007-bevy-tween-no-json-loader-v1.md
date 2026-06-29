# 0007: `bevy_tweening` for animations, no JSON keyframe loader in v1

Status: accepted (updated 2026-06-28)
Date: 2026-06-23

> **Status note (2026-06-28):** Original decision named crate `bevy_tween`
> (multirious). That crate is **Bevy 0.18 only** (0.12 final). We standardized on
> **`bevy_tweening`** (djeedai) instead — PR #170 merged at git rev `5e3d0c9`
> bringing bevy 0.19 support. Pinned in `Cargo.toml` `[workspace.dependencies]`,
> pulled by `dtx-ui`. The "no JSON keyframe loader in v1" decision stands
> unchanged; only the crate name in the original text is wrong.
>
> See: `docs/BEVY_UX_UI.md` §6 for the full crate matrix and rationale.

## Context

BocuD's fork added a runtime-loadable keyframe animation framework (JSON-driven)
so skin authors can ship custom animations without recompiling Rust.

We have two distinct needs:
- **Engine-internal tweens** (lane flashes, judgement pop, transitions): always in Rust.
- **Skin-extensible animations** (skins add custom judgement FX, menu transitions): need a loader.

## Decision

- **v1 (M0–M5):** use `bevy_tweening` (djeedai) with Rust-defined tweens. Cubic-bezier
  easing via `EaseFunction` / `EaseMethod::CustomFunction`. Pinned to git rev
  `5e3d0c9`; swap to crates.io 0.16 when published.
- **M5+ (skin system lands):** add a thin JSON keyframe loader on top of
  `bevy_tweening` if skin authors request it.

## Consequences

- Faster shipping — no loader to maintain until someone needs it.
- Skin extensibility deferred; artists can't yet customize animations.
- Loader, when added, is a thin layer (`JsonKeyframe → bevy_tweening::Tween`).
- YAGNI applies: build the loader when there's a concrete use case.

## Alternatives considered

- **JSON loader from M0:** speculative; doubles animation system surface.
- **Hand-rolled easing in `dtx-ui`:** partially done as `dtx-ui::tween::ScalarTween`
  for v1; will be replaced by `bevy_tweening` lenses as they land.

## Reference files

- `references/DTXmaniaNX-BocuD/README.md` — BocuD's animation framework section
- `docs/BEVY_UX_UI.md` §6 — crate decision matrix (current status)
- `docs/BEVY_PATTERNS.md` — animation rules + workspace pin note