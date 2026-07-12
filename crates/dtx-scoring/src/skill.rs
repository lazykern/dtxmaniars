//! DTXManiaNX XG performance and song-skill calculations.
//!
//! Reference: `CScoreIni.cs:1335-1380,1542-1576`.

/// Drum auto-play configuration used for NX's performance-skill adjustment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrumAutoPlay {
    /// Every drum lane is auto-played.
    pub all_drums: bool,
    /// Bass-drum pedal is auto-played.
    pub bass_drum: bool,
    /// Left-pedal lane is auto-played.
    pub left_pedal: bool,
    /// Left-bass-drum lane is auto-played.
    pub left_bass_drum: bool,
}

/// NX's performance skill / completion rate (0..100).
// Ports the NX formula, which takes every judgment count plus the autoplay
// mask — grouping them into a struct would obscure the reference mapping.
#[allow(clippy::too_many_arguments)]
pub fn drum_performance_skill(
    total: u32,
    perfect: u32,
    great: u32,
    good: u32,
    poor: u32,
    miss: u32,
    max_combo: u32,
    autoplay: DrumAutoPlay,
) -> f64 {
    if total == 0 {
        return 0.0;
    }

    let judged = perfect + great + good + poor + miss;
    let combo_pct = if judged == 0 {
        0.0
    } else {
        100.0 * max_combo as f64 / total as f64
    };
    let rate = 100.0 * perfect as f64 / total as f64 * 0.85
        + 100.0 * great as f64 / total as f64 * 0.35
        + combo_pct * 0.15;
    rate * drum_autoplay_multiplier(autoplay)
}

/// NX's per-song skill contribution: completion rate × chart level × 0.20.
pub fn drum_song_skill(chart_level: f64, performance_skill: f64, all_drums_auto: bool) -> f64 {
    if all_drums_auto {
        0.0
    } else {
        performance_skill * chart_level * 0.2
    }
}

fn drum_autoplay_multiplier(autoplay: DrumAutoPlay) -> f64 {
    if autoplay.all_drums {
        return 1.0;
    }
    if autoplay.bass_drum && autoplay.left_pedal && autoplay.left_bass_drum {
        0.25
    } else if autoplay.bass_drum || autoplay.left_pedal || autoplay.left_bass_drum {
        0.5
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_perfect_full_combo_is_one_hundred_percent() {
        assert_eq!(
            drum_performance_skill(100, 100, 0, 0, 0, 0, 100, DrumAutoPlay::default()),
            100.0
        );
    }

    #[test]
    fn greats_are_not_full_performance_skill() {
        assert_eq!(
            drum_performance_skill(100, 0, 100, 0, 0, 0, 100, DrumAutoPlay::default()),
            50.0
        );
    }

    #[test]
    fn partial_pedal_autoplay_halves_skill_once() {
        let manual = drum_performance_skill(100, 100, 0, 0, 0, 0, 100, DrumAutoPlay::default());
        let assisted = drum_performance_skill(
            100,
            100,
            0,
            0,
            0,
            0,
            100,
            DrumAutoPlay {
                bass_drum: true,
                ..DrumAutoPlay::default()
            },
        );
        assert_eq!(assisted, manual * 0.5);
    }

    #[test]
    fn all_drums_auto_has_no_song_skill() {
        assert_eq!(drum_song_skill(9.8, 100.0, true), 0.0);
    }
}
