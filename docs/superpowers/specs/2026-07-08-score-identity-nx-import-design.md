# Score Identity + DTXManiaNX Import — Design

**Date:** 2026-07-08
**Status:** Approved design, pre-plan
**Scope:** ScoreStore v2, canonical chart identity, DTXManiaNX `.score.ini`
import/export compatibility, section IDs, replay metadata skeleton.

## Goal

Replace the current raw-file-hash-only score identity with a stable chart
identity layer while preserving DTXManiaNX compatibility.

The current store works for basic local scores, but a metadata edit,
encoding change, or harmless header reorder changes the raw file hash and can
orphan history. Future systems need a durable key:

- replay files must bind to the chart they were played on,
- practice sections need stable IDs,
- weak-section analysis needs to attach results to the same bar range later,
- imported DTXManiaNX scores should coexist with native scores.

DTXManiaNX `.score.ini` remains a compatibility format. Our versioned JSON
store becomes the source of truth.

## Non-goals

- No UI import screen in this pass.
- No replay playback UI.
- No SQLite/database migration.
- No full reconstruction of old DTXManiaNX play history. NX stores best/last
  records, play/clear counts, and five textual history lines, not detailed
  per-play event streams.
- No changes to judgment windows, score math, or practice behavior.

## Architecture

```text
dtx-core
  └─ parsed Chart, Chip, EChannel
        │
        ▼
dtx-scoring
  ├─ canonical chart hash
  ├─ raw file hash
  ├─ ScoreStore v2 JSON
  ├─ DTXManiaNX .score.ini parser/exporter
  ├─ SectionId
  └─ ReplayHeader skeleton

game-results
  └─ live result resources → ScoreEntry v2
        │
        ├─ write ScoreStore v2
        └─ write/update NX .score.ini

dtx-cli
  └─ scores import-nx <songs-dir>
        │
        └─ scan *.dtx.score.ini → ScoreStore v2
```

Rules:

- `dtx-scoring` stays pure and owns the store, hashes, replay metadata types,
  and `.score.ini` compatibility.
- `dtx-core` remains score-agnostic; it only supplies parsed chart data.
- `game-results` only adapts runtime resources into a score record.
- `dtx-cli` provides the first user-facing import path.
- `.score.ini` import/export never becomes the canonical database.

## Canonical Chart Hash

The current `compute_chart_hash(path)` hashes raw file bytes. The new
canonical hash hashes normalized gameplay content from the parsed chart.

Raw hash:

```text
sha256(all chart file bytes)
```

Canonical hash:

```text
dtx1:sha256(normalized gameplay payload)
```

The normalized payload includes:

- parser/hash version marker,
- base BPM,
- BPM changes,
- bar-length changes,
- playable chips with channel, measure, exact position, and sound slot where
  the slot affects gameplay identity,
- BGM/key-sound timing markers needed to keep replay and scoring identity tied
  to the same chart structure.

The payload excludes:

- title, artist, genre, maker, comment,
- preview/preimage paths,
- file encoding,
- header order,
- comments and whitespace,
- asset filenames when the chip timing/slot identity did not change.

Same song vs same chart:

- Same song, different difficulty: different canonical hash.
- Same chart, metadata edited: same canonical hash.
- Same chart, notes or timing edited: different canonical hash.
- Same path, recharted file: different canonical hash.

The hash prefix (`dtx1:`) is part of the stored key so canonicalization can
evolve later without silently merging incompatible identities.

## ScoreStore v2 Data Model

```rust
pub struct ScoreStore {
    pub version: u32, // 2
    pub entries: Vec<ScoreEntry>,
    pub nx_imports: Vec<NxImportRecord>,
    pub path: Option<PathBuf>, // runtime only; skipped in JSON
}

pub struct ChartIdentity {
    pub canonical_hash: String,
    pub raw_sha256: Option<String>,
    pub raw_sha256_aliases: Vec<String>,
    pub source_path_hint: Option<PathBuf>,
}

pub struct ScoreEntry {
    pub id: String,
    pub chart: ChartIdentity,
    pub title: String,
    pub artist: String,
    pub score: u32,
    pub max_combo: u32,
    pub judgments: JudgmentTotals,
    pub rank: Rank,
    pub played_at: u64,
    pub source: ScoreSource,
    pub replay_ref: Option<ReplayRef>,
}

pub struct JudgmentTotals {
    pub perfect: u32,
    pub great: u32,
    pub good: u32,
    pub poor: u32,
    pub miss: u32,
}

pub enum ScoreSource {
    Native,
    ImportedNxHiScore,
    ImportedNxLastPlay,
}
```

`ok` in current code maps to `poor` in the v2 model to match DTXManiaNX naming
and existing `.score.ini` fields.

`source_path_hint` is optional provenance for backfill/import workflows, not
identity. It should be omitted when unavailable and must never be required for
score lookup. If persisted, implementation should prefer paths relative to the
scanned library root rather than absolute machine-local paths.

NX import metadata:

```rust
pub struct NxImportRecord {
    pub chart: ChartIdentity,
    pub score_ini_path: PathBuf,
    pub play_count: u32,
    pub clear_count: u32,
    pub bgm_adjust: i32,
    pub history: Vec<String>, // History0..History4
}
```

Section identity:

```rust
pub struct SectionId {
    pub canonical_chart_hash: String,
    pub bar_start: u32,
    pub bar_end: u32,
}
```

Replay skeleton:

```rust
pub struct ReplayHeader {
    pub format_version: u16,
    pub engine_version: u16,
    pub chart: ChartIdentity,
    pub played_at: u64,
    pub rate: f32,
    pub input_offset_ms: i32,
    pub bgm_offset_ms: i32,
    pub visual_offset_ms: i32,
}
```

The first pass defines replay metadata and references; it does not need to
record or replay input streams yet.

## Migration

Old `scores.json` entries have `chart_hash` only. Loading migrates them in
memory:

```text
old chart_hash = <raw sha256>
        │
        ▼
chart.canonical_hash = "legacy-raw:<raw sha256>"
chart.raw_sha256 = Some(<raw sha256>)
chart.raw_sha256_aliases = []
source = Native
```

`legacy-raw:` keys remain valid and visible. They are backfilled when the chart
is later available:

```text
scan/load chart
  ├─ compute raw_sha256
  ├─ compute canonical_hash
  └─ if old entry raw_sha256 matches:
       replace legacy-raw:<raw> with dtx1:<canonical>
```

The loader must not overwrite the on-disk file just because migration
happened. It saves only after an explicit save path, such as result save or
import.

Unknown future store versions must not be rewritten. The safe behavior is to
fail load or load read-only and refuse save, with a clear error.

## DTXManiaNX Import

Initial user-facing path:

```sh
cargo run -p dtx-cli -- scores import-nx <songs-dir>
```

Import flow:

```text
find **/*.dtx.score.ini
  │
  ├─ infer chart path by stripping ".score.ini"
  ├─ parse the chart if present
  ├─ compute canonical hash + raw hash
  ├─ parse [File]
  │    ├─ PlayCountDrums
  │    ├─ ClearCountDrums
  │    ├─ BestRankDrums
  │    ├─ BGMAdjust
  │    └─ History0..History4
  ├─ parse [HiScore.Drums]
  ├─ parse [LastPlay.Drums]
  ├─ add imported entries if not duplicate
  └─ save ScoreStore v2
```

If the chart file is missing, the importer may keep an unresolved import record
only when the `.score.ini` contains useful data. It cannot create a canonical
hash until the chart is available.

Duplicate prevention:

- Same canonical hash, source, score, timestamp/history signature: skip.
- Native scores are never overwritten by imported scores.
- Imported NX best and native scores may coexist.
- `best_for_chart()` returns the highest score across native and imported
  entries for the same canonical chart.

## DTXManiaNX Export

Result saving continues to write/update `<chart>.dtx.score.ini` beside the
chart. The export remains drums-focused and BocuD-compatible:

- `[File]` with play/clear count, best rank, BGMAdjust, and history fields,
- `[HiScore.Drums]`,
- `[HiSkill.Drums]` as current best for compatibility,
- `[LastPlay.Drums]`.

This preserves interop with DTXManiaNX while keeping richer native state in
ScoreStore v2.

## Error Handling

- Corrupt `scores.json`: return a clear load error; do not overwrite it.
- Old `scores.json`: migrate in memory; save only on explicit save.
- Malformed `.score.ini`: skip that file, report a skipped count.
- Missing paired `.dtx`: keep unresolved import only if useful score data
  exists; otherwise skip.
- Same canonical hash with multiple raw hashes: keep extra raw hashes in
  `raw_sha256_aliases`, not as separate charts.
- Unknown future store version: return a load error or read-only handle; do not
  mutate the file.

## Testing

Unit tests:

- Canonical hash stays stable across title, artist, comment, whitespace, and
  header-order changes.
- Canonical hash changes when note position, note channel, BPM, or bar length
  changes.
- Old `ScoreEntry` JSON migrates to v2 with `legacy-raw:` identity.
- Runtime `path` is not serialized.
- Extra raw hashes for the same canonical hash are retained as aliases.
- `best_for_chart()` works across native and imported entries.
- Unknown future store versions are not overwritten.

NX import/export tests:

- Parse NX `[File]`, `[HiScore.Drums]`, `[LastPlay.Drums]`, and `History0..4`.
- Import is idempotent.
- Missing chart file does not crash.
- Native and imported records coexist.
- Exported `.score.ini` remains readable by the existing parser.

Integration tests:

- `game-results` saves native v2 entries and updates `.score.ini`.
- `dtx-cli scores import-nx <dir>` imports a small fixture tree.
- Legacy scores remain visible before and after canonical backfill.

## Implementation Split

1. Add canonical hash and identity types in `dtx-scoring`.
2. Introduce ScoreStore v2 with migration from current JSON.
3. Extend `.score.ini` parser/exporter for NX history and last play.
4. Add NX import API in `dtx-scoring`.
5. Add `dtx-cli scores import-nx <songs-dir>`.
6. Wire `game-results` to emit v2 entries and continue `.score.ini` export.
7. Add SectionId and ReplayHeader skeleton types.

This order keeps compatibility active throughout the migration and avoids a
flag day where existing scores disappear.
