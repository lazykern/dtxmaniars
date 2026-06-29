//! BGA / AVI / Sound trigger pipeline (Phase F7).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/Common/CStagePerfCommonScreen.cs`
//! `OnChip発声時` method — chip → BGA / AVI / WAV cue.
//!
//! Routes a chip's channel + value to the appropriate asset cue:
//! - BGA layer channels (1, 2, 3..8) → BgaCue(layer=N, idx=value)
//! - Movie / MovieFull channels → AviCue(idx=value)
//! - SE channels (0x61-0x65) → WavCue(kind=SoundEffect, idx=value)
//! - BGM channel → WavCue(kind=Bgm, idx=value)
//!
//! The renderer / audio system consumes the cue and dispatches to
//! Bevy AssetServer / kira. Data flow only here; no rendering.

use crate::assets::DtxAssets;
use crate::channel::EChannel;
use crate::chart::Chip;

/// Discriminated trigger event.
#[derive(Debug, Clone, PartialEq)]
pub enum Trigger {
    /// BGA image trigger.
    Bga {
        /// 1..=8
        layer: u8,
        /// BgaEntry idx (or 0 for clear).
        idx: u32,
        /// Optional depth (1.0 = on top, 0.0 = hidden).
        depth: f32,
    },
    /// AVI / movie trigger.
    Avi {
        /// AviEntry idx.
        idx: u32,
        /// If true, full-screen. If false, layer-1 size.
        full_screen: bool,
    },
    /// WAV sound effect / BGM.
    Wav {
        /// True if BGM, false if SE.
        is_bgm: bool,
        /// WavEntry idx.
        idx: u32,
        /// Volume 0..100 (BocuD n音量).
        volume: u8,
    },
}

/// Compute the trigger event for a chip, given the asset registry.
///
/// Returns `None` for non-trigger channels (drums, guitar, BPM, etc.).
pub fn trigger_for(chip: &Chip, assets: &DtxAssets) -> Option<Trigger> {
    let idx = chip.value as u32;
    let volume = (chip.value as u8).min(100);
    match chip.channel {
        EChannel::BGALayer1 => Some(Trigger::Bga {
            layer: 1,
            idx,
            depth: 1.0,
        }),
        EChannel::BGALayer2 => Some(Trigger::Bga {
            layer: 2,
            idx,
            depth: 0.5,
        }),
        EChannel::BGALayer3 => Some(Trigger::Bga {
            layer: 3,
            idx,
            depth: 0.75,
        }),
        EChannel::BGALayer4 => Some(Trigger::Bga {
            layer: 4,
            idx,
            depth: 0.25,
        }),
        EChannel::BGALayer5 => Some(Trigger::Bga {
            layer: 5,
            idx,
            depth: 0.20,
        }),
        EChannel::BGALayer6 => Some(Trigger::Bga {
            layer: 6,
            idx,
            depth: 0.15,
        }),
        EChannel::BGALayer7 => Some(Trigger::Bga {
            layer: 7,
            idx,
            depth: 0.10,
        }),
        EChannel::BGALayer8 => Some(Trigger::Bga {
            layer: 8,
            idx,
            depth: 0.05,
        }),
        EChannel::Movie => Some(Trigger::Avi {
            idx,
            full_screen: false,
        }),
        EChannel::MovieFull => Some(Trigger::Avi {
            idx,
            full_screen: true,
        }),
        EChannel::BGM => {
            // BGM: chip.value is the WAV idx; lookup to confirm exists.
            if assets.wav.get(idx).is_some() {
                Some(Trigger::Wav {
                    is_bgm: true,
                    idx,
                    volume,
                })
            } else {
                None
            }
        }
        EChannel::SE01 | EChannel::SE02 | EChannel::SE03 | EChannel::SE04 | EChannel::SE05 => {
            if assets.wav.get(idx).is_some() {
                Some(Trigger::Wav {
                    is_bgm: false,
                    idx,
                    volume,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Compute the trigger and resolve the asset path (filename) for it.
///
/// Returns `(Trigger, Option<String>)` where the String is the asset
/// path if found in the registry, or None if the asset is missing.
pub fn trigger_resolved(chip: &Chip, assets: &DtxAssets) -> (Option<Trigger>, Option<String>) {
    let trigger = trigger_for(chip, assets);
    let path = match &trigger {
        Some(Trigger::Bga { idx, .. }) => assets
            .bmp
            .get(*idx)
            .map(|s| s.to_string())
            .or_else(|| assets.bga.get(*idx).map(|s| s.to_string())),
        Some(Trigger::Avi { idx, .. }) => assets.avi.get(*idx).map(|s| s.to_string()),
        Some(Trigger::Wav { idx, .. }) => assets.wav.get(*idx).map(|s| s.to_string()),
        None => None,
    };
    (trigger, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::DtxAssets;

    fn chip_with(channel: EChannel, value: f32) -> Chip {
        Chip {
            measure: 0,
            channel,
            value,
            wav_slot: 0,
        }
    }

    fn empty_assets() -> DtxAssets {
        DtxAssets::default()
    }

    fn assets_with_wav_bmp_avi() -> DtxAssets {
        let mut a = DtxAssets::default();
        a.wav.insert(1, "kick.wav".into());
        a.bmp.insert(2, "bg.bmp".into());
        a.bga.insert(3, "movie.bga".into());
        a.avi.insert(4, "intro.avi".into());
        a
    }

    #[test]
    fn bga_layer1_returns_bga_trigger() {
        let chip = chip_with(EChannel::BGALayer1, 2.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(
            t,
            Some(Trigger::Bga {
                layer: 1,
                idx: 2,
                depth: 1.0
            })
        );
    }

    #[test]
    fn bga_layer3_returns_layer_3() {
        let chip = chip_with(EChannel::BGALayer3, 5.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(
            t,
            Some(Trigger::Bga {
                layer: 3,
                idx: 5,
                depth: 0.75
            })
        );
    }

    #[test]
    fn bga_layer8_has_lowest_depth() {
        let chip = chip_with(EChannel::BGALayer8, 1.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(
            t,
            Some(Trigger::Bga {
                layer: 8,
                idx: 1,
                depth: 0.05
            })
        );
    }

    #[test]
    fn movie_returns_avi_trigger() {
        let chip = chip_with(EChannel::Movie, 4.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(
            t,
            Some(Trigger::Avi {
                idx: 4,
                full_screen: false
            })
        );
    }

    #[test]
    fn movie_full_returns_full_screen() {
        let chip = chip_with(EChannel::MovieFull, 4.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(
            t,
            Some(Trigger::Avi {
                idx: 4,
                full_screen: true
            })
        );
    }

    #[test]
    fn bgm_returns_wav_when_registered() {
        let chip = chip_with(EChannel::BGM, 1.0);
        let t = trigger_for(&chip, &assets_with_wav_bmp_avi());
        assert_eq!(
            t,
            Some(Trigger::Wav {
                is_bgm: true,
                idx: 1,
                volume: 1
            })
        );
    }

    #[test]
    fn bgm_returns_none_when_missing() {
        let chip = chip_with(EChannel::BGM, 99.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(t, None);
    }

    #[test]
    fn se01_returns_wav_se() {
        let chip = chip_with(EChannel::SE01, 1.0);
        let t = trigger_for(&chip, &assets_with_wav_bmp_avi());
        assert_eq!(
            t,
            Some(Trigger::Wav {
                is_bgm: false,
                idx: 1,
                volume: 1
            })
        );
    }

    #[test]
    fn drum_chip_returns_none() {
        let chip = chip_with(EChannel::BassDrum, 0.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(t, None);
    }

    #[test]
    fn bpm_chip_returns_none() {
        let chip = chip_with(EChannel::BPM, 120.0);
        let t = trigger_for(&chip, &empty_assets());
        assert_eq!(t, None);
    }

    #[test]
    fn trigger_resolved_returns_path() {
        let chip = chip_with(EChannel::BGM, 1.0);
        let (t, path) = trigger_resolved(&chip, &assets_with_wav_bmp_avi());
        assert!(t.is_some());
        assert_eq!(path, Some("kick.wav".into()));
    }

    #[test]
    fn trigger_resolved_bga_finds_path() {
        let chip = chip_with(EChannel::BGALayer1, 2.0);
        let (t, path) = trigger_resolved(&chip, &assets_with_wav_bmp_avi());
        assert!(t.is_some());
        assert_eq!(path, Some("bg.bmp".into()));
    }

    #[test]
    fn trigger_resolved_bga_falls_back_to_bga_list() {
        // Asset 3 only exists in `bgas` (legacy BGA list).
        let chip = chip_with(EChannel::BGALayer2, 3.0);
        let (t, path) = trigger_resolved(&chip, &assets_with_wav_bmp_avi());
        assert!(t.is_some());
        assert_eq!(path, Some("movie.bga".into()));
    }

    #[test]
    fn trigger_resolved_avi_finds_path() {
        let chip = chip_with(EChannel::Movie, 4.0);
        let (t, path) = trigger_resolved(&chip, &assets_with_wav_bmp_avi());
        assert!(t.is_some());
        assert_eq!(path, Some("intro.avi".into()));
    }

    #[test]
    fn trigger_resolved_returns_none_for_drum() {
        let chip = chip_with(EChannel::Snare, 0.0);
        let (t, path) = trigger_resolved(&chip, &assets_with_wav_bmp_avi());
        assert_eq!(t, None);
        assert_eq!(path, None);
    }

    #[test]
    fn volume_clamped_to_100() {
        // chip.value > 100 should still produce volume = 100.
        let mut assets = assets_with_wav_bmp_avi();
        assets.wav.insert(250, "loud.wav".into());
        let chip = chip_with(EChannel::BGM, 250.0);
        let t = trigger_for(&chip, &assets);
        if let Some(Trigger::Wav { volume, .. }) = t {
            assert!(volume <= 100, "volume should be clamped");
            assert_eq!(volume, 100);
        } else {
            panic!("expected Wav trigger");
        }
    }
}
