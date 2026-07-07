//! Editor overlay UI: left sidebar (widget list + actions), spawned while open.

use bevy::prelude::*;
use dtx_layout::WidgetKind;

use super::drag::Selection;
use super::{save, undo::UndoStack, EditorOpen};
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Component)]
struct EditorUiRoot;

#[derive(Component, Clone, Copy)]
enum EditorButton {
    Select(WidgetKind),
    ResetWidget,
    ResetAll,
    NextPreset,
    Save,
    Undo,
    Redo,
    Close,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_ui_on_open.run_if(resource_changed::<EditorOpen>),
            (handle_buttons, highlight_selection).run_if(super::editor_open),
            close_on_escape.run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_editor_ui);
}

/// Despawn the sidebar when leaving Performance (covers the song-ended-mid-edit
/// path; `close_editor_on_exit` in mod.rs clears the editor state alongside).
fn despawn_editor_ui(mut commands: Commands, existing: Query<Entity, With<EditorUiRoot>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

/// Spawn the sidebar when the editor opens; despawn when it closes.
fn spawn_ui_on_open(
    mut commands: Commands,
    open: Res<EditorOpen>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<EditorUiRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            EditorUiRoot,
            super::picking::EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(220.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.92)),
            GlobalZIndex(2000),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        spawn_label(p, &t, "LAYOUT EDITOR");
        spawn_label(p, &t, "- widgets -");
        for kind in WidgetKind::ALL {
            spawn_button(p, &t, EditorButton::Select(kind), kind.display_name());
        }
        spawn_label(p, &t, "- actions -");
        spawn_button(p, &t, EditorButton::ResetWidget, "Reset Widget");
        spawn_button(p, &t, EditorButton::ResetAll, "Reset All");
        spawn_button(p, &t, EditorButton::NextPreset, "Next Lane Preset");
        spawn_button(p, &t, EditorButton::Undo, "Undo (Ctrl+Z)");
        spawn_button(p, &t, EditorButton::Redo, "Redo (Ctrl+Y)");
        spawn_button(p, &t, EditorButton::Save, "Save (Ctrl+S)");
        spawn_button(p, &t, EditorButton::Close, "Close (Esc)");
    });
}

fn spawn_label(p: &mut ChildSpawnerCommands, theme: &dtx_ui::theme::Theme, text: &str) {
    p.spawn((
        Text::new(text.to_string()),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(theme.text_secondary),
    ));
}

fn spawn_button(
    p: &mut ChildSpawnerCommands,
    theme: &dtx_ui::theme::Theme,
    button: EditorButton,
    label: &str,
) {
    p.spawn((
        button,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
        children![(
            Text::new(label.to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(theme.text_primary),
        )],
    ));
}

/// Handle button clicks.
fn handle_buttons(
    mut interactions: Query<
        (&Interaction, &EditorButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut selection: ResMut<Selection>,
    mut open: ResMut<EditorOpen>,
    mut layouts: ResMut<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
    mut stack: ResMut<UndoStack>,
    prev: Res<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    for (interaction, button, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                bg.0 = Color::srgb(0.25, 0.25, 0.32);
                let snap = super::undo::Snapshot {
                    layouts: layouts.clone(),
                    lanes: lanes.clone(),
                };
                match *button {
                    EditorButton::Select(kind) => selection.0 = Some(kind),
                    EditorButton::ResetWidget => {
                        if let Some(kind) = selection.0 {
                            stack.push(&layouts, &lanes);
                            save::reset_widget(&mut layouts, kind);
                        }
                    }
                    EditorButton::ResetAll => {
                        stack.push(&layouts, &lanes);
                        save::reset_all_widgets(&mut layouts);
                    }
                    EditorButton::NextPreset => {
                        stack.push(&layouts, &lanes);
                        save::next_lane_preset(&mut lanes);
                    }
                    EditorButton::Undo => {
                        if let Some(s) = stack.undo(snap) {
                            *layouts = s.layouts;
                            *lanes = s.lanes;
                        }
                    }
                    EditorButton::Redo => {
                        if let Some(s) = stack.redo(snap) {
                            *layouts = s.layouts;
                            *lanes = s.lanes;
                        }
                    }
                    EditorButton::Save => {
                        let file = save::layout_file_from(&layouts, &lanes);
                        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
                            warn!("layout save failed: {e}");
                        }
                    }
                    EditorButton::Close => {
                        open.0 = false;
                        autoplay.0 = prev.0;
                    }
                }
            }
            Interaction::Hovered => bg.0 = Color::srgb(0.2, 0.2, 0.26),
            Interaction::None => bg.0 = Color::srgb(0.14, 0.14, 0.18),
        }
    }
}

/// Tint the selected widget's sidebar button.
fn highlight_selection(
    selection: Res<Selection>,
    mut buttons: Query<(&EditorButton, &mut BackgroundColor, &Interaction)>,
) {
    for (button, mut bg, interaction) in &mut buttons {
        if matches!(interaction, Interaction::None) {
            if let EditorButton::Select(kind) = *button {
                bg.0 = if selection.0 == Some(kind) {
                    Color::srgb(0.22, 0.3, 0.42)
                } else {
                    Color::srgb(0.14, 0.14, 0.18)
                };
            }
        }
    }
}

/// Esc closes the editor (pause is gated off while open).
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<EditorOpen>,
    prev: Res<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
        autoplay.0 = prev.0;
    }
}
