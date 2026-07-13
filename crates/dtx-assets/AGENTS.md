# crates/dtx-assets

Engine-layer crate. Loads DTX chart files into [`Chart`](dtx_core::Chart)
values for use by gameplay crates.

## Reference files (READ BEFORE IMPLEMENTING per ADR-0008)

- `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs` (272KB) — DTX parser
- `references/DTXmaniaNX/DTXMania/Score,Song/CChartData.cs` — Chart container
- `references/DTXmaniaNX/DTXMania/Stage/05.SongLoading/CStageSongLoading.cs` (1110 lines) — loading flow

## API

- `load_dtx(path: &Path) -> Result<Chart, DtxError>` — read file + parse
- `DtxCache` (Resource) — `HashMap<PathBuf, Chart>` keyed by path
- `DtxCache::get_or_load(&mut self, path: &Path) -> Result<&Chart, DtxError>`

## Layer

Engine. May depend on `dtx-core` (Pure). Must NOT depend on Game crates.

## v1 scope (M4)

- File-based loading only
- In-memory cache
- No bevy AssetLoader integration yet (M5+ when many DTX files load)
- No BGM asset loading yet (M5+)

## Future

- M5: bevy AssetLoader integration
- M5: BGM asset loading via `bevy_kira_audio`
- M6: SongDb indexing (background scan of song directory)