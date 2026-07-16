//! Dirty-profile close guard UI.

use bevy::prelude::*;

use super::profile_state::{
    dirty_dialog_layout, CloseDecision, CustomizeSession, PendingCloseState, ProfileKind,
};

#[derive(Debug, Clone, Copy, Message)]
pub struct CloseDecisionRequest(pub CloseDecision);

#[derive(Component)]
struct CloseDialogRoot;

#[derive(Component, Clone, Copy)]
struct CloseDialogButton(CloseDecision);

/// Keyboard-focused button index in the close dialog's row
/// (Cancel | Discard all | Save all). Reset to the layout's default focus
/// whenever the guard is (re)armed. Esc stays with `resolve_pending_close`'s
/// existing `close_decision_for_key` fallback (Cancel) — never double-handled
/// here.
#[derive(Resource, Default)]
pub struct CloseDialogFocus(pub usize);

#[derive(Component, Clone, Copy)]
struct CloseDialogBtnIndex(usize);

const CLOSE_DECISIONS: [CloseDecision; 3] = [
    CloseDecision::Cancel,
    CloseDecision::DiscardAll,
    CloseDecision::SaveAll,
];

/// Clamp-move a dialog-row focus index (shared with profile_dialog_ui).
pub(super) fn step_focus(focus: usize, len: usize, left: bool, right: bool) -> usize {
    if len == 0 {
        return 0;
    }
    let mut next = focus.min(len - 1);
    if left {
        next = next.saturating_sub(1);
    }
    if right {
        next = (next + 1).min(len - 1);
    }
    next
}

pub fn plugin(app: &mut App) {
    app.add_message::<CloseDecisionRequest>()
        .init_resource::<CloseDialogFocus>()
        .add_systems(
            Update,
            (
                sync_dialog.run_if(resource_changed::<PendingCloseState>),
                close_dialog_keys.before(super::resolve_pending_close),
                handle_buttons.before(super::resolve_pending_close),
                update_close_dialog_focus_ring,
            )
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(OnExit(game_shell::AppState::Performance), despawn_dialog);
}

fn despawn_dialog(mut commands: Commands, roots: Query<Entity, With<CloseDialogRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn selected_is_builtin(kind: ProfileKind, session: &CustomizeSession) -> bool {
    match kind {
        ProfileKind::Keyboard => {
            dtx_input::profiles::keyboard_builtins().contains_key(&session.0.keyboard.selected)
        }
        ProfileKind::Midi => {
            dtx_input::profiles::midi_builtins().contains_key(&session.0.midi.selected)
        }
        ProfileKind::Lanes => {
            dtx_layout::profiles::lane_builtins().contains_key(&session.0.lanes.selected)
        }
        ProfileKind::Settings => false,
    }
}

fn dirty_names(dirty: &[ProfileKind]) -> String {
    dirty
        .iter()
        .map(|kind| match kind {
            ProfileKind::Keyboard => "keyboard",
            ProfileKind::Midi => "MIDI",
            ProfileKind::Lanes => "lane layout",
            ProfileKind::Settings => "settings",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn sync_dialog(
    mut commands: Commands,
    pending: Res<PendingCloseState>,
    session: Res<CustomizeSession>,
    theme: Res<dtx_ui::ThemeResource>,
    mut focus: ResMut<CloseDialogFocus>,
    roots: Query<Entity, With<CloseDialogRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let PendingCloseState::Pending(close) = &*pending else {
        return;
    };
    let builtin = close
        .dirty
        .first()
        .is_some_and(|kind| selected_is_builtin(*kind, &session));
    let layout = dirty_dialog_layout(&close.dirty, builtin);
    focus.0 = layout.default_focus;
    let t = theme.0;

    commands
        .spawn((
            CloseDialogRoot,
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
                        width: Val::Px(460.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(t.stage_panel_bg),
                    BorderColor::all(t.stage_panel_border),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Save changes before leaving?"),
                        dtx_ui::theme::Theme::font(24.0),
                        TextColor(t.text_primary),
                    ));
                    panel.spawn((
                        Text::new(format!(
                            "Unsaved {} changes will be lost if discarded.",
                            dirty_names(&close.dirty)
                        )),
                        dtx_ui::theme::Theme::font(15.0),
                        TextColor(t.text_secondary),
                    ));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|buttons| {
                            for (index, (label, decision)) in
                                layout.buttons.iter().zip(CLOSE_DECISIONS).enumerate()
                            {
                                let color = if index == layout.destructive {
                                    t.judgment_miss
                                } else if index == layout.default_focus {
                                    t.accent
                                } else {
                                    Color::srgb(0.18, 0.18, 0.22)
                                };
                                buttons.spawn((
                                    CloseDialogButton(decision),
                                    CloseDialogBtnIndex(index),
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                        ..default()
                                    },
                                    BackgroundColor(color),
                                    Outline::new(Val::Px(0.0), Val::Px(2.0), Color::NONE),
                                    children![(
                                        Text::new((*label).to_owned()),
                                        dtx_ui::theme::Theme::font(14.0),
                                        TextColor(t.text_primary),
                                    )],
                                ));
                            }
                        });
                });
        });
}

fn handle_buttons(
    interactions: Query<(&Interaction, &CloseDialogButton), Changed<Interaction>>,
    mut requests: MessageWriter<CloseDecisionRequest>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            requests.write(CloseDecisionRequest(button.0));
        }
    }
}

/// ←/→ move the focused button (clamped); Enter activates it — the same
/// `CloseDecisionRequest` a click sends, resolved by `resolve_pending_close`
/// (whose Enter→SaveAll fallback only fires when no request arrived, so the
/// focused decision wins).
fn close_dialog_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut nav: MessageReader<game_shell::NavAction>,
    pending: Res<PendingCloseState>,
    mut focus: ResMut<CloseDialogFocus>,
    mut requests: MessageWriter<CloseDecisionRequest>,
) {
    if !matches!(*pending, PendingCloseState::Pending(_)) || pending.is_changed() {
        // Closed, or armed this frame (the arming Esc/Enter must not resolve it).
        return;
    }
    let mut nav_left = false;
    let mut nav_right = false;
    let mut nav_confirm = false;
    let mut nav_back = false;
    for action in nav.read() {
        use game_shell::SystemVerb;
        match action.verb {
            SystemVerb::Decrease | SystemVerb::NavigateUp => nav_left = true,
            SystemVerb::Increase | SystemVerb::NavigateDown => nav_right = true,
            SystemVerb::Confirm => nav_confirm = true,
            SystemVerb::Back => nav_back = true,
            _ => {}
        }
    }
    let next = step_focus(
        focus.0,
        CLOSE_DECISIONS.len(),
        keys.just_pressed(KeyCode::ArrowLeft) || nav_left,
        keys.just_pressed(KeyCode::ArrowRight) || nav_right,
    );
    if next != focus.0 {
        focus.0 = next;
    }
    if keys.just_pressed(KeyCode::Escape) || nav_back {
        requests.write(CloseDecisionRequest(CloseDecision::Cancel));
    } else if keys.just_pressed(KeyCode::Enter) || nav_confirm {
        requests.write(CloseDecisionRequest(CLOSE_DECISIONS[focus.0]));
    }
}

/// FOCUS_RING outline on the focused button, in addition to the existing
/// default/destructive coloring. Hover never moves keyboard focus.
fn update_close_dialog_focus_ring(
    focus: Res<CloseDialogFocus>,
    mut buttons: Query<(&CloseDialogBtnIndex, &mut Outline)>,
) {
    for (index, mut outline) in &mut buttons {
        if index.0 == focus.0 {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::editor::profile_state::{CloseIntent, PendingClose, ProfileKind};

    #[test]
    fn pending_midi_close_spawns_three_decision_buttons() {
        let mut app = App::new();
        app.insert_resource(PendingCloseState::Pending(PendingClose {
            intent: CloseIntent::Customize,
            dirty: vec![ProfileKind::Midi],
        }))
        .init_resource::<CustomizeSession>()
        .init_resource::<CloseDialogFocus>()
        .init_resource::<dtx_ui::ThemeResource>()
        .add_systems(Update, sync_dialog);

        app.update();

        let world = app.world_mut();
        let roots = world.query::<&CloseDialogRoot>().iter(world).count();
        let decisions: Vec<_> = world
            .query::<&CloseDialogButton>()
            .iter(world)
            .map(|button| button.0)
            .collect();
        assert_eq!(roots, 1);
        assert_eq!(
            decisions,
            vec![
                CloseDecision::Cancel,
                CloseDecision::DiscardAll,
                CloseDecision::SaveAll,
            ]
        );
    }

    #[test]
    fn step_focus_clamps_at_both_ends() {
        assert_eq!(step_focus(2, 3, true, false), 1);
        assert_eq!(step_focus(0, 3, true, false), 0, "clamps left");
        assert_eq!(step_focus(2, 3, false, true), 2, "clamps right");
        assert_eq!(
            step_focus(9, 3, false, false),
            2,
            "stale index clamps into range"
        );
        assert_eq!(step_focus(0, 0, true, true), 0, "empty row is inert");
    }

    #[test]
    fn arrows_move_focus_and_enter_dispatches_focused_decision() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CloseDialogFocus>()
            .insert_resource(PendingCloseState::Pending(PendingClose {
                intent: CloseIntent::Customize,
                dirty: vec![ProfileKind::Midi],
            }))
            .init_resource::<CustomizeSession>()
            .init_resource::<dtx_ui::ThemeResource>()
            .add_message::<game_shell::NavAction>()
            .add_message::<CloseDecisionRequest>()
            .add_systems(
                Update,
                (
                    sync_dialog.run_if(resource_changed::<PendingCloseState>),
                    close_dialog_keys,
                )
                    .chain(),
            );

        app.update(); // armed frame: sync sets focus, keys system skips it
        assert_eq!(
            app.world().resource::<CloseDialogFocus>().0,
            2,
            "initial focus = layout default (Save all), never the destructive button"
        );

        // ← moves to Discard; Enter dispatches THAT decision.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowLeft);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert_eq!(app.world().resource::<CloseDialogFocus>().0, 1);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        let requests: Vec<_> = app
            .world()
            .resource::<Messages<CloseDecisionRequest>>()
            .iter_current_update_messages()
            .map(|request| request.0)
            .collect();
        assert_eq!(requests, vec![CloseDecision::DiscardAll]);
    }
}
