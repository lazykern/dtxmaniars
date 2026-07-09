//! Post-process transforms applied after DTX parse.
//!
//! Mirrors `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs`
//! post-load methods:
//! - `t旧仕様のドコドコチップを振り分ける` (assign-to-LBD)
//! - `tドコドコ仕様変更` (DKDK type)
//! - `tドラムのランダム化` (mirror / random / super / hyper / master / another)
//! - `tRandomizeDrumPedal` (pedal random)
//! - `t譜面仕様変更` (lane count)
//! - `tRandomizeGuitarAndBass` (gt/bs random)
//!
//! We only port [`apply_mirror`] for now — it's the smallest BocuD transform
//! and the most-requested feature. The other modes are documented in the
//! roadmap as future work (M14+).

use crate::channel::EChannel;
use crate::chart::Chip;

/// Randomization mode for post-load transforms. Port-side subset of BocuD's
/// `ERandomMode` (CDTX.cs:1823-1855).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RandomMode {
    /// No randomization (default).
    #[default]
    Off,
    /// Mirror the lane layout left↔right. Drums only. BocuD mirror branch
    /// (`tドラムのランダム化` line 1887-1896).
    Mirror,
}

/// Apply the selected random transform in-place. Currently only `Mirror` does
/// anything; `Off` is a no-op (kept for forward-compat with config wiring).
///
/// Mirror rule (BocuD `tミラーチップのチャンネルを指定する`, CDTX.cs:2255):
/// drums lanes other than BD swap symmetrically around the centre. HH/SD/HT/LT/
/// CY/RD/HHO ↔ LC/LP/LBD (left vs right group), but BD stays as BD.
pub fn apply_random(chips: &mut [Chip], mode: RandomMode) {
    match mode {
        RandomMode::Off => {}
        RandomMode::Mirror => apply_mirror(chips),
    }
}

/// Mirror drums lane assignment in-place.
///
/// Maps each drum chip channel to its mirror partner:
/// - `HiHatClose` ↔ `LeftCymbal` (HH family ↔ LC family)
/// - `HiHatOpen`  ↔ `LeftCymbal` (open HH drops into LC pool; BocuD picks
///   `LeftCymbal` first because HHO is left-foot-pedal-adjacent in mirror)
/// - `Snare`      ↔ `HighTom` (right-hand inner ↔ left-hand inner)
/// - `LowTom`     ↔ `FloorTom` (low ↔ low on the opposite side)
/// - `Cymbal`     ↔ `RideCymbal` (CY ↔ RD swap)
/// - `BassDrum`, `LeftBassDrum`, `LeftPedal`, `DrumsFillin` stay put
///
/// Other chip types (BGM, SE, BGA, BPM, …) are left untouched.
pub fn apply_mirror(chips: &mut [Chip]) {
    for chip in chips.iter_mut() {
        chip.channel = match chip.channel {
            EChannel::HiHatClose => EChannel::LeftCymbal,
            EChannel::LeftCymbal => EChannel::HiHatClose,
            EChannel::HiHatOpen => EChannel::LeftCymbal,
            EChannel::Snare => EChannel::HighTom,
            EChannel::HighTom => EChannel::Snare,
            EChannel::LowTom => EChannel::FloorTom,
            EChannel::FloorTom => EChannel::LowTom,
            EChannel::Cymbal => EChannel::RideCymbal,
            EChannel::RideCymbal => EChannel::Cymbal,
            // BD / LBD / LP / fillin stay put (foot-side, no mirror partner).
            other => other,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip(ch: EChannel) -> Chip {
        Chip::new(0, ch, 1.0)
    }

    #[test]
    fn mirror_swaps_hh_and_lc() {
        let mut chips = vec![chip(EChannel::HiHatClose), chip(EChannel::LeftCymbal)];
        apply_mirror(&mut chips);
        assert_eq!(chips[0].channel, EChannel::LeftCymbal);
        assert_eq!(chips[1].channel, EChannel::HiHatClose);
    }

    #[test]
    fn mirror_keeps_bass_drum() {
        let mut chips = vec![chip(EChannel::BassDrum), chip(EChannel::LeftBassDrum)];
        apply_mirror(&mut chips);
        assert_eq!(chips[0].channel, EChannel::BassDrum);
        assert_eq!(chips[1].channel, EChannel::LeftBassDrum);
    }

    #[test]
    fn mirror_swaps_snare_and_high_tom() {
        let mut chips = vec![chip(EChannel::Snare), chip(EChannel::HighTom)];
        apply_mirror(&mut chips);
        assert_eq!(chips[0].channel, EChannel::HighTom);
        assert_eq!(chips[1].channel, EChannel::Snare);
    }

    #[test]
    fn mirror_swaps_cymbal_and_ride() {
        let mut chips = vec![chip(EChannel::Cymbal), chip(EChannel::RideCymbal)];
        apply_mirror(&mut chips);
        assert_eq!(chips[0].channel, EChannel::RideCymbal);
        assert_eq!(chips[1].channel, EChannel::Cymbal);
    }

    #[test]
    fn mirror_leaves_bgm_and_se_alone() {
        let mut chips = vec![
            chip(EChannel::BGM),
            chip(EChannel::SE01),
            chip(EChannel::BPM),
        ];
        apply_mirror(&mut chips);
        assert_eq!(chips[0].channel, EChannel::BGM);
        assert_eq!(chips[1].channel, EChannel::SE01);
        assert_eq!(chips[2].channel, EChannel::BPM);
    }

    #[test]
    fn apply_random_off_is_noop() {
        let mut chips = vec![chip(EChannel::HiHatClose)];
        apply_random(&mut chips, RandomMode::Off);
        assert_eq!(chips[0].channel, EChannel::HiHatClose);
    }

    #[test]
    fn apply_random_mirror_invokes_mirror() {
        let mut chips = vec![chip(EChannel::HiHatClose)];
        apply_random(&mut chips, RandomMode::Mirror);
        assert_eq!(chips[0].channel, EChannel::LeftCymbal);
    }
}
