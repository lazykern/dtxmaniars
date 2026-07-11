use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use dtx_core::EChannel;
use dtx_input::KeyCode;
use dtx_persistence::{
    replace_bytes, suggest_copy_name, validate_profile_name, PersistenceError, ProfileName,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{BindSource, BindingsFile, InputBindings, BINDABLE_CHANNELS};

pub const KEYBOARD_DEFAULT_NAME: &str = "DTXMania default";
pub const MIDI_DEFAULT_NAME: &str = "General MIDI drums";
pub const PROFILE_REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum RegistryLoadError {
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
    #[error("unsupported profile registry version {version} in {path}")]
    UnsupportedVersion { path: PathBuf, version: u32 },
    #[error("invalid profile registry in {path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

#[derive(Debug, Error)]
pub enum RegistryIoError {
    #[error("cannot serialize profile registry: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("cannot persist profile registry: {0}")]
    Persistence(#[from] PersistenceError),
    #[error("confirmation is required before resetting {path}")]
    ConfirmationRequired { path: PathBuf },
    #[error("cannot back up {path}: {source}")]
    Backup {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot save invalid profile registry at {path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

#[derive(Debug)]
pub enum CheckedLoad<T> {
    Missing,
    Loaded(T),
    Malformed(RegistryLoadError),
}

#[derive(Debug)]
pub enum RegistryStartup<T> {
    Ready(T),
    LegacySession {
        registry: T,
        write_error: RegistryIoError,
    },
    ReadOnlyBuiltins(RegistryLoadError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardProfile {
    pub map: HashMap<EChannel, Vec<KeyCode>>,
}

impl Default for KeyboardProfile {
    fn default() -> Self {
        split_default_bindings().0
    }
}

impl KeyboardProfile {
    pub fn add_key(&mut self, channel: EChannel, key: KeyCode) {
        let keys = self.map.entry(channel).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    pub fn key_owners(&self, key: KeyCode) -> Vec<EChannel> {
        BINDABLE_CHANNELS
            .into_iter()
            .filter(|channel| {
                self.map
                    .get(channel)
                    .is_some_and(|keys| keys.contains(&key))
            })
            .collect()
    }
}

impl Serialize for KeyboardProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        channel_map(&self.map).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyboardProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            map: parse_channel_map(BTreeMap::<String, Vec<KeyCode>>::deserialize(deserializer)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MidiProfile {
    pub port: Option<String>,
    pub velocity_threshold: u8,
    pub map: HashMap<EChannel, Vec<u8>>,
}

impl Default for MidiProfile {
    fn default() -> Self {
        split_default_bindings().1
    }
}

impl MidiProfile {
    pub fn note_owner(&self, note: u8) -> Option<EChannel> {
        BINDABLE_CHANNELS.into_iter().find(|channel| {
            self.map
                .get(channel)
                .is_some_and(|notes| notes.contains(&note))
        })
    }

    /// Caller confirms before this replaces another channel's note binding.
    pub fn bind_note(&mut self, channel: EChannel, note: u8) {
        for notes in self.map.values_mut() {
            notes.retain(|bound| *bound != note);
        }
        let notes = self.map.entry(channel).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct MidiProfileDto {
    port: Option<String>,
    velocity_threshold: u8,
    map: BTreeMap<String, Vec<u8>>,
}

impl Default for MidiProfileDto {
    fn default() -> Self {
        Self {
            port: None,
            velocity_threshold: 0,
            map: BTreeMap::new(),
        }
    }
}

impl Serialize for MidiProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        MidiProfileDto {
            port: self.port.clone(),
            velocity_threshold: self.velocity_threshold,
            map: channel_map(&self.map),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MidiProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dto = MidiProfileDto::deserialize(deserializer)?;
        let mut owners = HashMap::new();
        for (name, notes) in &dto.map {
            if EChannel::from_short_name(name).is_none() {
                continue;
            }
            for note in notes {
                if let Some(owner) = owners.insert(*note, name) {
                    return Err(serde::de::Error::custom(format!(
                        "MIDI note {note} is bound to both {owner} and {name}"
                    )));
                }
            }
        }
        Ok(Self {
            port: dto.port.filter(|port| !port.is_empty()),
            velocity_threshold: dto.velocity_threshold,
            map: parse_channel_map(dto.map),
        })
    }
}

fn channel_map<T: Clone>(map: &HashMap<EChannel, Vec<T>>) -> BTreeMap<String, Vec<T>> {
    BINDABLE_CHANNELS
        .into_iter()
        .filter_map(|channel| {
            map.get(&channel).map(|values| {
                (
                    channel.short_name().unwrap_or_default().to_owned(),
                    values.clone(),
                )
            })
        })
        .collect()
}

fn parse_channel_map<T>(map: BTreeMap<String, Vec<T>>) -> HashMap<EChannel, Vec<T>> {
    map.into_iter()
        .filter_map(|(name, values)| {
            EChannel::from_short_name(&name).map(|channel| (channel, values))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
pub struct ProfileRegistry<T> {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, T>,
}

impl<T> Default for ProfileRegistry<T> {
    fn default() -> Self {
        Self {
            version: PROFILE_REGISTRY_VERSION,
            active: String::new(),
            profiles: BTreeMap::new(),
        }
    }
}

pub enum RegistryAction<T> {
    Select(String),
    Save(T),
    SaveAs { name: ProfileName, value: T },
    Rename(ProfileName),
    Delete,
    Revert,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("built-in profile cannot be modified: {0}")]
    BuiltInProfile(String),
    #[error("profile not found: {0}")]
    MissingProfile(String),
    #[error("profile already exists: {0}")]
    DuplicateProfile(String),
}

pub fn keyboard_builtins() -> BTreeMap<String, KeyboardProfile> {
    let (keyboard, _) = default_profiles();
    BTreeMap::from([(KEYBOARD_DEFAULT_NAME.to_owned(), keyboard)])
}

pub fn midi_builtins() -> BTreeMap<String, MidiProfile> {
    let (_, midi) = default_profiles();
    BTreeMap::from([(MIDI_DEFAULT_NAME.to_owned(), midi)])
}

pub fn keyboard_registry() -> ProfileRegistry<KeyboardProfile> {
    ProfileRegistry {
        active: KEYBOARD_DEFAULT_NAME.to_owned(),
        ..ProfileRegistry::default()
    }
}

pub fn midi_registry() -> ProfileRegistry<MidiProfile> {
    ProfileRegistry {
        active: MIDI_DEFAULT_NAME.to_owned(),
        ..ProfileRegistry::default()
    }
}

fn default_profiles() -> (KeyboardProfile, MidiProfile) {
    split_default_bindings()
}

fn split_default_bindings() -> (KeyboardProfile, MidiProfile) {
    let bindings = InputBindings::default();
    let mut keyboard = KeyboardProfile {
        map: HashMap::new(),
    };
    let mut midi = MidiProfile {
        port: bindings.midi.port,
        velocity_threshold: bindings.midi.velocity_threshold,
        map: HashMap::new(),
    };

    for (channel, bindings) in bindings.map {
        for binding in bindings {
            match binding {
                BindSource::Key(key) => keyboard.add_key(channel, key),
                BindSource::Midi { note } => midi.bind_note(channel, note),
            }
        }
    }

    (keyboard, midi)
}

pub fn reduce_registry<T: Clone + PartialEq>(
    registry: &ProfileRegistry<T>,
    builtins: &BTreeMap<String, T>,
    action: RegistryAction<T>,
) -> Result<ProfileRegistry<T>, RegistryError> {
    let mut updated = registry.clone();
    match action {
        RegistryAction::Select(name) => {
            if !builtins.contains_key(&name) && !updated.profiles.contains_key(&name) {
                return Err(RegistryError::MissingProfile(name));
            }
            updated.active = name;
        }
        RegistryAction::Save(value) => {
            if builtins.contains_key(&updated.active) {
                return Err(RegistryError::BuiltInProfile(updated.active));
            }
            let Some(profile) = updated.profiles.get_mut(&updated.active) else {
                return Err(RegistryError::MissingProfile(updated.active));
            };
            *profile = value;
        }
        RegistryAction::SaveAs { name, value } => {
            let name = name.as_str();
            if builtins.contains_key(name) {
                return Err(RegistryError::BuiltInProfile(name.to_owned()));
            }
            if updated.profiles.contains_key(name) {
                return Err(RegistryError::DuplicateProfile(name.to_owned()));
            }
            updated.profiles.insert(name.to_owned(), value);
            updated.active = name.to_owned();
        }
        RegistryAction::Rename(name) => {
            if builtins.contains_key(&updated.active) {
                return Err(RegistryError::BuiltInProfile(updated.active));
            }
            let old_name = updated.active.clone();
            let Some(value) = updated.profiles.remove(&old_name) else {
                return Err(RegistryError::MissingProfile(old_name));
            };
            let name = name.as_str();
            if builtins.contains_key(name) {
                return Err(RegistryError::BuiltInProfile(name.to_owned()));
            }
            if updated.profiles.contains_key(name) {
                return Err(RegistryError::DuplicateProfile(name.to_owned()));
            }
            updated.profiles.insert(name.to_owned(), value);
            updated.active = name.to_owned();
        }
        RegistryAction::Delete => {
            if builtins.contains_key(&updated.active) {
                return Err(RegistryError::BuiltInProfile(updated.active));
            }
            let active = updated.active.clone();
            if updated.profiles.remove(&active).is_none() {
                return Err(RegistryError::MissingProfile(active));
            }
            updated.active = builtins
                .keys()
                .next()
                .cloned()
                .ok_or(RegistryError::MissingProfile(active))?;
        }
        RegistryAction::Revert => {}
    }
    Ok(updated)
}

fn validate_registry<T>(
    path: &Path,
    registry: &ProfileRegistry<T>,
    builtins: &BTreeMap<String, T>,
) -> Result<(), RegistryLoadError> {
    if registry.version != PROFILE_REGISTRY_VERSION {
        return Err(RegistryLoadError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: registry.version,
        });
    }
    let names: Vec<&str> = registry.profiles.keys().map(String::as_str).collect();
    for (name, _) in &registry.profiles {
        let existing = names.iter().copied().filter(|other| *other != name);
        if let Err(error) =
            validate_profile_name(name, builtins.keys().map(String::as_str), existing, None)
        {
            return Err(RegistryLoadError::Invalid {
                path: path.to_path_buf(),
                reason: format!("profile {name:?}: {error}"),
            });
        }
    }
    if !builtins.contains_key(&registry.active) && !registry.profiles.contains_key(&registry.active)
    {
        return Err(RegistryLoadError::Invalid {
            path: path.to_path_buf(),
            reason: format!("active profile {:?} does not exist", registry.active),
        });
    }
    Ok(())
}

fn read_registry<T>(path: &Path, builtins: &BTreeMap<String, T>) -> CheckedLoad<ProfileRegistry<T>>
where
    T: for<'de> Deserialize<'de> + Default,
{
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(source)
            if matches!(
                source.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return CheckedLoad::Missing;
        }
        Err(source) => {
            return CheckedLoad::Malformed(RegistryLoadError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let registry = match toml::from_str::<ProfileRegistry<T>>(&raw) {
        Ok(registry) => registry,
        Err(source) => {
            return CheckedLoad::Malformed(RegistryLoadError::Parse {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    match validate_registry(path, &registry, builtins) {
        Ok(()) => CheckedLoad::Loaded(registry),
        Err(error) => CheckedLoad::Malformed(error),
    }
}

fn write_registry<T>(
    path: &Path,
    registry: &ProfileRegistry<T>,
    builtins: &BTreeMap<String, T>,
) -> Result<(), RegistryIoError>
where
    T: Serialize + Clone,
{
    let registry = registry.clone();
    validate_registry(path, &registry, builtins).map_err(|error| RegistryIoError::Invalid {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let bytes = toml::to_string_pretty(&registry)?;
    replace_bytes(path, bytes.as_bytes())?;
    Ok(())
}

fn migrate_name(base: &str, builtins: &BTreeMap<String, impl Sized>) -> String {
    if builtins.contains_key(base) {
        suggest_copy_name(base, builtins.keys().map(String::as_str))
    } else {
        base.to_owned()
    }
}

fn legacy_bindings(path: &Path) -> Result<BindingsFile, RegistryLoadError> {
    let raw = std::fs::read_to_string(path).map_err(|source| RegistryLoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    crate::bindings::parse_bindings_checked(&raw).map_err(|source| match source {
        crate::ConfigError::Parse(source) => RegistryLoadError::Parse {
            path: path.to_path_buf(),
            source,
        },
        _ => unreachable!("checked bindings parser only parses TOML"),
    })
}

fn migrated_keyboard_registry(file: &BindingsFile) -> ProfileRegistry<KeyboardProfile> {
    let keyboard = split_default_bindings_from(file).0;
    let builtins = keyboard_builtins();
    if keyboard == builtins[KEYBOARD_DEFAULT_NAME] {
        keyboard_registry()
    } else {
        let name = migrate_name("Migrated keyboard", &builtins);
        ProfileRegistry {
            active: name.clone(),
            profiles: [(name, keyboard)].into_iter().collect(),
            ..ProfileRegistry::default()
        }
    }
}

fn migrated_midi_registry(file: &BindingsFile) -> ProfileRegistry<MidiProfile> {
    let midi = split_default_bindings_from(file).1;
    let builtins = midi_builtins();
    if midi == builtins[MIDI_DEFAULT_NAME] {
        midi_registry()
    } else {
        let name = migrate_name("Migrated MIDI", &builtins);
        ProfileRegistry {
            active: name.clone(),
            profiles: [(name, midi)].into_iter().collect(),
            ..ProfileRegistry::default()
        }
    }
}

fn split_default_bindings_from(file: &BindingsFile) -> (KeyboardProfile, MidiProfile) {
    let resolved = file.resolve();
    let mut keyboard = KeyboardProfile {
        map: HashMap::new(),
    };
    let mut midi = MidiProfile {
        port: resolved.midi.port,
        velocity_threshold: resolved.midi.velocity_threshold,
        map: HashMap::new(),
    };
    for (channel, sources) in resolved.map {
        for source in sources {
            match source {
                BindSource::Key(key) => keyboard.add_key(channel, key),
                BindSource::Midi { note } => midi.bind_note(channel, note),
            }
        }
    }
    (keyboard, midi)
}

fn load_registry_or_migrate<T>(
    path: &Path,
    legacy: &Path,
    builtins: &BTreeMap<String, T>,
    default: impl FnOnce() -> ProfileRegistry<T>,
    migrate: impl FnOnce(&BindingsFile) -> ProfileRegistry<T>,
) -> RegistryStartup<ProfileRegistry<T>>
where
    T: for<'de> Deserialize<'de> + Serialize + Clone + Default,
{
    match read_registry(path, builtins) {
        CheckedLoad::Loaded(registry) => RegistryStartup::Ready(registry),
        CheckedLoad::Malformed(error) => RegistryStartup::ReadOnlyBuiltins(error),
        CheckedLoad::Missing => match legacy_bindings(legacy) {
            Ok(file) => {
                let registry = migrate(&file);
                match write_registry(path, &registry, builtins) {
                    Ok(()) => RegistryStartup::Ready(registry),
                    Err(write_error) => RegistryStartup::LegacySession {
                        registry,
                        write_error,
                    },
                }
            }
            Err(RegistryLoadError::Read { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                RegistryStartup::Ready(default())
            }
            Err(error) => RegistryStartup::ReadOnlyBuiltins(error),
        },
    }
}

pub fn load_keyboard_registry(
    path: &Path,
    legacy: &Path,
) -> RegistryStartup<ProfileRegistry<KeyboardProfile>> {
    load_registry_or_migrate(
        path,
        legacy,
        &keyboard_builtins(),
        keyboard_registry,
        migrated_keyboard_registry,
    )
}

pub fn load_midi_registry(
    path: &Path,
    legacy: &Path,
) -> RegistryStartup<ProfileRegistry<MidiProfile>> {
    load_registry_or_migrate(
        path,
        legacy,
        &midi_builtins(),
        midi_registry,
        migrated_midi_registry,
    )
}

pub fn save_keyboard_registry(
    path: &Path,
    registry: &ProfileRegistry<KeyboardProfile>,
) -> Result<(), RegistryIoError> {
    write_registry(path, registry, &keyboard_builtins())
}

pub fn save_midi_registry(
    path: &Path,
    registry: &ProfileRegistry<MidiProfile>,
) -> Result<(), RegistryIoError> {
    write_registry(path, registry, &midi_builtins())
}

fn backup_and_reset<T: Serialize + Clone>(
    path: &Path,
    confirmed: bool,
    now: SystemTime,
    registry: &ProfileRegistry<T>,
    builtins: &BTreeMap<String, T>,
) -> Result<(), RegistryIoError> {
    if !confirmed {
        return Err(RegistryIoError::ConfirmationRequired {
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
            .unwrap_or("profiles.toml");
        let backup = path.with_file_name(format!("{file_name}.backup-{stamp}"));
        std::fs::rename(path, &backup).map_err(|source| RegistryIoError::Backup {
            path: path.to_path_buf(),
            source,
        })?;
    }
    write_registry(path, registry, builtins)
}

pub fn backup_and_reset_keyboard_registry(
    path: &Path,
    confirmed: bool,
    now: SystemTime,
) -> Result<ProfileRegistry<KeyboardProfile>, RegistryIoError> {
    let registry = keyboard_registry();
    backup_and_reset(path, confirmed, now, &registry, &keyboard_builtins())?;
    Ok(registry)
}

pub fn backup_and_reset_midi_registry(
    path: &Path,
    confirmed: bool,
    now: SystemTime,
) -> Result<ProfileRegistry<MidiProfile>, RegistryIoError> {
    let registry = midi_registry();
    backup_and_reset(path, confirmed, now, &registry, &midi_builtins())?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use dtx_core::EChannel;
    use dtx_input::KeyCode;
    use dtx_persistence::validate_profile_name;

    use super::*;

    fn user_name(raw: &str) -> dtx_persistence::ProfileName {
        validate_profile_name(raw, [KEYBOARD_DEFAULT_NAME, MIDI_DEFAULT_NAME], [], None)
            .expect("test profile name is valid")
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct NonDefaultPayload(String);

    #[test]
    fn registry_round_trips_non_default_payload() {
        let registry = ProfileRegistry {
            version: PROFILE_REGISTRY_VERSION,
            active: "Desk".to_owned(),
            profiles: [("Desk".to_owned(), NonDefaultPayload("value".to_owned()))]
                .into_iter()
                .collect(),
        };

        let raw = toml::to_string(&registry).expect("registry serializes");
        let parsed: ProfileRegistry<NonDefaultPayload> =
            toml::from_str(&raw).expect("registry parses");
        assert_eq!(parsed, registry);
    }

    #[test]
    fn keyboard_registry_round_trips_spec_shape() {
        let mut registry = ProfileRegistry::default();
        registry.active = "Desk".to_owned();
        registry.profiles.insert(
            "Desk".to_owned(),
            KeyboardProfile {
                map: [(EChannel::HiHatClose, vec![KeyCode::KeyX, KeyCode::KeyC])]
                    .into_iter()
                    .collect(),
            },
        );

        let raw = toml::to_string_pretty(&registry).expect("registry serializes");
        let value: toml::Value = toml::from_str(&raw).expect("serialized TOML parses");
        let profile = &value["profiles"]["Desk"];
        assert_eq!(profile["HH"].as_array().expect("HH is an array").len(), 2);
        assert!(profile.get("map").is_none());
        assert!(!raw.contains("Midi"));
        let parsed: ProfileRegistry<KeyboardProfile> =
            toml::from_str(&raw).expect("registry parses");
        assert_eq!(parsed, registry);
    }

    #[test]
    fn midi_registry_round_trips_spec_shape() {
        let mut registry = ProfileRegistry::default();
        registry.active = "Roland TD-17".to_owned();
        registry.profiles.insert(
            "Roland TD-17".to_owned(),
            MidiProfile {
                port: Some("TD-17".to_owned()),
                velocity_threshold: 12,
                map: [(EChannel::Snare, vec![38]), (EChannel::HiHatOpen, vec![46])]
                    .into_iter()
                    .collect(),
            },
        );

        let raw = toml::to_string_pretty(&registry).expect("registry serializes");
        let value: toml::Value = toml::from_str(&raw).expect("serialized TOML parses");
        let profile = &value["profiles"]["Roland TD-17"];
        assert_eq!(profile["port"].as_str(), Some("TD-17"));
        assert_eq!(profile["velocity_threshold"].as_integer(), Some(12));
        assert_eq!(
            profile["map"]["SD"]
                .as_array()
                .expect("SD is an array")
                .len(),
            1
        );
        assert!(!raw.contains("Midi"));
        let parsed: ProfileRegistry<MidiProfile> = toml::from_str(&raw).expect("registry parses");
        assert_eq!(parsed, registry);
    }

    #[test]
    fn missing_and_newer_registry_versions_are_distinct() {
        let missing: ProfileRegistry<KeyboardProfile> =
            toml::from_str("active = \"Desk\"").expect("missing version parses");
        let newer: ProfileRegistry<KeyboardProfile> =
            toml::from_str("version = 2\nactive = \"Desk\"").expect("newer version parses");

        assert_eq!(missing.version, 0);
        assert_eq!(newer.version, 2);
    }

    #[test]
    fn keyboard_key_can_exist_under_multiple_channels() {
        let mut profile = KeyboardProfile::default();
        profile.add_key(EChannel::Snare, KeyCode::KeyX);

        assert_eq!(
            profile.key_owners(KeyCode::KeyX),
            vec![EChannel::HiHatClose, EChannel::Snare]
        );
    }

    #[test]
    fn midi_note_conflict_reports_owner() {
        let profile = MidiProfile::default();

        assert_eq!(profile.note_owner(42), Some(EChannel::HiHatClose));
    }

    #[test]
    fn save_builtin_is_rejected() {
        let registry = keyboard_registry();
        let error = reduce_registry(
            &registry,
            &keyboard_builtins(),
            RegistryAction::Save(KeyboardProfile::default()),
        )
        .expect_err("built-in cannot save");

        assert!(matches!(error, RegistryError::BuiltInProfile(_)));
    }

    #[test]
    fn rename_moves_key_and_active_together() {
        let mut registry = keyboard_registry();
        registry.active = "Desk".to_owned();
        registry
            .profiles
            .insert("Desk".to_owned(), KeyboardProfile::default());

        let updated = reduce_registry(
            &registry,
            &keyboard_builtins(),
            RegistryAction::Rename(user_name("Studio")),
        )
        .expect("rename succeeds");

        assert_eq!(updated.active, "Studio");
        assert!(!updated.profiles.contains_key("Desk"));
        assert!(updated.profiles.contains_key("Studio"));
    }

    #[test]
    fn delete_active_selects_builtin_fallback() {
        let mut registry = midi_registry();
        registry.active = "Kit".to_owned();
        registry
            .profiles
            .insert("Kit".to_owned(), MidiProfile::default());

        let updated = reduce_registry(&registry, &midi_builtins(), RegistryAction::Delete)
            .expect("delete succeeds");

        assert_eq!(updated.active, MIDI_DEFAULT_NAME);
        assert!(updated.profiles.is_empty());
    }

    #[test]
    fn revert_is_registry_noop() {
        let mut registry = keyboard_registry();
        registry.active = "Desk".to_owned();
        let saved = KeyboardProfile {
            map: [(EChannel::Snare, vec![KeyCode::KeyD])]
                .into_iter()
                .collect(),
        };
        registry.profiles.insert("Desk".to_owned(), saved.clone());

        let reverted = reduce_registry(&registry, &keyboard_builtins(), RegistryAction::Revert)
            .expect("revert succeeds");

        assert_eq!(reverted.profiles["Desk"], saved);
    }

    #[test]
    fn empty_midi_port_normalizes_to_none() {
        let profile: MidiProfile =
            toml::from_str("port = \"\"\nvelocity_threshold = 0\n[map]").expect("profile parses");

        assert_eq!(profile.port, None);
    }

    #[test]
    fn midi_profile_rejects_note_bound_to_multiple_channels() {
        let error =
            toml::from_str::<MidiProfile>("velocity_threshold = 0\n[map]\nHH = [42]\nSD = [42]")
                .expect_err("duplicate MIDI note must fail");

        assert!(error
            .to_string()
            .contains("MIDI note 42 is bound to both HH and SD"));
    }

    #[test]
    fn midi_profile_rejects_note_repeated_within_channel() {
        let error = toml::from_str::<MidiProfile>("velocity_threshold = 0\n[map]\nHH = [42, 42]")
            .expect_err("duplicate MIDI note must fail");

        assert!(error
            .to_string()
            .contains("MIDI note 42 is bound to both HH and HH"));
    }

    #[test]
    fn save_as_uses_owned_profile_name_key() {
        let updated = reduce_registry(
            &keyboard_registry(),
            &keyboard_builtins(),
            RegistryAction::SaveAs {
                name: user_name("Desk"),
                value: KeyboardProfile::default(),
            },
        )
        .expect("save as succeeds");

        assert_eq!(updated.active, "Desk");
        assert!(updated.profiles.contains_key("Desk"));
    }

    #[test]
    fn save_as_rejects_existing_user_profile() {
        let mut registry = keyboard_registry();
        registry
            .profiles
            .insert("Desk".to_owned(), KeyboardProfile::default());

        let error = reduce_registry(
            &registry,
            &keyboard_builtins(),
            RegistryAction::SaveAs {
                name: user_name("Desk"),
                value: KeyboardProfile::default(),
            },
        )
        .expect_err("existing profile cannot be overwritten");

        assert_eq!(error, RegistryError::DuplicateProfile("Desk".to_owned()));
    }

    #[test]
    fn builtins_do_not_enter_user_profiles() {
        let builtins: BTreeMap<_, _> = keyboard_builtins();
        let registry = keyboard_registry();

        assert!(registry.profiles.is_empty());
        assert!(builtins.contains_key(KEYBOARD_DEFAULT_NAME));
    }

    fn migration_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join("dtx-config-profile-tests")
            .join(std::process::id().to_string())
            .join(name)
    }

    #[test]
    fn checked_legacy_load_distinguishes_missing_and_malformed() {
        let root = migration_dir("missing");
        let _ = std::fs::remove_dir_all(&root);
        let startup = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        assert!(
            matches!(startup, RegistryStartup::Ready(registry) if registry.active == KEYBOARD_DEFAULT_NAME)
        );

        std::fs::create_dir_all(&root).expect("test directory creates");
        std::fs::write(root.join("bindings.toml"), "this is not valid = [[toml")
            .expect("legacy writes");
        let startup = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        assert!(matches!(
            startup,
            RegistryStartup::ReadOnlyBuiltins(RegistryLoadError::Parse { .. })
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mixed_v1_bindings_partition_migration_preserves_device_fields() {
        let root = migration_dir("mixed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let mut bindings = InputBindings::default();
        bindings.midi.port = Some("TD-17".to_owned());
        bindings.midi.velocity_threshold = 12;
        bindings.bind_shared(EChannel::Snare, BindSource::Key(KeyCode::KeyX));
        std::fs::write(
            root.join("bindings.toml"),
            toml::to_string_pretty(&bindings.to_file()).expect("legacy serializes"),
        )
        .expect("legacy writes");

        let keyboard = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        let midi = load_midi_registry(
            &root.join("midi-profiles.toml"),
            &root.join("bindings.toml"),
        );
        let keyboard = match keyboard {
            RegistryStartup::Ready(registry) => registry,
            other => panic!("unexpected keyboard startup: {other:?}"),
        };
        let midi = match midi {
            RegistryStartup::Ready(registry) => registry,
            other => panic!("unexpected MIDI startup: {other:?}"),
        };
        assert!(keyboard.active.starts_with("Migrated keyboard"));
        assert!(midi.active.starts_with("Migrated MIDI"));
        assert_eq!(midi.profiles[&midi.active].port.as_deref(), Some("TD-17"));
        assert_eq!(midi.profiles[&midi.active].velocity_threshold, 12);
        assert!(keyboard.profiles[&keyboard.active]
            .key_owners(KeyCode::KeyX)
            .contains(&EChannel::Snare));
        assert!(root.join("bindings.toml").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn matching_legacy_halves_activate_builtins() {
        let root = migration_dir("matching");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let legacy =
            toml::to_string_pretty(&InputBindings::default().to_file()).expect("legacy serializes");
        std::fs::write(root.join("bindings.toml"), legacy).expect("legacy writes");

        let keyboard = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        let midi = load_midi_registry(
            &root.join("midi-profiles.toml"),
            &root.join("bindings.toml"),
        );
        assert!(
            matches!(keyboard, RegistryStartup::Ready(registry) if registry.active == KEYBOARD_DEFAULT_NAME && registry.profiles.is_empty())
        );
        assert!(
            matches!(midi, RegistryStartup::Ready(registry) if registry.active == MIDI_DEFAULT_NAME && registry.profiles.is_empty())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn changed_halves_get_migrated_names() {
        let root = migration_dir("changed-names");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let mut bindings = InputBindings::default();
        bindings.bind(EChannel::Snare, BindSource::Key(KeyCode::KeyQ));
        bindings.bind(EChannel::Snare, BindSource::Midi { note: 1 });
        std::fs::write(
            root.join("bindings.toml"),
            toml::to_string_pretty(&bindings.to_file()).expect("legacy serializes"),
        )
        .expect("legacy writes");

        let keyboard = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        let midi = load_midi_registry(
            &root.join("midi-profiles.toml"),
            &root.join("bindings.toml"),
        );
        assert!(
            matches!(keyboard, RegistryStartup::Ready(registry) if registry.active == "Migrated keyboard")
        );
        assert!(
            matches!(midi, RegistryStartup::Ready(registry) if registry.active == "Migrated MIDI")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_write_failure_retries_when_registry_remains_missing() {
        let root = migration_dir("retry");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let legacy = root.join("bindings.toml");
        std::fs::write(
            &legacy,
            toml::to_string_pretty(&InputBindings::default().to_file()).expect("legacy serializes"),
        )
        .expect("legacy writes");
        let blocked = root.join("blocked");
        std::fs::write(&blocked, "not a directory").expect("blocker writes");
        let registry = blocked.join("keyboard-profiles.toml");

        assert!(matches!(
            load_keyboard_registry(&registry, &legacy),
            RegistryStartup::LegacySession { .. }
        ));
        assert!(!registry.exists());
        std::fs::remove_file(&blocked).expect("blocker removes");
        std::fs::create_dir(&blocked).expect("registry parent creates");
        assert!(matches!(
            load_keyboard_registry(&registry, &legacy),
            RegistryStartup::Ready(_)
        ));
        assert!(registry.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_registry_cannot_be_saved() {
        let root = migration_dir("corrupt-save");
        let _ = std::fs::remove_dir_all(&root);
        let registry = ProfileRegistry {
            active: "missing".to_owned(),
            ..ProfileRegistry::default()
        };

        assert!(matches!(
            save_keyboard_registry(&root.join("keyboard-profiles.toml"), &registry),
            Err(RegistryIoError::Invalid { .. })
        ));
        assert!(!root.join("keyboard-profiles.toml").exists());
    }

    #[test]
    fn existing_registry_skips_legacy() {
        let root = migration_dir("existing");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let registry = ProfileRegistry {
            active: "Desk".to_owned(),
            profiles: [("Desk".to_owned(), KeyboardProfile::default())]
                .into_iter()
                .collect(),
            ..ProfileRegistry::default()
        };
        save_keyboard_registry(&root.join("keyboard-profiles.toml"), &registry)
            .expect("registry writes");
        std::fs::write(root.join("bindings.toml"), "not migrated").expect("legacy writes");
        let startup = load_keyboard_registry(
            &root.join("keyboard-profiles.toml"),
            &root.join("bindings.toml"),
        );
        assert!(matches!(startup, RegistryStartup::Ready(registry) if registry.active == "Desk"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reset_requires_confirmation_and_preserves_timestamped_backup() {
        let root = migration_dir("reset");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let path = root.join("keyboard-profiles.toml");
        save_keyboard_registry(&path, &keyboard_registry()).expect("registry writes");
        assert!(matches!(
            backup_and_reset_keyboard_registry(&path, false, UNIX_EPOCH),
            Err(RegistryIoError::ConfirmationRequired { .. })
        ));
        let reset =
            backup_and_reset_keyboard_registry(&path, true, UNIX_EPOCH).expect("reset succeeds");
        assert_eq!(reset.active, KEYBOARD_DEFAULT_NAME);
        let backups = std::fs::read_dir(&root)
            .expect("directory reads")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("backup-"))
            .count();
        assert_eq!(backups, 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
