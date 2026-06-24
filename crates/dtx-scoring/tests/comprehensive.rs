//! Comprehensive tests for dtx-scoring — judgment edge cases.

use dtx_scoring::{classify, JudgmentKind, Rank};

#[test]
fn classify_zero_is_perfect() {
    assert_eq!(classify(0), JudgmentKind::Perfect);
}

#[test]
fn classify_small_positive_is_perfect() {
    assert_eq!(classify(10), JudgmentKind::Perfect);
    assert_eq!(classify(16), JudgmentKind::Perfect);
}

#[test]
fn classify_at_perfect_boundary_is_great() {
    assert_eq!(classify(17), JudgmentKind::Great);
}

#[test]
fn classify_at_great_boundary_is_good() {
    assert_eq!(classify(33), JudgmentKind::Good);
}

#[test]
fn classify_at_good_boundary_is_ok() {
    assert_eq!(classify(65), JudgmentKind::Ok);
}

#[test]
fn classify_at_ok_boundary_is_miss() {
    assert_eq!(classify(129), JudgmentKind::Miss);
    assert_eq!(classify(200), JudgmentKind::Miss);
}

#[test]
fn classify_negative_is_miss() {
    assert_eq!(classify(-1000), JudgmentKind::Miss);
    assert_eq!(classify(-100), JudgmentKind::Ok);
}

#[test]
fn classify_large_positive_is_miss() {
    assert_eq!(classify(1000), JudgmentKind::Miss);
}

#[test]
fn judgment_kind_equality() {
    assert_eq!(JudgmentKind::Perfect, JudgmentKind::Perfect);
    assert_ne!(JudgmentKind::Perfect, JudgmentKind::Miss);
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
fn rank_equality_and_ordering() {
    assert_eq!(Rank::S, Rank::S);
    assert_ne!(Rank::S, Rank::E);
}

#[test]

#[test]
fn classify_symmetric_around_zero() {
    assert_eq!(classify(-1000), JudgmentKind::Miss);
    assert_eq!(classify(17), JudgmentKind::Great);
}
