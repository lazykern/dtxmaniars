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
use game_shell::NavVerb;

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
    fn pad_exclusion_matches_controls_contract() {
        // Pads must not navigate the Controls tab: a stray pad hit while
        // testing bindings would move focus.
        assert!(crate::editor::keyboard_nav::pad_excluded(
            CustomizeTab::Controls
        ));
    }
}
