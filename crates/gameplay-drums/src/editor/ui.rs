//! Editor overlay UI: left sidebar (widget list + actions), spawned while open.

use bevy::prelude::*;
use dtx_layout::WidgetKind;

use super::drag::Selection;
use super::{save, undo::UndoStack, EditorOpen};
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Component)]
struct EditorUiRoot;

/// A rail button that activates a Customize tab.
#[derive(Component, Clone, Copy)]
pub struct TabButton(pub game_shell::CustomizeTab);

#[derive(Component, Clone, Copy)]
enum EditorButton {
    Select(WidgetKind),
    ResetAll,
    Save,
    Undo,
    Redo,
    Close,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_ui_on_open.run_if(ui_needs_respawn),
            (handle_buttons, handle_tab_buttons, highlight_selection).run_if(super::editor_open),
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

/// Rebuild the sidebar when the editor opens/closes or the active tab changes.
fn ui_needs_respawn(open: Res<EditorOpen>, active: Res<super::tabs::ActiveTab>) -> bool {
    open.is_changed() || active.is_changed()
}

/// Spawn the sidebar when the editor opens; despawn when it closes.
fn spawn_ui_on_open(
    mut commands: Commands,
    open: Res<EditorOpen>,
    active: Res<super::tabs::ActiveTab>,
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

    let active_tab = active.0;
    commands.entity(root).with_children(|p| {
        spawn_label(p, &t, "CUSTOMIZE");
        spawn_label(p, &t, "SETTINGS");
        for tab in game_shell::CustomizeTab::SETTINGS {
            spawn_tab_button(p, &t, tab, tab == active_tab);
        }
        spawn_label(p, &t, "KIT");
        for tab in game_shell::CustomizeTab::KIT {
            spawn_tab_button(p, &t, tab, tab == active_tab);
        }
        if active_tab == game_shell::CustomizeTab::Widgets {
            spawn_label(p, &t, "- widgets -");
            for kind in WidgetKind::ALL {
                spawn_button(p, &t, EditorButton::Select(kind), kind.display_name());
            }
        }
        spawn_label(p, &t, "- actions -");
        spawn_button(p, &t, EditorButton::ResetAll, "Reset All");
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

/// Spawn a tab-rail button; `active` gets the brighter selected tint.
fn spawn_tab_button(
    p: &mut ChildSpawnerCommands,
    theme: &dtx_ui::theme::Theme,
    tab: game_shell::CustomizeTab,
    active: bool,
) {
    let bg = if active {
        Color::srgb(0.22, 0.3, 0.42)
    } else {
        Color::srgb(0.14, 0.14, 0.18)
    };
    p.spawn((
        TabButton(tab),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(bg),
        children![(
            Text::new(tab.label().to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(theme.text_primary),
        )],
    ));
}

/// Handle tab-rail clicks: activate the clicked Customize tab.
fn handle_tab_buttons(
    q: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut active: ResMut<super::tabs::ActiveTab>,
) {
    for (interaction, tab) in &q {
        if *interaction == Interaction::Pressed {
            active.0 = tab.0;
        }
    }
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
    mut session: ResMut<game_shell::EditorSession>,
    mut requests: MessageWriter<game_shell::TransitionRequest>,
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
                    EditorButton::ResetAll => {
                        stack.push(&layouts, &lanes);
                        save::reset_all_widgets(&mut layouts);
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
                        selection.0 = None;
                        if session.0 {
                            session.0 = false;
                            game_shell::request_transition(
                                &mut requests,
                                game_shell::AppState::Title,
                            );
                        }
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

/// Esc: first press deselects; with nothing selected it closes the editor
/// (pause is gated off while open).
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut open: ResMut<EditorOpen>,
    prev: Res<super::PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut session: ResMut<game_shell::EditorSession>,
    mut requests: MessageWriter<game_shell::TransitionRequest>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if selection.0.is_some() {
            selection.0 = None;
        } else {
            open.0 = false;
            autoplay.0 = prev.0;
            if session.0 {
                session.0 = false;
                game_shell::request_transition(&mut requests, game_shell::AppState::Title);
            }
        }
    }
}
