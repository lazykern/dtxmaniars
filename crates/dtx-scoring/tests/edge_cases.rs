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
use dtx_scoring::identity::ChartIdentity;
use dtx_scoring::JudgmentKind;
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource, ScoreStore};

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
    let mut e = make_entry("h", 0);
    e.rank = Rank::E;
    assert_eq!(e.total(), 0);
    assert_eq!(e.perfect_pct(), 0.0);
}

#[test]
fn score_entry_perfect_pct_full() {
    let mut e = make_entry("h", 0);
    e.max_combo = 100;
    e.judgments.perfect = 100;
    assert!((e.perfect_pct() - 100.0).abs() < 0.01);
}

#[test]
fn score_entry_perfect_pct_mixed() {
    let mut e = make_entry("h", 0);
    e.judgments = JudgmentTotals {
        perfect: 50,
        great: 30,
        good: 10,
        poor: 5,
        miss: 5,
    };
    e.rank = Rank::B;
    // 50/100 = 50%
    assert!((e.perfect_pct() - 50.0).abs() < 0.01);
    assert_eq!(e.total(), 100);
}

#[test]
fn score_store_add_save_load() {
    let tmp = std::env::temp_dir().join("dtx_scoring_test_save.json");
    let _ = std::fs::remove_file(&tmp);
    let mut s = ScoreStore::with_path(tmp.clone());
    let mut entry = make_entry("abc", 1000);
    entry.max_combo = 50;
    entry.judgments.perfect = 50;
    entry.played_at = 100;
    s.add(entry);
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
    s.add(make_entry("a", 1000));
    s.add(make_entry("a", 2000));
    let mut other = make_entry("b", 500);
    other.title = "T2".into();
    other.rank = Rank::A;
    s.add(other);
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
        id: format!("{hash}:{score}"),
        chart: ChartIdentity::new(hash.into(), None, None),
        title: "T".into(),
        artist: "A".into(),
        score: i64::from(score),
        chart_level: 0.0,
        performance_skill: 0.0,
        song_skill: 0.0,
        max_combo: 0,
        judgments: JudgmentTotals::default(),
        rank: Rank::S,
        played_at: 0,
        source: ScoreSource::Native,
        replay_ref: None,
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
    assert!(gauge_delta(JudgmentKind::Miss) < gauge_delta(JudgmentKind::Poor));
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
    assert!(c.is_full_combo()); // still FC since no miss/poor
}

#[test]
fn combo_state_poor_breaks_combo_but_keeps_max() {
    let mut c = ComboState::new();
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Poor);
    assert_eq!(c.current, 0);
    assert_eq!(c.max, 2);
    assert_eq!(c.imperfect_count, 1);
}

#[test]
fn hit_ranges_normal_classify_extremes() {
    let r = HitRanges::normal();
    assert_eq!(classify_with_ranges(0, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(-118, r), JudgmentKind::Miss);
    assert_eq!(classify_with_ranges(118, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_master_strict() {
    let r = HitRanges::master();
    // Master: 10/20/25/35/36
    assert_eq!(classify_with_ranges(10, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(11, r), JudgmentKind::Great);
    assert_eq!(classify_with_ranges(36, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_easy_loose() {
    let r = HitRanges::easy();
    // Easy: 51/101/126/176/177
    assert_eq!(classify_with_ranges(51, r), JudgmentKind::Perfect);
    assert_eq!(classify_with_ranges(52, r), JudgmentKind::Great);
    assert_eq!(classify_with_ranges(177, r), JudgmentKind::Miss);
    assert_eq!(classify_with_ranges(178, r), JudgmentKind::Miss);
}

#[test]
fn hit_ranges_window_helper() {
    let r = HitRanges::expert();
    assert_eq!(r.window(JudgmentKind::Perfect), 17);
    assert_eq!(r.window(JudgmentKind::Miss), 60);
}

#[test]
fn hit_ranges_new_constructor() {
    let r = HitRanges::new(20, 40, 80, 120, 250);
    assert_eq!(r.perfect, 20);
    assert_eq!(r.poor, 120);
    assert_eq!(r.miss, 250);
}

#[test]
fn classify_with_difficulty_uses_correct_ranges() {
    // 50ms delta: Normal=Great (50<=67), Master=Miss (50>35), Easy=Perfect (50<=51)
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Normal),
        JudgmentKind::Great
    );
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Master),
        JudgmentKind::Miss
    );
    assert_eq!(
        classify_with_difficulty(50, Difficulty::Easy),
        JudgmentKind::Perfect
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
    let a = make_entry("h", 100);
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn score_entry_clone() {
    let a = make_entry("h", 100);
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
