use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

use dtx_persistence::{replace_bytes, PersistenceError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::default_path;

pub const PRACTICE_PRESET_VERSION: u32 = 1;

const TEMPO_MIN: f32 = 0.5;
const TEMPO_MAX: f32 = 1.5;
const RAMP_TEMPO_GAP: f32 = 0.05;
const RAMP_STEP_MIN: f32 = 0.05;
const RAMP_STEP_MAX: f32 = 0.25;
const RAMP_THRESHOLD_MIN: f32 = 50.0;
const RAMP_THRESHOLD_MAX: f32 = 100.0;
const RAMP_SUCCESSES_MIN: u8 = 1;
const RAMP_SUCCESSES_MAX: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PracticeChartKey {
    pub canonical_chart_hash: String,
    pub difficulty: u8,
}

impl PracticeChartKey {
    pub fn new(canonical_chart_hash: impl Into<String>, difficulty: u8) -> Self {
        Self {
            canonical_chart_hash: canonical_chart_hash.into(),
            difficulty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticeSnapPreset {
    Bar,
    Beat,
    HalfBeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticePrerollPreset {
    OneBar,
    TwoSeconds,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RampPreset {
    pub start_tempo: f32,
    pub target_tempo: f32,
    pub step: f32,
    pub threshold_pct: f32,
    pub required_successes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PracticeTrainerPreset {
    Off,
    Wait,
    Ramp(RampPreset),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePresetConfig {
    pub loop_start_ms: Option<i64>,
    pub loop_end_ms: Option<i64>,
    pub snap: PracticeSnapPreset,
    pub tempo: f32,
    pub preroll: PracticePrerollPreset,
    pub count_in: bool,
    pub trainer: PracticeTrainerPreset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePreset {
    pub id: u64,
    pub chart: PracticeChartKey,
    pub name: Option<String>,
    pub source_path_hint: Option<PathBuf>,
    pub config: PracticePresetConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastUsedPractice {
    pub chart: PracticeChartKey,
    pub source_path_hint: Option<PathBuf>,
    pub config: PracticePresetConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticePresetRegistry {
    pub version: u32,
    pub next_id: u64,
    #[serde(default)]
    pub presets: Vec<PracticePreset>,
    #[serde(default)]
    pub last_used: Vec<LastUsedPractice>,
}

impl Default for PracticePresetRegistry {
    fn default() -> Self {
        Self {
            version: PRACTICE_PRESET_VERSION,
            next_id: 1,
            presets: Vec::new(),
            last_used: Vec::new(),
        }
    }
}

impl PracticePresetRegistry {
    pub fn create(
        &mut self,
        chart: PracticeChartKey,
        name: Option<&str>,
        source_path_hint: Option<PathBuf>,
        config: PracticePresetConfig,
    ) -> Result<u64, PracticePresetError> {
        let mut candidate = self.clone();
        let id = candidate.next_id;
        candidate.next_id = candidate
            .next_id
            .checked_add(1)
            .ok_or(PracticePresetError::IdExhausted)?;
        candidate.presets.push(PracticePreset {
            id,
            chart,
            name: name.map(str::to_owned),
            source_path_hint,
            config,
        });
        candidate.validate_and_normalize()?;
        *self = candidate;
        Ok(id)
    }

    pub fn update(
        &mut self,
        id: u64,
        name: Option<&str>,
        source_path_hint: Option<PathBuf>,
        config: PracticePresetConfig,
    ) -> Result<(), PracticePresetError> {
        let mut candidate = self.clone();
        let preset = candidate
            .presets
            .iter_mut()
            .find(|preset| preset.id == id)
            .ok_or(PracticePresetError::PresetNotFound(id))?;
        preset.name = name.map(str::to_owned);
        preset.source_path_hint = source_path_hint;
        preset.config = config;
        candidate.validate_and_normalize()?;
        *self = candidate;
        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> Result<(), PracticePresetError> {
        let mut candidate = self.clone();
        let index = candidate
            .presets
            .iter()
            .position(|preset| preset.id == id)
            .ok_or(PracticePresetError::PresetNotFound(id))?;
        candidate.presets.remove(index);
        candidate.validate_and_normalize()?;
        *self = candidate;
        Ok(())
    }

    pub fn preset(&self, id: u64) -> Option<&PracticePreset> {
        self.presets.iter().find(|preset| preset.id == id)
    }

    pub fn presets_for<'a>(
        &'a self,
        chart: &'a PracticeChartKey,
    ) -> impl Iterator<Item = &'a PracticePreset> + 'a {
        self.presets
            .iter()
            .filter(move |preset| &preset.chart == chart)
    }

    pub fn last_used(&self, chart: &PracticeChartKey) -> Option<&LastUsedPractice> {
        self.last_used.iter().find(|entry| &entry.chart == chart)
    }

    pub fn set_last_used(
        &mut self,
        chart: PracticeChartKey,
        source_path_hint: Option<PathBuf>,
        config: PracticePresetConfig,
    ) -> Result<(), PracticePresetError> {
        let mut candidate = self.clone();
        let replacement = LastUsedPractice {
            chart: chart.clone(),
            source_path_hint,
            config,
        };
        if let Some(entry) = candidate
            .last_used
            .iter_mut()
            .find(|entry| entry.chart == chart)
        {
            *entry = replacement;
        } else {
            candidate.last_used.push(replacement);
        }
        candidate.validate_and_normalize()?;
        *self = candidate;
        Ok(())
    }

    fn validate_and_normalize(&mut self) -> Result<(), PracticePresetError> {
        if self.version != PRACTICE_PRESET_VERSION {
            return Err(PracticePresetError::UnsupportedVersion {
                found: self.version,
                supported: PRACTICE_PRESET_VERSION,
            });
        }
        if self.next_id == 0 {
            return Err(PracticePresetError::InvalidNextId);
        }

        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut highest_id = 0;
        for preset in &mut self.presets {
            if preset.id == 0 || !ids.insert(preset.id) {
                return Err(PracticePresetError::DuplicatePresetId(preset.id));
            }
            highest_id = highest_id.max(preset.id);
            normalize_name(&mut preset.name)?;
            if let Some(name) = &preset.name {
                let key = (preset.chart.clone(), comparison_key(name));
                if !names.insert(key) {
                    return Err(PracticePresetError::DuplicateName(name.clone()));
                }
            }
            validate_config(&preset.config)?;
        }
        if self.next_id <= highest_id {
            return Err(PracticePresetError::InvalidNextId);
        }

        let mut last_used_charts = BTreeSet::new();
        for entry in &self.last_used {
            if !last_used_charts.insert(entry.chart.clone()) {
                return Err(PracticePresetError::DuplicateLastUsed);
            }
            validate_config(&entry.config)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PracticePresetError {
    #[error("cannot read practice presets from {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot parse practice presets from {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialize practice presets: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("practice preset version {found} is unsupported; supported version is {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("practice preset name exceeds 48 characters")]
    NameTooLong,
    #[error("practice preset name contains a control character")]
    NameControlCharacter,
    #[error("practice preset name already exists for this chart")]
    DuplicateName(String),
    #[error("practice preset id {0} is duplicated or invalid")]
    DuplicatePresetId(u64),
    #[error("practice preset id {0} does not exist")]
    PresetNotFound(u64),
    #[error("practice preset id space is exhausted")]
    IdExhausted,
    #[error("practice preset next_id is invalid")]
    InvalidNextId,
    #[error("practice preset has duplicate Last Used entries")]
    DuplicateLastUsed,
    #[error("practice loop endpoints must both be absent or satisfy 0 <= start < end")]
    InvalidLoop,
    #[error("practice tempo must be finite and within 0.5..=1.5")]
    InvalidTempo,
    #[error("practice ramp configuration is outside supported bounds")]
    InvalidRamp,
}

#[derive(Debug)]
pub enum PracticePresetStartup {
    Ready(PracticePresetRegistry),
    ReadOnly {
        registry: PracticePresetRegistry,
        error: PracticePresetError,
    },
}

pub fn practice_presets_path() -> PathBuf {
    let mut path = default_path();
    path.set_file_name("practice-presets.toml");
    path
}

pub fn load_practice_presets(path: &Path) -> PracticePresetStartup {
    match load_registry(path) {
        Ok(registry) => PracticePresetStartup::Ready(registry),
        Err(LoadOutcome::Missing) => {
            PracticePresetStartup::Ready(PracticePresetRegistry::default())
        }
        Err(LoadOutcome::Invalid(error)) => PracticePresetStartup::ReadOnly {
            registry: PracticePresetRegistry::default(),
            error,
        },
    }
}

pub fn save_practice_presets(
    path: &Path,
    registry: &PracticePresetRegistry,
) -> Result<(), PracticePresetError> {
    let mut validated = registry.clone();
    validated.validate_and_normalize()?;
    let bytes = toml::to_string_pretty(&validated)?;
    replace_bytes(path, bytes.as_bytes())?;
    Ok(())
}

enum LoadOutcome {
    Missing,
    Invalid(PracticePresetError),
}

#[derive(Deserialize)]
struct VersionHeader {
    version: u32,
}

fn load_registry(path: &Path) -> Result<PracticePresetRegistry, LoadOutcome> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(LoadOutcome::Missing);
        }
        Err(source) => {
            return Err(LoadOutcome::Invalid(PracticePresetError::Read {
                path: path.to_path_buf(),
                source,
            }));
        }
    };
    let header: VersionHeader = toml::from_str(&raw).map_err(|source| {
        LoadOutcome::Invalid(PracticePresetError::Parse {
            path: path.to_path_buf(),
            source,
        })
    })?;
    if header.version != PRACTICE_PRESET_VERSION {
        return Err(LoadOutcome::Invalid(
            PracticePresetError::UnsupportedVersion {
                found: header.version,
                supported: PRACTICE_PRESET_VERSION,
            },
        ));
    }
    let mut registry: PracticePresetRegistry = toml::from_str(&raw).map_err(|source| {
        LoadOutcome::Invalid(PracticePresetError::Parse {
            path: path.to_path_buf(),
            source,
        })
    })?;
    registry
        .validate_and_normalize()
        .map_err(LoadOutcome::Invalid)?;
    Ok(registry)
}

fn normalize_name(name: &mut Option<String>) -> Result<(), PracticePresetError> {
    let Some(raw) = name else {
        return Ok(());
    };
    if raw.chars().any(char::is_control) {
        return Err(PracticePresetError::NameControlCharacter);
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        *name = None;
        return Ok(());
    }
    if trimmed.chars().count() > 48 {
        return Err(PracticePresetError::NameTooLong);
    }
    *raw = trimmed.to_owned();
    Ok(())
}

fn comparison_key(name: &str) -> String {
    name.chars().flat_map(char::to_lowercase).collect()
}

fn validate_config(config: &PracticePresetConfig) -> Result<(), PracticePresetError> {
    match (config.loop_start_ms, config.loop_end_ms) {
        (None, None) => {}
        (Some(start), Some(end)) if start >= 0 && end > start => {}
        _ => return Err(PracticePresetError::InvalidLoop),
    }
    if !in_finite_range(config.tempo, TEMPO_MIN, TEMPO_MAX) {
        return Err(PracticePresetError::InvalidTempo);
    }
    if let PracticeTrainerPreset::Ramp(ramp) = config.trainer {
        if !in_finite_range(ramp.start_tempo, TEMPO_MIN, TEMPO_MAX)
            || !in_finite_range(ramp.target_tempo, TEMPO_MIN, TEMPO_MAX)
            || ramp.target_tempo < ramp.start_tempo + RAMP_TEMPO_GAP
            || !in_finite_range(ramp.step, RAMP_STEP_MIN, RAMP_STEP_MAX)
            || !in_finite_range(ramp.threshold_pct, RAMP_THRESHOLD_MIN, RAMP_THRESHOLD_MAX)
            || !(RAMP_SUCCESSES_MIN..=RAMP_SUCCESSES_MAX).contains(&ramp.required_successes)
        {
            return Err(PracticePresetError::InvalidRamp);
        }
    }
    Ok(())
}

fn in_finite_range(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);

    fn test_path(label: &str) -> PathBuf {
        let sequence = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "dtx-config-practice-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("test directory");
        directory.join(format!("{label}.toml"))
    }

    fn config() -> PracticePresetConfig {
        PracticePresetConfig {
            loop_start_ms: Some(43_200),
            loop_end_ms: Some(51_400),
            snap: PracticeSnapPreset::Bar,
            tempo: 0.8,
            preroll: PracticePrerollPreset::OneBar,
            count_in: true,
            trainer: PracticeTrainerPreset::Ramp(RampPreset {
                start_tempo: 0.7,
                target_tempo: 1.0,
                step: 0.05,
                threshold_pct: 90.0,
                required_successes: 1,
            }),
        }
    }

    fn ramp_config() -> RampPreset {
        match config().trainer {
            PracticeTrainerPreset::Ramp(ramp) => ramp,
            _ => unreachable!("fixture uses ramp"),
        }
    }

    #[test]
    fn practice_key_separates_difficulties() {
        let basic = PracticeChartKey::new("dtx1:abc", 0);
        let extreme = PracticeChartKey::new("dtx1:abc", 2);
        assert_ne!(basic, extreme);
    }

    #[test]
    fn practice_preset_registry_round_trips_every_field() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let config = config();
        let mut registry = PracticePresetRegistry::default();
        let id = registry
            .create(key.clone(), Some("Chorus"), None, config.clone())
            .expect("valid preset");
        let raw = toml::to_string_pretty(&registry).expect("serialize");
        let decoded: PracticePresetRegistry = toml::from_str(&raw).expect("parse");
        assert_eq!(decoded.preset(id).expect("saved").config, config);
        assert_eq!(decoded.presets_for(&key).count(), 1);
    }

    #[test]
    fn corrupt_or_newer_practice_file_is_preserved_as_read_only() {
        let path = test_path("newer");
        std::fs::write(&path, "version = 99\n").expect("fixture");
        let before = std::fs::read(&path).expect("bytes");
        assert!(matches!(
            load_practice_presets(&path),
            PracticePresetStartup::ReadOnly { .. }
        ));
        assert_eq!(std::fs::read(&path).expect("preserved"), before);

        std::fs::write(&path, "not valid = [[").expect("fixture");
        assert!(matches!(
            load_practice_presets(&path),
            PracticePresetStartup::ReadOnly { .. }
        ));
    }

    #[test]
    fn missing_practice_file_loads_default_registry() {
        let path = test_path("missing");
        assert!(matches!(
            load_practice_presets(&path),
            PracticePresetStartup::Ready(registry) if registry == PracticePresetRegistry::default()
        ));
    }

    #[test]
    fn practice_save_atomically_round_trips() {
        let path = test_path("round-trip");
        let mut registry = PracticePresetRegistry::default();
        registry
            .create(
                PracticeChartKey::new("dtx1:abc", 2),
                Some("Chorus"),
                Some(PathBuf::from("songs/example.dtx")),
                config(),
            )
            .expect("valid preset");

        save_practice_presets(&path, &registry).expect("save");

        assert!(matches!(
            load_practice_presets(&path),
            PracticePresetStartup::Ready(saved) if saved == registry
        ));
    }

    #[test]
    fn practice_path_replaces_general_config_filename() {
        assert_eq!(
            practice_presets_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("practice-presets.toml")
        );
    }

    #[test]
    fn practice_names_are_trimmed_and_unique_per_chart() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let other_key = PracticeChartKey::new("dtx1:abc", 1);
        let mut registry = PracticePresetRegistry::default();
        let id = registry
            .create(key.clone(), Some("  Chorus  "), None, config())
            .expect("valid preset");
        assert_eq!(
            registry
                .preset(id)
                .and_then(|preset| preset.name.as_deref()),
            Some("Chorus")
        );

        assert!(registry
            .create(key, Some("chorus"), None, config())
            .is_err());
        assert!(registry
            .create(other_key, Some("chorus"), None, config())
            .is_ok());
    }

    #[test]
    fn practice_names_normalize_blank_and_reject_control_and_overlong_values() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let mut registry = PracticePresetRegistry::default();

        let id = registry
            .create(key.clone(), Some("   "), None, config())
            .expect("blank optional name");
        assert_eq!(registry.preset(id).expect("saved").name, None);
        assert!(registry
            .create(key.clone(), Some("Chorus\nOne"), None, config())
            .is_err());
        assert!(registry
            .create(key, Some(&"x".repeat(49)), None, config())
            .is_err());
    }

    #[test]
    fn practice_config_rejects_invalid_bounds_and_values() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let mut registry = PracticePresetRegistry::default();
        let mut invalid = config();
        invalid.loop_end_ms = None;
        assert!(registry.create(key.clone(), None, None, invalid).is_err());

        let mut invalid = config();
        invalid.loop_end_ms = invalid.loop_start_ms;
        assert!(registry.create(key.clone(), None, None, invalid).is_err());

        let mut invalid = config();
        invalid.tempo = f32::NAN;
        assert!(registry.create(key.clone(), None, None, invalid).is_err());

        let mut invalid = config();
        invalid.trainer = PracticeTrainerPreset::Ramp(RampPreset {
            target_tempo: f32::INFINITY,
            ..match config().trainer {
                PracticeTrainerPreset::Ramp(ramp) => ramp,
                _ => unreachable!("fixture uses ramp"),
            }
        });
        assert!(registry.create(key, None, None, invalid).is_err());
    }

    #[test]
    fn practice_validation_matches_control_boundaries() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let mut registry = PracticePresetRegistry::default();
        let mut lower = config();
        lower.tempo = 0.5;
        lower.trainer = PracticeTrainerPreset::Ramp(RampPreset {
            start_tempo: 0.5,
            target_tempo: 0.55,
            step: 0.05,
            threshold_pct: 50.0,
            required_successes: 1,
        });
        registry
            .create(key.clone(), None, None, lower)
            .expect("lower boundaries are valid");

        let mut upper = config();
        upper.tempo = 1.5;
        upper.trainer = PracticeTrainerPreset::Ramp(RampPreset {
            start_tempo: 1.2,
            target_tempo: 1.5,
            step: 0.25,
            threshold_pct: 100.0,
            required_successes: 3,
        });
        registry
            .create(key.clone(), None, None, upper)
            .expect("upper boundaries are valid");

        let invalid_ramps = [
            RampPreset {
                start_tempo: 0.49,
                ..ramp_config()
            },
            RampPreset {
                target_tempo: 1.51,
                ..ramp_config()
            },
            RampPreset {
                start_tempo: 0.96,
                target_tempo: 1.0,
                ..ramp_config()
            },
            RampPreset {
                step: 0.04,
                ..ramp_config()
            },
            RampPreset {
                step: 0.26,
                ..ramp_config()
            },
            RampPreset {
                threshold_pct: 49.0,
                ..ramp_config()
            },
            RampPreset {
                threshold_pct: 101.0,
                ..ramp_config()
            },
            RampPreset {
                required_successes: 0,
                ..ramp_config()
            },
            RampPreset {
                required_successes: 4,
                ..ramp_config()
            },
        ];
        for ramp in invalid_ramps {
            let mut invalid = config();
            invalid.trainer = PracticeTrainerPreset::Ramp(ramp);
            assert!(registry.create(key.clone(), None, None, invalid).is_err());
        }

        let mut negative = config();
        negative.loop_start_ms = Some(-1);
        assert!(registry.create(key, None, None, negative).is_err());
    }

    #[test]
    fn invalid_practice_save_preserves_existing_file() {
        let path = test_path("invalid-save");
        std::fs::write(&path, b"existing bytes").expect("fixture");
        let mut registry = PracticePresetRegistry::default();
        let id = registry
            .create(PracticeChartKey::new("dtx1:abc", 2), None, None, config())
            .expect("valid preset");
        registry
            .presets
            .iter_mut()
            .find(|preset| preset.id == id)
            .expect("saved")
            .config
            .tempo = f32::NAN;

        assert!(save_practice_presets(&path, &registry).is_err());
        assert_eq!(std::fs::read(&path).expect("preserved"), b"existing bytes");
    }

    #[test]
    fn practice_update_and_delete_are_transactional() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let mut registry = PracticePresetRegistry::default();
        let first = registry
            .create(key.clone(), Some("Chorus"), None, config())
            .expect("valid preset");
        let second = registry
            .create(key, Some("Bridge"), None, config())
            .expect("valid preset");
        let before = registry.clone();

        assert!(registry
            .update(first, Some("bridge"), None, config())
            .is_err());
        assert_eq!(registry, before);

        let mut updated = config();
        updated.tempo = 0.9;
        registry
            .update(first, Some("Final Chorus"), None, updated.clone())
            .expect("valid update");
        assert_eq!(registry.preset(first).expect("updated").config, updated);
        registry.delete(second).expect("existing preset");
        assert!(registry.preset(second).is_none());
        assert!(registry.delete(second).is_err());
    }

    #[test]
    fn practice_last_used_replaces_one_snapshot_per_chart() {
        let key = PracticeChartKey::new("dtx1:abc", 2);
        let other_key = PracticeChartKey::new("dtx1:abc", 1);
        let mut registry = PracticePresetRegistry::default();
        registry
            .set_last_used(key.clone(), None, config())
            .expect("valid snapshot");
        registry
            .set_last_used(other_key, None, config())
            .expect("valid snapshot");

        let mut replacement = config();
        replacement.tempo = 0.9;
        registry
            .set_last_used(key.clone(), None, replacement.clone())
            .expect("valid replacement");

        assert_eq!(registry.last_used.len(), 2);
        assert_eq!(registry.last_used(&key).expect("saved").config, replacement);
    }
}
