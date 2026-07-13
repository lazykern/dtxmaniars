//! Runtime bind resolution: `InputBindings` → per-frame lookup tables.
//!
//! `BindResolver` flattens channel-keyed bindings into KeyCode→LaneIds and
//! note→LaneId maps using the fixed BocuD lane order (`lane_map::lane_of`).
//! Rebuilt on Performance enter (config may have changed on disk).

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use dtx_input::profiles::{
    keyboard_builtins, keyboard_registry, load_keyboard_registry, load_midi_registry,
    midi_builtins, midi_registry, KeyboardProfile, MidiProfile, ProfileRegistry, RegistryStartup,
};
use dtx_input::{
    lane_owner, BindSource, InputBindings, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS,
};

use crate::lane_map::{lane_of, LaneId};

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
pub struct LiveBindings(pub dtx_input::InputBindings);

/// Flattened lookup tables derived from `InputBindings`.
#[derive(Resource, Debug, Clone)]
pub struct BindResolver {
    key_to_lanes: HashMap<KeyCode, Vec<LaneId>>,
    note_to_lanes: HashMap<u8, Vec<LaneId>>,
    /// System verbs a key fires. A source a lane owns never reaches this table.
    key_to_system: HashMap<KeyCode, Vec<SystemVerb>>,
    /// System verbs a MIDI note fires. A source a lane owns never reaches this table.
    note_to_system: HashMap<u8, Vec<SystemVerb>>,
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
        Self::from_bindings(&compose_bindings(keyboard, midi))
    }

    /// Build lookup tables from channel-keyed bindings.
    pub fn from_bindings(b: &InputBindings) -> Self {
        let mut key_to_lanes = HashMap::new();
        let mut note_to_lanes = HashMap::new();
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
                        note_to_lanes
                            .entry(*note)
                            .or_insert_with(Vec::new)
                            .push(lane);
                    }
                }
            }
        }
        let mut key_to_system: HashMap<KeyCode, Vec<SystemVerb>> = HashMap::new();
        let mut note_to_system: HashMap<u8, Vec<SystemVerb>> = HashMap::new();
        for verb in SYSTEM_VERBS {
            for src in b.system.get(&verb).into_iter().flatten() {
                // Lanes win ties. A hand-edited bindings.toml cannot make one
                // note both judge and pause: the colliding source is dropped here.
                if let Some(owner) = lane_owner(b, src) {
                    warn!("system bind {verb:?} ignored: {src:?} already drives lane {owner:?}");
                    continue;
                }
                match src {
                    BindSource::Key(k) => key_to_system.entry(*k).or_default().push(verb),
                    BindSource::Midi { note } => {
                        note_to_system.entry(*note).or_default().push(verb)
                    }
                }
            }
        }
        Self {
            key_to_lanes,
            note_to_lanes,
            key_to_system,
            note_to_system,
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

    /// First lane for a MIDI note, if bound.
    pub fn lane_for_note(&self, note: u8) -> Option<LaneId> {
        self.lanes_for_note(note).next()
    }

    /// Lanes for a MIDI note (a note may be shared by several channels).
    pub fn lanes_for_note(&self, note: u8) -> impl Iterator<Item = LaneId> + '_ {
        self.note_to_lanes
            .get(&note)
            .into_iter()
            .flat_map(|lanes| lanes.iter().copied())
    }

    /// System verbs a MIDI note fires (empty unless bound and lane-free).
    pub fn system_for_note(&self, note: u8) -> impl Iterator<Item = SystemVerb> + '_ {
        self.note_to_system
            .get(&note)
            .into_iter()
            .flat_map(|verbs| verbs.iter().copied())
    }

    /// System verbs a keyboard key fires (empty unless bound and lane-free).
    pub fn system_for_key(&self, key: KeyCode) -> impl Iterator<Item = SystemVerb> + '_ {
        self.key_to_system
            .get(&key)
            .into_iter()
            .flat_map(|verbs| verbs.iter().copied())
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

/// Look up a registry's active profile value, falling back to built-ins then
/// the code default. Shared with the profile bar (Select/SaveAs/Rename/
/// Delete all need the resulting active value to refresh the session draft).
pub(crate) fn active_keyboard_profile(
    registry: &ProfileRegistry<KeyboardProfile>,
) -> KeyboardProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| keyboard_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

pub(crate) fn active_midi_profile(registry: &ProfileRegistry<MidiProfile>) -> MidiProfile {
    registry
        .profiles
        .get(&registry.active)
        .cloned()
        .or_else(|| midi_builtins().get(&registry.active).cloned())
        .unwrap_or_default()
}

/// Compose channel-keyed bindings from independent keyboard/MIDI profile
/// values so the legacy editor panels (chip list, capture, resolver) keep
/// working against `LiveBindings`. Shared by boot/reload and the profile
/// bar, which recomposes `LiveBindings` from the session drafts after every
/// Select/Save/SaveAs/Rename/Delete/Revert so the panel and resolver never
/// lag behind the committed profile.
pub(crate) fn compose_bindings(keyboard: &KeyboardProfile, midi: &MidiProfile) -> InputBindings {
    let mut bindings = InputBindings {
        midi: dtx_input::MidiDeviceConfig {
            port: midi.port.clone(),
            velocity_threshold: midi.velocity_threshold,
        },
        map: HashMap::new(),
        system: HashMap::new(),
    };
    for ch in BINDABLE_CHANNELS {
        let mut sources = Vec::new();
        for key in keyboard.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Key(*key));
        }
        for note in midi.map.get(&ch).into_iter().flatten() {
            sources.push(BindSource::Midi { note: *note });
        }
        if !sources.is_empty() {
            bindings.map.insert(ch, sources);
        }
    }
    // Without this the profiles' system binds would be wiped on every
    // `reload_profiles` — i.e. the moment a song starts.
    for verb in SYSTEM_VERBS {
        let mut sources = Vec::new();
        for key in keyboard.system.get(&verb).into_iter().flatten() {
            sources.push(BindSource::Key(*key));
        }
        for note in midi.system.get(&verb).into_iter().flatten() {
            sources.push(BindSource::Midi { note: *note });
        }
        if !sources.is_empty() {
            bindings.system.insert(verb, sources);
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
    let legacy = dtx_input::default_bindings_path();
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
    live.0 = compose_bindings(&profiles.keyboard, &profiles.midi);
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
        b.bind(EChannel::Snare, dtx_input::BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(1)); // now SD
    }

    #[test]
    fn shared_key_binding_maps_to_multiple_lanes() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::Snare, dtx_input::BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(
            r.lanes_for_key(KeyCode::KeyX).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn resolver_tracks_live_binding_edit() {
        let mut ib = dtx_input::InputBindings::default();
        let sd = dtx_core::EChannel::Snare;
        ib.bind(sd, dtx_input::BindSource::Key(KeyCode::KeyP));
        let resolver = BindResolver::from_bindings(&ib);
        assert_eq!(
            resolver.lane_for_key(KeyCode::KeyP),
            crate::lane_map::lane_of(sd)
        );
    }

    /// The reload path (`reload_profiles`) rebuilds `LiveBindings` from the
    /// profiles alone. A system bind that does not survive split → compose is a
    /// bind wiped the moment the drummer starts a song.
    #[test]
    fn system_binds_survive_split_and_compose() {
        use dtx_input::{profiles::split_bindings, SystemVerb};

        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let (keyboard, midi) = split_bindings(&b);

        let composed = compose_bindings(&keyboard, &midi);
        assert_eq!(
            composed.system_sources(SystemVerb::Pause),
            [BindSource::Midi { note: 37 }]
        );
        assert_eq!(
            composed.system_sources(SystemVerb::Restart),
            [BindSource::Key(KeyCode::F9)]
        );
        assert_eq!(composed.map, b.map, "lane map survives the round trip");
    }

    #[test]
    fn note_bound_to_a_lane_and_a_verb_resolves_to_the_lane_only() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        // 38 is the Snare default: a hand-edited file binding it to Pause too.
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 38 });
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_note(38), Some(1), "the lane still judges");
        assert_eq!(
            r.system_for_note(38).count(),
            0,
            "the colliding system source is skipped"
        );
    }

    #[test]
    fn free_note_resolves_to_the_verb_and_no_lane() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_note(37), None);
        assert_eq!(
            r.system_for_note(37).collect::<Vec<_>>(),
            vec![SystemVerb::Pause]
        );
    }

    #[test]
    fn free_key_resolves_to_the_verb_and_no_lane() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::F9));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::F9), None);
        assert_eq!(
            r.system_for_key(KeyCode::F9).collect::<Vec<_>>(),
            vec![SystemVerb::Restart]
        );
    }

    #[test]
    fn key_bound_to_a_lane_and_a_verb_resolves_to_the_lane_only() {
        use dtx_input::SystemVerb;
        let mut b = InputBindings::default();
        // KeyX is the HiHatClose default.
        b.bind_system(SystemVerb::Restart, BindSource::Key(KeyCode::KeyX));
        let r = BindResolver::from_bindings(&b);
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0));
        assert_eq!(r.system_for_key(KeyCode::KeyX).count(), 0);
    }

    #[test]
    fn from_profiles_builds_system_tables_and_skips_lane_collisions() {
        use dtx_input::SystemVerb;
        let mut keyboard = KeyboardProfile::default();
        let mut midi = MidiProfile::default();
        keyboard.add_system_key(SystemVerb::Restart, KeyCode::F9);
        keyboard.add_system_key(SystemVerb::Pause, KeyCode::KeyX); // HH's key: refused
        midi.bind_system_note(SystemVerb::Pause, 37); // free zone note
        midi.bind_system_note(SystemVerb::Pause, 38); // Snare's note: refused
        let r = BindResolver::from_profiles(&keyboard, &midi);
        assert_eq!(
            r.system_for_note(37).collect::<Vec<_>>(),
            vec![SystemVerb::Pause]
        );
        assert_eq!(r.system_for_note(38).count(), 0, "lane wins the tie");
        assert_eq!(r.lane_for_note(38), Some(1));
        assert_eq!(r.system_for_key(KeyCode::KeyX).count(), 0, "lane wins");
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0));
        assert_eq!(
            r.system_for_key(KeyCode::F9).collect::<Vec<_>>(),
            vec![SystemVerb::Restart]
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
        let midi = MidiProfile {
            velocity_threshold: 25,
            ..Default::default()
        };
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
    fn note_shared_by_two_channels_resolves_both_lanes() {
        let mut b = InputBindings::default();
        b.bind_shared(EChannel::LeftBassDrum, BindSource::Midi { note: 36 });
        let r = BindResolver::from_bindings(&b);
        let lanes: Vec<_> = r.lanes_for_note(36).collect();
        assert_eq!(lanes.len(), 2, "36 owned by BD and LBD: {lanes:?}");
        assert_eq!(r.lanes_for_note(42).count(), 1);
        assert_eq!(r.lanes_for_note(99).count(), 0);
    }

    #[test]
    fn note_shared_by_three_channels_resolves_three_lanes() {
        let mut midi = MidiProfile::default();
        midi.bind_note_shared(EChannel::LeftBassDrum, 36); // 36 already on BD
        midi.bind_note_shared(EChannel::Snare, 36);
        let r = BindResolver::from_profiles(&KeyboardProfile::default(), &midi);
        assert_eq!(r.lanes_for_note(36).count(), 3);
    }

    #[test]
    fn note_repeated_within_channel_resolves_one_lane() {
        // Within-channel duplicates are deduped when the profile deserializes,
        // so a saved [38, 38] fires the lane once (no double scoring).
        let midi: MidiProfile =
            toml::from_str("velocity_threshold = 0\n[map]\nSD = [38, 38]").expect("profile parses");
        let r = BindResolver::from_profiles(&KeyboardProfile::default(), &midi);
        assert_eq!(r.lanes_for_note(38).count(), 1);
    }

    #[test]
    fn failed_registry_selection_keeps_active_resolver() {
        use dtx_input::profiles::{reduce_registry, RegistryAction};
        let registry = dtx_input::profiles::keyboard_registry();
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
