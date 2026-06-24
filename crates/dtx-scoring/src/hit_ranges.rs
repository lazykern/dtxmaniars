//! Per-difficulty hit ranges (Phase F3).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/STHitRanges.cs` (200 LOC)
//!
//! Each difficulty tier defines the timing windows (in ms) for the 5
//! judgment kinds (Perfect / Great / Good / Ok / Miss). The default
//! (BocuD "Normal") is 16/32/64/100/200ms; tighter tiers (Expert/Master)
//! shrink the windows, looser tiers (Easy) expand them.
//!
//! Stage entry reads `difficulty` from chart metadata and applies the
//! tier's windows via [`classify_with_ranges`].

use crate::JudgmentKind;

/// 5 difficulty tiers matching DTXManiaNX (Easy/Normal/Hard/Expert/Master).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    /// Easiest — wider timing windows, fewer miss penalties.
    Easy,
    /// Standard — default 16/32/64/100/200ms windows.
    Normal,
    /// Slightly tighter than Normal.
    Hard,
    /// Tighter still.
    Expert,
    /// Tightest — expert / master level.
    Master,
}

impl Difficulty {
    /// All 5 tiers in difficulty order.
    pub const fn all() -> [Self; 5] {
        [
            Self::Easy,
            Self::Normal,
            Self::Hard,
            Self::Expert,
            Self::Master,
        ]
    }

    /// Default tier if chart has no difficulty set.
    pub const fn default() -> Self {
        Self::Normal
    }

    /// Parse from a chart's `dlevel` integer (BocuD uses 0.0-5.0 floats).
    ///
    /// 0..=1 → Easy
    /// 2     → Normal
    /// 3     → Hard
    /// 4     → Expert
    /// 5+    → Master
    pub const fn from_dlevel(dlevel: u8) -> Self {
        match dlevel {
            0 | 1 => Self::Easy,
            2 => Self::Normal,
            3 => Self::Hard,
            4 => Self::Expert,
            _ => Self::Master,
        }
    }

    /// Display name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
            Self::Expert => "Expert",
            Self::Master => "Master",
        }
    }
}

/// Per-difficulty timing windows.
///
/// Mirrors `STHitRanges` (BocuD Core/STHitRanges.cs). Each field is the
/// *half-window* in ms (|delta| <= field → that judgment). Miss is the
/// threshold beyond which the judgment is Miss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRanges {
    /// Perfect window (default 16ms).
    pub perfect: i32,
    /// Great window (default 32ms).
    pub great: i32,
    /// Good window (default 64ms).
    pub good: i32,
    /// Ok window (default 100ms).
    pub ok: i32,
    /// Miss threshold (default 200ms). Beyond this, judgment is Miss.
    pub miss: i32,
}

impl Default for HitRanges {
    fn default() -> Self {
        Self::normal()
    }
}

impl HitRanges {
    /// Normal (default) timing windows.
    pub const fn normal() -> Self {
        Self {
            perfect: 16,
            great: 32,
            good: 64,
            ok: 100,
            miss: 200,
        }
    }

    /// Easy timing windows (~1.5x Normal).
    pub const fn easy() -> Self {
        Self {
            perfect: 24,
            great: 48,
            good: 96,
            ok: 150,
            miss: 300,
        }
    }

    /// Hard timing windows (~0.7x Normal).
    pub const fn hard() -> Self {
        Self {
            perfect: 11,
            great: 22,
            good: 44,
            ok: 70,
            miss: 140,
        }
    }

    /// Expert timing windows (~0.5x Normal).
    pub const fn expert() -> Self {
        Self {
            perfect: 8,
            great: 16,
            good: 32,
            ok: 50,
            miss: 100,
        }
    }

    /// Master timing windows (~0.3x Normal).
    pub const fn master() -> Self {
        Self {
            perfect: 5,
            great: 10,
            good: 20,
            ok: 30,
            miss: 60,
        }
    }

    /// Get the timing windows for a given difficulty tier.
    pub const fn for_difficulty(d: Difficulty) -> Self {
        match d {
            Difficulty::Easy => Self::easy(),
            Difficulty::Normal => Self::normal(),
            Difficulty::Hard => Self::hard(),
            Difficulty::Expert => Self::expert(),
            Difficulty::Master => Self::master(),
        }
    }

    /// Build HitRanges from explicit values. Useful for tests + custom configs.
    pub const fn new(perfect: i32, great: i32, good: i32, ok: i32, miss: i32) -> Self {
        Self {
            perfect,
            great,
            good,
            ok,
            miss,
        }
    }

    /// Half-window in ms for the given judgment.
    pub const fn window(self, kind: JudgmentKind) -> i32 {
        match kind {
            JudgmentKind::Perfect => self.perfect,
            JudgmentKind::Great => self.great,
            JudgmentKind::Good => self.good,
            JudgmentKind::Ok => self.ok,
            JudgmentKind::Miss => self.miss,
        }
    }
}

/// Classify a delta (ms) using a specific difficulty's hit ranges.
pub fn classify_with_ranges(delta_ms: i32, ranges: HitRanges) -> JudgmentKind {
    let abs = delta_ms.unsigned_abs();
    if abs <= ranges.perfect as u32 {
        JudgmentKind::Perfect
    } else if abs <= ranges.great as u32 {
        JudgmentKind::Great
    } else if abs <= ranges.good as u32 {
        JudgmentKind::Good
    } else if abs <= ranges.ok as u32 {
        JudgmentKind::Ok
    } else {
        JudgmentKind::Miss
    }
}

/// Classify a delta (ms) using a difficulty tier's hit ranges.
pub fn classify_with_difficulty(delta_ms: i32, difficulty: Difficulty) -> JudgmentKind {
    classify_with_ranges(delta_ms, HitRanges::for_difficulty(difficulty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_matches_default() {
        let n = HitRanges::normal();
        assert_eq!(n.perfect, 16);
        assert_eq!(n.great, 32);
        assert_eq!(n.good, 64);
        assert_eq!(n.ok, 100);
        assert_eq!(n.miss, 200);
    }

    #[test]
    fn easy_widens_all_windows() {
        let e = HitRanges::easy();
        let n = HitRanges::normal();
        assert!(e.perfect > n.perfect);
        assert!(e.great > n.great);
        assert!(e.good > n.good);
        assert!(e.ok > n.ok);
        assert!(e.miss > n.miss);
    }

    #[test]
    fn master_tightens_all_windows() {
        let m = HitRanges::master();
        let n = HitRanges::normal();
        assert!(m.perfect < n.perfect);
        assert!(m.great < n.great);
        assert!(m.good < n.good);
        assert!(m.ok < n.ok);
        assert!(m.miss < n.miss);
    }

    #[test]
    fn expert_between_hard_and_master() {
        let h = HitRanges::hard();
        let e = HitRanges::expert();
        let m = HitRanges::master();
        assert!(h.perfect > e.perfect);
        assert!(e.perfect > m.perfect);
    }

    #[test]
    fn difficulty_from_dlevel() {
        assert_eq!(Difficulty::from_dlevel(0), Difficulty::Easy);
        assert_eq!(Difficulty::from_dlevel(1), Difficulty::Easy);
        assert_eq!(Difficulty::from_dlevel(2), Difficulty::Normal);
        assert_eq!(Difficulty::from_dlevel(3), Difficulty::Hard);
        assert_eq!(Difficulty::from_dlevel(4), Difficulty::Expert);
        assert_eq!(Difficulty::from_dlevel(5), Difficulty::Master);
        assert_eq!(Difficulty::from_dlevel(99), Difficulty::Master);
    }

    #[test]
    fn difficulty_as_str() {
        assert_eq!(Difficulty::Easy.as_str(), "Easy");
        assert_eq!(Difficulty::Master.as_str(), "Master");
    }

    #[test]
    fn classify_with_normal_ranges() {
        let r = HitRanges::normal();
        assert_eq!(classify_with_ranges(0, r), JudgmentKind::Perfect);
        assert_eq!(classify_with_ranges(16, r), JudgmentKind::Perfect);
        assert_eq!(classify_with_ranges(17, r), JudgmentKind::Great);
        assert_eq!(classify_with_ranges(32, r), JudgmentKind::Great);
        assert_eq!(classify_with_ranges(33, r), JudgmentKind::Good);
        assert_eq!(classify_with_ranges(100, r), JudgmentKind::Ok);
        assert_eq!(classify_with_ranges(101, r), JudgmentKind::Miss);
        assert_eq!(classify_with_ranges(201, r), JudgmentKind::Miss);
    }

    #[test]
    fn classify_with_easy_ranges_wider() {
        // Easy: Perfect window is 24ms, so 20ms is Perfect under Easy,
        // Great under Normal.
        let r_easy = HitRanges::easy();
        let r_norm = HitRanges::normal();
        assert_eq!(classify_with_ranges(20, r_easy), JudgmentKind::Perfect);
        assert_eq!(classify_with_ranges(20, r_norm), JudgmentKind::Great);
    }

    #[test]
    fn classify_with_master_ranges_tighter() {
        // Master: Perfect window is 5ms, so 8ms is Great under Master,
        // Perfect under Normal.
        let r_mast = HitRanges::master();
        let r_norm = HitRanges::normal();
        assert_eq!(classify_with_ranges(8, r_mast), JudgmentKind::Great);
        assert_eq!(classify_with_ranges(8, r_norm), JudgmentKind::Perfect);
    }

    #[test]
    fn classify_handles_negative_deltas() {
        let r = HitRanges::normal();
        assert_eq!(classify_with_ranges(-16, r), JudgmentKind::Perfect);
        assert_eq!(classify_with_ranges(-17, r), JudgmentKind::Great);
        assert_eq!(classify_with_ranges(-201, r), JudgmentKind::Miss);
    }

    #[test]
    fn classify_with_difficulty_helper() {
        assert_eq!(
            classify_with_difficulty(8, Difficulty::Master),
            JudgmentKind::Great
        );
        assert_eq!(
            classify_with_difficulty(8, Difficulty::Normal),
            JudgmentKind::Perfect
        );
    }

    #[test]
    fn window_for_kind() {
        let r = HitRanges::normal();
        assert_eq!(r.window(JudgmentKind::Perfect), 16);
        assert_eq!(r.window(JudgmentKind::Great), 32);
        assert_eq!(r.window(JudgmentKind::Good), 64);
        assert_eq!(r.window(JudgmentKind::Ok), 100);
        assert_eq!(r.window(JudgmentKind::Miss), 200);
    }

    #[test]
    fn classify_perfect_at_zero() {
        let r = HitRanges::master();
        // ±0ms is always Perfect.
        assert_eq!(classify_with_ranges(0, r), JudgmentKind::Perfect);
    }

    #[test]
    fn difficulty_default_is_normal() {
        assert_eq!(Difficulty::default(), Difficulty::Normal);
    }

    #[test]
    fn difficulty_all_iteration() {
        assert_eq!(Difficulty::all().len(), 5);
    }
}
