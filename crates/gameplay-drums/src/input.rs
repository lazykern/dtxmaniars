//! Keyboard input → `LaneHit` events.
//!
//! Keys resolve to lanes via [`crate::bindings::BindResolver`], built from the
//! persisted `dtx-config` `InputBindings`. MIDI is handled in the lib.rs
//! `midi_gate` module (pump lives in `dtx_input::pump`).
//!
//! ## Phase 0 p0-6
//!
//! Gated on `EGameMode::Drums` so when the user picks Guitar mode, the
//! digits 1-9 don't accidentally fire drum hits. Mirror gating is in
//! `gameplay-guitar::input`.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_shell::EGameMode;

use crate::bindings::BindResolver;
use crate::events::InputHit;
use crate::resources::GameplayClock;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PendingLaneInputs>()
        .add_systems(
            PreUpdate,
            // Not gated on `editor_closed`: while the Customize surface is open we
            // still want captured hits to reach `LaneHit` (flash + feedback sound).
            // Scoring is gated instead — see `judge::judge_lane_hit_system`.
            capture_key_to_lane_input
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(crate::practice::gameplay_input_active),
        )
        .add_systems(
            PreUpdate,
            // NO PauseState gate: this is the key that has to un-pause the song.
            keyboard_system_verbs
                .after(bevy::input::InputSystems)
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(
            FixedUpdate,
            emit_pending_lane_hits
                .in_set(super::DrumsSets::Input)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running))
                .run_if(crate::practice::gameplay_input_active),
        );
}

#[derive(Resource, Default, Debug)]
pub(crate) struct PendingLaneInputs {
    events: Vec<CapturedLaneInput>,
}

#[derive(Debug, Clone)]
struct CapturedLaneInput {
    lanes: Vec<u8>,
    captured_at: Instant,
}

pub(crate) fn clear_pending_lane_inputs(commands: &mut Commands) {
    commands.insert_resource(PendingLaneInputs::default());
}

pub(crate) fn clear_pending_lane_inputs_now(pending: &mut PendingLaneInputs) {
    pending.events.clear();
}

fn capture_key_to_lane_input(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<BindResolver>,
    mode: Res<EGameMode>,
    capture: Res<crate::editor::bindings_capture::CaptureState>,
    mut pending: ResMut<PendingLaneInputs>,
) {
    if *mode != EGameMode::Drums
        || !matches!(
            *capture,
            crate::editor::bindings_capture::CaptureState::Idle
        )
    {
        return;
    }
    for key in keys.get_just_pressed() {
        let captured_at = Instant::now();
        let lanes: Vec<_> = resolver.lanes_for_key(*key).collect();
        if !lanes.is_empty() {
            pending
                .events
                .push(CapturedLaneInput { lanes, captured_at });
        }
    }
}

/// Keyboard-bound system verbs → `SystemVerbHit`, on the same message the MIDI
/// path writes. Deliberately NOT gated on `PauseState::Running`: the key that
/// paused the song has to be able to un-pause it. Consumers (`pause.rs`) carry
/// their own gates.
fn keyboard_system_verbs(
    keys: Res<ButtonInput<KeyCode>>,
    resolver: Res<BindResolver>,
    capture: Res<crate::editor::bindings_capture::CaptureState>,
    mut out: MessageWriter<crate::events::SystemVerbHit>,
) {
    if !matches!(
        *capture,
        crate::editor::bindings_capture::CaptureState::Idle
    ) {
        return; // the capture flow owns the keyboard
    }
    for key in keys.get_just_pressed() {
        for verb in resolver.system_for_key(*key) {
            out.write(crate::events::SystemVerbHit { verb });
        }
    }
}

fn emit_pending_lane_hits(
    clock: Res<GameplayClock>,
    mut pending: ResMut<PendingLaneInputs>,
    mut events: MessageWriter<InputHit>,
) {
    if !clock.is_ready() {
        return;
    }
    if pending.events.is_empty() {
        return;
    }
    let now = Instant::now();
    for captured in std::mem::take(&mut pending.events) {
        events.write(InputHit {
            lanes: captured.lanes,
            audio_ms: compensated_audio_ms(
                clock.current_ms,
                now.saturating_duration_since(captured.captured_at),
            ),
            captured_at: captured.captured_at,
        });
    }
}

fn compensated_audio_ms(current_audio_ms: i64, capture_delay: Duration) -> i64 {
    current_audio_ms.saturating_sub(capture_delay.as_millis() as i64)
}

#[cfg(test)]
fn capture_targets_for_keys(
    keys: impl IntoIterator<Item = KeyCode>,
    resolver: &BindResolver,
) -> Vec<Vec<u8>> {
    keys.into_iter()
        .map(|key| resolver.lanes_for_key(key).collect())
        .filter(|lanes: &Vec<u8>| !lanes.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_drums_default_matches_bocud() {
        // BocuD default: X = HH, Space = BD.
        let r = crate::bindings::BindResolver::default();
        assert_eq!(r.lane_for_key(KeyCode::KeyX), Some(0));
        assert_eq!(r.lane_for_key(KeyCode::Space), Some(2));
    }

    #[test]
    fn input_audio_ms_is_compensated_for_capture_delay() {
        assert_eq!(
            compensated_audio_ms(1000, std::time::Duration::from_millis(12)),
            988
        );
    }

    #[test]
    fn bound_key_emits_the_system_verb() {
        use crate::editor::bindings_capture::CaptureState;
        use crate::events::SystemVerbHit;
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CaptureState>()
            .insert_resource(crate::bindings::BindResolver::from_bindings(&bindings))
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
    fn an_armed_capture_swallows_the_system_verb() {
        use crate::editor::bindings_capture::CaptureState;
        use crate::events::SystemVerbHit;
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut bindings = InputBindings::default();
        bindings.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(CaptureState::Keyboard(dtx_core::EChannel::Snare))
            .insert_resource(crate::bindings::BindResolver::from_bindings(&bindings))
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
            "a key pressed while capturing must not fire a verb"
        );
    }

    #[test]
    fn one_shared_key_captures_one_ordered_input() {
        let mut bindings = dtx_input::InputBindings::default();
        bindings.bind_shared(
            dtx_core::EChannel::LeftBassDrum,
            dtx_input::BindSource::Key(KeyCode::Space),
        );
        let resolver = crate::bindings::BindResolver::from_bindings(&bindings);

        let inputs = capture_targets_for_keys([KeyCode::Space], &resolver);

        assert_eq!(inputs, vec![vec![2, 11]]);
    }
}
