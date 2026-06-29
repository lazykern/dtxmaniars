# DTXManiaRS — UX/UI Design & Implementation Plan

> **Status:** approved (2026-06-28). ADR-0014 in effect.
> **Inputs:** `UX_UI_AUDIT.md`, `BEVY_UX_UI.md`, ADR-0010 (mechanics-only), ADR-0014.

---

## 1. Vision

DTXManiaRS **ports BocuD game mechanics** but **redesigns all visual UX** for osu-lazer-grade fluidity from day one.

| Layer | Source of truth |
|---|---|
| Mechanics | `references/DTXmaniaNX-BocuD/` (judgment, scoring, lanes, channels) |
| UX/UI | ADR-0014 + this document (osu-inspired redesign) |

**Success:** Smooth 300ms transitions, rolling HUD, modern song select, 60fps at 1280×720.

---

## 2. Key decisions

| Decision | Choice |
|---|---|
| Transitions | 300ms OutQuint fade overlay — no GitaDora, no 1500ms snapshot |
| HUD | Rolling score, bounce combo, tweened gauge, judgment popup |
| Song select | Modern vertical list + density + info panel (not osu carousel) |
| Theme v1 | Hardcoded dark theme in `dtx-ui::theme` |
| Animation | Hand-rolled `ScalarTween` + `bevy_tweening` (git rev `5e3d0c9`) |
| Debug | `bevy_brp_extras` + `.mcp.json` (debug builds only) |

---

## 3. Architecture

```
dtx-ui          widgets, theme, transition overlay, easing
game-shell      AppState + TransitionRequest + fade director
game-menu       screens (title, song select, loading, config, end)
gameplay-drums  mechanics + osu-style HUD widgets
game-results    animated result panel
```

All screen changes use `TransitionRequest` → `ScreenFade` (FadeOut → NextState → FadeIn).

---

## 4. Screen designs

| Screen | Implementation |
|---|---|
| Startup | Themed splash, auto-advance → Title |
| Title | Themed menu, ENTER/ESC |
| Song select | List + status/density overlays, theme selection highlight |
| Song loading | Themed progress, → Performance |
| Performance | BocuD mechanics + `dtx-ui` HUD widgets |
| Result | Fade-in stat panel, rank display |
| Config | Tab list (existing), themed |
| End | 1s countdown → exit |
| ChangeSkin | Minimal M13 placeholder; full skin browser deferred to M14+ |

---

## 5. Widget library (`dtx-ui/src/widget/`)

| Widget | File |
|---|---|
| Rolling counter | `rolling_counter.rs` |
| Combo display | `combo_display.rs` |
| Gauge bar | `gauge_bar.rs` |
| Judgment popup | `judgment_popup.rs` |
| Lane flush | `lane_flush.rs` |

---

## 6. Testing & debug

| Layer | Tool |
|---|---|
| Unit | `dtx-ui` easing/tween/transition tests |
| Integration | headless App spawn tests |
| Live | BRP MCP screenshot + send_keys |
| Baselines | `docs/notes/UX_UI_SCREENSHOTS/` |

Run windowed: `DTXMANIARS_WINDOWED=1 cargo run -p dtxmaniars-desktop`

---

## 7. Implementation status

| Phase | Status |
|---|---|
| ADR-0010 rescope + ADR-0014 | Done |
| Transition system (300ms) | Done |
| Theme + easing + widgets | Done |
| HUD wired (drums) | Done |
| Themed screens | Done |
| BRP debug (debug builds) | Done |
| `.mcp.json` | Done |
| InterpolatedAudioClock | Future |
| Full skin system | Future |
| bevy_framepace | Future |

---

## 8. References

- ADR-0014 — osu-inspired UX redesign
- `docs/BEVY_UX_UI.md` — Bevy patterns
- `docs/UX_UI_AUDIT.md` — research inventory
- osu-lazer: 300ms OutQuint fades, RollingCounter
