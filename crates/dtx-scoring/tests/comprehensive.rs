//! Comprehensive tests for dtx-scoring — judgment edge cases.

use dtx_scoring::{classify, JudgmentKind, Rank};

#[test]
fn classify_zero_is_perfect() {
    assert_eq!(classify(0), JudgmentKind::Perfect);
}

#[test]
fn classify_small_positive_is_perfect() {
    assert_eq!(classify(10), JudgmentKind::Perfect);
    assert_eq!(classify(34), JudgmentKind::Perfect);
}

#[test]
fn classify_at_perfect_boundary_is_great() {
    assert_eq!(classify(35), JudgmentKind::Great);
}

#[test]
fn classify_at_great_boundary_is_good() {
    assert_eq!(classify(68), JudgmentKind::Good);
}

#[test]
fn classify_at_good_boundary_is_ok() {
    assert_eq!(classify(85), JudgmentKind::Poor);
}

#[test]
fn classify_at_ok_boundary_is_miss() {
    assert_eq!(classify(118), JudgmentKind::Miss);
}

#[test]
fn classify_negative_is_miss() {
    assert_eq!(classify(-1000), JudgmentKind::Miss);
    assert_eq!(classify(-117), JudgmentKind::Poor);
}

#[test]
fn classify_matches_bocud_default_hit_ranges() {
    assert_eq!(classify(34), JudgmentKind::Perfect);
    assert_eq!(classify(35), JudgmentKind::Great);
    assert_eq!(classify(67), JudgmentKind::Great);
    assert_eq!(classify(68), JudgmentKind::Good);
    assert_eq!(classify(84), JudgmentKind::Good);
    assert_eq!(classify(85), JudgmentKind::Poor);
    assert_eq!(classify(117), JudgmentKind::Poor);
    assert_eq!(classify(118), JudgmentKind::Miss);
}

#[test]
fn classify_large_positive_is_miss() {
    assert_eq!(classify(1000), JudgmentKind::Miss);
}

#[test]
fn rank_from_perfect_pct_perfect() {
    assert_eq!(Rank::from_perfect_pct(100.0), Rank::S);
    assert_eq!(Rank::from_perfect_pct(99.0), Rank::S);
    assert_eq!(Rank::from_perfect_pct(85.0), Rank::A);
    assert_eq!(Rank::from_perfect_pct(70.0), Rank::B);
    assert_eq!(Rank::from_perfect_pct(50.0), Rank::C);
    assert_eq!(Rank::from_perfect_pct(0.0), Rank::E);
}

#[test]
fn classify_symmetric_around_zero() {
    assert_eq!(classify(-1000), JudgmentKind::Miss);
    assert_eq!(classify(35), JudgmentKind::Great);
}
