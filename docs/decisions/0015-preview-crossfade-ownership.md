# ADR-0015 — Preview Crossfade Ownership

Status: Accepted (reconstructed 2026-07-13)

## Context

Song selection needs one owner for preview audio state while library discovery,
menu selection, album art, parallax, and screen transitions remain separate.

## Decision

`dtx-library` resolves a chart's explicit preview and falls back to its BGM.
`dtx-audio::PreviewPlayer` owns handle caching and the crossfade state machine.
`game-menu` requests swaps; UI consumers observe `PreviewSwapEvent`; game-shell
publishes screen-fade events used to align audio fade with navigation.

## Evidence

- [preview/BGM resolution](../../crates/dtx-library/src/lib.rs)
- [crossfade state owner](../../crates/dtx-audio/src/preview.rs)
- [song-select request path](../../crates/game-menu/src/song_select.rs)
- [screen-fade publication](../../crates/game-shell/src/transition.rs)

## Consequences

Widgets do not play audio directly, repeated selections deduplicate, and a
full-song fallback is not treated as a looping preview clip.

## Supersedes / Superseded By

Reconstructs the implemented ADR-0015 phases; superseded by none.
