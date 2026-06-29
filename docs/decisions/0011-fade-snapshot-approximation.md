# 0011: Fade uses black-overlay approximation, not true framebuffer snapshot

Status: **superseded** by ADR-0014 (2026-06-28)
Date: 2026-06-23

## Supersession

This ADR described a 1500ms linear black overlay approximating DTXManiaNX's
StageManager snapshot fade (ADR-0010 strict visual port).

ADR-0014 replaces all screen transitions with **300ms OutQuint fade in/out**
via `dtx-ui::transition::TransitionOverlay`. GitaDora panel wipe and 1500ms
snapshot fade are dropped.

## Historical context (archived)

Original decision: M3 black overlay 1500ms linear per BocuD StageManager.cs:29.
Never fully implemented in code (`fade.rs` was planned but not shipped).

## Replacement

See ADR-0014 and `crates/dtx-ui/src/transition.rs`:
- FadeOut: alpha 0→1, 300ms, OutQuint
- FadeIn: alpha 1→0, 300ms, OutQuint
- All `AppState` changes route through `TransitionRequest` event
