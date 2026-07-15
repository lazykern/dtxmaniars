use std::path::PathBuf;

use bevy::prelude::*;
use dtx_config::{
    load_practice_presets, save_practice_presets, PracticeChartKey, PracticePresetRegistry,
    PracticePresetStartup,
};

use super::{PracticeDraft, PracticeDraftSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PracticePresetStoreStatus {
    Ready,
    ReadOnly(String),
}

#[derive(Resource, Debug, Clone)]
pub struct PracticePresetStore {
    pub path: PathBuf,
    pub chart: PracticeChartKey,
    pub source_path_hint: Option<PathBuf>,
    pub registry: PracticePresetRegistry,
    pub status: PracticePresetStoreStatus,
}

impl PracticePresetStore {
    pub fn ready(
        path: PathBuf,
        chart: PracticeChartKey,
        source_path_hint: Option<PathBuf>,
        registry: PracticePresetRegistry,
    ) -> Self {
        Self {
            path,
            chart,
            source_path_hint,
            registry,
            status: PracticePresetStoreStatus::Ready,
        }
    }

    pub fn read_only(
        path: PathBuf,
        chart: PracticeChartKey,
        source_path_hint: Option<PathBuf>,
        registry: PracticePresetRegistry,
        error: impl Into<String>,
    ) -> Self {
        Self {
            path,
            chart,
            source_path_hint,
            registry,
            status: PracticePresetStoreStatus::ReadOnly(error.into()),
        }
    }

    fn load() -> Self {
        let path = dtx_config::practice_presets_path();
        let chart = PracticeChartKey::new("dtx1:unloaded", 0);
        match load_practice_presets(&path) {
            PracticePresetStartup::Ready(registry) => Self::ready(path, chart, None, registry),
            PracticePresetStartup::ReadOnly { registry, error } => {
                Self::read_only(path, chart, None, registry, error.to_string())
            }
        }
    }
}

#[derive(Message, Debug, Clone, PartialEq)]
pub enum PresetCommand {
    SaveNew {
        name: Option<String>,
        draft: PracticeDraft,
    },
    UpdateSaved {
        id: u64,
        name: Option<String>,
        draft: PracticeDraft,
    },
    DeleteSaved {
        id: u64,
    },
    RecordLastUsed {
        draft: PracticeDraft,
    },
}

#[derive(Message, Debug, Clone, PartialEq)]
pub enum PresetResult {
    Saved {
        id: u64,
    },
    Updated {
        id: u64,
    },
    Deleted {
        id: u64,
    },
    LastUsedRecorded,
    Failed {
        message: String,
        retry: Box<PresetCommand>,
    },
}

pub fn apply_preset_command(
    store: &mut PracticePresetStore,
    command: PresetCommand,
) -> PresetResult {
    if let PracticePresetStoreStatus::ReadOnly(error) = &store.status {
        return PresetResult::Failed {
            message: format!(
                "Practice presets are read-only: {error}. Retry after fixing the file."
            ),
            retry: Box::new(command),
        };
    }

    let mut candidate = store.registry.clone();
    let result = match &command {
        PresetCommand::SaveNew { name, draft } => candidate
            .create(
                store.chart.clone(),
                name.as_deref(),
                store.source_path_hint.clone(),
                draft.into(),
            )
            .map(|id| PresetResult::Saved { id }),
        PresetCommand::UpdateSaved { id, name, draft } => candidate
            .update(
                *id,
                name.as_deref(),
                store.source_path_hint.clone(),
                draft.into(),
            )
            .map(|()| PresetResult::Updated { id: *id }),
        PresetCommand::DeleteSaved { id } => candidate
            .delete(*id)
            .map(|()| PresetResult::Deleted { id: *id }),
        PresetCommand::RecordLastUsed { draft } => candidate
            .set_last_used(
                store.chart.clone(),
                store.source_path_hint.clone(),
                draft.into(),
            )
            .map(|()| PresetResult::LastUsedRecorded),
    };
    let success = match result {
        Ok(success) => success,
        Err(error) => {
            return PresetResult::Failed {
                message: format!("Could not update practice presets: {error}. Retry the action."),
                retry: Box::new(command),
            };
        }
    };
    if let Err(error) = save_practice_presets(&store.path, &candidate) {
        return PresetResult::Failed {
            message: format!("Could not save practice presets: {error}. Retry the action."),
            retry: Box::new(command),
        };
    }
    store.registry = candidate;
    success
}

pub fn source_draft(
    store: &PracticePresetStore,
    source: PracticeDraftSource,
) -> Option<PracticeDraft> {
    match source {
        PracticeDraftSource::WholeSong => Some(PracticeDraft::default()),
        PracticeDraftSource::LastUsed => store.registry.last_used(&store.chart).map(|entry| {
            let mut draft = PracticeDraft::from(&entry.config);
            draft.source = PracticeDraftSource::LastUsed;
            draft
        }),
        PracticeDraftSource::Saved(id) => store.registry.preset(id).and_then(|preset| {
            (preset.chart == store.chart).then(|| PracticeDraft::from_preset(id, &preset.config))
        }),
        PracticeDraftSource::Recommended | PracticeDraftSource::Custom => None,
    }
}

pub fn automatic_preset_label(
    config: &dtx_config::PracticePresetConfig,
    timeline: &crate::timeline::ChipTimeline,
) -> String {
    match (config.loop_start_ms, config.loop_end_ms) {
        (Some(start), Some(end)) => format!(
            "Bars {}-{} / {}-{}",
            crate::practice::hud::timeline_ui::bar_number(&timeline.bar_ms, start),
            crate::practice::hud::timeline_ui::bar_number(&timeline.bar_ms, end),
            crate::practice::hud::format_chart_time(start),
            crate::practice::hud::format_chart_time(end)
        ),
        _ => "Whole Song".to_owned(),
    }
}

pub fn ordered_sources(
    store: &PracticePresetStore,
    recommended: bool,
    timeline: &crate::timeline::ChipTimeline,
) -> Vec<(PracticeDraftSource, String)> {
    let mut sources = vec![(PracticeDraftSource::WholeSong, "Whole Song".to_owned())];
    if store.registry.last_used(&store.chart).is_some() {
        sources.push((PracticeDraftSource::LastUsed, "Last Used".to_owned()));
    }
    if recommended {
        sources.push((
            PracticeDraftSource::Recommended,
            "Recommended Section".to_owned(),
        ));
    }
    let mut saved = store
        .registry
        .presets_for(&store.chart)
        .map(|preset| {
            let label = preset
                .name
                .clone()
                .unwrap_or_else(|| automatic_preset_label(&preset.config, timeline));
            (PracticeDraftSource::Saved(preset.id), label)
        })
        .collect::<Vec<_>>();
    saved.sort_by(|left, right| {
        left.1
            .to_lowercase()
            .cmp(&right.1.to_lowercase())
            .then_with(|| match (left.0, right.0) {
                (PracticeDraftSource::Saved(a), PracticeDraftSource::Saved(b)) => a.cmp(&b),
                _ => std::cmp::Ordering::Equal,
            })
    });
    sources.extend(saved);
    sources.push((PracticeDraftSource::Custom, "Custom".to_owned()));
    sources
}

fn configure_chart_context(
    chart: Res<crate::resources::ActiveChart>,
    stage: Res<crate::perf_common::PerformanceStageState>,
    mut store: ResMut<PracticePresetStore>,
) {
    store.chart = PracticeChartKey::new(
        dtx_scoring::identity::canonical_chart_hash(&chart.chart),
        stage.confirmed_difficulty,
    );
    store.source_path_hint = chart.source_path.clone();
}

pub fn preset_system(
    mut commands: MessageReader<PresetCommand>,
    mut store: ResMut<PracticePresetStore>,
    mut results: MessageWriter<PresetResult>,
    mut toasts: ResMut<super::toast::ToastQueue>,
) {
    for command in commands.read() {
        let result = apply_preset_command(&mut store, command.clone());
        if let PresetResult::Failed { message, .. } = &result {
            toasts.push(message.clone());
        }
        results.write(result);
    }
}

pub(super) fn plugin(app: &mut App) {
    if !app.world().contains_resource::<PracticePresetStore>() {
        let store = PracticePresetStore::load();
        if let PracticePresetStoreStatus::ReadOnly(error) = &store.status {
            app.world_mut()
                .resource_mut::<super::toast::ToastQueue>()
                .push(format!("Practice presets are read-only: {error}"));
        }
        app.insert_resource(store);
    }
    app.add_message::<PresetCommand>()
        .add_message::<PresetResult>()
        .add_systems(
            OnEnter(game_shell::AppState::Performance),
            configure_chart_context.after(crate::orchestrator::DrumsEnterSet),
        )
        .add_systems(Update, preset_system);
}
