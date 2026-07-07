//! dtx-config — persisted user configuration (TOML).
//!
//! Port of `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs` (baseline sections).
//! Full CConfigIni (KeyAssign, Drums tables) is ported. Skin / ChangeSkin /
//! Guitar/Bass tables dropped — no skin browser per roadmap refresh.
//!
//! ## Sections ported
//! - `System` — nBGAlpha, nMovieAlpha, bAVIEnabled, bBGAEnabled, bVerticalSyncWait (subset)
//! - `Gameplay` — bTight, bReverse, scroll speed, damage level, lane display
//! - `Audio` — bBGMを発声する, bドラム打音を発声する, per-track volume (subset)
//! - `Drums` — CY/HH/FT/BD grouping, cymbal-free, hit-sound priority, polyphony
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:1-100` (field names).

#![allow(dead_code)] // Some fields used by Phase 1 sub-acts, not yet wired.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod bindings;
pub mod drums;
pub mod key_assign;

pub use bindings::{
    default_bindings_path, load_bindings, save_bindings, BindSource, BindingsFile, InputBindings,
    MidiDeviceConfig, BINDABLE_CHANNELS,
};
pub use drums::{BdGroup, CyGroup, DrumsConfig, FtGroup, HhGroup, HitSoundPriority};
pub use key_assign::{KeyAssignPad, KeyAssignPart, KeyAssignTable, STKeyAssign};

/// Top-level persisted configuration. Each BocuD section becomes a sub-struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// System: windowing, BGA, BGM, logging.
    #[serde(default)]
    pub system: SystemConfig,
    /// Gameplay: scroll, dark mode, reverse.
    #[serde(default)]
    pub gameplay: GameplayConfig,
    /// Audio: volumes and device flags.
    #[serde(default)]
    pub audio: AudioConfig,
    /// Drums grouping / cymbal-free / hit-sound priority.
    #[serde(default)]
    pub drums: DrumsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            gameplay: GameplayConfig::default(),
            audio: AudioConfig::default(),
            drums: DrumsConfig::default(),
        }
    }
}

/// System section — CConfigIni.cs:1-100 (subset).
///
/// Reference: `CActConfigList.System.cs:1-50` — Windowed, FullScreen, VSyncWait, BGAEnabled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemConfig {
    /// `bVerticalSyncWait` — CConfigIni.cs:99
    pub vsync: bool,
    /// `nBGAlpha` (0..255) — CConfigIni.cs:55
    pub bg_alpha: u8,
    /// `nMovieAlpha` (0..255) — CConfigIni.cs:56
    pub movie_alpha: u8,
    /// `bAVIEnabled` — CConfigIni.cs:57
    pub bga_enabled: bool,
    /// `bBGAEnabled` — CConfigIni.cs:58
    pub movie_enabled: bool,
    /// `bOutputLogs` — CConfigIni.cs:99
    pub log_enabled: bool,
    /// `bShowPerformanceInformation` — CConfigIni.cs:99
    pub show_perf_info: bool,
    /// `bMetronome` — CConfigIni.cs:501 (`CActConfigList.Gameplay.cs:150`).
    #[serde(default)]
    pub metronome: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            vsync: true,
            bg_alpha: 255,
            movie_alpha: 255,
            bga_enabled: true,
            movie_enabled: true,
            log_enabled: false,
            show_perf_info: false,
            metronome: false,
        }
    }
}

/// Miss-damage level for the life gauge (BocuD `EDamageLevel`, CConstants.cs:44-48).
/// Config-local mirror of `dtx_core::constants::DamageLevel` (kept here so the
/// Pure config crate stays serde-only). Consumers map it as needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageLevel {
    /// No damage (HP=0 never fails).
    None,
    /// Small drain on miss.
    #[default]
    Small,
    /// Normal drain.
    Normal,
    /// High drain.
    High,
}

impl DamageLevel {
    /// All levels in ascending severity, for UI cycling.
    pub fn all() -> [Self; 4] {
        [Self::None, Self::Small, Self::Normal, Self::High]
    }

    /// Short display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Small => "Small",
            Self::Normal => "Normal",
            Self::High => "High",
        }
    }
}

fn default_input_offset_ms() -> i32 {
    0
}

fn default_bgm_adjust_ms() -> i32 {
    0
}

/// Default `nPlaySpeed` in BocuD units (0x14 == 20 == 1.0×).
fn default_play_speed() -> u8 {
    0x14
}

/// `nPlaySpeed` → playback multiplier. BocuD uses `value / 20.0`, clamped
/// 0.5×..2.0×. We store the raw byte and convert at use sites.
pub const PLAY_SPEED_MIN: u8 = 0x0A; // 0.5×
pub const PLAY_SPEED_MAX: u8 = 0x28; // 2.0×
pub const PLAY_SPEED_DEFAULT: u8 = 0x14; // 1.0×

/// Convert raw `nPlaySpeed` to a multiplier (`value / 20.0`).
pub fn play_speed_multiplier(raw: u8) -> f32 {
    let clamped = raw.clamp(PLAY_SPEED_MIN, PLAY_SPEED_MAX);
    clamped as f32 / 20.0
}

/// NX in-song / config clamp for `nInputAdjustTimeMs` and `nCommonBGMAdjustMs`.
pub const INPUT_OFFSET_CLAMP_MS: i32 = 99;
pub const BGM_ADJUST_CLAMP_MS: i32 = 99;

fn default_damage_level() -> DamageLevel {
    DamageLevel::default()
}

/// `nLaneDisp.Drums` — lane/line visibility (CActConfigList.Drums.cs:175-187).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneDisplay {
    #[default]
    AllOn = 0,
    Half = 1,
    LineOff = 2,
    AllOff = 3,
}

impl LaneDisplay {
    pub fn all() -> [Self; 4] {
        [Self::AllOn, Self::Half, Self::LineOff, Self::AllOff]
    }
    /// Bar/beat lines visible when ALL ON or HALF (BocuD `nLaneDisp == 0 || == 1`).
    pub const fn shows_timing_lines(self) -> bool {
        matches!(self, Self::AllOn | Self::Half)
    }
}

/// Gameplay section — CConfigIni.cs:100-200 (subset).
///
/// Reference: `CActConfigList.Gameplay.cs:1-50`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameplayConfig {
    /// `bTight` — CConfigIni.cs:99 (Tight mode = stricter judgment windows).
    pub tight: bool,
    /// `bReverse.Drums` — reverse scroll direction (CActConfigList.Gameplay.cs).
    pub reverse: bool,
    /// Scroll speed multiplier 0.5..4.0.
    pub scroll_speed: f32,
    /// `bDark` — hide notes / show only judgment (CActConfigList.Gameplay.cs).
    pub dark_mode: bool,
    /// `bFillInEnabled` — CConfigIni.cs:99.
    pub fillin_enabled: bool,
    /// `bSTAGEFAILEDEnabled` — CConfigIni.cs:99.
    pub stage_failed_enabled: bool,
    /// Global input timing offset in ms applied to the judgement clock
    /// (`nInputAdjustTimeMs`, CConfigIni.cs). Positive = notes judged later.
    #[serde(default = "default_input_offset_ms")]
    pub input_offset_ms: i32,
    /// Common BGM auto-chip offset (`nCommonBGMAdjustMs`, CActConfigList.Audio.cs).
    #[serde(default = "default_bgm_adjust_ms")]
    pub bgm_adjust_ms: i32,
    /// Playback speed in BocuD units (`nPlaySpeed`). 0x14 = 1.0×, range
    /// 0x0A..0x28 (0.5×..2.0×). Converted to multiplier via
    /// [`play_speed_multiplier`].
    #[serde(default = "default_play_speed")]
    pub play_speed: u8,
    /// Miss-damage level for the life gauge.
    #[serde(default = "default_damage_level")]
    pub damage_level: DamageLevel,
    /// Lane background + timing line visibility (`nLaneDisp.Drums`).
    #[serde(default)]
    pub lane_display: LaneDisplay,
    /// Path of the last song entered in normal play (editor session uses it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<PathBuf>,
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            tight: false,
            reverse: false,
            scroll_speed: 1.0,
            dark_mode: false,
            fillin_enabled: true,
            stage_failed_enabled: true,
            input_offset_ms: default_input_offset_ms(),
            bgm_adjust_ms: default_bgm_adjust_ms(),
            play_speed: default_play_speed(),
            damage_level: default_damage_level(),
            lane_display: LaneDisplay::default(),
            last_played: None,
        }
    }
}

/// Audio section — CConfigIni.cs:200-300 (subset).
///
/// Reference: `CActConfigList.Audio.cs:1-50`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    /// `bBGMを発声する` (BGM play enabled) — CConfigIni.cs:99.
    pub bgm_enabled: bool,
    /// `bドラム打音を発声する` (drum hit sound) — CConfigIni.cs:99.
    pub drum_sound_enabled: bool,
    /// Master volume 0.0..1.0.
    pub master_volume: f32,
    /// BGM volume 0.0..1.0.
    pub bgm_volume: f32,
    /// Drum hit volume 0.0..1.0.
    pub drum_volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            bgm_enabled: true,
            drum_sound_enabled: true,
            master_volume: 0.8,
            bgm_volume: 0.7,
            drum_volume: 0.8,
        }
    }
}

/// Config I/O errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// TOML parse failure.
    #[error("parse: {0}")]
    Parse(#[from] toml::de::Error),
    /// TOML serialize failure.
    #[error("serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Default config path: `$XDG_CONFIG_HOME/dtxmaniars/config.toml` or
/// `$HOME/.config/dtxmaniars/config.toml` or `config.toml` (cwd fallback).
///
/// Reference: CConfigIni.cs:75 (`ConfigIniファイル名`).
pub fn default_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("dtxmaniars");
        p.push("config.toml");
        return p;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("dtxmaniars");
        p.push("config.toml");
        return p;
    }
    PathBuf::from("config.toml")
}

/// Load config from `path`. Returns `Config::default()` if file is missing or unreadable.
/// Logs a warning on parse failure but still returns defaults.
pub fn load(path: &Path) -> Config {
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Config::default(),
    };
    match toml::from_str(&contents) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("dtx-config: parse failed for {path:?}: {e}; using defaults");
            Config::default()
        }
    }
}

/// Save config to `path`. Creates parent dirs.
pub fn save(path: &Path, cfg: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg)?;
    std::fs::write(path, s)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_system_vsync_on() {
        let s = SystemConfig::default();
        assert!(s.vsync);
        assert_eq!(s.bg_alpha, 255);
        assert!(s.bga_enabled);
    }

    #[test]
    fn default_gameplay_scroll_one() {
        let g = GameplayConfig::default();
        assert!((g.scroll_speed - 1.0).abs() < f32::EPSILON);
        assert!(!g.tight);
        assert!(!g.reverse);
        assert!(g.fillin_enabled);
    }

    #[test]
    fn default_audio_master_eighty_pct() {
        let a = AudioConfig::default();
        assert!((a.master_volume - 0.8).abs() < 0.01);
        assert!(a.bgm_enabled);
    }

    #[test]
    fn round_trip_serde_toml() {
        let cfg = Config::default();
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn save_load_round_trip() {
        let tmp = std::env::temp_dir().join("dtxmaniars_cfg_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let p = tmp.join("config.toml");
        let mut cfg = Config::default();
        cfg.gameplay.scroll_speed = 1.5;
        cfg.audio.bgm_volume = 0.42;
        save(&p, &cfg).unwrap();
        let back = load(&p);
        assert!((back.gameplay.scroll_speed - 1.5).abs() < 0.001);
        assert!((back.audio.bgm_volume - 0.42).abs() < 0.001);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn last_played_round_trips_and_defaults_none() {
        let mut cfg = Config::default();
        assert_eq!(cfg.gameplay.last_played, None);
        cfg.gameplay.last_played = Some(std::path::PathBuf::from("/tmp/x.dtx"));
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.gameplay.last_played, cfg.gameplay.last_played);
    }

    #[test]
    fn load_missing_returns_defaults() {
        let p = PathBuf::from("/nonexistent/dtxmaniars/no_such_config.toml");
        let cfg = load(&p);
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn load_corrupt_returns_defaults() {
        let tmp = std::env::temp_dir().join("dtxmaniars_cfg_corrupt");
        let _ = std::fs::create_dir_all(&tmp);
        let p = tmp.join("config.toml");
        std::fs::write(&p, "this is = not valid toml = [[").unwrap();
        let cfg = load(&p);
        assert_eq!(cfg, Config::default());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn partial_config_fills_defaults() {
        // Missing sections must be filled with defaults.
        let s = "";
        let cfg: Config = toml::from_str(s).unwrap();
        assert_eq!(cfg.system, SystemConfig::default());
        assert_eq!(cfg.gameplay, GameplayConfig::default());
        assert_eq!(cfg.audio, AudioConfig::default());
        assert_eq!(cfg.drums, drums::DrumsConfig::default());
    }

    #[test]
    fn save_creates_parent_dirs() {
        let tmp = std::env::temp_dir()
            .join("dtxmaniars_cfg_nested")
            .join("deep")
            .join("path");
        let p = tmp.join("config.toml");
        save(&p, &Config::default()).unwrap();
        assert!(p.exists());
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn default_path_resolves_to_a_filename() {
        let p = default_path();
        assert!(p.file_name().is_some());
        assert_eq!(p.file_name().unwrap(), "config.toml");
    }
}
