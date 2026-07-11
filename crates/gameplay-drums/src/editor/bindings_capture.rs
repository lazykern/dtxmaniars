//! Binding-capture state machine for the Controls tab.
//!
//! `+` on a channel row (bindings_panel) arms `CaptureState::Keyboard(ch)` or
//! `CaptureState::Midi(ch)` depending on the active Controls segment. The
//! keyboard machine listens only for key presses (refusing reserved keys and
//! modifier combos); the MIDI machine listens only for strictly-new NoteOns.
//! Neither commits immediately — a captured source always stops at an
//! `Arrived` preview (`KeyArrived` / `MidiArrived`) before commit. When the
//! source already belongs to another channel, the preview offers a
//! shared/move choice (←/→ toggles, defaulting to shared); Enter commits,
//! Esc cancels. With no conflict, Enter just adds the shared binding.
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
use dtx_input::BindSource;

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
    /// A captured key awaits confirm; `owners` = OTHER channels already
    /// holding it (empty = no conflict, Enter just adds it shared).
    KeyArrived {
        /// Channel the user is binding.
        channel: dtx_core::EChannel,
        /// Key that was captured.
        key: KeyCode,
        /// Other channels that already hold `key`.
        owners: Vec<dtx_core::EChannel>,
        /// Shared vs. move, toggled with ←/→ while a conflict exists.
        choice: ArrivedChoice,
    },
    /// A learned note awaits confirm; velocity is shown in the modal.
    MidiArrived {
        /// Channel the user is binding.
        channel: dtx_core::EChannel,
        /// Note that was captured.
        note: u8,
        /// Velocity of the captured hit (display only).
        velocity: u8,
        /// Other channels that already hold `note`.
        owners: Vec<dtx_core::EChannel>,
        /// Shared vs. move, toggled with ←/→ while a conflict exists.
        choice: ArrivedChoice,
    },
}

/// Shared vs. move choice offered at the `Arrived` stage when a captured
/// source already belongs to another channel. Shared is the default — it
/// never removes an existing binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrivedChoice {
    #[default]
    Shared,
    Move,
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
                capture_binding.run_if(super::profile_dialog::profile_dialog_closed),
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
}

/// Pure MIDI-learn decision: Esc cancels, keyboard keys are invisible, only a
/// strictly-new positive-velocity NoteOn counts. Conflict with an existing
/// owner is no longer decided here — the `Arrived` stage handles it.
pub fn midi_capture_step(escape: bool, new_note: Option<u8>) -> MidiCaptureStep {
    if escape {
        return MidiCaptureStep::Cancelled;
    }
    match new_note {
        Some(note) => MidiCaptureStep::Bind(note),
        None => MidiCaptureStep::Pending,
    }
}

/// Input to the `Arrived` reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivedInput {
    Confirm,
    Cancel,
    Toggle,
    None,
}

/// Outcome of one `Arrived`-stage step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivedStep {
    Pending,
    Cancelled,
    CommitShared,
    CommitMove,
    Choice(ArrivedChoice),
}

/// Pure arrived-stage decision. Toggle flips shared/move only while a conflict
/// exists; Confirm commits the current choice (no conflict → CommitShared,
/// which with zero other owners is a plain add).
pub fn arrived_step(input: ArrivedInput, choice: ArrivedChoice, has_conflict: bool) -> ArrivedStep {
    match input {
        ArrivedInput::Cancel => ArrivedStep::Cancelled,
        ArrivedInput::Confirm => match choice {
            ArrivedChoice::Move if has_conflict => ArrivedStep::CommitMove,
            _ => ArrivedStep::CommitShared,
        },
        ArrivedInput::Toggle if has_conflict => ArrivedStep::Choice(match choice {
            ArrivedChoice::Shared => ArrivedChoice::Move,
            ArrivedChoice::Move => ArrivedChoice::Shared,
        }),
        _ => ArrivedStep::Pending,
    }
}

/// Hitting the SAME note again while `MidiArrived` confirms it (fast-retry
/// re-arm shortcut, so the user doesn't have to reach for Enter).
pub fn rearm_confirms(arrived_note: u8, new_note: Option<u8>) -> bool {
    new_note == Some(arrived_note)
}

/// The conflict set for a capture: every channel already holding the source
/// except the one being bound.
fn other_owners(
    owners: Vec<dtx_core::EChannel>,
    channel: dtx_core::EChannel,
) -> Vec<dtx_core::EChannel> {
    owners.into_iter().filter(|c| *c != channel).collect()
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
    // `steal`=false → bind_shared (add without disturbing other owners);
    // `steal`=true → bind (removes `src` from every other channel first).
    let commit = |live: &mut LiveBindings,
                  rev: &mut BindingsRev,
                  hits: &mut MessageWriter<LaneHit>,
                  channel: dtx_core::EChannel,
                  src: BindSource,
                  steal: bool| {
        if steal {
            live.0.bind(channel, src);
        } else {
            live.0.bind_shared(channel, src);
        }
        rev.0 = rev.0.wrapping_add(1);
        if clock.is_ready() {
            if let Some(hit) = captured_lane_hit(channel, clock.current_ms) {
                hits.write(hit);
            }
        }
    };
    // Arrow/Enter/Esc are shared across both `Arrived` variants.
    let arrived_input = || {
        if keys.just_pressed(KeyCode::Escape) {
            ArrivedInput::Cancel
        } else if keys.just_pressed(KeyCode::Enter) {
            ArrivedInput::Confirm
        } else if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
            ArrivedInput::Toggle
        } else {
            ArrivedInput::None
        }
    };
    // Take by value (leaving Idle) so the `owners: Vec<_>` can be destructured
    // and moved without a per-frame heap clone; every branch returns the next
    // state, assigned once below (Pending returns the current state unchanged).
    let next = match std::mem::take(&mut *capture) {
        CaptureState::Idle => {
            // While idle, track the latest MIDI hit so a pre-existing (stale)
            // hit isn't instantly consumed on the first frame of the next
            // capture — only a strictly-newer NoteOn counts once armed.
            *seen_midi_at = last_midi.at;
            CaptureState::Idle
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
                KeyboardCaptureStep::Pending => CaptureState::Keyboard(channel),
                KeyboardCaptureStep::Cancelled => CaptureState::Idle,
                KeyboardCaptureStep::Bind(key) => CaptureState::KeyArrived {
                    channel,
                    key,
                    owners: other_owners(live.0.channels_for_key(key), channel),
                    choice: ArrivedChoice::default(),
                },
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
            let step = midi_capture_step(keys.just_pressed(KeyCode::Escape), new_note);
            match step {
                MidiCaptureStep::Pending => CaptureState::Midi(channel),
                MidiCaptureStep::Cancelled => CaptureState::Idle,
                MidiCaptureStep::Bind(note) => CaptureState::MidiArrived {
                    channel,
                    note,
                    velocity: last_midi.velocity,
                    owners: other_owners(live.0.channels_for_note(note), channel),
                    choice: ArrivedChoice::default(),
                },
            }
        }
        CaptureState::KeyArrived {
            channel,
            key,
            owners,
            choice,
        } => {
            let has_conflict = !owners.is_empty();
            match arrived_step(arrived_input(), choice, has_conflict) {
                ArrivedStep::Pending => CaptureState::KeyArrived {
                    channel,
                    key,
                    owners,
                    choice,
                },
                ArrivedStep::Cancelled => CaptureState::Idle,
                ArrivedStep::Choice(choice) => CaptureState::KeyArrived {
                    channel,
                    key,
                    owners,
                    choice,
                },
                ArrivedStep::CommitShared => {
                    commit(&mut live, &mut rev, &mut hits, channel, BindSource::Key(key), false);
                    CaptureState::Idle
                }
                ArrivedStep::CommitMove => {
                    commit(&mut live, &mut rev, &mut hits, channel, BindSource::Key(key), true);
                    CaptureState::Idle
                }
            }
        }
        CaptureState::MidiArrived {
            channel,
            note,
            velocity,
            owners,
            choice,
        } => {
            // Advancing `seen_midi_at` dedupes a held/sustained note so it
            // can't re-trigger every frame.
            let new_note = strictly_new_note(
                last_midi.note,
                last_midi.velocity,
                last_midi.at,
                *seen_midi_at,
            );
            if new_note.is_some() {
                *seen_midi_at = last_midi.at;
            }
            if let Some(fresh) = new_note.filter(|n| *n != note) {
                // Fast retry: a different pad hit re-arms in place instead of
                // requiring Esc then a fresh capture.
                CaptureState::MidiArrived {
                    channel,
                    note: fresh,
                    velocity: last_midi.velocity,
                    owners: other_owners(live.0.channels_for_note(fresh), channel),
                    choice: ArrivedChoice::default(),
                }
            } else {
                let has_conflict = !owners.is_empty();
                let input = if rearm_confirms(note, new_note) {
                    ArrivedInput::Confirm
                } else {
                    arrived_input()
                };
                match arrived_step(input, choice, has_conflict) {
                    ArrivedStep::Pending => CaptureState::MidiArrived {
                        channel,
                        note,
                        velocity,
                        owners,
                        choice,
                    },
                    ArrivedStep::Cancelled => CaptureState::Idle,
                    ArrivedStep::Choice(choice) => CaptureState::MidiArrived {
                        channel,
                        note,
                        velocity,
                        owners,
                        choice,
                    },
                    ArrivedStep::CommitShared => {
                        commit(&mut live, &mut rev, &mut hits, channel, BindSource::Midi { note }, false);
                        CaptureState::Idle
                    }
                    ArrivedStep::CommitMove => {
                        commit(&mut live, &mut rev, &mut hits, channel, BindSource::Midi { note }, true);
                        CaptureState::Idle
                    }
                }
            }
        }
    };
    *capture = next;
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

/// Tint the selected channel row (ROW_SELECTED_BG + accent left border) so
/// the pick is visible in the list; an unselected, unbound row keeps its
/// WARN_TINT baseline instead of going transparent.
fn highlight_selected_row(
    selected: Res<SelectedChannel>,
    mut rows: Query<(
        &BindChannelRow,
        Has<super::bindings_panel::UnboundRow>,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (row, unbound, mut bg, mut border) in &mut rows {
        let on = selected.0 == Some(row.0);
        *bg = BackgroundColor(if on {
            super::chrome::ROW_SELECTED_BG
        } else if unbound {
            super::chrome::WARN_TINT
        } else {
            Color::NONE
        });
        *border = BorderColor::all(if on { super::chrome::ACCENT } else { Color::NONE });
    }
}

#[cfg(test)]
mod tests {
    use dtx_input::InputBindings;

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
        // No new NoteOn means pending — the MIDI machine has no keyboard
        // input besides Esc.
        let step = midi_capture_step(false, None);
        assert_eq!(step, MidiCaptureStep::Pending);
    }

    #[test]
    fn midi_capture_binds_regardless_of_conflict() {
        // The step machine no longer decides steal-vs-shared: any strictly
        // new note just captures. Conflict resolution moved to the Arrived
        // stage (`arrived_step`), tested below.
        assert_eq!(midi_capture_step(false, Some(42)), MidiCaptureStep::Bind(42));
        assert_eq!(midi_capture_step(true, Some(42)), MidiCaptureStep::Cancelled);
    }

    #[test]
    fn arrived_reducer_covers_confirm_cancel_toggle() {
        let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Shared, true);
        assert_eq!(d, ArrivedStep::CommitShared);
        let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Move, true);
        assert_eq!(d, ArrivedStep::CommitMove);
        // No conflict: plain commit regardless of choice.
        let d = arrived_step(ArrivedInput::Confirm, ArrivedChoice::Shared, false);
        assert_eq!(d, ArrivedStep::CommitShared);
        let d = arrived_step(ArrivedInput::Cancel, ArrivedChoice::Shared, true);
        assert_eq!(d, ArrivedStep::Cancelled);
        // Toggle flips choice only under conflict.
        let d = arrived_step(ArrivedInput::Toggle, ArrivedChoice::Shared, true);
        assert_eq!(d, ArrivedStep::Choice(ArrivedChoice::Move));
        let d = arrived_step(ArrivedInput::Toggle, ArrivedChoice::Shared, false);
        assert_eq!(d, ArrivedStep::Pending);
        // None is inert.
        assert_eq!(
            arrived_step(ArrivedInput::None, ArrivedChoice::Move, true),
            ArrivedStep::Pending
        );
    }

    #[test]
    fn same_note_again_confirms_arrived() {
        assert!(rearm_confirms(38, Some(38)));
        assert!(!rearm_confirms(38, Some(40)));
        assert!(!rearm_confirms(38, None));
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

    /// Drive `capture_binding` once with Enter pressed against a seeded
    /// `MidiArrived` and return the resulting live bindings. Note 42 is a
    /// HiHatClose default; the target is a conflicting channel already owning
    /// it via `owners`, so the choice actually decides steal vs. share.
    fn run_commit(choice: ArrivedChoice) -> InputBindings {
        use dtx_core::EChannel;
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CaptureState>()
            .init_resource::<LiveBindings>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::LastMidiHit>()
            .init_resource::<GameplayClock>()
            .add_message::<LaneHit>()
            .insert_resource(CaptureState::MidiArrived {
                channel: EChannel::Snare,
                note: 42,
                velocity: 90,
                owners: vec![EChannel::HiHatClose],
                choice,
            })
            .add_systems(Update, capture_binding);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.world().resource::<LiveBindings>().0.clone()
    }

    #[test]
    fn commit_move_steals_source_from_other_channel() {
        use dtx_core::EChannel;
        let b = run_commit(ArrivedChoice::Move);
        let src = BindSource::Midi { note: 42 };
        // Steal: gone from HiHatClose, present on Snare.
        assert!(!b.map[&EChannel::HiHatClose].contains(&src));
        assert!(b.map[&EChannel::Snare].contains(&src));
    }

    #[test]
    fn commit_shared_keeps_source_on_both_channels() {
        use dtx_core::EChannel;
        let b = run_commit(ArrivedChoice::Shared);
        let src = BindSource::Midi { note: 42 };
        // Share: still on HiHatClose AND now also on Snare.
        assert!(b.map[&EChannel::HiHatClose].contains(&src));
        assert!(b.map[&EChannel::Snare].contains(&src));
    }
}
