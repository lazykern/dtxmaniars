//! Binding-capture state machine for the Bindings tab.
//!
//! `+` on a channel row (bindings_panel) arms `CaptureState::Capturing(ch)`.
//! From there `capture_binding` listens for the first input (keyboard key or
//! MIDI NoteOn), refuses reserved keys, and either binds immediately or
//! — when a MIDI note already belongs to another channel — routes through a
//! `ConfirmSteal` step so no bind is stolen silently (Enter steals, Esc cancels).
//!
//! Esc while `Capturing`/`ConfirmSteal` cancels capture WITHOUT closing the
//! surface: `close_on_escape` (ui.rs) is gated `not(capture_active)` so the same
//! Esc press can't also close the Customize overlay mid-capture.
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

/// Channel that currently owns exclusive `src`, if any. Keyboard binds are
/// intentionally non-exclusive, so only MIDI can conflict.
fn owner_of(bindings: &InputBindings, src: BindSource) -> Option<dtx_core::EChannel> {
    match src {
        BindSource::Key(_) => None,
        BindSource::Midi { note } => bindings.channel_for_note(note),
    }
}

fn bind_captured(bindings: &mut InputBindings, channel: dtx_core::EChannel, src: BindSource) {
    match src {
        BindSource::Key(_) => bindings.bind_shared(channel, src),
        BindSource::Midi { .. } => bindings.bind(channel, src),
    }
}

fn captured_lane_hit(channel: dtx_core::EChannel, audio_ms: i64) -> Option<LaneHit> {
    lane_of(channel).map(|lane| LaneHit { lane, audio_ms })
}

/// Drive the capture state machine: first non-reserved input wins; conflicts go
/// through `ConfirmSteal`; Esc cancels at any stage.
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
    match *capture {
        CaptureState::Idle => {
            // While idle, track the latest MIDI hit so a pre-existing (stale)
            // hit isn't instantly consumed on the first frame of the next
            // capture — only a strictly-newer NoteOn counts once armed.
            *seen_midi_at = last_midi.at;
        }
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
            // First candidate wins: keyboard is checked first, then MIDI. A
            // key just pressed this frame beats a NoteOn arriving the same
            // frame (deterministic tie-break).
            let key_candidate = keys
                .get_just_pressed()
                .copied()
                .find(|&k| !is_reserved(k))
                .map(BindSource::Key);
            // A NEW NoteOn (velocity > 0, `at` strictly newer than the one this
            // capture already consumed) offers a MIDI candidate. Advancing
            // `seen_midi_at` here dedupes a held/sustained note so it can't
            // re-bind every frame while the source keeps reporting it.
            let midi_candidate = match last_midi.at {
                Some(t) if last_midi.velocity > 0 && *seen_midi_at != Some(t) => {
                    *seen_midi_at = Some(t);
                    Some(BindSource::Midi {
                        note: last_midi.note,
                    })
                }
                _ => None,
            };
            let candidate = key_candidate.or(midi_candidate);
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
                        bind_captured(&mut live.0, channel, src);
                        rev.0 = rev.0.wrapping_add(1);
                        if clock.is_ready() {
                            if let Some(hit) = captured_lane_hit(channel, clock.current_ms) {
                                hits.write(hit);
                            }
                        }
                        *capture = CaptureState::Idle;
                    }
                }
            }
        }
        CaptureState::ConfirmSteal {
            channel, source, ..
        } => {
            if keys.just_pressed(KeyCode::Enter) {
                bind_captured(&mut live.0, channel, source);
                rev.0 = rev.0.wrapping_add(1);
                if clock.is_ready() {
                    if let Some(hit) = captured_lane_hit(channel, clock.current_ms) {
                        hits.write(hit);
                    }
                }
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
    if active.0 != game_shell::CustomizeTab::Bindings {
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
    fn steal_detection_only_applies_to_midi() {
        let mut ib = InputBindings::default();
        ib.bind(
            dtx_core::EChannel::HiHatClose,
            BindSource::Midi { note: 99 },
        );
        assert_eq!(owner_of(&ib, BindSource::Key(KeyCode::KeyX)), None);
        assert_eq!(
            owner_of(&ib, BindSource::Midi { note: 99 }),
            Some(dtx_core::EChannel::HiHatClose)
        );
    }

    #[test]
    fn capture_feedback_targets_newly_bound_channel() {
        let hit = captured_lane_hit(dtx_core::EChannel::Snare, 1234);
        assert_eq!(hit.map(|value| (value.lane, value.audio_ms)), Some((1, 1234)));
    }
}
