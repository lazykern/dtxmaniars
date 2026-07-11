//! Shared profile bar model: the reusable selector/action list serving the
//! Keyboard, MIDI, and Lanes surfaces (`[ Profile name v ] [ Save ]
//! [ Save as... ] [ ... ]`) plus the `ProfileUiError` shown under the bar.
//! The transactional writes themselves live in the editor engine
//! (`mod.rs::commit_registry_actions` / `commit_lane_actions`).

use std::path::{Path, PathBuf};

use crate::editor::profile_state::{ProfileError, ProfileKind};

/// An action requested from the shared profile bar.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileBarAction {
    Select(String),
    Save,
    SaveAs,
    Rename,
    Revert,
    Delete,
}

/// A transaction failure surfaced to the UI with its file path and cause.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileUiError {
    pub kind: ProfileKind,
    pub path: PathBuf,
    pub message: String,
}

impl ProfileUiError {
    /// Pair a transaction failure with the canonical registry path so the UI
    /// always shows profile kind, file, and full cause.
    pub fn from_error(error: &ProfileError, path: &Path) -> Self {
        Self {
            kind: error.kind,
            path: path.to_path_buf(),
            message: error.message.clone(),
        }
    }
}

/// One selector entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileBarItem {
    pub name: String,
    pub builtin: bool,
    pub selected: bool,
}

/// Selector list: built-ins first (given order), then user profiles in
/// registry insertion/key order, with the current selection marked.
pub fn profile_bar_items<'a>(
    builtins: impl IntoIterator<Item = &'a str>,
    users: impl IntoIterator<Item = &'a str>,
    selected: &str,
) -> Vec<ProfileBarItem> {
    let mut items: Vec<ProfileBarItem> = builtins
        .into_iter()
        .map(|name| ProfileBarItem {
            name: name.to_owned(),
            builtin: true,
            selected: name == selected,
        })
        .collect();
    items.extend(users.into_iter().map(|name| ProfileBarItem {
        name: name.to_owned(),
        builtin: false,
        selected: name == selected,
    }));
    items
}

/// Overflow menu contents: built-ins only offer Save As; user profiles get
/// Rename, Revert, Delete.
pub fn overflow_actions(builtin_selected: bool) -> Vec<ProfileBarAction> {
    if builtin_selected {
        vec![ProfileBarAction::SaveAs]
    } else {
        vec![
            ProfileBarAction::Rename,
            ProfileBarAction::Revert,
            ProfileBarAction::Delete,
        ]
    }
}

/// Save is enabled only for a dirty draft on a user profile.
pub fn save_enabled(builtin_selected: bool, dirty: bool) -> bool {
    dirty && !builtin_selected
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn profile_bar_groups_builtins_before_users() {
        let items = profile_bar_items(["DTXMania default"], ["Desk", "Travel"], "Desk");
        assert_eq!(
            items
                .iter()
                .map(|item| (item.name.as_str(), item.builtin, item.selected))
                .collect::<Vec<_>>(),
            vec![
                ("DTXMania default", true, false),
                ("Desk", false, true),
                ("Travel", false, false),
            ]
        );
    }

    #[test]
    fn builtin_overflow_only_offers_save_as() {
        assert_eq!(overflow_actions(true), vec![ProfileBarAction::SaveAs]);
        assert!(!save_enabled(true, true));
    }

    #[test]
    fn user_overflow_offers_rename_revert_delete() {
        assert_eq!(
            overflow_actions(false),
            vec![
                ProfileBarAction::Rename,
                ProfileBarAction::Revert,
                ProfileBarAction::Delete,
            ]
        );
        assert!(save_enabled(false, true));
        assert!(!save_enabled(false, false));
    }

    #[test]
    fn transaction_error_contains_path_and_cause() {
        let error = ProfileError {
            kind: ProfileKind::Midi,
            message: "midi-profiles.toml: disk full".to_owned(),
        };
        let ui = ProfileUiError::from_error(&error, &PathBuf::from("/cfg/midi-profiles.toml"));
        assert_eq!(ui.kind, ProfileKind::Midi);
        assert_eq!(ui.path, PathBuf::from("/cfg/midi-profiles.toml"));
        assert!(ui.message.contains("disk full"));
    }
}
