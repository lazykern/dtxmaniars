use bevy::prelude::*;

use crate::practice::session::{LoopRegion, PrerollSetting, RATE_MAX, RATE_MIN, RATE_STEP};
use crate::practice::{
    PracticeDraft, PracticeDraftSource, PracticeFlow, PracticePhase, PracticePresetStore,
    PracticeSourceCatalog, PracticeTrainerMode, PresetCommand, PreviewAction, PreviewState,
};
use crate::timeline::{ChipTimeline, SnapDivisor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupItem {
    Source,
    LoopStart,
    LoopEnd,
    Tempo,
    Snap,
    Preroll,
    CountIn,
    TrainerMode,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
    RampPasses,
    SaveAsNew,
    UpdateSaved,
    DeleteSaved,
    ConfirmDelete,
    CancelDelete,
    RetryPreset,
    CancelRetry,
    StartOrContinue,
}

const SETUP_ITEMS: [SetupItem; 21] = [
    SetupItem::Source,
    SetupItem::LoopStart,
    SetupItem::LoopEnd,
    SetupItem::Tempo,
    SetupItem::Snap,
    SetupItem::Preroll,
    SetupItem::CountIn,
    SetupItem::TrainerMode,
    SetupItem::RampStart,
    SetupItem::RampTarget,
    SetupItem::RampStep,
    SetupItem::RampThreshold,
    SetupItem::RampPasses,
    SetupItem::SaveAsNew,
    SetupItem::UpdateSaved,
    SetupItem::DeleteSaved,
    SetupItem::ConfirmDelete,
    SetupItem::CancelDelete,
    SetupItem::RetryPreset,
    SetupItem::CancelRetry,
    SetupItem::StartOrContinue,
];

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupSelection(pub SetupItem);

impl Default for SetupSelection {
    fn default() -> Self {
        Self(SetupItem::Source)
    }
}

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub enum PracticePresetPrompt {
    #[default]
    None,
    ConfirmDelete {
        id: u64,
    },
    Retry {
        message: String,
        command: Box<PresetCommand>,
    },
}

pub fn visible_setup_items(draft: &PracticeDraft, prompt: &PracticePresetPrompt) -> Vec<SetupItem> {
    SETUP_ITEMS
        .into_iter()
        .filter(|item| {
            (!matches!(
                item,
                SetupItem::RampStart
                    | SetupItem::RampTarget
                    | SetupItem::RampStep
                    | SetupItem::RampThreshold
                    | SetupItem::RampPasses
            ) || draft.trainer_mode() == PracticeTrainerMode::Ramp)
                && (!matches!(item, SetupItem::UpdateSaved | SetupItem::DeleteSaved)
                    || matches!(draft.source, PracticeDraftSource::Saved(_)))
                && (!matches!(item, SetupItem::ConfirmDelete | SetupItem::CancelDelete)
                    || matches!(prompt, PracticePresetPrompt::ConfirmDelete { .. }))
                && (!matches!(item, SetupItem::RetryPreset | SetupItem::CancelRetry)
                    || matches!(prompt, PracticePresetPrompt::Retry { .. }))
        })
        .collect()
}

pub fn normalize_selection(
    selection: &mut SetupSelection,
    draft: &PracticeDraft,
    prompt: &PracticePresetPrompt,
) {
    let visible = visible_setup_items(draft, prompt);
    if visible.contains(&selection.0) {
        return;
    }
    let old = SETUP_ITEMS
        .iter()
        .position(|item| *item == selection.0)
        .unwrap_or(0);
    selection.0 = visible
        .iter()
        .rev()
        .copied()
        .find(|item| {
            SETUP_ITEMS
                .iter()
                .position(|candidate| candidate == item)
                .is_some_and(|index| index < old)
        })
        .unwrap_or(SetupItem::Source);
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum PracticeUiAction {
    SelectSource(PracticeDraftSource),
    SelectItem(SetupItem),
    MoveSelection(i8),
    Adjust(i8),
    SetLoopStart(i64),
    SetLoopEnd(i64),
    SetLoopRegion(LoopRegion),
    SetTempo(f32),
    SetSnap(SnapDivisor),
    SetPreroll(PrerollSetting),
    SetCountIn(bool),
    SetTrainerMode(PracticeTrainerMode),
    SetRampStart(f32),
    SetRampTarget(f32),
    SetRampStep(f32),
    SetRampThreshold(f32),
    SetRampPasses(u8),
    SaveAsNew,
    UpdateSaved,
    RequestDeleteSaved,
    ConfirmDeleteSaved,
    RetryPreset,
    CancelPresetPrompt,
    StartOrContinue,
    Preview(PreviewAction),
    SelectTab(super::setup::PracticeTab),
    MoveTab(i8),
    Confirm,
    Back,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartOrContinueRequested;

pub fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    tab: Res<super::setup::PracticeTab>,
    flow: Res<PracticeFlow>,
    mut actions: MessageWriter<PracticeUiAction>,
) {
    let action = if keys.just_pressed(KeyCode::Tab) {
        Some(PracticeUiAction::MoveTab(1))
    } else if *tab == super::setup::PracticeTab::Preview && keys.just_pressed(KeyCode::ArrowLeft) {
        Some(PracticeUiAction::Preview(PreviewAction::PrevBar))
    } else if *tab == super::setup::PracticeTab::Preview && keys.just_pressed(KeyCode::ArrowRight) {
        Some(PracticeUiAction::Preview(PreviewAction::NextBar))
    } else if *tab == super::setup::PracticeTab::Preview
        && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space))
    {
        Some(PracticeUiAction::Preview(
            if flow.preview == PreviewState::Playing {
                PreviewAction::Pause
            } else {
                PreviewAction::Play
            },
        ))
    } else if *tab == super::setup::PracticeTab::Progress
        && (keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowLeft))
    {
        Some(PracticeUiAction::MoveTab(-1))
    } else if *tab == super::setup::PracticeTab::Progress
        && (keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::ArrowRight))
    {
        Some(PracticeUiAction::MoveTab(1))
    } else if *tab == super::setup::PracticeTab::Progress
        && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space))
    {
        Some(PracticeUiAction::SelectTab(
            super::setup::PracticeTab::Setup,
        ))
    } else if *tab == super::setup::PracticeTab::Preview && keys.just_pressed(KeyCode::ArrowUp) {
        Some(PracticeUiAction::MoveTab(-1))
    } else if *tab == super::setup::PracticeTab::Preview && keys.just_pressed(KeyCode::ArrowDown) {
        Some(PracticeUiAction::MoveTab(1))
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        Some(PracticeUiAction::MoveSelection(-1))
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        Some(PracticeUiAction::MoveSelection(1))
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some(PracticeUiAction::Adjust(-1))
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        Some(PracticeUiAction::Adjust(1))
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        Some(PracticeUiAction::Confirm)
    } else if keys.just_pressed(KeyCode::Escape) {
        Some(PracticeUiAction::Back)
    } else {
        None
    };
    if let Some(action) = action {
        actions.write(action);
    }
}

pub fn nav_actions(
    mut nav: MessageReader<game_shell::NavAction>,
    tab: Res<super::setup::PracticeTab>,
    flow: Res<PracticeFlow>,
    mut actions: MessageWriter<PracticeUiAction>,
) {
    for action in nav.read() {
        let action = match (*tab, action.verb) {
            (_, game_shell::NavVerb::Back) => PracticeUiAction::Back,
            (_, game_shell::NavVerb::Practice) => PracticeUiAction::MoveTab(1),
            (super::setup::PracticeTab::Setup, game_shell::NavVerb::Up) => {
                PracticeUiAction::MoveSelection(-1)
            }
            (super::setup::PracticeTab::Setup, game_shell::NavVerb::Down) => {
                PracticeUiAction::MoveSelection(1)
            }
            (super::setup::PracticeTab::Setup, game_shell::NavVerb::Dec) => {
                PracticeUiAction::Adjust(-1)
            }
            (super::setup::PracticeTab::Setup, game_shell::NavVerb::Inc) => {
                PracticeUiAction::Adjust(1)
            }
            (super::setup::PracticeTab::Setup, game_shell::NavVerb::Confirm) => {
                PracticeUiAction::Confirm
            }
            (
                super::setup::PracticeTab::Progress,
                game_shell::NavVerb::Up | game_shell::NavVerb::Dec,
            )
            | (super::setup::PracticeTab::Preview, game_shell::NavVerb::Up) => {
                PracticeUiAction::MoveTab(-1)
            }
            (
                super::setup::PracticeTab::Progress,
                game_shell::NavVerb::Down | game_shell::NavVerb::Inc,
            )
            | (super::setup::PracticeTab::Preview, game_shell::NavVerb::Down) => {
                PracticeUiAction::MoveTab(1)
            }
            (super::setup::PracticeTab::Progress, game_shell::NavVerb::Confirm) => {
                PracticeUiAction::SelectTab(super::setup::PracticeTab::Setup)
            }
            (super::setup::PracticeTab::Preview, game_shell::NavVerb::Dec) => {
                PracticeUiAction::Preview(PreviewAction::PrevBar)
            }
            (super::setup::PracticeTab::Preview, game_shell::NavVerb::Inc) => {
                PracticeUiAction::Preview(PreviewAction::NextBar)
            }
            (super::setup::PracticeTab::Preview, game_shell::NavVerb::Confirm) => {
                PracticeUiAction::Preview(if flow.preview == PreviewState::Playing {
                    PreviewAction::Pause
                } else {
                    PreviewAction::Play
                })
            }
        };
        actions.write(action);
    }
}

pub fn apply_ui_actions(
    mut actions: MessageReader<PracticeUiAction>,
    mut selection: ResMut<SetupSelection>,
    mut tab: ResMut<super::setup::PracticeTab>,
    mut draft: ResMut<PracticeDraft>,
    mut flow: ResMut<PracticeFlow>,
    mut prompt: ResMut<PracticePresetPrompt>,
    timeline: Res<ChipTimeline>,
    store: Option<Res<PracticePresetStore>>,
    catalog: Option<Res<PracticeSourceCatalog>>,
    mut preset_commands: MessageWriter<PresetCommand>,
    mut previews: MessageWriter<PreviewAction>,
    mut starts: MessageWriter<StartOrContinueRequested>,
    mut cancels: MessageWriter<crate::practice::CancelPracticeSettings>,
    mut initial_cancels: MessageWriter<crate::practice::InitialSetupCancelRequested>,
    mut toasts: ResMut<crate::practice::toast::ToastQueue>,
) {
    for action in actions.read().copied() {
        match action {
            PracticeUiAction::SelectSource(source) => {
                if let Some(next) =
                    selected_source_draft(&draft, store.as_deref(), catalog.as_deref(), source)
                {
                    let validated = next
                        .validate(&timeline)
                        .expect("draft validation is infallible");
                    if let Some(warning) = &validated.warning {
                        toasts.push(warning.clone());
                    }
                    *draft = validated.draft;
                    flow.preview = PreviewState::Stopped;
                    previews.write(PreviewAction::Pause);
                    previews.write(PreviewAction::Seek(
                        draft.loop_region.map_or(0, |region| region.start_ms),
                    ));
                }
            }
            PracticeUiAction::SelectItem(item) => selection.0 = item,
            PracticeUiAction::MoveSelection(direction) => {
                let visible = visible_setup_items(&draft, &prompt);
                let current = visible
                    .iter()
                    .position(|item| *item == selection.0)
                    .unwrap_or(0);
                let next = if direction > 0 {
                    (current + 1) % visible.len()
                } else {
                    (current + visible.len() - 1) % visible.len()
                };
                selection.0 = visible[next];
            }
            PracticeUiAction::Adjust(direction) => {
                if selection.0 == SetupItem::Source {
                    if let Some(store) = store.as_deref() {
                        let sources = crate::practice::presets::ordered_sources(
                            store,
                            catalog.as_deref(),
                            &timeline,
                        );
                        let current = sources
                            .iter()
                            .position(|(source, _)| *source == draft.source)
                            .unwrap_or(0);
                        let next = if direction > 0 {
                            (current + 1) % sources.len()
                        } else {
                            (current + sources.len() - 1) % sources.len()
                        };
                        let source = sources[next].0;
                        if let Some(next) =
                            selected_source_draft(&draft, Some(store), catalog.as_deref(), source)
                        {
                            let validated = next
                                .validate(&timeline)
                                .expect("draft validation is infallible");
                            if let Some(warning) = &validated.warning {
                                toasts.push(warning.clone());
                            }
                            *draft = validated.draft;
                            flow.preview = PreviewState::Stopped;
                            previews.write(PreviewAction::Pause);
                            previews.write(PreviewAction::Seek(
                                draft.loop_region.map_or(0, |region| region.start_ms),
                            ));
                        }
                    }
                } else {
                    edit_draft(&mut draft, &timeline, |draft| {
                        adjust_selected(draft, &timeline, selection.0, direction)
                    });
                }
            }
            PracticeUiAction::SetLoopStart(ms) => {
                edit_draft(&mut draft, &timeline, |draft| {
                    set_loop_bound(draft, ms, true, timeline.end_ms)
                });
            }
            PracticeUiAction::SetLoopEnd(ms) => {
                edit_draft(&mut draft, &timeline, |draft| {
                    set_loop_bound(draft, ms, false, timeline.end_ms)
                });
            }
            PracticeUiAction::SetLoopRegion(region) => {
                edit_draft(&mut draft, &timeline, |draft| {
                    draft.loop_region = Some(region)
                });
            }
            PracticeUiAction::SetTempo(value) => {
                edit_draft(&mut draft, &timeline, |draft| draft.user_tempo = value)
            }
            PracticeUiAction::SetSnap(value) => {
                edit_draft(&mut draft, &timeline, |draft| draft.snap = value)
            }
            PracticeUiAction::SetPreroll(value) => edit_draft(&mut draft, &timeline, |draft| {
                draft.preroll = normalize_preroll(value)
            }),
            PracticeUiAction::SetCountIn(value) => {
                edit_draft(&mut draft, &timeline, |draft| draft.count_in = value)
            }
            PracticeUiAction::SetTrainerMode(value) => {
                edit_draft(&mut draft, &timeline, |draft| draft.set_trainer_mode(value))
            }
            PracticeUiAction::SetRampStart(value) => edit_draft(&mut draft, &timeline, |draft| {
                draft.trainer.ramp_config.start_tempo = value
            }),
            PracticeUiAction::SetRampTarget(value) => edit_draft(&mut draft, &timeline, |draft| {
                draft.trainer.ramp_config.target_tempo = value
            }),
            PracticeUiAction::SetRampStep(value) => edit_draft(&mut draft, &timeline, |draft| {
                draft.trainer.ramp_config.step = value
            }),
            PracticeUiAction::SetRampThreshold(value) => {
                edit_draft(&mut draft, &timeline, |draft| {
                    draft.trainer.ramp_config.threshold_pct = value
                })
            }
            PracticeUiAction::SetRampPasses(value) => edit_draft(&mut draft, &timeline, |draft| {
                draft.trainer.ramp_config.required_successes = value
            }),
            PracticeUiAction::SaveAsNew => {
                preset_commands.write(PresetCommand::SaveNew {
                    name: None,
                    draft: draft.clone(),
                });
            }
            PracticeUiAction::UpdateSaved => {
                write_update_command(&draft, store.as_deref(), &mut preset_commands)
            }
            PracticeUiAction::RequestDeleteSaved => {
                if let PracticeDraftSource::Saved(id) = draft.source {
                    *prompt = PracticePresetPrompt::ConfirmDelete { id };
                }
            }
            PracticeUiAction::ConfirmDeleteSaved => {
                if let PracticePresetPrompt::ConfirmDelete { id } = *prompt {
                    preset_commands.write(PresetCommand::DeleteSaved { id });
                    *prompt = PracticePresetPrompt::None;
                }
            }
            PracticeUiAction::RetryPreset => {
                if let PracticePresetPrompt::Retry { command, .. } = &*prompt {
                    preset_commands.write((**command).clone());
                }
            }
            PracticeUiAction::CancelPresetPrompt => *prompt = PracticePresetPrompt::None,
            PracticeUiAction::StartOrContinue => {
                starts.write(StartOrContinueRequested);
            }
            PracticeUiAction::Preview(action) => {
                previews.write(action);
            }
            PracticeUiAction::SelectTab(next) => *tab = next,
            PracticeUiAction::MoveTab(direction) => {
                *tab = tab.offset(direction);
            }
            PracticeUiAction::Confirm => activate_selected(
                selection.0,
                &draft,
                store.as_deref(),
                &mut prompt,
                &mut preset_commands,
                &mut starts,
            ),
            PracticeUiAction::Back => {
                flow.preview = PreviewState::Stopped;
                match flow.phase {
                    PracticePhase::Setup => {
                        initial_cancels.write(crate::practice::InitialSetupCancelRequested);
                    }
                    PracticePhase::Editing => {
                        cancels.write(crate::practice::CancelPracticeSettings);
                    }
                    PracticePhase::Running => {}
                }
            }
        }
        let validated = draft
            .validate(&timeline)
            .expect("draft validation is infallible");
        if let Some(warning) = &validated.warning {
            toasts.push(warning.clone());
        }
        *draft = validated.draft;
        normalize_selection(&mut selection, &draft, &prompt);
    }
}

pub fn apply_preset_results(
    mut results: MessageReader<crate::practice::PresetResult>,
    mut draft: ResMut<PracticeDraft>,
    mut prompt: ResMut<PracticePresetPrompt>,
) {
    for result in results.read() {
        match result {
            crate::practice::PresetResult::Saved { id }
            | crate::practice::PresetResult::Updated { id } => {
                draft.source = PracticeDraftSource::Saved(*id);
                *prompt = PracticePresetPrompt::None;
            }
            crate::practice::PresetResult::Deleted { id }
                if draft.source == PracticeDraftSource::Saved(*id) =>
            {
                draft.source = PracticeDraftSource::Custom;
                *prompt = PracticePresetPrompt::None;
            }
            crate::practice::PresetResult::Deleted { .. }
            | crate::practice::PresetResult::LastUsedRecorded => {
                *prompt = PracticePresetPrompt::None;
            }
            crate::practice::PresetResult::Failed { message, retry } => {
                *prompt = PracticePresetPrompt::Retry {
                    message: message.clone(),
                    command: retry.clone(),
                };
            }
        }
    }
}

fn activate_selected(
    item: SetupItem,
    draft: &PracticeDraft,
    store: Option<&PracticePresetStore>,
    prompt: &mut PracticePresetPrompt,
    commands: &mut MessageWriter<PresetCommand>,
    starts: &mut MessageWriter<StartOrContinueRequested>,
) {
    match item {
        SetupItem::SaveAsNew => {
            commands.write(PresetCommand::SaveNew {
                name: None,
                draft: draft.clone(),
            });
        }
        SetupItem::UpdateSaved => write_update_command(draft, store, commands),
        SetupItem::DeleteSaved => {
            if let PracticeDraftSource::Saved(id) = draft.source {
                *prompt = PracticePresetPrompt::ConfirmDelete { id };
            }
        }
        SetupItem::ConfirmDelete => {
            if let PracticePresetPrompt::ConfirmDelete { id } = *prompt {
                commands.write(PresetCommand::DeleteSaved { id });
                *prompt = PracticePresetPrompt::None;
            }
        }
        SetupItem::CancelDelete | SetupItem::CancelRetry => *prompt = PracticePresetPrompt::None,
        SetupItem::RetryPreset => {
            if let PracticePresetPrompt::Retry { command, .. } = prompt {
                commands.write((**command).clone());
            }
        }
        SetupItem::StartOrContinue => {
            starts.write(StartOrContinueRequested);
        }
        _ => {}
    }
}

fn adjust_selected(
    draft: &mut PracticeDraft,
    timeline: &ChipTimeline,
    item: SetupItem,
    direction: i8,
) {
    match item {
        SetupItem::LoopStart => {
            let current = draft.loop_region.map_or(0, |region| region.start_ms);
            set_loop_bound(
                draft,
                timeline.snap_neighbor(current, draft.snap, direction),
                true,
                timeline.end_ms,
            );
        }
        SetupItem::LoopEnd => {
            let current = draft
                .loop_region
                .map_or(timeline.end_ms, |region| region.end_ms);
            set_loop_bound(
                draft,
                timeline.snap_neighbor(current, draft.snap, direction),
                false,
                timeline.end_ms,
            );
        }
        SetupItem::Tempo => {
            draft.user_tempo =
                (draft.user_tempo + RATE_STEP * direction as f32).clamp(RATE_MIN, RATE_MAX)
        }
        SetupItem::Snap => {
            draft.snap = adjust_snap(draft.snap, direction);
        }
        SetupItem::Preroll => draft.preroll = adjust_preroll(draft.preroll, direction),
        SetupItem::CountIn => draft.count_in = !draft.count_in,
        SetupItem::TrainerMode => {
            let next = match (draft.trainer_mode(), direction > 0) {
                (PracticeTrainerMode::Off, true) | (PracticeTrainerMode::Ramp, false) => {
                    PracticeTrainerMode::Wait
                }
                (PracticeTrainerMode::Wait, true) => PracticeTrainerMode::Ramp,
                (PracticeTrainerMode::Wait, false) | (PracticeTrainerMode::Ramp, true) => {
                    PracticeTrainerMode::Off
                }
                (PracticeTrainerMode::Off, false) => PracticeTrainerMode::Ramp,
            };
            draft.set_trainer_mode(next);
        }
        SetupItem::RampStart => {
            draft.trainer.ramp_config.start_tempo += RATE_STEP * direction as f32
        }
        SetupItem::RampTarget => {
            draft.trainer.ramp_config.target_tempo += RATE_STEP * direction as f32
        }
        SetupItem::RampStep => draft.trainer.ramp_config.step += RATE_STEP * direction as f32,
        SetupItem::RampThreshold => {
            draft.trainer.ramp_config.threshold_pct += 1.0 * direction as f32
        }
        SetupItem::RampPasses => {
            draft.trainer.ramp_config.required_successes =
                (draft.trainer.ramp_config.required_successes as i16 + direction as i16).clamp(1, 3)
                    as u8
        }
        _ => {}
    }
}

fn adjust_snap(value: SnapDivisor, direction: i8) -> SnapDivisor {
    match (value, direction.cmp(&0)) {
        (value, std::cmp::Ordering::Equal) => value,
        (SnapDivisor::Bar, std::cmp::Ordering::Greater)
        | (SnapDivisor::Quarter, std::cmp::Ordering::Less) => SnapDivisor::Beat,
        (SnapDivisor::Beat, std::cmp::Ordering::Greater)
        | (SnapDivisor::Bar, std::cmp::Ordering::Less) => SnapDivisor::Quarter,
        (SnapDivisor::Quarter, std::cmp::Ordering::Greater)
        | (SnapDivisor::Beat, std::cmp::Ordering::Less) => SnapDivisor::Bar,
    }
}

fn adjust_preroll(value: PrerollSetting, direction: i8) -> PrerollSetting {
    match (normalize_preroll(value), direction.cmp(&0)) {
        (value, std::cmp::Ordering::Equal) => value,
        (PrerollSetting::OneBar, std::cmp::Ordering::Greater)
        | (PrerollSetting::Off, std::cmp::Ordering::Less) => PrerollSetting::Seconds(2.0),
        (PrerollSetting::Seconds(_), std::cmp::Ordering::Greater)
        | (PrerollSetting::OneBar, std::cmp::Ordering::Less) => PrerollSetting::Off,
        (PrerollSetting::Off, std::cmp::Ordering::Greater)
        | (PrerollSetting::Seconds(_), std::cmp::Ordering::Less) => PrerollSetting::OneBar,
    }
}

fn set_loop_bound(draft: &mut PracticeDraft, ms: i64, start: bool, chart_end: i64) {
    let ms = ms.clamp(0, chart_end.max(0));
    let mut region = draft.loop_region.unwrap_or(LoopRegion {
        start_ms: 0,
        end_ms: chart_end.max(0),
    });
    if start {
        region.start_ms = ms;
    } else {
        region.end_ms = ms;
    }
    if region.start_ms > region.end_ms {
        std::mem::swap(&mut region.start_ms, &mut region.end_ms);
    }
    draft.loop_region = (region.start_ms < region.end_ms).then_some(region);
}

fn edit_draft(
    draft: &mut PracticeDraft,
    _timeline: &ChipTimeline,
    edit: impl FnOnce(&mut PracticeDraft),
) {
    let source = draft.source;
    edit(draft);
    draft.source = match source {
        PracticeDraftSource::Saved(id) => PracticeDraftSource::Saved(id),
        PracticeDraftSource::Custom => PracticeDraftSource::Custom,
        _ => PracticeDraftSource::Custom,
    };
}

fn selected_source_draft(
    current: &PracticeDraft,
    store: Option<&PracticePresetStore>,
    catalog: Option<&PracticeSourceCatalog>,
    source: PracticeDraftSource,
) -> Option<PracticeDraft> {
    if source == PracticeDraftSource::Custom {
        let mut custom = current.clone();
        custom.source = PracticeDraftSource::Custom;
        Some(custom)
    } else {
        crate::practice::presets::source_draft(store, catalog, source)
    }
}

fn normalize_preroll(value: PrerollSetting) -> PrerollSetting {
    match value {
        PrerollSetting::OneBar | PrerollSetting::Off => value,
        PrerollSetting::Seconds(value) if value.is_finite() => PrerollSetting::Seconds(2.0),
        PrerollSetting::Seconds(_) => PrerollSetting::OneBar,
    }
}

fn write_update_command(
    draft: &PracticeDraft,
    store: Option<&PracticePresetStore>,
    commands: &mut MessageWriter<PresetCommand>,
) {
    if let PracticeDraftSource::Saved(id) = draft.source {
        let name = store
            .and_then(|store| store.registry.preset(id))
            .and_then(|preset| preset.name.clone());
        commands.write(PresetCommand::UpdateSaved {
            id,
            name,
            draft: draft.clone(),
        });
    }
}

pub(super) fn reset_selection(
    mut selection: ResMut<SetupSelection>,
    mut prompt: ResMut<PracticePresetPrompt>,
) {
    *selection = SetupSelection::default();
    *prompt = PracticePresetPrompt::None;
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SetupSelection>()
        .init_resource::<PracticePresetPrompt>()
        .add_message::<PracticeUiAction>()
        .add_message::<StartOrContinueRequested>()
        .add_message::<crate::practice::InitialSetupCancelRequested>()
        .add_systems(OnEnter(game_shell::AppState::Performance), reset_selection)
        .add_systems(
            Update,
            (keyboard_actions, nav_actions)
                .run_if(crate::practice::practice_surface_open)
                .before(apply_ui_actions),
        )
        .add_systems(
            Update,
            apply_ui_actions
                .after(super::setup::setup_button_actions)
                .after(super::setup::update_tab_selection)
                .after(super::timeline_ui::timeline_mouse)
                .after(super::timeline_ui::preview_transport_buttons)
                .before(super::setup::ensure_setup_shell)
                .run_if(crate::practice::practice_surface_open),
        )
        .add_systems(
            Update,
            apply_preset_results
                .after(crate::practice::presets::preset_system)
                .before(super::setup::ensure_setup_shell)
                .run_if(resource_exists::<PracticeDraft>)
                .run_if(resource_exists::<crate::practice::PracticeSession>),
        );
}
