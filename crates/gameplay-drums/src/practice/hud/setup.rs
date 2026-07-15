use bevy::prelude::*;

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

pub fn practice_layout_mode(width: f32, height: f32, text_multiplier: f32) -> PracticeLayoutMode {
    let scale = (width / dtx_ui::REF_WIDTH).min(height / dtx_ui::REF_HEIGHT);
    let settings_need = 400.0 * scale.max(1.0) * text_multiplier;
    let preview_need = 520.0 * scale.max(1.0);
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

#[derive(Component)]
pub struct PracticeSettingsPane;

#[derive(Component)]
pub struct PracticePreviewRegion;

#[derive(Component)]
pub struct PracticePrimaryAction;

#[derive(Component)]
struct PrimaryActionButton;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PracticeTabButton(PracticeTab);

#[derive(Component)]
struct SetupContent;

#[derive(Component)]
struct ProgressContent;

#[derive(Component)]
struct PreviewContent;

pub(super) fn ensure_setup_shell(
    mut commands: Commands,
    flow: Res<PracticeFlow>,
    draft: Res<PracticeDraft>,
    session: Res<PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    layout: Option<Res<crate::layout::PlayfieldLayout>>,
    accessibility: Option<Res<dtx_ui::AccessibilityPolicy>>,
    tab: Res<PracticeTab>,
    roots: Query<(Entity, &PracticeSetupLayout), With<PracticeSetupRoot>>,
    mut primary: Query<&mut Text, With<PracticePrimaryAction>>,
) {
    let surface_open = matches!(flow.phase, PracticePhase::Setup | PracticePhase::Editing);
    if !surface_open {
        for (root, _) in &roots {
            commands.entity(root).despawn();
        }
        return;
    }

    let (width, height) = layout
        .as_deref()
        .map_or((dtx_ui::REF_WIDTH, dtx_ui::REF_HEIGHT), |layout| {
            (layout.width, layout.height)
        });
    let text_multiplier = accessibility
        .as_deref()
        .map_or(1.0, dtx_ui::AccessibilityPolicy::text_multiplier);
    let mode = practice_layout_mode(width, height, text_multiplier);

    if let Ok((root, current)) = roots.single() {
        if current.0 != mode {
            commands.entity(root).despawn();
        } else if let Ok(mut text) = primary.single_mut() {
            text.0 = primary_action_label(flow.phase).to_owned();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }

    spawn_setup_shell(
        &mut commands,
        mode,
        *tab,
        &flow,
        &draft,
        &session,
        &timeline,
    );
}

fn spawn_setup_shell(
    commands: &mut Commands,
    mode: PracticeLayoutMode,
    tab: PracticeTab,
    flow: &PracticeFlow,
    draft: &PracticeDraft,
    session: &PracticeSession,
    timeline: &crate::timeline::ChipTimeline,
) {
    let theme = dtx_ui::Theme::default();
    commands
        .spawn((
            PracticeSetupRoot,
            PracticeSetupLayout(mode),
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
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            })
            .with_children(|main| {
                spawn_settings(main, &theme, mode, tab, flow.phase, draft, session);
                spawn_preview(main, &theme, mode, tab);
            });
            super::timeline_ui::spawn_timeline(root, &theme, flow, draft, timeline);
        });
}

fn spawn_settings(
    main: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    mode: PracticeLayoutMode,
    tab: PracticeTab,
    phase: PracticePhase,
    draft: &PracticeDraft,
    session: &PracticeSession,
) {
    let visible = mode == PracticeLayoutMode::Split || tab != PracticeTab::Preview;
    main.spawn((
        PracticeSettingsPane,
        Node {
            width: if mode == PracticeLayoutMode::Split {
                Val::Percent(38.0)
            } else {
                Val::Percent(100.0)
            },
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(dtx_ui::SpacingRole::Md.px())),
            row_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            ..default()
        },
        BackgroundColor(theme.stage_bg),
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ))
    .with_children(|pane| {
        spawn_text(
            pane,
            "PRACTICE",
            dtx_ui::TypographyRole::Heading,
            theme.text_primary,
        );
        if mode == PracticeLayoutMode::Tabbed {
            spawn_text(
                pane,
                "PREVIEW: INPUT IS NOT JUDGED",
                dtx_ui::TypographyRole::Hint,
                theme.text_primary,
            );
        }
        pane.spawn(Node {
            width: Val::Percent(100.0),
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
                spawn_setup_content(content, theme, draft);
            }
            PracticeTab::Progress => {
                content.spawn(ProgressContent);
                super::progress::spawn_progress(content, theme, session);
            }
        });

        pane.spawn((
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
            children![(
                PracticePrimaryAction,
                Text::new(primary_action_label(phase)),
                dtx_ui::Theme::font(16.0),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                TextColor(theme.stage_bg),
            )],
        ));
    });
}

fn spawn_setup_content(
    content: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    draft: &PracticeDraft,
) {
    spawn_section_heading(content, theme, "Loop");
    spawn_setting_row(
        content,
        theme,
        "Source",
        format!(
            "{} {}",
            dtx_ui::StateMarker::Selected.label(),
            source_label(draft.source)
        ),
    );
    let (a, b) = draft
        .loop_region
        .map_or(("Whole song".to_owned(), "End".to_owned()), |r| {
            (
                super::format_chart_time(r.start_ms),
                super::format_chart_time(r.end_ms),
            )
        });
    spawn_setting_row(content, theme, "A", a);
    spawn_setting_row(content, theme, "B", b);

    spawn_section_heading(content, theme, "Transport");
    spawn_setting_row(content, theme, "Tempo", format!("{:.2}×", draft.user_tempo));
    spawn_setting_row(content, theme, "Snap", draft.snap.label());
    spawn_setting_row(content, theme, "Pre-roll", draft.preroll.label());
    spawn_setting_row(
        content,
        theme,
        "Count-in",
        if draft.count_in { "On" } else { "Off" },
    );

    spawn_section_heading(content, theme, "Trainer");
    spawn_setting_row(content, theme, "Mode", trainer_label(draft.trainer.mode));
}

fn spawn_preview(
    main: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    mode: PracticeLayoutMode,
    tab: PracticeTab,
) {
    let visible = mode == PracticeLayoutMode::Split || tab == PracticeTab::Preview;
    main.spawn((
        PracticePreviewRegion,
        PreviewContent,
        Node {
            width: if mode == PracticeLayoutMode::Split {
                Val::Percent(62.0)
            } else {
                Val::Percent(100.0)
            },
            height: Val::Percent(100.0),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::End,
            padding: UiRect::all(Val::Px(dtx_ui::SpacingRole::Md.px())),
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            ..default()
        },
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
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
    label: impl Into<String>,
    value: impl Into<String>,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(36.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
            padding: UiRect::vertical(Val::Px(dtx_ui::SpacingRole::Xs.px())),
            ..default()
        })
        .with_children(|row| {
            spawn_text(
                row,
                label,
                dtx_ui::TypographyRole::Body,
                theme.text_secondary,
            );
            spawn_text(
                row,
                value,
                dtx_ui::TypographyRole::Label,
                theme.text_primary,
            );
        });
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

pub(super) fn update_tab_selection(
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
