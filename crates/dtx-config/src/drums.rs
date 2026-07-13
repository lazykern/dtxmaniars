//! Drums gameplay config — grouping, cymbal-free, hit-sound priority.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Core/Config/CConfigIni.cs`
//! Reference: `references/DTXmaniaNX/DTXMania/Core/CConstants.cs:5-28`

use serde::{Deserialize, Serialize};

/// CY/RD grouping (`ECYGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CyGroup {
    /// 打ち分ける — CY pad hits CY chips only; RD pad hits RD only.
    #[default]
    Separate,
    /// 共通 — CY and RD pads share CY/RD chips.
    Common,
}

impl CyGroup {
    pub fn all() -> [Self; 2] {
        [Self::Separate, Self::Common]
    }
}

/// HH/HC/HHO/LC grouping (`EHHGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HhGroup {
    /// 全部打ち分ける
    #[default]
    SeparateAll,
    /// ハイハットのみ打ち分ける (HC+LC vs HO)
    HhAndLc,
    /// 左シンバルのみ打ち分ける (HC+HO vs LC)
    HhAndHo,
    /// 全部共通
    CommonAll,
}

impl HhGroup {
    pub fn all() -> [Self; 4] {
        [
            Self::SeparateAll,
            Self::HhAndLc,
            Self::HhAndHo,
            Self::CommonAll,
        ]
    }
}

/// LT/FT grouping (`EFTGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FtGroup {
    #[default]
    Separate,
    Common,
}

impl FtGroup {
    pub fn all() -> [Self; 2] {
        [Self::Separate, Self::Common]
    }
}

/// BD/LP/LBD grouping (`EBDGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BdGroup {
    /// LP | LBD | BD all separate.
    #[default]
    Separate,
    /// LP | (LBD & BD).
    BdAndLbd,
    /// LP & (LBD | BD) — pedals only split.
    PedalsOnly,
    /// LP & LBD & BD all treated as BD.
    AllBd,
}

impl BdGroup {
    pub fn all() -> [Self; 4] {
        [
            Self::Separate,
            Self::BdAndLbd,
            Self::PedalsOnly,
            Self::AllBd,
        ]
    }
}

/// Chip-over-pad vs pad-over-chip (`EPlaybackPriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitSoundPriority {
    #[default]
    ChipOverPad,
    PadOverChip,
}

impl HitSoundPriority {
    pub fn all() -> [Self; 2] {
        [Self::ChipOverPad, Self::PadOverChip]
    }
}

/// Drums-specific settings from Config.ini Drums section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumsConfig {
    #[serde(default)]
    pub cy_group: CyGroup,
    #[serde(default)]
    pub hh_group: HhGroup,
    #[serde(default)]
    pub ft_group: FtGroup,
    #[serde(default)]
    pub bd_group: BdGroup,
    #[serde(default)]
    pub cymbal_free: bool,
    /// `MutingLP` — HHC/LP input silences ringing hi-hat sounds.
    #[serde(default = "default_muting_lp")]
    pub muting_lp: bool,
    #[serde(default)]
    pub hit_sound_priority_hh: HitSoundPriority,
    #[serde(default)]
    pub hit_sound_priority_ft: HitSoundPriority,
    #[serde(default)]
    pub hit_sound_priority_cy: HitSoundPriority,
    #[serde(default)]
    pub hit_sound_priority_lp: HitSoundPriority,
    /// `PolyphonicSounds` — default 4, range 1..=8.
    #[serde(default = "default_polyphonic")]
    pub polyphonic_sounds: u8,
}

fn default_polyphonic() -> u8 {
    4
}

fn default_muting_lp() -> bool {
    true
}

impl Default for DrumsConfig {
    fn default() -> Self {
        Self {
            cy_group: CyGroup::default(),
            hh_group: HhGroup::default(),
            ft_group: FtGroup::default(),
            bd_group: BdGroup::default(),
            cymbal_free: false,
            muting_lp: default_muting_lp(),
            hit_sound_priority_hh: HitSoundPriority::default(),
            hit_sound_priority_ft: HitSoundPriority::default(),
            hit_sound_priority_cy: HitSoundPriority::default(),
            hit_sound_priority_lp: HitSoundPriority::default(),
            polyphonic_sounds: default_polyphonic(),
        }
    }
}

impl DrumsConfig {
    pub fn clamp_polyphony(&mut self) {
        self.polyphonic_sounds = self.polyphonic_sounds.clamp(1, 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_config_ini() {
        let d = DrumsConfig::default();
        assert_eq!(d.cy_group, CyGroup::Separate);
        assert_eq!(d.hh_group, HhGroup::SeparateAll);
        assert!(!d.cymbal_free);
        assert_eq!(d.polyphonic_sounds, 4);
    }
}
