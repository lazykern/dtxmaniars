//! Shared profile bar model and transactional action path.
//!
//! One reusable model serves the Keyboard, MIDI, and Lanes surfaces:
//! `[ Profile name v ] [ Save ] [ Save as... ] [ ... ]`. Every action runs a
//! cloned-registry transaction — build the complete next registry, write it
//! atomically, then update runtime state. A failed write leaves the prior
//! file, selection, and draft untouched and forces a canonical re-read
//! before the next write.

use std::path::{Path, PathBuf};

use crate::editor::profile_state::{ProfileDraft, ProfileError, ProfileKind, TransactionResult};

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

/// Per-kind transaction gate. After a failed write the canonical registry
/// must be re-read before the next write so a stale in-memory registry can
/// never clobber on-disk state that changed underneath the failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransactionGate {
    pub needs_reload: bool,
}

/// Run one profile transaction: optionally re-read the canonical registry
/// (after an earlier failure), build the complete next registry and draft,
/// write it, and report the outcome. All I/O comes in as closures so this
/// stays testable without disk or Bevy.
#[allow(clippy::type_complexity)]
pub fn run_transaction<R, T: Clone + PartialEq>(
    gate: &mut TransactionGate,
    registry: &R,
    kind: ProfileKind,
    path: &Path,
    reload: impl FnOnce() -> Result<R, String>,
    build_next: impl FnOnce(&R) -> Result<(R, ProfileDraft<T>), String>,
    write: impl FnOnce(&R) -> Result<(), String>,
) -> TransactionResult<R, T>
where
    R: Clone,
{
    let canonical: R = if gate.needs_reload {
        match reload() {
            Ok(fresh) => fresh,
            Err(message) => {
                return TransactionResult::Failed(ProfileError {
                    kind,
                    message: format!("{}: reload failed: {message}", path.display()),
                });
            }
        }
    } else {
        registry.clone()
    };
    let (next_registry, next_draft) = match build_next(&canonical) {
        Ok(next) => next,
        Err(message) => {
            return TransactionResult::Failed(ProfileError {
                kind,
                message: format!("{}: {message}", path.display()),
            });
        }
    };
    match write(&next_registry) {
        Ok(()) => {
            gate.needs_reload = false;
            TransactionResult::Committed {
                registry: next_registry,
                draft: next_draft,
            }
        }
        Err(message) => {
            gate.needs_reload = true;
            TransactionResult::Failed(ProfileError {
                kind,
                message: format!("{}: {message}", path.display()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;

    use dtx_input::profiles::{
        keyboard_builtins, reduce_registry, KeyboardProfile, ProfileRegistry, RegistryAction,
    };

    use super::*;

    fn user_registry(names: &[&str]) -> ProfileRegistry<KeyboardProfile> {
        let mut registry = dtx_input::profiles::keyboard_registry();
        for name in names {
            registry
                .profiles
                .insert((*name).to_owned(), KeyboardProfile::default());
        }
        registry
    }

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
    fn transaction_failure_keeps_draft_and_active_selection() {
        let registry = user_registry(&["Desk"]);
        let mut gate = TransactionGate::default();
        let draft = ProfileDraft::clean("Desk", KeyboardProfile::default());
        let result = run_transaction(
            &mut gate,
            &registry,
            ProfileKind::Keyboard,
            &PathBuf::from("keyboard-profiles.toml"),
            || unreachable!("no reload needed on first write"),
            |canonical| {
                let next = reduce_registry(
                    canonical,
                    &keyboard_builtins(),
                    RegistryAction::Select("DTXMania default".to_owned()),
                )
                .map_err(|error| error.to_string())?;
                Ok((
                    next,
                    ProfileDraft::clean("DTXMania default", KeyboardProfile::default()),
                ))
            },
            |_| Err("disk full".to_owned()),
        );
        let TransactionResult::<_, KeyboardProfile>::Failed(error) = result else {
            panic!("write failure must fail the transaction");
        };
        assert_eq!(error.kind, ProfileKind::Keyboard);
        assert!(error.message.contains("disk full"));
        assert!(error.message.contains("keyboard-profiles.toml"));
        assert!(gate.needs_reload);
        // Caller keeps the prior registry, draft, and selection on failure.
        assert_eq!(registry.active, "DTXMania default");
        assert_eq!(draft.selected, "Desk");
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

    #[test]
    fn canonical_reread_blocks_write_after_external_corruption() {
        let registry = user_registry(&["Desk"]);
        let mut gate = TransactionGate { needs_reload: true };
        let result: TransactionResult<_, KeyboardProfile> = run_transaction(
            &mut gate,
            &registry,
            ProfileKind::Keyboard,
            &PathBuf::from("keyboard-profiles.toml"),
            || Err("registry is malformed".to_owned()),
            |_| unreachable!("must not build on a failed reload"),
            |_| unreachable!("must not write after a failed reload"),
        );
        let TransactionResult::Failed(error) = result else {
            panic!("malformed canonical registry must abort the write");
        };
        assert!(error.message.contains("malformed"));
        assert!(gate.needs_reload, "gate stays armed until a clean reload");
    }

    #[test]
    fn next_write_reloads_after_failure() {
        let registry = user_registry(&["Desk"]);
        let mut gate = TransactionGate { needs_reload: true };
        let reloaded = Cell::new(false);
        let result = run_transaction(
            &mut gate,
            &registry,
            ProfileKind::Keyboard,
            &PathBuf::from("keyboard-profiles.toml"),
            || {
                reloaded.set(true);
                Ok(user_registry(&["Desk", "AddedElsewhere"]))
            },
            |canonical| {
                assert!(
                    canonical.profiles.contains_key("AddedElsewhere"),
                    "transaction must build on the reloaded canonical registry"
                );
                Ok((
                    canonical.clone(),
                    ProfileDraft::clean("Desk", KeyboardProfile::default()),
                ))
            },
            |_| Ok(()),
        );
        assert!(reloaded.get());
        assert!(matches!(result, TransactionResult::Committed { .. }));
        assert!(!gate.needs_reload);
    }
}
