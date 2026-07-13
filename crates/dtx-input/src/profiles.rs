use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::KeyCode;
use dtx_core::EChannel;
use dtx_persistence::{
    replace_bytes, suggest_copy_name, validate_profile_name, PersistenceError, ProfileName,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::bindings::{
    BindSource, BindingsFile, InputBindings, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS,
};

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
    /// System verbs bound to keys. Serialized under the profile's `system`
    /// table; absent in older files (`old_profile_without_system_table_loads_empty`).
    pub system: HashMap<SystemVerb, Vec<KeyCode>>,
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

    /// Bind `key` to `verb`. Never steals from a lane — the caller refuses a
    /// lane-owned key up front (`bindings::lane_owner`).
    pub fn add_system_key(&mut self, verb: SystemVerb, key: KeyCode) {
        let keys = self.system.entry(verb).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
}

impl Serialize for KeyboardProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        // Channel arrays, then the `system` sub-table — the order the file reads
        // best in. Cosmetic only: toml emits a table's values before its
        // sub-tables whatever order they are serialized in.
        let channels = channel_map(&self.map);
        let system = verb_map(&self.system);
        let mut map =
            serializer.serialize_map(Some(channels.len() + usize::from(!system.is_empty())))?;
        for (name, keys) in &channels {
            map.serialize_entry(name, keys)?;
        }
        if !system.is_empty() {
            map.serialize_entry("system", &system)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for KeyboardProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// A profile entry is either a channel's key array or the `system` table.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Entry {
            Keys(Vec<KeyCode>),
            System(BTreeMap<String, Vec<KeyCode>>),
        }
        let raw = BTreeMap::<String, Entry>::deserialize(deserializer)?;
        let mut channels: BTreeMap<String, Vec<KeyCode>> = BTreeMap::new();
        let mut system: BTreeMap<String, Vec<KeyCode>> = BTreeMap::new();
        for (name, entry) in raw {
            match (name.as_str(), entry) {
                ("system", Entry::System(table)) => system = table,
                // `system = ["F9"]` parses as a key array, so it would otherwise
                // land in `channels` and be dropped as an unknown channel.
                ("system", Entry::Keys(_)) => eprintln!(
                    "dtx-input: keyboard profile `system` must be a table of verb = [keys]; dropped"
                ),
                (_, Entry::Keys(keys)) => {
                    channels.insert(name, keys);
                }
                (_, Entry::System(_)) => {
                    eprintln!("dtx-input: keyboard profile unknown table {name:?}; skipped")
                }
            }
        }
        Ok(Self {
            map: parse_channel_map(channels),
            system: parse_verb_map(system),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MidiProfile {
    pub port: Option<String>,
    pub velocity_threshold: u8,
    pub map: HashMap<EChannel, Vec<u8>>,
    /// System verbs bound to MIDI notes. A spare zone note (xstick 37, ride
    /// bell 53, HH edge 22/26) costs no gameplay pad.
    pub system: HashMap<SystemVerb, Vec<u8>>,
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

    /// Append `note` to `channel` without removing it from other channels.
    pub fn bind_note_shared(&mut self, channel: EChannel, note: u8) {
        let notes = self.map.entry(channel).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }

    /// Bind `note` to `verb`. Never steals from a lane — the caller refuses a
    /// lane-owned note up front (`bindings::lane_owner`).
    pub fn bind_system_note(&mut self, verb: SystemVerb, note: u8) {
        let notes = self.system.entry(verb).or_default();
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct MidiProfileDto {
    port: Option<String>,
    velocity_threshold: u8,
    map: BTreeMap<String, Vec<u8>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    system: BTreeMap<String, Vec<u8>>,
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
            system: verb_map(&self.system),
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
        Ok(Self {
            port: dto.port.filter(|port| !port.is_empty()),
            velocity_threshold: dto.velocity_threshold,
            map: parse_channel_map(dto.map),
            system: parse_verb_map(dto.system),
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

/// Deserialize a channel map, warning on unknown channel names and deduping
/// values within each channel (a key/note bound twice to one channel must
/// fire that lane once). Cross-channel sharing is preserved.
fn parse_channel_map<T: PartialEq>(map: BTreeMap<String, Vec<T>>) -> HashMap<EChannel, Vec<T>> {
    map.into_iter()
        .filter_map(|(name, values)| {
            let Some(channel) = EChannel::from_short_name(&name) else {
                eprintln!("dtx-input: profile unknown channel {name:?}; skipped");
                return None;
            };
            Some((channel, dedup(values)))
        })
        .collect()
}

/// Deserialize a verb map, warning on unknown verb keys and deduping within
/// each verb (a key/note bound twice to one verb must fire it once).
fn parse_verb_map<T: PartialEq>(map: BTreeMap<String, Vec<T>>) -> HashMap<SystemVerb, Vec<T>> {
    map.into_iter()
        .filter_map(|(name, values)| {
            let Some(verb) = SystemVerb::from_key(&name) else {
                eprintln!("dtx-input: profile unknown system verb {name:?}; skipped");
                return None;
            };
            Some((verb, dedup(values)))
        })
        .collect()
}

fn dedup<T: PartialEq>(values: Vec<T>) -> Vec<T> {
    let mut unique = Vec::with_capacity(values.len());
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

/// Serialize a verb map with stable, brand-independent keys (`SystemVerb::key`).
fn verb_map<T: Clone>(map: &HashMap<SystemVerb, Vec<T>>) -> BTreeMap<String, Vec<T>> {
    SYSTEM_VERBS
        .into_iter()
        .filter_map(|verb| {
            map.get(&verb)
                .filter(|values| !values.is_empty())
                .map(|values| (verb.key().to_owned(), values.clone()))
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
    split_bindings(&InputBindings::default())
}

/// Partition channel-keyed bindings into independent keyboard and MIDI
/// profiles. Keys and notes may each be shared across channels
/// (`add_key` / `bind_note_shared`), deduped within each channel.
pub fn split_bindings(bindings: &InputBindings) -> (KeyboardProfile, MidiProfile) {
    let mut keyboard = KeyboardProfile {
        map: HashMap::new(),
        system: HashMap::new(),
    };
    let mut midi = MidiProfile {
        port: bindings.midi.port.clone(),
        velocity_threshold: bindings.midi.velocity_threshold,
        map: HashMap::new(),
        system: HashMap::new(),
    };

    for (channel, sources) in &bindings.map {
        for source in sources {
            match source {
                BindSource::Key(key) => keyboard.add_key(*channel, *key),
                BindSource::Midi { note } => midi.bind_note_shared(*channel, *note),
            }
        }
    }
    for (verb, sources) in &bindings.system {
        for source in sources {
            match source {
                BindSource::Key(key) => keyboard.add_system_key(*verb, *key),
                BindSource::Midi { note } => midi.bind_system_note(*verb, *note),
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
    for name in registry.profiles.keys() {
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
        dtx_config::ConfigError::Parse(source) => RegistryLoadError::Parse {
            path: path.to_path_buf(),
            source,
        },
        _ => unreachable!("checked bindings parser only parses TOML"),
    })
}

fn migrated_keyboard_registry(
    file: &BindingsFile,
) -> Result<ProfileRegistry<KeyboardProfile>, String> {
    let keyboard = split_default_bindings_from(file)?.0;
    let builtins = keyboard_builtins();
    Ok(if keyboard == builtins[KEYBOARD_DEFAULT_NAME] {
        keyboard_registry()
    } else {
        let name = migrate_name("Migrated keyboard", &builtins);
        ProfileRegistry {
            active: name.clone(),
            profiles: [(name, keyboard)].into_iter().collect(),
            ..ProfileRegistry::default()
        }
    })
}

fn migrated_midi_registry(file: &BindingsFile) -> Result<ProfileRegistry<MidiProfile>, String> {
    let midi = split_default_bindings_from(file)?.1;
    let builtins = midi_builtins();
    Ok(if midi == builtins[MIDI_DEFAULT_NAME] {
        midi_registry()
    } else {
        let name = migrate_name("Migrated MIDI", &builtins);
        ProfileRegistry {
            active: name.clone(),
            profiles: [(name, midi)].into_iter().collect(),
            ..ProfileRegistry::default()
        }
    })
}

fn split_default_bindings_from(
    file: &BindingsFile,
) -> Result<(KeyboardProfile, MidiProfile), String> {
    // `resolve` fans a shared key/note out to every owning channel and dedups
    // within each; `split_bindings` partitions that into the two profiles.
    // Kept fallible so the migrate closures in `load_registry_or_migrate` still
    // type-check, but shared bindings are now valid and never rejected.
    Ok(split_bindings(&file.resolve()))
}

fn load_registry_or_migrate<T>(
    path: &Path,
    legacy: &Path,
    builtins: &BTreeMap<String, T>,
    default: impl FnOnce() -> ProfileRegistry<T>,
    migrate: impl FnOnce(&BindingsFile) -> Result<ProfileRegistry<T>, String>,
) -> RegistryStartup<ProfileRegistry<T>>
where
    T: for<'de> Deserialize<'de> + Serialize + Clone + Default,
{
    match read_registry(path, builtins) {
        CheckedLoad::Loaded(registry) => RegistryStartup::Ready(registry),
        CheckedLoad::Malformed(error) => RegistryStartup::ReadOnlyBuiltins(error),
        CheckedLoad::Missing => match legacy_bindings(legacy) {
            Ok(file) => {
                let registry = match migrate(&file) {
                    Ok(registry) => registry,
                    Err(reason) => {
                        return RegistryStartup::ReadOnlyBuiltins(RegistryLoadError::Invalid {
                            path: legacy.to_path_buf(),
                            reason,
                        });
                    }
                };
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
        // hard_link fails atomically with AlreadyExists, so a concurrently
        // created backup can never be overwritten.
        std::fs::hard_link(path, &backup).map_err(|source| RegistryIoError::Backup {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::remove_file(path).map_err(|source| RegistryIoError::Backup {
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

    use crate::KeyCode;
    use dtx_core::EChannel;
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
        let mut registry = ProfileRegistry {
            active: "Desk".to_owned(),
            ..Default::default()
        };
        registry.profiles.insert(
            "Desk".to_owned(),
            KeyboardProfile {
                map: [(EChannel::HiHatClose, vec![KeyCode::KeyX, KeyCode::KeyC])]
                    .into_iter()
                    .collect(),
                system: [(SystemVerb::Pause, vec![KeyCode::F9])]
                    .into_iter()
                    .collect(),
            },
        );

        let raw = toml::to_string_pretty(&registry).expect("registry serializes");
        let value: toml::Value = toml::from_str(&raw).expect("serialized TOML parses");
        let profile = &value["profiles"]["Desk"];
        assert_eq!(profile["HH"].as_array().expect("HH is an array").len(), 2);
        assert!(profile.get("map").is_none());
        // The on-disk shape is nested: [profiles.Desk] + [profiles.Desk.system].
        assert_eq!(
            profile["system"]["pause"]
                .as_array()
                .expect("pause is an array")
                .len(),
            1
        );
        assert!(!raw.contains("Midi"));
        let parsed: ProfileRegistry<KeyboardProfile> =
            toml::from_str(&raw).expect("registry parses");
        assert_eq!(parsed, registry);
    }

    #[test]
    fn midi_registry_round_trips_spec_shape() {
        let mut registry = ProfileRegistry {
            active: "Roland TD-17".to_owned(),
            ..Default::default()
        };
        registry.profiles.insert(
            "Roland TD-17".to_owned(),
            MidiProfile {
                port: Some("TD-17".to_owned()),
                velocity_threshold: 12,
                map: [(EChannel::Snare, vec![38]), (EChannel::HiHatOpen, vec![46])]
                    .into_iter()
                    .collect(),
                system: [(SystemVerb::Restart, vec![37])].into_iter().collect(),
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
        // The on-disk shape is nested: [profiles."Roland TD-17".system].
        assert_eq!(
            profile["system"]["restart"]
                .as_array()
                .expect("restart is an array")
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
    fn midi_profile_allows_shared_note_and_round_trips() {
        let mut p = MidiProfile::default();
        p.bind_note_shared(EChannel::BassDrum, 36);
        p.bind_note_shared(EChannel::LeftBassDrum, 36);
        assert!(p.map[&EChannel::BassDrum].contains(&36));
        assert!(p.map[&EChannel::LeftBassDrum].contains(&36));
        let toml = toml::to_string(&p).unwrap();
        let back: MidiProfile = toml::from_str(&toml).unwrap();
        assert_eq!(back.map[&EChannel::BassDrum], p.map[&EChannel::BassDrum]);
        assert_eq!(
            back.map[&EChannel::LeftBassDrum],
            p.map[&EChannel::LeftBassDrum]
        );
    }

    #[test]
    fn bind_note_still_steals_for_move_semantics() {
        let mut p = MidiProfile::default();
        p.bind_note_shared(EChannel::BassDrum, 36);
        p.bind_note(EChannel::LeftBassDrum, 36);
        assert!(!p.map[&EChannel::BassDrum].contains(&36));
        assert!(p.map[&EChannel::LeftBassDrum].contains(&36));
    }

    #[test]
    fn split_bindings_preserves_shared_midi_note() {
        use crate::{BindSource, InputBindings};
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
        let (_kb, midi) = split_bindings(&b);
        assert!(midi.map[&EChannel::BassDrum].contains(&36));
        assert!(midi.map[&EChannel::LeftBassDrum].contains(&36));
    }

    #[test]
    fn keyboard_profile_round_trips_system_binds() {
        let mut profile = KeyboardProfile::default();
        profile.add_system_key(SystemVerb::Pause, KeyCode::F9);
        let raw = toml::to_string_pretty(&profile).expect("profile serializes");
        let back: KeyboardProfile = toml::from_str(&raw).expect("profile parses");
        assert_eq!(back.system[&SystemVerb::Pause], vec![KeyCode::F9]);
        assert_eq!(back.map, profile.map, "channel map survives");
    }

    #[test]
    fn midi_profile_round_trips_system_binds() {
        let mut profile = MidiProfile::default();
        profile.bind_system_note(SystemVerb::Pause, 37);
        let raw = toml::to_string_pretty(&profile).expect("profile serializes");
        let back: MidiProfile = toml::from_str(&raw).expect("profile parses");
        assert_eq!(back.system[&SystemVerb::Pause], vec![37]);
        assert_eq!(back.map, profile.map, "channel map survives");
    }

    #[test]
    fn old_profile_without_system_table_loads_empty() {
        let profile: KeyboardProfile =
            toml::from_str("HH = [\"KeyX\"]").expect("legacy keyboard profile parses");
        assert!(profile.system.is_empty());
        assert_eq!(profile.map[&EChannel::HiHatClose], vec![KeyCode::KeyX]);

        let profile: MidiProfile = toml::from_str("velocity_threshold = 0\n[map]\nHH = [42]")
            .expect("legacy MIDI profile parses");
        assert!(profile.system.is_empty());
        assert_eq!(profile.map[&EChannel::HiHatClose], vec![42]);
    }

    /// A hand-edited `system = ["F9"]` (array, not a verb table) is the wrong
    /// shape: it is dropped with a warning, and must not leak into the channel
    /// map — a bogus "system" channel would be erased on the next write anyway.
    #[test]
    fn malformed_system_array_is_dropped_not_taken_as_a_channel() {
        let profile: KeyboardProfile = toml::from_str("HH = [\"KeyX\"]\nsystem = [\"F9\"]")
            .expect("malformed system still parses");
        assert!(profile.system.is_empty());
        assert_eq!(profile.map[&EChannel::HiHatClose], vec![KeyCode::KeyX]);
        assert_eq!(profile.map.len(), 1, "no bogus channel from `system`");
    }

    #[test]
    fn unknown_system_verb_key_is_dropped() {
        let profile: KeyboardProfile =
            toml::from_str("[system]\nnope = [\"F9\"]\npause = [\"F8\"]")
                .expect("unknown verb still parses");
        assert_eq!(profile.system[&SystemVerb::Pause], vec![KeyCode::F8]);
        assert_eq!(profile.system.len(), 1);
    }

    #[test]
    fn split_bindings_partitions_system_binds_by_device() {
        use crate::{BindSource, InputBindings};
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let (keyboard, midi) = split_bindings(&b);
        assert_eq!(midi.system[&SystemVerb::Pause], vec![37]);
        assert_eq!(keyboard.system[&SystemVerb::Restart], vec![KeyCode::F9]);
        assert!(!keyboard.system.contains_key(&SystemVerb::Pause));
        assert!(!midi.system.contains_key(&SystemVerb::Restart));
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
            system: HashMap::new(),
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
    fn midi_profile_allows_note_bound_to_multiple_channels_from_toml() {
        let profile =
            toml::from_str::<MidiProfile>("velocity_threshold = 0\n[map]\nHH = [42]\nSD = [42]")
                .expect("shared MIDI note parses");

        assert!(profile.map[&EChannel::HiHatClose].contains(&42));
        assert!(profile.map[&EChannel::Snare].contains(&42));
    }

    #[test]
    fn midi_profile_dedupes_note_repeated_within_channel_from_toml() {
        let profile = toml::from_str::<MidiProfile>("velocity_threshold = 0\n[map]\nHH = [42, 42]")
            .expect("repeated MIDI note parses");

        assert_eq!(profile.map[&EChannel::HiHatClose], vec![42]);
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
    fn shared_legacy_midi_migrates_note_under_every_channel() {
        let root = migration_dir("shared-midi");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let legacy = root.join("bindings.toml");
        // note 36 is BassDrum by default; also share it onto LeftBassDrum.
        let mut file = InputBindings::default().to_file();
        file.map
            .entry("LBD".to_owned())
            .or_default()
            .push(BindSource::Midi { note: 36 });
        std::fs::write(
            &legacy,
            toml::to_string_pretty(&file).expect("legacy serializes"),
        )
        .expect("legacy writes");
        let midi_path = root.join("midi-profiles.toml");

        let midi = match load_midi_registry(&midi_path, &legacy) {
            RegistryStartup::Ready(registry) => registry,
            other => panic!("expected successful migration, got {other:?}"),
        };
        let profile = &midi.profiles[&midi.active];
        assert!(profile.map[&EChannel::BassDrum].contains(&36));
        assert!(profile.map[&EChannel::LeftBassDrum].contains(&36));
        assert!(midi_path.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reset_requires_confirmation_and_preserves_timestamped_backup() {
        let root = migration_dir("reset");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let path = root.join("keyboard-profiles.toml");
        save_keyboard_registry(&path, &keyboard_registry()).expect("registry writes");
        let original = std::fs::read_to_string(&path).expect("original registry reads");
        assert!(matches!(
            backup_and_reset_keyboard_registry(&path, false, UNIX_EPOCH),
            Err(RegistryIoError::ConfirmationRequired { .. })
        ));
        let reset =
            backup_and_reset_keyboard_registry(&path, true, UNIX_EPOCH).expect("reset succeeds");
        assert_eq!(reset.active, KEYBOARD_DEFAULT_NAME);
        let backups: Vec<_> = std::fs::read_dir(&root)
            .expect("directory reads")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).expect("backup reads"),
            original
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reset_rejects_existing_timestamped_backup_without_overwriting() {
        let root = migration_dir("reset-backup-collision");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory creates");
        let path = root.join("keyboard-profiles.toml");
        let backup = root.join("keyboard-profiles.toml.backup-0");
        std::fs::write(&path, "current registry").expect("current registry writes");
        std::fs::write(&backup, "existing backup").expect("backup writes");

        assert!(matches!(
            backup_and_reset_keyboard_registry(&path, true, UNIX_EPOCH),
            Err(RegistryIoError::Backup { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(
            std::fs::read_to_string(&path).expect("current registry reads"),
            "current registry"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).expect("backup reads"),
            "existing backup"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
