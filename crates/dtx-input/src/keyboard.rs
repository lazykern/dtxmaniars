//! Keyboard → LaneHit helper + system-verb translator.
//!
//! Per ADR-0009 the actual Bevy system lives in the gameplay crate (which
//! owns the concrete `LaneMap`). dtx-input provides:
//! - [`KeyLaneMap`] trait that gameplay crates' LaneMap impls.
//! - [`emit_pressed_lanes`] / [`emit_released_lanes`] helpers.
//! - [`keyboard_system_verbs`]: bound keys → [`crate::SystemVerbHit`], the
//!   consuming crate wires run conditions (menu-nav extraction, 2026-07-15).
//!
//! Gameplay crate pseudocode:
//! ```ignore
//! fn keyboard_to_lane_hits(
//!     keys: Res<ButtonInput<KeyCode>>,
//!     map: Res<MyLaneMap>,
//!     clock: Res<AudioClock>,
//!     mut hits: MessageWriter<LaneHit>,
//! ) {
//!     let now = clock.current_ms.unwrap_or(0);
//!     emit_pressed_lanes(&keys, &*map, now, &mut hits);
//! }
//! ```

use bevy::ecs::message::MessageWriter;
use bevy::ecs::system::Res;
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use crate::events::{LaneHit, LaneHitKind};

/// Map KeyCode → LaneId. Implemented by each gameplay crate's LaneMap.
pub trait KeyLaneMap {
    /// Return Some(lane_id) if `key` is mapped to a lane, None otherwise.
    fn lane_for_key(&self, key: KeyCode) -> Option<crate::events::LaneId>;
}

/// Emit LaneHit::Press for every just-pressed mapped key.
pub fn emit_pressed_lanes(
    keys: &ButtonInput<KeyCode>,
    map: &dyn KeyLaneMap,
    now_ms: i64,
    out: &mut Vec<LaneHit>,
) {
    for key in keys.get_just_pressed() {
        if let Some(lane) = map.lane_for_key(*key) {
            out.push(LaneHit::press(lane, now_ms));
        }
    }
}

/// Emit LaneHit::Release for every just-released mapped key.
pub fn emit_released_lanes(
    keys: &ButtonInput<KeyCode>,
    map: &dyn KeyLaneMap,
    now_ms: i64,
    out: &mut Vec<LaneHit>,
) {
    for key in keys.get_just_released() {
        if let Some(lane) = map.lane_for_key(*key) {
            out.push(LaneHit {
                lane,
                audio_ms: now_ms,
                kind: LaneHitKind::Release,
            });
        }
    }
}

/// Keyboard-bound system verbs → [`crate::SystemVerbHit`], on the same message
/// the MIDI pump writes. Carries NO state gating — the consuming game crate
/// wires run conditions (e.g. only during Performance, and deliberately not
/// gated on pause: the key that paused the song has to un-pause it). Emits
/// nothing while [`crate::RawInputOwned`] is set: a capture flow owns the
/// keyboard.
pub fn keyboard_system_verbs(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<crate::resolver::BindResolver>,
    owned: Res<crate::RawInputOwned>,
    mut out: MessageWriter<crate::SystemVerbHit>,
) {
    if owned.0 {
        return;
    }
    for key in keys.get_just_pressed() {
        for verb in resolver.system_for_key(*key) {
            out.write(crate::SystemVerbHit { verb });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{LaneHit, LaneHitKind};
    use bevy::input::keyboard::KeyCode;
    use bevy::prelude::*;

    struct TestMap;

    impl KeyLaneMap for TestMap {
        fn lane_for_key(&self, key: KeyCode) -> Option<crate::events::LaneId> {
            match key {
                KeyCode::KeyA => Some(0),
                KeyCode::KeyS => Some(1),
                _ => None,
            }
        }
    }

    #[test]
    fn test_map_a_returns_0() {
        let m = TestMap;
        assert_eq!(m.lane_for_key(KeyCode::KeyA), Some(0));
    }

    #[test]
    fn test_map_z_returns_none() {
        let m = TestMap;
        assert_eq!(m.lane_for_key(KeyCode::KeyZ), None);
    }

    #[test]
    fn lane_hit_press_equals_helper() {
        let a = LaneHit::press(1, 100);
        let b = LaneHit {
            lane: 1,
            audio_ms: 100,
            kind: LaneHitKind::Press,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn bound_key_emits_the_system_verb() {
        use crate::{BindSource, InputBindings, RawInputOwned, SystemVerb, SystemVerbHit};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<RawInputOwned>()
            .insert_resource(crate::resolver::BindResolver::from_bindings(&bindings))
            .add_message::<SystemVerbHit>()
            .add_systems(Update, keyboard_system_verbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();

        let hits: Vec<SystemVerbHit> = app
            .world()
            .resource::<Messages<SystemVerbHit>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(
            hits,
            vec![SystemVerbHit {
                verb: SystemVerb::Pause
            }]
        );
    }

    #[test]
    fn owned_raw_input_swallows_the_system_verb() {
        use crate::{BindSource, InputBindings, RawInputOwned, SystemVerb, SystemVerbHit};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(RawInputOwned(true))
            .insert_resource(crate::resolver::BindResolver::from_bindings(&bindings))
            .add_message::<SystemVerbHit>()
            .add_systems(Update, keyboard_system_verbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<SystemVerbHit>>()
                .iter_current_update_messages()
                .count(),
            0,
            "a key pressed while capture owns input must not fire a verb"
        );
    }
}
