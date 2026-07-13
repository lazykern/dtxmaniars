use bevy::prelude::Component;

use crate::{InteractionTone, StateMarker};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogAction {
    Confirm,
    Cancel,
    Destructive,
    Custom(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationSource {
    Keyboard,
    Pad,
    Pointer,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActionButtonState {
    #[default]
    Idle,
    Focused,
    Pressed,
    Disabled,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionButton {
    pub action: DialogAction,
    pub state: ActionButtonState,
    pub tone: InteractionTone,
    pub marker: StateMarker,
}

impl ActionButton {
    pub fn new(action: DialogAction, tone: InteractionTone) -> Self {
        Self {
            action,
            state: ActionButtonState::Idle,
            tone,
            marker: tone.marker(),
        }
    }
}

pub const fn reduce_activation(
    _source: ActivationSource,
    action: DialogAction,
    disabled: bool,
) -> Option<DialogAction> {
    if disabled {
        None
    } else {
        Some(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_activation_sources_emit_the_same_action() {
        let action = DialogAction::Confirm;
        for source in [
            ActivationSource::Keyboard,
            ActivationSource::Pad,
            ActivationSource::Pointer,
        ] {
            assert_eq!(reduce_activation(source, action, false), Some(action));
        }
        assert_eq!(
            reduce_activation(ActivationSource::Keyboard, action, true),
            None
        );
    }
}
