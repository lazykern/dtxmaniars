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

pub fn plugin(app: &mut App) {
    app.add_message::<CloseDecisionRequest>()
        .add_systems(
            Update,
            (
                sync_dialog.run_if(resource_changed::<PendingCloseState>),
                handle_buttons.before(super::resolve_pending_close),
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
    }
}

fn dirty_names(dirty: &[ProfileKind]) -> String {
    dirty
        .iter()
        .map(|kind| match kind {
            ProfileKind::Keyboard => "keyboard",
            ProfileKind::Midi => "MIDI",
            ProfileKind::Lanes => "lane layout",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn sync_dialog(
    mut commands: Commands,
    pending: Res<PendingCloseState>,
    session: Res<CustomizeSession>,
    theme: Res<dtx_ui::ThemeResource>,
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
    let decisions = [
        CloseDecision::Cancel,
        CloseDecision::DiscardAll,
        CloseDecision::SaveAll,
    ];
    let t = theme.0;

    commands
        .spawn((
            CloseDialogRoot,
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
                                layout.buttons.iter().zip(decisions).enumerate()
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
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                        ..default()
                                    },
                                    BackgroundColor(color),
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
}
