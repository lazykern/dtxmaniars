//! `CSkin` — port of `references/DTXmaniaNX-BocuD/DTXMania/Core/CSkin.cs` (1,147 LoC).
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! ## Role
//!
//! `CSkin` is the central skin / asset path resolver. The C# class has:
//! - An `ESystemSound` enum naming the 16+ system sounds (BGM per stage,
//!   cursor, decide, cancel, full-combo, etc.)
//! - A `CSystemSound` class per system sound (load + play + stop)
//! - A `Path(string relative)` static method that resolves a relative
//!   path to the current skin's `Graphics/Default/<subfolder>/<relative>`
//! - Texture preloading methods (replaced by Bevy AssetServer)
//!
//! In Rust we model the path resolver + enum; the CSystemSound class
//! is essentially a thin wrapper over `bevy_kira_audio` which we
//! already use via `dtx-audio`. The texture preload methods are not
//! ported (Bevy AssetServer replaces).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CSkin.cs:1-1147`

#![allow(dead_code)] // Some sub-acts consumed by future Phase 1+ stages.

use std::path::{Path, PathBuf};

/// System sound identifiers (BocuD `ESystemSound` CSkin.cs:8-26).
///
/// 16 system sounds used by UI navigation, stage transitions, and
/// gameplay feedback. Each sound is loaded from
/// `Graphics/Default/System/<name>.ogg` (BocuD CSkin.cs conventions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ESystemSound {
    /// BGM options screen.
    BGMOptions = 0,
    /// BGM config screen.
    BGMConfig,
    /// BGM startup screen.
    BGMStartup,
    /// BGM song select screen.
    BGMSongSelect,
    /// BGM result screen.
    BGMResult,
    /// Stage failed SFX.
    SoundStageFailed,
    /// Cursor movement SFX.
    SoundCursorMovement,
    /// Game start SFX.
    SoundGameStart,
    /// Game end SFX.
    SoundGameEnd,
    /// Stage clear SFX.
    SoundStageClear,
    /// Title SFX.
    SoundTitle,
    /// Full combo SFX.
    SoundFullCombo,
    /// Audience SFX.
    SoundAudience,
    /// Song load start SFX.
    SoundSongLoadStart,
    /// Decide SFX.
    SoundDecide,
    /// Cancel SFX.
    SoundCancel,
    /// Change SFX.
    SoundChange,
    /// Decide song SFX.
    SoundDecideSong,
    /// Excellent SFX.
    SoundExcellent,
    /// New record SFX.
    SoundNewRecord,
    /// Select music SFX.
    SoundSelectMusic,
    /// Novice rank SFX.
    SoundNovice,
    /// Regular rank SFX.
    SoundRegular,
    /// Expert rank SFX.
    SoundExpert,
    /// Master rank SFX.
    SoundMaster,
    /// Basic rank SFX.
    SoundBasic,
    /// Advanced rank SFX.
    SoundAdvanced,
    /// Extreme rank SFX.
    SoundExtreme,
    /// Metronome SFX.
    SoundMetronome,
    /// Count sentinel (BocuD `ESystemSound.Count`).
    Count,
}

impl ESystemSound {
    /// All 29 system sounds (BocuD has 28 named + Count).
    pub fn all() -> [Self; 28] {
        [
            Self::BGMOptions,
            Self::BGMConfig,
            Self::BGMStartup,
            Self::BGMSongSelect,
            Self::BGMResult,
            Self::SoundStageFailed,
            Self::SoundCursorMovement,
            Self::SoundGameStart,
            Self::SoundGameEnd,
            Self::SoundStageClear,
            Self::SoundTitle,
            Self::SoundFullCombo,
            Self::SoundAudience,
            Self::SoundSongLoadStart,
            Self::SoundDecide,
            Self::SoundCancel,
            Self::SoundChange,
            Self::SoundDecideSong,
            Self::SoundExcellent,
            Self::SoundNewRecord,
            Self::SoundSelectMusic,
            Self::SoundNovice,
            Self::SoundRegular,
            Self::SoundExpert,
            Self::SoundMaster,
            Self::SoundBasic,
            Self::SoundAdvanced,
            Self::SoundExtreme,
        ]
    }

    /// Display label (BocuD `ToString()`).
    pub fn label(self) -> &'static str {
        match self {
            Self::BGMOptions => "BGM Options",
            Self::BGMConfig => "BGM Config",
            Self::BGMStartup => "BGM Startup",
            Self::BGMSongSelect => "BGM SongSelect",
            Self::BGMResult => "BGM Result",
            Self::SoundStageFailed => "SoundStageFailed",
            Self::SoundCursorMovement => "SoundCursorMovement",
            Self::SoundGameStart => "SoundGameStart",
            Self::SoundGameEnd => "SoundGameEnd",
            Self::SoundStageClear => "SoundStageClear",
            Self::SoundTitle => "SoundTitle",
            Self::SoundFullCombo => "SoundFullCombo",
            Self::SoundAudience => "SoundAudience",
            Self::SoundSongLoadStart => "SoundSongLoadStart",
            Self::SoundDecide => "SoundDecide",
            Self::SoundCancel => "SoundCancel",
            Self::SoundChange => "SoundChange",
            Self::SoundDecideSong => "SoundDecideSong",
            Self::SoundExcellent => "SoundExcellent",
            Self::SoundNewRecord => "SoundNewRecord",
            Self::SoundSelectMusic => "SoundSelectMusic",
            Self::SoundNovice => "SoundNovice",
            Self::SoundRegular => "SoundRegular",
            Self::SoundExpert => "SoundExpert",
            Self::SoundMaster => "SoundMaster",
            Self::SoundBasic => "SoundBasic",
            Self::SoundAdvanced => "SoundAdvanced",
            Self::SoundExtreme => "SoundExtreme",
            Self::SoundMetronome => "SoundMetronome",
            Self::Count => "Count",
        }
    }
}

/// Skin subfolder resolver (BocuD `CSkin.Path(string relative)`).
///
/// Resolves a relative asset path to the current skin's
/// `Graphics/Default/<subfolder>/<relative>`. The C# method is static;
/// the current skin name is stored in `CConfigIni.skin` (which we
/// hold as `Config::skin` in `dtx-config`).
///
/// In Rust we model this as a `SkinResolver` resource that holds the
/// current skin name + system graphics root, and provides `path()`
/// for resolving relative paths.
#[derive(Debug, Clone)]
pub struct SkinResolver {
    /// System graphics root (BocuD `CSkin.strSystemRoot`).
    /// Typically `"Graphics/Default"`.
    pub system_root: PathBuf,
    /// Current skin subfolder (e.g., `"Default"`, `"MySkin"`).
    pub current_skin: String,
}

impl Default for SkinResolver {
    fn default() -> Self {
        Self {
            system_root: PathBuf::from("Graphics").join("Default"),
            current_skin: "Default".to_string(),
        }
    }
}

impl SkinResolver {
    /// Construct with a specific skin name.
    pub fn new(current_skin: impl Into<String>) -> Self {
        Self {
            current_skin: current_skin.into(),
            ..Default::default()
        }
    }

    /// Resolve a relative asset path (BocuD `CSkin.Path(relative)`).
    ///
    /// Returns `system_root/<skin>/<relative>`.
    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.system_root.join(&self.current_skin).join(relative)
    }

    /// Resolve a system sound path.
    pub fn system_sound_path(&self, sound: ESystemSound) -> PathBuf {
        // BocuD CSkin convention: System/<name>.ogg
        self.path(format!("System/{}.ogg", sound.label()))
    }

    /// Switch to a different skin (BocuD `CSkin.skinChange`).
    pub fn change_skin(&mut self, new_skin: impl Into<String>) {
        self.current_skin = new_skin.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ESystemSound ===

    #[test]
    fn e_system_sound_all_has_28() {
        // BocuD has 28 named + Count = 29, but the all() array returns
        // 28 (excludes Count sentinel).
        assert_eq!(ESystemSound::all().len(), 28);
    }

    #[test]
    fn e_system_sound_label_unique() {
        let mut labels: Vec<&str> = ESystemSound::all().iter().map(|s| s.label()).collect();
        let original = labels.clone();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), original.len());
    }

    #[test]
    fn e_system_sound_decide_label() {
        // Used in song_loading / config / etc.
        assert_eq!(ESystemSound::SoundDecide.label(), "SoundDecide");
    }

    // === SkinResolver ===

    #[test]
    fn resolver_default_uses_default_skin() {
        let r = SkinResolver::default();
        assert_eq!(r.current_skin, "Default");
    }

    #[test]
    fn resolver_path_includes_skin_subfolder() {
        let r = SkinResolver::new("MySkin");
        let p = r.path("Textures/foo.png");
        assert!(p.to_string_lossy().contains("MySkin"));
        assert!(p.to_string_lossy().contains("foo.png"));
    }

    #[test]
    fn resolver_system_sound_path() {
        let r = SkinResolver::default();
        let p = r.system_sound_path(ESystemSound::SoundDecide);
        assert!(p.to_string_lossy().contains("SoundDecide"));
    }

    #[test]
    fn resolver_change_skin() {
        let mut r = SkinResolver::new("Default");
        r.change_skin("MyCoolSkin");
        assert_eq!(r.current_skin, "MyCoolSkin");
        let p = r.path("foo.png");
        assert!(p.to_string_lossy().contains("MyCoolSkin"));
    }

    #[test]
    fn resolver_path_with_nested_relative() {
        let r = SkinResolver::default();
        let p = r.path("7_Gauge_Guitar.png");
        assert!(p.to_string_lossy().ends_with("7_Gauge_Guitar.png"));
    }
}
