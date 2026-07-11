//! Runtime bind resolution: `InputBindings` → per-frame lookup tables.
//!
//! `BindResolver` flattens channel-keyed bindings into KeyCode→LaneIds and
//! note→LaneId maps using the fixed BocuD lane order (`lane_map::lane_of`).
//! Rebuilt on Performance enter (config may have changed on disk).

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use dtx_config::profiles::{
    keyboard_builtins, keyboard_registry, load_keyboard_registry, load_midi_registry,
    midi_builtins, midi_registry, KeyboardProfile, MidiProfile, ProfileRegistry, RegistryStartup,
};
use dtx_config::{BindSource, InputBindings, BINDABLE_CHANNELS};

use crate::lane_map::{LaneId, lane_of};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BindResolver>()
        .init_resource::<LiveBindings>()
        .init_resource::<ActiveInputProfiles>()
        // Seeded at boot too: pads navigate menus before any Performance enter.
        .add_systems(Startup, reload_profiles)
        .add_systems(OnEnter(game_shell::AppState::Performance), reload_profiles)
        .add_systems(
            Update,
            apply_live_bindings
                .run_if(resource_changed::<LiveBindings>)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// The committed active input profiles. Changes only after a registry write
/// succeeds; editor previews rebuild `BindResolver` from drafts instead.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveInputProfiles {
    pub keyboard: KeyboardProfile,
    pub midi: MidiProfile,
}

/// Live, editable bindings — the Bindings tab mutates this; the resolver +
/// disk follow. Seeded from bindings.toml on Performance enter.
#[derive(Resource, Debug, Clone, Default)]
pub struct LiveBindings(pub dtx_config::InputBindings);

/// Flattened lookup tables derived from `InputBindings`.
#[derive(Resource, Debug, Clone)]
pub struct BindResolver {
    key_to_lanes: HashMap<KeyCode, Vec<LaneId>>,
    note_to_lane: HashMap<u8, LaneId>,
    /// NoteOn velocities at or below this are ignored.
    pub velocity_threshold: u8,
}

impl Default for BindResolver {
    fn default() -> Self {
        Self::from_bindings(&InputBindings::default())
    }
}

impl BindResolver {
    /// Build lookup tables from independent keyboard and MIDI profiles.
    /// Lane ids come from the fixed logical order (`lane_map::lane_of`) and
    /// never depend on the display lane arrangement.
    pub fn from_profiles(keyboard: &KeyboardProfile, midi: &MidiProfile) -> Self {
        let mut key_to_lanes = HashMap::new();
        let mut note_to_lane = HashMap::new();
        for ch in BINDABLE_CHANNELS {
            let Some(lane) = lane_of(ch) else { continue };
            for key in keyboard.map.get(&ch).into_iter().flatten() {
                key_to_lanes.entry(*key).or_insert_with(Vec::new).push(lane);
            }
            for note in midi.map.get(&ch).into_iter().flatten() {
                note_to_lane.insert(*note, lane);
            }
        }
        Self {
            key_to_lanes,
            note_to_lane,
            velocity_threshold: midi.velocity_threshold,
        }
    }

    /// Build lookup tables from channel-keyed bindings.
    pub fn from_bindings(b: &InputBindings) -> Self {
        let mut key_to_lanes = HashMap::new();
        let mut note_to_lane = HashMap::new();
        for ch in BINDABLE_CHANNELS {
            let Some(lane) = lane_of(ch) else { continue };
            let Some(sources) = b.map.get(&ch) else {
                continue;
            };
            for src in sources {
                match src {
                    BindSource::Key(k) => {
                        key_to_lanes.entry(*k).or_insert_with(Vec::new).push(lane);
                    }
                    BindSource::Midi { note } => {
                        note_to_lane.insert(*note, lane);
                    }
                }
            }
        }
        Self {
            key_to_lanes,
            note_to_lane,
            velocity_threshold: b.midi.velocity_threshold,
        }
    }

    /// First lane for a keyboard key, if bound.
    pub fn lane_for_key(&self, key: KeyCode) -> Option<LaneId> {
        self.key_to_lanes
            .get(&key)
            .and_then(|lanes| lanes.first().copied())
    }

    /// Lanes for a keyboard key, if bound.
    pub fn lanes_for_key(&self, key: KeyCode) -> impl Iterator<Item = LaneId> + '_ {
        self.key_to_lanes
            .get(&key)
            .into_iter()
            .flat_map(|lanes| lanes.iter().copied())
    }

    /// Lane for a MIDI note, if bound.
    pub fn lane_for_note(&self, note: u8) -> Option<LaneId> {
        self.note_to_lane.get(&note).copied()
    }
}

pub fn keyboard_registry_path() -> PathBuf {
    let mut p = dtx_config::default_path();
    p.set_file_name("keyboard-profiles.toml");
    p
}

pub fn midi_registry_path() -> PathBuf {
    let mut p = dtx_config::default_path();
    p.set_file_name("midi-profiles.toml");
    p
}

/// Unwrap a registry startup for runtime use: a failed migration write still
/// yields a usable in-memory registry for this session; an unreadable or
/// invalid registry falls back to built-ins without touching the file.
fn startup_registry<T>(
    what: &str,
    startup: RegistryStartup<ProfileRegistry<T>>,
    fallback: ProfileRegistry<T>,
) -> ProfileRegistry<T> {
    match startup {
        RegistryStartup::Ready(registry) => registry,
        RegistryStartup::LegacySession {
            registry,
            write_error,
        } => {
            error!("{what} profile registry migration write failed: {write_error}");
            registry
        }
        RegistryStartup::ReadOnlyBuiltins(error) => {
            error!("{what} profile registry unusable, using built-ins: {error}");
            fallback
        }
    }
}

fn active_keyboard_profile(registry: &ProfileRegistry<KeyboardProfile>) -> KeyboardProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| keyboard_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

fn active_midi_profile(registry: &ProfileRegistry<MidiProfile>) -> MidiProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| midi_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

/// Compose channel-keyed bindings from the active profiles so the legacy
/// editor panels keep working against `LiveBindings` until the profile
/// editors replace them.
fn compose_bindings(profiles: &ActiveInputProfiles) -> InputBindings {
    let mut bindings = InputBindings {
        midi: dtx_config::MidiDeviceConfig {
            port: profiles.midi.port.clone(),
            velocity_threshold: profiles.midi.velocity_threshold,
        },
        map: HashMap::new(),
    };
    for ch in BINDABLE_CHANNELS {
        let mut sources = Vec::new();
        for key in profiles.keyboard.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Key(*key));
        }
        for note in profiles.midi.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Midi { note: *note });
        }
        if !sources.is_empty() {
            bindings.map.insert(ch, sources);
        }
    }
    bindings
}

/// Load (or migrate) the keyboard and MIDI profile registries and resolve the
/// active profiles into the committed resources. Runs at boot (pads navigate
/// menus before any Performance enter) and on Performance enter.
fn reload_profiles(
    mut profiles: ResMut<ActiveInputProfiles>,
    mut resolver: ResMut<BindResolver>,
    mut live: ResMut<LiveBindings>,
) {
    let legacy = dtx_config::default_bindings_path();
    let keyboard = startup_registry(
        "keyboard",
        load_keyboard_registry(&keyboard_registry_path(), &legacy),
        keyboard_registry(),
    );
    let midi = startup_registry(
        "MIDI",
        load_midi_registry(&midi_registry_path(), &legacy),
        midi_registry(),
    );
    *profiles = ActiveInputProfiles {
        keyboard: active_keyboard_profile(&keyboard),
        midi: active_midi_profile(&midi),
    };
    *resolver = BindResolver::from_profiles(&profiles.keyboard, &profiles.midi);
    live.0 = compose_bindings(&profiles);
}

/// Rebuild `BindResolver` whenever `LiveBindings` changes (editor preview
/// feedback). Does NOT save — committed profiles change only after a
/// registry write succeeds.
fn apply_live_bindings(live: Res<LiveBindings>, mut resolver: ResMut<BindResolver>) {
    *resolver = BindResolver::from_bindings(&live.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;

    #[test]
    fn default_resolver_matches_bocud_keys() {
        let r = BindResolver::default();
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0)); // HH
        assert_eq!(r.lane_for_key(KeyCode::KeyC), Some(1)); // SD
        assert_eq!(r.lane_for_key(KeyCode::KeyD), Some(1)); // SD alt
        assert_eq!(r.lane_for_key(KeyCode::Space), Some(2)); // BD
        assert_eq!(r.lane_for_key(KeyCode::KeyS), Some(7)); // HHO
        assert_eq!(r.lane_for_key(KeyCode::AltLeft), Some(11)); // LBD
        assert_eq!(r.lane_for_key(KeyCode::KeyQ), None);
    }

    #[test]
    fn default_resolver_maps_gm_notes_to_lanes() {
        let r = BindResolver::default();
        assert_eq!(r.lane_for_note(36), Some(2)); // BD
        assert_eq!(r.lane_for_note(38), Some(1)); // SD
        assert_eq!(r.lane_for_note(42), Some(0)); // HH close
        assert_eq!(r.lane_for_note(46), Some(7)); // HH open
        assert_eq!(r.lane_for_note(49), Some(9)); // Crash 1 → LC (GM fix)
        assert_eq!(r.lane_for_note(51), Some(8)); // Ride
        assert_eq!(r.lane_for_note(48), Some(3)); // High tom — newly mapped
        assert_eq!(r.lane_for_note(43), Some(5)); // Floor tom — newly mapped
        assert_eq!(r.lane_for_note(20), None);
    }

    #[test]
    fn custom_binding_reroutes_lane() {
        let mut b = InputBindings::default();
        b.bind(EChannel::Snare, dtx_config::BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(1)); // now SD
    }

    #[test]
    fn shared_key_binding_maps_to_multiple_lanes() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::Snare, dtx_config::BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(
            r.lanes_for_key(KeyCode::KeyX).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn resolver_tracks_live_binding_edit() {
        let mut ib = dtx_config::InputBindings::default();
        let sd = dtx_core::EChannel::Snare;
        ib.bind(sd, dtx_config::BindSource::Key(KeyCode::KeyP));
        let resolver = BindResolver::from_bindings(&ib);
        assert_eq!(
            resolver.lane_for_key(KeyCode::KeyP),
            crate::lane_map::lane_of(sd)
        );
    }

    #[test]
    fn threshold_copied_from_bindings() {
        let mut b = InputBindings::default();
        b.midi.velocity_threshold = 30;
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.velocity_threshold, 30);
    }

    #[test]
    fn active_profiles_compose_keyboard_and_midi() {
        let keyboard = KeyboardProfile::default();
        let mut midi = MidiProfile::default();
        midi.velocity_threshold = 25;
        let r = BindResolver::from_profiles(&keyboard, &midi);
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0)); // HH
        assert_eq!(r.lane_for_key(KeyCode::Space), Some(2)); // BD
        assert_eq!(r.lane_for_note(38), Some(1)); // SD
        assert_eq!(r.lane_for_note(36), Some(2)); // BD
        assert_eq!(r.velocity_threshold, 25);
    }

    #[test]
    fn shared_key_emits_all_fixed_logical_lanes() {
        let mut keyboard = KeyboardProfile::default();
        keyboard.add_key(EChannel::HiHatClose, KeyCode::KeyQ);
        keyboard.add_key(EChannel::Snare, KeyCode::KeyQ);
        let r = BindResolver::from_profiles(&keyboard, &MidiProfile::default());
        assert_eq!(
            r.lanes_for_key(KeyCode::KeyQ).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn exclusive_note_emits_one_fixed_logical_lane() {
        let mut midi = MidiProfile::default();
        midi.bind_note(EChannel::Snare, 36); // steal BD's note
        let r = BindResolver::from_profiles(&KeyboardProfile::default(), &midi);
        assert_eq!(r.lane_for_note(36), Some(1)); // now SD, exactly one lane
        assert_eq!(midi.note_owner(36), Some(EChannel::Snare));
    }

    #[test]
    fn changing_lane_arrangement_does_not_change_resolver_lane_id() {
        // The resolver only consults the fixed logical order (lane_of); the
        // display arrangement is not an input, so ids match lane_of exactly.
        let r = BindResolver::from_profiles(&KeyboardProfile::default(), &MidiProfile::default());
        assert_eq!(r.lane_for_note(38), lane_of(EChannel::Snare));
        assert_eq!(r.lane_for_note(36), lane_of(EChannel::BassDrum));
        assert_eq!(r.lane_for_key(KeyCode::KeyX), lane_of(EChannel::HiHatClose));
    }

    #[test]
    fn failed_registry_selection_keeps_active_resolver() {
        use dtx_config::profiles::{reduce_registry, RegistryAction};
        let registry = dtx_config::profiles::keyboard_registry();
        let before = ActiveInputProfiles::default();
        let resolver = BindResolver::from_profiles(&before.keyboard, &before.midi);
        let result = reduce_registry(
            &registry,
            &keyboard_builtins(),
            RegistryAction::Select("no such profile".to_owned()),
        );
        assert!(result.is_err());
        let after = BindResolver::from_profiles(&before.keyboard, &before.midi);
        assert_eq!(
            resolver.lane_for_key(KeyCode::KeyX),
            after.lane_for_key(KeyCode::KeyX)
        );
        assert_eq!(resolver.lane_for_note(38), after.lane_for_note(38));
    }
}
