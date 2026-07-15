use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::practice::{PracticeDraft, PracticeFlow, PracticePhase, PracticeSession};

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PracticeTab {
    #[default]
    Setup,
    Progress,
    Preview,
}

impl PracticeTab {
    const ALL: [Self; 3] = [Self::Setup, Self::Progress, Self::Preview];

    const fn label(self) -> &'static str {
        match self {
            Self::Setup => "Setup",
            Self::Progress => "Progress",
            Self::Preview => "Preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeLayoutMode {
    Split,
    Tabbed,
}

const SETTINGS_REF_MIN: f32 = 400.0;
const PREVIEW_REF_MIN: f32 = 520.0;
const TAB_CHROME_MIN_HEIGHT: f32 = 48.0;
const TIMELINE_MIN_HEIGHT: f32 = 88.0;
const WRAPPED_TIMELINE_GROWTH: f32 = 24.0;
pub(super) const TIMELINE_HORIZONTAL_PADDING: f32 = 16.0;
pub(super) const TRANSPORT_BUTTON_MIN_WIDTH: f32 = 72.0;
pub(super) const TRANSPORT_TIME_MIN_WIDTH: f32 = 72.0;
pub(super) const TRANSPORT_CONTROL_GAP: f32 = 16.0;
pub(super) const TIMELINE_STRIP_MIN_WIDTH: f32 = 220.0;
const TRANSPORT_BUTTON_COUNT: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeTransportRowMode {
    Single,
    Stacked,
}

pub fn practice_transport_row_mode(width: f32) -> PracticeTransportRowMode {
    if width >= transport_single_row_min_width() {
        PracticeTransportRowMode::Single
    } else {
        PracticeTransportRowMode::Stacked
    }
}

pub fn transport_single_row_min_width() -> f32 {
    let controls = TRANSPORT_BUTTON_COUNT * TRANSPORT_BUTTON_MIN_WIDTH + TRANSPORT_TIME_MIN_WIDTH;
    let gaps = (TRANSPORT_BUTTON_COUNT + 1.0) * TRANSPORT_CONTROL_GAP;
    2.0 * TIMELINE_HORIZONTAL_PADDING + controls + gaps + TIMELINE_STRIP_MIN_WIDTH
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PracticeChromeGeometry {
    tab_height: f32,
    timeline_height: f32,
    transport_rows: PracticeTransportRowMode,
}

impl PracticeChromeGeometry {
    fn resolve(width: f32, text_multiplier: f32) -> Self {
        let scale_growth = (text_multiplier - 1.0).max(0.0);
        let tab_height = TAB_CHROME_MIN_HEIGHT
            + dtx_ui::Typography.base_px(dtx_ui::TypographyRole::Heading) * scale_growth;
        let transport_rows = practice_transport_row_mode(width);
        let timeline_height = TIMELINE_MIN_HEIGHT
            + dtx_ui::Typography.base_px(dtx_ui::TypographyRole::Label) * scale_growth
            + if transport_rows == PracticeTransportRowMode::Stacked {
                WRAPPED_TIMELINE_GROWTH
            } else {
                0.0
            };
        Self {
            tab_height,
            timeline_height,
            transport_rows,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct PracticePreviewGeometry(pub Option<crate::stage_rect::StageRect>);

fn required_pane_widths(width: f32, height: f32, text_multiplier: f32) -> (f32, f32) {
    let scale = (width / dtx_ui::REF_WIDTH)
        .min(height / dtx_ui::REF_HEIGHT)
        .max(1.0);
    (
        SETTINGS_REF_MIN * scale * text_multiplier,
        PREVIEW_REF_MIN * scale,
    )
}

pub fn practice_layout_mode(width: f32, height: f32, text_multiplier: f32) -> PracticeLayoutMode {
    let (settings_need, preview_need) = required_pane_widths(width, height, text_multiplier);
    if width >= settings_need + preview_need {
        PracticeLayoutMode::Split
    } else {
        PracticeLayoutMode::Tabbed
    }
}

#[derive(Component)]
pub struct PracticeSetupRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PracticeSetupLayout(pub PracticeLayoutMode);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(super) struct PracticeShellGeometry(PracticeChromeGeometry);

#[derive(Component, Debug, Clone, PartialEq)]
pub(super) struct PracticeShellSignature {
    tab: PracticeTab,
    ramp_rows: bool,
    saved_rows: bool,
    prompt: super::setup_controls::PracticePresetPrompt,
}

#[derive(Component)]
pub struct PracticeSettingsPane;

#[derive(Component)]
pub struct PracticePreviewRegion;

#[derive(Component)]
pub struct PracticePrimaryAction;

#[derive(Component)]
pub(super) struct PrimaryActionButton;

#[derive(Component, Debug, Clone, Copy)]
pub struct PracticeTabButton(pub PracticeTab);

#[derive(Component)]
struct SetupContent;

#[derive(Component)]
struct ProgressContent;

#[derive(Component)]
struct PreviewContent;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum SetupValue {
    Source,
    LoopStart,
    LoopEnd,
    Tempo,
    Snap,
    Preroll,
    CountIn,
    Trainer,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
    RampPasses,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct SetupRow(crate::practice::hud::setup_controls::SetupItem);

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct SetupRowLabel(crate::practice::hud::setup_controls::SetupItem);

#[derive(Component)]
pub(super) struct SetupValueText(SetupValue);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupAdjustButton {
    pub item: super::setup_controls::SetupItem,
    pub direction: i8,
}

pub(super) fn ensure_setup_shell(
    mut commands: Commands,
    flow: Res<PracticeFlow>,
    draft: Res<PracticeDraft>,
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    windows: Query<&Window, With<PrimaryWindow>>,
    accessibility: Option<Res<dtx_ui::AccessibilityPolicy>>,
    mut tab: ResMut<PracticeTab>,
    mut selection: ResMut<super::setup_controls::SetupSelection>,
    prompt: Res<super::setup_controls::PracticePresetPrompt>,
    mut preview_geometry: ResMut<PracticePreviewGeometry>,
    roots: Query<
        (
            Entity,
            &PracticeSetupLayout,
            &PracticeShellGeometry,
            &PracticeShellSignature,
        ),
        With<PracticeSetupRoot>,
    >,
    mut panes: ParamSet<(
        Query<&mut Node, With<PracticeSettingsPane>>,
        Query<&mut Node, With<PracticePreviewRegion>>,
    )>,
) {
    let surface_open = matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing);
    if !surface_open {
        preview_geometry.0 = None;
        for (root, _, _, _) in &roots {
            commands.entity(root).despawn();
        }
        return;
    }

    let (width, height) = windows
        .single()
        .map_or((dtx_ui::REF_WIDTH, dtx_ui::REF_HEIGHT), |window| {
            (window.width(), window.height())
        });
    let text_multiplier = accessibility
        .as_deref()
        .map_or(1.0, dtx_ui::AccessibilityPolicy::text_multiplier);
    let mode = practice_layout_mode(width, height, text_multiplier);
    let chrome = PracticeChromeGeometry::resolve(width, text_multiplier);
    let (settings_width, preview_min_width) = required_pane_widths(width, height, text_multiplier);
    if mode == PracticeLayoutMode::Split && *tab == PracticeTab::Preview {
        *tab = PracticeTab::Setup;
    }
    super::setup_controls::normalize_selection(&mut selection, &draft, &prompt);
    let signature = PracticeShellSignature {
        tab: *tab,
        ramp_rows: draft.trainer_mode() == crate::practice::PracticeTrainerMode::Ramp,
        saved_rows: matches!(draft.source, crate::practice::PracticeDraftSource::Saved(_)),
        prompt: prompt.clone(),
    };
    preview_geometry.0 = preview_stage_rect(width, height, mode, settings_width, *tab, chrome);

    if let Ok((_, current, current_chrome, current_signature)) = roots.single() {
        if current.0 == mode && current_chrome.0 == chrome && *current_signature == signature {
            if mode == PracticeLayoutMode::Split {
                if let Ok(mut settings) = panes.p0().single_mut() {
                    settings.width = Val::Px(settings_width);
                }
                if let Ok(mut preview) = panes.p1().single_mut() {
                    preview.width = Val::Auto;
                    preview.min_width = Val::Px(preview_min_width);
                    preview.flex_grow = 1.0;
                }
            }
            return;
        }
    }
    for (root, _, _, _) in &roots {
        commands.entity(root).despawn();
    }

    spawn_setup_shell(
        &mut commands,
        mode,
        settings_width,
        preview_min_width,
        chrome,
        signature,
        *tab,
        &flow,
        &draft,
        &session,
        &timeline,
        &prompt,
    );
}

fn preview_stage_rect(
    width: f32,
    height: f32,
    mode: PracticeLayoutMode,
    settings_width: f32,
    tab: PracticeTab,
    chrome: PracticeChromeGeometry,
) -> Option<crate::stage_rect::StageRect> {
    if mode == PracticeLayoutMode::Tabbed && tab != PracticeTab::Preview {
        return None;
    }
    let origin_x = if mode == PracticeLayoutMode::Split {
        settings_width
    } else {
        0.0
    };
    Some(crate::stage_rect::StageRect {
        origin: Vec2::new(origin_x, chrome.tab_height),
        size: Vec2::new(
            (width - origin_x).max(0.0),
            (height - chrome.tab_height - chrome.timeline_height).max(0.0),
        ),
    })
}

fn spawn_setup_shell(
    commands: &mut Commands,
    mode: PracticeLayoutMode,
    settings_width: f32,
    preview_min_width: f32,
    chrome: PracticeChromeGeometry,
    signature: PracticeShellSignature,
    tab: PracticeTab,
    flow: &PracticeFlow,
    draft: &PracticeDraft,
    session: &PracticeSession,
    timeline: &crate::timeline::ChipTimeline,
    prompt: &super::setup_controls::PracticePresetPrompt,
) {
    let theme = dtx_ui::Theme::default();
    commands
        .spawn((
            PracticeSetupRoot,
            PracticeSetupLayout(mode),
            PracticeShellGeometry(chrome),
            signature,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            GlobalZIndex(crate::ui_z::PRACTICE_FULL_HUD),
        ))
        .with_children(|root| {
            spawn_tab_chrome(root, &theme, mode, tab, chrome.tab_height);
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            })
            .with_children(|main| {
                spawn_settings(
                    main,
                    &theme,
                    mode,
                    settings_width,
                    tab,
                    flow.phase,
                    draft,
                    session,
                    timeline,
                    prompt,
                );
                spawn_preview(main, &theme, mode, preview_min_width, tab);
            });
            super::timeline_ui::spawn_timeline(
                root,
                &theme,
                flow,
                draft,
                timeline,
                chrome.timeline_height,
                chrome.transport_rows,
            );
        });
}

fn spawn_tab_chrome(
    root: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    mode: PracticeLayoutMode,
    tab: PracticeTab,
    height: f32,
) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            min_height: Val::Px(height),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(dtx_ui::SpacingRole::Md.px()),
            row_gap: Val::Px(dtx_ui::SpacingRole::Xs.px()),
            padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(theme.stage_bg),
    ))
    .with_children(|chrome| {
        spawn_text(
            chrome,
            "PRACTICE",
            dtx_ui::TypographyRole::Heading,
            theme.text_primary,
        );
        if mode == PracticeLayoutMode::Tabbed {
            spawn_text(
                chrome,
                "PREVIEW: INPUT IS NOT JUDGED",
                dtx_ui::TypographyRole::Hint,
                theme.text_secondary,
            );
        }
        chrome
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
                ..default()
            })
            .with_children(|tabs| {
                for candidate in PracticeTab::ALL {
                    if mode == PracticeLayoutMode::Split && candidate == PracticeTab::Preview {
                        continue;
                    }
                    let selected = candidate == tab;
                    tabs.spawn((
                        PracticeTabButton(candidate),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(if selected {
                            theme.selection_highlight
                        } else {
                            Color::NONE
                        }),
                    ))
                    .with_children(|button| {
                        let label = if selected {
                            format!(
                                "{} {}",
                                dtx_ui::StateMarker::Selected.label(),
                                candidate.label()
                            )
                        } else {
                            candidate.label().to_owned()
                        };
                        spawn_text(
                            button,
                            label,
                            dtx_ui::TypographyRole::Label,
                            if selected {
                                theme.accent
                            } else {
                                theme.text_primary
                            },
                        );
                    });
                }
            });
    });
}

fn spawn_settings(
    main: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    mode: PracticeLayoutMode,
    settings_width: f32,
    tab: PracticeTab,
    phase: PracticePhase,
    draft: &PracticeDraft,
    session: &PracticeSession,
    timeline: &crate::timeline::ChipTimeline,
    prompt: &super::setup_controls::PracticePresetPrompt,
) {
    let visible = mode == PracticeLayoutMode::Split || tab != PracticeTab::Preview;
    main.spawn((
        PracticeSettingsPane,
        Node {
            width: if mode == PracticeLayoutMode::Split {
                Val::Px(settings_width)
            } else {
                Val::Percent(100.0)
            },
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(dtx_ui::SpacingRole::Md.px())),
            row_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            display: if visible {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        },
        BackgroundColor(theme.stage_bg),
        Visibility::Inherited,
    ))
    .with_children(|pane| {
        pane.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
                min_height: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .with_children(|content| match tab {
            PracticeTab::Setup | PracticeTab::Preview => {
                content.spawn(SetupContent);
                spawn_setup_content(content, theme, draft, prompt);
            }
            PracticeTab::Progress => {
                content.spawn(ProgressContent);
                super::progress::spawn_progress(content, theme, session, timeline);
            }
        });

        pane.spawn((
            SetupRow(super::setup_controls::SetupItem::StartOrContinue),
            PrimaryActionButton,
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(52.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme.accent),
        ))
        .with_children(|button| {
            let label = button
                .spawn((
                    PracticePrimaryAction,
                    Text::new(primary_action_label(phase)),
                    dtx_ui::Theme::font(16.0),
                    dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                    TextColor(theme.stage_bg),
                ))
                .id();
            button.commands().entity(label).insert(SetupRowLabel(
                super::setup_controls::SetupItem::StartOrContinue,
            ));
        });
    });
}

fn spawn_setup_content(
    content: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    draft: &PracticeDraft,
    prompt: &super::setup_controls::PracticePresetPrompt,
) {
    spawn_section_heading(content, theme, "Loop");
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::Source,
        SetupValue::Source,
        "Source",
        setup_value(SetupValue::Source, draft),
    );
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::LoopStart,
        SetupValue::LoopStart,
        "A",
        setup_value(SetupValue::LoopStart, draft),
    );
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::LoopEnd,
        SetupValue::LoopEnd,
        "B",
        setup_value(SetupValue::LoopEnd, draft),
    );

    spawn_section_heading(content, theme, "Transport");
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::Tempo,
        SetupValue::Tempo,
        "Tempo",
        setup_value(SetupValue::Tempo, draft),
    );
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::Snap,
        SetupValue::Snap,
        "Snap",
        setup_value(SetupValue::Snap, draft),
    );
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::Preroll,
        SetupValue::Preroll,
        "Pre-roll",
        setup_value(SetupValue::Preroll, draft),
    );
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::CountIn,
        SetupValue::CountIn,
        "Count-in",
        setup_value(SetupValue::CountIn, draft),
    );

    spawn_section_heading(content, theme, "Trainer");
    spawn_setting_row(
        content,
        theme,
        super::setup_controls::SetupItem::TrainerMode,
        SetupValue::Trainer,
        "Mode",
        setup_value(SetupValue::Trainer, draft),
    );
    if draft.trainer.mode == crate::practice::PracticeTrainerMode::Ramp {
        for (item, field, label) in [
            (
                super::setup_controls::SetupItem::RampStart,
                SetupValue::RampStart,
                "Start tempo",
            ),
            (
                super::setup_controls::SetupItem::RampTarget,
                SetupValue::RampTarget,
                "Target tempo",
            ),
            (
                super::setup_controls::SetupItem::RampStep,
                SetupValue::RampStep,
                "Step",
            ),
            (
                super::setup_controls::SetupItem::RampThreshold,
                SetupValue::RampThreshold,
                "Pass threshold",
            ),
            (
                super::setup_controls::SetupItem::RampPasses,
                SetupValue::RampPasses,
                "Required passes",
            ),
        ] {
            spawn_setting_row(
                content,
                theme,
                item,
                field,
                label,
                setup_value(field, draft),
            );
        }
    }

    spawn_section_heading(content, theme, "Saved presets");
    spawn_action_row(
        content,
        theme,
        super::setup_controls::SetupItem::SaveAsNew,
        "Save as New",
    );
    if matches!(draft.source, crate::practice::PracticeDraftSource::Saved(_)) {
        spawn_action_row(
            content,
            theme,
            super::setup_controls::SetupItem::UpdateSaved,
            "Update Saved Loop",
        );
        spawn_action_row(
            content,
            theme,
            super::setup_controls::SetupItem::DeleteSaved,
            "Delete Saved Loop",
        );
    }
    match prompt {
        super::setup_controls::PracticePresetPrompt::ConfirmDelete { .. } => {
            spawn_action_row(
                content,
                theme,
                super::setup_controls::SetupItem::ConfirmDelete,
                "Confirm Delete",
            );
            spawn_action_row(
                content,
                theme,
                super::setup_controls::SetupItem::CancelDelete,
                "Cancel Delete",
            );
        }
        super::setup_controls::PracticePresetPrompt::Retry { message, .. } => {
            spawn_text(
                content,
                message,
                dtx_ui::TypographyRole::Hint,
                theme.text_secondary,
            );
            spawn_action_row(
                content,
                theme,
                super::setup_controls::SetupItem::RetryPreset,
                "Retry Save",
            );
            spawn_action_row(
                content,
                theme,
                super::setup_controls::SetupItem::CancelRetry,
                "Cancel Retry",
            );
        }
        super::setup_controls::PracticePresetPrompt::None => {}
    }
}

fn spawn_preview(
    main: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    mode: PracticeLayoutMode,
    preview_min_width: f32,
    tab: PracticeTab,
) {
    let visible = mode == PracticeLayoutMode::Split || tab == PracticeTab::Preview;
    main.spawn((
        PracticePreviewRegion,
        PreviewContent,
        Node {
            width: if mode == PracticeLayoutMode::Split {
                Val::Auto
            } else {
                Val::Percent(100.0)
            },
            height: Val::Percent(100.0),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::End,
            padding: UiRect::all(Val::Px(dtx_ui::SpacingRole::Md.px())),
            min_width: if mode == PracticeLayoutMode::Split {
                Val::Px(preview_min_width)
            } else {
                Val::Px(0.0)
            },
            min_height: Val::Px(0.0),
            flex_grow: if mode == PracticeLayoutMode::Split {
                1.0
            } else {
                0.0
            },
            display: if visible {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        },
        Visibility::Inherited,
    ))
    .with_children(|preview| {
        preview.spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
            children![(
                Text::new("PREVIEW: INPUT IS NOT JUDGED"),
                dtx_ui::Theme::font(16.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                TextColor(theme.text_primary),
            )],
        ));
    });
}

fn spawn_section_heading(
    parent: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    label: impl Into<String>,
) {
    parent.spawn(Node {
        margin: UiRect::top(Val::Px(dtx_ui::SpacingRole::Md.px())),
        ..default()
    });
    spawn_text(
        parent,
        label,
        dtx_ui::TypographyRole::Heading,
        theme.text_primary,
    );
}

fn spawn_setting_row(
    parent: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    item: super::setup_controls::SetupItem,
    field: SetupValue,
    label: impl Into<String>,
    value: impl Into<String>,
) {
    parent
        .spawn((
            SetupRow(item),
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
                padding: UiRect::vertical(Val::Px(dtx_ui::SpacingRole::Xs.px())),
                ..default()
            },
        ))
        .with_children(|row| {
            let label = spawn_text(
                row,
                label,
                dtx_ui::TypographyRole::Body,
                theme.text_secondary,
            );
            row.commands().entity(label).insert(SetupRowLabel(item));
            let value = spawn_text(
                row,
                value,
                dtx_ui::TypographyRole::Label,
                theme.text_primary,
            );
            row.commands().entity(value).insert(SetupValueText(field));
            for (direction, label) in [(-1, "−"), (1, "+")] {
                row.spawn((
                    SetupAdjustButton { item, direction },
                    Button,
                    Node {
                        min_width: Val::Px(32.0),
                        min_height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|button| {
                    spawn_text(
                        button,
                        label,
                        dtx_ui::TypographyRole::Label,
                        theme.text_primary,
                    );
                });
            }
        });
}

fn spawn_action_row(
    parent: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    item: super::setup_controls::SetupItem,
    label: &'static str,
) {
    parent
        .spawn((
            SetupRow(item),
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(40.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        ))
        .with_children(|row| {
            let entity = spawn_text(
                row,
                label,
                dtx_ui::TypographyRole::Label,
                theme.text_primary,
            );
            row.commands().entity(entity).insert(SetupRowLabel(item));
        });
}

fn setup_value(field: SetupValue, draft: &PracticeDraft) -> String {
    match field {
        SetupValue::Source => format!(
            "{} {}",
            dtx_ui::StateMarker::Selected.label(),
            source_label(draft.source)
        ),
        SetupValue::LoopStart => draft.loop_region.map_or_else(
            || "Whole song".to_owned(),
            |region| super::format_chart_time(region.start_ms),
        ),
        SetupValue::LoopEnd => draft.loop_region.map_or_else(
            || "End".to_owned(),
            |region| super::format_chart_time(region.end_ms),
        ),
        SetupValue::Tempo => format!("{:.2}×", draft.user_tempo),
        SetupValue::Snap => draft.snap.label().to_owned(),
        SetupValue::Preroll => draft.preroll.label(),
        SetupValue::CountIn => if draft.count_in { "On" } else { "Off" }.to_owned(),
        SetupValue::Trainer => trainer_label(draft.trainer.mode).to_owned(),
        SetupValue::RampStart => format!("{:.2}×", draft.trainer.ramp_config.start_tempo),
        SetupValue::RampTarget => format!("{:.2}×", draft.trainer.ramp_config.target_tempo),
        SetupValue::RampStep => format!("{:.2}×", draft.trainer.ramp_config.step),
        SetupValue::RampThreshold => format!("{:.0}%", draft.trainer.ramp_config.threshold_pct),
        SetupValue::RampPasses => draft.trainer.ramp_config.required_successes.to_string(),
    }
}

pub(super) fn refresh_setup_copy(
    flow: Res<PracticeFlow>,
    draft: Res<PracticeDraft>,
    mut values: Query<(&SetupValueText, &mut Text), Without<PracticePrimaryAction>>,
    mut primary: Query<&mut Text, With<PracticePrimaryAction>>,
    selection: Res<super::setup_controls::SetupSelection>,
    store: Option<Res<crate::practice::PracticePresetStore>>,
    timeline: Res<crate::timeline::ChipTimeline>,
    mut labels: Query<
        (&SetupRowLabel, &mut Text),
        (Without<SetupValueText>, Without<PracticePrimaryAction>),
    >,
) {
    for (field, mut text) in &mut values {
        text.0 = if field.0 == SetupValue::Source {
            format!(
                "{} {}",
                dtx_ui::StateMarker::Selected.label(),
                selected_source_label(draft.source, store.as_deref(), &timeline)
            )
        } else {
            setup_value(field.0, &draft)
        };
    }
    for mut text in &mut primary {
        let label = primary_action_label(flow.phase);
        text.0 = if selection.0 == super::setup_controls::SetupItem::StartOrContinue {
            format!("› {label}")
        } else {
            label.to_owned()
        };
    }
    for (row, mut text) in &mut labels {
        let raw = text.0.trim_start_matches("› ").to_owned();
        text.0 = if row.0 == selection.0 {
            format!("› {raw}")
        } else {
            raw
        };
    }
}

fn selected_source_label(
    source: crate::practice::PracticeDraftSource,
    store: Option<&crate::practice::PracticePresetStore>,
    timeline: &crate::timeline::ChipTimeline,
) -> String {
    if let (crate::practice::PracticeDraftSource::Saved(id), Some(store)) = (source, store) {
        if let Some(preset) = store.registry.preset(id) {
            return preset.name.clone().unwrap_or_else(|| {
                crate::practice::presets::automatic_preset_label(&preset.config, timeline)
            });
        }
    }
    source_label(source).to_owned()
}

pub(super) fn setup_button_actions(
    rows: Query<(&Interaction, &SetupRow), Changed<Interaction>>,
    adjusters: Query<(&Interaction, &SetupAdjustButton), Changed<Interaction>>,
    primary: Query<&Interaction, (With<PrimaryActionButton>, Changed<Interaction>)>,
    mut actions: MessageWriter<super::setup_controls::PracticeUiAction>,
) {
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            actions.write(super::setup_controls::PracticeUiAction::SelectItem(row.0));
            if matches!(
                row.0,
                super::setup_controls::SetupItem::SaveAsNew
                    | super::setup_controls::SetupItem::UpdateSaved
                    | super::setup_controls::SetupItem::DeleteSaved
                    | super::setup_controls::SetupItem::ConfirmDelete
                    | super::setup_controls::SetupItem::CancelDelete
                    | super::setup_controls::SetupItem::RetryPreset
                    | super::setup_controls::SetupItem::CancelRetry
            ) {
                actions.write(super::setup_controls::PracticeUiAction::Confirm);
            }
        }
    }
    for (interaction, adjuster) in &adjusters {
        if *interaction == Interaction::Pressed {
            actions.write(super::setup_controls::PracticeUiAction::SelectItem(
                adjuster.item,
            ));
            actions.write(super::setup_controls::PracticeUiAction::Adjust(
                adjuster.direction,
            ));
        }
    }
    for interaction in &primary {
        if *interaction == Interaction::Pressed {
            actions.write(super::setup_controls::PracticeUiAction::StartOrContinue);
        }
    }
}

pub(super) fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    role: dtx_ui::TypographyRole,
    color: Color,
) -> Entity {
    parent
        .spawn((
            Text::new(text),
            dtx_ui::Theme::font(dtx_ui::Typography.base_px(role)),
            dtx_ui::SemanticText(role),
            TextColor(color),
        ))
        .id()
}

const fn primary_action_label(phase: PracticePhase) -> &'static str {
    match phase {
        PracticePhase::Editing => "Continue Practice",
        PracticePhase::Setup | PracticePhase::Running => "Start Practice",
    }
}

const fn source_label(source: crate::practice::PracticeDraftSource) -> &'static str {
    match source {
        crate::practice::PracticeDraftSource::WholeSong => "Whole Song",
        crate::practice::PracticeDraftSource::LastUsed => "Last Used",
        crate::practice::PracticeDraftSource::Recommended => "Recommended Section",
        crate::practice::PracticeDraftSource::Saved(_) => "Saved Preset",
        crate::practice::PracticeDraftSource::Custom => "Custom",
    }
}

const fn trainer_label(mode: crate::practice::PracticeTrainerMode) -> &'static str {
    match mode {
        crate::practice::PracticeTrainerMode::Off => "Off",
        crate::practice::PracticeTrainerMode::Wait => "Wait",
        crate::practice::PracticeTrainerMode::Ramp => "Ramp",
    }
}

pub fn update_tab_selection(
    clicks: Query<(&Interaction, &PracticeTabButton), Changed<Interaction>>,
    mut tab: ResMut<PracticeTab>,
    roots: Query<Entity, With<PracticeSetupRoot>>,
    mut commands: Commands,
) {
    for (interaction, button) in &clicks {
        if *interaction == Interaction::Pressed && *tab != button.0 {
            *tab = button.0;
            for root in &roots {
                commands.entity(root).despawn();
            }
        }
    }
}

pub(super) fn despawn_setup_shell(
    mut commands: Commands,
    roots: Query<Entity, With<PracticeSetupRoot>>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

pub(super) fn reset_tab(mut tab: ResMut<PracticeTab>) {
    *tab = PracticeTab::Setup;
}
