# crates/dtx-library

Engine-layer crate. SongDb — scans a directory of `.dtx` files, parses each
into a `SongInfo`, and supports multiple sort modes.

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX/DTXMania/SongDb/SongDb.cs` (26.5KB, 800+ lines) — full impl
- `references/DTXmaniaNX/DTXMania/SongDb/SongNode.cs` — folder/song tree
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortDefault.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortByTitle.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortByArtist.cs`
- `references/DTXmaniaNX/DTXMania/SongDb/Sorting/SortByDifficulty.cs`

## API

- `SongInfo { path, title, artist, bpm, dlevel, bgm_path }` — one chart
- `SongDb` (Resource) — `Vec<SongInfo>` + scan state
- `scan_directory(root: &Path) -> Result<Vec<SongInfo>, ScanError>` — walk + parse
- `sort_*` functions: `sort_by_title`, `sort_by_artist`, `sort_by_difficulty`
- `SortMode` enum: Default, ByTitle, ByArtist, ByDifficulty

## M5 scope

- Sync scan (no async task — DTXManiaNX uses Task but M5 keeps it simple)
- No SQLite cache (M5+ — SongCacheSqlite.cs port deferred to M6+)
- No zip unpacking (M5+)
- No folder/box tree (M5+ — SongNode.cs port)
- 3 sort modes: Default (file order), ByTitle (alpha), ByArtist (alpha)
- BGM path detection: try `<dtx>.ogg` and `1.ogg` in same dir

## Layer

Engine. Depends on `dtx-core` (Pure). No Game deps.