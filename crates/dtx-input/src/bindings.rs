//! Input bindings — `bindings.toml` schema + runtime types.
//!
//! Design: docs/superpowers/specs/2026-07-07-customize-surface-design.md §3.
//! A key or MIDI note may map to multiple channels (shared bindings).
//! File schema keys channels by dtx-core short names.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use dtx_core::EChannel;
use serde::{Deserialize, Serialize};

use crate::KeyCode;

/// Current bindings.toml schema version.
pub const BINDINGS_VERSION: u32 = 1;

/// The 12 bindable drum channels, in BocuD lane order.
pub const BINDABLE_CHANNELS: [EChannel; 12] = [
    EChannel::HiHatClose,
    EChannel::Snare,
    EChannel::BassDrum,
    EChannel::HighTom,
    EChannel::LowTom,
    EChannel::FloorTom,
    EChannel::Cymbal,
    EChannel::HiHatOpen,
    EChannel::RideCymbal,
    EChannel::LeftCymbal,
    EChannel::LeftPedal,
    EChannel::LeftBassDrum,
];

/// A non-lane action a key or pad can trigger.
///
/// System verbs are **not** DTX chart channels, so `EChannel` gains no pseudo-
/// variants; they live in a parallel map on `InputBindings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemVerb {
    /// Toggle the pause overlay during a performance.
    Pause,
    /// Restart the current song from the top.
    Restart,
}

/// Every bindable system verb, in Controls-tab row order.
pub const SYSTEM_VERBS: [SystemVerb; 2] = [SystemVerb::Pause, SystemVerb::Restart];

impl SystemVerb {
    /// Stable on-disk key (the TOML table key under `[system]`). Mirrors the
    /// channel short-name scheme: the file never depends on Rust variant names.
    pub fn key(self) -> &'static str {
        match self {
            SystemVerb::Pause => "pause",
            SystemVerb::Restart => "restart",
        }
    }

    /// Inverse of [`SystemVerb::key`]; unknown keys are skipped on load.
    pub fn from_key(key: &str) -> Option<Self> {
        SYSTEM_VERBS.into_iter().find(|verb| verb.key() == key)
    }

    /// Human label for the Controls-tab row.
    pub fn label(self) -> &'static str {
        match self {
            SystemVerb::Pause => "Pause",
            SystemVerb::Restart => "Restart",
        }
    }
}

/// The lane channel that already owns `src`, if any. A system verb may not
/// share an input with a lane: the same hit would both judge and fire the verb.
///
/// One-directional: lane binds are never refused — lanes win ties. The editor's
/// capture path and `BindResolver` are the only two callers, and they are the
/// only two places the rule is enforced.
pub fn lane_owner(bindings: &InputBindings, src: &BindSource) -> Option<EChannel> {
    BINDABLE_CHANNELS
        .into_iter()
        .find(|ch| bindings.map.get(ch).is_some_and(|v| v.contains(src)))
}

/// One input source bound to a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindSource {
    /// Keyboard key (Bevy `KeyCode`, serialized as its variant name).
    Key(KeyCode),
    /// MIDI note number (device-agnostic in v1).
    Midi { note: u8 },
}

/// MIDI device options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiDeviceConfig {
    /// Substring filter for the input port name; None = first available.
    pub port: Option<String>,
    /// NoteOn velocities at or below this value are ignored.
    pub velocity_threshold: u8,
}

/// On-disk schema (`bindings.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BindingsFile {
    /// Schema version for migrations.
    pub version: u32,
    /// MIDI device options.
    pub midi: MidiDeviceConfig,
    /// Channel short name → sources. BTreeMap for stable file output.
    pub map: BTreeMap<String, Vec<BindSource>>,
    /// System-verb key (`SystemVerb::key`) → sources. Empty by default; an
    /// older file with no `[system]` table loads clean (container `serde(default)`).
    pub system: BTreeMap<String, Vec<BindSource>>,
}

impl Default for BindingsFile {
    fn default() -> Self {
        InputBindings::default().to_file()
    }
}

/// Runtime bindings, channel-keyed.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBindings {
    /// MIDI device options.
    pub midi: MidiDeviceConfig,
    /// Channel → sources. Keyboard and MIDI sources may each appear under
    /// multiple channels (`bind_shared`); every owning channel's lane fires.
    pub map: HashMap<EChannel, Vec<BindSource>>,
    /// System verb → sources. Empty by default: Escape keeps working, and note
    /// maps vary by brand, so we never guess a pad on the user's behalf.
    pub system: HashMap<SystemVerb, Vec<BindSource>>,
}

impl Default for InputBindings {
    fn default() -> Self {
        use BindSource::{Key, Midi};
        use EChannel::*;
        use KeyCode as K;
        let mut map: HashMap<EChannel, Vec<BindSource>> = HashMap::new();
        // Keyboard: BocuD tSetDefaultKeyAssignments
        // (ported from gameplay-drums lane_map::default_drums).
        // MIDI: General MIDI percussion map, completed for toms/cymbals
        // (fixes old mapping.rs 49→HH; GM 49 = Crash 1 → LC).
        map.insert(HiHatClose, vec![Key(K::KeyX), Midi { note: 42 }]);
        map.insert(
            Snare,
            vec![
                Key(K::KeyC),
                Key(K::KeyD),
                Midi { note: 38 },
                Midi { note: 40 },
            ],
        );
        map.insert(
            BassDrum,
            vec![
                Key(K::Space),
                Key(K::Convert),
                Midi { note: 36 },
                Midi { note: 35 },
            ],
        );
        map.insert(
            HighTom,
            vec![
                Key(K::KeyV),
                Key(K::KeyF),
                Midi { note: 48 },
                Midi { note: 50 },
            ],
        );
        map.insert(
            LowTom,
            vec![
                Key(K::KeyB),
                Key(K::KeyG),
                Midi { note: 45 },
                Midi { note: 47 },
            ],
        );
        map.insert(
            FloorTom,
            vec![
                Key(K::KeyN),
                Key(K::KeyH),
                Midi { note: 43 },
                Midi { note: 41 },
            ],
        );
        map.insert(
            Cymbal,
            vec![
                Key(K::KeyM),
                Key(K::KeyJ),
                Midi { note: 57 },
                Midi { note: 52 },
            ],
        );
        map.insert(HiHatOpen, vec![Key(K::KeyS), Midi { note: 46 }]);
        map.insert(
            RideCymbal,
            vec![
                Key(K::Comma),
                Key(K::KeyK),
                Midi { note: 51 },
                Midi { note: 59 },
            ],
        );
        map.insert(
            LeftCymbal,
            vec![
                Key(K::KeyZ),
                Key(K::KeyA),
                Midi { note: 49 },
                Midi { note: 55 },
            ],
        );
        map.insert(LeftPedal, vec![Key(K::NonConvert), Midi { note: 44 }]);
        map.insert(LeftBassDrum, vec![Key(K::AltLeft)]);
        Self {
            midi: MidiDeviceConfig::default(),
            map,
            system: HashMap::new(),
        }
    }
}

impl InputBindings {
    /// First channel for a keyboard key, if bound.
    pub fn channel_for_key(&self, key: KeyCode) -> Option<EChannel> {
        self.channels_for(BindSource::Key(key)).into_iter().next()
    }

    /// All channels for a keyboard key, in bindable lane order.
    pub fn channels_for_key(&self, key: KeyCode) -> Vec<EChannel> {
        self.channels_for(BindSource::Key(key))
    }

    /// Channel for a MIDI note, if bound.
    pub fn channel_for_note(&self, note: u8) -> Option<EChannel> {
        self.channels_for(BindSource::Midi { note })
            .into_iter()
            .next()
    }

    /// All channels for a MIDI note, in bindable lane order.
    pub fn channels_for_note(&self, note: u8) -> Vec<EChannel> {
        self.channels_for(BindSource::Midi { note })
    }

    fn channels_for(&self, src: BindSource) -> Vec<EChannel> {
        BINDABLE_CHANNELS
            .into_iter()
            .filter(|ch| self.map.get(ch).is_some_and(|v| v.contains(&src)))
            .collect()
    }

    /// Bind `src` to `channel`, removing it from any other channel first
    /// (steal semantics — UI confirms before calling).
    pub fn bind(&mut self, channel: EChannel, src: BindSource) {
        for v in self.map.values_mut() {
            v.retain(|s| *s != src);
        }
        let entry = self.map.entry(channel).or_default();
        if !entry.contains(&src) {
            entry.push(src);
        }
    }

    /// Bind `src` to `channel` without removing it from other channels.
    pub fn bind_shared(&mut self, channel: EChannel, src: BindSource) {
        let entry = self.map.entry(channel).or_default();
        if !entry.contains(&src) {
            entry.push(src);
        }
    }

    /// Sources bound to `verb` (empty when unbound).
    pub fn system_sources(&self, verb: SystemVerb) -> &[BindSource] {
        self.system.get(&verb).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Bind `src` to `verb`. Never steals: one source may drive several verbs,
    /// and a lane-owned source is refused by the caller (`lane_owner`), not here.
    pub fn bind_system(&mut self, verb: SystemVerb, src: BindSource) {
        let entry = self.system.entry(verb).or_default();
        if !entry.contains(&src) {
            entry.push(src);
        }
    }

    /// Serialize to the on-disk schema.
    pub fn to_file(&self) -> BindingsFile {
        let mut map = BTreeMap::new();
        for ch in BINDABLE_CHANNELS {
            let name = ch.short_name().expect("bindable channel has short name");
            let sources = self.map.get(&ch).cloned().unwrap_or_default();
            map.insert(name.to_string(), sources);
        }
        BindingsFile {
            version: BINDINGS_VERSION,
            midi: self.midi.clone(),
            map,
            system: SYSTEM_VERBS
                .into_iter()
                .filter_map(|verb| {
                    self.system
                        .get(&verb)
                        .filter(|sources| !sources.is_empty())
                        .map(|sources| (verb.key().to_owned(), sources.clone()))
                })
                .collect(),
        }
    }
}

impl BindingsFile {
    /// Resolve to runtime bindings. Unknown channel names are skipped with a
    /// warning. A key or MIDI note may appear under multiple channels
    /// (shared bindings); every owning channel is kept.
    pub fn resolve(&self) -> InputBindings {
        let mut map: HashMap<EChannel, Vec<BindSource>> = HashMap::new();
        for (name, sources) in &self.map {
            let Some(ch) = EChannel::from_short_name(name) else {
                eprintln!("dtx-config: bindings.toml unknown channel {name:?}; skipped");
                continue;
            };
            let entry = map.entry(ch).or_default();
            for src in sources {
                if !entry.contains(src) {
                    entry.push(*src);
                }
            }
        }
        let mut system: HashMap<SystemVerb, Vec<BindSource>> = HashMap::new();
        for (name, sources) in &self.system {
            let Some(verb) = SystemVerb::from_key(name) else {
                eprintln!("dtx-input: bindings.toml unknown system verb {name:?}; skipped");
                continue;
            };
            let entry = system.entry(verb).or_default();
            for src in sources {
                if !entry.contains(src) {
                    entry.push(*src);
                }
            }
        }
        InputBindings {
            midi: self.midi.clone(),
            map,
            system,
        }
    }
}

/// Parse raw TOML, running the version migration chain (same policy as
/// dtx-layout `parse_with_migrations`).
pub fn parse_bindings_checked(raw: &str) -> Result<BindingsFile, dtx_config::ConfigError> {
    let mut file: BindingsFile = toml::from_str(raw)?;
    if file.version <= BINDINGS_VERSION {
        file.version = BINDINGS_VERSION;
    }
    Ok(file)
}

pub fn parse_with_migrations(raw: &str) -> BindingsFile {
    let mut file: BindingsFile = match parse_bindings_checked(raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dtx-config: bindings parse failed: {e}; using defaults");
            return BindingsFile::default();
        }
    };
    if file.version > BINDINGS_VERSION {
        eprintln!(
            "dtx-config: bindings.toml version {} newer than supported {}; best-effort load",
            file.version, BINDINGS_VERSION
        );
        return file;
    }
    #[allow(clippy::single_match)]
    match file.version {
        0 => file.version = 1,
        _ => {}
    }
    file
}

/// `$XDG_CONFIG_HOME/dtxmaniars/bindings.toml` (same directory scheme as
/// config.toml, see `dtx_config::default_path`).
pub fn default_bindings_path() -> PathBuf {
    let mut p = dtx_config::default_path();
    p.set_file_name("bindings.toml");
    p
}

/// Load bindings; missing/corrupt file → defaults.
pub fn load_bindings(path: &Path) -> InputBindings {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_with_migrations(&raw).resolve(),
        Err(_) => InputBindings::default(),
    }
}

/// Save bindings. Creates parent dirs.
pub fn save_bindings(path: &Path, b: &InputBindings) -> Result<(), dtx_config::ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(&b.to_file())?;
    std::fs::write(path, s)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_all_bindable_channels() {
        let b = InputBindings::default();
        for ch in BINDABLE_CHANNELS {
            assert!(b.map.contains_key(&ch), "{ch:?} missing");
        }
    }

    #[test]
    fn defaults_have_no_duplicate_sources() {
        let b = InputBindings::default();
        let all: Vec<_> = b.map.values().flatten().collect();
        let mut dedup = all.clone();
        dedup.sort_by_key(|s| format!("{s:?}"));
        dedup.dedup();
        assert_eq!(all.len(), dedup.len());
    }

    #[test]
    fn gm_note_49_is_left_cymbal_not_hh() {
        let b = InputBindings::default();
        assert_eq!(b.channel_for_note(49), Some(EChannel::LeftCymbal));
        assert_eq!(b.channel_for_note(42), Some(EChannel::HiHatClose));
        assert_eq!(b.channel_for_note(46), Some(EChannel::HiHatOpen));
        assert_eq!(b.channel_for_note(48), Some(EChannel::HighTom));
    }

    #[test]
    fn key_lookup_matches_bocud_defaults() {
        let b = InputBindings::default();
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
        assert_eq!(b.channel_for_key(KeyCode::Space), Some(EChannel::BassDrum));
        assert_eq!(
            b.channel_for_key(KeyCode::AltLeft),
            Some(EChannel::LeftBassDrum)
        );
        assert_eq!(b.channel_for_key(KeyCode::KeyQ), None);
    }

    #[test]
    fn round_trip_file_resolve() {
        let b = InputBindings::default();
        let s = toml::to_string_pretty(&b.to_file()).unwrap();
        let back = parse_with_migrations(&s).resolve();
        assert_eq!(back, b);
    }

    #[test]
    fn bind_steals_from_other_channel() {
        let mut b = InputBindings::default();
        b.bind(EChannel::Snare, BindSource::Key(KeyCode::KeyX));
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::Snare));
        assert!(!b.map[&EChannel::HiHatClose].contains(&BindSource::Key(KeyCode::KeyX)));
    }

    #[test]
    fn bind_shared_allows_one_key_on_multiple_channels() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::Snare, BindSource::Key(KeyCode::KeyX));
        assert_eq!(
            b.channels_for_key(KeyCode::KeyX),
            vec![EChannel::HiHatClose, EChannel::Snare]
        );
    }

    #[test]
    fn unknown_channel_name_skipped() {
        let raw = r#"
version = 1
[midi]
velocity_threshold = 10
[map]
NOPE = [{ key = "KeyQ" }]
HH = [{ key = "KeyX" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert_eq!(b.channel_for_key(KeyCode::KeyQ), None);
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
        assert_eq!(b.midi.velocity_threshold, 10);
    }

    #[test]
    fn corrupt_file_yields_defaults() {
        let f = parse_with_migrations("not = valid = [[toml");
        assert_eq!(f, BindingsFile::default());
    }

    #[test]
    fn version_zero_migrates_to_one() {
        let f = parse_with_migrations("version = 0");
        assert_eq!(f.version, 1);
    }

    #[test]
    fn duplicate_key_in_file_maps_multiple_channels() {
        let raw = r#"
version = 1
[map]
BD = [{ key = "Space" }]
SD = [{ key = "Space" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert_eq!(
            b.channels_for_key(KeyCode::Space),
            vec![EChannel::Snare, EChannel::BassDrum]
        );
    }

    #[test]
    fn channels_for_note_lists_every_owner() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::Snare, BindSource::Midi { note: 42 });
        assert_eq!(
            b.channels_for_note(42),
            vec![EChannel::HiHatClose, EChannel::Snare]
        );
    }

    #[test]
    fn duplicate_midi_in_file_binds_every_owning_channel() {
        let raw = r#"
version = 1
[map]
BD = [{ midi = { note = 36 } }]
SD = [{ midi = { note = 36 } }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert!(b.map[&EChannel::BassDrum].contains(&BindSource::Midi { note: 36 }));
        assert!(b.map[&EChannel::Snare].contains(&BindSource::Midi { note: 36 }));
    }

    #[test]
    fn save_load_round_trip() {
        let tmp = std::env::temp_dir().join("dtxmaniars_bindings_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let p = tmp.join("bindings.toml");
        let mut b = InputBindings::default();
        b.midi.velocity_threshold = 24;
        b.bind(EChannel::Snare, BindSource::Midi { note: 99 });
        save_bindings(&p, &b).unwrap();
        let back = load_bindings(&p);
        assert_eq!(back, b);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let b = load_bindings(Path::new("/nonexistent/dtxmaniars/bindings.toml"));
        assert_eq!(b, InputBindings::default());
    }

    #[test]
    fn default_bindings_path_filename() {
        assert_eq!(
            default_bindings_path().file_name().unwrap(),
            "bindings.toml"
        );
    }

    #[test]
    fn old_file_without_system_table_loads_empty_system_map() {
        let raw = r#"
version = 1
[midi]
velocity_threshold = 10
[map]
HH = [{ key = "KeyX" }]
"#;
        let b = parse_with_migrations(raw).resolve();
        assert!(b.system.is_empty(), "no [system] table → empty system map");
        assert_eq!(b.channel_for_key(KeyCode::KeyX), Some(EChannel::HiHatClose));
    }

    #[test]
    fn system_binds_round_trip_through_the_file() {
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let s = toml::to_string_pretty(&b.to_file()).expect("bindings serialize");
        // `toml` emits no bare `[system]` parent header (nor a bare `[map]` one):
        // a table whose every value is a table is written only via its children.
        assert!(s.contains("[[system.pause]]"), "{s}");
        assert!(s.contains("[[system.restart]]"), "{s}");
        let back = parse_with_migrations(&s).resolve();
        assert_eq!(back, b);
        assert_eq!(
            back.system_sources(SystemVerb::Pause),
            [BindSource::Midi { note: 37 }]
        );
    }

    #[test]
    fn system_verb_file_keys_are_stable() {
        assert_eq!(SystemVerb::Pause.key(), "pause");
        assert_eq!(SystemVerb::Restart.key(), "restart");
        assert_eq!(SystemVerb::from_key("pause"), Some(SystemVerb::Pause));
        assert_eq!(SystemVerb::from_key("nope"), None);
        assert_eq!(SYSTEM_VERBS.len(), 2);
    }
}
