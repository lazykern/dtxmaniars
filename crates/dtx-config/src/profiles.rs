use std::collections::{BTreeMap, HashMap};

use dtx_core::EChannel;
use dtx_input::KeyCode;
use dtx_persistence::ProfileName;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{BindSource, InputBindings, BINDABLE_CHANNELS};

pub const KEYBOARD_DEFAULT_NAME: &str = "DTXMania default";
pub const MIDI_DEFAULT_NAME: &str = "General MIDI drums";
pub const PROFILE_REGISTRY_VERSION: u32 = 1;

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
                    if owner != name {
                        return Err(serde::de::Error::custom(format!(
                            "MIDI note {note} is bound to both {owner} and {name}"
                        )));
                    }
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
}
