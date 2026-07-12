//! Controls tab segment state and focus contract.
//!
//! The Controls tab hosts two segments (`Keyboard | MIDI`) behind one
//! top-level Customize tab. Focus moves in levels: tab bar → segment
//! selector → mapping rows. Down/Enter descends, Up returns one level,
//! Left/Right switches the segment while the selector has focus. Switching
//! segments or tabs never touches profile drafts.

use bevy::prelude::*;
use dtx_core::EChannel;
use dtx_layout::{lane_chips, LaneArrangement};
use game_shell::{NavAction, NavSource, NavVerb};

use super::bindings_capture::{CaptureState, SelectedChannel};
use super::bindings_panel::BindingsRev;
use crate::bindings::LiveBindings;
use crate::lanes::Lanes;

/// Channels in display order for Controls rows: display lanes left-to-right,
/// primary channel first, remaining mapped channels in canonical
/// `DRUM_CHANNELS` order, each channel exactly once.
pub fn channels_in_display_order(arrangement: &LaneArrangement) -> Vec<EChannel> {
    let mut out = Vec::new();
    for index in 0..arrangement.lanes.len() {
        for channel in lane_chips(arrangement, index) {
            if !out.contains(&channel) {
                out.push(channel);
            }
        }
    }
    out
}

/// How a MIDI port filter resolved against the enumerated input ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMatch {
    /// No filter: connect to the first available port.
    FirstAvailable,
    /// Exact case-sensitive full-name match.
    Exact(usize),
    /// First case-sensitive substring match in enumeration order;
    /// `ambiguous` warns when several ports matched.
    Substring { index: usize, ambiguous: bool },
    /// Filter matched nothing. The profile stays active and editable.
    Disconnected,
}

/// Normalize a stored port filter: empty or whitespace-only becomes `None`
/// (first available).
pub fn normalize_port_filter(filter: Option<&str>) -> Option<String> {
    filter
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .map(str::to_owned)
}

/// Resolve a port filter against enumerated port names. Exact case-sensitive
/// full name wins; otherwise the first case-sensitive substring match in
/// enumeration order, flagged ambiguous when several match; no match is
/// `Disconnected`. Never switches profiles.
pub fn match_midi_port(filter: Option<&str>, enumerated: &[String]) -> PortMatch {
    let Some(filter) = normalize_port_filter(filter) else {
        return PortMatch::FirstAvailable;
    };
    if let Some(index) = enumerated.iter().position(|port| *port == filter) {
        return PortMatch::Exact(index);
    }
    let mut matches = enumerated
        .iter()
        .enumerate()
        .filter(|(_, port)| port.contains(&filter))
        .map(|(index, _)| index);
    match matches.next() {
        Some(index) => PortMatch::Substring {
            index,
            ambiguous: matches.next().is_some(),
        },
        None => PortMatch::Disconnected,
    }
}

/// Which Controls segment is active.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlsSegment {
    #[default]
    Keyboard,
    Midi,
}

impl ControlsSegment {
    pub fn toggled(self) -> Self {
        match self {
            Self::Keyboard => Self::Midi,
            Self::Midi => Self::Keyboard,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Midi => "MIDI",
        }
    }
}

/// Focus level inside the Controls tab.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlsFocus {
    /// Focus rests on the Customize tab bar.
    #[default]
    TabBar,
    /// The `Keyboard | MIDI` segment selector has focus.
    SegmentSelector,
    /// A profile/mapping row inside the active segment has focus.
    Rows,
}

/// Apply one navigation verb to the Controls focus state. Returns the next
/// focus level and segment; drafts are never part of this transition.
pub fn reduce_controls_nav(
    focus: ControlsFocus,
    segment: ControlsSegment,
    verb: NavVerb,
) -> (ControlsFocus, ControlsSegment) {
    match focus {
        ControlsFocus::TabBar => match verb {
            NavVerb::Down | NavVerb::Confirm => (ControlsFocus::SegmentSelector, segment),
            _ => (focus, segment),
        },
        ControlsFocus::SegmentSelector => match verb {
            NavVerb::Dec | NavVerb::Inc => (focus, segment.toggled()),
            NavVerb::Down | NavVerb::Confirm => (ControlsFocus::Rows, segment),
            NavVerb::Up | NavVerb::Back => (ControlsFocus::TabBar, segment),
            _ => (focus, segment),
        },
        ControlsFocus::Rows => match verb {
            NavVerb::Up | NavVerb::Back => (ControlsFocus::SegmentSelector, segment),
            _ => (focus, segment),
        },
    }
}

/// Outcome of one Up/Down step through the Controls rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStep {
    /// Up from the first row: focus returns to the segment selector.
    ToSegmentSelector,
    /// The row cursor lands on this channel.
    Select(EChannel),
    /// Nothing to do (Down on an empty row list).
    None,
}

/// Step the row cursor through `channels` (the panel's display order). A
/// missing or stale `current` clamps to the first channel; Up from the first
/// row hands focus back to the segment selector (the reducer's Rows+Up arm).
pub fn step_channel(channels: &[EChannel], current: Option<EChannel>, dir: i32) -> RowStep {
    if channels.is_empty() {
        return if dir < 0 {
            RowStep::ToSegmentSelector
        } else {
            RowStep::None
        };
    }
    let Some(index) = current.and_then(|ch| channels.iter().position(|c| *c == ch)) else {
        return RowStep::Select(channels[0]);
    };
    if dir < 0 {
        if index == 0 {
            RowStep::ToSegmentSelector
        } else {
            RowStep::Select(channels[index - 1])
        }
    } else {
        RowStep::Select(channels[(index + 1).min(channels.len() - 1)])
    }
}

/// Keyboard-only `NavAction` consumer for the Controls tab. Level moves go
/// through the pure `reduce_controls_nav`; at `Rows` the consumer owns what
/// the reducer doesn't model: the row cursor (`SelectedChannel` through the
/// panel's display order), Enter → capture arming, Backspace → delete the
/// last binding of the selected channel in the active segment.
///
/// Ordered `.after(capture_binding)`: Enter is NOT a reserved capture key,
/// so the press that arms a capture must already be stale when the capture
/// machine next reads the keyboard — and the Enter that commits an Arrived
/// state must not re-enter here and instantly re-arm (covered by the
/// `capture.is_changed()` skip).
#[allow(clippy::too_many_arguments)]
pub(super) fn controls_nav_consumer(
    mut actions: MessageReader<NavAction>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<super::tabs::ActiveTab>,
    lanes: Res<Lanes>,
    mut capture: ResMut<CaptureState>,
    mut focus: ResMut<ControlsFocus>,
    mut segment: ResMut<ControlsSegment>,
    mut selected: ResMut<SelectedChannel>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    if active.0 != game_shell::CustomizeTab::Controls {
        return; // own reader: unread messages just expire
    }
    if active.is_changed() && *focus != ControlsFocus::TabBar {
        // Fresh visit (tab switched here): keyboard focus restarts at the bar.
        *focus = ControlsFocus::TabBar;
    }
    // Only the state matters, never `capture.is_changed()`: `capture_binding`
    // does a `mem::take` on `CaptureState` every frame, so the change tick is
    // always set and testing it would dead-lock this consumer forever. The
    // two self-capture hazards are already closed elsewhere — we run
    // `.after(capture_binding)`, and `keyboard_emit_nav` is gated on
    // `not(capture_active)` so a capture keypress emits no verb at all.
    if !matches!(*capture, CaptureState::Idle) {
        actions.clear();
        return;
    }
    let channels = super::bindings_panel::bindable_channels_in_order(&lanes.0);
    // Backspace is not a NavVerb — read it directly, Rows level only.
    if *focus == ControlsFocus::Rows && keys.just_pressed(KeyCode::Backspace) {
        if let Some(channel) = selected.0 {
            if let Some(index) =
                super::bindings_panel::last_segment_source_index(&live.0, channel, *segment)
            {
                if let Some(sources) = live.0.map.get_mut(&channel) {
                    sources.remove(index);
                    rev.0 = rev.0.wrapping_add(1);
                }
            }
        }
    }
    for action in actions.read() {
        if action.source != NavSource::Keyboard {
            continue;
        }
        match (*focus, action.verb) {
            (ControlsFocus::Rows, NavVerb::Up) | (ControlsFocus::Rows, NavVerb::Down) => {
                let dir = if action.verb == NavVerb::Up { -1 } else { 1 };
                match step_channel(&channels, selected.0, dir) {
                    RowStep::ToSegmentSelector => {
                        let (next_focus, next_segment) =
                            reduce_controls_nav(*focus, *segment, NavVerb::Up);
                        if *focus != next_focus {
                            *focus = next_focus;
                        }
                        if *segment != next_segment {
                            *segment = next_segment;
                        }
                    }
                    RowStep::Select(channel) => {
                        if selected.0 != Some(channel) {
                            selected.0 = Some(channel);
                        }
                    }
                    RowStep::None => {}
                }
            }
            (ControlsFocus::Rows, NavVerb::Confirm) => {
                if let Some(channel) = selected.0.filter(|ch| channels.contains(ch)) {
                    *capture = match *segment {
                        ControlsSegment::Keyboard => CaptureState::Keyboard(channel),
                        ControlsSegment::Midi => CaptureState::Midi(channel),
                    };
                    actions.clear();
                    return; // the capture flow owns input from here
                }
            }
            _ => {
                let (next_focus, next_segment) = reduce_controls_nav(*focus, *segment, action.verb);
                let entered_rows =
                    *focus != ControlsFocus::Rows && next_focus == ControlsFocus::Rows;
                if *focus != next_focus {
                    *focus = next_focus;
                }
                if *segment != next_segment {
                    *segment = next_segment;
                }
                if entered_rows && !selected.0.is_some_and(|ch| channels.contains(&ch)) {
                    // Seed the row cursor so Enter/Backspace always target a row.
                    if let Some(first) = channels.first() {
                        selected.0 = Some(*first);
                    }
                }
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        controls_nav_consumer
            .after(super::bindings_capture::capture_binding)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open)
            .run_if(super::profile_dialog::profile_dialog_closed)
            .run_if(super::profile_state::pending_close_none),
    );
}

#[cfg(test)]
mod tests {
    use game_shell::CustomizeTab;

    use super::*;

    #[test]
    fn empty_port_filter_normalizes_to_none() {
        assert_eq!(normalize_port_filter(None), None);
        assert_eq!(normalize_port_filter(Some("")), None);
        assert_eq!(normalize_port_filter(Some("   ")), None);
        assert_eq!(
            normalize_port_filter(Some(" TD-17 ")),
            Some("TD-17".to_owned())
        );
        assert_eq!(match_midi_port(Some("  "), &[]), PortMatch::FirstAvailable);
    }

    #[test]
    fn port_match_prefers_exact_case_sensitive_name() {
        let ports = vec![
            "TD-17 MIDI 1".to_owned(),
            "TD-17".to_owned(),
            "td-17".to_owned(),
        ];
        assert_eq!(match_midi_port(Some("TD-17"), &ports), PortMatch::Exact(1));
        assert_eq!(match_midi_port(Some("td-17"), &ports), PortMatch::Exact(2));
    }

    #[test]
    fn port_match_uses_first_case_sensitive_substring_and_warns() {
        let ports = vec![
            "Roland TD-17 MIDI 1".to_owned(),
            "Roland TD-17 MIDI 2".to_owned(),
        ];
        assert_eq!(
            match_midi_port(Some("TD-17"), &ports),
            PortMatch::Substring {
                index: 0,
                ambiguous: true
            }
        );
        assert_eq!(
            match_midi_port(Some("MIDI 2"), &ports),
            PortMatch::Substring {
                index: 1,
                ambiguous: false
            }
        );
        // Case-sensitive: lowercase filter does not match uppercase names.
        assert_eq!(
            match_midi_port(Some("td-17"), &ports),
            PortMatch::Disconnected
        );
    }

    #[test]
    fn missing_port_is_disconnected_without_profile_switch() {
        let ports = vec!["Some other device".to_owned()];
        assert_eq!(
            match_midi_port(Some("TD-17"), &ports),
            PortMatch::Disconnected
        );
        // Disconnected is a display state only: nothing here selects another
        // profile — there is no profile input to this API at all.
    }

    #[test]
    fn display_order_uses_primary_then_canonical_secondaries_once() {
        let arrangement = dtx_layout::classic();
        let order = channels_in_display_order(&arrangement);
        // Every drum channel appears exactly once.
        let mut seen = order.clone();
        seen.sort_by_key(|ch| *ch as u32);
        seen.dedup();
        assert_eq!(seen.len(), order.len(), "no channel repeats");
        for (index, lane) in arrangement.lanes.iter().enumerate() {
            let chips = lane_chips(&arrangement, index);
            // Primary channel leads its lane's chip group.
            assert_eq!(chips.first(), Some(&lane.primary));
            // The lane's chips appear as one contiguous run in the flat order.
            let start = order
                .iter()
                .position(|ch| *ch == lane.primary)
                .expect("primary present");
            assert_eq!(&order[start..start + chips.len()], chips.as_slice());
        }
    }

    #[test]
    fn footer_describes_keyboard_capture() {
        use crate::editor::bindings_capture::CaptureState;
        use crate::editor::footer::capture_footer_text;
        assert_eq!(capture_footer_text(&CaptureState::Idle), None);
        let text = capture_footer_text(&CaptureState::Keyboard(dtx_core::EChannel::Snare))
            .expect("keyboard capture has footer text");
        assert!(text.contains("key"), "{text}");
        assert!(text.contains("SD"), "{text}");
        let text = capture_footer_text(&CaptureState::Midi(dtx_core::EChannel::Snare))
            .expect("midi capture has footer text");
        assert!(text.contains("pad"), "{text}");
    }

    #[test]
    fn controls_segment_left_right_switches() {
        let (focus, segment) = reduce_controls_nav(
            ControlsFocus::SegmentSelector,
            ControlsSegment::Keyboard,
            NavVerb::Inc,
        );
        assert_eq!(focus, ControlsFocus::SegmentSelector);
        assert_eq!(segment, ControlsSegment::Midi);
        let (_, segment) =
            reduce_controls_nav(ControlsFocus::SegmentSelector, segment, NavVerb::Dec);
        assert_eq!(segment, ControlsSegment::Keyboard);
    }

    #[test]
    fn controls_down_enters_segment_then_rows() {
        let (focus, segment) = reduce_controls_nav(
            ControlsFocus::TabBar,
            ControlsSegment::Keyboard,
            NavVerb::Down,
        );
        assert_eq!(focus, ControlsFocus::SegmentSelector);
        let (focus, _) = reduce_controls_nav(focus, segment, NavVerb::Down);
        assert_eq!(focus, ControlsFocus::Rows);
    }

    #[test]
    fn controls_up_returns_one_level() {
        let (focus, _) =
            reduce_controls_nav(ControlsFocus::Rows, ControlsSegment::Midi, NavVerb::Up);
        assert_eq!(focus, ControlsFocus::SegmentSelector);
        let (focus, segment) = reduce_controls_nav(focus, ControlsSegment::Midi, NavVerb::Up);
        assert_eq!(focus, ControlsFocus::TabBar);
        assert_eq!(segment, ControlsSegment::Midi, "segment survives leaving");
    }

    #[test]
    fn row_step_walks_display_order_and_hands_off_at_top() {
        use dtx_core::EChannel;
        let chs = [EChannel::LeftCymbal, EChannel::HiHatClose, EChannel::Snare];
        // Stale / missing selection clamps to the first channel.
        assert_eq!(
            step_channel(&chs, None, 1),
            RowStep::Select(EChannel::LeftCymbal)
        );
        assert_eq!(
            step_channel(&chs, Some(EChannel::Cymbal), -1),
            RowStep::Select(EChannel::LeftCymbal),
            "channel not in display order clamps to first"
        );
        // Down walks and clamps at the bottom.
        assert_eq!(
            step_channel(&chs, Some(EChannel::LeftCymbal), 1),
            RowStep::Select(EChannel::HiHatClose)
        );
        assert_eq!(
            step_channel(&chs, Some(EChannel::Snare), 1),
            RowStep::Select(EChannel::Snare)
        );
        // Up from the first row returns focus to the segment selector.
        assert_eq!(
            step_channel(&chs, Some(EChannel::HiHatClose), -1),
            RowStep::Select(EChannel::LeftCymbal)
        );
        assert_eq!(
            step_channel(&chs, Some(EChannel::LeftCymbal), -1),
            RowStep::ToSegmentSelector
        );
        // Empty list: Up hands off, Down does nothing.
        assert_eq!(step_channel(&[], None, -1), RowStep::ToSegmentSelector);
        assert_eq!(step_channel(&[], None, 1), RowStep::None);
    }

    #[test]
    fn keyboard_descends_arms_capture_and_deletes_last_binding() {
        use crate::editor::bindings_capture::{CaptureState, SelectedChannel};
        use crate::editor::bindings_panel::BindingsRev;
        use bevy::prelude::*;
        use dtx_core::EChannel;
        use game_shell::{NavAction, NavSource};

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsFocus>()
            .init_resource::<ControlsSegment>()
            .init_resource::<CaptureState>()
            .init_resource::<SelectedChannel>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::bindings::LiveBindings>()
            .init_resource::<crate::lanes::Lanes>()
            .insert_resource(crate::editor::tabs::ActiveTab(
                game_shell::CustomizeTab::Controls,
            ))
            .add_message::<NavAction>()
            .add_systems(Update, controls_nav_consumer);
        // First update flushes the ActiveTab insertion's change tick.
        app.update();

        fn nav(app: &mut App, verb: NavVerb) {
            app.world_mut()
                .resource_mut::<Messages<NavAction>>()
                .write(NavAction {
                    verb,
                    source: NavSource::Keyboard,
                    coarse: false,
                });
            app.update();
        }

        nav(&mut app, NavVerb::Down);
        assert_eq!(
            *app.world().resource::<ControlsFocus>(),
            ControlsFocus::SegmentSelector
        );
        nav(&mut app, NavVerb::Down);
        assert_eq!(
            *app.world().resource::<ControlsFocus>(),
            ControlsFocus::Rows
        );
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::LeftCymbal),
            "entering Rows seeds the cursor with the first display channel"
        );
        nav(&mut app, NavVerb::Down);
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::HiHatClose)
        );

        // Backspace deletes the LAST keyboard source of the selected channel.
        let before = app
            .world()
            .resource::<crate::bindings::LiveBindings>()
            .0
            .map
            .get(&EChannel::HiHatClose)
            .map(Vec::len)
            .expect("HH has default bindings");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Backspace);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        let after = app
            .world()
            .resource::<crate::bindings::LiveBindings>()
            .0
            .map
            .get(&EChannel::HiHatClose)
            .map(Vec::len)
            .expect("HH row still present");
        assert_eq!(after, before - 1, "one keyboard source removed");
        assert_eq!(
            app.world().resource::<BindingsRev>().0,
            1,
            "repaint requested"
        );

        // Enter arms keyboard capture for the selected channel.
        nav(&mut app, NavVerb::Confirm);
        assert!(matches!(
            *app.world().resource::<CaptureState>(),
            CaptureState::Keyboard(EChannel::HiHatClose)
        ));

        // While the capture is armed the consumer is inert.
        nav(&mut app, NavVerb::Down);
        assert_eq!(
            app.world().resource::<SelectedChannel>().0,
            Some(EChannel::HiHatClose),
            "capture owns input; row cursor frozen"
        );
    }

    #[test]
    fn consumer_survives_capture_state_touched_every_frame() {
        // Production `capture_binding` does a `mem::take` on `CaptureState`
        // every frame, so its change tick is permanently set. A consumer that
        // bailed on `capture.is_changed()` would be dead in the real app while
        // still passing every test that leaves the resource alone.
        use crate::editor::bindings_capture::{CaptureState, SelectedChannel};
        use crate::editor::bindings_panel::BindingsRev;
        use bevy::prelude::*;
        use game_shell::{NavAction, NavSource};

        fn touch_capture(mut capture: ResMut<CaptureState>) {
            let taken = std::mem::take(&mut *capture);
            *capture = taken;
        }

        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ControlsFocus>()
            .init_resource::<ControlsSegment>()
            .init_resource::<CaptureState>()
            .init_resource::<SelectedChannel>()
            .init_resource::<BindingsRev>()
            .init_resource::<crate::bindings::LiveBindings>()
            .init_resource::<crate::lanes::Lanes>()
            .insert_resource(crate::editor::tabs::ActiveTab(
                game_shell::CustomizeTab::Controls,
            ))
            .add_message::<NavAction>()
            .add_systems(Update, (touch_capture, controls_nav_consumer).chain());
        app.update();

        app.world_mut()
            .resource_mut::<Messages<NavAction>>()
            .write(NavAction {
                verb: NavVerb::Down,
                source: NavSource::Keyboard,
                coarse: false,
            });
        app.update();

        assert_eq!(
            *app.world().resource::<ControlsFocus>(),
            ControlsFocus::SegmentSelector,
            "a permanently-changed CaptureState must not freeze the consumer"
        );
    }

    #[test]
    fn pad_exclusion_matches_controls_contract() {
        // Pads must not navigate the Controls tab: a stray pad hit while
        // testing bindings would move focus.
        assert!(crate::editor::keyboard_nav::pad_excluded(
            CustomizeTab::Controls
        ));
    }
}
