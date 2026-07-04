//! Skill calculation — BocuD `CScoreIni.tCalculatePlayingSkill` port.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs:1363-1380`
//! (new formula) and `:1480-1531` (old formula + game skill).
//!
//! New formula (skill% in 0-100):
//! ```text
//! skill% = perfect% * 0.85 + great% * 0.30 + combo% * 0.15
//! ```
//!
//! Old formula (Drums rate, decimal 0-1):
//! ```text
//! rate = (perfect*0.8 + great*0.3 + combo*0.2) / total
//! skill% = rate * 100
//! ```
//!
//! Game skill (final number shown in HUD):
//! ```text
//! skill% × song_level × 0.33
//! ```

/// New skill formula (BocuD `tCalculatePlayingSkill`).
///
/// Returns skill in 0-100 scale. Caller multiplies by chart level × 0.33
/// to get the displayed "Skills by Song" number.
///
/// `total` = total drum chips. `max_combo` = maximum theoretical combo.
/// `b_auto_play` = if the play was assisted (skill multiplied by 0.5 in BocuD).
pub fn calculate_skill_new(
    perfect: u32,
    great: u32,
    good: u32,
    poor: u32,
    miss: u32,
    total: u32,
    max_combo: u32,
    b_auto_play: bool,
) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let _ = good;
    let _ = poor;
    let perfect_pct = 100.0 * perfect as f64 / total as f64;
    let great_pct = 100.0 * great as f64 / total as f64;
    let combo_pct = if total == (total - (perfect + great + good + poor + miss)) {
        0.0
    } else {
        100.0 * max_combo as f64 / total as f64
    };
    let ret = perfect_pct * 0.85 + great_pct * 0.30 + combo_pct * 0.15;
    if b_auto_play {
        ret * 0.5
    } else {
        ret
    }
}

/// Old skill formula (BocuD `tCalculatePlayingSkillOld`).
///
/// Drums-specific: `rate = (perfect*0.8 + great*0.3 + combo*0.2) / total`.
/// Returns skill in 0-1 scale; multiply by 100 for percentage display.
pub fn calculate_skill_old(
    perfect: u32,
    great: u32,
    max_combo: u32,
    total: u32,
    b_auto_play: bool,
) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let rate = (perfect as f64 * 0.8 + great as f64 * 0.3 + max_combo as f64 * 0.2) / total as f64;
    let ret = rate * 100.0;
    if b_auto_play {
        ret * 0.5
    } else {
        ret
    }
}

/// Final "Skills by Song" number (BocuD `tCalculateGameSkill`).
///
/// `skill_pct` = result of [`calculate_skill_new`] (0-100).
/// `db_level` = chart difficulty level (e.g., 8.20 for MASTER).
pub fn game_skill(skill_pct: f64, db_level: f64, b_auto_play: bool) -> f64 {
    let ret = db_level * skill_pct * 0.01 * 0.33;
    if b_auto_play {
        ret * 0.5
    } else {
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_skill_all_perfect_full_combo() {
        let s = calculate_skill_new(100, 0, 0, 0, 0, 100, 100, false);
        assert!((s - 100.0).abs() < 0.01);
    }

    #[test]
    fn new_skill_all_miss() {
        let s = calculate_skill_new(0, 0, 0, 0, 100, 100, 0, false);
        assert!(s.abs() < 0.01);
    }

    #[test]
    fn new_skill_mixed() {
        // 50P, 30G, 10GO, 5OK, 5Miss, max combo 95.
        // 50*0.85 + 30*0.30 + 95*0.15 = 42.5 + 9 + 14.25 = 65.75.
        let s = calculate_skill_new(50, 30, 10, 5, 5, 100, 95, false);
        assert!((s - 65.75).abs() < 0.01);
    }

    #[test]
    fn new_skill_zero_total() {
        assert!(calculate_skill_new(0, 0, 0, 0, 0, 0, 0, false).abs() < 0.01);
    }

    #[test]
    fn new_skill_autoplay_halves() {
        let s_human = calculate_skill_new(50, 30, 10, 5, 5, 100, 95, false);
        let s_auto = calculate_skill_new(50, 30, 10, 5, 5, 100, 95, true);
        assert!((s_human - s_auto * 2.0).abs() < 0.01);
    }

    #[test]
    fn old_skill_all_perfect() {
        let s = calculate_skill_old(100, 0, 100, 100, false);
        assert!((s - 100.0).abs() < 0.01);
    }

    #[test]
    fn old_skill_mixed() {
        let s = calculate_skill_old(50, 30, 95, 100, false);
        assert!((s - 68.0).abs() < 0.01);
    }

    #[test]
    fn game_skill_formula() {
        let s = game_skill(65.75, 8.20, false);
        assert!((s - 1.778).abs() < 0.01);
    }
}
