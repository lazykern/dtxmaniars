use bevy::prelude::*;

use crate::practice::session::{LoopRegion, PrerollSetting, RATE_MAX, RATE_MIN, RATE_STEP};
use crate::practice::{
    PracticeDraft, PracticeDraftSource, PracticeFlow, PracticePhase, PracticePresetStore,
    PracticeTrainerMode, PresetCommand, PreviewAction, PreviewState,
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
    StartOrContinue,
}

const SETUP_ITEMS: [SetupItem; 17] = [
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
    SetupItem::StartOrContinue,
];

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupSelection(pub SetupItem);

impl Default for SetupSelection {
    fn default() -> Self {
        Self(SetupItem::Source)
    }
}

pub fn visible_setup_items(draft: &PracticeDraft) -> Vec<SetupItem> {
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
        })
        .collect()
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum PracticeUiAction {
    SelectSource(PracticeDraftSource),
    SelectItem(SetupItem),
    MoveSelection(i8),
    Adjust(i8),
    SetLoopStart(i64),
    SetLoopEnd(i64),
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
    DeleteSaved,
    StartOrContinue,
    Preview(PreviewAction),
    Confirm,
    Back,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartOrContinueRequested;

pub fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageWriter<PracticeUiAction>,
) {
    let action = if keys.just_pressed(KeyCode::ArrowUp) {
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
    mut actions: MessageWriter<PracticeUiAction>,
) {
    for action in nav.read() {
        let action = match action.verb {
            game_shell::NavVerb::Up => PracticeUiAction::MoveSelection(-1),
            game_shell::NavVerb::Down => PracticeUiAction::MoveSelection(1),
            game_shell::NavVerb::Dec => PracticeUiAction::Adjust(-1),
            game_shell::NavVerb::Inc => PracticeUiAction::Adjust(1),
            game_shell::NavVerb::Confirm => PracticeUiAction::Confirm,
            game_shell::NavVerb::Back => PracticeUiAction::Back,
            game_shell::NavVerb::Practice => continue,
        };
        actions.write(action);
    }
}

pub fn apply_ui_actions(
    mut actions: MessageReader<PracticeUiAction>,
    mut selection: ResMut<SetupSelection>,
    mut draft: ResMut<PracticeDraft>,
    mut flow: ResMut<PracticeFlow>,
    timeline: Res<ChipTimeline>,
    store: Option<Res<PracticePresetStore>>,
    mut preset_commands: MessageWriter<PresetCommand>,
    mut previews: MessageWriter<PreviewAction>,
    mut starts: MessageWriter<StartOrContinueRequested>,
    mut cancels: MessageWriter<crate::practice::CancelPracticeSettings>,
) {
    for action in actions.read().copied() {
        match action {
            PracticeUiAction::SelectSource(source) => {
                if let Some(next) = store
                    .as_deref()
                    .and_then(|store| crate::practice::presets::source_draft(store, source))
                {
                    *draft = next;
                    flow.preview = PreviewState::Stopped;
                }
            }
            PracticeUiAction::SelectItem(item) => selection.0 = item,
            PracticeUiAction::MoveSelection(direction) => {
                let visible = visible_setup_items(&draft);
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
                            draft.source == PracticeDraftSource::Recommended,
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
                        if source == PracticeDraftSource::Custom {
                            draft.source = source;
                        } else if let Some(next) =
                            crate::practice::presets::source_draft(store, source)
                        {
                            *draft = next;
                            flow.preview = PreviewState::Stopped;
                        }
                    }
                } else {
                    adjust_selected(&mut draft, &timeline, selection.0, direction)
                }
            }
            PracticeUiAction::SetLoopStart(ms) => {
                set_loop_bound(&mut draft, ms, true, timeline.end_ms)
            }
            PracticeUiAction::SetLoopEnd(ms) => {
                set_loop_bound(&mut draft, ms, false, timeline.end_ms)
            }
            PracticeUiAction::SetTempo(tempo) => draft.user_tempo = tempo.clamp(RATE_MIN, RATE_MAX),
            PracticeUiAction::SetSnap(snap) => draft.snap = snap,
            PracticeUiAction::SetPreroll(preroll) => draft.preroll = preroll,
            PracticeUiAction::SetCountIn(enabled) => draft.count_in = enabled,
            PracticeUiAction::SetTrainerMode(mode) => draft.set_trainer_mode(mode),
            PracticeUiAction::SetRampStart(value) => draft.trainer.ramp_config.start_tempo = value,
            PracticeUiAction::SetRampTarget(value) => {
                draft.trainer.ramp_config.target_tempo = value
            }
            PracticeUiAction::SetRampStep(value) => draft.trainer.ramp_config.step = value,
            PracticeUiAction::SetRampThreshold(value) => {
                draft.trainer.ramp_config.threshold_pct = value
            }
            PracticeUiAction::SetRampPasses(value) => {
                draft.trainer.ramp_config.required_successes = value
            }
            PracticeUiAction::SaveAsNew => {
                preset_commands.write(PresetCommand::SaveNew {
                    name: None,
                    draft: draft.clone(),
                });
            }
            PracticeUiAction::UpdateSaved => {
                if let PracticeDraftSource::Saved(id) = draft.source {
                    let name = store
                        .as_deref()
                        .and_then(|store| store.registry.preset(id))
                        .and_then(|preset| preset.name.clone());
                    preset_commands.write(PresetCommand::UpdateSaved {
                        id,
                        name,
                        draft: draft.clone(),
                    });
                }
            }
            PracticeUiAction::DeleteSaved => {
                if let PracticeDraftSource::Saved(id) = draft.source {
                    preset_commands.write(PresetCommand::DeleteSaved { id });
                }
            }
            PracticeUiAction::StartOrContinue => {
                starts.write(StartOrContinueRequested);
            }
            PracticeUiAction::Preview(action) => {
                previews.write(action);
            }
            PracticeUiAction::Confirm => {
                activate_selected(selection.0, &draft, &mut preset_commands, &mut starts)
            }
            PracticeUiAction::Back => {
                flow.preview = PreviewState::Stopped;
                if flow.phase == PracticePhase::Editing {
                    cancels.write(crate::practice::CancelPracticeSettings);
                }
            }
        }
    }
}

pub fn apply_preset_results(
    mut results: MessageReader<crate::practice::PresetResult>,
    mut draft: ResMut<PracticeDraft>,
) {
    for result in results.read() {
        match result {
            crate::practice::PresetResult::Saved { id }
            | crate::practice::PresetResult::Updated { id } => {
                draft.source = PracticeDraftSource::Saved(*id);
            }
            crate::practice::PresetResult::Deleted { id }
                if draft.source == PracticeDraftSource::Saved(*id) =>
            {
                draft.source = PracticeDraftSource::Custom;
            }
            crate::practice::PresetResult::Deleted { .. }
            | crate::practice::PresetResult::LastUsedRecorded
            | crate::practice::PresetResult::Failed { .. } => {}
        }
    }
}

fn activate_selected(
    item: SetupItem,
    draft: &PracticeDraft,
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
        SetupItem::UpdateSaved => {
            if let PracticeDraftSource::Saved(id) = draft.source {
                commands.write(PresetCommand::UpdateSaved {
                    id,
                    name: None,
                    draft: draft.clone(),
                });
            }
        }
        SetupItem::DeleteSaved => {
            if let PracticeDraftSource::Saved(id) = draft.source {
                commands.write(PresetCommand::DeleteSaved { id });
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
            draft.snap = if direction == 0 {
                draft.snap
            } else {
                draft.snap.next()
            }
        }
        SetupItem::Preroll => draft.preroll = draft.preroll.next(),
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
    draft.source = PracticeDraftSource::Custom;
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
    draft.source = PracticeDraftSource::Custom;
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SetupSelection>()
        .add_message::<PracticeUiAction>()
        .add_message::<StartOrContinueRequested>()
        .add_systems(Update, (keyboard_actions, nav_actions))
        .add_systems(
            Update,
            (apply_ui_actions, apply_preset_results)
                .chain()
                .after(super::PracticeShellUpdate)
                .run_if(crate::practice::practice_surface_open),
        );
}
