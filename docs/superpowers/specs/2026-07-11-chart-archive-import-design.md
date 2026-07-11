# Chart Archive Import — Design

**Date:** 2026-07-11
**Status:** Approved, ready for planning

## Problem

Users download DTX chart packs as `.zip` / `.7z` (occasionally `.rar`) archives —
typically landing in `~/Downloads`. Today the only way to add charts is to manually
extract them into `~/.config/dtxmaniars/` and press F5. This is friction, and the
manual path differs across Windows / macOS / Linux. We want an in-app import that
works identically on all three platforms.

## Goals

- Import a chart archive into the song library from inside the game.
- Cross-platform, no native/system dependencies, no licensing landmines.
- Handle the varied internal shapes real chart archives come in.
- Clear feedback on success and on every failure mode.

## Non-goals

- `.rar` extraction (native lib, non-free license, messy Win/Mac builds). Rar
  archives get a clear "unsupported — extract manually" message instead.
- Auto-watching `~/Downloads`. Explicit user action only.
- Deleting or de-duplicating the source archive after import.

## Architecture

Core logic is a **pure, Bevy-free function** in `crates/dtx-library/src/import.rs`,
consistent with the repo pattern of keeping parser/library logic testable without
Bevy (`dd1facf`). Bevy is only the glue that calls it.

```rust
fn import_archive(archive: &Path, song_root: &Path) -> Result<ImportOutcome, ImportError>
```

### New dependencies

- `zip` — pure-Rust zip extraction.
- `sevenz-rust2` — pure-Rust 7z extraction.
- `rfd` — native file-picker dialog (used only by the Bevy glue).

`encoding_rs` is already a dependency (used for Shift-JIS filename decoding).

## Extraction flow (`import_archive`)

1. **Detect format by magic bytes**, not just extension:
   - `PK\x03\x04` → zip
   - `7z\xBC\xAF\x27\x1C` → 7z
   - `Rar!\x1A\x07` → return `ImportError::UnsupportedFormat("rar")`
   - anything else → `ImportError::UnsupportedFormat(...)`
2. **Extract to a temp dir** (sibling of `song_root`, or system temp). Never write
   into the library until validated — a half-extracted or bogus archive must never
   pollute the song list.
3. **zip-slip guard** (trust boundary — archives are untrusted downloads): reject any
   entry whose normalized path escapes the temp dir (`..`, absolute paths). On
   violation → `ImportError::UnsafePath`, abort, clean up temp.
4. **Filename encoding**: decode each entry name as UTF-8; on failure fall back to
   Shift-JIS via `encoding_rs`. DTX packs are commonly Japanese-encoded.

## Placement algorithm

The library scanner (`scan_directory`) already walks the tree recursively for
`.dtx` files, so multi-song packs do **not** need to be split apart. The import
only needs to (a) avoid a pointless double-nested folder and (b) pick a sane name.

After extraction to `temp_dir`:

```
content = temp_dir
while content has exactly ONE child AND that child is a dir:
    content = that child            # collapse redundant wrapper dir(s)
```

Then move `content` → `song_root/<name>/`, where `<name>` is the collapsed dir's
name if we descended into one, else the archive basename (extension stripped).

Coverage of real archive shapes:

| Archive shape | Result |
|---|---|
| One wrapper dir with chart files inside | `song_root/<wrapper>/` — no double-nest |
| Bare `.dtx` + assets at archive root | `song_root/<archive-name>/` |
| Wrapper dir holding multiple song subdirs | `song_root/<wrapper>/` — scan recurses, finds all |
| Multiple dirs at archive root, no wrapper | `song_root/<archive-name>/` — scan recurses |

### Validation

Before the move: at least one `.dtx` must exist anywhere under `content`. If none →
`ImportError::NoCharts`, abort, clean up temp. (Archive was probably not a chart.)

### Collision handling

If the destination folder name already exists in `song_root`, the import is
skipped: return `ImportError::AlreadyImported(name)` and clean up temp. Never
overwrite existing charts. Rationale: a name collision almost always means the
user re-dropped the same archive; a suffix like ` (2)` would silently duplicate
the song in the list. The rare different-archive-same-name case is resolved by
the user renaming the existing folder.

### ImportOutcome

```rust
struct ImportOutcome {
    dest_name: String,   // final folder name placed in song_root
    chart_count: usize,  // number of .dtx found under it
}
```

### ImportError

```rust
enum ImportError {
    UnsupportedFormat(String),  // "rar", or other magic
    UnsafePath,                 // zip-slip attempt
    NoCharts,                   // no .dtx inside
    AlreadyImported(String),    // dest folder name already in song_root
    Io(std::io::Error),         // extraction / move failure
}
```

## Bevy glue (`game-menu`, song-select screen)

- **Drag & drop**: read `FileDragAndDrop::DroppedFile` events; each dropped path is
  sent to the import worker.
- **File picker**: new key **F6** (F5 is rescan) opens an `rfd` native dialog
  filtered to `*.zip *.7z`, multi-select; selected paths sent to the worker.
- **Background worker**: extraction can be slow for large packs, so `import_archive`
  runs on a background thread. Result is delivered back to the UI via a channel and
  polled each frame — the UI never blocks.
- **On success**: trigger the existing rescan path (`db.rescan`) and show a status
  toast: `imported "Zattou..." (3 charts)`.
- **On failure**: toast the reason:
  - `UnsupportedFormat("rar")` → `unsupported: rar — extract manually`
  - `NoCharts` → `no charts found in archive`
  - `UnsafePath` → `archive rejected (unsafe paths)`
  - `AlreadyImported` → `already imported: "<name>"`
  - `Io` → `import failed: <message>`

The glue stays thin; all logic and edge handling lives in the pure function.

## Testing

Unit tests on `import_archive`, Bevy-free, building tiny in-memory zips per case:

1. Shape A — single wrapper dir with `.dtx` inside → placed without double-nest.
2. Shape B — bare `.dtx` at root → placed under archive-name folder.
3. Shape C — wrapper with multiple song subdirs → all charts scannable after import.
4. Shape D — multiple dirs at root → placed, all charts scannable.
5. zip-slip entry (`../evil`) → `UnsafePath`, nothing written to song_root.
6. Shift-JIS entry name → decoded, no panic, folder created.
7. No `.dtx` inside → `NoCharts`, nothing written.
8. Collision → second import of same name returns `AlreadyImported`, song_root
   unchanged.

Bevy glue (drag-drop wiring, picker, toast) is thin and left untested.

## Open questions

None. Re-import behavior resolved: name collision → skip with
`AlreadyImported` toast (see Collision handling).
