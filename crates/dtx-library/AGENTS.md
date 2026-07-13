# crates/dtx-library

Engine-layer song discovery, metadata database, archive import, preview/media
resolution, sort modes, scan diagnostics, and library favorites.

## Current contract

- Recursively discover case-insensitive DTX, GDA, and G2D files; explicitly
  classify BMS/BME as rejected and ignore unrelated extensions.
- Parse charts into `SongInfo`, retain structured warnings/problems and measured
  scan counts/duration, and keep scanning after individual bad charts.
- Resolve BGM, preview, and preimage paths; supported audio is OGG/WAV/MP3.
- `SongDb` supports Default, title, and artist sort plus rescan/refresh. Search,
  difficulty grouping, composable filters, and random selection are Game-menu
  concerns.
- ZIP and 7z import sanitizes paths, extracts through a temporary directory,
  collapses wrappers, rejects duplicates, reports format/media diagnostics, and
  cleans partial imports. RAR is identified but not extracted.
- `LibraryPreferences` persists version-1 favorites JSON.

The scan is synchronous. There is no SQLite cache or reference `SongNode` box
tree. Do not claim extension recognition alone means playable support; follow
[Compatibility](../../docs/compatibility.md).

## Reference evidence

- `references/DTXmaniaNX/DTXMania/SongDb/SongDb.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/SongNode.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortDefault.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortByTitle.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortByArtist.cs`

## Verify

```sh
cargo test -p dtx-library --lib
cargo test -p dtx-library --test import
cargo test -p dtx-library --test bgm_preview
cargo check -p dtx-library
```
