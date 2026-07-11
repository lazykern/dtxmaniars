//! Binding-capture state machine for the Controls tab.
//!
//! `+` on a channel row (bindings_panel) arms `CaptureState::Keyboard(ch)` or
//! `CaptureState::Midi(ch)` depending on the active Controls segment. The
//! keyboard machine listens only for key presses (refusing reserved keys and
//! modifier combos) and binds shared; the MIDI machine listens only for
//! strictly-new NoteOns and — when a note already belongs to another channel —
//! routes through `ConfirmMidiSteal` so no bind is stolen silently (Enter
//! steals, Esc cancels).
//!
//! Esc while capturing/confirming cancels WITHOUT closing the surface:
//! `close_on_escape` (ui.rs) is gated `not(capture_active)` so the same Esc
//! press can't also close the Customize overlay mid-capture.
//!
//! Channel selection (drives `SelectedChannel` → the spatial lane display):
//! clicking a channel row (`select_channel_on_row_click`) is the primary path;
//! a REAL hardware NoteOn also auto-selects its channel (`midi_hit_autoselect`,
//! spec §5). Selection is deliberately NOT driven by the autoplay `LaneHit`
//! stream — that made the pick chase whatever note was being judged.

use bevy::prelude::*;
use dtx_config::{BindSource, InputBindings};

use crate::bindings::LiveBindings;
use crate::events::LaneHit;
use crate::lane_map::lane_of;
use crate::resources::GameplayClock;

use super::bindings_panel::{BindChannelRow, BindingsRev};

/// Source-split capture state machine. Keyboard capture listens only to the
/// keyboard; MIDI learning listens only to strictly-new NoteOns. A stray hit
/// on the other device can never bind.
#[derive(Resource, Default, Debug, Clone)]
pub enum CaptureState {
    /// Not capturing.
    #[default]
    Idle,
    /// `Add key`: listening for the first bindable key for this channel.
    Keyboard(dtx_core::EChannel),
    /// `Learn pad`: listening for the next new NoteOn for this channel.
    Midi(dtx_core::EChannel),
    /// The learned note already belongs to `from`; await Enter (steal) / Esc.
    ConfirmMidiSteal {
        /// Channel the user is binding.
        channel: dtx_core::EChannel,
        /// Note that was captured.
        note: u8,
        /// Channel that currently owns `note`.
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
            (
                capture_binding,
                select_channel_on_row_click,
                midi_hit_autoselect,
                highlight_selected_row,
            )
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

fn captured_lane_hit(channel: dtx_core::EChannel, audio_ms: i64) -> Option<LaneHit> {
    lane_of(channel).map(|lane| LaneHit { lane, audio_ms })
}

/// Outcome of one keyboard-capture step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardCaptureStep {
    Pending,
    Cancelled,
    Bind(KeyCode),
}

/// Pure keyboard-capture decision: Esc cancels, modifiers and reserved keys
/// are refused, MIDI input is invisible to this machine. A captured key is
/// bound shared — it never steals from another channel.
pub fn keyboard_capture_step(
    escape: bool,
    modifier_held: bool,
    new_key: Option<KeyCode>,
) -> KeyboardCaptureStep {
    if escape {
        return KeyboardCaptureStep::Cancelled;
    }
    if modifier_held {
        return KeyboardCaptureStep::Pending;
    }
    match new_key {
        Some(key) if !is_reserved(key) => KeyboardCaptureStep::Bind(key),
        _ => KeyboardCaptureStep::Pending,
    }
}

/// A NoteOn is learnable only if it is strictly newer than the last hit this
/// capture already consumed and has positive velocity. A stale hit that
/// predates arming can never be learned.
pub fn strictly_new_note(
    note: u8,
    velocity: u8,
    at: Option<std::time::Instant>,
    seen: Option<std::time::Instant>,
) -> Option<u8> {
    match at {
        Some(t) if velocity > 0 && seen != Some(t) => Some(note),
        _ => None,
    }
}

/// Outcome of one MIDI-learn step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiCaptureStep {
    Pending,
    Cancelled,
    Bind(u8),
    ConfirmSteal { note: u8, from: dtx_core::EChannel },
}

/// Pure MIDI-learn decision: Esc cancels, keyboard keys are invisible, only a
/// strictly-new positive-velocity NoteOn counts. A note owned by another
/// channel routes through explicit steal confirmation.
pub fn midi_capture_step(
    channel: dtx_core::EChannel,
    escape: bool,
    new_note: Option<u8>,
    owner: impl Fn(u8) -> Option<dtx_core::EChannel>,
) -> MidiCaptureStep {
    if escape {
        return MidiCaptureStep::Cancelled;
    }
    let Some(note) = new_note else {
        return MidiCaptureStep::Pending;
    };
    match owner(note) {
        Some(from) if from != channel => MidiCaptureStep::ConfirmSteal { note, from },
        _ => MidiCaptureStep::Bind(note),
    }
}

/// Drive the source-split capture machine: keyboard capture sees only keys,
/// MIDI learning sees only new NoteOns; Esc cancels at any stage.
fn capture_binding(
    keys: Res<ButtonInput<KeyCode>>,
    last_midi: Res<crate::LastMidiHit>,
    mut seen_midi_at: Local<Option<std::time::Instant>>,
    mut capture: ResMut<CaptureState>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
    clock: Res<GameplayClock>,
    mut hits: MessageWriter<LaneHit>,
) {
    let mut bind = |live: &mut LiveBindings,
                    rev: &mut BindingsRev,
                    hits: &mut MessageWriter<LaneHit>,
                    channel: dtx_core::EChannel,
                    src: BindSource| {
        match src {
            BindSource::Key(_) => live.0.bind_shared(channel, src),
            BindSource::Midi { .. } => live.0.bind(channel, src),
        }
        rev.0 = rev.0.wrapping_add(1);
        if clock.is_ready() {
            if let Some(hit) = captured_lane_hit(channel, clock.current_ms) {
                hits.write(hit);
            }
        }
    };
    match *capture {
        CaptureState::Idle => {
            // While idle, track the latest MIDI hit so a pre-existing (stale)
            // hit isn't instantly consumed on the first frame of the next
            // capture — only a strictly-newer NoteOn counts once armed.
            *seen_midi_at = last_midi.at;
        }
        CaptureState::Keyboard(channel) => {
            // First non-reserved key wins even when a reserved key lands the
            // same frame; the step still refuses reserved keys defensively.
            let step = keyboard_capture_step(
                keys.just_pressed(KeyCode::Escape),
                modifier_held(&keys),
                keys.get_just_pressed().copied().find(|k| !is_reserved(*k)),
            );
            match step {
                KeyboardCaptureStep::Pending => {}
                KeyboardCaptureStep::Cancelled => *capture = CaptureState::Idle,
                KeyboardCaptureStep::Bind(key) => {
                    bind(
                        &mut live,
                        &mut rev,
                        &mut hits,
                        channel,
                        BindSource::Key(key),
                    );
                    *capture = CaptureState::Idle;
                }
            }
        }
        CaptureState::Midi(channel) => {
            // Advancing `seen_midi_at` dedupes a held/sustained note so it
            // can't re-bind every frame.
            let new_note = strictly_new_note(
                last_midi.note,
                last_midi.velocity,
                last_midi.at,
                *seen_midi_at,
            );
            if new_note.is_some() {
                *seen_midi_at = last_midi.at;
            }
            let step = midi_capture_step(
                channel,
                keys.just_pressed(KeyCode::Escape),
                new_note,
                |note| live.0.channel_for_note(note),
            );
            match step {
                MidiCaptureStep::Pending => {}
                MidiCaptureStep::Cancelled => *capture = CaptureState::Idle,
                MidiCaptureStep::Bind(note) => {
                    bind(
                        &mut live,
                        &mut rev,
                        &mut hits,
                        channel,
                        BindSource::Midi { note },
                    );
                    *capture = CaptureState::Idle;
                }
                MidiCaptureStep::ConfirmSteal { note, from } => {
                    *capture = CaptureState::ConfirmMidiSteal {
                        channel,
                        note,
                        from,
                    };
                }
            }
        }
        CaptureState::ConfirmMidiSteal { channel, note, .. } => {
            if keys.just_pressed(KeyCode::Enter) {
                bind(
                    &mut live,
                    &mut rev,
                    &mut hits,
                    channel,
                    BindSource::Midi { note },
                );
                *capture = CaptureState::Idle;
            } else if keys.just_pressed(KeyCode::Escape) {
                *capture = CaptureState::Idle;
            }
        }
    }
}

/// Clicking a channel row selects it (drives the spatial lane display + the
/// capture target). This is the primary way to pick a channel — the old
/// autoplay-driven `pad_hit_autoselect` made the selection chase whatever note
/// the autoplay was judging, so a user could never hold a channel selected.
fn select_channel_on_row_click(
    rows: Query<(&Interaction, &BindChannelRow), Changed<Interaction>>,
    mut selected: ResMut<SelectedChannel>,
) {
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            selected.0 = Some(row.0);
        }
    }
}

/// A REAL hardware NoteOn (via `LastMidiHit`) auto-selects the channel it's
/// bound to (spec §5, DJMAX-style "hit a pad to inspect it"). Driven off MIDI —
/// not the autoplay `LaneHit` stream — so autoplay never moves the selection.
fn midi_hit_autoselect(
    active: Res<super::tabs::ActiveTab>,
    last_midi: Res<crate::LastMidiHit>,
    live: Res<LiveBindings>,
    mut seen_at: Local<Option<std::time::Instant>>,
    mut selected: ResMut<SelectedChannel>,
) {
    if active.0 != game_shell::CustomizeTab::Controls {
        return;
    }
    match last_midi.at {
        Some(t) if last_midi.velocity > 0 && *seen_at != Some(t) => {
            *seen_at = Some(t);
            if let Some(ch) = live.0.channel_for_note(last_midi.note) {
                selected.0 = Some(ch);
            }
        }
        _ => {}
    }
}

/// Tint the selected channel row so the pick is visible in the list.
fn highlight_selected_row(
    selected: Res<SelectedChannel>,
    mut rows: Query<(&BindChannelRow, &mut BackgroundColor)>,
) {
    for (row, mut bg) in &mut rows {
        let on = selected.0 == Some(row.0);
        *bg = BackgroundColor(if on {
            Color::srgba(0.30, 0.34, 0.42, 1.0)
        } else {
            Color::NONE
        });
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
    fn keyboard_capture_ignores_new_midi_hit() {
        // The keyboard machine has no MIDI input at all: a fresh NoteOn while
        // armed leaves the capture pending.
        let step = keyboard_capture_step(false, false, None);
        assert_eq!(step, KeyboardCaptureStep::Pending);
    }

    #[test]
    fn keyboard_capture_rejects_reserved_and_modified_keys() {
        assert_eq!(
            keyboard_capture_step(false, false, Some(KeyCode::Tab)),
            KeyboardCaptureStep::Pending
        );
        assert_eq!(
            keyboard_capture_step(false, false, Some(KeyCode::F3)),
            KeyboardCaptureStep::Pending
        );
        assert_eq!(
            keyboard_capture_step(false, true, Some(KeyCode::KeyX)),
            KeyboardCaptureStep::Pending
        );
    }

    #[test]
    fn keyboard_capture_adds_shared_key_without_steal() {
        use dtx_core::EChannel;
        // The step machine binds a key already owned elsewhere without any
        // confirm state (shared semantics)...
        assert_eq!(
            keyboard_capture_step(false, false, Some(KeyCode::KeyX)),
            KeyboardCaptureStep::Bind(KeyCode::KeyX)
        );
        // ...and the bind itself is shared: the key stays on its old channel.
        let mut live = LiveBindings(InputBindings::default());
        live.0
            .bind_shared(EChannel::Snare, BindSource::Key(KeyCode::KeyX));
        assert_eq!(
            live.0.channels_for_key(KeyCode::KeyX),
            vec![EChannel::HiHatClose, EChannel::Snare]
        );
    }

    #[test]
    fn escape_cancels_keyboard_capture() {
        assert_eq!(
            keyboard_capture_step(true, false, Some(KeyCode::KeyX)),
            KeyboardCaptureStep::Cancelled
        );
    }

    #[test]
    fn midi_capture_ignores_keyboard() {
        use dtx_core::EChannel;
        // No new NoteOn means pending — the MIDI machine has no keyboard
        // input besides Esc.
        let step = midi_capture_step(EChannel::Snare, false, None, |_| None);
        assert_eq!(step, MidiCaptureStep::Pending);
    }

    #[test]
    fn midi_conflict_requires_confirmed_steal() {
        use dtx_core::EChannel;
        let step = midi_capture_step(EChannel::Snare, false, Some(42), |note| {
            (note == 42).then_some(EChannel::HiHatClose)
        });
        assert_eq!(
            step,
            MidiCaptureStep::ConfirmSteal {
                note: 42,
                from: EChannel::HiHatClose
            }
        );
        // Re-learning a note the channel already owns binds without confirm.
        let step = midi_capture_step(EChannel::Snare, false, Some(38), |note| {
            (note == 38).then_some(EChannel::Snare)
        });
        assert_eq!(step, MidiCaptureStep::Bind(38));
    }

    #[test]
    fn stale_midi_hit_is_not_learned() {
        let armed_at = std::time::Instant::now();
        // Hit consumed before/at arming: stale, never learned.
        assert_eq!(
            strictly_new_note(38, 90, Some(armed_at), Some(armed_at)),
            None
        );
        // No hit at all.
        assert_eq!(strictly_new_note(38, 90, None, None), None);
        // Zero velocity is ignored.
        let later = armed_at + std::time::Duration::from_millis(5);
        assert_eq!(strictly_new_note(38, 0, Some(later), Some(armed_at)), None);
        // Strictly newer positive-velocity NoteOn is learnable.
        assert_eq!(
            strictly_new_note(38, 90, Some(later), Some(armed_at)),
            Some(38)
        );
    }

    #[test]
    fn capture_feedback_targets_newly_bound_channel() {
        let hit = captured_lane_hit(dtx_core::EChannel::Snare, 1234);
        assert_eq!(
            hit.map(|value| (value.lane, value.audio_ms)),
            Some((1, 1234))
        );
    }
}
