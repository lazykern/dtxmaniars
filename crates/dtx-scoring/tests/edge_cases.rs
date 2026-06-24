//! Edge case tests for dtx-scoring.
//!
//! Covers score entry, rank thresholds, hit range boundary values,
//! and gauge state transitions.

use dtx_scoring::gauge::{
    gauge_delta, ComboState, GaugeState, GAUGE_EXCELLENT, GAUGE_GOOD, GAUGE_START,
};
use dtx_scoring::hit_ranges::{
    classify_with_difficulty, classify_with_ranges, Difficulty, HitRanges,
};
use dtx_scoring::JudgmentKind;
use dtx_scoring::{Rank, ScoreEntry, ScoreStore};

#[test]
fn rank_s_lower_bound() {
    assert_eq!(Rank::from_perfect_pct(95.0), Rank::S);
    assert_eq!(Rank::from_perfect_pct(94.99), Rank::A);
}

#[test]
fn rank_a_lower_bound() {
    assert_eq!(Rank::from_perfect_pct(85.0), Rank::A);
    assert_eq!(Rank::from_perfect_pct(84.99), Rank::B);
}

#[test]
fn rank_b_lower_bound() {
    assert_eq!(Rank::from_perfect_pct(70.0), Rank::B);
    assert_eq!(Rank::from_perfect_pct(69.99), Rank::C);
}

#[test]
fn rank_c_lower_bound() {
    assert_eq!(Rank::from_perfect_pct(50.0), Rank::C);
    assert_eq!(Rank::from_perfect_pct(49.99), Rank::D);
}

#[test]
fn rank_d_lower_bound() {
    assert_eq!(Rank::from_perfect_pct(25.0), Rank::D);
    assert_eq!(Rank::from_perfect_pct(24.99), Rank::E);
}

#[test]
fn rank_e_at_zero() {
    assert_eq!(Rank::from_perfect_pct(0.0), Rank::E);
}

#[test]
fn rank_display_all() {
    assert_eq!(format!("{}", Rank::S), "S");
    assert_eq!(format!("{}", Rank::A), "A");
    assert_eq!(format!("{}", Rank::B), "B");
    assert_eq!(format!("{}", Rank::C), "C");
    assert_eq!(format!("{}", Rank::D), "D");
    assert_eq!(format!("{}", Rank::E), "E");
}

#[test]
fn rank_equality_and_hash() {
    assert_eq!(Rank::S, Rank::S);
    assert_ne!(Rank::S, Rank::A);
    let mut set = std::collections::HashSet::new();
    set.insert(Rank::S);
    set.insert(Rank::A);
    set.insert(Rank::S);
    assert_eq!(set.len(), 2);
}

#[test]
fn rank_clone_copy() {
    let r = Rank::S;
    let r2 = r; // Copy
    assert_eq!(r, r2);
}

#[test]
fn score_entry_default_total() {
    let e = ScoreEntry {
        chart_hash: "h".into(),
        title: "T".into(),
        artist: "A".into(),
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
    assert_eq!(e.total(), 0);
    assert_eq!(e.perfect_pct(), 0.0);
}

#[test]
fn score_entry_perfect_pct_full() {
    let e = ScoreEntry {
        chart_hash: "h".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 0,
        max_combo: 100,
        perfect: 100,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    };
    assert!((e.perfect_pct() - 100.0).abs() < 0.01);
}

#[test]
fn score_entry_perfect_pct_mixed() {
    let e = ScoreEntry {
        chart_hash: "h".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 0,
        max_combo: 0,
        perfect: 50,
        great: 30,
        good: 10,
        ok: 5,
        miss: 5,
        rank: Rank::B,
        played_at: 0,
    };
    // 50/100 = 50%
    assert!((e.perfect_pct() - 50.0).abs() < 0.01);
    assert_eq!(e.total(), 100);
}

#[test]
fn score_store_add_save_load() {
    let tmp = std::env::temp_dir().join("dtx_scoring_test_save.json");
    let _ = std::fs::remove_file(&tmp);
    let mut s = ScoreStore::with_path(tmp.clone());
    s.add(ScoreEntry {
        chart_hash: "abc".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 1000,
        max_combo: 50,
        perfect: 50,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 100,
    });
    s.save().unwrap();
    let mut s2 = ScoreStore::with_path(tmp.clone());
    s2.load().unwrap();
    assert_eq!(s2.entries.len(), 1);
    assert_eq!(s2.entries[0].score, 1000);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn score_store_best_for_chart() {
    let mut s = ScoreStore::default();
    s.add(ScoreEntry {
        chart_hash: "a".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 1000,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    });
    s.add(ScoreEntry {
        chart_hash: "a".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 2000,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    });
    s.add(ScoreEntry {
        chart_hash: "b".into(),
        title: "T2".into(),
        artist: "A".into(),
        score: 500,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::A,
        played_at: 0,
    });
    let best = s.best_for("a").unwrap();
    assert_eq!(best.score, 2000);
    assert!(s.best_for("nonexistent").is_none());
}

#[test]
fn score_store_chart_count() {
    let mut s = ScoreStore::default();
    s.add(make_entry("a", 100));
    s.add(make_entry("a", 200));
    s.add(make_entry("b", 300));
    s.add(make_entry("c", 400));
    assert_eq!(s.chart_count(), 3);
    assert_eq!(s.len(), 4);
    assert!(!s.is_empty());
}

fn make_entry(hash: &str, score: u32) -> ScoreEntry {
    ScoreEntry {
        chart_hash: hash.into(),
        title: "T".into(),
        artist: "A".into(),
        score,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    }
}

#[test]
fn gauge_constants() {
    assert!((GAUGE_START - 20.0).abs() < 0.01);
    assert!((GAUGE_GOOD - 80.0).abs() < 0.01);
    assert!((GAUGE_EXCELLENT - 100.0).abs() < 0.01);
}

#[test]
fn gauge_delta_perfect_positive() {
    assert!(gauge_delta(JudgmentKind::Perfect) > 0.0);
}

#[test]
fn gauge_delta_great_smaller_than_perfect() {
    assert!(gauge_delta(JudgmentKind::Great) < gauge_delta(JudgmentKind::Perfect));
}

#[test]
fn gauge_delta_miss_largest_loss() {
    assert!(gauge_delta(JudgmentKind::Miss) < gauge_delta(JudgmentKind::Ok));
}

#[test]
fn gauge_state_starts_at_20() {
    let g = GaugeState::new();
    assert!((g.value - 20.0).abs() < 0.01);
    assert!(!g.cleared);
    assert!(!g.failed);
}

#[test]
fn combo_state_fc_breaks_on_first_miss() {
    let mut c = ComboState::new();
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Miss);
    assert!(!c.is_full_combo());
    assert!(!c.is_all_perfect());
}

#[test]
fn combo_state_ap_breaks_on_great() {
    let mut c = ComboState::new();
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Great); // not perfect
    assert!(!c.is_all_perfect());
    assert!(c.is_full_combo()); // still FC since no miss/ok
}

#[test]
fn combo_state_ok_breaks_combo_but_keeps_max() {
    let mut c = ComboState::new();
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Ok);
    assert_eq!(c.current, 0);
    assert_eq!(c.max, 2);
    assert_eq!(c.imperfect_count, 1);
}

#[test]
fn hit_ranges_normal_classify_extremes() {
    let r = HitRanges::normal();
    assert_eq!(classify_with_ranges(0, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(-200, r), JudgmentKind::Miss);
    assert_eq!(classify_with_ranges(200, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_master_strict() {
    let r = HitRanges::master();
    // Master: 5/10/20/30/60
    assert_eq!(classify_with_ranges(5, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(6, r), JudgmentKind::Great);
    assert_eq!(classify_with_ranges(60, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_easy_loose() {
    let r = HitRanges::easy();
    // Easy: 24/48/96/150/300
    assert_eq!(classify_with_ranges(24, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(25, r), JudgmentKind::Great);
    assert_eq!(classify_with_ranges(300, r), JudgmentKind::Miss);
    assert_eq!(classify_with_ranges(301, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_window_helper() {
    let r = HitRanges::expert();
    assert_eq!(r.window(JudgmentKind::Perfect), 8);
    assert_eq!(r.window(JudgmentKind::Miss), 100);
}

#[test]
fn hit_ranges_new_constructor() {
    let r = HitRanges::new(20, 40, 80, 120, 250);
    assert_eq!(r.perfect, 20);
    assert_eq!(r.miss, 250);
}

#[test]
fn classify_with_difficulty_uses_correct_ranges() {
    // 50ms delta: Normal=Good (50<=64), Master=Miss (50>30), Easy=Great (50<=96)
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Normal),
        JudgmentKind::Good
    );
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Master),
        JudgmentKind::Miss
    );
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Easy),
        JudgmentKind::Good
    );
}

#[test]
fn difficulty_default_is_normal() {
    assert_eq!(Difficulty::default(), Difficulty::Normal);
}

#[test]
fn difficulty_as_str_all() {
    for d in Difficulty::all() {
        let s = d.as_str();
        assert!(!s.is_empty());
    }
}

#[test]
fn score_entry_equality() {
    let a = ScoreEntry {
        chart_hash: "h".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 100,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn score_entry_clone() {
    let a = ScoreEntry {
        chart_hash: "h".into(),
        title: "T".into(),
        artist: "A".into(),
        score: 100,
        max_combo: 0,
        perfect: 0,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        rank: Rank::S,
        played_at: 0,
    };
    let b = a.clone();
    assert_eq!(a.score, b.score);
    assert_eq!(a.title, b.title);
}

#[test]
fn score_store_is_empty_default() {
    let s = ScoreStore::default();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert_eq!(s.chart_count(), 0);
}
