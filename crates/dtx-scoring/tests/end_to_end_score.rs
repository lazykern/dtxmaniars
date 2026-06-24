//! End-to-end integration test (p0-8): load real_chart fixture, simulate a
//! play, persist ScoreEntry, reload, verify round-trip.
//!
//! Walks the data path: dtx-core parse → dtx-scoring ScoreEntry → JSON.
//! Bevy App staging is exercised in app/dtxmaniars-desktop boot test.

use std::fs::File;
use std::path::PathBuf;

use dtx_core::parser::parse;
use dtx_scoring::{Rank, ScoreEntry, ScoreStore};

/// Local counts struct; gameplay-drums has the same shape but lives in
/// game layer (bevy deps). dtx-scoring is Pure.
struct Counts {
    perfect: u32,
    great: u32,
    good: u32,
    ok: u32,
    miss: u32,
}

impl Counts {
    fn total(&self) -> u32 {
        self.perfect + self.great + self.good + self.ok + self.miss
    }
    fn perfect_pct(&self) -> f32 {
        if self.total() == 0 {
            0.0
        } else {
            100.0 * self.perfect as f32 / self.total() as f32
        }
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dtx-core")
        .join("tests")
        .join("fixtures")
        .join("real_chart.dtx")
}

#[test]
fn end_to_end_load_score_persist() {
    // 1. Load fixture
    let f = File::open(fixture()).expect("fixture exists");
    let chart = parse(f).expect("parses");
    assert_eq!(chart.metadata.title.as_deref(), Some("Real Chart Demo"));

    // 2. Build a ScoreEntry as if the user just played the chart.
    let counts = Counts {
        perfect: 18,
        great: 2,
        good: 1,
        ok: 0,
        miss: 0,
    };
    let entry = ScoreEntry {
        chart_hash: dtx_scoring::compute_chart_hash(&fixture()),
        title: chart.metadata.title.clone().unwrap(),
        artist: chart.metadata.artist.clone().unwrap(),
        score: 9000,
        max_combo: 21,
        perfect: counts.perfect,
        great: counts.great,
        good: counts.good,
        ok: counts.ok,
        miss: counts.miss,
        rank: Rank::from_perfect_pct(counts.perfect_pct()),
        played_at: 1_700_000_000,
    };

    // 3. Save to disk.
    let tmp = std::env::temp_dir().join("dtxmaniars_e2e_score");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let store_path = tmp.join("scores.json");
    let mut store = ScoreStore::with_path(store_path.clone());
    store.add(entry.clone());
    store.save().expect("save");

    // 4. Reload from disk.
    let mut store2 = ScoreStore::with_path(store_path);
    store2.load().expect("load");
    assert_eq!(store2.len(), 1);
    let loaded = &store2.entries[0];
    assert_eq!(loaded.title, "Real Chart Demo");
    assert_eq!(loaded.score, 9000);
    assert_eq!(loaded.max_combo, 21);
    assert_eq!(loaded.perfect, 18);
    assert_eq!(loaded.miss, 0);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn end_to_end_rank_computed_from_pct() {
    // 100% perfect → S
    let c1 = Counts {
        perfect: 21,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
    };
    assert_eq!(Rank::from_perfect_pct(c1.perfect_pct()), Rank::S);

    // 0% perfect → E
    let c2 = Counts {
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 21,
    };
    assert_eq!(Rank::from_perfect_pct(c2.perfect_pct()), Rank::E);
}

#[test]
fn end_to_end_score_serialization_round_trip() {
    // Verify the JSON the binary actually writes is the JSON it actually reads.
    let tmp = std::env::temp_dir().join("dtxmaniars_e2e_round");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let p = tmp.join("scores.json");

    let mut store = ScoreStore::with_path(p.clone());
    for i in 0..3 {
        store.add(ScoreEntry {
            chart_hash: format!("hash{i}"),
            title: format!("Song {i}"),
            artist: "test".into(),
            score: 1000 * (i as u32 + 1),
            max_combo: 50,
            perfect: 40,
            great: 5,
            good: 3,
            ok: 1,
            miss: 1,
            rank: Rank::A,
            played_at: 1_700_000_000 + i as u64,
        });
    }
    store.save().unwrap();
    assert!(p.exists());

    let mut store2 = ScoreStore::with_path(p);
    store2.load().unwrap();
    assert_eq!(store2.len(), 3);
    for (i, e) in store2.entries.iter().enumerate() {
        assert_eq!(e.title, format!("Song {i}"));
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
