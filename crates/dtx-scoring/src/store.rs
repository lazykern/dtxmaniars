//! Versioned score store and legacy migration.

use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub score: i64,
    /// Chart level used to calculate `song_skill`.
    #[serde(default)]
    pub chart_level: f64,
    /// NX performance skill / completion rate (0..100).
    #[serde(default)]
    pub performance_skill: f64,
    /// NX per-song skill contribution.
    #[serde(default)]
    pub song_skill: f64,
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
    /// Total judgment count.
    pub fn total(&self) -> u32 {
        self.judgments.total()
    }

    /// Perfect percentage (0..100). Returns 0 when there are no judgments.
    pub fn perfect_pct(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.judgments.perfect as f32 / total as f32 * 100.0
        }
    }

    /// Weighted achievement percentage used in the player-facing UI.
    pub fn achievement_pct(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            let judgments = &self.judgments;
            let weighted = judgments.perfect as f32 * 100.0
                + judgments.great as f32 * 80.0
                + judgments.good as f32 * 60.0
                + judgments.poor as f32 * 40.0;
            weighted / total as f32
        }
    }

    /// NX performance skill, derived from recorded judgments for legacy entries.
    pub fn effective_performance_skill(&self) -> f64 {
        if self.performance_skill != 0.0 {
            self.performance_skill
        } else {
            crate::skill::drum_performance_skill(
                self.total(),
                self.judgments.perfect,
                self.judgments.great,
                self.judgments.good,
                self.judgments.poor,
                self.judgments.miss,
                self.max_combo,
                crate::skill::DrumAutoPlay::default(),
            )
        }
    }

    /// NX per-song skill, derived when a legacy entry carries its chart level.
    pub fn effective_song_skill(&self) -> f64 {
        if self.song_skill != 0.0 {
            self.song_skill
        } else {
            crate::skill::drum_song_skill(
                self.chart_level,
                self.effective_performance_skill(),
                false,
            )
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
    /// Imported DTXManiaNX high-skill record.
    ImportedNxHiSkill,
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
        if !self
            .entries
            .iter()
            .any(|existing| equivalent_entry(existing, &entry))
        {
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

    /// Highest-performance result for a canonical chart hash.
    pub fn best_skill_for_chart(&self, canonical_hash: &str) -> Option<&ScoreEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.chart.canonical_hash == canonical_hash)
            .max_by(|a, b| {
                a.effective_performance_skill()
                    .total_cmp(&b.effective_performance_skill())
            })
    }

    /// NX player skill: highest chart contribution per song folder, then top 50.
    ///
    /// DTXManiaNX's `SongNode` holds every difficulty for one song. Our song
    /// selector uses the chart's parent directory as the corresponding node;
    /// legacy entries without a source path remain independent.
    pub fn player_skill(&self) -> f64 {
        let mut by_song = std::collections::HashMap::<String, f64>::new();
        for entry in &self.entries {
            let skill = entry.effective_song_skill();
            let song_key = entry
                .chart
                .source_path_hint
                .as_deref()
                .and_then(Path::parent)
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_else(|| entry.chart.canonical_hash.clone());
            by_song
                .entry(song_key)
                .and_modify(|best| *best = best.max(skill))
                .or_insert(skill);
        }
        let mut skills: Vec<f64> = by_song.into_values().filter(|skill| *skill > 0.0).collect();
        skills.sort_by(|a, b| b.total_cmp(a));
        skills.into_iter().take(50).sum()
    }

    /// Backward-compatible alias for best score lookup.
    pub fn best_for(&self, canonical_hash: &str) -> Option<&ScoreEntry> {
        self.best_for_chart(canonical_hash)
    }

    /// Plays whose `source_path_hint` matches `path`, best score first
    /// (ties: most recent first), truncated to `limit`.
    pub fn history_for_path(&self, path: &Path, limit: usize) -> Vec<&ScoreEntry> {
        let mut hits: Vec<&ScoreEntry> = self
            .entries
            .iter()
            .filter(|e| e.chart.source_path_hint.as_deref() == Some(path))
            .collect();
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(b.played_at.cmp(&a.played_at)));
        hits.truncate(limit);
        hits
    }

    /// Number of distinct canonical chart hashes in the store.
    pub fn chart_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for entry in &self.entries {
            seen.insert(entry.chart.canonical_hash.clone());
        }
        seen.len()
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
        let entries = self
            .entries
            .into_iter()
            .map(LegacyScoreEntry::into_v2)
            .collect();
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
            score: i64::from(self.score),
            chart_level: 0.0,
            performance_skill: 0.0,
            song_skill: 0.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ChartIdentity;

    #[test]
    fn player_skill_sums_the_top_fifty_distinct_charts() {
        let mut store = ScoreStore::default();
        for value in 1..=51_i64 {
            store.add(ScoreEntry {
                id: value.to_string(),
                chart: ChartIdentity::new(format!("dtx1:{value}"), None, None),
                title: String::new(),
                artist: String::new(),
                score: value,
                chart_level: 0.0,
                performance_skill: 0.0,
                song_skill: value as f64,
                max_combo: 0,
                judgments: JudgmentTotals::default(),
                rank: Rank::Unknown,
                played_at: 0,
                source: ScoreSource::Native,
                replay_ref: None,
            });
        }
        assert_eq!(store.player_skill(), 1325.0);
    }

    #[test]
    fn player_skill_counts_one_best_chart_per_song_folder() {
        let mut store = ScoreStore::default();
        for (name, folder, skill) in [
            ("basic", "/songs/a", 10.0),
            ("master", "/songs/a", 20.0),
            ("other", "/songs/b", 30.0),
        ] {
            store.add(ScoreEntry {
                id: name.to_string(),
                chart: ChartIdentity::new(
                    format!("dtx1:{name}"),
                    None,
                    Some(PathBuf::from(folder).join(format!("{name}.dtx"))),
                ),
                title: String::new(),
                artist: String::new(),
                score: 0,
                chart_level: 0.0,
                performance_skill: 0.0,
                song_skill: skill,
                max_combo: 0,
                judgments: JudgmentTotals::default(),
                rank: Rank::Unknown,
                played_at: 0,
                source: ScoreSource::Native,
                replay_ref: None,
            });
        }
        assert_eq!(store.player_skill(), 50.0);
    }
}
