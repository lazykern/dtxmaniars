# 0012: Song-select visual simplification (M4 minimum viable)

Status: **superseded** by ADR-0014 (2026-06-28)
Date: 2026-06-23

## Supersession

This ADR allowed simplified song-select visuals during M4 (text list only,
no album art/density graph).

ADR-0014 specifies a **modern vertical list with osu-grade polish**:
virtualized scroll, album art panel, density preview, parallax background,
smooth selection animations. BocuD information architecture preserved; BocuD
pixel layout not copied.

## Historical context (archived)

M4 shipped navigation logic verbatim; visuals were intentionally minimal until
M4.1 BocuD visual parity. That parity target is cancelled in favor of ADR-0014.

## Replacement

See `crates/game-menu/src/song_select.rs` and ADR-0014 song-select design.
