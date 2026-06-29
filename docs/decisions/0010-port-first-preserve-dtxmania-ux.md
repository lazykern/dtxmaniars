# 0010: Port-first — preserve DTXManiaNX game mechanics; UX/UI redesigned

Status: accepted (amended 2026-06-28)
Date: 2026-06-23

## Context

We are **porting** DTXManiaNX-BocuD (C# / DX9) to Rust/Bevy. The port has two
distinct layers:

1. **Game mechanics** — judgment windows, scoring, lane order, EChannel mapping,
   chart parsing, scroll logic, input bindings. These must match BocuD verbatim.
2. **UX/UI** — screen layout, transitions, HUD animation, song-select visuals.
   These are **redesigned** for osu-lazer-grade fluidity per ADR-0014.

Prior wording applied "port-first" to all UX/UI. That blocked the redesign goal.
This ADR rescopes port-first to **mechanics only**.

## Decision

**Strict port baseline for game mechanics.** For every mechanical element:

1. **Source of truth** is `references/DTXmaniaNX-BocuD/` for timing, scoring,
   lanes, channels, chart semantics.
2. **Match the reference verbatim** for: lane order, judgment timing windows,
   scroll direction, default input bindings, score/combo math, gauge drain rules.
3. **Cite the reference file:line** in the commit for any non-trivial mechanical
   behavior ported (same rule as ADR-0008).
4. **UX/UI is NOT port-first.** Visual design follows ADR-0014 (osu-inspired
   redesign). Do not copy BocuD pixel positions, GitaDora transitions, or
   static HUD layout when ADR-0014 specifies otherwise.

## Concrete examples

| Element | Port-first (mechanics) | Redesign (UX/UI, ADR-0014) |
|---|---|---|
| Judgment windows | ±values from `ConfigIni` defaults | Judgment popup animation style |
| Lane order | LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO | HUD widget placement/animation |
| Score math | BocuD scoring rules | Rolling counter display |
| Screen transitions | N/A (not mechanical) | 300ms OutQuint fades (not GitaDora) |
| Song select data | Same metadata fields | Modern list layout + smooth scroll |

## Consequences

- Mechanics tests compare against DTXManiaNX reference values.
- UX/UI tests compare against design targets (300ms fades, 60fps, readability).
- ADR-0011/0012/0013 (temporary BocuD visual simplifications) are superseded.
- `AGENTS.md` port-first section applies to mechanics only.

## Verification

Before merging any mechanics-touching PR:

- [ ] Behavior exists in `references/DTXmaniaNX-BocuD/` and is cited
- [ ] Timing/scoring/lane values match reference

Before merging any UX/UI PR:

- [ ] Follows ADR-0014 design decisions
- [ ] Transitions use unified 300ms OutQuint system (not GitaDora)

## Reference files

- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*` — drum mechanics
- `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs` — stage flow (logic only)
- ADR-0014 — osu-inspired UX redesign
