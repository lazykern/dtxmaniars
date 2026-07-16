use bevy::prelude::*;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::widget::stage_background::spawn_stage_background;
use dtx_ui::{Notification, NotificationQueue, Theme, ThemeResource};
use game_shell::{
    AppState, NavAction, SystemVerb, TransitionRequest, despawn_stage, request_transition,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomeItem {
    #[default]
    Play,
    Settings,
    Exit,
}

impl HomeItem {
    const ALL: [Self; 3] = [Self::Play, Self::Settings, Self::Exit];

    fn step(self, delta: i32) -> Self {
        let index = Self::ALL.iter().position(|item| *item == self).unwrap_or(0) as i32;
        Self::ALL[(index + delta).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExitChoice {
    #[default]
    Closed,
    Cancel,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeInput {
    Previous,
    Next,
    Confirm,
    Back,
}

#[derive(Message, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomeCommand {
    #[default]
    None,
    Play,
    Settings,
    Exit,
}

#[derive(Component)]
pub struct HomeEntity;

#[derive(Component)]
struct HomeMenuButton(HomeItem);

#[derive(Component)]
struct HomeMenuLabel;

#[derive(Component)]
struct ExitDialogRoot;

#[derive(Component)]
struct ExitDialogButton(ExitChoice);

#[derive(Component)]
struct ExitDialogLabel;

pub fn plugin(app: &mut App) {
    app.init_resource::<HomeState>()
        .add_message::<HomeCommand>()
        .add_systems(
            OnEnter(AppState::Title),
            (reset_home_state, spawn_home).chain(),
        )
        .add_systems(OnExit(AppState::Title), despawn_stage::<HomeEntity>)
        .add_systems(
            Update,
            (
                home_input,
                home_pointer_input,
                execute_home_commands,
                render_home_state,
            )
                .chain()
                .run_if(in_state(AppState::Title)),
        );
}

fn reset_home_state(mut state: ResMut<HomeState>) {
    *state = HomeState::default();
}

fn spawn_home(mut commands: Commands, theme: Res<ThemeResource>) {
    let t = theme.0;
    commands
        .spawn((
            HomeEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(34.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_stage_background(root, &t);
            root.spawn((
                Text::new("DTXMANIARS"),
                Theme::font(56.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Display),
                TextColor(t.text_primary),
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, -100.0), 0.0, 360.0),
            ));
            root.spawn((
                Node {
                    width: Val::Px(360.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 70.0), 100.0, 280.0),
            ))
            .with_children(|menu| {
                for (item, label) in [
                    (HomeItem::Play, "PLAY"),
                    (HomeItem::Settings, "SETTINGS"),
                    (HomeItem::Exit, "EXIT"),
                ] {
                    menu.spawn((
                        Button,
                        HomeMenuButton(item),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(64.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(t.stage_panel_bg),
                        BorderColor::all(t.stage_panel_border),
                        UiTransform::default(),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            HomeMenuLabel,
                            Text::new(label),
                            Theme::font(25.0),
                            dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                            TextColor(t.text_primary),
                        ));
                    });
                }
            });
            root.spawn((
                Text::new(format!("v{}", env!("CARGO_PKG_VERSION"))),
                Theme::font(12.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                TextColor(t.text_secondary),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    bottom: Val::Px(12.0),
                    ..default()
                },
            ));
            spawn_exit_dialog(root, &t);
        });
}

fn spawn_exit_dialog(root: &mut ChildSpawnerCommands, t: &Theme) {
    root.spawn((
        ExitDialogRoot,
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
        GlobalZIndex(20),
    ))
    .with_children(|scrim| {
        scrim
            .spawn((
                Node {
                    width: Val::Px(520.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(28.0),
                    padding: UiRect::all(Val::Px(30.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.stage_panel_border),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("EXIT DTXMANIARS?"),
                    Theme::font(30.0),
                    dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                    TextColor(t.text_primary),
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(18.0),
                    ..default()
                })
                .with_children(|actions| {
                    for (choice, label) in
                        [(ExitChoice::Cancel, "CANCEL"), (ExitChoice::Exit, "EXIT")]
                    {
                        actions
                            .spawn((
                                Button,
                                ExitDialogButton(choice),
                                Node {
                                    width: Val::Px(180.0),
                                    height: Val::Px(54.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(t.stage_panel_bg),
                                BorderColor::all(t.stage_panel_border),
                                UiTransform::default(),
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    ExitDialogLabel,
                                    Text::new(label),
                                    Theme::font(18.0),
                                    TextColor(t.text_primary),
                                ));
                            });
                    }
                });
            });
    });
}

fn home_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<NavAction>,
    mut state: ResMut<HomeState>,
    mut commands: MessageWriter<HomeCommand>,
) {
    let keyboard = if keys.just_pressed(KeyCode::ArrowUp)
        || (state.exit_dialog != ExitChoice::Closed && keys.just_pressed(KeyCode::ArrowLeft))
    {
        Some(HomeInput::Previous)
    } else if keys.just_pressed(KeyCode::ArrowDown)
        || (state.exit_dialog != ExitChoice::Closed && keys.just_pressed(KeyCode::ArrowRight))
    {
        Some(HomeInput::Next)
    } else if keys.just_pressed(KeyCode::Enter) {
        Some(HomeInput::Confirm)
    } else if keys.just_pressed(KeyCode::Escape) {
        Some(HomeInput::Back)
    } else {
        None
    };
    if let Some(input) = keyboard {
        emit_home_command(state.apply(input), &mut commands);
    }
    for action in actions.read() {
        let input = match action.verb {
            SystemVerb::NavigateUp => Some(HomeInput::Previous),
            SystemVerb::NavigateDown => Some(HomeInput::Next),
            SystemVerb::Confirm => Some(HomeInput::Confirm),
            SystemVerb::Back => Some(HomeInput::Back),
            _ => None,
        };
        if let Some(input) = input {
            emit_home_command(state.apply(input), &mut commands);
        }
    }
}

fn home_pointer_input(
    mut state: ResMut<HomeState>,
    mut commands: MessageWriter<HomeCommand>,
    menu: Query<(&Interaction, &HomeMenuButton), Changed<Interaction>>,
    dialog: Query<(&Interaction, &ExitDialogButton), Changed<Interaction>>,
) {
    for (interaction, button) in &menu {
        if state.exit_dialog != ExitChoice::Closed {
            continue;
        }
        if matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            state.focused = button.0;
        }
        if *interaction == Interaction::Pressed {
            emit_home_command(state.apply(HomeInput::Confirm), &mut commands);
        }
    }
    for (interaction, button) in &dialog {
        if *interaction == Interaction::Pressed && state.exit_dialog != ExitChoice::Closed {
            state.exit_dialog = button.0;
            emit_home_command(state.apply(HomeInput::Confirm), &mut commands);
        }
    }
}

fn emit_home_command(command: HomeCommand, commands: &mut MessageWriter<HomeCommand>) {
    if command != HomeCommand::None {
        commands.write(command);
    }
}

fn execute_home_commands(
    mut commands: MessageReader<HomeCommand>,
    mut requests: MessageWriter<TransitionRequest>,
    mut db: ResMut<dtx_library::SongDb>,
    mut pending: ResMut<game_shell::PendingCustomizeTab>,
    mut session: ResMut<game_shell::EditorSession>,
    mut selected: ResMut<crate::song_select::SelectedSong>,
    mut notifications: ResMut<NotificationQueue>,
) {
    for command in commands.read() {
        match command {
            HomeCommand::Play => request_transition(&mut requests, AppState::SongSelect),
            HomeCommand::Settings => {
                if !crate::title::request_gameplay_settings(
                    &mut db,
                    &mut pending,
                    &mut session,
                    &mut selected,
                    &mut requests,
                ) {
                    notifications.push(Notification::warning(
                        "Settings needs at least one available chart",
                    ));
                }
            }
            HomeCommand::Exit => request_transition(&mut requests, AppState::End),
            HomeCommand::None => {}
        }
    }
}

fn render_home_state(
    state: Res<HomeState>,
    theme: Res<ThemeResource>,
    mut menu: Query<
        (
            &HomeMenuButton,
            &mut BorderColor,
            &mut BackgroundColor,
            &mut UiTransform,
        ),
        Without<ExitDialogButton>,
    >,
    mut dialog_root: Query<&mut Node, With<ExitDialogRoot>>,
    mut dialog: Query<
        (
            &ExitDialogButton,
            &mut BorderColor,
            &mut BackgroundColor,
            &mut UiTransform,
        ),
        Without<HomeMenuButton>,
    >,
) {
    if !state.is_changed() {
        return;
    }
    let t = theme.0;
    for (button, mut border, mut bg, mut transform) in &mut menu {
        let focused = state.exit_dialog == ExitChoice::Closed && state.focused == button.0;
        border.set_all(if focused {
            t.select_yellow
        } else {
            t.stage_panel_border
        });
        bg.0 = if focused {
            t.selection_highlight
        } else {
            t.stage_panel_bg
        };
        transform.scale = if focused {
            Vec2::splat(1.04)
        } else {
            Vec2::ONE
        };
    }
    for mut node in &mut dialog_root {
        node.display = if state.exit_dialog == ExitChoice::Closed {
            Display::None
        } else {
            Display::Flex
        };
    }
    for (button, mut border, mut bg, mut transform) in &mut dialog {
        let focused = state.exit_dialog == button.0;
        let focus_color = if button.0 == ExitChoice::Exit {
            t.judgment_miss
        } else {
            t.select_yellow
        };
        border.set_all(if focused {
            focus_color
        } else {
            t.stage_panel_border
        });
        bg.0 = if focused {
            focus_color.with_alpha(0.18)
        } else {
            t.stage_panel_bg
        };
        transform.scale = if focused {
            Vec2::splat(1.04)
        } else {
            Vec2::ONE
        };
    }
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HomeState {
    pub focused: HomeItem,
    pub exit_dialog: ExitChoice,
}

impl HomeState {
    pub fn apply(&mut self, input: HomeInput) -> HomeCommand {
        if self.exit_dialog != ExitChoice::Closed {
            match input {
                HomeInput::Previous | HomeInput::Next => {
                    self.exit_dialog = match self.exit_dialog {
                        ExitChoice::Cancel => ExitChoice::Exit,
                        ExitChoice::Exit => ExitChoice::Cancel,
                        ExitChoice::Closed => ExitChoice::Cancel,
                    };
                }
                HomeInput::Back => self.exit_dialog = ExitChoice::Closed,
                HomeInput::Confirm => match self.exit_dialog {
                    ExitChoice::Cancel => self.exit_dialog = ExitChoice::Closed,
                    ExitChoice::Exit => return HomeCommand::Exit,
                    ExitChoice::Closed => {}
                },
            }
            return HomeCommand::None;
        }

        match input {
            HomeInput::Previous => self.focused = self.focused.step(-1),
            HomeInput::Next => self.focused = self.focused.step(1),
            HomeInput::Back => self.exit_dialog = ExitChoice::Cancel,
            HomeInput::Confirm => match self.focused {
                HomeItem::Play => return HomeCommand::Play,
                HomeItem::Settings => return HomeCommand::Settings,
                HomeItem::Exit => self.exit_dialog = ExitChoice::Cancel,
            },
        }
        HomeCommand::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_defaults_to_play_and_closed_modal() {
        let state = HomeState::default();
        assert_eq!(state.focused, HomeItem::Play);
        assert_eq!(state.exit_dialog, ExitChoice::Closed);
    }

    #[test]
    fn home_focus_wraps_across_exactly_three_items() {
        let mut state = HomeState::default();
        assert_eq!(state.apply(HomeInput::Previous), HomeCommand::None);
        assert_eq!(state.focused, HomeItem::Exit);
        assert_eq!(state.apply(HomeInput::Next), HomeCommand::None);
        assert_eq!(state.focused, HomeItem::Play);
    }

    #[test]
    fn back_opens_exit_dialog_with_cancel_focused() {
        let mut state = HomeState::default();
        assert_eq!(state.apply(HomeInput::Back), HomeCommand::None);
        assert_eq!(state.exit_dialog, ExitChoice::Cancel);
    }

    #[test]
    fn exit_dialog_can_cancel_or_request_end() {
        let mut state = HomeState::default();
        state.apply(HomeInput::Back);
        assert_eq!(state.apply(HomeInput::Confirm), HomeCommand::None);
        assert_eq!(state.exit_dialog, ExitChoice::Closed);

        state.apply(HomeInput::Back);
        state.apply(HomeInput::Next);
        assert_eq!(state.exit_dialog, ExitChoice::Exit);
        assert_eq!(state.apply(HomeInput::Confirm), HomeCommand::Exit);
    }

    #[test]
    fn activating_each_home_item_emits_its_command() {
        let mut state = HomeState::default();
        assert_eq!(state.apply(HomeInput::Confirm), HomeCommand::Play);
        state.apply(HomeInput::Next);
        assert_eq!(state.apply(HomeInput::Confirm), HomeCommand::Settings);
        state.apply(HomeInput::Next);
        assert_eq!(state.apply(HomeInput::Confirm), HomeCommand::None);
        assert_eq!(state.exit_dialog, ExitChoice::Cancel);
    }

    #[test]
    fn home_plugin_spawns_three_items_without_system_conflicts() {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, bevy::state::app::StatesPlugin));
        app.init_state::<AppState>()
            .add_message::<NavAction>()
            .add_message::<TransitionRequest>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<dtx_library::SongDb>()
            .init_resource::<game_shell::PendingCustomizeTab>()
            .init_resource::<game_shell::EditorSession>()
            .init_resource::<crate::song_select::SelectedSong>()
            .init_resource::<NotificationQueue>()
            .init_resource::<ThemeResource>();
        plugin(&mut app);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Title);

        app.update();

        let count = app
            .world_mut()
            .query::<&HomeMenuButton>()
            .iter(app.world())
            .count();
        assert_eq!(count, 3);
        assert_eq!(app.world().resource::<HomeState>().focused, HomeItem::Play);
    }
}
