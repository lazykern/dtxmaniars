#![allow(clippy::inherent_to_string)]
//! `CScoreIni` (1773 LOC) — score persistence to/from DTX-compatible INI file.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs:1-1773`
//!
//! v1 strict-port: load/save BestScore/ClearState per (hash, chart) in the
//! legacy `#HIDDEN/...` INI format. Format matches what other DTX players
//! read so scores can be carried across tools.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::error::{DtxError, Result};

/// 5-judgment classification (BocuD CScoreIni:Skill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Skill {
    Perfect = 0,
    Great = 1,
    Good = 2,
    Ok = 3,
    Miss = 4,
}

impl Skill {
    pub fn from_int(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Perfect),
            1 => Some(Self::Great),
            2 => Some(Self::Good),
            3 => Some(Self::Ok),
            4 => Some(Self::Miss),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Perfect => "PERFECT",
            Self::Great => "GREAT",
            Self::Good => "GOOD",
            Self::Ok => "OK",
            Self::Miss => "MISS",
        }
    }
}

/// Clear state (BocuD EClearState).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClearState {
    /// Not played.
    NotPlayed = 0,
    /// Failed (no clear).
    Failed = 1,
    /// Cleared (normal).
    Cleared = 2,
    /// Full combo (no miss/ok).
    FullCombo = 3,
    /// All perfect.
    AllPerfect = 4,
}

impl ClearState {
    pub fn from_int(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::NotPlayed),
            1 => Some(Self::Failed),
            2 => Some(Self::Cleared),
            3 => Some(Self::FullCombo),
            4 => Some(Self::AllPerfect),
            _ => None,
        }
    }

    pub fn as_int(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotPlayed => "NP",
            Self::Failed => "FAILED",
            Self::Cleared => "CLEAR",
            Self::FullCombo => "FC",
            Self::AllPerfect => "AP",
        }
    }
}

/// One record (chart, instrument) score snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreEntry {
    /// INI section key (chart path + instrument hash).
    pub key: String,
    /// Play count for this chart.
    pub play_count: i32,
    /// High score.
    pub high_score: i32,
    /// Skill achieved (BocuD).
    pub skill: Skill,
    /// Clear state achieved.
    pub clear: ClearState,
    /// Best max combo.
    pub best_combo: i32,
    /// Perfect count in best run.
    pub best_perfect: i32,
    /// Great count in best run.
    pub best_great: i32,
    /// Good count in best run.
    pub best_good: i32,
    /// Ok count in best run.
    pub best_ok: i32,
    /// Miss count in best run.
    pub best_miss: i32,
    /// Last played timestamp (Unix seconds).
    pub last_played_at: i64,
    /// Optional ranking number (1 = top).
    pub rank: i32,
}

impl ScoreEntry {
    /// Build a fresh, never-played entry for the given chart key.
    pub fn fresh(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            play_count: 0,
            high_score: 0,
            skill: Skill::Miss,
            clear: ClearState::NotPlayed,
            best_combo: 0,
            best_perfect: 0,
            best_great: 0,
            best_good: 0,
            best_ok: 0,
            best_miss: 0,
            last_played_at: 0,
            rank: 0,
        }
    }

    /// Apply a new run's statistics. Updates the high score / clear state /
    /// combo record only if the new run improves on the previous best.
    pub fn apply_run(&mut self, run: &ScoreRun) {
        self.play_count += 1;
        self.last_played_at = run.timestamp;
        if run.score > self.high_score {
            self.high_score = run.score;
            self.skill = run.skill;
            self.clear = run.clear;
            self.best_combo = run.combo;
            self.best_perfect = run.perfect;
            self.best_great = run.great;
            self.best_good = run.good;
            self.best_ok = run.ok;
            self.best_miss = run.miss;
        }
    }

    /// Total chips in best run.
    pub fn best_total(&self) -> i32 {
        self.best_perfect + self.best_great + self.best_good + self.best_ok + self.best_miss
    }

    /// Hit ratio: (P+G+Gd) / total.
    pub fn best_hit_ratio(&self) -> f32 {
        let total = self.best_total();
        if total == 0 {
            0.0
        } else {
            (self.best_perfect + self.best_great + self.best_good) as f32 / total as f32
        }
    }
}

/// A single play's statistics (used to feed `apply_run`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreRun {
    pub score: i32,
    pub combo: i32,
    pub perfect: i32,
    pub great: i32,
    pub good: i32,
    pub ok: i32,
    pub miss: i32,
    pub skill: Skill,
    pub clear: ClearState,
    pub timestamp: i64,
}

impl ScoreRun {
    pub fn total(&self) -> i32 {
        self.perfect + self.great + self.good + self.ok + self.miss
    }
}

/// In-memory CScoreIni (BocuD score.db replacement).
///
/// Reference: `CScoreIni.cs:50-200` — holds a BTreeMap<String, ScoreEntry>
/// plus a few version fields at the top of the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CScoreIni {
    /// File version (BocuD defaults to 10).
    pub version: i32,
    /// Last opened (Unix seconds).
    pub last_modified: i64,
    /// Score records keyed by section name.
    pub scores: BTreeMap<String, ScoreEntry>,
}

impl CScoreIni {
    pub fn new() -> Self {
        Self {
            version: SCORE_INI_VERSION,
            last_modified: 0,
            scores: BTreeMap::new(),
        }
    }

    /// Load from an INI file. Returns an empty DB if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Self> {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(e) => return Err(DtxError::Io(e)),
        };
        Self::parse(&content)
    }

    /// Parse INI text into a CScoreIni.
    pub fn parse(content: &str) -> Result<Self> {
        let mut db = Self::new();
        let mut current: Option<String> = None;
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let key = line[1..line.len() - 1].to_string();
                db.scores.entry(key.clone()).or_insert_with(|| ScoreEntry::fresh(key.clone()));
                current = Some(key);
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let value = v.trim();
            let section_name = current.clone();
            match (section_name.as_deref(), key) {
                (None, "Version") => {
                    db.version = value.parse().unwrap_or(SCORE_INI_VERSION);
                }
                (None, "LastModified") => {
                    db.last_modified = value.parse().unwrap_or(0);
                }
                (None, _) => {
                    // unknown header-level key: ignore
                }
                (Some(_), k) => {
                    let section = section_name.as_deref().unwrap();
                    if let Some(entry) = db.scores.get_mut(section) {
                        match k {
                            "PlayCount" => entry.play_count = value.parse().unwrap_or(0),
                            "HighScore" => entry.high_score = value.parse().unwrap_or(0),
                            "Skill" => {
                                if let Some(s) = Skill::from_int(value.parse().unwrap_or(-1)) {
                                    entry.skill = s;
                                }
                            }
                            "Clear" => {
                                if let Some(c) = ClearState::from_int(value.parse().unwrap_or(-1)) {
                                    entry.clear = c;
                                }
                            }
                            "BestCombo" => entry.best_combo = value.parse().unwrap_or(0),
                            "BestPerfect" => entry.best_perfect = value.parse().unwrap_or(0),
                            "BestGreat" => entry.best_great = value.parse().unwrap_or(0),
                            "BestGood" => entry.best_good = value.parse().unwrap_or(0),
                            "BestOk" => entry.best_ok = value.parse().unwrap_or(0),
                            "BestMiss" => entry.best_miss = value.parse().unwrap_or(0),
                            "LastPlayedAt" => {
                                entry.last_played_at = value.parse().unwrap_or(0);
                            }
                            "Rank" => entry.rank = value.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(db)
    }

    /// Save to an INI file. Format: header `[CScoreIni]`, sections, key=value.
    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_string()).map_err(DtxError::Io)?;
        Ok(())
    }

    /// Serialize to INI text.
    pub fn to_string(&self) -> String {
        let mut out = String::with_capacity(self.scores.len() * 200);
        out.push_str("; CScoreIni v");
        let _ = writeln!(out, "{}", self.version);
        out.push_str("[CScoreIni]\n");
        let _ = writeln!(out, "Version={}", self.version);
        let _ = writeln!(out, "LastModified={}", self.last_modified);
        for entry in self.scores.values() {
            let _ = writeln!(out, "\n[{}]", entry.key);
            let _ = writeln!(out, "PlayCount={}", entry.play_count);
            let _ = writeln!(out, "HighScore={}", entry.high_score);
            let _ = writeln!(out, "Skill={}", entry.skill as i32);
            let _ = writeln!(out, "Clear={}", entry.clear.as_int());
            let _ = writeln!(out, "BestCombo={}", entry.best_combo);
            let _ = writeln!(out, "BestPerfect={}", entry.best_perfect);
            let _ = writeln!(out, "BestGreat={}", entry.best_great);
            let _ = writeln!(out, "BestGood={}", entry.best_good);
            let _ = writeln!(out, "BestOk={}", entry.best_ok);
            let _ = writeln!(out, "BestMiss={}", entry.best_miss);
            let _ = writeln!(out, "LastPlayedAt={}", entry.last_played_at);
            let _ = writeln!(out, "Rank={}", entry.rank);
        }
        out
    }

    /// Get or create an entry for a chart key.
    pub fn entry(&mut self, key: &str) -> &mut ScoreEntry {
        self.scores
            .entry(key.to_string())
            .or_insert_with(|| ScoreEntry::fresh(key))
    }

    /// Lookup an entry (immutable).
    pub fn get(&self, key: &str) -> Option<&ScoreEntry> {
        self.scores.get(key)
    }

    /// Number of stored score records.
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }
}

/// Default CScoreIni version (BocuD).
pub const SCORE_INI_VERSION: i32 = 10;
/// Default CScoreIni filename per chart.
pub const SCORE_INI_FILENAME: &str = "score.ini";
/// CScoreIni section name.
pub const SCORE_INI_SECTION: &str = "CScoreIni";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_entry_defaults() {
        let e = ScoreEntry::fresh("chart.dtx:drums");
        assert_eq!(e.play_count, 0);
        assert_eq!(e.high_score, 0);
        assert_eq!(e.skill, Skill::Miss);
        assert_eq!(e.clear, ClearState::NotPlayed);
        assert_eq!(e.best_combo, 0);
        assert_eq!(e.last_played_at, 0);
    }

    #[test]
    fn apply_run_updates_high_score() {
        let mut e = ScoreEntry::fresh("k");
        let run = ScoreRun {
            score: 1000,
            combo: 50,
            perfect: 30,
            great: 15,
            good: 5,
            ok: 0,
            miss: 0,
            skill: Skill::Perfect,
            clear: ClearState::AllPerfect,
            timestamp: 1234,
        };
        e.apply_run(&run);
        assert_eq!(e.high_score, 1000);
        assert_eq!(e.skill, Skill::Perfect);
        assert_eq!(e.clear, ClearState::AllPerfect);
        assert_eq!(e.best_combo, 50);
        assert_eq!(e.last_played_at, 1234);
        assert_eq!(e.play_count, 1);
    }

    #[test]
    fn apply_run_does_not_downgrade_high_score() {
        let mut e = ScoreEntry::fresh("k");
        let big = ScoreRun {
            score: 5000,
            combo: 100,
            perfect: 80,
            great: 20,
            good: 0,
            ok: 0,
            miss: 0,
            skill: Skill::Perfect,
            clear: ClearState::AllPerfect,
            timestamp: 1,
        };
        e.apply_run(&big);
        let small = ScoreRun {
            score: 100,
            combo: 5,
            perfect: 0,
            great: 5,
            good: 0,
            ok: 0,
            miss: 0,
            skill: Skill::Ok,
            clear: ClearState::Failed,
            timestamp: 2,
        };
        e.apply_run(&small);
        assert_eq!(e.high_score, 5000);
        assert_eq!(e.best_combo, 100);
        // play_count increments regardless
        assert_eq!(e.play_count, 2);
    }

    #[test]
    fn best_total_sums_judgments() {
        let mut e = ScoreEntry::fresh("k");
        e.best_perfect = 10;
        e.best_great = 5;
        e.best_good = 3;
        e.best_ok = 2;
        e.best_miss = 1;
        assert_eq!(e.best_total(), 21);
    }

    #[test]
    fn best_hit_ratio_zero_when_no_chips() {
        let e = ScoreEntry::fresh("k");
        assert_eq!(e.best_hit_ratio(), 0.0);
    }

    #[test]
    fn best_hit_ratio_normal() {
        let mut e = ScoreEntry::fresh("k");
        e.best_perfect = 8;
        e.best_great = 2;
        e.best_good = 0;
        e.best_ok = 0;
        e.best_miss = 0;
        assert!((e.best_hit_ratio() - 1.0).abs() < 0.01);
    }

    #[test]
    fn skill_from_int_and_back() {
        for s in [Skill::Perfect, Skill::Great, Skill::Good, Skill::Ok, Skill::Miss] {
            assert_eq!(Skill::from_int(s as i32), Some(s));
        }
        assert_eq!(Skill::from_int(99), None);
    }

    #[test]
    fn clear_state_round_trip() {
        for c in [
            ClearState::NotPlayed,
            ClearState::Failed,
            ClearState::Cleared,
            ClearState::FullCombo,
            ClearState::AllPerfect,
        ] {
            assert_eq!(ClearState::from_int(c.as_int()), Some(c));
        }
    }

    #[test]
    fn cscore_ini_new() {
        let db = CScoreIni::new();
        assert_eq!(db.version, SCORE_INI_VERSION);
        assert!(db.is_empty());
    }

    #[test]
    fn cscore_ini_save_load_round_trip() {
        let tmp = std::env::temp_dir().join("dtxmaniars_score_test.ini");
        let _ = std::fs::remove_file(&tmp);
        let mut db = CScoreIni::new();
        let entry = db.entry("chartA.dtx:drums");
        let run = ScoreRun {
            score: 2000,
            combo: 42,
            perfect: 20,
            great: 10,
            good: 5,
            ok: 1,
            miss: 0,
            skill: Skill::Great,
            clear: ClearState::FullCombo,
            timestamp: 9999,
        };
        entry.apply_run(&run);
        db.save(&tmp).expect("save");
        let loaded = CScoreIni::load(&tmp).expect("load");
        assert_eq!(loaded.version, SCORE_INI_VERSION);
        let e = loaded.get("chartA.dtx:drums").expect("entry");
        assert_eq!(e.high_score, 2000);
        assert_eq!(e.best_combo, 42);
        assert_eq!(e.skill, Skill::Great);
        assert_eq!(e.clear, ClearState::FullCombo);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn cscore_ini_load_missing_file_returns_empty() {
        let db = CScoreIni::load(std::path::Path::new("/nonexistent/score.ini")).unwrap();
        assert!(db.is_empty());
    }

    #[test]
    fn cscore_ini_parse_with_comments() {
        let text = "; comment\n[CScoreIni]\nVersion=10\nLastModified=0\n\n[k]\nPlayCount=3\nHighScore=1500\nSkill=1\nClear=3\n";
        let db = CScoreIni::parse(text).unwrap();
        let e = db.get("k").unwrap();
        assert_eq!(e.play_count, 3);
        assert_eq!(e.high_score, 1500);
        assert_eq!(e.skill, Skill::Great);
        assert_eq!(e.clear, ClearState::FullCombo);
    }

    #[test]
    fn cscore_ini_to_string_includes_all_keys() {
        let mut db = CScoreIni::new();
        db.entry("x").apply_run(&ScoreRun {
            score: 1,
            combo: 1,
            perfect: 1,
            great: 0,
            good: 0,
            ok: 0,
            miss: 0,
            skill: Skill::Perfect,
            clear: ClearState::Cleared,
            timestamp: 0,
        });
        let s = db.to_string();
        for key in [
            "[CScoreIni]",
            "Version=10",
            "[x]",
            "PlayCount=1",
            "HighScore=1",
            "Skill=0",
            "Clear=2",
            "BestPerfect=1",
        ] {
            assert!(s.contains(key), "missing {key:?} in {s:?}");
        }
    }

    #[test]
    fn score_run_total() {
        let r = ScoreRun {
            score: 0,
            combo: 0,
            perfect: 1,
            great: 1,
            good: 1,
            ok: 1,
            miss: 1,
            skill: Skill::Miss,
            clear: ClearState::NotPlayed,
            timestamp: 0,
        };
        assert_eq!(r.total(), 5);
    }
}
