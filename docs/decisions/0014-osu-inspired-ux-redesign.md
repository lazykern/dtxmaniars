# 0014: osu-inspired UX redesign (mechanics port unchanged)

Status: accepted
Date: 2026-06-28

## Context

DTXManiaRS ports BocuD **game mechanics** (ADR-0010 amended) but **redesigns
all visual UX** for osu-lazer-grade fluidity. Prior ADRs (0011/0012/0013)
assumed BocuD visual parity first, then M6+ polish. User intent: smooth UX from
day one.

## Decision

### Transitions

- **Drop** GitaDora panel wipe and 1500ms snapshot fade.
- **Adopt** unified 300ms OutQuint fade overlay on all `AppState` changes.
- Implementation: `dtx-ui::transition::TransitionOverlay` + `TransitionRequest` event.

### Performance HUD

- BocuD mechanics underneath (score, combo, gauge from `dtx-scoring`).
- osu-style rendering: rolling counters, tweened gauge, animated judgment popups.
- Widgets live in `dtx-ui/src/widget/`.

### Song select

- Modern vertical list (NOT osu carousel — multi-instrument DTX format).
- BocuD information architecture (metadata, density, instrument).
- osu visual fluidity: smooth scroll, album art, selection animation, parallax BG.

### Theme

- v1: one hardcoded dark theme (`dtx-ui::theme`). Skinning deferred.

### Animation

- v1: hand-rolled tweens with `EaseFunction` enum in `dtx-ui::easing`.
- Optional: `bevy_tweening` when Bevy 0.19-compatible release ships.

### Debug / test

- `bevy_brp_extras` + `bevy_brp_mcp` for live screenshot + keyboard drive (debug builds).
- Headless unit tests for easing, transition state machine, screen spawn.
- Screenshot baselines in `docs/notes/UX_UI_SCREENSHOTS/`.

## Consequences

- ADR-0010 rescoped: port-first = mechanics only.
- ADR-0011/0012/0013 superseded.
- `SCREEN_FADE_MS` constant becomes 300 (OutQuint), not 1500.
- Success criteria: 300ms transitions, 60fps, readable at 1280×720 — not BocuD pixel diff.

## References

- osu-lazer: `SongSelect.cs:79` (300ms fade), `HUDOverlay.cs:37` (OutQuint)
- osu-framework: `TransformableExtensions.cs` (FadeIn/FadeOut)
- `docs/UX_UI_DESIGN.md` — full screen-by-screen design
- `docs/BEVY_UX_UI.md` — Bevy implementation patterns

## Verification

- [ ] All screen changes use `TransitionRequest` (no raw `NextState` from input handlers)
- [ ] Fade duration = 300ms, easing = OutQuint
- [ ] HUD widgets use rolling/tweened display
- [ ] Theme tokens from `dtx-ui::theme`
