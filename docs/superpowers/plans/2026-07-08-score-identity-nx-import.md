# Score Identity + NX Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build ScoreStore v2 with stable canonical chart identity, DTXManiaNX `.score.ini` import compatibility, and section/replay foundation types.

**Architecture:** `dtx-scoring` owns pure identity, score-store, NX import/export, section ID, and replay metadata logic. `game-results` adapts runtime result resources into v2 score entries and continues writing NX-compatible `.score.ini`; `dtx-cli` exposes `scores import-nx <songs-dir>` as the first import surface.

**Tech Stack:** Rust workspace, `dtx-core` parsed `Chart`, `dtx-scoring` pure persistence, `serde`/`serde_json`, `sha2`, `clap`, `anyhow`, Cargo tests.

---

## Spec

Approved design: `docs/superpowers/specs/2026-07-08-score-identity-nx-import-design.md`

## File Structure

- Create `crates/dtx-scoring/src/identity.rs`
  - Owns `ChartIdentity`, `SectionId`, `raw_file_sha256`, and `canonical_chart_hash`.
- Create `crates/dtx-scoring/src/replay.rs`
  - Owns `ReplayHeader` and `ReplayRef` metadata skeletons.
- Create `crates/dtx-scoring/src/store.rs`
  - Owns ScoreStore v2, legacy JSON migration, best-score lookup, duplicate handling, and save/load.
- Create `crates/dtx-scoring/src/nx_import.rs`
  - Owns scan/import data flow for `.dtx.score.ini` files.
- Modify `crates/dtx-scoring/src/lib.rs`
  - Re-export new modules and remove old store definitions after migration.
- Modify `crates/dtx-scoring/src/score_ini.rs`
  - Extend parser/exporter to read `History0..History4` and `LastPlay.Drums`.
- Modify `crates/game-results/src/lib.rs`
  - Build v2 `ScoreEntry` from runtime resources and preserve NX export.
- Modify `tools/dtx-cli/src/main.rs`
  - Add `scores import-nx <songs-dir>`.
- Modify `crates/dtx-scoring/tests/edge_cases.rs`
  - Update store tests to v2 API.
- Add `crates/dtx-scoring/tests/identity.rs`
  - Canonical hash stability/change tests.
- Add `crates/dtx-scoring/tests/store_v2.rs`
  - Migration and lookup tests.
- Add `crates/dtx-scoring/tests/nx_import.rs`
  - NX parser/import tests.

## Task 1: Canonical Chart Identity

**Files:**
- Create: `crates/dtx-scoring/src/identity.rs`
- Modify: `crates/dtx-scoring/src/lib.rs`
- Test: `crates/dtx-scoring/tests/identity.rs`

- [ ] **Step 1: Write failing identity tests**

Create `crates/dtx-scoring/tests/identity.rs`:

```rust
use dtx_core::parse_str;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity, SectionId};

fn parse(input: &str) -> dtx_core::Chart {
    parse_str(input).expect("fixture must parse")
}

#[test]
fn canonical_hash_ignores_metadata_and_comments() {
    let a = parse(
        r#"
#TITLE: Song A
#ARTIST: Alice
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
; changed comment
#ARTIST: Bob
#TITLE: Song A fixed title
#BPM: 120
#00111: 0100
"#,
    );

    assert_eq!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn canonical_hash_changes_when_note_moves() {
    let a = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
#BPM: 120
#00111: 0010
"#,
    );

    assert_ne!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn canonical_hash_changes_when_timing_changes() {
    let a = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let b = parse(
        r#"
#BPM: 121
#00111: 0100
"#,
    );

    assert_ne!(canonical_chart_hash(&a), canonical_chart_hash(&b));
}

#[test]
fn section_id_uses_canonical_chart_hash_and_bars() {
    let chart = parse(
        r#"
#BPM: 120
#00111: 0100
"#,
    );
    let section = SectionId::new(canonical_chart_hash(&chart), 4, 8);
    assert_eq!(section.bar_start, 4);
    assert_eq!(section.bar_end, 8);
    assert!(section.canonical_chart_hash.starts_with("dtx1:"));
}

#[test]
fn raw_file_hash_is_plain_sha256_hex() {
    let dir = std::env::temp_dir().join(format!("dtx_identity_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chart.dtx");
    std::fs::write(&path, b"#TITLE: X\n").unwrap();

    let hash = raw_file_sha256(&path).unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn chart_identity_keeps_aliases_unique() {
    let mut id = ChartIdentity::legacy_raw("abc".to_string());
    id.add_raw_alias("def".to_string());
    id.add_raw_alias("def".to_string());

    assert_eq!(id.canonical_hash, "legacy-raw:abc");
    assert_eq!(id.raw_sha256.as_deref(), Some("abc"));
    assert_eq!(id.raw_sha256_aliases, vec!["def"]);
}
```

- [ ] **Step 2: Run identity tests to verify they fail**

Run: `cargo test -p dtx-scoring --test identity`

Expected: FAIL because `dtx_scoring::identity` does not exist.

- [ ] **Step 3: Implement `identity.rs`**

Create `crates/dtx-scoring/src/identity.rs`:

```rust
//! Stable chart identity types and hash functions.

use std::path::{Path, PathBuf};

use dtx_core::{Chart, EChannel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable identity for a parsed chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartIdentity {
    /// Primary chart key. New charts use `dtx1:<sha256>`.
    pub canonical_hash: String,
    /// Raw-file SHA-256 for compatibility with legacy stores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_sha256: Option<String>,
    /// Additional raw hashes seen for the same canonical chart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_sha256_aliases: Vec<String>,
    /// Optional provenance hint. Not used for identity lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path_hint: Option<PathBuf>,
}

impl ChartIdentity {
    /// Build identity for a parsed chart, with optional raw hash and path hint.
    pub fn new(
        canonical_hash: String,
        raw_sha256: Option<String>,
        source_path_hint: Option<PathBuf>,
    ) -> Self {
        Self {
            canonical_hash,
            raw_sha256,
            raw_sha256_aliases: Vec::new(),
            source_path_hint,
        }
    }

    /// Build a migrated identity when only the old raw hash is known.
    pub fn legacy_raw(raw_sha256: String) -> Self {
        Self {
            canonical_hash: format!("legacy-raw:{raw_sha256}"),
            raw_sha256: Some(raw_sha256),
            raw_sha256_aliases: Vec::new(),
            source_path_hint: None,
        }
    }

    /// Add a raw hash alias if it is distinct from the primary raw hash.
    pub fn add_raw_alias(&mut self, raw_sha256: String) {
        if self.raw_sha256.as_deref() == Some(raw_sha256.as_str()) {
            return;
        }
        if !self.raw_sha256_aliases.iter().any(|h| h == &raw_sha256) {
            self.raw_sha256_aliases.push(raw_sha256);
        }
    }

    /// True when any raw hash slot matches `raw`.
    pub fn matches_raw(&self, raw: &str) -> bool {
        self.raw_sha256.as_deref() == Some(raw)
            || self.raw_sha256_aliases.iter().any(|h| h == raw)
    }
}

/// Durable practice/result section key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId {
    /// Canonical chart hash.
    pub canonical_chart_hash: String,
    /// Inclusive start bar.
    pub bar_start: u32,
    /// Exclusive end bar.
    pub bar_end: u32,
}

impl SectionId {
    /// Construct a section key.
    pub fn new(canonical_chart_hash: String, bar_start: u32, bar_end: u32) -> Self {
        Self {
            canonical_chart_hash,
            bar_start,
            bar_end,
        }
    }
}

/// Compute SHA-256 over raw file bytes.
pub fn raw_file_sha256(path: impl AsRef<Path>) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex_sha256(&bytes))
}

/// Compute the v1 canonical chart hash from parsed gameplay content.
pub fn canonical_chart_hash(chart: &Chart) -> String {
    let payload = canonical_payload(chart);
    format!("dtx1:{}", hex_sha256(payload.as_bytes()))
}

fn canonical_payload(chart: &Chart) -> String {
    let mut lines = Vec::new();
    lines.push("dtx-chart-id-v1".to_string());
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    lines.push(format!("base_bpm={}", stable_f32(base_bpm)));

    let mut chips = chart.chips.clone();
    chips.sort_by(|a, b| {
        (a.measure, stable_f32(a.position), a.channel as u8, stable_f32(a.value), a.wav_slot)
            .cmp(&(
                b.measure,
                stable_f32(b.position),
                b.channel as u8,
                stable_f32(b.value),
                b.wav_slot,
            ))
    });

    for chip in chips {
        if identity_channel(chip.channel) {
            lines.push(format!(
                "chip m={} c={:02X} p={} v={} wav={}",
                chip.measure,
                chip.channel as u8,
                stable_f32(chip.position),
                stable_f32(chip.value),
                chip.wav_slot
            ));
        }
    }

    lines.join("\n")
}

fn identity_channel(channel: EChannel) -> bool {
    channel.is_drum()
        || channel.is_guitar()
        || matches!(
            channel,
            EChannel::BGM | EChannel::BPM | EChannel::BPMEx | EChannel::BarLength
        )
}

fn stable_f32(value: f32) -> String {
    format!("{value:.6}")
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}
```

- [ ] **Step 4: Export identity module**

Modify `crates/dtx-scoring/src/lib.rs` near the module list:

```rust
pub mod gauge;
pub mod hit_ranges;
pub mod identity;
pub mod score_ini;
pub mod xg_score;
```

- [ ] **Step 5: Run identity tests to verify they pass**

Run: `cargo test -p dtx-scoring --test identity`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/dtx-scoring/src/identity.rs crates/dtx-scoring/src/lib.rs crates/dtx-scoring/tests/identity.rs
git commit -m "feat(scoring): add canonical chart identity"
```

## Task 2: ScoreStore v2 and Legacy Migration

**Files:**
- Create: `crates/dtx-scoring/src/store.rs`
- Modify: `crates/dtx-scoring/src/lib.rs`
- Modify: `crates/dtx-scoring/tests/edge_cases.rs`
- Test: `crates/dtx-scoring/tests/store_v2.rs`

- [ ] **Step 1: Write failing store v2 tests**

Create `crates/dtx-scoring/tests/store_v2.rs`:

```rust
use dtx_scoring::identity::ChartIdentity;
use dtx_scoring::store::{
    JudgmentTotals, ScoreEntry, ScoreSource, ScoreStore, ScoreStoreError, STORE_VERSION,
};
use dtx_scoring::Rank;

fn entry(hash: &str, score: u32, source: ScoreSource) -> ScoreEntry {
    ScoreEntry {
        id: format!("{hash}:{score}:{source:?}"),
        chart: ChartIdentity::new(hash.to_string(), None, None),
        title: "Title".to_string(),
        artist: "Artist".to_string(),
        score,
        max_combo: 10,
        judgments: JudgmentTotals {
            perfect: 8,
            great: 1,
            good: 1,
            poor: 0,
            miss: 0,
        },
        rank: Rank::S,
        played_at: 123,
        source,
        replay_ref: None,
    }
}

#[test]
fn score_store_v2_round_trips_without_runtime_path() {
    let dir = std::env::temp_dir().join(format!("dtx_store_v2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.json");
    let _ = std::fs::remove_file(&path);

    let mut store = ScoreStore::with_path(path.clone());
    store.add(entry("dtx1:abc", 1000, ScoreSource::Native));
    store.save().unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\"version\": 2"));
    assert!(!raw.contains("\"path\""));

    let mut loaded = ScoreStore::with_path(path.clone());
    loaded.load().unwrap();
    assert_eq!(loaded.version, STORE_VERSION);
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].score, 1000);

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn old_score_json_migrates_to_legacy_raw_identity() {
    let dir = std::env::temp_dir().join(format!("dtx_store_old_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.json");
    let old = r#"{
  "entries": [
    {
      "chart_hash": "abc",
      "title": "Old",
      "artist": "Artist",
      "score": 1234,
      "max_combo": 99,
      "perfect": 90,
      "great": 5,
      "good": 3,
      "ok": 1,
      "miss": 0,
      "rank": "S",
      "played_at": 1700000000
    }
  ],
  "path": "scores.json"
}"#;
    std::fs::write(&path, old).unwrap();

    let mut store = ScoreStore::with_path(path.clone());
    store.load().unwrap();

    assert_eq!(store.entries.len(), 1);
    assert_eq!(store.entries[0].chart.canonical_hash, "legacy-raw:abc");
    assert_eq!(store.entries[0].chart.raw_sha256.as_deref(), Some("abc"));
    assert_eq!(store.entries[0].judgments.poor, 1);

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn best_for_chart_uses_highest_score_across_sources() {
    let mut store = ScoreStore::default();
    store.add(entry("dtx1:same", 1000, ScoreSource::ImportedNxHiScore));
    store.add(entry("dtx1:same", 2000, ScoreSource::Native));
    store.add(entry("dtx1:other", 9999, ScoreSource::Native));

    assert_eq!(store.best_for_chart("dtx1:same").unwrap().score, 2000);
}

#[test]
fn duplicate_imported_entries_are_skipped() {
    let mut store = ScoreStore::default();
    let imported = entry("dtx1:same", 1000, ScoreSource::ImportedNxHiScore);
    store.add_if_new(imported.clone());
    store.add_if_new(imported);

    assert_eq!(store.entries.len(), 1);
}

#[test]
fn future_store_version_is_not_overwritten() {
    let dir = std::env::temp_dir().join(format!("dtx_store_future_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.json");
    std::fs::write(&path, r#"{"version":999,"entries":[],"nx_imports":[]}"#).unwrap();

    let mut store = ScoreStore::with_path(path.clone());
    let err = store.load().unwrap_err();
    assert!(matches!(err, ScoreStoreError::UnsupportedVersion(999)));

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\"version\":999"));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}
```

- [ ] **Step 2: Run store tests to verify they fail**

Run: `cargo test -p dtx-scoring --test store_v2`

Expected: FAIL because `dtx_scoring::store` does not exist.

- [ ] **Step 3: Implement `store.rs`**

Create `crates/dtx-scoring/src/store.rs`:

```rust
//! Versioned score store and legacy migration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::ChartIdentity;
use crate::replay::ReplayRef;
use crate::Rank;

/// Current native store version.
pub const STORE_VERSION: u32 = 2;

/// One score store file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreStore {
    /// Serialized schema version.
    #[serde(default = "store_version")]
    pub version: u32,
    /// Native and imported score entries.
    #[serde(default)]
    pub entries: Vec<ScoreEntry>,
    /// Imported NX metadata that is not a score entry.
    #[serde(default)]
    pub nx_imports: Vec<NxImportRecord>,
    /// Runtime load/save path. Not serialized.
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Default for ScoreStore {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            entries: Vec::new(),
            nx_imports: Vec::new(),
            path: None,
        }
    }
}

/// One persisted play/result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoreEntry {
    /// Stable entry identifier.
    pub id: String,
    /// Chart identity.
    pub chart: ChartIdentity,
    /// Song title at time of play/import.
    pub title: String,
    /// Song artist at time of play/import.
    pub artist: String,
    /// Score value.
    pub score: u32,
    /// Maximum combo.
    pub max_combo: u32,
    /// Judgment totals.
    pub judgments: JudgmentTotals,
    /// Result rank.
    pub rank: Rank,
    /// Unix seconds.
    pub played_at: u64,
    /// Origin of this score.
    pub source: ScoreSource,
    /// Optional replay reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_ref: Option<ReplayRef>,
}

impl ScoreEntry {
    /// Total judgments.
    pub fn total(&self) -> u32 {
        self.judgments.total()
    }

    /// Perfect percentage.
    pub fn perfect_pct(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.judgments.perfect as f32 / total as f32 * 100.0
        }
    }
}

/// Judgment count bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JudgmentTotals {
    /// Perfect count.
    pub perfect: u32,
    /// Great count.
    pub great: u32,
    /// Good count.
    pub good: u32,
    /// Poor count. This replaces old `ok`.
    pub poor: u32,
    /// Miss count.
    pub miss: u32,
}

impl JudgmentTotals {
    /// Sum of all judgments.
    pub fn total(&self) -> u32 {
        self.perfect + self.great + self.good + self.poor + self.miss
    }
}

/// Score origin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScoreSource {
    /// Native DTXManiaRS result.
    Native,
    /// Imported DTXManiaNX best score.
    ImportedNxHiScore,
    /// Imported DTXManiaNX last play.
    ImportedNxLastPlay,
}

/// NX metadata imported from `[File]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NxImportRecord {
    /// Chart identity.
    pub chart: ChartIdentity,
    /// Path to the imported score.ini file.
    pub score_ini_path: PathBuf,
    /// NX play count.
    pub play_count: u32,
    /// NX clear count.
    pub clear_count: u32,
    /// NX per-song BGM adjust.
    pub bgm_adjust: i32,
    /// History0..History4 strings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<String>,
}

/// Store load/save errors.
#[derive(Debug, Error)]
pub enum ScoreStoreError {
    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Store file is newer than this binary supports.
    #[error("unsupported score store version {0}")]
    UnsupportedVersion(u32),
}

impl ScoreStore {
    /// Construct a store backed by `path`.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            ..Self::default()
        }
    }

    /// Default score path.
    pub fn default_path() -> PathBuf {
        if let Ok(p) = std::env::var("DTX_SCORES_PATH") {
            return PathBuf::from(p);
        }
        PathBuf::from("scores.json")
    }

    /// Load from `self.path`.
    pub fn load(&mut self) -> Result<(), ScoreStoreError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !path.exists() {
            return Ok(());
        }
        let bytes = std::fs::read(path)?;
        if bytes.is_empty() {
            return Ok(());
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let parsed = parse_store_value(value)?;
        self.version = parsed.version;
        self.entries = parsed.entries;
        self.nx_imports = parsed.nx_imports;
        Ok(())
    }

    /// Save to `self.path`.
    pub fn save(&self) -> Result<(), ScoreStoreError> {
        if self.version > STORE_VERSION {
            return Err(ScoreStoreError::UnsupportedVersion(self.version));
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Add an entry.
    pub fn add(&mut self, entry: ScoreEntry) {
        self.entries.push(entry);
    }

    /// Add an entry when an equivalent entry is not already present.
    pub fn add_if_new(&mut self, entry: ScoreEntry) {
        if !self.entries.iter().any(|existing| equivalent_entry(existing, &entry)) {
            self.entries.push(entry);
        }
    }

    /// Best score for a canonical chart hash.
    pub fn best_for_chart(&self, canonical_hash: &str) -> Option<&ScoreEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.chart.canonical_hash == canonical_hash)
            .max_by_key(|entry| entry.score)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no entries exist.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn store_version() -> u32 {
    STORE_VERSION
}

fn parse_store_value(value: serde_json::Value) -> Result<ScoreStore, ScoreStoreError> {
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    if version > STORE_VERSION {
        return Err(ScoreStoreError::UnsupportedVersion(version));
    }
    if version == 1 {
        let legacy: LegacyScoreStore = serde_json::from_value(value)?;
        return Ok(legacy.into_store_v2());
    }
    let mut store: ScoreStore = serde_json::from_value(value)?;
    store.path = None;
    Ok(store)
}

fn equivalent_entry(a: &ScoreEntry, b: &ScoreEntry) -> bool {
    a.chart.canonical_hash == b.chart.canonical_hash
        && a.source == b.source
        && a.score == b.score
        && a.played_at == b.played_at
        && a.judgments == b.judgments
}

#[derive(Debug, Deserialize)]
struct LegacyScoreStore {
    #[serde(default)]
    entries: Vec<LegacyScoreEntry>,
}

impl LegacyScoreStore {
    fn into_store_v2(self) -> ScoreStore {
        let entries = self.entries.into_iter().map(LegacyScoreEntry::into_v2).collect();
        ScoreStore {
            version: STORE_VERSION,
            entries,
            nx_imports: Vec::new(),
            path: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacyScoreEntry {
    chart_hash: String,
    title: String,
    artist: String,
    score: u32,
    max_combo: u32,
    perfect: u32,
    great: u32,
    good: u32,
    ok: u32,
    miss: u32,
    rank: Rank,
    played_at: u64,
}

impl LegacyScoreEntry {
    fn into_v2(self) -> ScoreEntry {
        ScoreEntry {
            id: format!("legacy:{}:{}", self.chart_hash, self.played_at),
            chart: ChartIdentity::legacy_raw(self.chart_hash),
            title: self.title,
            artist: self.artist,
            score: self.score,
            max_combo: self.max_combo,
            judgments: JudgmentTotals {
                perfect: self.perfect,
                great: self.great,
                good: self.good,
                poor: self.ok,
                miss: self.miss,
            },
            rank: self.rank,
            played_at: self.played_at,
            source: ScoreSource::Native,
            replay_ref: None,
        }
    }
}
```

- [ ] **Step 4: Add replay skeleton required by store**

Create `crates/dtx-scoring/src/replay.rs`:

```rust
//! Replay metadata skeleton.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::ChartIdentity;

/// Reference to a replay file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRef {
    /// Replay format version.
    pub format_version: u16,
    /// Relative or absolute replay path.
    pub path: PathBuf,
}

/// Header metadata for future replay files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayHeader {
    /// Replay file format version.
    pub format_version: u16,
    /// Scoring/judgment engine version.
    pub engine_version: u16,
    /// Chart identity.
    pub chart: ChartIdentity,
    /// Unix seconds.
    pub played_at: u64,
    /// Playback rate.
    pub rate: f32,
    /// Input offset in milliseconds.
    pub input_offset_ms: i32,
    /// BGM offset in milliseconds.
    pub bgm_offset_ms: i32,
    /// Visual offset in milliseconds.
    pub visual_offset_ms: i32,
}
```

- [ ] **Step 5: Re-export store and replay from lib**

Modify `crates/dtx-scoring/src/lib.rs` module list:

```rust
pub mod gauge;
pub mod hit_ranges;
pub mod identity;
pub mod replay;
pub mod score_ini;
pub mod store;
pub mod xg_score;

pub use store::{JudgmentTotals, NxImportRecord, ScoreEntry, ScoreSource, ScoreStore, ScoreStoreError};
```

Keep `compute_chart_hash` temporarily for consumers, but change it to call `identity::raw_file_sha256`:

```rust
pub fn compute_chart_hash(path: &Path) -> String {
    identity::raw_file_sha256(path).unwrap_or_else(|_| {
        let mut hasher = Sha256::new();
        hasher.update(path.to_string_lossy().as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in digest {
            use std::fmt::Write as _;
            let _ = write!(&mut hex, "{b:02x}");
        }
        hex
    })
}
```

- [ ] **Step 6: Update existing edge-case tests to v2 constructors**

Modify `crates/dtx-scoring/tests/edge_cases.rs` imports:

```rust
use dtx_scoring::identity::ChartIdentity;
use dtx_scoring::{JudgmentKind, JudgmentTotals, Rank, ScoreEntry, ScoreSource, ScoreStore};
```

Replace repeated struct literals for `ScoreEntry` with this helper near the bottom:

```rust
fn make_entry(hash: &str, score: u32) -> ScoreEntry {
    ScoreEntry {
        id: format!("{hash}:{score}"),
        chart: ChartIdentity::new(hash.to_string(), None, None),
        title: "T".into(),
        artist: "A".into(),
        score,
        max_combo: 0,
        judgments: JudgmentTotals {
            perfect: 0,
            great: 0,
            good: 0,
            poor: 0,
            miss: 0,
        },
        rank: Rank::S,
        played_at: 0,
        source: ScoreSource::Native,
        replay_ref: None,
    }
}
```

For tests that need non-zero totals, mutate `judgments`:

```rust
let mut e = make_entry("h", 0);
e.judgments.perfect = 50;
e.judgments.great = 30;
e.judgments.good = 10;
e.judgments.poor = 5;
e.judgments.miss = 5;
assert_eq!(e.total(), 100);
```

- [ ] **Step 7: Run store tests**

Run: `cargo test -p dtx-scoring --test store_v2`

Expected: PASS.

- [ ] **Step 8: Run existing scoring tests**

Run: `cargo test -p dtx-scoring`

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/dtx-scoring/src/lib.rs crates/dtx-scoring/src/replay.rs crates/dtx-scoring/src/store.rs crates/dtx-scoring/tests/edge_cases.rs crates/dtx-scoring/tests/store_v2.rs
git commit -m "feat(scoring): migrate score store to v2"
```

## Task 3: Extend DTXManiaNX `.score.ini` Parsing

**Files:**
- Modify: `crates/dtx-scoring/src/score_ini.rs`
- Test: `crates/dtx-scoring/tests/nx_import.rs`

- [ ] **Step 1: Write failing `.score.ini` parser tests**

Create `crates/dtx-scoring/tests/nx_import.rs`:

```rust
use dtx_scoring::score_ini::{parse_score_ini_text, DrumScoreIni};

fn sample_score_ini() -> &'static str {
    r#"[File]
Title=Sample
Name=Tester
PlayCountDrums=7
PlayCountGuitars=0
PlayCountBass=0
ClearCountDrums=3
ClearCountGuitars=0
ClearCountBass=0
BestRankDrums=1
BestRankGuitar=99
BestRankBass=99
HistoryCount=2
History0=2.26/7/8 Stage cleared
History1=1.26/7/7 Stage failed
BGMAdjust=-12

[HiScore.Drums]
Score=900000
Perfect=80
Great=10
Good=5
Poor=3
Miss=2
MaxCombo=88
TotalChips=100
Drums=1
DateTime=2026/7/8 10:11:12

[LastPlay.Drums]
Score=800000
Perfect=70
Great=15
Good=5
Poor=5
Miss=5
MaxCombo=66
TotalChips=100
Drums=1
DateTime=2026/7/8 11:12:13
"#
}

#[test]
fn parses_file_history_best_and_last_play() {
    let parsed = parse_score_ini_text(sample_score_ini()).unwrap();

    assert_eq!(parsed.file.play_count_drums, 7);
    assert_eq!(parsed.file.clear_count_drums, 3);
    assert_eq!(parsed.file.bgm_adjust, -12);
    assert_eq!(parsed.file.history, vec!["2.26/7/8 Stage cleared", "1.26/7/7 Stage failed"]);

    assert_eq!(parsed.hi_score_drums.as_ref().unwrap().score, 900000);
    assert_eq!(parsed.last_play_drums.as_ref().unwrap().score, 800000);
}

#[test]
fn rendered_score_ini_keeps_history_fields() {
    let mut best = DrumScoreIni {
        score: 100,
        perfect: 10,
        great: 0,
        good: 0,
        poor: 0,
        miss: 0,
        max_combo: 10,
        total_chips: 10,
        rank: "SS".to_string(),
        play_count: 2,
        clear_count: 1,
        bgm_adjust: 5,
        date_time: "2026/7/8 1:02:03".to_string(),
    };
    let text = dtx_scoring::score_ini::render_with_history(
        &best,
        &best,
        &["2.26/7/8 Stage cleared".to_string(), "1.26/7/7 Stage failed".to_string()],
    );
    let parsed = parse_score_ini_text(&text).unwrap();

    assert_eq!(parsed.file.history_count, 2);
    assert_eq!(parsed.file.history[0], "2.26/7/8 Stage cleared");
    assert_eq!(parsed.hi_score_drums.take().unwrap().score, 100);
}
```

- [ ] **Step 2: Run parser tests to verify they fail**

Run: `cargo test -p dtx-scoring --test nx_import`

Expected: FAIL because `parse_score_ini_text` and `render_with_history` do not exist.

- [ ] **Step 3: Add parsed NX structs and parser API**

Modify `crates/dtx-scoring/src/score_ini.rs` after `DrumScoreIni`:

```rust
/// Parsed `[File]` metadata from DTXManiaNX `.score.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreIniFileSection {
    /// Title field.
    pub title: String,
    /// Name field.
    pub name: String,
    /// Hash field when present.
    pub hash: String,
    /// Drums play count.
    pub play_count_drums: u32,
    /// Drums clear count.
    pub clear_count_drums: u32,
    /// Best rank code for drums.
    pub best_rank_drums: i32,
    /// Number of history entries reported by NX.
    pub history_count: u32,
    /// History0..History4 values.
    pub history: Vec<String>,
    /// BGM adjust.
    pub bgm_adjust: i32,
}

/// Parsed drums-focused `.score.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedScoreIni {
    /// File section.
    pub file: ScoreIniFileSection,
    /// HiScore.Drums section.
    pub hi_score_drums: Option<DrumScoreIni>,
    /// LastPlay.Drums section.
    pub last_play_drums: Option<DrumScoreIni>,
}

/// Parse drums-focused DTXManiaNX score.ini text.
pub fn parse_score_ini_text(text: &str) -> Option<ParsedScoreIni> {
    let sections = parse_sections(text);
    let file = parse_file_section(sections.get("File"));
    let hi_score_drums = sections
        .get("HiScore.Drums")
        .map(|section| parse_drum_section(section, &file));
    let last_play_drums = sections
        .get("LastPlay.Drums")
        .map(|section| parse_drum_section(section, &file));
    Some(ParsedScoreIni {
        file,
        hi_score_drums,
        last_play_drums,
    })
}

fn parse_file_section(section: Option<&HashMap<String, String>>) -> ScoreIniFileSection {
    let Some(section) = section else {
        return ScoreIniFileSection::default();
    };
    let history_count = get_u32(section, "HistoryCount");
    let mut history = Vec::new();
    for idx in 0..5 {
        let key = format!("History{idx}");
        if let Some(value) = section.get(&key) {
            if !value.is_empty() {
                history.push(value.clone());
            }
        }
    }
    ScoreIniFileSection {
        title: section.get("Title").cloned().unwrap_or_default(),
        name: section.get("Name").cloned().unwrap_or_default(),
        hash: section.get("Hash").cloned().unwrap_or_default(),
        play_count_drums: get_u32(section, "PlayCountDrums"),
        clear_count_drums: get_u32(section, "ClearCountDrums"),
        best_rank_drums: get_i32(section, "BestRankDrums", 99),
        history_count,
        history,
        bgm_adjust: get_i32(section, "BGMAdjust", 0),
    }
}

fn parse_drum_section(
    drums: &HashMap<String, String>,
    file: &ScoreIniFileSection,
) -> DrumScoreIni {
    DrumScoreIni {
        score: get_u32(drums, "Score"),
        perfect: get_u32(drums, "Perfect"),
        great: get_u32(drums, "Great"),
        good: get_u32(drums, "Good"),
        poor: get_u32(drums, "Poor"),
        miss: get_u32(drums, "Miss"),
        max_combo: get_u32(drums, "MaxCombo"),
        total_chips: get_u32(drums, "TotalChips"),
        rank: rank_name(file.best_rank_drums).to_string(),
        play_count: file.play_count_drums,
        clear_count: file.clear_count_drums,
        bgm_adjust: file.bgm_adjust,
        date_time: drums.get("DateTime").cloned().unwrap_or_default(),
    }
}
```

- [ ] **Step 4: Make existing `read_best` use the parsed API**

Replace `parse_best` body with:

```rust
fn parse_best(text: &str) -> Option<DrumScoreIni> {
    parse_score_ini_text(text)?.hi_score_drums
}
```

- [ ] **Step 5: Add history-aware renderer**

Add this public helper near `render`:

```rust
/// Render `[File]` + drums sections with preserved history lines.
pub fn render_with_history(best: &DrumScoreIni, last: &DrumScoreIni, history: &[String]) -> String {
    render_internal(best, last, history)
}
```

Replace `fn render(best: &DrumScoreIni, last: &DrumScoreIni) -> String` with:

```rust
fn render(best: &DrumScoreIni, last: &DrumScoreIni) -> String {
    render_internal(best, last, &[])
}

fn render_internal(best: &DrumScoreIni, last: &DrumScoreIni, history: &[String]) -> String {
    let rank = rank_code(&best.rank);
    let mut text = format!(
        "[File]\nTitle=\nName=\nPlayCountDrums={play}\nPlayCountGuitars=0\nPlayCountBass=0\nClearCountDrums={clear}\nClearCountGuitars=0\nClearCountBass=0\nBestRankDrums={rank}\nBestRankGuitar=99\nBestRankBass=99\nHistoryCount={history_count}\n",
        play = best.play_count,
        clear = best.clear_count,
        rank = rank,
        history_count = history.len().min(5),
    );
    for idx in 0..5 {
        let value = history.get(idx).map(String::as_str).unwrap_or("");
        text.push_str(&format!("History{idx}={value}\n"));
    }
    text.push_str(&format!("BGMAdjust={}\n\n", best.bgm_adjust));
    render_section(&mut text, "HiScore.Drums", best);
    render_section(&mut text, "HiSkill.Drums", best);
    render_section(&mut text, "LastPlay.Drums", last);
    text
}
```

- [ ] **Step 6: Run NX parser tests**

Run: `cargo test -p dtx-scoring --test nx_import`

Expected: PASS.

- [ ] **Step 7: Run all score_ini tests**

Run: `cargo test -p dtx-scoring score_ini`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/dtx-scoring/src/score_ini.rs crates/dtx-scoring/tests/nx_import.rs
git commit -m "feat(scoring): parse nx score history"
```

## Task 4: NX Import API

**Files:**
- Create: `crates/dtx-scoring/src/nx_import.rs`
- Modify: `crates/dtx-scoring/src/lib.rs`
- Modify: `crates/dtx-scoring/tests/nx_import.rs`

- [ ] **Step 1: Add failing import API tests**

Append to `crates/dtx-scoring/tests/nx_import.rs`:

```rust
use dtx_scoring::nx_import::{import_nx_scores, ImportOptions};
use dtx_scoring::store::{ScoreSource, ScoreStore};

#[test]
fn import_nx_scores_adds_best_and_last_play_once() {
    let root = std::env::temp_dir().join(format!("dtx_nx_import_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let chart = root.join("song.dtx");
    let ini = root.join("song.dtx.score.ini");

    std::fs::write(
        &chart,
        r#"
#TITLE: Import Song
#ARTIST: Import Artist
#BPM: 120
#00111: 0100
"#,
    )
    .unwrap();
    std::fs::write(&ini, sample_score_ini()).unwrap();

    let mut store = ScoreStore::default();
    let report = import_nx_scores(&mut store, ImportOptions { root: root.clone() }).unwrap();
    let report2 = import_nx_scores(&mut store, ImportOptions { root: root.clone() }).unwrap();

    assert_eq!(report.imported_entries, 2);
    assert_eq!(report2.imported_entries, 0);
    assert_eq!(store.entries.len(), 2);
    assert!(store.entries.iter().any(|e| e.source == ScoreSource::ImportedNxHiScore));
    assert!(store.entries.iter().any(|e| e.source == ScoreSource::ImportedNxLastPlay));
    assert_eq!(store.nx_imports.len(), 1);
    assert_eq!(store.nx_imports[0].history.len(), 2);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_nx_scores_reports_missing_chart_without_crashing() {
    let root = std::env::temp_dir().join(format!("dtx_nx_missing_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("missing.dtx.score.ini"), sample_score_ini()).unwrap();

    let mut store = ScoreStore::default();
    let report = import_nx_scores(&mut store, ImportOptions { root: root.clone() }).unwrap();

    assert_eq!(report.missing_charts, 1);
    assert_eq!(report.imported_entries, 0);
    assert_eq!(store.entries.len(), 0);

    let _ = std::fs::remove_dir_all(root);
}
```

- [ ] **Step 2: Run import API tests to verify they fail**

Run: `cargo test -p dtx-scoring --test nx_import import_nx_scores`

Expected: FAIL because `nx_import` does not exist.

- [ ] **Step 3: Implement import API**

Create `crates/dtx-scoring/src/nx_import.rs`:

```rust
//! DTXManiaNX `.score.ini` import.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use crate::score_ini::{parse_score_ini_text, ParsedScoreIni};
use crate::store::{JudgmentTotals, NxImportRecord, ScoreEntry, ScoreSource, ScoreStore};
use crate::Rank;

/// Import options.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Directory to scan recursively.
    pub root: PathBuf,
}

/// Import summary.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// Found score.ini files.
    pub scanned_score_inis: u32,
    /// Imported score entries.
    pub imported_entries: u32,
    /// Files skipped due to malformed content.
    pub skipped_malformed: u32,
    /// `.score.ini` files with no paired `.dtx`.
    pub missing_charts: u32,
}

/// Import errors.
#[derive(Debug, Error)]
pub enum ImportError {
    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Chart parse error.
    #[error("chart parse error: {0}")]
    Parse(#[from] dtx_core::DtxError),
}

/// Import DTXManiaNX score files into a store.
pub fn import_nx_scores(
    store: &mut ScoreStore,
    options: ImportOptions,
) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let mut files = Vec::new();
    collect_score_ini_files(&options.root, &mut files)?;
    files.sort();

    for score_ini_path in files {
        report.scanned_score_inis += 1;
        let chart_path = chart_path_for_score_ini(&score_ini_path);
        if !chart_path.exists() {
            report.missing_charts += 1;
            continue;
        }

        let text = std::fs::read_to_string(&score_ini_path)?;
        let Some(parsed) = parse_score_ini_text(&text) else {
            report.skipped_malformed += 1;
            continue;
        };

        let file = File::open(&chart_path)?;
        let chart = dtx_core::parse(BufReader::new(file))?;
        let raw = raw_file_sha256(&chart_path).ok();
        let identity = ChartIdentity::new(
            canonical_chart_hash(&chart),
            raw,
            Some(chart_path.clone()),
        );

        let title = chart.metadata.title.clone().unwrap_or_else(|| parsed.file.title.clone());
        let artist = chart.metadata.artist.clone().unwrap_or_default();

        let before = store.entries.len();
        if let Some(best) = parsed.hi_score_drums.clone() {
            store.add_if_new(entry_from_ini(
                &identity,
                &title,
                &artist,
                &best,
                ScoreSource::ImportedNxHiScore,
            ));
        }
        if let Some(last) = parsed.last_play_drums.clone() {
            store.add_if_new(entry_from_ini(
                &identity,
                &title,
                &artist,
                &last,
                ScoreSource::ImportedNxLastPlay,
            ));
        }
        report.imported_entries += (store.entries.len() - before) as u32;

        if !store.nx_imports.iter().any(|record| {
            record.chart.canonical_hash == identity.canonical_hash
                && record.score_ini_path == score_ini_path
        }) {
            store.nx_imports.push(NxImportRecord {
                chart: identity,
                score_ini_path,
                play_count: parsed.file.play_count_drums,
                clear_count: parsed.file.clear_count_drums,
                bgm_adjust: parsed.file.bgm_adjust,
                history: parsed.file.history,
            });
        }
    }

    Ok(report)
}

fn collect_score_ini_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_score_ini_files(&path, out)?;
        } else if path.to_string_lossy().ends_with(".dtx.score.ini") {
            out.push(path);
        }
    }
    Ok(())
}

fn chart_path_for_score_ini(score_ini_path: &Path) -> PathBuf {
    let text = score_ini_path.to_string_lossy();
    let chart = text.strip_suffix(".score.ini").unwrap_or(&text);
    PathBuf::from(chart)
}

fn entry_from_ini(
    identity: &ChartIdentity,
    title: &str,
    artist: &str,
    ini: &crate::score_ini::DrumScoreIni,
    source: ScoreSource,
) -> ScoreEntry {
    ScoreEntry {
        id: format!(
            "{}:{source:?}:{}:{}",
            identity.canonical_hash, ini.score, ini.date_time
        ),
        chart: identity.clone(),
        title: title.to_string(),
        artist: artist.to_string(),
        score: ini.score,
        max_combo: ini.max_combo,
        judgments: JudgmentTotals {
            perfect: ini.perfect,
            great: ini.great,
            good: ini.good,
            poor: ini.poor,
            miss: ini.miss,
        },
        rank: rank_from_ini(&ini.rank),
        played_at: 0,
        source,
        replay_ref: None,
    }
}

fn rank_from_ini(rank: &str) -> Rank {
    match rank {
        "SS" => Rank::SS,
        "S" => Rank::S,
        "A" => Rank::A,
        "B" => Rank::B,
        "C" => Rank::C,
        "D" => Rank::D,
        "E" => Rank::E,
        _ => Rank::Unknown,
    }
}
```

- [ ] **Step 4: Export nx_import module**

Modify `crates/dtx-scoring/src/lib.rs` module list:

```rust
pub mod identity;
pub mod nx_import;
pub mod replay;
pub mod score_ini;
pub mod store;
```

- [ ] **Step 5: Run import API tests**

Run: `cargo test -p dtx-scoring --test nx_import import_nx_scores`

Expected: PASS.

- [ ] **Step 6: Run all dtx-scoring tests**

Run: `cargo test -p dtx-scoring`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/dtx-scoring/src/lib.rs crates/dtx-scoring/src/nx_import.rs crates/dtx-scoring/tests/nx_import.rs
git commit -m "feat(scoring): import dtxmanianx scores"
```

## Task 5: CLI Import Command

**Files:**
- Modify: `tools/dtx-cli/src/main.rs`
- Test: manual CLI command with fixture directory

- [ ] **Step 1: Add failing CLI test by running intended command**

Run: `cargo run -p dtx-cli -- scores import-nx /tmp/nonexistent-dtxmaniars-scores`

Expected: FAIL with clap error because `scores` subcommand does not exist.

- [ ] **Step 2: Add `Scores` command enum**

Modify `tools/dtx-cli/src/main.rs` command definitions:

```rust
#[derive(Subcommand, Debug)]
enum Cmd {
    /// Parse a .dtx file and report metadata + chip count.
    Validate {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Print chips grouped by channel (debug aid).
    Inspect {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Play a chart end-to-end and report final score+combo+gauge.
    PlayChart {
        /// Path to the .dtx file.
        path: PathBuf,
    },
    /// Score store utilities.
    Scores {
        /// Score utility command.
        #[command(subcommand)]
        cmd: ScoresCmd,
    },
}

#[derive(Subcommand, Debug)]
enum ScoresCmd {
    /// Import DTXManiaNX .dtx.score.ini files from a song tree.
    ImportNx {
        /// Root song directory to scan.
        songs_dir: PathBuf,
    },
}
```

- [ ] **Step 3: Wire command dispatch**

Modify `run` in `tools/dtx-cli/src/main.rs`:

```rust
fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Validate { path } => validate(&path),
        Cmd::Inspect { path } => inspect(&path),
        Cmd::PlayChart { path } => play_chart(&path),
        Cmd::Scores { cmd } => match cmd {
            ScoresCmd::ImportNx { songs_dir } => import_nx_scores_cli(&songs_dir),
        },
    }
}
```

Add this function near `play_chart`:

```rust
fn import_nx_scores_cli(songs_dir: &PathBuf) -> Result<()> {
    use dtx_scoring::nx_import::{import_nx_scores, ImportOptions};
    use dtx_scoring::ScoreStore;

    let mut store = ScoreStore::with_path(ScoreStore::default_path());
    store.load().context("loading score store")?;
    let report = import_nx_scores(
        &mut store,
        ImportOptions {
            root: songs_dir.clone(),
        },
    )
    .context("importing DTXManiaNX scores")?;
    store.save().context("saving score store")?;

    println!("scanned score.ini files: {}", report.scanned_score_inis);
    println!("imported entries: {}", report.imported_entries);
    println!("missing paired charts: {}", report.missing_charts);
    println!("malformed score.ini files: {}", report.skipped_malformed);
    Ok(())
}
```

- [ ] **Step 4: Run CLI help**

Run: `cargo run -p dtx-cli -- scores --help`

Expected: PASS and output includes `import-nx`.

- [ ] **Step 5: Run CLI import against missing directory**

Run: `cargo run -p dtx-cli -- scores import-nx /tmp/nonexistent-dtxmaniars-scores`

Expected: FAIL with context containing `importing DTXManiaNX scores`.

- [ ] **Step 6: Run CLI import against a fixture directory**

Create a temporary fixture:

```bash
tmp="$(mktemp -d)"
cat > "$tmp/song.dtx" <<'EOF'
#TITLE: CLI Import
#ARTIST: Tester
#BPM: 120
#00111: 0100
EOF
cat > "$tmp/song.dtx.score.ini" <<'EOF'
[File]
PlayCountDrums=1
ClearCountDrums=1
BestRankDrums=1
HistoryCount=1
History0=1.26/7/8 Stage cleared
BGMAdjust=0

[HiScore.Drums]
Score=123456
Perfect=1
Great=0
Good=0
Poor=0
Miss=0
MaxCombo=1
TotalChips=1
DateTime=2026/7/8 1:02:03
EOF
DTX_SCORES_PATH="$tmp/scores.json" cargo run -p dtx-cli -- scores import-nx "$tmp"
```

Expected: PASS and output includes `imported entries: 1`.

- [ ] **Step 7: Commit**

```bash
git add tools/dtx-cli/src/main.rs
git commit -m "feat(cli): import dtxmanianx score files"
```

## Task 6: Wire Game Results to ScoreStore v2

**Files:**
- Modify: `crates/game-results/src/lib.rs`
- Test: `cargo test -p game-results -p dtx-scoring`

- [ ] **Step 1: Update imports**

Modify the import line in `crates/game-results/src/lib.rs`:

```rust
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use dtx_scoring::{
    JudgmentTotals, Rank, ScoreEntry, ScoreSource, ScoreStore,
};
```

- [ ] **Step 2: Build chart identity in result save**

Replace the current raw-only `chart_hash` block in `save_result_then_despawn` with:

```rust
let chart_identity = chart
    .source_path
    .as_ref()
    .map(|p| {
        let raw = raw_file_sha256(p).ok();
        ChartIdentity::new(
            canonical_chart_hash(&chart.chart),
            raw,
            Some(p.clone()),
        )
    })
    .unwrap_or_else(|| ChartIdentity::new(canonical_chart_hash(&chart.chart), None, None));
```

- [ ] **Step 3: Build v2 score entry**

Replace the current `ScoreEntry` construction with:

```rust
let entry = ScoreEntry {
    id: format!(
        "{}:native:{}:{}",
        chart_identity.canonical_hash,
        score.0,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ),
    chart: chart_identity,
    title,
    artist,
    score: score.0 as u32,
    max_combo: combo.max,
    judgments: JudgmentTotals {
        perfect: counts.perfect,
        great: counts.great,
        good: counts.good,
        poor: counts.ok,
        miss: counts.miss,
    },
    rank,
    played_at: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0),
    source: ScoreSource::Native,
    replay_ref: None,
};
```

- [ ] **Step 4: Keep NX `.score.ini` export unchanged**

Leave the existing block that writes `dtx_scoring::score_ini::write_result` in place. Confirm it still maps:

```rust
poor: counts.ok,
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p game-results -p dtx-scoring`

Expected: PASS.

- [ ] **Step 6: Run workspace check**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/game-results/src/lib.rs
git commit -m "feat(results): save score entries with chart identity"
```

## Task 7: Final Verification and Spec Cross-Check

**Files:**
- Verify all changed files.
- No new production files beyond those listed in this plan.

- [ ] **Step 1: Run scoring tests**

Run: `cargo test -p dtx-scoring`

Expected: PASS.

- [ ] **Step 2: Run CLI build and help**

Run: `cargo run -p dtx-cli -- scores --help`

Expected: PASS and output includes `import-nx`.

- [ ] **Step 3: Run results tests/check**

Run: `cargo test -p game-results`

Expected: PASS.

- [ ] **Step 4: Run workspace check**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 5: Run formatting**

Run: `cargo fmt --all -- --check`

Expected: PASS. If it fails, run `cargo fmt --all`, inspect the diff, then repeat `cargo fmt --all -- --check`.

- [ ] **Step 6: Verify spec coverage**

Read `docs/superpowers/specs/2026-07-08-score-identity-nx-import-design.md` and confirm the diff implements:

- ScoreStore v2 schema.
- Canonical chart hash.
- Raw hash compatibility.
- Legacy `scores.json` migration.
- NX history and last play parsing.
- NX CLI import.
- Existing `.score.ini` export path preserved.
- `SectionId`.
- `ReplayHeader` and `ReplayRef`.

- [ ] **Step 7: Final commit if formatting changed files**

If Step 5 produced a formatting-only diff, commit it:

```bash
git add crates/dtx-scoring crates/game-results tools/dtx-cli
git commit -m "style: format score identity changes"
```

If Step 5 produced no diff, skip this commit.
