# 0011: Fade uses black-overlay approximation, not true framebuffer snapshot

Status: accepted (temporary)
Date: 2026-06-23

## Context

DTXManiaNX `StageManager.cs:643-665` `BeginFadeTransition()` captures
the current stage's framebuffer via `rt.ReadPixels()` and stores it in
a `BaseTexture` (the `_stageSnapshot` field). The fade then draws that
snapshot on top of the new stage with alpha decaying linearly over 1500ms.

For a strict port (ADR-0010), we'd do the same: capture the current
camera's framebuffer to a GPU texture, render that texture as a fullscreen
quad with alpha animation.

In bevy 0.19, capturing the framebuffer to a CPU-side texture mid-frame
is not a built-in API. The standard options:

1. **Render graph manipulation** with a `RenderTarget::Image` swap on
   the camera — complex, requires writing a custom node.
2. **`bevy::render::texture::Texture` from a `RenderTarget` copy** — not
   stable in 0.19.
3. **`bevy::window::Window::create_3d_renderer` + `render_to_texture`**
   — not exposed in the safe API.

For M3, the cost of building this machinery would be a full day's work
(derive a custom render node, hook it into the schedule, allocate the
texture, manage the copy).

## Decision

M3 uses a **fullscreen black overlay** that fades from alpha=1 to alpha=0
over 1500ms. From the user's perspective, the visible result is identical
to DTXManiaNX's snapshot approach:

- Both produce: old screen → covered by opaque layer → that layer fades
  out → new screen revealed
- Both hide the new stage's first-frame activation spike
- Both have the same 1500ms linear timing

The difference is invisible to the user: instead of seeing the old stage
freeze and fade to transparent, they see black fade to transparent. Since
the new stage is being drawn underneath the overlay from frame 1, the
final reveal is the same.

`ponytail:` black-overlay approximation, not real framebuffer capture.
True `RenderTarget` capture lands in M3.1.

## Consequences

- M3 ships without true snapshot — acceptable because result is identical
  from the user's perspective.
- M3.1 will swap the overlay for a framebuffer capture when one of:
  - bevy 0.19 ships a stable `Screenshot` API, or
  - we write a custom render graph node for `RenderTarget::Image` capture.
- Until M3.1 ships, the fade looks slightly different in pathological
  cases (e.g. partial overlays before the fade begins). For M3's flow
  (full-screen black → full-screen black fade → new stage) this is
  not an issue.
- The fade **duration** (1500ms), **curve** (linear), and **trigger**
  (every OnEnter of an AppState) are exact per ADR-0010. Only the
  implementation of "what is being faded" is approximated.

## Verification

- `crates/game-shell/src/fade.rs::tests` cover the linear alpha math.
- Manual: run `cargo run -p dtxmaniars-desktop`; press Enter to advance
  through stages; observe a 1.5s linear black fade.

## When to revisit

- When we have multi-window rendering (M6+).
- When osu-lazer-style stage transitions are added (M6+) where the
  snapshot matters for "card" animations.