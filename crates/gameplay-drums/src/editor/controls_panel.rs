//! Controls tab segment state and focus contract.
//!
//! The Controls tab hosts two segments (`Keyboard | MIDI`) behind one
//! top-level Customize tab. Focus moves in levels: tab bar → segment
//! selector → mapping rows. Down/Enter descends, Up returns one level,
//! Left/Right switches the segment while the selector has focus. Switching
//! segments or tabs never touches profile drafts.

use bevy::prelude::*;
use game_shell::NavVerb;

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
