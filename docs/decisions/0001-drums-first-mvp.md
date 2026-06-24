# 0001: Drums-first MVP

Status: accepted
Date: 2026-06-23

## Context

DTXMania supports drums + guitar + bass. BocuD fork's primary instrument is drums.
Gitadora-style 3-mode is a large surface area.

## Decision

Build the MVP around **drums** (the 9-lane kit: HH, SD, BD, HT, LT, CY, FT, HHO, RD).
Guitar/bass come in M6+.

## Consequences

- Easier to ship one playable chart end-to-end (M2).
- 9-lane visualization is simpler than 6-fret color rendering.
- Scoring rules can be DTXmaniaNX-standard; no need to fork for guitar wailing.
- Guitar/Bass data in `.dtx` files is parsed but ignored in v1 (filter channels).

## Alternatives considered

- **Guitar-first:** smaller lane count but adds wailing/long-note complexity.
- **All three from day 1:** 3× the work, 3× the bugs, no shipped MVP.

## Reference files

- `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/` — drums UI
- `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/EChannel.cs:HiHatClose..RideCymbal` — drum channels