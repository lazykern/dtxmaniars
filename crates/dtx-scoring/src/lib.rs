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

pub mod gauge;
pub mod hit_ranges;
pub mod identity;
pub mod replay;
pub mod score_ini;
pub mod store;
pub mod xg_score;

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use store::{
    JudgmentTotals, NxImportRecord, ScoreEntry, ScoreSource, ScoreStore, ScoreStoreError,
};

/// Judgment kind for a single hit. Maps to DTXmaniaNX timing windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JudgmentKind {
    /// Tightest window (default ±34ms).
    Perfect,
    /// Second tier (default ±67ms).
    Great,
    /// Third tier (default ±84ms).
    Good,
    /// Fourth tier (default ±117ms).
    Poor,
    /// Out of window or no input.
    Miss,
}

/// Default DTXmaniaNX timing windows in milliseconds.
#[allow(missing_docs)]
pub const DEFAULT_WINDOWS_MS: &[(JudgmentKind, i32)] = &[
    (JudgmentKind::Perfect, 34),
    (JudgmentKind::Great, 67),
    (JudgmentKind::Good, 84),
    (JudgmentKind::Poor, 117),
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

/// DTXManiaNX result rank.
///
/// XG rank formula is `P%*0.85 + G%*0.35 + combo%*0.15` with thresholds
/// SS/S/A/B/C/D/E = 95/80/73/63/53/45/0.
/// Reference: `CScoreIni.cs:1307-1327`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rank {
    /// No judgeable total.
    Unknown,
    /// Best.
    SS,
    /// Excellent.
    S,
    /// Very good.
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
    /// Legacy perfect-only rank kept for older callers/tests.
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

    /// Compute DTXManiaNX XG rank from result counts.
    pub fn from_bocud_counts(
        total: u32,
        perfect: u32,
        great: u32,
        good: u32,
        poor: u32,
        miss: u32,
        max_combo: u32,
    ) -> Self {
        if total == 0 {
            return Rank::Unknown;
        }
        let judged = perfect + great + good + poor + miss;
        if judged == 0 {
            return Rank::SS;
        }
        let denom = judged as f64;
        let rate = 100.0 * perfect as f64 / denom * 0.85
            + 100.0 * great as f64 / denom * 0.35
            + 100.0 * max_combo as f64 / denom * 0.15;
        match rate {
            r if r >= 95.0 => Rank::SS,
            r if r >= 80.0 => Rank::S,
            r if r >= 73.0 => Rank::A,
            r if r >= 63.0 => Rank::B,
            r if r >= 53.0 => Rank::C,
            r if r >= 45.0 => Rank::D,
            _ => Rank::E,
        }
    }
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Rank::Unknown => "UNKNOWN",
            Rank::SS => "SS",
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
    use std::path::PathBuf;

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

    #[test]
    fn bocud_rank_all_perfect_full_combo_is_ss() {
        assert_eq!(Rank::from_bocud_counts(100, 100, 0, 0, 0, 0, 100), Rank::SS);
    }

    #[test]
    fn bocud_rank_counts_great_and_combo() {
        assert_eq!(Rank::from_bocud_counts(100, 80, 20, 0, 0, 0, 100), Rank::S);
    }

    #[test]
    fn bocud_rank_zero_total_is_unknown() {
        assert_eq!(Rank::from_bocud_counts(0, 0, 0, 0, 0, 0, 0), Rank::Unknown);
    }

    fn fake_entry(score: u32, chart_hash: &str) -> ScoreEntry {
        ScoreEntry {
            id: format!("{chart_hash}:{score}"),
            chart: identity::ChartIdentity::new(chart_hash.to_string(), None, None),
            title: "T".into(),
            artist: "A".into(),
            score,
            max_combo: 100,
            judgments: JudgmentTotals {
                perfect: 50,
                great: 5,
                good: 3,
                poor: 1,
                miss: 0,
            },
            rank: Rank::S,
            played_at: 1700000000,
            source: ScoreSource::Native,
            replay_ref: None,
        }
    }

    #[test]
    fn score_entry_total_sums_all_judgments() {
        let e = fake_entry(1000, "h1");
        assert_eq!(e.total(), 59);
    }

    #[test]
    fn score_entry_perfect_pct_zero_total() {
        let mut e = fake_entry(0, "h");
        e.judgments = JudgmentTotals::default();
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
        let mut entry = fake_entry(9999, "abc123");
        entry.title = "Round Trip".into();
        entry.artist = "dtxmaniars".into();
        entry.judgments = JudgmentTotals {
            perfect: 50,
            great: 10,
            good: 5,
            poor: 2,
            miss: 1,
        };
        entry.played_at = 1_700_000_000;
        let json = serde_json::to_string(&entry).unwrap();
        let back: ScoreEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.title, back.title);
        assert_eq!(entry.score, back.score);
        assert_eq!(entry.rank, back.rank);
    }

    #[test]
    fn rank_serializes_as_letter() {
        let mut entry = fake_entry(0, "x");
        entry.title = "y".into();
        entry.artist = "z".into();
        entry.rank = Rank::A;
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"A\""), "json={}", json);
    }

    fn tempdir_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("dtxmaniars_test_{name}"));
        p
    }
}
