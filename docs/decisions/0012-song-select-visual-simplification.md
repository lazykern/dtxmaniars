# 0012: Song-select visual simplification (M4 minimum viable)

Status: accepted (temporary)
Date: 2026-06-23

## Context

DTXManiaNX `CStageSongSelectionNew.cs` is 21.5KB of C# (see also the
companion `SongSelectionContainer.cs` 20.9KB, `StatusPane.cs` 8.2KB,
`SortMenuContainer.cs` 7.0KB, `StatusPanel.cs` 4.5KB, `DensityGraph.cs`
11.5KB — total ~70KB just for song-select rendering). Full port is months
of work.

M4 ships the **logic** port verbatim (per ADR-0010):
- `EReturnValue` enum: Selected/ReturnToTitle/CallConfig/ChangeSkin
- Arrow nav (Up/Down)
- ENTER → Selected → SongLoading
- ESC → ReturnToTitle → Title
- F1 → CallConfig → Config
- `SelectedSong` resource carries the chosen path from SongSelect to SongLoading

The **visuals** are simplified: bevy_ui column with a single hardcoded
song row + selection highlight + info panel. No bigAlbumArt, no density
graphs, no sort menu container, no GITADORA-style background.

## Decision

M4 ships with the simplified visuals. M4.1 will port the visual layout
faithfully:

- Big album art panel (left side, loads from chart's preview image)
- Density graph (preview of note density over time)
- Sort menu container (sort by: default/title/artist/difficulty/level/...)
- Status panel (current sort + chart count)
- Background (skin image + optional video)

`ponytail:` simplified bevy_ui for M4 — no album art, no density graphs,
no sort menu. Just a list + arrow nav. Logic is exact.

## Consequences

- Song-select feels "minimal" in M4. Acceptable because the architecture
  is proven end-to-end (select → load → play).
- M4.1 restores visual parity with DTXManiaNX. Until then, contributors
  should NOT propose visual changes ("make it look nicer") — that's
  out of scope per ADR-0010 strict-port rule.
- `m4_song_list()` is hardcoded to the `drums_basic.dtx` fixture. Real
  SongDb (background scan of song directory + multi-song sort) is M5+.
- SelectedSong resource is set in `song_select_navigation` and consumed
  in `song_loading::start_load`. Same shape works for M5's SongDb.

## When to revisit

- M4.1: visual parity (album art, density graph, sort menu).
- M5: real SongDb replaces hardcoded list.
- M6+: osu-lazer-style song-select improvements (deferred, blocked by
  ADR-0010 until port baseline ships).