# 0010: Port-first — preserve DTXManiaNX UX/UI as baseline; improvements later

Status: accepted
Date: 2026-06-23

## Context

We are **porting** DTXManiaNX-BocuD (C# / DX9) to Rust/Bevy. We are NOT
building a new rhythm game. The two must not be confused:

- The **port's job** is to make a working Rust equivalent whose UX/UI
  matches DTXManiaNX closely enough that an existing DTXMania player feels
  at home.
- **Improvements** (osu-lazer-grade fluidity, custom skins, new modes) are
  separate, opt-in work that ships AFTER the port baseline is stable.

This rule matters because porting tempts you to "improve while you're here."
Every visible UX element — fade durations, lane positions, judgment timing
windows, HUD layout, transition choreography — has a specific value in the
DTXManiaNX reference. Inventing your own value, even if "better," is scope
creep and breaks the goal.

## Decision

**Strict port baseline.** For every UX/UI element in v1:

1. **Source of truth** is `references/DTXmaniaNX-BocuD/`. Not osu-lazer.
   Not your own design instinct. Not what's "modern."
2. **Match the reference verbatim** for: lane visual order + X coordinates,
   judgment timing windows, fade durations, HUD position/size, hit line
   position, scroll direction, combo/score animations, transition style
   (snapshot vs live), input bindings (default), judgment text style/colors,
   song-select sort modes, everything else.
3. **Cite the reference file:line** in the commit for any non-trivial UX
   element ported (same rule as ADR-0008 for general code).
4. **Improvements are blocked** until the port baseline ships (M6+). Exception:
   correctness fixes for outright bugs (data corruption, crashes).
5. **BocuD is the baseline, not upstream DTXManiaNX.** BocuD's UX fixes
   (new renderer, animation framework, sorted settings, song select improvements)
   are part of "DTXManiaNX UX" for our purposes. Upstream DTXManiaNX is a
   fallback reference for systems BocuD hasn't touched.

## Concrete examples of what this means

| Element | My instinct | Strict port says |
|---|---|---|
| Stage fade duration | 300ms OutQuint (osu-lazer) | **1500ms linear** (StageManager.cs:29) |
| Lane order | whatever feels right | LC, HH, SD, BD, HT, LT, FT, CY, LP, RD, HHO (CActPerfDrumsLaneFlushD.cs) |
| Judgment windows | ±15/±30/±60/±90/±150ms | match DTXmaniaNX `ConfigIni.e判定タイミング` defaults |
| Song select sort | nice categories | match BocuD's GITADORA-style sort (SongSelectionContainer.cs) |
| Combo counter | top-left small text | match DTXmania's CActPerfDrumsComboDGB.cs position/font |

## Consequences

- Port will feel "old" / less fluid than osu-lazer. **That's the point.**
- Tests, screenshots, and community feedback will compare against DTXManiaNX,
  not osu-lazer, until M6+ improvements land.
- **Existing contradictions must be fixed:**
  - `crates/dtx-ui/src/lib.rs` `SCREEN_FADE_MS = 300` → **1500** (DTXmaniaNX value)
  - `docs/BEVY_PATTERNS.md` `SCREEN_FADE_MS: u32 = 300` → **1500**
  - `docs/ROADMAP.md` M3 "osu-style fades (300ms OutQuint, 1800ms load hold)"
    → "DTXmaniaNX fades (1500ms snapshot)"
  - `crates/dtx-scoring/src/lib.rs` `DEFAULT_WINDOWS_MS` — verify against
    DTXmaniaNX `ConfigIni` defaults, not invented values.
- AI agents must NOT propose "improvements" during port work. If a suggestion
  would change visible UX, defer to M6+ or reject.
- The "osu-lazer fluidity" target from the project brief is still valid —
  it's the M6+ destination. Not a v1 requirement.

## Alternatives considered

- **Mix port + improvements opportunistically:** bad — produces inconsistent
  UX (some screens old, some new), hard to test, hard to attribute bugs.
- **Use osu-lazer UX as the v1 baseline:** scope explosion (rewrite from
  scratch), defeats the "port" goal.
- **Re-design UX while porting:** never ships, breaks DTXMania compatibility.

## Verification

Before merging any UX-touching PR:

- [ ] Element exists in `references/DTXmaniaNX-BocuD/` and is cited
- [ ] Value/size/position matches the reference (or has a documented reason
      to differ, with an ADR superseding this one)
- [ ] No new visual elements that don't exist in DTXManiaNX

## Reference files

- `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs:29` — fade = 1500ms
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/*` — drum UI
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/*` — song select
- `references/DTXmaniaNX-BocuD/DTXMania/Stage/07.Result/*` — results screen