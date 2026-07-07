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
