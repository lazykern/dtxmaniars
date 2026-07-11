//! Modal dialog state for profile actions: naming (Save As / Rename),
//! delete confirmation, dirty-draft guards, and corrupt-registry reset.
//!
//! Pure state + reducers; the UI renders whatever state holds. An invalid
//! name submit keeps the dialog open with an inline error so the user can
//! correct it in place.

use bevy::prelude::Resource;
use dtx_persistence::{validate_profile_name, ProfileName, ProfileNameError};

use crate::editor::profile_state::{PendingProfileAction, ProfileKind};

/// What the open name dialog will do with a valid name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameAction {
    SaveAs,
    Rename,
}

/// Current modal dialog, if any. One dialog at a time.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub enum ProfileDialogState {
    #[default]
    Closed,
    /// Save As / Rename name entry with inline validation error.
    Name {
        action: NameAction,
        value: String,
        error: Option<ProfileNameError>,
    },
    ConfirmDelete {
        name: String,
    },
    /// Dirty-draft guard raised by `pending`; buttons map to DirtyDecision.
    Dirty {
        kind: ProfileKind,
        pending: PendingProfileAction,
        builtin_selected: bool,
    },
    /// Corrupt registry detected at load; offers confirmed backup+reset.
    CorruptReset {
        kind: ProfileKind,
        message: String,
    },
}

/// Open the name dialog preloaded with the shared suggestion.
pub fn open_name_dialog(action: NameAction, suggestion: String) -> ProfileDialogState {
    ProfileDialogState::Name {
        action,
        value: suggestion,
        error: None,
    }
}

/// Submit the name dialog. A valid name closes the dialog and returns the
/// validated name; an invalid one keeps the dialog open with the error and
/// the user's text intact.
pub fn submit_name<'a>(
    state: &ProfileDialogState,
    builtins: impl IntoIterator<Item = &'a str>,
    existing: impl IntoIterator<Item = &'a str>,
    current: Option<&str>,
) -> (ProfileDialogState, Option<ProfileName>) {
    let ProfileDialogState::Name { action, value, .. } = state else {
        return (state.clone(), None);
    };
    match validate_profile_name(value, builtins, existing, current) {
        Ok(name) => (ProfileDialogState::Closed, Some(name)),
        Err(error) => (
            ProfileDialogState::Name {
                action: *action,
                value: value.clone(),
                error: Some(error),
            },
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_save_as_keeps_name_dialog_open() {
        let state = open_name_dialog(NameAction::SaveAs, "DTXMania default".to_owned());
        let (next, name) = submit_name(&state, ["DTXMania default"], [], None);
        assert!(name.is_none());
        let ProfileDialogState::Name {
            action,
            value,
            error,
        } = next
        else {
            panic!("invalid name must keep the dialog open");
        };
        assert_eq!(action, NameAction::SaveAs);
        assert_eq!(value, "DTXMania default", "user text is retained");
        assert!(error.is_some(), "inline error is shown");
    }

    #[test]
    fn valid_name_closes_dialog() {
        let state = open_name_dialog(NameAction::Rename, "My kit".to_owned());
        let (next, name) = submit_name(&state, ["DTXMania default"], ["Desk"], Some("Desk"));
        assert_eq!(next, ProfileDialogState::Closed);
        assert_eq!(name.expect("name validates").as_str(), "My kit");
    }
}
