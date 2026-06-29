//! Drums gameplay config — grouping, cymbal-free, hit-sound priority.
//!
//! Reference: `references/DTXmaniaNX-BocuD/Runtime/Config.ini:166-195`
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConstants.cs:5-28`

use serde::{Deserialize, Serialize};

/// CY/RD grouping (`ECYGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CyGroup {
    /// 打ち分ける — CY pad hits CY chips only; RD pad hits RD only.
    Separate,
    /// 共通 — CY and RD pads share CY/RD chips.
    Common,
}

impl Default for CyGroup {
    fn default() -> Self {
        Self::Separate
    }
}

/// HH/HC/HHO/LC grouping (`EHHGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HhGroup {
    /// 全部打ち分ける
    SeparateAll,
    /// ハイハットのみ打ち分ける (HC+LC vs HO)
    HhAndLc,
    /// 左シンバルのみ打ち分ける (HC+HO vs LC)
    HhAndHo,
    /// 全部共通
    CommonAll,
}

impl Default for HhGroup {
    fn default() -> Self {
        Self::SeparateAll
    }
}

/// LT/FT grouping (`EFTGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FtGroup {
    Separate,
    Common,
}

impl Default for FtGroup {
    fn default() -> Self {
        Self::Separate
    }
}

/// BD/LP/LBD grouping (`EBDGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BdGroup {
    /// LP | LBD | BD all separate.
    Separate,
    /// LP | (LBD & BD).
    BdAndLbd,
    /// LP & (LBD | BD) — pedals only split.
    PedalsOnly,
    /// LP & LBD & BD all treated as BD.
    AllBd,
}

impl Default for BdGroup {
    fn default() -> Self {
        Self::Separate
    }
}

/// Chip-over-pad vs pad-over-chip (`EPlaybackPriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitSoundPriority {
    ChipOverPad,
    PadOverChip,
}

impl Default for HitSoundPriority {
    fn default() -> Self {
        Self::ChipOverPad
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

impl Default for DrumsConfig {
    fn default() -> Self {
        Self {
            cy_group: CyGroup::default(),
            hh_group: HhGroup::default(),
            ft_group: FtGroup::default(),
            bd_group: BdGroup::default(),
            cymbal_free: false,
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
