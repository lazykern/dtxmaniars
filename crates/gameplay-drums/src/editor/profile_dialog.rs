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

/// Open the corrupt-registry recovery dialog. `Back up and reset` in the
/// panel routes here first; the destructive reset itself only runs from an
/// explicit confirmation on this dialog.
pub fn open_corrupt_reset(kind: ProfileKind, message: String) -> ProfileDialogState {
    ProfileDialogState::CorruptReset { kind, message }
}

/// Confirm the corrupt-registry reset. Returns the kind to reset only when
/// the recovery dialog is actually open — a stray confirm from any other
/// state performs nothing.
pub fn confirm_corrupt_reset(state: &ProfileDialogState) -> Option<ProfileKind> {
    match state {
        ProfileDialogState::CorruptReset { kind, .. } => Some(*kind),
        _ => None,
    }
}

/// Apply the owning crate's backup/reset outcome to the dialog: success
/// closes it; failure keeps it open with the full cause so the corrupt
/// canonical file is never silently replaced by a default registry.
pub fn apply_reset_outcome(kind: ProfileKind, result: Result<(), String>) -> ProfileDialogState {
    match result {
        Ok(()) => ProfileDialogState::Closed,
        Err(message) => ProfileDialogState::CorruptReset { kind, message },
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
    fn corrupt_registry_shows_read_only_builtins() {
        use crate::editor::profile_state::RegistryHealth;
        let health = RegistryHealth::read_only("cannot parse keyboard-profiles.toml");
        assert!(!health.mutation_allowed());
        assert!(health.error.as_deref().unwrap().contains("parse"));
        let healthy = RegistryHealth::default();
        assert!(healthy.mutation_allowed());
    }

    #[test]
    fn reset_button_requires_second_confirmation() {
        // Reset only fires from the open recovery dialog.
        assert_eq!(confirm_corrupt_reset(&ProfileDialogState::Closed), None);
        let open = open_corrupt_reset(ProfileKind::Keyboard, "corrupt".to_owned());
        assert_eq!(
            confirm_corrupt_reset(&open),
            Some(ProfileKind::Keyboard),
            "confirm works only after the dialog opened"
        );
    }

    #[test]
    fn failed_backup_does_not_create_default_registry() {
        let open = open_corrupt_reset(ProfileKind::Midi, "corrupt".to_owned());
        let kind = confirm_corrupt_reset(&open).expect("dialog open");
        // Owning crate refused (e.g. backup collision): dialog stays open
        // with the cause; no default registry state is installed here.
        let next = apply_reset_outcome(kind, Err("backup exists".to_owned()));
        assert_eq!(
            next,
            ProfileDialogState::CorruptReset {
                kind: ProfileKind::Midi,
                message: "backup exists".to_owned()
            }
        );
        // Success closes the dialog.
        assert_eq!(
            apply_reset_outcome(kind, Ok(())),
            ProfileDialogState::Closed
        );
    }

    #[test]
    fn valid_name_closes_dialog() {
        let state = open_name_dialog(NameAction::Rename, "My kit".to_owned());
        let (next, name) = submit_name(&state, ["DTXMania default"], ["Desk"], Some("Desk"));
        assert_eq!(next, ProfileDialogState::Closed);
        assert_eq!(name.expect("name validates").as_str(), "My kit");
    }
}
