//! Binding-capture state machine for the Bindings tab.
//!
//! `+` on a channel row (bindings_panel) arms `CaptureState::Capturing(ch)`.
//! From there `capture_binding` listens for the first input (keyboard now; MIDI
//! is TODO — see below), refuses reserved keys, and either binds immediately or
//! — when the source already belongs to another channel — routes through a
//! `ConfirmSteal` step so no bind is stolen silently (Enter steals, Esc cancels).
//!
//! Esc while `Capturing`/`ConfirmSteal` cancels capture WITHOUT closing the
//! surface: `close_on_escape` (ui.rs) is gated `not(capture_active)` so the same
//! Esc press can't also close the Customize overlay mid-capture.
//!
//! `pad_hit_autoselect` mirrors spec §5: hitting a pad on the Bindings tab
//! selects that lane's channel into `SelectedChannel` (Task 6's spatial display).

use bevy::prelude::*;
use dtx_config::{BindSource, InputBindings};

use crate::bindings::LiveBindings;
use crate::events::LaneHit;

use super::bindings_panel::BindingsRev;

/// Keyboard/MIDI capture state machine.
#[derive(Resource, Default, Debug, Clone)]
pub enum CaptureState {
    /// Not capturing.
    #[default]
    Idle,
    /// Listening for the first input to bind to this channel.
    Capturing(dtx_core::EChannel),
    /// The captured source already belongs to `from`; await Enter (steal) / Esc.
    ConfirmSteal {
        /// Channel the user is binding.
        channel: dtx_core::EChannel,
        /// Source that was captured.
        source: dtx_config::BindSource,
        /// Channel that currently owns `source`.
        from: dtx_core::EChannel,
    },
}

/// The channel most recently selected by a pad hit on the Bindings tab (drives
/// Task 6's spatial display). `None` until the first hit.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct SelectedChannel(pub Option<dtx_core::EChannel>);

/// Reserved keys that cannot be bound (caller also rejects when Ctrl/Alt/Super
/// held). Escape cancels capture, Tab / function keys drive the surface itself.
pub fn is_reserved(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::Escape
            | KeyCode::Tab
            | KeyCode::F1
            | KeyCode::F2
            | KeyCode::F3
            | KeyCode::F4
            | KeyCode::F5
            | KeyCode::F6
            | KeyCode::F7
            | KeyCode::F8
            | KeyCode::F9
            | KeyCode::F10
            | KeyCode::F11
            | KeyCode::F12
    )
}

/// Run condition: a capture is in progress (used to gate `close_on_escape` off,
/// so Esc cancels capture instead of closing the surface).
pub fn capture_active(state: Res<CaptureState>) -> bool {
    !matches!(*state, CaptureState::Idle)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<CaptureState>()
        .init_resource::<SelectedChannel>()
        .add_systems(
            Update,
            (capture_binding, pad_hit_autoselect)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        );
}

/// Whether any Ctrl/Alt/Super modifier is currently held (such combos are
/// refused as bind sources — they belong to the app's chord space).
fn modifier_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::AltLeft)
        || keys.pressed(KeyCode::AltRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight)
}

/// Channel that currently owns `src`, if any (dispatch over the public lookups
/// since `InputBindings::channel_for` is private).
fn owner_of(bindings: &InputBindings, src: BindSource) -> Option<dtx_core::EChannel> {
    match src {
        BindSource::Key(k) => bindings.channel_for_key(k),
        BindSource::Midi { note } => bindings.channel_for_note(note),
    }
}

/// Drive the capture state machine: first non-reserved input wins; conflicts go
/// through `ConfirmSteal`; Esc cancels at any stage.
fn capture_binding(
    keys: Res<ButtonInput<KeyCode>>,
    mut capture: ResMut<CaptureState>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    match *capture {
        CaptureState::Idle => {}
        CaptureState::Capturing(channel) => {
            // Esc cancels FIRST (before reserved-key logic), and never binds.
            if keys.just_pressed(KeyCode::Escape) {
                *capture = CaptureState::Idle;
                return;
            }
            // Refuse bindings while a modifier is held (Ctrl/Alt/Super combos).
            if modifier_held(&keys) {
                return;
            }
            // First candidate wins. Keyboard only for Phase 3a.
            // TODO(3b): also drain just-arrived MIDI notes from the same source
            // poll_midi reads (dtx_input::midi::VirtualSource) and offer the
            // first NoteOn as `BindSource::Midi { note }`. Draining here would
            // race poll_midi's FixedUpdate drain, so it's deferred until the
            // real-device path (3b) owns MIDI event routing.
            let candidate = keys
                .get_just_pressed()
                .copied()
                .find(|&k| !is_reserved(k))
                .map(BindSource::Key);
            if let Some(src) = candidate {
                match owner_of(&live.0, src) {
                    Some(other) if other != channel => {
                        *capture = CaptureState::ConfirmSteal {
                            channel,
                            source: src,
                            from: other,
                        };
                    }
                    _ => {
                        live.0.bind(channel, src);
                        rev.0 = rev.0.wrapping_add(1);
                        *capture = CaptureState::Idle;
                    }
                }
            }
        }
        CaptureState::ConfirmSteal {
            channel, source, ..
        } => {
            if keys.just_pressed(KeyCode::Enter) {
                live.0.bind(channel, source);
                rev.0 = rev.0.wrapping_add(1);
                *capture = CaptureState::Idle;
            } else if keys.just_pressed(KeyCode::Escape) {
                *capture = CaptureState::Idle;
            }
        }
    }
}

/// On the Bindings tab, a pad hit selects its lane's channel (spec §5). Always
/// drains the reader so the cursor doesn't replay stale hits when the tab flips.
fn pad_hit_autoselect(
    mut hits: MessageReader<LaneHit>,
    active: Res<super::tabs::ActiveTab>,
    mut selected: ResMut<SelectedChannel>,
) {
    let on_bindings = active.0 == game_shell::CustomizeTab::Bindings;
    for hit in hits.read() {
        if on_bindings {
            if let Some(ch) = crate::lane_map::lane_channel(hit.lane) {
                selected.0 = Some(ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_keys_refused() {
        assert!(is_reserved(KeyCode::Escape));
        assert!(is_reserved(KeyCode::F5));
        assert!(is_reserved(KeyCode::Tab));
        assert!(!is_reserved(KeyCode::KeyX));
    }

    #[test]
    fn steal_detection_finds_prior_owner() {
        let mut ib = InputBindings::default();
        ib.bind(
            dtx_core::EChannel::HiHatClose,
            BindSource::Key(KeyCode::KeyX),
        );
        // KeyX now owned by HiHatClose: a fresh bind attempt from another
        // channel must detect the conflict for the steal-confirm flow.
        assert_eq!(
            owner_of(&ib, BindSource::Key(KeyCode::KeyX)),
            Some(dtx_core::EChannel::HiHatClose)
        );
        assert_eq!(owner_of(&ib, BindSource::Key(KeyCode::KeyQ)), None);
    }
}
