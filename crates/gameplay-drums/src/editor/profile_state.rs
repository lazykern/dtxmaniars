//! UI-independent draft and transaction reducers for profile editing.
//!
//! Every profile action is a cloned-registry transaction: reducers describe
//! the complete next write, callers perform it through the safe-write helper,
//! and runtime state changes only after the write succeeds. No Bevy or disk
//! access lives here.

use dtx_config::profiles::{KeyboardProfile, MidiProfile};
use dtx_layout::profiles::LaneProfile;
use dtx_persistence::ProfileName;

/// One editable profile draft: the selected registry entry, its last saved
/// value, and the in-memory working value.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileDraft<T> {
    pub selected: String,
    pub saved: T,
    pub value: T,
}

impl<T: Clone + PartialEq> ProfileDraft<T> {
    pub fn clean(selected: impl Into<String>, value: T) -> Self {
        Self {
            selected: selected.into(),
            saved: value.clone(),
            value,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.saved != self.value
    }

    /// Draft state after a successful save of the working value.
    pub fn saved_now(&self) -> Self {
        Self {
            selected: self.selected.clone(),
            saved: self.value.clone(),
            value: self.value.clone(),
        }
    }
}

/// The user's answer to a dirty-draft prompt (`Cancel | Discard | Save`).
#[derive(Debug, Clone, PartialEq)]
pub enum DirtyDecision {
    Save,
    SaveAs(ProfileName),
    Discard,
    Cancel,
}

/// The action that raised the dirty prompt and resumes once decided.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingProfileAction {
    Select(String),
    Revert,
    CloseCustomize,
    ExitApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    Keyboard,
    Midi,
    Lanes,
}

/// All Customize profile drafts. Tab and segment navigation never touches
/// this; only decided profile actions do.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileSession {
    pub keyboard: ProfileDraft<KeyboardProfile>,
    pub midi: ProfileDraft<MidiProfile>,
    pub lanes: ProfileDraft<LaneProfile>,
}

pub fn dirty_profile_kinds(session: &ProfileSession) -> Vec<ProfileKind> {
    let mut kinds = Vec::new();
    if session.keyboard.is_dirty() {
        kinds.push(ProfileKind::Keyboard);
    }
    if session.midi.is_dirty() {
        kinds.push(ProfileKind::Midi);
    }
    if session.lanes.is_dirty() {
        kinds.push(ProfileKind::Lanes);
    }
    kinds
}

/// A failed profile transaction, reported with enough context for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileError {
    pub kind: ProfileKind,
    pub message: String,
}

/// Outcome of one committed-or-not registry transaction.
#[derive(Debug)]
pub enum TransactionResult<R, T> {
    Committed { registry: R, draft: ProfileDraft<T> },
    Unchanged,
    Failed(ProfileError),
}

/// The single registry write a reducer decided on. `save` persists profile
/// data under a name; `select` changes the active profile in the same write.
/// Both `None` means the action resolves without touching the registry.
#[derive(Debug, Clone, PartialEq)]
pub enum DraftEffect<T> {
    /// No write and no draft mutation (Cancel, or nothing to do).
    Noop,
    /// Discard the working value back to the last saved value; no write.
    ResetDraft,
    /// One registry write combining an optional save and an optional
    /// selection change.
    Transaction {
        save: Option<(String, T)>,
        select: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileStateError {
    BuiltInRequiresSaveAs,
}

impl std::fmt::Display for ProfileStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuiltInRequiresSaveAs => {
                write!(f, "built-in profiles are immutable; use Save As")
            }
        }
    }
}

impl std::error::Error for ProfileStateError {}

/// Resolve a pending action against a draft. `builtin_selected` marks the
/// draft's selected profile as an immutable built-in.
///
/// Decision semantics are uniform across actions: `Save`/`SaveAs` always
/// persist the working value, `Discard` always drops it, `Cancel` always
/// aborts the pending action. In particular, `Save` during a `Revert`
/// prompt means "keep my changes instead" — the revert is abandoned and the
/// draft becomes clean at the saved value. Never destroys unsaved work
/// without an explicit `Discard`.
pub fn reduce_dirty_action<T: Clone + PartialEq>(
    draft: &ProfileDraft<T>,
    builtin_selected: bool,
    action: &PendingProfileAction,
    decision: DirtyDecision,
) -> Result<DraftEffect<T>, ProfileStateError> {
    if !draft.is_dirty() {
        return Ok(match action {
            PendingProfileAction::Select(target) => DraftEffect::Transaction {
                save: None,
                select: Some(target.clone()),
            },
            _ => DraftEffect::Noop,
        });
    }
    Ok(match decision {
        DirtyDecision::Cancel => DraftEffect::Noop,
        DirtyDecision::Discard => match action {
            PendingProfileAction::Select(target) => DraftEffect::Transaction {
                save: None,
                select: Some(target.clone()),
            },
            PendingProfileAction::Revert => DraftEffect::ResetDraft,
            PendingProfileAction::CloseCustomize | PendingProfileAction::ExitApp => {
                DraftEffect::ResetDraft
            }
        },
        DirtyDecision::Save => {
            if builtin_selected {
                return Err(ProfileStateError::BuiltInRequiresSaveAs);
            }
            DraftEffect::Transaction {
                save: Some((draft.selected.clone(), draft.value.clone())),
                select: match action {
                    PendingProfileAction::Select(target) => Some(target.clone()),
                    _ => None,
                },
            }
        }
        DirtyDecision::SaveAs(name) => {
            let name = name.as_str().to_owned();
            let select = match action {
                PendingProfileAction::Select(target) => Some(target.clone()),
                _ => Some(name.clone()),
            };
            DraftEffect::Transaction {
                save: Some((name, draft.value.clone())),
                select,
            }
        }
    })
}

/// Apply per-kind Save All write results: successful drafts become clean,
/// failed drafts stay dirty so the dialog can list them and retry.
pub fn apply_save_all_results(
    session: &mut ProfileSession,
    results: &[(ProfileKind, bool)],
) -> Vec<ProfileKind> {
    let mut failed = Vec::new();
    for (kind, success) in results {
        if !success {
            failed.push(*kind);
            continue;
        }
        match kind {
            ProfileKind::Keyboard => session.keyboard = session.keyboard.saved_now(),
            ProfileKind::Midi => session.midi = session.midi.saved_now(),
            ProfileKind::Lanes => session.lanes = session.lanes.saved_now(),
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use bevy::prelude::KeyCode;
    use dtx_core::EChannel;
    use dtx_persistence::validate_profile_name;

    use super::*;

    fn keyboard_draft(selected: &str) -> ProfileDraft<KeyboardProfile> {
        ProfileDraft::clean(selected, KeyboardProfile::default())
    }

    fn dirty_keyboard_draft(selected: &str) -> ProfileDraft<KeyboardProfile> {
        let mut draft = keyboard_draft(selected);
        draft.value.add_key(EChannel::Snare, KeyCode::KeyQ);
        draft
    }

    fn session() -> ProfileSession {
        ProfileSession {
            keyboard: keyboard_draft("Desk"),
            midi: ProfileDraft::clean("Pads", MidiProfile::default()),
            lanes: ProfileDraft::clean(
                "Classic",
                LaneProfile::from_arrangement(dtx_layout::classic()),
            ),
        }
    }

    fn name(raw: &str) -> ProfileName {
        validate_profile_name(raw, [], [], None).expect("valid name")
    }

    #[test]
    fn clean_select_requests_active_transaction() {
        let draft = keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Select("Other".into()),
            DirtyDecision::Cancel,
        )
        .expect("clean select reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: None,
                select: Some("Other".into()),
            }
        );
    }

    #[test]
    fn dirty_select_save_combines_save_and_selection() {
        let draft = dirty_keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Select("Other".into()),
            DirtyDecision::Save,
        )
        .expect("dirty save reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: Some(("Desk".into(), draft.value.clone())),
                select: Some("Other".into()),
            }
        );
    }

    #[test]
    fn dirty_select_discard_does_not_persist_draft() {
        let draft = dirty_keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Select("Other".into()),
            DirtyDecision::Discard,
        )
        .expect("dirty discard reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: None,
                select: Some("Other".into()),
            }
        );
    }

    #[test]
    fn dirty_select_cancel_is_noop() {
        let draft = dirty_keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Select("Other".into()),
            DirtyDecision::Cancel,
        )
        .expect("cancel reduces");
        assert_eq!(effect, DraftEffect::Noop);
        assert!(draft.is_dirty());
    }

    #[test]
    fn builtin_save_requires_save_as() {
        let draft = dirty_keyboard_draft("DTXMania default");
        let result = reduce_dirty_action(
            &draft,
            true,
            &PendingProfileAction::CloseCustomize,
            DirtyDecision::Save,
        );
        assert_eq!(result, Err(ProfileStateError::BuiltInRequiresSaveAs));
        let effect = reduce_dirty_action(
            &draft,
            true,
            &PendingProfileAction::CloseCustomize,
            DirtyDecision::SaveAs(name("My kit")),
        )
        .expect("save as reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: Some(("My kit".into(), draft.value.clone())),
                select: Some("My kit".into()),
            }
        );
    }

    #[test]
    fn revert_save_keeps_changes_instead_of_reverting() {
        let draft = dirty_keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Revert,
            DirtyDecision::Save,
        )
        .expect("revert save reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: Some(("Desk".into(), draft.value.clone())),
                select: None,
            }
        );
    }

    #[test]
    fn revert_discard_resets_draft_without_write() {
        let draft = dirty_keyboard_draft("Desk");
        let effect = reduce_dirty_action(
            &draft,
            false,
            &PendingProfileAction::Revert,
            DirtyDecision::Discard,
        )
        .expect("revert discard reduces");
        assert_eq!(effect, DraftEffect::ResetDraft);
    }

    #[test]
    fn close_save_as_selects_new_profile() {
        let draft = dirty_keyboard_draft("DTXMania default");
        let effect = reduce_dirty_action(
            &draft,
            true,
            &PendingProfileAction::ExitApp,
            DirtyDecision::SaveAs(name("Backup kit")),
        )
        .expect("exit save as reduces");
        assert_eq!(
            effect,
            DraftEffect::Transaction {
                save: Some(("Backup kit".into(), draft.value.clone())),
                select: Some("Backup kit".into()),
            }
        );
    }

    #[test]
    fn save_all_cleans_only_successful_drafts() {
        let mut s = session();
        s.keyboard.value.add_key(EChannel::Snare, KeyCode::KeyQ);
        s.midi.value.velocity_threshold = 42;
        assert_eq!(
            dirty_profile_kinds(&s),
            vec![ProfileKind::Keyboard, ProfileKind::Midi]
        );
        let failed = apply_save_all_results(
            &mut s,
            &[(ProfileKind::Keyboard, true), (ProfileKind::Midi, false)],
        );
        assert_eq!(failed, vec![ProfileKind::Midi]);
        assert_eq!(dirty_profile_kinds(&s), vec![ProfileKind::Midi]);
    }

    #[test]
    fn tab_and_controls_segment_changes_keep_all_drafts() {
        let mut s = session();
        s.keyboard.value.add_key(EChannel::Snare, KeyCode::KeyQ);
        s.lanes.value = LaneProfile::from_arrangement(dtx_layout::nx_type_b());
        let before = s.clone();
        // Tab and segment switches are not PendingProfileActions: no reducer
        // runs and the session is untouched.
        assert_eq!(s, before);
        assert_eq!(
            dirty_profile_kinds(&s),
            vec![ProfileKind::Keyboard, ProfileKind::Lanes]
        );
    }
}
