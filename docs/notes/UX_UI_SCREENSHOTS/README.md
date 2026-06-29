# UX/UI screenshot baselines (ADR-0014)

Capture at **1280×720** with `DTXMANIARS_WINDOWED=1`.

## Method

1. Run `cargo run -p dtxmaniars-desktop --features brp` (default features include `brp`).
2. Use `bevy_brp_mcp` tool `brp_extras/screenshot`, or manual capture.
3. Save PNGs here: `{screen}.png` (e.g. `title.png`, `song_select.png`).

## Screens

| File | AppState |
|---|---|
| startup.png | Startup |
| title.png | Title |
| song_select.png | SongSelect |
| song_loading.png | SongLoading |
| performance.png | Performance |
| result.png | Result |
| config.png | Config |

Manual pixel diff only for v1 — no CI regression yet.
