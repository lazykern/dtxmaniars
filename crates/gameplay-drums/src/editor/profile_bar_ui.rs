//! Shared profile bar: `[ Name ▾ ] [dot] [Save] [Save As] [...]`, rendered
//! above the tab content on Controls (Keyboard/MIDI segment) and Lanes.
//!
//! This file owns bar rendering plus the profile-bar action engine (Select,
//! Save, SaveAs, Rename, Delete, and the dirty-guard's combined
//! save-then-select transaction). `profile_dialog_ui` renders the modals and
//! calls back into the `pub(super)` engine functions here so every registry
//! write goes through one place per kind.

use bevy::prelude::*;
use dtx_input::profiles::{RegistryAction, RegistryStartup};

use super::chrome;
use super::profile_bar::{self, ProfileBarAction, ProfileUiError};
use super::profile_dialog;
use super::profile_state::{
    self, CustomizeSession, DraftEffect, LaneProfileDraft, ProfileDraft, ProfileKind,
};

/// Which surface (tab + Controls segment) owns the profile bar, if any.
/// `Widgets`/settings tabs have no profile bar.
pub fn bar_kind(
    tab: game_shell::CustomizeTab,
    segment: super::controls_panel::ControlsSegment,
) -> Option<ProfileKind> {
    use super::controls_panel::ControlsSegment;
    match tab {
        game_shell::CustomizeTab::Controls => Some(match segment {
            ControlsSegment::Keyboard => ProfileKind::Keyboard,
            ControlsSegment::Midi => ProfileKind::Midi,
        }),
        game_shell::CustomizeTab::Lanes => Some(ProfileKind::Lanes),
        _ => None,
    }
}

/// Selector dropdown / overflow menu — one at a time, closed by default.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ProfileBarPopup {
    #[default]
    None,
    Selector,
    Overflow,
}

/// Last profile transaction failure, surfaced under the bar until the next
/// successful action clears it.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct ProfileUiErrorState(pub Option<ProfileUiError>);

/// Which kind the currently-open Name/ConfirmDelete dialog belongs to.
/// `Dirty`/`CorruptReset` already carry their own kind; this covers the two
/// variants that don't, so a mid-dialog tab switch can't apply the decision
/// to the wrong registry.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DialogKind(pub Option<ProfileKind>);

/// Name button + ▾: toggles the selector dropdown.
#[derive(Component)]
pub struct ProfileSelectorBtn;

/// One row in the selector dropdown.
#[derive(Component, Clone)]
pub struct ProfileSelectorItem(pub String);

/// `…` overflow button: toggles the overflow menu.
#[derive(Component)]
pub struct ProfileSelectorOverflowBtn;

/// Save / Save As (bar-level) and Rename / Revert / Delete / Save As
/// (overflow menu) all share this marker + the shared `ProfileBarAction`.
#[derive(Component, Clone)]
pub struct ProfileBarBtn(pub ProfileBarAction);

pub fn plugin(app: &mut App) {
    app.init_resource::<ProfileBarPopup>()
        .init_resource::<ProfileUiErrorState>()
        .init_resource::<DialogKind>()
        .add_systems(
            Update,
            (
                handle_selector_toggle,
                handle_overflow_toggle,
                handle_selector_item_click,
                handle_bar_action_buttons,
            )
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        );
}

/// Rendering snapshot for the active kind: current selection, dirty, and
/// whether that selection is a built-in.
#[derive(Clone)]
pub(super) struct BarInfo {
    pub selected: String,
    pub dirty: bool,
    pub builtin: bool,
}

pub(super) fn bar_info(kind: ProfileKind, session: &CustomizeSession) -> BarInfo {
    match kind {
        ProfileKind::Keyboard => BarInfo {
            selected: session.0.keyboard.selected.clone(),
            dirty: session.0.keyboard.is_dirty(),
            builtin: dtx_input::profiles::keyboard_builtins()
                .contains_key(&session.0.keyboard.selected),
        },
        ProfileKind::Midi => BarInfo {
            selected: session.0.midi.selected.clone(),
            dirty: session.0.midi.is_dirty(),
            builtin: dtx_input::profiles::midi_builtins().contains_key(&session.0.midi.selected),
        },
        ProfileKind::Lanes => BarInfo {
            selected: session.0.lanes.selected.clone(),
            dirty: session.0.lanes.is_dirty(),
            builtin: dtx_layout::profiles::lane_builtins().contains_key(&session.0.lanes.selected),
        },
    }
}

/// Built-in names (given order) and on-disk user profile names for `kind`.
/// Reads the registry fresh from disk — the profile bar rebuilds rarely
/// (panel signature-gated), so this mirrors the load-fresh pattern the rest
/// of the profile write path already uses.
pub(super) fn kind_names(kind: ProfileKind) -> (Vec<String>, Vec<String>) {
    // ponytail: ReadOnlyBuiltins is swallowed to an empty user list here —
    // unreachable until startup corruption detection wires open_corrupt_reset
    // (deferred); a corrupt registry then routes to CorruptReset instead.
    match kind {
        ProfileKind::Keyboard => {
            let builtins = dtx_input::profiles::keyboard_builtins()
                .into_keys()
                .collect();
            let users = match super::load_keyboard_startup() {
                RegistryStartup::Ready(r) | RegistryStartup::LegacySession { registry: r, .. } => {
                    r.profiles.into_keys().collect()
                }
                RegistryStartup::ReadOnlyBuiltins(_) => Vec::new(),
            };
            (builtins, users)
        }
        ProfileKind::Midi => {
            let builtins = dtx_input::profiles::midi_builtins().into_keys().collect();
            let users = match super::load_midi_startup() {
                RegistryStartup::Ready(r) | RegistryStartup::LegacySession { registry: r, .. } => {
                    r.profiles.into_keys().collect()
                }
                RegistryStartup::ReadOnlyBuiltins(_) => Vec::new(),
            };
            (builtins, users)
        }
        ProfileKind::Lanes => {
            let builtins = dtx_layout::profiles::lane_builtins().into_keys().collect();
            let users = match super::load_lane_startup() {
                dtx_layout::profiles::LaneRegistryStartup::Ready(r)
                | dtx_layout::profiles::LaneRegistryStartup::LegacySession {
                    registry: r, ..
                } => r.profiles.into_keys().collect(),
                dtx_layout::profiles::LaneRegistryStartup::ReadOnlyBuiltins(_) => Vec::new(),
            };
            (builtins, users)
        }
    }
}

pub(super) fn registry_path(kind: ProfileKind) -> std::path::PathBuf {
    match kind {
        ProfileKind::Keyboard => crate::bindings::keyboard_registry_path(),
        ProfileKind::Midi => crate::bindings::midi_registry_path(),
        ProfileKind::Lanes => crate::lanes::lane_registry_path(),
    }
}

/// Suggested Save As text (unvalidated — the dialog's own submit validates).
pub(super) fn save_as_suggestion(kind: ProfileKind, session: &CustomizeSession) -> String {
    let info = bar_info(kind, session);
    let (builtins, users) = kind_names(kind);
    let existing: Vec<&str> = builtins
        .iter()
        .chain(users.iter())
        .map(String::as_str)
        .collect();
    dtx_persistence::suggest_copy_name(&info.selected, existing.iter().copied())
}

/// A validated Save As name auto-generated for the dirty guard's builtin
/// path (mirrors the close guard's `save_dirty_kind`, which does the same
/// without prompting — Save As on a built-in never blocks on user input).
pub(super) fn auto_saveas_name(
    kind: ProfileKind,
    session: &CustomizeSession,
) -> Result<dtx_persistence::ProfileName, String> {
    let suggestion = save_as_suggestion(kind, session);
    let (builtins, users) = kind_names(kind);
    dtx_persistence::validate_profile_name(
        &suggestion,
        builtins.iter().map(String::as_str),
        users.iter().map(String::as_str),
        None,
    )
    .map_err(|error| error.to_string())
}

/// Recompose `LiveBindings` from the current session drafts and bump the
/// bindings revision so the Controls panel/resolver pick up the change.
/// No-op for `ProfileKind::Lanes` (lane runtime refresh goes through
/// `LaneProfileDraft` instead).
pub(super) fn refresh_live_bindings(
    kind: ProfileKind,
    session: &CustomizeSession,
    live: &mut crate::bindings::LiveBindings,
    rev: &mut super::bindings_panel::BindingsRev,
) {
    if !matches!(kind, ProfileKind::Keyboard | ProfileKind::Midi) {
        return;
    }
    live.0 = crate::bindings::compose_bindings(&session.0.keyboard.value, &session.0.midi.value);
    rev.0 = rev.0.wrapping_add(1);
}

/// Select transaction: persist `registry.active = target` and refresh the
/// session draft to the target's clean value. Only reachable on a clean
/// draft — a dirty draft routes through the `Dirty` dialog instead.
pub(super) fn select_kind(
    kind: ProfileKind,
    target: String,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
) -> Result<(), String> {
    match kind {
        ProfileKind::Keyboard => {
            let builtins = dtx_input::profiles::keyboard_builtins();
            let registry = super::commit_registry_actions(
                super::load_keyboard_startup(),
                &builtins,
                vec![RegistryAction::Select(target)],
                |next| {
                    dtx_input::profiles::save_keyboard_registry(
                        &crate::bindings::keyboard_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_keyboard_profile(&registry);
            session.0.keyboard = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Midi => {
            let builtins = dtx_input::profiles::midi_builtins();
            let registry = super::commit_registry_actions(
                super::load_midi_startup(),
                &builtins,
                vec![RegistryAction::Select(target)],
                |next| {
                    dtx_input::profiles::save_midi_registry(
                        &crate::bindings::midi_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_midi_profile(&registry);
            session.0.midi = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Lanes => {
            let registry = super::commit_lane_actions(
                super::load_lane_startup(),
                vec![super::LaneRegAction::Select(target)],
                super::save_lane_to_disk,
            )?;
            let draft = ProfileDraft::clean(
                registry.active.clone(),
                dtx_layout::profiles::LaneProfile::from_arrangement(
                    dtx_layout::profiles::active_lane_arrangement(&registry),
                ),
            );
            lane_draft.0 = draft.clone();
            session.0.lanes = draft;
        }
    }
    Ok(())
}

/// SaveAs transaction: `name` is already validated (dialog submit or
/// `auto_saveas_name`).
pub(super) fn saveas_kind(
    kind: ProfileKind,
    name: dtx_persistence::ProfileName,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
) -> Result<(), String> {
    match kind {
        ProfileKind::Keyboard => {
            let builtins = dtx_input::profiles::keyboard_builtins();
            let value = session.0.keyboard.value.clone();
            let registry = super::commit_registry_actions(
                super::load_keyboard_startup(),
                &builtins,
                vec![RegistryAction::SaveAs { name, value }],
                |next| {
                    dtx_input::profiles::save_keyboard_registry(
                        &crate::bindings::keyboard_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_keyboard_profile(&registry);
            session.0.keyboard = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Midi => {
            let builtins = dtx_input::profiles::midi_builtins();
            let value = session.0.midi.value.clone();
            let registry = super::commit_registry_actions(
                super::load_midi_startup(),
                &builtins,
                vec![RegistryAction::SaveAs { name, value }],
                |next| {
                    dtx_input::profiles::save_midi_registry(
                        &crate::bindings::midi_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_midi_profile(&registry);
            session.0.midi = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Lanes => {
            let value = session.0.lanes.value.clone();
            let registry = super::commit_lane_actions(
                super::load_lane_startup(),
                vec![super::LaneRegAction::SaveAs {
                    name: name.as_str().to_owned(),
                    value,
                }],
                super::save_lane_to_disk,
            )?;
            let draft = ProfileDraft::clean(
                registry.active.clone(),
                dtx_layout::profiles::LaneProfile::from_arrangement(
                    dtx_layout::profiles::active_lane_arrangement(&registry),
                ),
            );
            lane_draft.0 = draft.clone();
            session.0.lanes = draft;
        }
    }
    Ok(())
}

/// Rename transaction: only the registry key and `.selected` move; the
/// draft's dirty state (`.saved`/`.value`) is untouched.
pub(super) fn rename_kind(
    kind: ProfileKind,
    name: dtx_persistence::ProfileName,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
) -> Result<(), String> {
    match kind {
        ProfileKind::Keyboard => {
            let builtins = dtx_input::profiles::keyboard_builtins();
            let registry = super::commit_registry_actions(
                super::load_keyboard_startup(),
                &builtins,
                vec![RegistryAction::Rename(name)],
                |next| {
                    dtx_input::profiles::save_keyboard_registry(
                        &crate::bindings::keyboard_registry_path(),
                        next,
                    )
                },
            )?;
            session.0.keyboard.selected = registry.active;
        }
        ProfileKind::Midi => {
            let builtins = dtx_input::profiles::midi_builtins();
            let registry = super::commit_registry_actions(
                super::load_midi_startup(),
                &builtins,
                vec![RegistryAction::Rename(name)],
                |next| {
                    dtx_input::profiles::save_midi_registry(
                        &crate::bindings::midi_registry_path(),
                        next,
                    )
                },
            )?;
            session.0.midi.selected = registry.active;
        }
        ProfileKind::Lanes => {
            let registry = super::commit_lane_actions(
                super::load_lane_startup(),
                vec![super::LaneRegAction::Rename(name.as_str().to_owned())],
                super::save_lane_to_disk,
            )?;
            lane_draft.0.selected = registry.active.clone();
            session.0.lanes.selected = registry.active;
        }
    }
    Ok(())
}

/// Delete transaction: the registry falls back to a built-in; the draft
/// becomes clean at that built-in's value.
pub(super) fn delete_kind(
    kind: ProfileKind,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
) -> Result<(), String> {
    match kind {
        ProfileKind::Keyboard => {
            let builtins = dtx_input::profiles::keyboard_builtins();
            let registry = super::commit_registry_actions(
                super::load_keyboard_startup(),
                &builtins,
                vec![RegistryAction::Delete],
                |next| {
                    dtx_input::profiles::save_keyboard_registry(
                        &crate::bindings::keyboard_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_keyboard_profile(&registry);
            session.0.keyboard = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Midi => {
            let builtins = dtx_input::profiles::midi_builtins();
            let registry = super::commit_registry_actions(
                super::load_midi_startup(),
                &builtins,
                vec![RegistryAction::Delete],
                |next| {
                    dtx_input::profiles::save_midi_registry(
                        &crate::bindings::midi_registry_path(),
                        next,
                    )
                },
            )?;
            let value = crate::bindings::active_midi_profile(&registry);
            session.0.midi = ProfileDraft::clean(registry.active, value);
        }
        ProfileKind::Lanes => {
            let registry = super::commit_lane_actions(
                super::load_lane_startup(),
                vec![super::LaneRegAction::Delete],
                super::save_lane_to_disk,
            )?;
            let draft = ProfileDraft::clean(
                registry.active.clone(),
                dtx_layout::profiles::LaneProfile::from_arrangement(
                    dtx_layout::profiles::active_lane_arrangement(&registry),
                ),
            );
            lane_draft.0 = draft.clone();
            session.0.lanes = draft;
        }
    }
    Ok(())
}

/// Apply one `DraftEffect<T>` against a keyboard/MIDI-shaped registry: an
/// optional save (under the draft's own name, or a validated new name),
/// then an optional select, written once.
fn apply_kv_effect<T: Clone + PartialEq>(
    effect: DraftEffect<T>,
    draft: &ProfileDraft<T>,
    builtins: &std::collections::BTreeMap<String, T>,
    startup: RegistryStartup<dtx_input::profiles::ProfileRegistry<T>>,
    active_value: impl Fn(&dtx_input::profiles::ProfileRegistry<T>) -> T,
    save_fn: impl FnOnce(
        &dtx_input::profiles::ProfileRegistry<T>,
    ) -> Result<(), dtx_input::profiles::RegistryIoError>,
) -> Result<Option<ProfileDraft<T>>, String> {
    let (save, select) = match effect {
        DraftEffect::Noop => return Ok(None),
        DraftEffect::ResetDraft => {
            return Ok(Some(ProfileDraft::clean(
                draft.selected.clone(),
                draft.saved.clone(),
            )));
        }
        DraftEffect::Transaction { save, select } => (save, select),
    };
    let canonical = match &startup {
        RegistryStartup::Ready(r) | RegistryStartup::LegacySession { registry: r, .. } => {
            Some(r.clone())
        }
        RegistryStartup::ReadOnlyBuiltins(_) => None,
    };
    let mut actions = Vec::new();
    if let Some((name, value)) = save {
        let action = if name == draft.selected {
            RegistryAction::Save(value)
        } else {
            let existing: Vec<String> = canonical
                .as_ref()
                .map(|r| r.profiles.keys().cloned().collect())
                .unwrap_or_default();
            let name = dtx_persistence::validate_profile_name(
                &name,
                builtins.keys().map(String::as_str),
                existing.iter().map(String::as_str),
                None,
            )
            .map_err(|error| error.to_string())?;
            RegistryAction::SaveAs { name, value }
        };
        actions.push(action);
    }
    if let Some(target) = select {
        actions.push(RegistryAction::Select(target));
    }
    let registry = super::commit_registry_actions(startup, builtins, actions, save_fn)?;
    let value = active_value(&registry);
    Ok(Some(ProfileDraft::clean(registry.active.clone(), value)))
}

/// Apply one `DraftEffect<LaneProfile>` — the lane-registry mirror of
/// `apply_kv_effect` (lane profiles have no generic `ProfileRegistry<T>`).
fn apply_lane_effect(
    effect: DraftEffect<dtx_layout::profiles::LaneProfile>,
    draft: &ProfileDraft<dtx_layout::profiles::LaneProfile>,
) -> Result<Option<ProfileDraft<dtx_layout::profiles::LaneProfile>>, String> {
    let (save, select) = match effect {
        DraftEffect::Noop => return Ok(None),
        DraftEffect::ResetDraft => {
            return Ok(Some(ProfileDraft::clean(
                draft.selected.clone(),
                draft.saved.clone(),
            )));
        }
        DraftEffect::Transaction { save, select } => (save, select),
    };
    let mut actions = Vec::new();
    if let Some((name, value)) = save {
        let action = if name == draft.selected {
            super::LaneRegAction::Save(value)
        } else {
            super::LaneRegAction::SaveAs { name, value }
        };
        actions.push(action);
    }
    if let Some(target) = select {
        actions.push(super::LaneRegAction::Select(target));
    }
    let registry = super::commit_lane_actions(
        super::load_lane_startup(),
        actions,
        super::save_lane_to_disk,
    )?;
    let value = dtx_layout::profiles::LaneProfile::from_arrangement(
        dtx_layout::profiles::active_lane_arrangement(&registry),
    );
    Ok(Some(ProfileDraft::clean(registry.active.clone(), value)))
}

/// Resolve a `Dirty` dialog decision for `kind`: builds the `DirtyDecision`
/// (auto-naming Save on a built-in, mirroring the close guard), reduces it
/// against the draft, and applies the resulting effect. Returns whether
/// keyboard/MIDI `LiveBindings` need recomposing.
pub(super) fn resolve_dirty(
    kind: ProfileKind,
    pending: &profile_state::PendingProfileAction,
    builtin_selected: bool,
    decision: profile_state::CloseDecision,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
) -> Result<bool, String> {
    use profile_state::{reduce_dirty_action, CloseDecision, DirtyDecision};

    let dirty_decision = match decision {
        CloseDecision::Cancel => DirtyDecision::Cancel,
        CloseDecision::DiscardAll => DirtyDecision::Discard,
        CloseDecision::SaveAll if builtin_selected => {
            DirtyDecision::SaveAs(auto_saveas_name(kind, session)?)
        }
        CloseDecision::SaveAll => DirtyDecision::Save,
    };

    match kind {
        ProfileKind::Keyboard => {
            let effect = reduce_dirty_action(
                &session.0.keyboard,
                builtin_selected,
                pending,
                dirty_decision,
            )
            .map_err(|error| error.to_string())?;
            let builtins = dtx_input::profiles::keyboard_builtins();
            let next = apply_kv_effect(
                effect,
                &session.0.keyboard,
                &builtins,
                super::load_keyboard_startup(),
                crate::bindings::active_keyboard_profile,
                |next| {
                    dtx_input::profiles::save_keyboard_registry(
                        &crate::bindings::keyboard_registry_path(),
                        next,
                    )
                },
            )?;
            let changed = next.is_some();
            if let Some(draft) = next {
                session.0.keyboard = draft;
            }
            Ok(changed)
        }
        ProfileKind::Midi => {
            let effect =
                reduce_dirty_action(&session.0.midi, builtin_selected, pending, dirty_decision)
                    .map_err(|error| error.to_string())?;
            let builtins = dtx_input::profiles::midi_builtins();
            let next = apply_kv_effect(
                effect,
                &session.0.midi,
                &builtins,
                super::load_midi_startup(),
                crate::bindings::active_midi_profile,
                |next| {
                    dtx_input::profiles::save_midi_registry(
                        &crate::bindings::midi_registry_path(),
                        next,
                    )
                },
            )?;
            let changed = next.is_some();
            if let Some(draft) = next {
                session.0.midi = draft;
            }
            Ok(changed)
        }
        ProfileKind::Lanes => {
            let effect =
                reduce_dirty_action(&session.0.lanes, builtin_selected, pending, dirty_decision)
                    .map_err(|error| error.to_string())?;
            if let Some(draft) = apply_lane_effect(effect, &session.0.lanes)? {
                lane_draft.0 = draft.clone();
                session.0.lanes = draft;
            }
            Ok(false)
        }
    }
}

// ===== Rendering =====

/// Spawns the bar as the first child of the left content root, above the tab
/// content. Called from `panel::rebuild_left_content`.
pub fn spawn_bar(
    parent: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    kind: ProfileKind,
    session: &CustomizeSession,
    popup: ProfileBarPopup,
    error: Option<&ProfileUiError>,
) {
    let info = bar_info(kind, session);
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Relative,
            margin: UiRect::bottom(Val::Px(8.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ProfileSelectorBtn,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CARD_BG),
                    BorderColor::all(chrome::CARD_BORDER),
                    children![(
                        Text::new(format!("{} \u{25BE}", info.selected)),
                        dtx_ui::theme::Theme::font(12.0),
                        TextColor(t.text_primary),
                    )],
                ));
                if info.dirty {
                    row.spawn((
                        Node {
                            width: Val::Px(7.0),
                            height: Val::Px(7.0),
                            border_radius: BorderRadius::all(Val::Px(3.5)),
                            ..default()
                        },
                        BackgroundColor(chrome::DIRTY),
                    ));
                }
                let save_enabled = profile_bar::save_enabled(info.builtin, info.dirty);
                spawn_bar_btn(row, t, ProfileBarAction::Save, "Save", save_enabled);
                spawn_bar_btn(row, t, ProfileBarAction::SaveAs, "Save As", true);
                row.spawn((
                    ProfileSelectorOverflowBtn,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CARD_BG),
                    children![(
                        Text::new("\u{2026}"),
                        dtx_ui::theme::Theme::font(12.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
            if let Some(error) = error {
                col.spawn((
                    Text::new(format!("{}: {}", error.path.display(), error.message)),
                    dtx_ui::theme::Theme::font(10.0),
                    TextColor(chrome::ERR),
                    Node {
                        margin: UiRect::top(Val::Px(2.0)),
                        max_width: Val::Px(super::chrome::LEFT_PANEL_WIDTH - 16.0),
                        ..default()
                    },
                ));
            }
            match popup {
                ProfileBarPopup::Selector => spawn_selector_popup(col, t, kind, &info),
                ProfileBarPopup::Overflow => spawn_overflow_popup(col, t, info.builtin),
                ProfileBarPopup::None => {}
            }
        });
}

fn spawn_bar_btn(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    action: ProfileBarAction,
    label: &str,
    enabled: bool,
) {
    p.spawn((
        ProfileBarBtn(action),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(if enabled {
            chrome::CARD_BG
        } else {
            chrome::PANEL_BG
        }),
        children![(
            Text::new(label.to_owned()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(if enabled {
                t.text_primary
            } else {
                chrome::TEXT_MUTED
            }),
        )],
    ));
}

fn spawn_popup_container<'a>(p: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(30.0),
            left: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            min_width: Val::Px(200.0),
            row_gap: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(chrome::CARD_BG),
        BorderColor::all(chrome::CARD_BORDER),
        GlobalZIndex(crate::ui_z::EDITOR_MODAL),
    ))
}

fn spawn_selector_popup(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    kind: ProfileKind,
    info: &BarInfo,
) {
    let (builtins, users) = kind_names(kind);
    let items = profile_bar::profile_bar_items(
        builtins.iter().map(String::as_str),
        users.iter().map(String::as_str),
        &info.selected,
    );
    spawn_popup_container(p).with_children(|popup| {
        for item in items {
            let label = if item.builtin {
                format!("{} (built-in)", item.name)
            } else {
                item.name.clone()
            };
            popup
                .spawn((
                    ProfileSelectorItem(item.name.clone()),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(if item.selected {
                        chrome::ROW_SELECTED_BG
                    } else {
                        Color::NONE
                    }),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(label),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(if item.builtin {
                            chrome::TEXT_MUTED
                        } else {
                            t.text_primary
                        }),
                    ));
                });
        }
    });
}

fn spawn_overflow_popup(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    builtin_selected: bool,
) {
    let actions = profile_bar::overflow_actions(builtin_selected);
    spawn_popup_container(p).with_children(|popup| {
        for action in actions {
            let label = match action {
                ProfileBarAction::SaveAs => "Save As",
                ProfileBarAction::Rename => "Rename",
                ProfileBarAction::Revert => "Revert",
                ProfileBarAction::Delete => "Delete",
                ProfileBarAction::Select(_) | ProfileBarAction::Save => "",
            };
            popup
                .spawn((
                    ProfileBarBtn(action),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(label.to_owned()),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(t.text_primary),
                    ));
                });
        }
    });
}

// ===== Interaction =====

fn handle_selector_toggle(
    q: Query<&Interaction, (With<ProfileSelectorBtn>, Changed<Interaction>)>,
    mut popup: ResMut<ProfileBarPopup>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            *popup = if *popup == ProfileBarPopup::Selector {
                ProfileBarPopup::None
            } else {
                ProfileBarPopup::Selector
            };
        }
    }
}

fn handle_overflow_toggle(
    q: Query<&Interaction, (With<ProfileSelectorOverflowBtn>, Changed<Interaction>)>,
    mut popup: ResMut<ProfileBarPopup>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            *popup = if *popup == ProfileBarPopup::Overflow {
                ProfileBarPopup::None
            } else {
                ProfileBarPopup::Overflow
            };
        }
    }
}

pub(super) fn ui_error(kind: ProfileKind, message: String) -> ProfileUiError {
    ProfileUiError {
        kind,
        path: registry_path(kind),
        message,
    }
}

/// Selector dropdown row clicks: same profile is a no-op, a dirty draft
/// raises the `Dirty` guard, a clean draft selects immediately.
#[allow(clippy::too_many_arguments)]
fn handle_selector_item_click(
    items: Query<(&Interaction, &ProfileSelectorItem), Changed<Interaction>>,
    active: Res<super::tabs::ActiveTab>,
    segment: Res<super::controls_panel::ControlsSegment>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<crate::bindings::LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut dialog: ResMut<profile_dialog::ProfileDialogState>,
    mut error: ResMut<ProfileUiErrorState>,
    mut popup: ResMut<ProfileBarPopup>,
) {
    let Some(kind) = bar_kind(active.0, *segment) else {
        return;
    };
    let Some(target) = items
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, item)| item.0.clone())
    else {
        return;
    };
    *popup = ProfileBarPopup::None;
    let info = bar_info(kind, &session);
    if target == info.selected {
        return;
    }
    if info.dirty {
        *dialog = profile_dialog::ProfileDialogState::Dirty {
            kind,
            pending: profile_state::PendingProfileAction::Select(target),
            builtin_selected: info.builtin,
        };
        return;
    }
    match select_kind(kind, target, &mut session, &mut lane_draft) {
        Ok(()) => {
            error.0 = None;
            refresh_live_bindings(kind, &session, &mut live, &mut rev);
        }
        Err(message) => error.0 = Some(ui_error(kind, message)),
    }
}

/// Save/SaveAs/Rename/Revert/Delete clicks — both the bar-level Save/Save As
/// buttons and every overflow-menu row carry the same `ProfileBarBtn`.
#[allow(clippy::too_many_arguments)]
fn handle_bar_action_buttons(
    buttons: Query<(&Interaction, &ProfileBarBtn), Changed<Interaction>>,
    active: Res<super::tabs::ActiveTab>,
    segment: Res<super::controls_panel::ControlsSegment>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut dialog: ResMut<profile_dialog::ProfileDialogState>,
    mut dialog_kind: ResMut<DialogKind>,
    mut error: ResMut<ProfileUiErrorState>,
    mut popup: ResMut<ProfileBarPopup>,
) {
    let Some(kind) = bar_kind(active.0, *segment) else {
        return;
    };
    let Some(action) = buttons
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, button)| button.0.clone())
    else {
        return;
    };
    *popup = ProfileBarPopup::None;
    let info = bar_info(kind, &session);
    match action {
        ProfileBarAction::Select(_) => {}
        ProfileBarAction::Save => {
            if !profile_bar::save_enabled(info.builtin, info.dirty) {
                return;
            }
            match super::save_dirty_kind(kind, &session.0) {
                Ok(()) => {
                    error.0 = None;
                    match kind {
                        ProfileKind::Keyboard => {
                            session.0.keyboard = session.0.keyboard.saved_now()
                        }
                        ProfileKind::Midi => session.0.midi = session.0.midi.saved_now(),
                        ProfileKind::Lanes => {
                            lane_draft.0 = lane_draft.0.saved_now();
                            session.0.lanes = lane_draft.0.clone();
                        }
                    }
                }
                Err(message) => error.0 = Some(ui_error(kind, message)),
            }
        }
        ProfileBarAction::SaveAs => {
            dialog_kind.0 = Some(kind);
            *dialog = profile_dialog::open_name_dialog(
                profile_dialog::NameAction::SaveAs,
                save_as_suggestion(kind, &session),
            );
        }
        ProfileBarAction::Rename => {
            dialog_kind.0 = Some(kind);
            *dialog = profile_dialog::open_name_dialog(
                profile_dialog::NameAction::Rename,
                info.selected.clone(),
            );
        }
        ProfileBarAction::Revert => {
            if info.dirty {
                *dialog = profile_dialog::ProfileDialogState::Dirty {
                    kind,
                    pending: profile_state::PendingProfileAction::Revert,
                    builtin_selected: info.builtin,
                };
            }
        }
        ProfileBarAction::Delete => {
            dialog_kind.0 = Some(kind);
            *dialog = profile_dialog::ProfileDialogState::ConfirmDelete {
                name: info.selected.clone(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::controls_panel::ControlsSegment;
    use crate::editor::profile_state::ProfileKind;
    use game_shell::CustomizeTab;

    #[test]
    fn bar_kind_follows_tab_and_segment() {
        assert_eq!(
            bar_kind(CustomizeTab::Controls, ControlsSegment::Keyboard),
            Some(ProfileKind::Keyboard)
        );
        assert_eq!(
            bar_kind(CustomizeTab::Controls, ControlsSegment::Midi),
            Some(ProfileKind::Midi)
        );
        assert_eq!(
            bar_kind(CustomizeTab::Lanes, ControlsSegment::Keyboard),
            Some(ProfileKind::Lanes)
        );
        assert_eq!(
            bar_kind(CustomizeTab::Widgets, ControlsSegment::Keyboard),
            None
        );
    }
}
