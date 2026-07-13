//! Profile dialog rendering + text entry: Save As / Rename name entry,
//! delete confirmation, the per-kind dirty guard, and corrupt-registry
//! recovery. Mirrors `close_dialog.rs`'s modal skeleton (centered card over
//! a full-screen scrim, `EDITOR_MODAL` z-index). The actual registry writes
//! live in `profile_bar_ui` — this file only renders `ProfileDialogState`
//! and calls back into that engine.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::chrome;
use super::profile_bar_ui::{self, DialogKind, ProfileUiErrorState};
use super::profile_dialog::{self, NameAction, ProfileDialogState};
use super::profile_state::{self, CloseDecision, CustomizeSession, LaneProfileDraft, ProfileKind};
use crate::bindings::LiveBindings;

#[derive(Component)]
struct ProfileDialogRoot;

#[derive(Component, Clone, Copy)]
enum DialogButton {
    NameOk,
    NameCancel,
    ConfirmDelete,
    CancelDelete,
    Dirty(CloseDecision),
    CorruptConfirm,
    CorruptCancel,
}

/// Keyboard-focused button index for the current button-row dialog
/// (ConfirmDelete / Dirty / CorruptReset). The Name dialog keeps its own
/// text-entry key handling and ignores this.
#[derive(Resource, Default)]
pub struct ProfileDialogFocus(pub usize);

#[derive(Component, Clone, Copy)]
struct DialogBtnIndex(usize);

/// Button row (left→right, matching spawn order) for the current dialog.
/// Index 0 is ALWAYS the safe dismiss. Empty for Closed/Name.
fn dialog_buttons(state: &ProfileDialogState) -> Vec<DialogButton> {
    match state {
        ProfileDialogState::ConfirmDelete { .. } => {
            vec![DialogButton::CancelDelete, DialogButton::ConfirmDelete]
        }
        ProfileDialogState::Dirty { .. } => vec![
            DialogButton::Dirty(CloseDecision::Cancel),
            DialogButton::Dirty(CloseDecision::DiscardAll),
            DialogButton::Dirty(CloseDecision::SaveAll),
        ],
        ProfileDialogState::CorruptReset { .. } => {
            vec![DialogButton::CorruptCancel, DialogButton::CorruptConfirm]
        }
        ProfileDialogState::Closed | ProfileDialogState::Name { .. } => Vec::new(),
    }
}

/// Initial keyboard focus per dialog: the dirty guard uses its layout's
/// default (Save — the layout guarantees it is never the destructive
/// button); everything else starts on the safe dismiss.
fn initial_focus(state: &ProfileDialogState) -> usize {
    match state {
        ProfileDialogState::Dirty {
            kind,
            builtin_selected,
            ..
        } => profile_state::dirty_dialog_layout(&[*kind], *builtin_selected).default_focus,
        _ => 0,
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ProfileDialogFocus>()
        .add_systems(
            Update,
            (
                sync_dialog.run_if(resource_changed::<ProfileDialogState>),
                handle_name_dialog_input,
                // Chained: both dispatch through `dispatch_dialog_button` and
                // take `ProfileDialogState`, so a same-frame keypress and click
                // must resolve in a defined order. Ordered after
                // `close_on_escape` so one Esc cannot both dismiss the dialog
                // and close Customize.
                (handle_dialog_keys, handle_dialog_buttons)
                    .chain()
                    .after(super::ui::close_on_escape),
                update_profile_dialog_focus_ring,
            )
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(OnExit(game_shell::AppState::Performance), despawn_dialog);
}

fn despawn_dialog(mut commands: Commands, roots: Query<Entity, With<ProfileDialogRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn sync_dialog(
    mut commands: Commands,
    dialog: Res<ProfileDialogState>,
    theme: Res<dtx_ui::ThemeResource>,
    mut focus: ResMut<ProfileDialogFocus>,
    roots: Query<Entity, With<ProfileDialogRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    focus.0 = initial_focus(&dialog);
    let t = theme.0;
    match &*dialog {
        ProfileDialogState::Closed => {}
        ProfileDialogState::Name {
            action,
            value,
            error,
        } => spawn_name_dialog(&mut commands, &t, *action, value, error.as_ref()),
        ProfileDialogState::ConfirmDelete { name } => spawn_confirm_delete(&mut commands, &t, name),
        ProfileDialogState::Dirty {
            kind,
            builtin_selected,
            ..
        } => spawn_dirty_dialog(&mut commands, &t, *kind, *builtin_selected),
        // ponytail: unreachable until startup corruption detection wires
        // open_corrupt_reset (deferred). Rendering + reset path are ready.
        ProfileDialogState::CorruptReset { message, .. } => {
            spawn_corrupt_dialog(&mut commands, &t, message)
        }
    }
}

/// Centered card over a full-screen scrim — same skeleton as
/// `close_dialog.rs`, generalized to any dialog's content.
fn spawn_modal(
    commands: &mut Commands,
    width: f32,
    t: &dtx_ui::theme::Theme,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            ProfileDialogRoot,
            dtx_ui::ModalDialog::new(vec![
                dtx_ui::DialogAction::Cancel,
                dtx_ui::DialogAction::Destructive,
                dtx_ui::DialogAction::Confirm,
            ]),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(crate::ui_z::EDITOR_MODAL),
        ))
        .with_children(|scrim| {
            scrim
                .spawn((
                    Node {
                        width: Val::Px(width),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(t.stage_panel_bg),
                    BorderColor::all(t.stage_panel_border),
                ))
                .with_children(content);
        });
}

fn spawn_dialog_btn(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    button: DialogButton,
    index: usize,
    label: &str,
    color: Color,
) {
    p.spawn((
        button,
        DialogBtnIndex(index),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(color),
        Outline::new(Val::Px(0.0), Val::Px(2.0), Color::NONE),
        children![(
            Text::new(label.to_owned()),
            dtx_ui::theme::Theme::font(14.0),
            TextColor(t.text_primary)
        )],
    ));
}

fn spawn_name_dialog(
    commands: &mut Commands,
    t: &dtx_ui::theme::Theme,
    action: NameAction,
    value: &str,
    error: Option<&dtx_persistence::ProfileNameError>,
) {
    let title = match action {
        NameAction::SaveAs => "Save profile as",
        NameAction::Rename => "Rename profile",
    };
    let value = value.to_owned();
    let error_text = error.map(|e| e.to_string());
    let t = *t;
    spawn_modal(commands, 420.0, &t, move |card| {
        card.spawn((
            Text::new(title),
            dtx_ui::theme::Theme::font(20.0),
            TextColor(t.text_primary),
        ));
        card.spawn((
            Node {
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(chrome::CARD_BG),
            BorderColor::all(chrome::CARD_BORDER),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(format!("{value}\u{2588}")),
                dtx_ui::theme::Theme::font(14.0),
                TextColor(t.text_primary),
            ));
        });
        if let Some(msg) = &error_text {
            card.spawn((
                Text::new(msg.clone()),
                dtx_ui::theme::Theme::font(11.0),
                TextColor(chrome::ERR),
            ));
        }
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|buttons| {
            spawn_dialog_btn(
                buttons,
                &t,
                DialogButton::NameCancel,
                0,
                "Cancel",
                Color::srgb(0.18, 0.18, 0.22),
            );
            spawn_dialog_btn(buttons, &t, DialogButton::NameOk, 1, "OK", t.accent);
        });
    });
}

fn spawn_confirm_delete(commands: &mut Commands, t: &dtx_ui::theme::Theme, name: &str) {
    let name = name.to_owned();
    let t = *t;
    spawn_modal(commands, 380.0, &t, move |card| {
        card.spawn((
            Text::new("Delete profile?"),
            dtx_ui::theme::Theme::font(20.0),
            TextColor(t.text_primary),
        ));
        card.spawn((
            Text::new(format!(
                "\u{201c}{name}\u{201d} will be permanently deleted."
            )),
            dtx_ui::theme::Theme::font(14.0),
            TextColor(t.text_secondary),
        ));
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|buttons| {
            spawn_dialog_btn(
                buttons,
                &t,
                DialogButton::CancelDelete,
                0,
                "Cancel",
                Color::srgb(0.18, 0.18, 0.22),
            );
            spawn_dialog_btn(
                buttons,
                &t,
                DialogButton::ConfirmDelete,
                1,
                "Delete",
                chrome::ERR,
            );
        });
    });
}

fn kind_label(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Keyboard => "keyboard",
        ProfileKind::Midi => "MIDI",
        ProfileKind::Lanes => "lane layout",
        ProfileKind::Settings => "settings",
    }
}

fn spawn_dirty_dialog(
    commands: &mut Commands,
    t: &dtx_ui::theme::Theme,
    kind: ProfileKind,
    builtin_selected: bool,
) {
    let layout = profile_state::dirty_dialog_layout(&[kind], builtin_selected);
    let decisions = [
        CloseDecision::Cancel,
        CloseDecision::DiscardAll,
        CloseDecision::SaveAll,
    ];
    let label = kind_label(kind);
    let t = *t;
    spawn_modal(commands, 420.0, &t, move |card| {
        card.spawn((
            Text::new("Save changes before switching?"),
            dtx_ui::theme::Theme::font(20.0),
            TextColor(t.text_primary),
        ));
        card.spawn((
            Text::new(format!(
                "Unsaved {label} changes will be lost if discarded."
            )),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_secondary),
        ));
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|buttons| {
            for (index, (label, decision)) in layout.buttons.iter().zip(decisions).enumerate() {
                let color = if index == layout.destructive {
                    chrome::ERR
                } else if index == layout.default_focus {
                    t.accent
                } else {
                    Color::srgb(0.18, 0.18, 0.22)
                };
                spawn_dialog_btn(
                    buttons,
                    &t,
                    DialogButton::Dirty(decision),
                    index,
                    label,
                    color,
                );
            }
        });
    });
}

fn spawn_corrupt_dialog(commands: &mut Commands, t: &dtx_ui::theme::Theme, message: &str) {
    let message = message.to_owned();
    let t = *t;
    spawn_modal(commands, 460.0, &t, move |card| {
        card.spawn((
            Text::new("Profile file is unreadable"),
            dtx_ui::theme::Theme::font(20.0),
            TextColor(t.text_primary),
        ));
        card.spawn((
            Text::new(message),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(chrome::ERR),
        ));
        card.spawn((
            Text::new("Backing up and resetting replaces it with built-in defaults; other profiles are unaffected."),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|buttons| {
            spawn_dialog_btn(
                buttons,
                &t,
                DialogButton::CorruptCancel,
                0,
                "Cancel",
                Color::srgb(0.18, 0.18, 0.22),
            );
            spawn_dialog_btn(
                buttons,
                &t,
                DialogButton::CorruptConfirm,
                1,
                "Back up & reset",
                t.accent,
            );
        });
    });
}

// ===== Text entry + submit/cancel =====

/// Printable keys append, Backspace pops, Enter submits (validated via
/// `submit_name`), Escape closes. Only active while `Name` is open.
#[allow(clippy::too_many_arguments)]
fn handle_name_dialog_input(
    mut chars: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    dialog_kind: Res<DialogKind>,
    mut dialog: ResMut<ProfileDialogState>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut error: ResMut<ProfileUiErrorState>,
) {
    if !matches!(*dialog, ProfileDialogState::Name { .. }) {
        chars.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        *dialog = ProfileDialogState::Closed;
        chars.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        submit_name_dialog(
            &mut dialog,
            dialog_kind.0,
            &mut session,
            &mut lane_draft,
            &mut live,
            &mut rev,
            &mut error,
        );
        chars.clear();
        return;
    }
    let ProfileDialogState::Name { value, error, .. } = &mut *dialog else {
        return;
    };
    let mut changed = false;
    for ev in chars.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    if value.chars().count() < 48 && !c.is_control() {
                        value.push(c);
                        changed = true;
                    }
                }
            }
            Key::Space => {
                if value.chars().count() < 48 {
                    value.push(' ');
                    changed = true;
                }
            }
            Key::Backspace => {
                value.pop();
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        *error = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn submit_name_dialog(
    dialog: &mut ProfileDialogState,
    kind: Option<ProfileKind>,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
    live: &mut LiveBindings,
    rev: &mut super::bindings_panel::BindingsRev,
    error: &mut ProfileUiErrorState,
) {
    let Some(kind) = kind else {
        *dialog = ProfileDialogState::Closed;
        return;
    };
    let action = match &*dialog {
        ProfileDialogState::Name { action, .. } => *action,
        _ => return,
    };
    let (builtins, users) = profile_bar_ui::kind_names(kind);
    let info = profile_bar_ui::bar_info(kind, session);
    // Rename may resubmit the current name unchanged; SaveAs must always
    // produce a name distinct from every existing one, including the
    // current selection.
    let current = matches!(action, NameAction::Rename).then_some(info.selected.as_str());
    let (next_state, name) = profile_dialog::submit_name(
        dialog,
        builtins.iter().map(String::as_str),
        users.iter().map(String::as_str),
        current,
    );
    let Some(name) = name else {
        *dialog = next_state;
        return;
    };
    let result = match action {
        NameAction::SaveAs => profile_bar_ui::saveas_kind(kind, name, session, lane_draft),
        NameAction::Rename => profile_bar_ui::rename_kind(kind, name, session, lane_draft),
    };
    match result {
        Ok(()) => {
            error.0 = None;
            profile_bar_ui::refresh_live_bindings(kind, session, live, rev);
        }
        Err(message) => error.0 = Some(profile_bar_ui::ui_error(kind, message)),
    }
    *dialog = ProfileDialogState::Closed;
}

/// Apply one activated dialog button — shared by mouse clicks and the
/// keyboard's Enter/Esc so both dispatch identically. Snapshots the dialog
/// state before mutating it so the match doesn't hold a live borrow across
/// the `*dialog = ...` writes below.
#[allow(clippy::too_many_arguments)]
fn dispatch_dialog_button(
    pressed: DialogButton,
    dialog_kind: Option<ProfileKind>,
    dialog: &mut ProfileDialogState,
    session: &mut CustomizeSession,
    lane_draft: &mut LaneProfileDraft,
    live: &mut LiveBindings,
    rev: &mut super::bindings_panel::BindingsRev,
    error: &mut ProfileUiErrorState,
) {
    let snapshot = dialog.clone();
    match (&snapshot, pressed) {
        (ProfileDialogState::ConfirmDelete { .. }, DialogButton::ConfirmDelete) => {
            let Some(kind) = dialog_kind else {
                *dialog = ProfileDialogState::Closed;
                return;
            };
            match profile_bar_ui::delete_kind(kind, session, lane_draft) {
                Ok(()) => {
                    error.0 = None;
                    profile_bar_ui::refresh_live_bindings(kind, session, live, rev);
                }
                Err(message) => error.0 = Some(profile_bar_ui::ui_error(kind, message)),
            }
            *dialog = ProfileDialogState::Closed;
        }
        (ProfileDialogState::ConfirmDelete { .. }, DialogButton::CancelDelete) => {
            *dialog = ProfileDialogState::Closed;
        }
        (
            ProfileDialogState::Dirty {
                kind,
                pending,
                builtin_selected,
            },
            DialogButton::Dirty(decision),
        ) => {
            let (kind, builtin_selected) = (*kind, *builtin_selected);
            match profile_bar_ui::resolve_dirty(
                kind,
                pending,
                builtin_selected,
                decision,
                session,
                lane_draft,
            ) {
                Ok(needs_refresh) => {
                    error.0 = None;
                    if needs_refresh {
                        profile_bar_ui::refresh_live_bindings(kind, session, live, rev);
                    }
                }
                Err(message) => error.0 = Some(profile_bar_ui::ui_error(kind, message)),
            }
            *dialog = ProfileDialogState::Closed;
        }
        (ProfileDialogState::CorruptReset { kind, .. }, DialogButton::CorruptConfirm) => {
            let result = backup_and_reset(*kind);
            *dialog = profile_dialog::apply_reset_outcome(*kind, result);
        }
        (ProfileDialogState::CorruptReset { .. }, DialogButton::CorruptCancel) => {
            *dialog = ProfileDialogState::Closed;
        }
        _ => {}
    }
}

/// ConfirmDelete / Dirty / CorruptReset button clicks.
#[allow(clippy::too_many_arguments)]
fn handle_dialog_buttons(
    buttons: Query<(&Interaction, &DialogButton), Changed<Interaction>>,
    dialog_kind: Res<DialogKind>,
    mut dialog: ResMut<ProfileDialogState>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut error: ResMut<ProfileUiErrorState>,
) {
    let Some(pressed) = buttons
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, button)| *button)
    else {
        return;
    };
    dispatch_dialog_button(
        pressed,
        dialog_kind.0,
        &mut dialog,
        &mut session,
        &mut lane_draft,
        &mut live,
        &mut rev,
        &mut error,
    );
}

/// ←/→/Enter/Esc for the button-row dialogs. Esc always activates the safe
/// dismiss (row index 0); Enter activates the focused button through the
/// exact same dispatcher as a click. Skips its opening frame (the keypress
/// that raised the dialog must not immediately act on it) and the Name
/// dialog (own key handling).
#[allow(clippy::too_many_arguments)]
fn handle_dialog_keys(
    keys: Res<ButtonInput<KeyCode>>,
    dialog_kind: Res<DialogKind>,
    mut focus: ResMut<ProfileDialogFocus>,
    mut dialog: ResMut<ProfileDialogState>,
    mut session: ResMut<CustomizeSession>,
    mut lane_draft: ResMut<LaneProfileDraft>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<super::bindings_panel::BindingsRev>,
    mut error: ResMut<ProfileUiErrorState>,
) {
    let buttons = dialog_buttons(&dialog);
    if buttons.is_empty() || dialog.is_changed() {
        return;
    }
    let next = super::close_dialog::step_focus(
        focus.0,
        buttons.len(),
        keys.just_pressed(KeyCode::ArrowLeft),
        keys.just_pressed(KeyCode::ArrowRight),
    );
    if next != focus.0 {
        focus.0 = next;
    }
    let pressed = if keys.just_pressed(KeyCode::Escape) {
        Some(buttons[0])
    } else if keys.just_pressed(KeyCode::Enter) {
        buttons.get(focus.0.min(buttons.len() - 1)).copied()
    } else {
        None
    };
    if let Some(pressed) = pressed {
        dispatch_dialog_button(
            pressed,
            dialog_kind.0,
            &mut dialog,
            &mut session,
            &mut lane_draft,
            &mut live,
            &mut rev,
            &mut error,
        );
    }
}

/// FOCUS_RING on the focused button; cleared entirely for Name/Closed.
fn update_profile_dialog_focus_ring(
    dialog: Res<ProfileDialogState>,
    focus: Res<ProfileDialogFocus>,
    mut buttons: Query<(&DialogBtnIndex, &mut Outline)>,
) {
    let focusable = !dialog_buttons(&dialog).is_empty();
    for (index, mut outline) in &mut buttons {
        if focusable && index.0 == focus.0 {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}

fn backup_and_reset(kind: ProfileKind) -> Result<(), String> {
    let now = std::time::SystemTime::now();
    match kind {
        ProfileKind::Keyboard => dtx_input::profiles::backup_and_reset_keyboard_registry(
            &crate::bindings::keyboard_registry_path(),
            true,
            now,
        )
        .map(|_| ())
        .map_err(|error| error.to_string()),
        ProfileKind::Midi => dtx_input::profiles::backup_and_reset_midi_registry(
            &crate::bindings::midi_registry_path(),
            true,
            now,
        )
        .map(|_| ())
        .map_err(|error| error.to_string()),
        ProfileKind::Lanes => dtx_layout::profiles::backup_and_reset_lane_registry(
            &crate::lanes::lane_registry_path(),
            true,
            now,
        )
        .map(|_| ())
        .map_err(|error| error.to_string()),
        ProfileKind::Settings => Err("settings do not use profile recovery".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::profile_state::PendingProfileAction;

    fn dirty_state() -> ProfileDialogState {
        ProfileDialogState::Dirty {
            kind: ProfileKind::Keyboard,
            pending: PendingProfileAction::Select("Desk".to_owned()),
            builtin_selected: false,
        }
    }

    #[test]
    fn dialog_buttons_put_safe_dismiss_first_and_focus_never_destructive() {
        // ConfirmDelete: [Cancel, Delete] — initial focus on Cancel.
        let confirm = ProfileDialogState::ConfirmDelete {
            name: "Desk".to_owned(),
        };
        assert!(matches!(
            dialog_buttons(&confirm)[0],
            DialogButton::CancelDelete
        ));
        assert_eq!(dialog_buttons(&confirm).len(), 2);
        assert_eq!(initial_focus(&confirm), 0);

        // Dirty: [Cancel, Discard, Save] — initial focus = layout default
        // (Save), which is asserted never-destructive by profile_state tests.
        let dirty = dirty_state();
        let buttons = dialog_buttons(&dirty);
        assert_eq!(buttons.len(), 3);
        assert!(matches!(
            buttons[0],
            DialogButton::Dirty(CloseDecision::Cancel)
        ));
        let layout = profile_state::dirty_dialog_layout(&[ProfileKind::Keyboard], false);
        assert_eq!(initial_focus(&dirty), layout.default_focus);
        assert_ne!(initial_focus(&dirty), layout.destructive);

        // CorruptReset: [Cancel, Back up & reset] — initial focus on Cancel.
        let corrupt = ProfileDialogState::CorruptReset {
            kind: ProfileKind::Midi,
            message: "corrupt".to_owned(),
        };
        assert!(matches!(
            dialog_buttons(&corrupt)[0],
            DialogButton::CorruptCancel
        ));
        assert_eq!(initial_focus(&corrupt), 0);

        // Name / Closed have no traversable row (Name owns its own keys).
        assert!(dialog_buttons(&ProfileDialogState::Closed).is_empty());
    }

    #[test]
    fn escape_dismisses_confirm_delete_without_deleting() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<DialogKind>()
            .init_resource::<ProfileDialogFocus>()
            .init_resource::<CustomizeSession>()
            .init_resource::<LaneProfileDraft>()
            .init_resource::<LiveBindings>()
            .init_resource::<super::super::bindings_panel::BindingsRev>()
            .init_resource::<ProfileUiErrorState>()
            .insert_resource(ProfileDialogState::ConfirmDelete {
                name: "Desk".to_owned(),
            })
            .add_systems(Update, handle_dialog_keys);
        app.update(); // opening frame: is_changed skip
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(
            *app.world().resource::<ProfileDialogState>(),
            ProfileDialogState::Closed,
            "Esc = safe dismiss (CancelDelete path — no registry write)"
        );
    }
}
