//! Runtime bind resolution: `InputBindings` → per-frame lookup tables.
//!
//! `BindResolver` flattens channel-keyed bindings into KeyCode→LaneId and
//! note→LaneId maps using the fixed BocuD lane order (`lane_map::lane_of`).
//! Rebuilt on Performance enter (config may have changed on disk).

use std::collections::HashMap;

use bevy::prelude::*;
use dtx_config::{BindSource, InputBindings};

use crate::lane_map::{lane_of, LaneId};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BindResolver>()
        .init_resource::<LiveBindings>()
        .add_systems(OnEnter(game_shell::AppState::Performance), reload_bindings)
        .add_systems(
            Update,
            (
                apply_live_bindings.run_if(resource_changed::<LiveBindings>),
                save_bindings_on_close,
            )
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// Live, editable bindings — the Bindings tab mutates this; the resolver +
/// disk follow. Seeded from bindings.toml on Performance enter.
#[derive(Resource, Debug, Clone)]
pub struct LiveBindings(pub dtx_config::InputBindings);

impl Default for LiveBindings {
    fn default() -> Self {
        Self(dtx_config::InputBindings::default())
    }
}

/// Flattened lookup tables derived from `InputBindings`.
#[derive(Resource, Debug, Clone)]
pub struct BindResolver {
    key_to_lane: HashMap<KeyCode, LaneId>,
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
    /// Build lookup tables from channel-keyed bindings.
    pub fn from_bindings(b: &InputBindings) -> Self {
        let mut key_to_lane = HashMap::new();
        let mut note_to_lane = HashMap::new();
        for (ch, sources) in &b.map {
            let Some(lane) = lane_of(*ch) else { continue };
            for src in sources {
                match src {
                    BindSource::Key(k) => {
                        key_to_lane.insert(*k, lane);
                    }
                    BindSource::Midi { note } => {
                        note_to_lane.insert(*note, lane);
                    }
                }
            }
        }
        Self {
            key_to_lane,
            note_to_lane,
            velocity_threshold: b.midi.velocity_threshold,
        }
    }

    /// Lane for a keyboard key, if bound.
    pub fn lane_for_key(&self, key: KeyCode) -> Option<LaneId> {
        self.key_to_lane.get(&key).copied()
    }

    /// Lane for a MIDI note, if bound.
    pub fn lane_for_note(&self, note: u8) -> Option<LaneId> {
        self.note_to_lane.get(&note).copied()
    }
}

/// Reload bindings.toml on entering Performance (mirrors config load style,
/// see lib.rs `load(&default_path())` call sites). Seeds both `LiveBindings`
/// and `BindResolver` from the same load so they start consistent.
fn reload_bindings(mut resolver: ResMut<BindResolver>, mut live: ResMut<LiveBindings>) {
    let b = dtx_config::load_bindings(&dtx_config::default_bindings_path());
    *resolver = BindResolver::from_bindings(&b);
    live.0 = b;
}

/// Rebuild `BindResolver` whenever `LiveBindings` changes (immediate feedback).
/// Does NOT save — disk persistence happens on surface close.
fn apply_live_bindings(live: Res<LiveBindings>, mut resolver: ResMut<BindResolver>) {
    *resolver = BindResolver::from_bindings(&live.0);
}

/// When the Customize surface closes, persist the live bindings to disk
/// (mirrors `tabs::save_draft_on_close`).
fn save_bindings_on_close(open: Res<crate::editor::EditorOpen>, live: Res<LiveBindings>) {
    if !open.is_changed() || open.0 {
        return;
    }
    let path = dtx_config::default_bindings_path();
    if let Err(e) = dtx_config::save_bindings(&path, &live.0) {
        error!("customize: failed to save bindings {}: {e}", path.display());
    }
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
}
