use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use dtx_persistence::{replace_bytes, suggest_copy_name, validate_profile_name, PersistenceError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    classic, nx_type_b, nx_type_d, parse_checked, LaneArrangement, LanePreset, LanesSection,
    LayoutError, LayoutFile,
};

pub const LANE_DEFAULT_NAME: &str = "Classic";
pub const LANE_REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct LaneProfile {
    pub arrangement: LaneArrangement,
}

impl LaneProfile {
    pub fn from_arrangement(arrangement: LaneArrangement) -> Self {
        Self { arrangement }
    }
}

impl Serialize for LaneProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LanesSection::from_arrangement(&self.arrangement).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LaneProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            arrangement: LanesSection::deserialize(deserializer)?.resolve(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneProfileRegistry {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, LaneProfile>,
}

impl Default for LaneProfileRegistry {
    fn default() -> Self {
        Self {
            version: LANE_REGISTRY_VERSION,
            active: LANE_DEFAULT_NAME.to_owned(),
            profiles: BTreeMap::new(),
        }
    }
}

pub fn lane_builtins() -> BTreeMap<String, LaneProfile> {
    BTreeMap::from([
        (
            LANE_DEFAULT_NAME.to_owned(),
            LaneProfile::from_arrangement(classic()),
        ),
        (
            "NX Type-B".to_owned(),
            LaneProfile::from_arrangement(nx_type_b()),
        ),
        (
            "NX Type-D".to_owned(),
            LaneProfile::from_arrangement(nx_type_d()),
        ),
    ])
}

pub fn lane_registry() -> LaneProfileRegistry {
    LaneProfileRegistry::default()
}

pub fn active_lane_arrangement(registry: &LaneProfileRegistry) -> LaneArrangement {
    registry
        .profiles
        .get(&registry.active)
        .map(|p| p.arrangement.clone())
        .or_else(|| {
            lane_builtins()
                .get(&registry.active)
                .map(|p| p.arrangement.clone())
        })
        .unwrap_or_else(classic)
}

#[derive(Debug, Error)]
pub enum LaneRegistryLoadError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("unsupported lane registry version {version} in {path}")]
    UnsupportedVersion { path: PathBuf, version: u32 },
    #[error("invalid lane registry in {path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

#[derive(Debug, Error)]
pub enum LaneRegistryError {
    #[error("cannot serialize lane registry: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("cannot persist lane registry: {0}")]
    Persistence(#[from] PersistenceError),
    #[error("confirmation is required before resetting {path}")]
    ConfirmationRequired { path: PathBuf },
    #[error("cannot back up {path}: {source}")]
    Backup {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub enum LaneRegistryStartup {
    Ready(LaneProfileRegistry),
    LegacySession {
        registry: LaneProfileRegistry,
        write_error: LaneRegistryError,
    },
    ReadOnlyBuiltins(LaneRegistryLoadError),
}

fn validate_registry(
    path: &Path,
    registry: &LaneProfileRegistry,
) -> Result<(), LaneRegistryLoadError> {
    if registry.version != LANE_REGISTRY_VERSION {
        return Err(LaneRegistryLoadError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: registry.version,
        });
    }
    let builtins = lane_builtins();
    let names: Vec<&str> = registry.profiles.keys().map(String::as_str).collect();
    for name in registry.profiles.keys() {
        let existing = names.iter().copied().filter(|other| *other != name);
        if let Err(error) =
            validate_profile_name(name, builtins.keys().map(String::as_str), existing, None)
        {
            return Err(LaneRegistryLoadError::Invalid {
                path: path.to_path_buf(),
                reason: format!("profile {name:?}: {error}"),
            });
        }
    }
    if !builtins.contains_key(&registry.active) && !registry.profiles.contains_key(&registry.active)
    {
        return Err(LaneRegistryLoadError::Invalid {
            path: path.to_path_buf(),
            reason: format!("active profile {:?} does not exist", registry.active),
        });
    }
    Ok(())
}

fn write_registry(path: &Path, registry: &LaneProfileRegistry) -> Result<(), LaneRegistryError> {
    let bytes = toml::to_string_pretty(registry)?;
    replace_bytes(path, bytes.as_bytes())?;
    Ok(())
}

fn migrated_registry(layout: &LayoutFile) -> LaneProfileRegistry {
    let builtins = lane_builtins();
    let active = match layout.lanes.preset {
        LanePreset::Classic => LANE_DEFAULT_NAME.to_owned(),
        LanePreset::NxTypeB => "NX Type-B".to_owned(),
        LanePreset::NxTypeD => "NX Type-D".to_owned(),
        LanePreset::Custom => {
            suggest_copy_name("Migrated lanes", builtins.keys().map(String::as_str))
        }
    };
    if builtins.contains_key(&active) {
        LaneProfileRegistry {
            active,
            ..LaneProfileRegistry::default()
        }
    } else {
        LaneProfileRegistry {
            active: active.clone(),
            profiles: [(
                active,
                LaneProfile::from_arrangement(layout.lanes.resolve()),
            )]
            .into_iter()
            .collect(),
            ..LaneProfileRegistry::default()
        }
    }
}

pub fn load_lane_registry(path: &Path, legacy_layout: &Path) -> LaneRegistryStartup {
    match std::fs::read_to_string(path) {
        Ok(raw) => match toml::from_str::<LaneProfileRegistry>(&raw)
            .map_err(|source| LaneRegistryLoadError::Parse {
                path: path.to_path_buf(),
                source,
            })
            .and_then(|registry| validate_registry(path, &registry).map(|()| registry))
        {
            Ok(registry) => LaneRegistryStartup::Ready(registry),
            Err(error) => LaneRegistryStartup::ReadOnlyBuiltins(error),
        },
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            match std::fs::read_to_string(legacy_layout) {
                Ok(raw) => match parse_checked(&raw)
                    .map_err(|source| LaneRegistryLoadError::Parse {
                        path: legacy_layout.to_path_buf(),
                        source,
                    })
                    .and_then(|layout| {
                        if layout.version == crate::LATEST_VERSION {
                            Ok(layout)
                        } else {
                            Err(LaneRegistryLoadError::Invalid {
                                path: legacy_layout.to_path_buf(),
                                reason: format!(
                                    "unsupported legacy layout version {}",
                                    layout.version
                                ),
                            })
                        }
                    }) {
                    Ok(layout) => {
                        let registry = migrated_registry(&layout);
                        match write_registry(path, &registry) {
                            Ok(()) => LaneRegistryStartup::Ready(registry),
                            Err(write_error) => LaneRegistryStartup::LegacySession {
                                registry,
                                write_error,
                            },
                        }
                    }
                    Err(error) => LaneRegistryStartup::ReadOnlyBuiltins(error),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    LaneRegistryStartup::Ready(LaneProfileRegistry::default())
                }
                Err(source) => LaneRegistryStartup::ReadOnlyBuiltins(LaneRegistryLoadError::Read {
                    path: legacy_layout.to_path_buf(),
                    source,
                }),
            }
        }
        Err(source) => LaneRegistryStartup::ReadOnlyBuiltins(LaneRegistryLoadError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn save_lane_registry(
    path: &Path,
    registry: &LaneProfileRegistry,
) -> Result<(), LaneRegistryError> {
    write_registry(path, registry)
}

pub fn load_layout_with_lane_authority(
    layout_path: &Path,
    lane_registry_path: &Path,
) -> Result<(LayoutFile, LaneRegistryStartup), LayoutError> {
    let layout = match std::fs::read_to_string(layout_path) {
        Ok(raw) => parse_checked(&raw)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => LayoutFile::default(),
        Err(error) => return Err(LayoutError::Io(error)),
    };
    let startup = load_lane_registry(lane_registry_path, layout_path);
    Ok((layout, startup))
}

pub fn backup_and_reset_lane_registry(
    path: &Path,
    confirmed: bool,
    now: SystemTime,
) -> Result<LaneProfileRegistry, LaneRegistryError> {
    if !confirmed {
        return Err(LaneRegistryError::ConfirmationRequired {
            path: path.to_path_buf(),
        });
    }
    if path.exists() {
        let stamp = now
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("lane-profiles.toml");
        let backup = path.with_file_name(format!("{file_name}.backup-{stamp}"));
        // hard_link fails atomically with AlreadyExists, so a concurrently
        // created backup can never be overwritten.
        std::fs::hard_link(path, &backup).map_err(|source| LaneRegistryError::Backup {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::remove_file(path).map_err(|source| LaneRegistryError::Backup {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let registry = LaneProfileRegistry::default();
    write_registry(path, &registry)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("dtx-layout-profile-tests")
            .join(std::process::id().to_string())
            .join(name)
    }

    #[test]
    fn lane_registry_round_trips_custom_order_widths_and_map() {
        let arrangement = LanesSection {
            preset: LanePreset::Custom,
            order: Some(vec!["BD".into(), "SD".into()]),
            widths: Some([("BD".into(), 120.0)].into()),
            map: Some([("CY".into(), "SD".into())].into()),
        }
        .resolve();
        let registry = LaneProfileRegistry {
            active: "Desk".into(),
            profiles: [("Desk".into(), LaneProfile::from_arrangement(arrangement))]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let raw = toml::to_string_pretty(&registry).expect("registry serializes");
        let parsed: LaneProfileRegistry = toml::from_str(&raw).expect("registry parses");
        assert_eq!(parsed, registry);
    }

    #[test]
    fn lane_builtins_resolve_exact_presets() {
        let builtins = lane_builtins();
        assert_eq!(builtins["Classic"].arrangement, classic());
        assert_eq!(builtins["NX Type-B"].arrangement, nx_type_b());
        assert_eq!(builtins["NX Type-D"].arrangement, nx_type_d());
    }

    #[test]
    fn custom_legacy_layout_becomes_migrated_lanes() {
        let dir = root("custom");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry = dir.join("lane-profiles.toml");
        let file = LayoutFile {
            lanes: LanesSection {
                preset: LanePreset::Custom,
                order: Some(vec!["BD".into(), "SD".into()]),
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&layout, toml::to_string_pretty(&file).expect("layout")).expect("write");
        let startup = load_lane_registry(&registry, &layout);
        assert!(
            matches!(startup, LaneRegistryStartup::Ready(value) if value.active.starts_with("Migrated lanes"))
        );
        assert!(layout.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn named_legacy_layout_activates_builtin() {
        let dir = root("named");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry = dir.join("lane-profiles.toml");
        let file = LayoutFile {
            lanes: LanesSection {
                preset: LanePreset::NxTypeB,
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&layout, toml::to_string_pretty(&file).expect("layout")).expect("write");
        let startup = load_lane_registry(&registry, &layout);
        assert!(
            matches!(startup, LaneRegistryStartup::Ready(value) if value.active == "NX Type-B" && value.profiles.is_empty())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_registry_takes_precedence_over_compatibility_snapshot() {
        let dir = root("authority");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry_path = dir.join("lane-profiles.toml");
        let mut registry = LaneProfileRegistry::default();
        registry.active = "Desk".into();
        registry
            .profiles
            .insert("Desk".into(), LaneProfile::from_arrangement(nx_type_d()));
        save_lane_registry(&registry_path, &registry).expect("registry");
        let file = LayoutFile {
            lanes: LanesSection {
                preset: LanePreset::NxTypeB,
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&layout, toml::to_string_pretty(&file).expect("layout")).expect("write");
        let (_, startup) = load_layout_with_lane_authority(&layout, &registry_path).expect("load");
        assert!(matches!(startup, LaneRegistryStartup::Ready(value) if value.active == "Desk"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_legacy_layout_blocks_migration() {
        let dir = root("malformed-legacy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry = dir.join("lane-profiles.toml");
        std::fs::write(&layout, "not = [valid").expect("write");
        let startup = load_lane_registry(&registry, &layout);
        assert!(matches!(
            startup,
            LaneRegistryStartup::ReadOnlyBuiltins(LaneRegistryLoadError::Parse { .. })
        ));
        assert!(!registry.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_migration_does_not_rewrite_layout_scene() {
        let dir = root("scene-preserved");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry = dir.join("lane-profiles.toml");
        let file = LayoutFile {
            lanes: LanesSection {
                preset: LanePreset::NxTypeB,
                ..Default::default()
            },
            ..Default::default()
        };
        let raw = toml::to_string_pretty(&file).expect("layout");
        std::fs::write(&layout, &raw).expect("write");
        let startup = load_lane_registry(&registry, &layout);
        assert!(matches!(startup, LaneRegistryStartup::Ready(_)));
        assert!(registry.exists());
        assert_eq!(
            std::fs::read_to_string(&layout).expect("layout reads"),
            raw,
            "migration must not rewrite the legacy layout file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_migration_retry_is_idempotent() {
        let dir = root("retry");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry = dir.join("blocked").join("lane-profiles.toml");
        let file = LayoutFile {
            lanes: LanesSection {
                preset: LanePreset::NxTypeD,
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&layout, toml::to_string_pretty(&file).expect("layout")).expect("write");
        std::fs::write(dir.join("blocked"), "file blocks directory").expect("blocker writes");
        let first = load_lane_registry(&registry, &layout);
        assert!(matches!(
            first,
            LaneRegistryStartup::LegacySession { ref registry, .. }
                if registry.active == "NX Type-D"
        ));
        std::fs::remove_file(dir.join("blocked")).expect("blocker removes");
        std::fs::create_dir(dir.join("blocked")).expect("registry parent creates");
        let second = load_lane_registry(&registry, &layout);
        assert!(matches!(
            second,
            LaneRegistryStartup::Ready(value) if value.active == "NX Type-D"
        ));
        assert!(registry.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_lane_registry_requires_confirmed_backup_reset() {
        let dir = root("corrupt-reset");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let layout = dir.join("layout.toml");
        let registry_path = dir.join("lane-profiles.toml");
        std::fs::write(&registry_path, "not = [valid").expect("write");
        assert!(matches!(
            load_lane_registry(&registry_path, &layout),
            LaneRegistryStartup::ReadOnlyBuiltins(LaneRegistryLoadError::Parse { .. })
        ));
        assert!(matches!(
            backup_and_reset_lane_registry(&registry_path, false, UNIX_EPOCH),
            Err(LaneRegistryError::ConfirmationRequired { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(&registry_path).expect("registry reads"),
            "not = [valid"
        );
        let reset = backup_and_reset_lane_registry(&registry_path, true, UNIX_EPOCH)
            .expect("reset succeeds");
        assert_eq!(reset.active, LANE_DEFAULT_NAME);
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .expect("dir reads")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).expect("backup reads"),
            "not = [valid"
        );
        assert!(matches!(
            load_lane_registry(&registry_path, &layout),
            LaneRegistryStartup::Ready(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_rejects_existing_timestamped_backup_without_overwriting() {
        let dir = root("reset-collision");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let registry_path = dir.join("lane-profiles.toml");
        let backup = dir.join("lane-profiles.toml.backup-0");
        std::fs::write(&registry_path, "current registry").expect("write");
        std::fs::write(&backup, "existing backup").expect("write");
        assert!(matches!(
            backup_and_reset_lane_registry(&registry_path, true, UNIX_EPOCH),
            Err(LaneRegistryError::Backup { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(
            std::fs::read_to_string(&registry_path).expect("registry reads"),
            "current registry"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).expect("backup reads"),
            "existing backup"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
