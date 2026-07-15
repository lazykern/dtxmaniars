//! Local Song Select substate for final chart confirmation.

use bevy::image::{ImageFormatSetting, ImageLoaderSettings};
use bevy::prelude::*;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::{Notification, NotificationQueue, Theme, ThemeResource};
use game_shell::{
    AppState, NavAction, NavSource, NavVerb, PracticeIntent, TransitionRequest, despawn_stage,
    request_transition,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SongReadyLayer {
    #[default]
    Closed,
    Browse,
    Edit,
    PrimaryDetail,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadyMode {
    #[default]
    Normal,
    Practice,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadyCard {
    Modifiers,
    Mode,
    #[default]
    Song,
    LaneSpeed,
    Audio,
}

impl ReadyCard {
    const ALL: [Self; 5] = [
        Self::Modifiers,
        Self::Mode,
        Self::Song,
        Self::LaneSpeed,
        Self::Audio,
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AudioField {
    #[default]
    Bgm,
    Drums,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ReadyConfigDraft {
    pub config: dtx_config::Config,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct ReadyActionCapture(pub bool);

#[derive(Debug, Clone)]
struct ReadyEditSnapshot {
    mode: ReadyMode,
    fail_mode: dtx_config::FailMode,
    scroll_speed: f32,
    bgm_volume: f32,
    drum_volume: f32,
    audio_field: AudioField,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SongReadyState {
    pub layer: SongReadyLayer,
    pub focus: ReadyCard,
    pub mode: ReadyMode,
    pub audio_field: AudioField,
    snapshot: Option<ReadyEditSnapshot>,
    input_guarded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadyKeyboardEffect {
    None,
    AdjustValue(i32),
    AdjustDifficulty(i32),
    Launch,
    Close,
}

impl SongReadyState {
    pub fn open(&mut self, mode: ReadyMode) {
        self.layer = SongReadyLayer::Browse;
        self.focus = ReadyCard::Song;
        self.mode = mode;
        self.audio_field = AudioField::Bgm;
        self.snapshot = None;
        self.input_guarded = true;
    }

    pub fn close(&mut self) {
        self.layer = SongReadyLayer::Closed;
        self.snapshot = None;
        self.input_guarded = false;
    }

    pub fn step_card(&mut self, delta: i32) {
        let current = ReadyCard::ALL
            .iter()
            .position(|card| *card == self.focus)
            .unwrap_or(2) as i32;
        let mut next = (current + delta.signum()).clamp(0, ReadyCard::ALL.len() as i32 - 1);
        if self.mode == ReadyMode::Practice && ReadyCard::ALL[next as usize] == ReadyCard::Modifiers
        {
            next = 1;
        }
        self.focus = ReadyCard::ALL[next as usize];
    }

    pub fn begin_edit(&mut self, draft: &ReadyConfigDraft) {
        self.snapshot = Some(ReadyEditSnapshot {
            mode: self.mode,
            fail_mode: draft.config.gameplay.fail_mode(),
            scroll_speed: draft.config.gameplay.scroll_speed,
            bgm_volume: draft.config.audio.bgm_volume,
            drum_volume: draft.config.audio.drum_volume,
            audio_field: self.audio_field,
        });
        self.layer = SongReadyLayer::Edit;
    }

    pub fn apply_edit(&mut self) {
        self.snapshot = None;
        self.layer = SongReadyLayer::Browse;
    }

    pub fn cancel_edit(&mut self, draft: &mut ReadyConfigDraft) {
        if let Some(snapshot) = self.snapshot.take() {
            self.mode = snapshot.mode;
            draft.config.gameplay.set_fail_mode(snapshot.fail_mode);
            draft.config.gameplay.scroll_speed = snapshot.scroll_speed;
            draft.config.audio.bgm_volume = snapshot.bgm_volume;
            draft.config.audio.drum_volume = snapshot.drum_volume;
            self.audio_field = snapshot.audio_field;
        }
        self.layer = SongReadyLayer::Browse;
    }
}

fn reduce_ready_keyboard_browse(state: &mut SongReadyState, verb: NavVerb) -> ReadyKeyboardEffect {
    match verb {
        NavVerb::Dec => {
            state.step_card(-1);
            ReadyKeyboardEffect::None
        }
        NavVerb::Inc => {
            state.step_card(1);
            ReadyKeyboardEffect::None
        }
        NavVerb::Up if state.focus == ReadyCard::Song => ReadyKeyboardEffect::AdjustDifficulty(-1),
        NavVerb::Down if state.focus == ReadyCard::Song => ReadyKeyboardEffect::AdjustDifficulty(1),
        NavVerb::Up => ReadyKeyboardEffect::AdjustValue(-1),
        NavVerb::Down => ReadyKeyboardEffect::AdjustValue(1),
        NavVerb::Confirm if state.focus == ReadyCard::Song => ReadyKeyboardEffect::Launch,
        NavVerb::Confirm if state.focus == ReadyCard::Audio => {
            state.audio_field = match state.audio_field {
                AudioField::Bgm => AudioField::Drums,
                AudioField::Drums => AudioField::Bgm,
            };
            ReadyKeyboardEffect::None
        }
        NavVerb::Confirm => ReadyKeyboardEffect::None,
        NavVerb::Back => ReadyKeyboardEffect::Close,
        _ => ReadyKeyboardEffect::None,
    }
}

pub fn adjust_lane_speed(config: &mut dtx_config::Config, delta: i32) {
    config.gameplay.scroll_speed =
        (config.gameplay.scroll_speed + delta.signum() as f32 * 0.5).clamp(0.5, 9.0);
}

pub fn adjust_audio(config: &mut dtx_config::Config, field: AudioField, delta: i32) {
    let value = match field {
        AudioField::Bgm => &mut config.audio.bgm_volume,
        AudioField::Drums => &mut config.audio.drum_volume,
    };
    *value = (*value + delta.signum() as f32 * 0.05).clamp(0.0, 1.0);
    *value = (*value * 100.0).round() / 100.0;
}

pub fn primary_action_label(mode: ReadyMode, fail_mode: dtx_config::FailMode) -> &'static str {
    match (mode, fail_mode) {
        (ReadyMode::Practice, _) => "OPEN PRACTICE SETUP",
        (ReadyMode::Normal, dtx_config::FailMode::NoFail) => "START ASSISTED / NO FAIL",
        (ReadyMode::Normal, dtx_config::FailMode::Standard) => "START SONG",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadyLayout {
    pub widths: [f32; 5],
    pub gap: f32,
    pub edge_peek_px: f32,
    pub rows: u8,
    pub jacket_width: f32,
    pub difficulty_width: f32,
    pub metadata_min_width: f32,
    pub content_gap: f32,
}

pub fn ready_layout(viewport_width: f32) -> ReadyLayout {
    if viewport_width <= 1280.0 {
        ReadyLayout {
            widths: [142.0, 128.0, 610.0, 158.0, 190.0],
            gap: 10.0,
            edge_peek_px: 24.0,
            rows: 1,
            jacket_width: 150.0,
            difficulty_width: 180.0,
            metadata_min_width: 204.0,
            content_gap: 10.0,
        }
    } else {
        ReadyLayout {
            widths: [210.0, 180.0, 820.0, 220.0, 250.0],
            gap: 16.0,
            edge_peek_px: 0.0,
            rows: 1,
            jacket_width: 190.0,
            difficulty_width: 230.0,
            metadata_min_width: 344.0,
            content_gap: 10.0,
        }
    }
}

pub fn visible_difficulty_ordinals(count: usize, selected: usize) -> Vec<usize> {
    if count <= 5 {
        return (0..count).collect();
    }
    let start = selected.saturating_sub(2).min(count - 5);
    (start..start + 5).collect()
}

pub(crate) fn song_ready_closed(state: Res<SongReadyState>) -> bool {
    state.layer == SongReadyLayer::Closed
}

#[derive(Component)]
struct SongReadyEntity;

#[derive(Component)]
struct ReadyStrip;

#[derive(Component)]
struct ReadyCardNode(ReadyCard);

#[derive(Component)]
struct ReadyCardTitle(ReadyCard);

#[derive(Component)]
struct ReadyCardValue(ReadyCard);

#[derive(Component)]
struct ReadyAudioFieldButton(AudioField);

#[derive(Component)]
struct ReadySongTitle;

#[derive(Component)]
struct ReadySongMetadata;

#[derive(Component)]
struct ReadySongLevel;

#[derive(Component)]
struct ReadyJacket;

#[derive(Component)]
struct ReadySongContent;

#[derive(Component)]
struct ReadyDifficultyRail;

#[derive(Component)]
struct ReadyMetadataColumn;

#[derive(Component)]
struct ReadyDifficultyRow(usize);

#[derive(Component)]
struct ReadyDifficultyText;

#[derive(Component)]
struct ReadyPrimaryAction;

#[derive(Component)]
struct ReadyBackAction;

#[derive(Component)]
struct ReadyStepButton {
    card: ReadyCard,
    delta: i32,
    field: Option<AudioField>,
}

#[derive(Message, Debug, Clone, Copy)]
enum ReadyLaunch {
    Current,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SongReadyState>()
        .init_resource::<ReadyConfigDraft>()
        .init_resource::<ReadyActionCapture>()
        .add_message::<ReadyLaunch>()
        .add_systems(
            OnExit(AppState::SongSelect),
            (close_song_ready, despawn_stage::<SongReadyEntity>).chain(),
        )
        .add_systems(
            Update,
            (
                manage_song_ready_ui,
                song_ready_nav_input.after(crate::song_select::song_select_kb_emit),
                song_ready_pointer_input,
                execute_ready_launch,
                render_song_ready,
                layout_song_ready,
            )
                .chain()
                .run_if(in_state(AppState::SongSelect)),
        )
        .add_systems(Last, clear_ready_action_capture);
}

fn clear_ready_action_capture(mut capture: ResMut<ReadyActionCapture>) {
    capture.0 = false;
}

fn close_song_ready(mut state: ResMut<SongReadyState>) {
    state.close();
}

fn current_song<'a>(
    selection: &crate::song_select::Selection,
    selection_state: &crate::song_select::SongSelectSelection,
    db: &'a dtx_library::SongDb,
) -> Option<&'a dtx_library::SongInfo> {
    selection
        .chart_index(selection_state)
        .and_then(|index| db.songs.get(index))
}

fn load_jacket(asset_server: &AssetServer, path: &std::path::Path) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.format = ImageFormatSetting::Guess;
        })
        .load(path.to_string_lossy().to_string())
}

fn manage_song_ready_ui(
    mut commands: Commands,
    state: Res<SongReadyState>,
    existing: Query<Entity, With<SongReadyEntity>>,
    selection: Res<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    assets: Res<AssetServer>,
    theme: Res<ThemeResource>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    if state.layer == SongReadyLayer::Closed {
        for entity in &existing {
            commands
                .entity(entity)
                .queue_silenced(bevy::ecs::system::entity_command::despawn());
        }
        return;
    }
    if !existing.is_empty() {
        return;
    }

    let t = theme.0;
    let song = current_song(&selection, &selection_state, &db);
    commands
        .spawn((
            SongReadyEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            GlobalZIndex(15),
        ))
        .with_children(|root| {
            root.spawn((
                ReadyStrip,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::NoWrap,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    overflow: Overflow::visible(),
                    ..default()
                },
                UiTransform::default(),
                EnterChoreo::slide(Vec2::new(0.0, 90.0), 0.0, 260.0),
            ))
            .with_children(|strip| {
                spawn_option_card(strip, &t, ReadyCard::Modifiers, "MODIFIERS");
                spawn_option_card(strip, &t, ReadyCard::Mode, "MODE");
                spawn_song_card(strip, &t, song, &selection, &selection_state, &db, &assets);
                spawn_option_card(strip, &t, ReadyCard::LaneSpeed, "LANE SPEED");
                spawn_option_card(strip, &t, ReadyCard::Audio, "AUDIO");
            });
            root.spawn((
                Button,
                ReadyBackAction,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(24.0),
                    bottom: Val::Px(22.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(t.stage_panel_bg),
                BorderColor::all(t.text_primary),
                Text::new("← BACK"),
                Theme::font(15.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                TextColor(t.text_primary),
            ));
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(24.0),
                    bottom: Val::Px(24.0),
                    max_width: Val::Percent(72.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Text::new(if midi.is_some_and(|midi| midi.0) {
                    "HH/CY CARD OR VALUE  ·  BD ENTER/APPLY  ·  SD BACK  ·  FT AUDIO FIELD"
                } else {
                    "←→ SELECT  ·  ↑↓ CHANGE  ·  ENTER ACTION/AUDIO ROW  ·  ESC BACK"
                }),
                Theme::font(12.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Hint),
                TextColor(t.text_secondary),
            ));
        });
}

fn spawn_option_card(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    card: ReadyCard,
    title: &'static str,
) {
    parent
        .spawn((
            Button,
            ReadyCardNode(card),
            Node {
                height: Val::Px(270.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(t.stage_panel_bg),
            BorderColor::all(t.stage_panel_border),
            UiTransform::default(),
        ))
        .with_children(|body| {
            body.spawn((
                ReadyCardTitle(card),
                Text::new(title),
                Theme::font(14.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                TextColor(t.text_secondary),
            ));
            body.spawn((
                ReadyCardValue(card),
                Text::new(""),
                Theme::font(22.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                TextColor(t.text_primary),
                Node {
                    max_width: Val::Percent(100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ));
            if card == ReadyCard::Audio {
                body.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|fields| {
                    for (field, label) in [(AudioField::Bgm, "BGM"), (AudioField::Drums, "DRUMS")] {
                        fields.spawn((
                            Button,
                            ReadyAudioFieldButton(field),
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(30.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(t.stage_panel_bg),
                            BorderColor::all(t.stage_panel_border),
                            Text::new(label),
                            Theme::font(11.0),
                            TextColor(t.text_primary),
                        ));
                    }
                });
            }
            body.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|steps| {
                for (delta, label) in [(-1, "▲"), (1, "▼")] {
                    steps.spawn((
                        Button,
                        ReadyStepButton {
                            card,
                            delta,
                            field: None,
                        },
                        Node {
                            width: Val::Px(44.0),
                            height: Val::Px(38.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(t.stage_panel_bg),
                        BorderColor::all(t.text_secondary),
                        Text::new(label),
                        Theme::font(18.0),
                        TextColor(t.text_primary),
                    ));
                }
            });
        });
}

fn spawn_song_card(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    song: Option<&dtx_library::SongInfo>,
    selection: &crate::song_select::Selection,
    selection_state: &crate::song_select::SongSelectSelection,
    db: &dtx_library::SongDb,
    assets: &AssetServer,
) {
    parent
        .spawn((
            Button,
            ReadyCardNode(ReadyCard::Song),
            Node {
                height: Val::Px(470.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(t.stage_panel_bg),
            BorderColor::all(t.select_yellow),
            UiTransform::default(),
        ))
        .with_children(|card| {
            card.spawn((
                ReadySongLevel,
                Text::new("CHART UNAVAILABLE"),
                Theme::font(24.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                TextColor(t.select_yellow),
            ));
            card.spawn((
                ReadySongContent,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|content| {
                let jacket = song
                    .and_then(|song| song.preimage_path.as_deref())
                    .map(|path| load_jacket(assets, path))
                    .unwrap_or_default();
                content.spawn((
                    ReadyJacket,
                    Node {
                        width: Val::Px(190.0),
                        height: Val::Px(190.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(t.stage_panel_border),
                    ImageNode {
                        image: jacket,
                        ..default()
                    },
                ));
                content
                    .spawn((
                        ReadyDifficultyRail,
                        Node {
                            width: Val::Px(230.0),
                            max_height: Val::Px(238.0),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(5.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|rail| {
                        if let Some(folder) = selection_state.visible.get(selection.folder) {
                            for ordinal in (0..folder.chart_indices.len()).rev() {
                                let chart = folder
                                    .chart_indices
                                    .get(ordinal)
                                    .and_then(|index| db.songs.get(*index));
                                let label = chart
                                    .map(|chart| {
                                        crate::song_select::SongFolderView::difficulty_label_for(
                                            &chart.path,
                                            ordinal as u8,
                                        )
                                    })
                                    .unwrap_or_else(|| "?".into());
                                let level = chart
                                    .and_then(|chart| chart.dlevel)
                                    .map(dtx_core::display_dlevel)
                                    .map(|value| format!("{value:.2}"))
                                    .unwrap_or_else(|| "--".into());
                                rail.spawn((
                                    Button,
                                    ReadyDifficultyRow(ordinal),
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(40.0),
                                        align_items: AlignItems::Center,
                                        padding: UiRect::horizontal(Val::Px(8.0)),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    BackgroundColor(t.stage_panel_bg),
                                    BorderColor::all(t.stage_panel_border),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        ReadyDifficultyText,
                                        Text::new(format!("  {label:<10} {level}")),
                                        Theme::font(14.0),
                                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                                        TextColor(t.text_primary),
                                    ));
                                });
                            }
                        }
                    });
                content
                    .spawn((
                        ReadyMetadataColumn,
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(160.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|meta| {
                        meta.spawn((
                            ReadySongTitle,
                            Text::new(song.map(|s| s.title.as_str()).unwrap_or("Unavailable")),
                            Theme::font(28.0),
                            dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                            TextColor(t.text_primary),
                            Node {
                                max_width: Val::Percent(100.0),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ));
                        meta.spawn((
                            ReadySongMetadata,
                            Text::new(""),
                            Theme::font(14.0),
                            dtx_ui::SemanticText(dtx_ui::TypographyRole::Body),
                            TextColor(t.text_secondary),
                            Node {
                                max_width: Val::Percent(100.0),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ));
                    });
            });
            card.spawn((
                Button,
                ReadyPrimaryAction,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(t.select_yellow),
                BorderColor::all(t.text_primary),
                Text::new("START SONG"),
                Theme::font(20.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Heading),
                TextColor(Color::BLACK),
            ));
        });
}

fn step_ready_difficulty(
    selection: &mut crate::song_select::Selection,
    selection_state: &crate::song_select::SongSelectSelection,
    delta: i32,
) {
    let Some(folder) = selection_state.visible.get(selection.folder) else {
        return;
    };
    if folder.difficulty_count() == 0 {
        selection.difficulty = 0;
    } else if delta < 0 {
        selection.difficulty = selection.difficulty.saturating_sub(1);
    } else if delta > 0 {
        selection.difficulty =
            (selection.difficulty + 1).min((folder.difficulty_count() - 1) as u8);
    }
}

fn adjust_ready_value(state: &mut SongReadyState, draft: &mut ReadyConfigDraft, delta: i32) {
    match state.focus {
        ReadyCard::Modifiers if state.mode == ReadyMode::Normal => {
            let mode = match draft.config.gameplay.fail_mode() {
                dtx_config::FailMode::Standard => dtx_config::FailMode::NoFail,
                dtx_config::FailMode::NoFail => dtx_config::FailMode::Standard,
            };
            draft.config.gameplay.set_fail_mode(mode);
        }
        ReadyCard::Mode => {
            state.mode = match state.mode {
                ReadyMode::Normal => ReadyMode::Practice,
                ReadyMode::Practice => ReadyMode::Normal,
            };
        }
        ReadyCard::LaneSpeed => adjust_lane_speed(&mut draft.config, delta),
        ReadyCard::Audio => adjust_audio(&mut draft.config, state.audio_field, delta),
        ReadyCard::Modifiers | ReadyCard::Song => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadyPointerStepEffect {
    Persist(ReadyCard),
    Applied,
    DraftOnly,
    Ignored,
}

fn apply_ready_pointer_step(
    state: &mut SongReadyState,
    draft: &mut ReadyConfigDraft,
    card: ReadyCard,
    field: Option<AudioField>,
    delta: i32,
) -> ReadyPointerStepEffect {
    if matches!(
        state.layer,
        SongReadyLayer::Closed | SongReadyLayer::PrimaryDetail
    ) || (card == ReadyCard::Modifiers && state.mode == ReadyMode::Practice)
    {
        return ReadyPointerStepEffect::Ignored;
    }
    if state.layer == SongReadyLayer::Edit && state.focus != card {
        return ReadyPointerStepEffect::Ignored;
    }
    state.focus = card;
    if let Some(field) = field {
        state.audio_field = field;
    }
    adjust_ready_value(state, draft, delta);
    match state.layer {
        SongReadyLayer::Browse if card == ReadyCard::Mode => ReadyPointerStepEffect::Applied,
        SongReadyLayer::Browse => ReadyPointerStepEffect::Persist(card),
        SongReadyLayer::Edit => ReadyPointerStepEffect::DraftOnly,
        SongReadyLayer::Closed | SongReadyLayer::PrimaryDetail => ReadyPointerStepEffect::Ignored,
    }
}

fn select_ready_audio_field(state: &mut SongReadyState, field: AudioField) {
    state.focus = ReadyCard::Audio;
    state.audio_field = field;
}

fn merge_ready_card_config(
    card: ReadyCard,
    draft: &dtx_config::Config,
    current: &mut dtx_config::Config,
) {
    match card {
        ReadyCard::Modifiers => current.gameplay.set_fail_mode(draft.gameplay.fail_mode()),
        ReadyCard::LaneSpeed => current.gameplay.scroll_speed = draft.gameplay.scroll_speed,
        ReadyCard::Audio => {
            current.audio.bgm_volume = draft.audio.bgm_volume;
            current.audio.drum_volume = draft.audio.drum_volume;
        }
        ReadyCard::Mode | ReadyCard::Song => {}
    }
}

fn persist_ready_card_value(
    card: ReadyCard,
    draft: &mut ReadyConfigDraft,
    notifications: &mut NotificationQueue,
) -> bool {
    if matches!(card, ReadyCard::Mode | ReadyCard::Song) {
        return true;
    }
    let path = dtx_config::default_path();
    let mut current = dtx_config::load(&path);
    merge_ready_card_config(card, &draft.config, &mut current);
    match dtx_config::save(&path, &current) {
        Ok(()) => {
            draft.config = current;
            true
        }
        Err(error) => {
            notifications.push(Notification::error(format!(
                "Could not save Ready settings: {error}"
            )));
            false
        }
    }
}

fn finish_ready_edit(
    state: &mut SongReadyState,
    draft: &mut ReadyConfigDraft,
    notifications: &mut NotificationQueue,
) {
    if persist_ready_card_value(state.focus, draft, notifications) {
        state.apply_edit();
    }
}

fn song_ready_nav_input(
    mut actions: MessageReader<NavAction>,
    mut state: ResMut<SongReadyState>,
    mut capture: ResMut<ReadyActionCapture>,
    mut draft: ResMut<ReadyConfigDraft>,
    mut selection: ResMut<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    mut launches: MessageWriter<ReadyLaunch>,
    mut notifications: ResMut<NotificationQueue>,
) {
    if state.layer == SongReadyLayer::Closed {
        actions.clear();
        return;
    }
    capture.0 = true;
    if state.input_guarded {
        state.input_guarded = false;
        actions.clear();
        return;
    }

    for action in actions.read() {
        match state.layer {
            SongReadyLayer::Closed => {}
            SongReadyLayer::Browse => match action.source {
                NavSource::Keyboard => {
                    match reduce_ready_keyboard_browse(&mut state, action.verb) {
                        ReadyKeyboardEffect::None => {}
                        ReadyKeyboardEffect::AdjustValue(delta) => {
                            let card = state.focus;
                            adjust_ready_value(&mut state, &mut draft, delta);
                            if card != ReadyCard::Mode {
                                persist_ready_card_value(card, &mut draft, &mut notifications);
                            }
                        }
                        ReadyKeyboardEffect::AdjustDifficulty(delta) => {
                            step_ready_difficulty(&mut selection, &selection_state, delta);
                        }
                        ReadyKeyboardEffect::Launch => {
                            launches.write(ReadyLaunch::Current);
                        }
                        ReadyKeyboardEffect::Close => state.close(),
                    }
                }
                NavSource::Pad => match action.verb {
                    NavVerb::Up => state.step_card(-1),
                    NavVerb::Down => state.step_card(1),
                    NavVerb::Confirm if state.focus == ReadyCard::Song => {
                        state.layer = SongReadyLayer::PrimaryDetail;
                    }
                    NavVerb::Confirm => state.begin_edit(&draft),
                    NavVerb::Back => state.close(),
                    _ => {}
                },
            },
            SongReadyLayer::Edit => match action.source {
                NavSource::Keyboard => match action.verb {
                    NavVerb::Dec => adjust_ready_value(&mut state, &mut draft, -1),
                    NavVerb::Inc => adjust_ready_value(&mut state, &mut draft, 1),
                    NavVerb::Up if state.focus == ReadyCard::Audio => {
                        state.audio_field = AudioField::Bgm;
                    }
                    NavVerb::Down if state.focus == ReadyCard::Audio => {
                        state.audio_field = AudioField::Drums;
                    }
                    NavVerb::Confirm => {
                        finish_ready_edit(&mut state, &mut draft, &mut notifications)
                    }
                    NavVerb::Back => state.cancel_edit(&mut draft),
                    _ => {}
                },
                NavSource::Pad => match action.verb {
                    NavVerb::Up => adjust_ready_value(&mut state, &mut draft, -1),
                    NavVerb::Down => adjust_ready_value(&mut state, &mut draft, 1),
                    NavVerb::Practice if state.focus == ReadyCard::Audio => {
                        state.audio_field = match state.audio_field {
                            AudioField::Bgm => AudioField::Drums,
                            AudioField::Drums => AudioField::Bgm,
                        };
                    }
                    NavVerb::Confirm => {
                        finish_ready_edit(&mut state, &mut draft, &mut notifications)
                    }
                    NavVerb::Back => state.cancel_edit(&mut draft),
                    _ => {}
                },
            },
            SongReadyLayer::PrimaryDetail => match action.verb {
                NavVerb::Up => step_ready_difficulty(&mut selection, &selection_state, -1),
                NavVerb::Down => step_ready_difficulty(&mut selection, &selection_state, 1),
                NavVerb::Confirm => {
                    launches.write(ReadyLaunch::Current);
                }
                NavVerb::Back => state.layer = SongReadyLayer::Browse,
                _ => {}
            },
        }
    }
}

fn song_ready_pointer_input(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    cards: Query<(&Interaction, &ReadyCardNode), Changed<Interaction>>,
    card_hover: Query<(&Interaction, &ReadyCardNode)>,
    steps: Query<(&Interaction, &ReadyStepButton), Changed<Interaction>>,
    audio_fields: Query<(&Interaction, &ReadyAudioFieldButton), Changed<Interaction>>,
    difficulties: Query<(&Interaction, &ReadyDifficultyRow), Changed<Interaction>>,
    primary: Query<&Interaction, (With<ReadyPrimaryAction>, Changed<Interaction>)>,
    back: Query<&Interaction, (With<ReadyBackAction>, Changed<Interaction>)>,
    mut state: ResMut<SongReadyState>,
    mut draft: ResMut<ReadyConfigDraft>,
    mut selection: ResMut<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    mut launches: MessageWriter<ReadyLaunch>,
    mut notifications: ResMut<NotificationQueue>,
) {
    if state.layer == SongReadyLayer::Closed {
        return;
    }
    for (interaction, card) in &cards {
        if *interaction != Interaction::Pressed || state.layer != SongReadyLayer::Browse {
            continue;
        }
        if state.focus != card.0 {
            state.focus = card.0;
        }
    }
    for (interaction, field) in &audio_fields {
        if *interaction == Interaction::Pressed && state.layer == SongReadyLayer::Browse {
            select_ready_audio_field(&mut state, field.0);
        }
    }
    for (interaction, step) in &steps {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let effect =
            apply_ready_pointer_step(&mut state, &mut draft, step.card, step.field, step.delta);
        if let ReadyPointerStepEffect::Persist(card) = effect {
            persist_ready_card_value(card, &mut draft, &mut notifications);
        }
    }
    for (interaction, row) in &difficulties {
        if *interaction == Interaction::Pressed
            && matches!(
                state.layer,
                SongReadyLayer::Browse | SongReadyLayer::PrimaryDetail
            )
        {
            selection.difficulty = row.0 as u8;
            state.focus = ReadyCard::Song;
        }
    }
    let song_hovered = card_hover.iter().any(|(interaction, card)| {
        *interaction == Interaction::Hovered && card.0 == ReadyCard::Song
    });
    for event in wheel.read() {
        if song_hovered {
            let delta = if event.y > 0.0 {
                -1
            } else if event.y < 0.0 {
                1
            } else {
                0
            };
            step_ready_difficulty(&mut selection, &selection_state, delta);
            state.focus = ReadyCard::Song;
        }
    }
    if matches!(
        state.layer,
        SongReadyLayer::Browse | SongReadyLayer::PrimaryDetail
    ) && primary
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        launches.write(ReadyLaunch::Current);
    }
    if back
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        match state.layer {
            SongReadyLayer::Edit => state.cancel_edit(&mut draft),
            SongReadyLayer::PrimaryDetail => state.layer = SongReadyLayer::Browse,
            SongReadyLayer::Browse => state.close(),
            SongReadyLayer::Closed => {}
        }
    }
}

fn execute_ready_launch(
    mut launches: MessageReader<ReadyLaunch>,
    state: Res<SongReadyState>,
    selection: Res<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    mut selected: ResMut<crate::song_select::SelectedSong>,
    mut practice: ResMut<PracticeIntent>,
    mut requests: MessageWriter<TransitionRequest>,
    mut notifications: ResMut<NotificationQueue>,
) {
    for _ in launches.read() {
        let Some(song) = current_song(&selection, &selection_state, &db) else {
            notifications.push(Notification::warning("Selected chart is unavailable"));
            continue;
        };
        match state.mode {
            ReadyMode::Normal => {
                selected.0 = Some(song.path.clone());
                *practice = PracticeIntent::None;
                request_transition(&mut requests, AppState::SongLoading);
            }
            ReadyMode::Practice => request_practice_setup_from_song_ready(
                song,
                &mut selected,
                &mut practice,
                &mut requests,
            ),
        }
    }
}

/// Isolated handoff seam for the parallel Practice overhaul.
///
/// Until that branch exposes a stable setup event/resource, preserve the
/// existing fallback: mark a manual practice intent and enter SongLoading.
fn request_practice_setup_from_song_ready(
    song: &dtx_library::SongInfo,
    selected: &mut crate::song_select::SelectedSong,
    practice: &mut PracticeIntent,
    requests: &mut MessageWriter<TransitionRequest>,
) {
    selected.0 = Some(song.path.clone());
    *practice = PracticeIntent::Manual;
    request_transition(requests, AppState::SongLoading);
}

fn record_summary(song: &dtx_library::SongInfo) -> String {
    let path = dtx_scoring::score_ini::score_ini_path(&song.path);
    let Some(best) = dtx_scoring::score_ini::read_best(path) else {
        return "NO RECORD".into();
    };
    let status = if best.total_chips > 0 && best.max_combo >= best.total_chips {
        "FULL COMBO"
    } else if best.clear_count > 0 {
        "CLEAR"
    } else {
        "NO CLEAR"
    };
    format!(
        "BEST {}  {}  {:.2}%\n{}",
        best.score,
        best.rank,
        best.achievement_pct(),
        status
    )
}

fn render_song_ready(
    state: Res<SongReadyState>,
    draft: Res<ReadyConfigDraft>,
    selection: Res<crate::song_select::Selection>,
    selection_state: Res<crate::song_select::SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    theme: Res<ThemeResource>,
    policy: Res<dtx_ui::AccessibilityPolicy>,
    mut cards: Query<
        (
            &ReadyCardNode,
            &mut Node,
            &mut BorderColor,
            &mut BackgroundColor,
            &mut UiTransform,
        ),
        Without<ReadyDifficultyRow>,
    >,
    mut texts: ParamSet<(
        Query<(&ReadyCardTitle, &mut Text, &mut TextColor)>,
        Query<(&ReadyCardValue, &mut Text, &mut TextColor)>,
        Query<&mut Text, With<ReadySongLevel>>,
        Query<&mut Text, With<ReadySongTitle>>,
        Query<&mut Text, With<ReadySongMetadata>>,
        Query<&mut Text, With<ReadyDifficultyText>>,
        Query<
            (&mut Text, &mut BackgroundColor),
            (
                With<ReadyPrimaryAction>,
                Without<ReadyCardNode>,
                Without<ReadyDifficultyRow>,
            ),
        >,
    )>,
    mut rows: Query<
        (
            &ReadyDifficultyRow,
            &mut Node,
            &mut BorderColor,
            &mut BackgroundColor,
            &Children,
        ),
        Without<ReadyCardNode>,
    >,
    mut audio_fields: Query<
        (
            &ReadyAudioFieldButton,
            &mut Node,
            &mut BorderColor,
            &mut BackgroundColor,
        ),
        (
            Without<ReadyCardNode>,
            Without<ReadyDifficultyRow>,
            Without<ReadyPrimaryAction>,
        ),
    >,
    mut record_cache: Local<Option<(std::path::PathBuf, String)>>,
) {
    if state.layer == SongReadyLayer::Closed {
        return;
    }
    let t = theme.0;
    let reduced = policy.motion_decision() == dtx_ui::MotionDecision::Reduced;
    for (card, mut node, mut border, mut bg, mut transform) in &mut cards {
        let disabled = card.0 == ReadyCard::Modifiers && state.mode == ReadyMode::Practice;
        let focused = state.focus == card.0;
        node.border = UiRect::all(Val::Px(if focused { 4.0 } else { 2.0 }));
        border.set_all(if focused {
            t.select_yellow
        } else {
            t.stage_panel_border
        });
        bg.0 = if disabled {
            t.stage_panel_bg.with_alpha(0.46)
        } else if focused {
            t.selection_highlight
        } else {
            t.stage_panel_bg
        };
        transform.scale = if focused && !reduced {
            Vec2::splat(1.02)
        } else {
            Vec2::ONE
        };
    }
    for (title, mut text, mut color) in &mut texts.p0() {
        let disabled = title.0 == ReadyCard::Modifiers && state.mode == ReadyMode::Practice;
        let label = match title.0 {
            ReadyCard::Modifiers => "MODIFIERS",
            ReadyCard::Mode => "MODE",
            ReadyCard::Song => "SONG",
            ReadyCard::LaneSpeed => "LANE SPEED",
            ReadyCard::Audio => "AUDIO",
        };
        text.0 = format!("{} {label}", if state.focus == title.0 { "▶" } else { " " });
        color.0 = if disabled {
            t.text_secondary.with_alpha(0.45)
        } else {
            t.text_secondary
        };
    }
    for (value, mut text, mut color) in &mut texts.p1() {
        let (content, disabled) = match value.0 {
            ReadyCard::Modifiers if state.mode == ReadyMode::Practice => {
                ("NORMAL MODE ONLY".to_string(), true)
            }
            ReadyCard::Modifiers => (
                match draft.config.gameplay.fail_mode() {
                    dtx_config::FailMode::Standard => "NONE",
                    dtx_config::FailMode::NoFail => "NO FAIL",
                }
                .to_string(),
                false,
            ),
            ReadyCard::Mode => (
                match state.mode {
                    ReadyMode::Normal => "NORMAL",
                    ReadyMode::Practice => "PRACTICE",
                }
                .to_string(),
                false,
            ),
            ReadyCard::LaneSpeed => (format!("{:.1}x", draft.config.gameplay.scroll_speed), false),
            ReadyCard::Audio => (
                format!(
                    "BGM {:>3}%\nDRUMS {:>3}%",
                    (draft.config.audio.bgm_volume * 100.0).round() as i32,
                    (draft.config.audio.drum_volume * 100.0).round() as i32
                ),
                false,
            ),
            ReadyCard::Song => (String::new(), false),
        };
        text.0 = content;
        color.0 = if disabled {
            t.text_secondary.with_alpha(0.45)
        } else {
            t.text_primary
        };
    }
    for (field, mut node, mut border, mut bg) in &mut audio_fields {
        let selected = state.focus == ReadyCard::Audio && state.audio_field == field.0;
        node.border = UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 }));
        border.set_all(if selected {
            t.select_yellow
        } else {
            t.stage_panel_border
        });
        bg.0 = if selected {
            t.selection_highlight
        } else {
            t.stage_panel_bg
        };
    }

    let song = current_song(&selection, &selection_state, &db);
    let cached_record = song.map(|song| {
        if record_cache
            .as_ref()
            .is_none_or(|(path, _)| path != &song.path)
        {
            *record_cache = Some((song.path.clone(), record_summary(song)));
        }
        record_cache
            .as_ref()
            .map(|(_, summary)| summary.clone())
            .unwrap_or_else(|| "NO RECORD".into())
    });
    let label = song
        .map(|song| {
            crate::song_select::SongFolderView::difficulty_label_for(
                &song.path,
                selection.difficulty,
            )
        })
        .unwrap_or_else(|| "CHART UNAVAILABLE".into());
    let level = song
        .and_then(|song| song.dlevel)
        .map(dtx_core::display_dlevel)
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "--".into());
    for mut text in &mut texts.p2() {
        text.0 = format!("{label}  {level}");
    }
    for mut text in &mut texts.p3() {
        text.0 = song
            .map(|song| song.title.clone())
            .unwrap_or_else(|| "Unavailable".into());
    }
    for mut text in &mut texts.p4() {
        text.0 = song
            .map(|song| {
                format!(
                    "{}\nBPM {}\n{}",
                    song.artist,
                    song.bpm
                        .map(|bpm| format!("{bpm:.0}"))
                        .unwrap_or_else(|| "--".into()),
                    cached_record.as_deref().unwrap_or("NO RECORD")
                )
            })
            .unwrap_or_else(|| "The selected chart is no longer available".into());
    }
    let count = selection_state
        .visible
        .get(selection.folder)
        .map(|folder| folder.difficulty_count())
        .unwrap_or(0);
    let visible = visible_difficulty_ordinals(count, selection.difficulty as usize);
    for (row, mut node, mut border, mut bg, children) in &mut rows {
        node.display = if visible.contains(&row.0) {
            Display::Flex
        } else {
            Display::None
        };
        let selected = row.0 == selection.difficulty as usize;
        border.set_all(if selected {
            t.select_yellow
        } else {
            t.stage_panel_border
        });
        bg.0 = if selected {
            t.selection_highlight
        } else {
            t.stage_panel_bg
        };
        for child in children.iter() {
            if let Ok(mut text) = texts.p5().get_mut(child) {
                let raw = text.0.trim_start_matches([' ', '▶']).to_string();
                text.0 = format!("{} {raw}", if selected { "▶" } else { " " });
            }
        }
    }
    for (mut text, mut bg) in &mut texts.p6() {
        if song.is_some() {
            text.0 = primary_action_label(state.mode, draft.config.gameplay.fail_mode()).into();
            bg.0 = t.select_yellow;
        } else {
            text.0 = "CHART UNAVAILABLE".into();
            bg.0 = t.stage_panel_border;
        }
    }
}

fn layout_song_ready(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<ReadyStrip>>,
        Query<(&ReadyCardNode, &mut Node)>,
        Query<&mut Node, With<ReadySongContent>>,
        Query<&mut Node, With<ReadyJacket>>,
        Query<&mut Node, With<ReadyDifficultyRail>>,
        Query<&mut Node, With<ReadyMetadataColumn>>,
    )>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let layout = ready_layout(window.width());
    for mut strip in &mut nodes.p0() {
        strip.column_gap = Val::Px(layout.gap);
    }
    for (card, mut node) in &mut nodes.p1() {
        let index = ReadyCard::ALL
            .iter()
            .position(|candidate| *candidate == card.0)
            .unwrap_or(2);
        node.width = Val::Px(layout.widths[index]);
        node.flex_shrink = 0.0;
    }
    for mut content in &mut nodes.p2() {
        content.column_gap = Val::Px(layout.content_gap);
    }
    for mut jacket in &mut nodes.p3() {
        jacket.width = Val::Px(layout.jacket_width);
        jacket.height = Val::Px(layout.jacket_width);
        jacket.flex_shrink = 0.0;
    }
    for mut difficulty in &mut nodes.p4() {
        difficulty.width = Val::Px(layout.difficulty_width);
        difficulty.flex_shrink = 0.0;
    }
    for mut metadata in &mut nodes.p5() {
        metadata.min_width = Val::Px(layout.metadata_min_width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_opens_in_browse_on_the_central_card() {
        let mut state = SongReadyState::default();
        assert_eq!(state.layer, SongReadyLayer::Closed);
        state.open(ReadyMode::Normal);
        assert_eq!(state.layer, SongReadyLayer::Browse);
        assert_eq!(state.focus, ReadyCard::Song);
        assert_eq!(state.mode, ReadyMode::Normal);
    }

    #[test]
    fn keyboard_browse_uses_horizontal_focus_and_vertical_change_effects() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);

        assert_eq!(
            reduce_ready_keyboard_browse(&mut state, NavVerb::Dec),
            ReadyKeyboardEffect::None
        );
        assert_eq!(state.focus, ReadyCard::Mode);
        assert_eq!(
            reduce_ready_keyboard_browse(&mut state, NavVerb::Up),
            ReadyKeyboardEffect::AdjustValue(-1)
        );
        assert_eq!(state.layer, SongReadyLayer::Browse);
    }

    #[test]
    fn keyboard_audio_confirm_toggles_field_without_entering_edit() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);
        state.focus = ReadyCard::Audio;
        state.audio_field = AudioField::Bgm;

        assert_eq!(
            reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
            ReadyKeyboardEffect::None
        );
        assert_eq!(state.audio_field, AudioField::Drums);
        assert_eq!(state.layer, SongReadyLayer::Browse);
    }

    #[test]
    fn keyboard_song_confirm_launches_and_other_cards_do_not_enter_edit() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);
        state.focus = ReadyCard::LaneSpeed;
        assert_eq!(
            reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
            ReadyKeyboardEffect::None
        );
        assert_eq!(state.layer, SongReadyLayer::Browse);

        state.focus = ReadyCard::Song;
        assert_eq!(
            reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
            ReadyKeyboardEffect::Launch
        );
    }

    #[test]
    fn pointer_step_applies_immediately_in_browse_but_not_primary_detail() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);
        let mut draft = ReadyConfigDraft::default();
        draft.config.gameplay.scroll_speed = 5.5;

        assert_eq!(
            apply_ready_pointer_step(&mut state, &mut draft, ReadyCard::LaneSpeed, None, 1,),
            ReadyPointerStepEffect::Persist(ReadyCard::LaneSpeed)
        );
        assert_eq!(draft.config.gameplay.scroll_speed, 6.0);
        assert_eq!(state.layer, SongReadyLayer::Browse);

        state.layer = SongReadyLayer::PrimaryDetail;
        assert_eq!(
            apply_ready_pointer_step(&mut state, &mut draft, ReadyCard::LaneSpeed, None, 1,),
            ReadyPointerStepEffect::Ignored
        );
        assert_eq!(draft.config.gameplay.scroll_speed, 6.0);
    }

    #[test]
    fn pointer_audio_row_selects_field_without_entering_edit() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);

        select_ready_audio_field(&mut state, AudioField::Drums);

        assert_eq!(state.focus, ReadyCard::Audio);
        assert_eq!(state.audio_field, AudioField::Drums);
        assert_eq!(state.layer, SongReadyLayer::Browse);
    }

    #[test]
    fn practice_card_navigation_skips_disabled_modifiers() {
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Practice);
        state.focus = ReadyCard::Mode;
        state.step_card(-1);
        assert_eq!(state.focus, ReadyCard::Mode);
        state.step_card(1);
        assert_eq!(state.focus, ReadyCard::Song);
    }

    #[test]
    fn option_edit_cancel_restores_snapshot() {
        let mut draft = ReadyConfigDraft::default();
        draft.config.gameplay.scroll_speed = 6.0;
        let mut state = SongReadyState::default();
        state.open(ReadyMode::Normal);
        state.focus = ReadyCard::LaneSpeed;
        state.begin_edit(&draft);
        adjust_lane_speed(&mut draft.config, 1);
        assert_eq!(draft.config.gameplay.scroll_speed, 6.5);
        state.cancel_edit(&mut draft);
        assert_eq!(state.layer, SongReadyLayer::Browse);
        assert_eq!(draft.config.gameplay.scroll_speed, 6.0);
    }

    #[test]
    fn applying_one_card_preserves_newer_selection_config() {
        let mut draft = dtx_config::Config::default();
        draft.gameplay.scroll_speed = 7.5;
        draft.gameplay.last_selected_difficulty = 1;
        let mut current = dtx_config::Config::default();
        current.gameplay.scroll_speed = 3.0;
        current.gameplay.last_selected_difficulty = 4;

        merge_ready_card_config(ReadyCard::LaneSpeed, &draft, &mut current);

        assert_eq!(current.gameplay.scroll_speed, 7.5);
        assert_eq!(current.gameplay.last_selected_difficulty, 4);
    }

    #[test]
    fn ready_value_adjustments_use_required_steps_and_clamps() {
        let mut cfg = dtx_config::Config::default();
        cfg.gameplay.scroll_speed = 9.0;
        adjust_lane_speed(&mut cfg, 1);
        assert_eq!(cfg.gameplay.scroll_speed, 9.0);
        cfg.gameplay.scroll_speed = 0.5;
        adjust_lane_speed(&mut cfg, -1);
        assert_eq!(cfg.gameplay.scroll_speed, 0.5);

        cfg.audio.bgm_volume = 0.98;
        adjust_audio(&mut cfg, AudioField::Bgm, 1);
        assert_eq!(cfg.audio.bgm_volume, 1.0);
        cfg.audio.drum_volume = 0.02;
        adjust_audio(&mut cfg, AudioField::Drums, -1);
        assert_eq!(cfg.audio.drum_volume, 0.0);
    }

    #[test]
    fn primary_label_explains_mode_and_assist() {
        assert_eq!(
            primary_action_label(ReadyMode::Normal, dtx_config::FailMode::Standard),
            "START SONG"
        );
        assert_eq!(
            primary_action_label(ReadyMode::Normal, dtx_config::FailMode::NoFail),
            "START ASSISTED / NO FAIL"
        );
        assert_eq!(
            primary_action_label(ReadyMode::Practice, dtx_config::FailMode::NoFail),
            "OPEN PRACTICE SETUP"
        );
    }

    #[test]
    fn adaptive_layout_keeps_five_cards_on_one_line_and_center_largest() {
        for width in [1280.0, 1920.0] {
            let layout = ready_layout(width);
            assert_eq!(layout.widths.len(), 5);
            assert!(layout.widths[2] > layout.widths[0]);
            assert!(layout.widths[2] > layout.widths[4]);
            assert_eq!(layout.rows, 1);
            let central_inner = layout.widths[2] - 36.0;
            let occupied = layout.jacket_width
                + layout.content_gap * 2.0
                + layout.difficulty_width
                + layout.metadata_min_width;
            assert!(occupied <= central_inner);
        }
        assert!(ready_layout(1280.0).edge_peek_px > 0.0);
        assert_eq!(ready_layout(1920.0).edge_peek_px, 0.0);
    }

    #[test]
    fn long_difficulty_lists_show_selected_and_adjacent_rows_without_assuming_names() {
        assert_eq!(visible_difficulty_ordinals(4, 2), vec![0, 1, 2, 3]);
        assert_eq!(visible_difficulty_ordinals(8, 0), vec![0, 1, 2, 3, 4]);
        assert_eq!(visible_difficulty_ordinals(8, 4), vec![2, 3, 4, 5, 6]);
        assert_eq!(visible_difficulty_ordinals(8, 7), vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn ready_plugin_spawns_exactly_five_cards_without_system_conflicts() {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::state::app::StatesPlugin,
        ));
        app.init_state::<AppState>()
            .add_message::<NavAction>()
            .add_message::<TransitionRequest>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .init_resource::<dtx_library::SongDb>()
            .init_resource::<crate::song_select::Selection>()
            .init_resource::<crate::song_select::SongSelectSelection>()
            .init_resource::<crate::song_select::SelectedSong>()
            .init_resource::<PracticeIntent>()
            .init_resource::<NotificationQueue>()
            .init_resource::<ThemeResource>()
            .init_resource::<dtx_ui::AccessibilityPolicy>();
        plugin(&mut app);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::SongSelect);
        app.update();
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<SongReadyState>()
                .open(ReadyMode::Normal);
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&ReadyCardNode>()
                    .iter(app.world())
                    .count(),
                5
            );

            app.world_mut().resource_mut::<SongReadyState>().close();
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&SongReadyEntity>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }
}
