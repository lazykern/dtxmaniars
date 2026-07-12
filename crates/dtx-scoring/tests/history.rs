//! `ScoreStore::history_for_path` ordering, filtering, and limits.

use std::path::{Path, PathBuf};

use dtx_scoring::identity::ChartIdentity;
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource, ScoreStore};

fn entry(path: &str, score: u32, played_at: u64) -> ScoreEntry {
    ScoreEntry {
        id: format!("test:{path}:{score}:{played_at}"),
        chart: ChartIdentity::new(format!("dtx1:{path}"), None, Some(PathBuf::from(path))),
        title: "Title".into(),
        artist: "Artist".into(),
        score: i64::from(score),
        chart_level: 0.0,
        performance_skill: 0.0,
        song_skill: 0.0,
        max_combo: 0,
        judgments: JudgmentTotals::default(),
        rank: Rank::A,
        played_at,
        source: ScoreSource::Native,
        replay_ref: None,
    }
}

fn store_with(entries: Vec<ScoreEntry>) -> ScoreStore {
    let mut store = ScoreStore::default();
    for e in entries {
        store.add(e);
    }
    store
}

#[test]
fn orders_by_score_descending() {
    let store = store_with(vec![
        entry("a.dtx", 100, 1),
        entry("a.dtx", 300, 2),
        entry("a.dtx", 200, 3),
    ]);
    let scores: Vec<i64> = store
        .history_for_path(Path::new("a.dtx"), 8)
        .iter()
        .map(|e| e.score)
        .collect();
    assert_eq!(scores, vec![300, 200, 100]);
}

#[test]
fn score_ties_break_most_recent_first() {
    let store = store_with(vec![
        entry("a.dtx", 200, 10),
        entry("a.dtx", 200, 30),
        entry("a.dtx", 200, 20),
    ]);
    let played: Vec<u64> = store
        .history_for_path(Path::new("a.dtx"), 8)
        .iter()
        .map(|e| e.played_at)
        .collect();
    assert_eq!(played, vec![30, 20, 10]);
}

#[test]
fn respects_limit() {
    let store = store_with((0..12).map(|i| entry("a.dtx", i, i as u64)).collect());
    assert_eq!(store.history_for_path(Path::new("a.dtx"), 8).len(), 8);
}

#[test]
fn filters_by_path_hint() {
    let mut store = store_with(vec![entry("a.dtx", 100, 1), entry("b.dtx", 200, 2)]);
    let mut no_hint = entry("a.dtx", 999, 3);
    no_hint.chart.source_path_hint = None;
    store.add(no_hint);
    let hits = store.history_for_path(Path::new("a.dtx"), 8);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].score, 100);
}

#[test]
fn empty_store_returns_empty() {
    let store = ScoreStore::default();
    assert!(store.history_for_path(Path::new("a.dtx"), 8).is_empty());
}
