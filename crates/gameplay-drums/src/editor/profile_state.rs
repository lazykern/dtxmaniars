//! UI-independent draft and transaction reducers for profile editing.
//!
//! Every profile action is a cloned-registry transaction: reducers describe
//! the complete next write, callers perform it through the safe-write helper,
//! and runtime state changes only after the write succeeds. No Bevy or disk
//! access lives here.

use bevy::prelude::Resource;
use dtx_input::profiles::{KeyboardProfile, MidiProfile};
use dtx_layout::profiles::{LaneProfile, LANE_DEFAULT_NAME};
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
    Settings,
}

/// All Customize profile drafts. Tab and segment navigation never touches
/// this; only decided profile actions do.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileSession {
    pub keyboard: ProfileDraft<KeyboardProfile>,
    pub midi: ProfileDraft<MidiProfile>,
    pub lanes: ProfileDraft<LaneProfile>,
}

impl Default for ProfileSession {
    fn default() -> Self {
        Self {
            keyboard: ProfileDraft::clean(
                dtx_input::profiles::KEYBOARD_DEFAULT_NAME,
                KeyboardProfile::default(),
            ),
            midi: ProfileDraft::clean(
                dtx_input::profiles::MIDI_DEFAULT_NAME,
                MidiProfile::default(),
            ),
            lanes: ProfileDraft::clean(
                LANE_DEFAULT_NAME,
                LaneProfile::from_arrangement(dtx_layout::classic()),
            ),
        }
    }
}

/// Runtime home of the Customize profile session.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct CustomizeSession(pub ProfileSession);

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

/// The lane profile draft edited on the Lanes tab. Manual edits mutate the
/// draft arrangement (keeping the selected profile name) and the playfield
/// preview mirrors it; the committed registry changes only via the profile
/// bar's transactional actions.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct LaneProfileDraft(pub ProfileDraft<LaneProfile>);

impl Default for LaneProfileDraft {
    fn default() -> Self {
        Self(ProfileDraft::clean(
            LANE_DEFAULT_NAME,
            LaneProfile::from_arrangement(dtx_layout::classic()),
        ))
    }
}

/// A failed profile transaction, reported with enough context for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileError {
    pub kind: ProfileKind,
    pub message: String,
}

/// Per-kind registry health derived from startup. A registry that could not
/// be read or validated runs read-only on built-ins: every profile mutation
/// is disabled until the user confirms a backup-and-reset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegistryHealth {
    /// Registry unusable: show built-ins, disable all profile mutation.
    pub read_only: bool,
    /// Human-readable load error for the recovery dialog.
    pub error: Option<String>,
}

impl RegistryHealth {
    pub fn read_only(error: impl Into<String>) -> Self {
        Self {
            read_only: true,
            error: Some(error.into()),
        }
    }

    pub fn mutation_allowed(&self) -> bool {
        !self.read_only
    }
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

/// What a close request wants to close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseIntent {
    Customize,
    GracefulAppExit,
}

/// A close intercepted because drafts were dirty; the surface stays open
/// until the user decides.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingClose {
    pub intent: CloseIntent,
    pub dirty: Vec<ProfileKind>,
}

/// Runtime holder for an intercepted close.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub enum PendingCloseState {
    #[default]
    None,
    Pending(PendingClose),
}

/// Run condition: the dirty-close guard dialog is NOT up. Keyboard nav
/// (and the new per-tab consumers) must yield to the dialog exactly as they
/// already yield to profile dialogs via `profile_dialog_closed`.
pub fn pending_close_none(pending: bevy::prelude::Res<PendingCloseState>) -> bool {
    matches!(*pending, PendingCloseState::None)
}

/// The user's answer to the close guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    Cancel,
    DiscardAll,
    SaveAll,
}

/// Whether a close may proceed immediately or must wait on a decision.
#[derive(Debug, Clone, PartialEq)]
pub enum CloseRequestOutcome {
    /// No dirty drafts: close now.
    Proceed,
    /// Dirty drafts: keep the surface open and raise the guard dialog.
    Guard(PendingClose),
}

/// Intercept a close/exit request BEFORE `EditorOpen` flips: any dirty draft
/// raises the guard instead of closing.
pub fn request_close(intent: CloseIntent, session: &ProfileSession) -> CloseRequestOutcome {
    request_close_with_settings(intent, session, false)
}

pub fn request_close_with_settings(
    intent: CloseIntent,
    session: &ProfileSession,
    settings_dirty: bool,
) -> CloseRequestOutcome {
    let mut dirty = dirty_profile_kinds(session);
    if settings_dirty {
        dirty.push(ProfileKind::Settings);
    }
    if dirty.is_empty() {
        CloseRequestOutcome::Proceed
    } else {
        CloseRequestOutcome::Guard(PendingClose { intent, dirty })
    }
}

/// Result of applying a close decision.
#[derive(Debug, Clone, PartialEq)]
pub enum CloseOutcome {
    /// Guard dismissed; the surface stays open, drafts untouched.
    Cancelled,
    /// Close may finalize (drafts discarded or all saves succeeded).
    Close(CloseIntent),
    /// Some saves failed: dialog stays open listing the failed kinds,
    /// successful drafts are already clean.
    StayOpen { failed: Vec<ProfileKind> },
}

/// Resolve the close guard. `save_results` reports each dirty kind's write
/// outcome and is only consulted for `SaveAll`; saves run sequentially and
/// independently, so one failure never rolls back another kind's success.
pub fn reduce_close_decision(
    pending: &PendingClose,
    decision: CloseDecision,
    session: &mut ProfileSession,
    save_results: &[(ProfileKind, bool)],
) -> CloseOutcome {
    match decision {
        CloseDecision::Cancel => CloseOutcome::Cancelled,
        CloseDecision::DiscardAll => {
            session.keyboard = ProfileDraft::clean(
                session.keyboard.selected.clone(),
                session.keyboard.saved.clone(),
            );
            session.midi =
                ProfileDraft::clean(session.midi.selected.clone(), session.midi.saved.clone());
            session.lanes =
                ProfileDraft::clean(session.lanes.selected.clone(), session.lanes.saved.clone());
            CloseOutcome::Close(pending.intent)
        }
        CloseDecision::SaveAll => {
            let failed = apply_save_all_results(session, save_results);
            if failed.is_empty() {
                CloseOutcome::Close(pending.intent)
            } else {
                CloseOutcome::StayOpen { failed }
            }
        }
    }
}

/// Dirty-guard dialog layout: button labels in left-to-right order plus
/// which button holds default focus and which is destructive.
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyDialogLayout {
    pub buttons: Vec<&'static str>,
    pub default_focus: usize,
    pub destructive: usize,
}

/// Build the guard dialog for the given dirty kinds. One dirty user profile:
/// `Cancel | Discard changes | Save changes` (built-in primary reads
/// `Save as new profile`); several dirty kinds: `Cancel | Discard all |
/// Save all`. Save holds default focus; the destructive button never does.
pub fn dirty_dialog_layout(dirty: &[ProfileKind], builtin_selected: bool) -> DirtyDialogLayout {
    let buttons = if dirty.len() > 1 {
        vec!["Cancel", "Discard all", "Save all"]
    } else if builtin_selected {
        vec!["Cancel", "Discard changes", "Save as new profile"]
    } else {
        vec!["Cancel", "Discard changes", "Save changes"]
    };
    DirtyDialogLayout {
        default_focus: buttons.len() - 1,
        destructive: 1,
        buttons,
    }
}

/// Keyboard shortcuts on the guard: Enter saves (default focus), Escape
/// cancels. Discard has no shortcut — it requires an explicit click or
/// focus movement.
pub fn close_decision_for_key(enter: bool, escape: bool) -> Option<CloseDecision> {
    if escape {
        Some(CloseDecision::Cancel)
    } else if enter {
        Some(CloseDecision::SaveAll)
    } else {
        None
    }
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
            ProfileKind::Settings => {}
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

    #[test]
    fn dirty_settings_join_the_existing_close_guard() {
        let outcome = request_close_with_settings(CloseIntent::Customize, &session(), true);
        assert_eq!(
            outcome,
            CloseRequestOutcome::Guard(PendingClose {
                intent: CloseIntent::Customize,
                dirty: vec![ProfileKind::Settings],
            })
        );
    }

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
    fn dirty_close_does_not_flip_editor_open() {
        let mut s = session();
        s.keyboard.value.add_key(EChannel::Snare, KeyCode::KeyQ);
        let outcome = request_close(CloseIntent::Customize, &s);
        assert_eq!(
            outcome,
            CloseRequestOutcome::Guard(PendingClose {
                intent: CloseIntent::Customize,
                dirty: vec![ProfileKind::Keyboard],
            }),
            "dirty drafts must guard the close instead of proceeding"
        );
        assert_eq!(
            request_close(CloseIntent::Customize, &session()),
            CloseRequestOutcome::Proceed,
            "clean session closes immediately"
        );
    }

    #[test]
    fn single_user_dialog_orders_cancel_discard_save() {
        let layout = dirty_dialog_layout(&[ProfileKind::Keyboard], false);
        assert_eq!(
            layout.buttons,
            vec!["Cancel", "Discard changes", "Save changes"]
        );
        assert_eq!(layout.default_focus, 2);
    }

    #[test]
    fn builtin_dialog_uses_save_as_primary() {
        let layout = dirty_dialog_layout(&[ProfileKind::Midi], true);
        assert_eq!(layout.buttons[2], "Save as new profile");
        assert_eq!(layout.default_focus, 2);
    }

    #[test]
    fn multiple_dirty_dialog_lists_kinds() {
        let dirty = vec![ProfileKind::Keyboard, ProfileKind::Lanes];
        let layout = dirty_dialog_layout(&dirty, false);
        assert_eq!(layout.buttons, vec!["Cancel", "Discard all", "Save all"]);
        let pending = PendingClose {
            intent: CloseIntent::Customize,
            dirty,
        };
        assert_eq!(
            pending.dirty,
            vec![ProfileKind::Keyboard, ProfileKind::Lanes]
        );
    }

    #[test]
    fn enter_saves_and_escape_cancels() {
        assert_eq!(
            close_decision_for_key(true, false),
            Some(CloseDecision::SaveAll)
        );
        assert_eq!(
            close_decision_for_key(false, true),
            Some(CloseDecision::Cancel)
        );
        assert_eq!(close_decision_for_key(false, false), None);
    }

    #[test]
    fn discard_never_has_default_focus() {
        for (dirty, builtin) in [
            (vec![ProfileKind::Keyboard], false),
            (vec![ProfileKind::Keyboard], true),
            (vec![ProfileKind::Keyboard, ProfileKind::Midi], false),
        ] {
            let layout = dirty_dialog_layout(&dirty, builtin);
            assert_ne!(
                layout.default_focus, layout.destructive,
                "destructive button must never hold default focus"
            );
        }
    }

    #[test]
    fn partial_save_all_closes_only_successful_drafts() {
        let mut s = session();
        s.keyboard.value.add_key(EChannel::Snare, KeyCode::KeyQ);
        s.midi.value.velocity_threshold = 42;
        let pending = PendingClose {
            intent: CloseIntent::Customize,
            dirty: vec![ProfileKind::Keyboard, ProfileKind::Midi],
        };
        let outcome = reduce_close_decision(
            &pending,
            CloseDecision::SaveAll,
            &mut s,
            &[(ProfileKind::Keyboard, true), (ProfileKind::Midi, false)],
        );
        assert_eq!(
            outcome,
            CloseOutcome::StayOpen {
                failed: vec![ProfileKind::Midi]
            }
        );
        assert!(!s.keyboard.is_dirty(), "successful save cleaned the draft");
        assert!(s.midi.is_dirty(), "failed save keeps the draft dirty");
    }

    #[test]
    fn graceful_exit_waits_for_dirty_decision() {
        let mut s = session();
        s.lanes.value = LaneProfile::from_arrangement(dtx_layout::nx_type_b());
        let outcome = request_close(CloseIntent::GracefulAppExit, &s);
        assert!(
            matches!(outcome, CloseRequestOutcome::Guard(_)),
            "graceful exit must wait for the dirty decision"
        );
        // Cancel keeps everything as it was.
        let pending = PendingClose {
            intent: CloseIntent::GracefulAppExit,
            dirty: dirty_profile_kinds(&s),
        };
        let before = s.clone();
        assert_eq!(
            reduce_close_decision(&pending, CloseDecision::Cancel, &mut s, &[]),
            CloseOutcome::Cancelled
        );
        assert_eq!(s, before);
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
