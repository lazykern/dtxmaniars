# 0006: Native `bevy_ui` for game, `bevy_egui` for dev tools

Status: accepted
Date: 2026-06-23

## Context

Two UI paradigms:
- **Native bevy_ui:** retained-mode, scene graph, full animation control, ships with the engine.
- **bevy_egui (immediate-mode):** fast iteration, easy tools, but feels less fluid for shipping UI.

osu-lazer uses its own custom retained-mode (`osu.Framework`) for shipping UI;
ImGui/egui only inside tooling windows.

## Decision

- **Game UI** = native `bevy_ui` (and custom widgets in `dtx-ui` crate).
- **Dev tools** = `bevy_egui` (inspector, FPS counter, log viewer, perf graph).
- Dev tools gated behind a feature; not present in release builds.

## Consequences

- Smooth, skinnable, animation-friendly game UI.
- Fast iteration on internal tools.
- Slight dependency cost (`bevy_egui` in dev only).
- Two UI APIs to know; mitigated by clear separation.

## Alternatives considered

- **egui everywhere:** faster to build, less fluid feel.
- **Custom retained mode from scratch:** max control, huge dev cost.

## Reference files

- `references/osu-lazer/osu.Game/Graphics/UserInterface/` — shipping UI style
- `references/osu-lazer/osu.Game/Overlays/` — immediate-mode-style overlays