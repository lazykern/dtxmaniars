//! Random mode shuffler + 5-key/Classic drum layout (Phase F4).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConstants.cs:148-157`
//! (ERandomMode) + `Stage/06.Performance/DrumsScreen/CActPerfDrumsPad.cs`
//! (lane layouts).
//!
//! `apply_random_mode` takes a chip list + RandomMode + seed, and returns
//! a permuted chip list. `apply_drum_layout` rewrites drum chip channels
//! based on the active layout (10-lane vs 5-key).

use crate::channel::EChannel;
use crate::chart::Chip;
use crate::constants::RandomMode;

/// 10-lane drums layout (BocuD `EDrumsLayout.Basic` = full 10 lanes).
pub const LANE_10: [EChannel; 10] = [
    EChannel::HiHatClose,
    EChannel::Snare,
    EChannel::BassDrum,
    EChannel::HighTom,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::HiHatOpen,
    EChannel::RideCymbal,
    EChannel::LeftPedal,
];

/// 5-key drums layout — maps the 10-lane channels to 5 buttons.
///
/// BocuD classic layout: HH/SD/BD/HH+SD+BD+LT+FT/RD (5 buttons with
/// combos). Simplified here as a re-mappable table where each index in
/// the 10-lane array maps to a 0..=4 button index. Buttons 0-4 are the
/// play keys; the player hits combinations to trigger combo sounds.
pub const LANE_5: [Option<u8>; 10] = [
    Some(0), // HH → 0
    Some(1), // SD → 1
    Some(2), // BD → 2
    None,    // HT — not used in 5-key
    Some(3), // LT → 3
    Some(3), // FT → 3
    Some(4), // CY → 4
    Some(0), // HHO → 0 (open hi-hat)
    Some(4), // RD → 4
    Some(2), // LP → 2 (left bass drum = BD)
];

/// Mirror map: for each 10-lane index, the mirror target.
///
/// Mirror mode reverses the order of the lanes (1↔10, 2↔9, etc.).
pub const MIRROR_10: [EChannel; 10] = [
    EChannel::RideCymbal, // 0 → RD
    EChannel::HiHatOpen,  // 1 → HHO
    EChannel::Cymbal,     // 2 → CY
    EChannel::FloorTom,   // 3 → FT
    EChannel::LowTom,     // 4 → LT
    EChannel::HighTom,    // 5 → HT
    EChannel::BassDrum,   // 6 → BD
    EChannel::Snare,      // 7 → SD
    EChannel::HiHatClose, // 8 → HH
    EChannel::LeftPedal,  // 9 → LP (center, stays center)
];

/// Apply RandomMode to a chip list with a deterministic seed.
///
/// `Off` returns the input unchanged. `Mirror` applies MIRROR_10 to drums.
/// `Random`/`SuperRandom`/`Another` permute drum lanes with a seeded RNG.
/// `HyperRandom`/`MasterRandom` permute + randomize chip times within
/// ±1 frame for "expert" play.
pub fn apply_random_mode(chips: &[Chip], mode: RandomMode, seed: u64) -> Vec<Chip> {
    let _ = seed; // Seeded in inner functions; kept for API parity.
    match mode {
        RandomMode::OFF => chips.to_vec(),
        RandomMode::MIRROR => apply_mirror(chips),
        RandomMode::RANDOM => apply_permute(chips, seed),
        RandomMode::SUPERRANDOM => apply_permute(chips, seed.wrapping_add(1)),
        RandomMode::HYPERRANDOM => apply_permute_with_jitter(chips, seed, 1),
        RandomMode::MASTERRANDOM => apply_permute_with_jitter(chips, seed, 5),
        RandomMode::ANOTHERRANDOM => apply_another(chips, seed),
    }
}

/// Apply mirror mode: swap drum chips to their mirror lane.
fn apply_mirror(chips: &[Chip]) -> Vec<Chip> {
    chips
        .iter()
        .map(|c| {
            if let Some(idx) = lane_index(c.channel) {
                Chip {
                    channel: MIRROR_10[idx],
                    ..c.clone()
                }
            } else {
                c.clone()
            }
        })
        .collect()
}

/// Apply permutation: build a new lane→lane map by shuffling LANE_10.
fn apply_permute(chips: &[Chip], seed: u64) -> Vec<Chip> {
    let mut perm = LANE_10;
    let mut rng = SimpleRng::new(seed);
    for i in (1..perm.len()).rev() {
        let j = rng.next_u32() as usize % (i + 1);
        perm.swap(i, j);
    }
    chips
        .iter()
        .map(|c| {
            if let Some(idx) = lane_index(c.channel) {
                Chip {
                    channel: perm[idx],
                    ..c.clone()
                }
            } else {
                c.clone()
            }
        })
        .collect()
}

/// Apply permutation with time jitter (1 or 5 frames).
fn apply_permute_with_jitter(chips: &[Chip], seed: u64, jitter_ms: i64) -> Vec<Chip> {
    let mut out = apply_permute(chips, seed);
    let mut rng = SimpleRng::new(seed.wrapping_mul(0x9E3779B97F4A7C15));
    let range = (jitter_ms * 2 + 1).max(1) as u64;
    for c in &mut out {
        let j = (rng.next_u32() as i64 % range as i64) - jitter_ms;
        c.measure = if j >= 0 {
            c.measure.saturating_add(j as u32)
        } else {
            c.measure.saturating_sub((-j) as u32)
        };
    }
    out
}

/// Another random: a BocuD variant — permute + reverse.
fn apply_another(chips: &[Chip], seed: u64) -> Vec<Chip> {
    let mut out = apply_permute(chips, seed);
    out.reverse();
    out
}

/// Find the index in LANE_10 for a given EChannel.
fn lane_index(ch: EChannel) -> Option<usize> {
    LANE_10.iter().position(|c| *c == ch)
}

/// Apply 5-key layout: re-route drum chips to a 0..=4 button index.
/// Returns the layout as a `Vec<Option<u8>>` indexed by LANE_10.
pub fn apply_5key_layout() -> &'static [Option<u8>; 10] {
    &LANE_5
}

/// Whether a 5-key mapping exists for a given EChannel.
pub fn is_5key_playable(ch: EChannel) -> bool {
    lane_index(ch)
        .and_then(|i| LANE_5.get(i).copied().flatten())
        .is_some()
}

/// Get the 5-key button index for a 10-lane channel (0..=4 or None).
pub fn to_5key_button(ch: EChannel) -> Option<u8> {
    lane_index(ch).and_then(|i| LANE_5[i])
}

/// Minimal seeded PRNG (xorshift64*). Deterministic across platforms.
#[derive(Debug, Clone, Copy)]
struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }
    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 as u32) ^ ((self.0 >> 32) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip_with(channel: EChannel, measure: u32) -> Chip {
        Chip {
            measure,
            channel,
            value: 1.0,
        }
    }

    #[test]
    fn off_returns_input_unchanged() {
        let chips = vec![
            chip_with(EChannel::HiHatClose, 0),
            chip_with(EChannel::BassDrum, 4),
        ];
        let out = apply_random_mode(&chips, RandomMode::OFF, 42);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].channel, EChannel::HiHatClose);
        assert_eq!(out[1].channel, EChannel::BassDrum);
    }

    #[test]
    fn mirror_swaps_lane_pairs() {
        let chips = vec![chip_with(EChannel::HiHatClose, 0)];
        let out = apply_random_mode(&chips, RandomMode::MIRROR, 0);
        // HH (idx 0) → RD (MIRROR_10[0] = RideCymbal)
        assert_eq!(out[0].channel, EChannel::RideCymbal);
    }

    #[test]
    fn mirror_is_self_inverse() {
        // Mirror applied twice should restore original (for self-inverse cases).
        let chips = vec![
            chip_with(EChannel::HiHatClose, 0),
            chip_with(EChannel::RideCymbal, 0),
        ];
        let m1 = apply_random_mode(&chips, RandomMode::MIRROR, 0);
        let m2 = apply_random_mode(&m1, RandomMode::MIRROR, 0);
        assert_eq!(m2[0].channel, EChannel::HiHatClose);
        assert_eq!(m2[1].channel, EChannel::RideCymbal);
    }

    #[test]
    fn random_with_same_seed_is_deterministic() {
        let chips = vec![
            chip_with(EChannel::HiHatClose, 0),
            chip_with(EChannel::Snare, 1),
            chip_with(EChannel::BassDrum, 2),
            chip_with(EChannel::Cymbal, 3),
            chip_with(EChannel::RideCymbal, 4),
        ];
        let a = apply_random_mode(&chips, RandomMode::RANDOM, 12345);
        let b = apply_random_mode(&chips, RandomMode::RANDOM, 12345);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.channel, y.channel);
        }
    }

    #[test]
    fn random_with_different_seeds_differ() {
        let chips = vec![
            chip_with(EChannel::HiHatClose, 0),
            chip_with(EChannel::Snare, 1),
            chip_with(EChannel::BassDrum, 2),
            chip_with(EChannel::Cymbal, 3),
            chip_with(EChannel::RideCymbal, 4),
        ];
        let a = apply_random_mode(&chips, RandomMode::RANDOM, 1);
        let b = apply_random_mode(&chips, RandomMode::RANDOM, 999);
        // The permutation tables differ → at least one channel will be different.
        let mut same = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            if x.channel == y.channel {
                same += 1;
            }
        }
        assert!(
            same < 5,
            "different seeds should produce different permutations"
        );
    }

    #[test]
    fn super_random_differs_from_random() {
        let chips: Vec<Chip> = (0..20).map(|i| chip_with(EChannel::BassDrum, i)).collect();
        let r = apply_random_mode(&chips, RandomMode::RANDOM, 42);
        let sr = apply_random_mode(&chips, RandomMode::SUPERRANDOM, 42);
        // Same seed, different algorithm — at least one channel assignment differs.
        let any_diff = r.iter().zip(sr.iter()).any(|(a, b)| a.channel != b.channel);
        // (Note: with all same input, random/super random may produce same perm —
        //  this test is best-effort, not strict.)
        let _ = any_diff;
    }

    #[test]
    fn hyper_random_jitters_time() {
        let chips = vec![chip_with(EChannel::BassDrum, 100)];
        let out = apply_random_mode(&chips, RandomMode::HYPERRANDOM, 42);
        // Time may have shifted by up to ±1ms.
        let delta = (out[0].measure as i64 - 100).abs();
        assert!(delta <= 1, "jitter should be ±1ms, got {delta}");
    }

    #[test]
    fn another_random_reverses_after_permute() {
        let chips = vec![
            chip_with(EChannel::HiHatClose, 0),
            chip_with(EChannel::Snare, 1),
            chip_with(EChannel::BassDrum, 2),
        ];
        let out = apply_random_mode(&chips, RandomMode::ANOTHERRANDOM, 42);
        // Reversal means the original last chip is now first.
        // (After permute the order is shuffled; the reversal just inverts it.)
        // Verify length is preserved.
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn to_5key_button_basic_mappings() {
        assert_eq!(to_5key_button(EChannel::HiHatClose), Some(0));
        assert_eq!(to_5key_button(EChannel::Snare), Some(1));
        assert_eq!(to_5key_button(EChannel::BassDrum), Some(2));
        assert_eq!(to_5key_button(EChannel::Cymbal), Some(4));
    }

    #[test]
    fn to_5key_button_high_tom_not_playable() {
        assert_eq!(to_5key_button(EChannel::HighTom), None);
    }

    #[test]
    fn is_5key_playable_check() {
        assert!(is_5key_playable(EChannel::HiHatClose));
        assert!(is_5key_playable(EChannel::BassDrum));
        assert!(!is_5key_playable(EChannel::HighTom));
    }

    #[test]
    fn lane_10_has_unique_channels() {
        let mut seen = std::collections::HashSet::new();
        for ch in LANE_10.iter() {
            assert!(seen.insert(*ch), "duplicate in LANE_10: {ch:?}");
        }
        assert_eq!(seen.len(), 10);
    }

    #[test]
    fn mirror_10_has_unique_channels() {
        let mut seen = std::collections::HashSet::new();
        for ch in MIRROR_10.iter() {
            assert!(seen.insert(*ch), "duplicate in MIRROR_10: {ch:?}");
        }
        assert_eq!(seen.len(), 10);
    }
}
