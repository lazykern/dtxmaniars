//! DTXManiaNX XG-mode drum scoring (`nSkillMode == 1`).
//!
//! Pure port of the score block in
//! `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:1606-1658`
//! and the end-of-song bonuses in
//! `references/DTXmaniaNX/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs:191-215`.
//!
//! Formula (drums):
//! ```text
//! base = (1_000_000 - 500 * bonusChips) / (1275 + 50 * (maxCombo - 50))
//! delta = base * { Perfect: 1.0, Great: 0.5, Good: 0.2, else: 0 }
//! combo multiplier:
//!   combo < 50                    -> * combo
//!   combo == maxCombo (final)     -> * 1   (leaves room for exact-1e6 remainder)
//!   otherwise                     -> * 50
//! all-perfect last note           -> delta = 1_000_000 - currentScore
//! end bonus (0 miss & 0 poor):    -> EXC (+30000) if all perfect else FULL COMBO (+15000)
//! ```

use crate::JudgmentKind;

/// Maximum true score before end bonuses (`1_000_000`).
pub const TARGET_SCORE: i64 = 1_000_000;
/// Full-combo end bonus (0 Miss + 0 Poor), XG only. `CStagePerfDrumsScreen.cs:214`.
pub const FULL_COMBO_BONUS: i64 = 15_000;
/// Excellent end bonus (all Perfect), XG only. `CStagePerfDrumsScreen.cs:207`.
pub const EXCELLENT_BONUS: i64 = 30_000;

/// Per-Perfect base score unit.
///
/// Ref `CStagePerfCommonScreen.cs:1624`. The denominator can go non-positive for
/// very short charts (< ~25 notes); this intentionally preserves that NX quirk.
pub fn perfect_base(total_notes: u32, bonus_chips: u32) -> f32 {
    let denom = 1275.0_f32 + 50.0 * (total_notes as f32 - 50.0);
    (TARGET_SCORE as f32 - 500.0 * bonus_chips as f32) / denom
}

/// Score delta for one judged drum chip.
///
/// `combo_after` and `perfect_after` are the combo / Perfect counters *after*
/// this hit is folded in (DTXManiaNX increments the combo before the score
/// block runs). `current_score` is the true score before this hit.
///
/// Poor and Miss score nothing (`CStagePerfCommonScreen.cs:1613`).
pub fn xg_drum_score_delta(
    kind: JudgmentKind,
    combo_after: u32,
    perfect_after: u32,
    total_notes: u32,
    bonus_chips: u32,
    current_score: i64,
) -> i64 {
    if !matches!(
        kind,
        JudgmentKind::Perfect | JudgmentKind::Great | JudgmentKind::Good
    ) {
        return 0;
    }

    let base = perfect_base(total_notes, bonus_chips);

    // Perfect has the exact-1e6 remainder path when the whole chart is Perfect.
    let mut delta = match kind {
        JudgmentKind::Perfect => {
            if combo_after < total_notes {
                base
            } else if perfect_after >= total_notes {
                // Final note of an all-Perfect run: assign the remainder so the
                // true score lands exactly on TARGET_SCORE. Combo multiplier is
                // intentionally skipped (`CStagePerfCommonScreen.cs:1627-1630`).
                return TARGET_SCORE - current_score;
            } else {
                0.0
            }
        }
        JudgmentKind::Great => base * 0.5,
        JudgmentKind::Good => base * 0.2,
        _ => 0.0,
    };

    // Combo multiplier (`CStagePerfCommonScreen.cs:1644-1654`).
    if combo_after < 50 {
        delta *= combo_after as f32;
    } else if combo_after == total_notes || perfect_after == total_notes {
        // No multiply (final-note / all-perfect edge).
    } else {
        delta *= 50.0;
    }

    delta as i64
}

/// End-of-song bonus. Requires zero Miss and zero Poor. EXC (all Perfect) and
/// Full Combo are mutually exclusive; EXC wins.
pub fn xg_end_bonus(miss: u32, poor: u32, perfect: u32, total_notes: u32) -> i64 {
    if miss == 0 && poor == 0 {
        if perfect == total_notes {
            EXCELLENT_BONUS
        } else {
            FULL_COMBO_BONUS
        }
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_base_matches_reference_at_100_notes() {
        // base = 1e6 / (1275 + 50*(100-50)) = 1e6 / 3775 ≈ 264.9
        let b = perfect_base(100, 0);
        assert!((b - (1_000_000.0 / 3775.0)).abs() < 1e-3);
    }

    #[test]
    fn poor_and_miss_score_zero() {
        assert_eq!(xg_drum_score_delta(JudgmentKind::Poor, 10, 0, 100, 0, 0), 0);
        assert_eq!(xg_drum_score_delta(JudgmentKind::Miss, 10, 0, 100, 0, 0), 0);
    }

    #[test]
    fn great_is_half_of_perfect_unit() {
        let base = perfect_base(100, 0);
        // combo 10 → multiplier ×10.
        let p = xg_drum_score_delta(JudgmentKind::Perfect, 10, 10, 100, 0, 0);
        let g = xg_drum_score_delta(JudgmentKind::Great, 10, 0, 100, 0, 0);
        assert_eq!(p, (base * 10.0) as i64);
        assert_eq!(g, (base * 0.5 * 10.0) as i64);
    }

    #[test]
    fn combo_ramps_then_caps_at_50() {
        let base = perfect_base(200, 0);
        let at_49 = xg_drum_score_delta(JudgmentKind::Perfect, 49, 49, 200, 0, 0);
        let at_60 = xg_drum_score_delta(JudgmentKind::Perfect, 60, 60, 200, 0, 0);
        assert_eq!(at_49, (base * 49.0) as i64);
        assert_eq!(at_60, (base * 50.0) as i64);
    }

    #[test]
    fn all_perfect_run_reaches_exactly_one_million() {
        // Simulate an all-Perfect chart of 60 notes; final delta snaps to 1e6.
        let total = 60u32;
        let mut score = 0;
        for i in 1..=total {
            let d = xg_drum_score_delta(JudgmentKind::Perfect, i, i, total, 0, score);
            score += d;
        }
        assert_eq!(score, TARGET_SCORE, "got {score}");
    }

    #[test]
    fn end_bonus_exc_and_full_combo() {
        assert_eq!(xg_end_bonus(0, 0, 100, 100), EXCELLENT_BONUS);
        assert_eq!(xg_end_bonus(0, 0, 80, 100), FULL_COMBO_BONUS);
        assert_eq!(xg_end_bonus(1, 0, 99, 100), 0);
        assert_eq!(xg_end_bonus(0, 1, 99, 100), 0);
    }
}
