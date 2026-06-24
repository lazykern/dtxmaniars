//! Pure scoring rules + CScoreIni-style persistence.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs` (1773 lines, ported minimally)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CPerformanceEntry.cs`
//!
//! M6a: persist per-chart results to a JSON file. We do NOT port the full
//! 1773-line CScoreIni (config .ini parsing, 9-section STSection, etc.) —
//! just what game-results needs: append history + best-score lookup.
//!
//! No bevy. Bevy Resource wrapping lives in the consumer (game-results).

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Judgment kind for a single hit. Maps to DTXmaniaNX timing windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JudgmentKind {
    /// Tightest window (default ±16ms).
    Perfect,
    /// Second tier (default ±32ms).
    Great,
    /// Third tier (default ±64ms).
    Good,
    /// Fourth tier (default ±100ms).
    Ok,
    /// Out of window or no input.
    Miss,
}

/// Default DTXmaniaNX timing windows in milliseconds.
pub const DEFAULT_WINDOWS_MS: &[(JudgmentKind, i32)] = &[
    (JudgmentKind::Perfect, 16),
    (JudgmentKind::Great, 32),
    (JudgmentKind::Good, 64),
    (JudgmentKind::Ok, 100),
    (JudgmentKind::Miss, 200),
];

/// Classify a delta (ms) into a judgment.
pub fn classify(delta_ms: i32) -> JudgmentKind {
    let abs = delta_ms.unsigned_abs();
    for (kind, window) in DEFAULT_WINDOWS_MS {
        if abs <= *window as u32 {
            return *kind;
        }
    }
    JudgmentKind::Miss
}

/// Rank letter computed from perfect percentage (0..100).
///
/// Thresholds match DTXManiaNX ConfigIni defaults:
/// S ≥ 95, A ≥ 85, B ≥ 70, C ≥ 50, D ≥ 25, else E.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rank {
    /// Best.
    S,
    /// Excellent.
    A,
    /// Good.
    B,
    /// Average.
    C,
    /// Below average.
    D,
    /// Lowest.
    E,
}

impl Rank {
    /// Compute rank from perfect percentage.
    pub fn from_perfect_pct(pct: f32) -> Self {
        match pct {
            p if p >= 95.0 => Rank::S,
            p if p >= 85.0 => Rank::A,
            p if p >= 70.0 => Rank::B,
            p if p >= 50.0 => Rank::C,
            p if p >= 25.0 => Rank::D,
            _ => Rank::E,
        }
    }
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Rank::S => "S",
            Rank::A => "A",
            Rank::B => "B",
            Rank::C => "C",
            Rank::D => "D",
            Rank::E => "E",
        };
        write!(f, "{s}")
    }
}

/// One persisted result for one play of one chart.
///
/// Mirrors a subset of `CPerformanceEntry` (CScoreIni.cs:177-256): the fields
/// needed to show "best score for this chart" in SongSelect + to reconstruct
/// a Result screen for replay history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoreEntry {
    /// SHA-256 (hex) of the DTX file contents, or path-derived fallback.
    pub chart_hash: String,
    /// Song title (from DTX `#TITLE`).
    pub title: String,
    /// Song artist (from DTX `#ARTIST`).
    pub artist: String,
    /// Cumulative score (from gameplay-drums Score resource).
    pub score: u32,
    /// Maximum combo achieved.
    pub max_combo: u32,
    /// Per-judgment counts.
    pub perfect: u32,
    /// Per-judgment counts.
    pub great: u32,
    /// Per-judgment counts.
    pub good: u32,
    /// Per-judgment counts.
    pub ok: u32,
    /// Per-judgment counts.
    pub miss: u32,
    /// Computed rank letter.
    pub rank: Rank,
    /// Unix seconds when the result was recorded.
    pub played_at: u64,
}

impl ScoreEntry {
    /// Total judgment count (sum of all 5 kinds).
    pub fn total(&self) -> u32 {
        self.perfect + self.great + self.good + self.ok + self.miss
    }

    /// Perfect percentage (0..100). 0 if total == 0.
    pub fn perfect_pct(&self) -> f32 {
        let t = self.total();
        if t == 0 {
            0.0
        } else {
            self.perfect as f32 / t as f32 * 100.0
        }
    }
}

/// Errors from loading or saving the score store.
#[derive(Debug, Error)]
pub enum ScoreStoreError {
    /// I/O error (read/write).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Persistent score history. One JSON file, all charts.
///
/// Reference: CScoreIni.cs manages 9 sections per chart (HiScore Drums/Guitar/Bass
/// × Score/Skill + LastPlay). M6a flattens to one Vec<ScoreEntry>; full 9-section
/// split is M6.1+ if needed.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ScoreStore {
    /// All recorded results, newest last.
    pub entries: Vec<ScoreEntry>,
    /// Where to load/save on disk. None = ephemeral (in-memory only).
    pub path: Option<PathBuf>,
}

impl ScoreStore {
    /// Construct a ScoreStore backed by `path`. File may not exist yet.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            entries: Vec::new(),
            path: Some(path),
        }
    }

    /// Default location: `<cwd>/scores.json`. Caller may override via env
    /// (`DTX_SCORES_PATH`) or `with_path`.
    pub fn default_path() -> PathBuf {
        if let Ok(p) = std::env::var("DTX_SCORES_PATH") {
            return PathBuf::from(p);
        }
        PathBuf::from("scores.json")
    }

    /// Load entries from `self.path`. Missing file → empty (not an error).
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
        let parsed: ScoreStore = serde_json::from_slice(&bytes)?;
        self.entries = parsed.entries;
        Ok(())
    }

    /// Save entries to `self.path`. Creates parent dirs. No-op if path is None.
    pub fn save(&self) -> Result<(), ScoreStoreError> {
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

    /// Append an entry (does NOT auto-save).
    pub fn add(&mut self, entry: ScoreEntry) {
        self.entries.push(entry);
    }

    /// Find the highest-score entry for a chart hash, if any.
    pub fn best_for(&self, chart_hash: &str) -> Option<&ScoreEntry> {
        self.entries
            .iter()
            .filter(|e| e.chart_hash == chart_hash)
            .max_by_key(|e| e.score)
    }

    /// Number of distinct charts in the store.
    pub fn chart_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for e in &self.entries {
            seen.insert(e.chart_hash.clone());
        }
        seen.len()
    }

    /// Total entries (history depth).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Compute a deterministic chart hash from file contents.
///
/// SHA-256 (hex) of the file bytes. If the file cannot be read (e.g. missing
/// during tests), falls back to hashing the path string so behavior stays
/// deterministic per path. Real DTX files always succeed in practice.
pub fn compute_chart_hash(path: &Path) -> String {
    let mut hasher = Sha256::new();
    match std::fs::read(path) {
        Ok(bytes) => hasher.update(&bytes),
        Err(_) => hasher.update(path.to_string_lossy().as_bytes()),
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_window() {
        assert_eq!(classify(0), JudgmentKind::Perfect);
        assert_eq!(classify(15), JudgmentKind::Perfect);
        assert_eq!(classify(-16), JudgmentKind::Perfect);
    }

    #[test]
    fn miss_outside_windows() {
        assert_eq!(classify(201), JudgmentKind::Miss);
        assert_eq!(classify(-500), JudgmentKind::Miss);
    }

    #[test]
    fn rank_s_at_95_plus() {
        assert_eq!(Rank::from_perfect_pct(100.0), Rank::S);
        assert_eq!(Rank::from_perfect_pct(95.0), Rank::S);
    }

    #[test]
    fn rank_a_85_to_95() {
        assert_eq!(Rank::from_perfect_pct(94.9), Rank::A);
        assert_eq!(Rank::from_perfect_pct(85.0), Rank::A);
    }

    #[test]
    fn rank_b_70_to_85() {
        assert_eq!(Rank::from_perfect_pct(84.9), Rank::B);
        assert_eq!(Rank::from_perfect_pct(70.0), Rank::B);
    }

    #[test]
    fn rank_c_50_to_70() {
        assert_eq!(Rank::from_perfect_pct(69.9), Rank::C);
        assert_eq!(Rank::from_perfect_pct(50.0), Rank::C);
    }

    #[test]
    fn rank_d_25_to_50() {
        assert_eq!(Rank::from_perfect_pct(49.9), Rank::D);
        assert_eq!(Rank::from_perfect_pct(25.0), Rank::D);
    }

    #[test]
    fn rank_e_below_25() {
        assert_eq!(Rank::from_perfect_pct(24.9), Rank::E);
        assert_eq!(Rank::from_perfect_pct(0.0), Rank::E);
    }

    fn fake_entry(score: u32, chart_hash: &str) -> ScoreEntry {
        ScoreEntry {
            chart_hash: chart_hash.to_string(),
            title: "T".into(),
            artist: "A".into(),
            score,
            max_combo: 100,
            perfect: 50,
            great: 5,
            good: 3,
            ok: 1,
            miss: 0,
            rank: Rank::S,
            played_at: 1700000000,
        }
    }

    #[test]
    fn score_entry_total_sums_all_judgments() {
        let e = fake_entry(1000, "h1");
        assert_eq!(e.total(), 59);
    }

    #[test]
    fn score_entry_perfect_pct_zero_total() {
        let e = ScoreEntry {
            chart_hash: "h".into(),
            title: "".into(),
            artist: "".into(),
            score: 0,
            max_combo: 0,
            perfect: 0,
            great: 0,
            good: 0,
            ok: 0,
            miss: 0,
            rank: Rank::E,
            played_at: 0,
        };
        assert_eq!(e.perfect_pct(), 0.0);
    }

    #[test]
    fn score_store_default_path_uses_env_or_cwd() {
        // Either DTX_SCORES_PATH override or "scores.json" — both should be non-empty.
        let p = ScoreStore::default_path();
        assert!(!p.as_os_str().is_empty());
    }

    #[test]
    fn score_store_empty_when_load_missing_file() {
        let tmp = tempdir_path("missing.json");
        let mut s = ScoreStore::with_path(tmp.clone());
        s.load().unwrap();
        assert!(s.is_empty());
        // cleanup
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn score_store_save_load_roundtrip() {
        let tmp = tempdir_path("roundtrip.json");
        let mut s = ScoreStore::with_path(tmp.clone());
        s.add(fake_entry(1000, "abc"));
        s.add(fake_entry(2000, "abc"));
        s.add(fake_entry(500, "xyz"));
        s.save().unwrap();

        let mut loaded = ScoreStore::with_path(tmp.clone());
        loaded.load().unwrap();
        assert_eq!(loaded.entries.len(), 3);
        assert_eq!(loaded.entries[0].score, 1000);
        assert_eq!(loaded.entries[2].score, 500);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn score_store_best_for_returns_highest_score() {
        let mut s = ScoreStore::default();
        s.add(fake_entry(1000, "abc"));
        s.add(fake_entry(3000, "abc"));
        s.add(fake_entry(2000, "abc"));
        s.add(fake_entry(9999, "xyz"));
        let best = s.best_for("abc").unwrap();
        assert_eq!(best.score, 3000);
        assert!(s.best_for("nonexistent").is_none());
    }

    #[test]
    fn score_store_chart_count_counts_distinct_hashes() {
        let mut s = ScoreStore::default();
        s.add(fake_entry(100, "a"));
        s.add(fake_entry(200, "a"));
        s.add(fake_entry(300, "b"));
        s.add(fake_entry(400, "c"));
        assert_eq!(s.chart_count(), 3);
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn chart_hash_deterministic_for_same_content() {
        let tmp1 = tempdir_path("hash_a.txt");
        let tmp2 = tempdir_path("hash_b.txt");
        std::fs::write(&tmp1, b"hello world").unwrap();
        std::fs::write(&tmp2, b"hello world").unwrap();
        assert_eq!(compute_chart_hash(&tmp1), compute_chart_hash(&tmp2));
        let _ = std::fs::remove_file(&tmp1);
        let _ = std::fs::remove_file(&tmp2);
    }

    #[test]
    fn chart_hash_differs_for_different_content() {
        let tmp1 = tempdir_path("hash_x.txt");
        let tmp2 = tempdir_path("hash_y.txt");
        std::fs::write(&tmp1, b"alpha").unwrap();
        std::fs::write(&tmp2, b"beta").unwrap();
        assert_ne!(compute_chart_hash(&tmp1), compute_chart_hash(&tmp2));
        let _ = std::fs::remove_file(&tmp1);
        let _ = std::fs::remove_file(&tmp2);
    }

    #[test]
    fn chart_hash_falls_back_to_path_when_unreadable() {
        let p = PathBuf::from("/nonexistent/never/read.txt");
        let h = compute_chart_hash(&p);
        assert_eq!(h.len(), 64); // SHA-256 hex
                                 // Same path → same hash (deterministic).
        assert_eq!(h, compute_chart_hash(&p));
    }

    #[test]
    fn score_entry_serde_round_trip() {
        let entry = ScoreEntry {
            chart_hash: "abc123".into(),
            title: "Round Trip".into(),
            artist: "dtxmaniars".into(),
            score: 9999,
            max_combo: 100,
            perfect: 50,
            great: 10,
            good: 5,
            ok: 2,
            miss: 1,
            rank: Rank::S,
            played_at: 1_700_000_000,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ScoreEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.title, back.title);
        assert_eq!(entry.score, back.score);
        assert_eq!(entry.rank, back.rank);
    }

    #[test]
    fn rank_serializes_as_letter() {
        let entry = ScoreEntry {
            chart_hash: "x".into(),
            title: "y".into(),
            artist: "z".into(),
            score: 0,
            max_combo: 0,
            perfect: 0,
            great: 0,
            good: 0,
            ok: 0,
            miss: 0,
            rank: Rank::A,
            played_at: 0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"A\""), "json={}", json);
    }

    fn tempdir_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("dtxmaniars_test_{name}"));
        p
    }
}
