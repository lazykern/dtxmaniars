use bevy::camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use game_shell::{AppState, PauseState};
use gameplay_drums::practice::hud::progress::progress_rows;
use gameplay_drums::practice::hud::setup::{
    practice_layout_mode, practice_transport_row_mode, update_tab_selection, PracticeLayoutMode,
    PracticePanelTransition, PracticePreviewGeometry, PracticePreviewRegion, PracticePrimaryAction,
    PracticeSettingsPane, PracticeSetupLayout, PracticeSetupRoot, PracticeSurfaceFocus,
    PracticeTab, PracticeTabButton, PracticeTabCrossfade, PracticeTransportRowMode,
    SetupAdjustButton,
};
use gameplay_drums::practice::hud::setup_controls::{
    PracticeUiAction, PresetNameInput, SetupItem, SetupSelection,
};
use gameplay_drums::practice::hud::timeline_ui::{
    PracticeLoopFill, PracticeLoopHandle, PracticeTimelineRoot, PracticeTimelineStrip,
    PreviewTransportButton, TimelineGesture,
};
use gameplay_drums::practice::session::{AttemptRecord, LoopRegion};
use gameplay_drums::practice::{
    apply_preset_command, PracticeDraft, PracticeDraftSource, PracticeFlow, PracticePresetPrompt,
    PracticePresetStore, PracticeSession, PracticeSourceCatalog, PracticeTrainerMode,
    PresetCommand, PresetResult,
};
use gameplay_drums::resources::GameplayClock;
use gameplay_drums::timeline::{ChipTimeline, SnapDivisor};

#[derive(Resource, Default)]
struct PendingTabClick(Option<PracticeTab>);

#[derive(Resource, Default)]
struct ConsumerLayout(Option<gameplay_drums::stage_rect::StageRect>);

fn record_consumer_layout(
    layout: Res<gameplay_drums::layout::PlayfieldLayout>,
    mut seen: ResMut<ConsumerLayout>,
) {
    seen.0 = Some(gameplay_drums::stage_rect::StageRect {
        origin: layout.origin,
        size: Vec2::new(layout.width, layout.height),
    });
}

fn inject_tab_click(
    mut pending: ResMut<PendingTabClick>,
    mut buttons: Query<(&PracticeTabButton, &mut Interaction)>,
) {
    let Some(target) = pending.0.take() else {
        return;
    };
    for (button, mut interaction) in &mut buttons {
        if button.0 == target {
            *interaction = Interaction::Pressed;
        }
    }
}

fn build_hud_app(width: f32, height: f32, text_scale: dtx_config::TextScale) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin {
            primary_window: None,
            ..default()
        },
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::image::TextureAtlasPlugin,
        bevy::text::TextPlugin,
        bevy::picking::DefaultPickingPlugins,
        bevy::ui::UiPlugin,
        dtx_ui::plugin,
    ))
    .init_state::<AppState>()
    .init_state::<PauseState>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_message::<gameplay_drums::practice::actions::PracticeAction>()
    .add_message::<gameplay_drums::practice::PreviewAction>()
    .init_resource::<GameplayClock>()
    .init_resource::<ChipTimeline>()
    .init_resource::<gameplay_drums::lanes::Lanes>()
    .insert_resource(gameplay_drums::layout::PlayfieldLayout::from_size(
        width,
        height,
        &gameplay_drums::lanes::Lanes::default(),
    ))
    .insert_resource(dtx_ui::AccessibilityPolicy::from(
        &dtx_config::AccessibilityConfig {
            text_scale,
            ..Default::default()
        },
    ))
    .insert_resource(PracticeSession::default())
    .insert_resource(PracticeDraft::default())
    .insert_resource(PracticeFlow::default());

    let physical_size = UVec2::new(width as u32, height as u32);
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size,
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(Viewport {
                physical_size,
                ..default()
            }),
            ..default()
        },
    ));
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(width as u32, height as u32)
                .with_scale_factor_override(1.0),
            ..default()
        },
        PrimaryWindow,
    ));

    gameplay_drums::practice::hud::plugin(&mut app);
    app.init_resource::<PendingTabClick>()
        .init_resource::<ConsumerLayout>()
        .configure_sets(
            Update,
            (
                gameplay_drums::layout::PlayfieldLayoutSync,
                gameplay_drums::layout::PlayfieldLayoutConsumers,
            )
                .chain(),
        )
        .add_systems(Update, inject_tab_click.before(update_tab_selection))
        .add_systems(
            Update,
            record_consumer_layout.in_set(gameplay_drums::layout::PlayfieldLayoutConsumers),
        );
    app.add_systems(
        Update,
        gameplay_drums::layout::sync_playfield_layout
            .in_set(gameplay_drums::layout::PlayfieldLayoutSync),
    );
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app
}

fn setup_hud_app(width: f32, height: f32, text_scale: dtx_config::TextScale) -> App {
    let mut app = build_hud_app(width, height, text_scale);
    app.update();
    app.update();
    app.update();
    app
}

fn setup_hud_app_with_accessibility(
    width: f32,
    height: f32,
    config: dtx_config::AccessibilityConfig,
) -> App {
    let mut app = build_hud_app(width, height, config.text_scale);
    app.world_mut()
        .insert_resource(dtx_ui::AccessibilityPolicy::from(&config));
    app.update();
    app.update();
    app.update();
    app
}

fn send_ui_action(app: &mut App, action: PracticeUiAction) {
    app.world_mut().write_message(action);
}

fn press_key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.world_mut()
        .run_system_once(gameplay_drums::practice::hud::setup_controls::keyboard_actions)
        .expect("keyboard actions run");
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
}

fn send_nav(app: &mut App, verb: game_shell::SystemVerb) {
    app.world_mut().write_message(game_shell::NavAction {
        verb,
        source: game_shell::InputSource::Keyboard,
        coarse: false,
        repeated: false,
    });
    app.update();
}

fn texts(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect()
}

#[test]
fn selecting_saved_source_populates_draft_without_starting_preview() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let chart = dtx_config::PracticeChartKey::new("dtx1:test", 0);
    let mut registry = dtx_config::PracticePresetRegistry::default();
    let id = registry
        .create(
            chart.clone(),
            Some("Chorus"),
            None,
            dtx_config::PracticePresetConfig {
                loop_start_ms: Some(10_000),
                loop_end_ms: Some(20_000),
                snap: dtx_config::PracticeSnapPreset::Beat,
                tempo: 0.8,
                preroll: dtx_config::PracticePrerollPreset::TwoSeconds,
                count_in: false,
                trainer: dtx_config::PracticeTrainerPreset::Wait,
            },
        )
        .expect("valid preset");
    app.world_mut().insert_resource(PracticePresetStore::ready(
        std::env::temp_dir().join("practice-hud-source.toml"),
        chart,
        None,
        registry,
    ));

    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Saved(id)),
    );
    app.update();

    let draft = app.world().resource::<PracticeDraft>();
    assert_eq!(draft.user_tempo, 0.8);
    assert_eq!(draft.snap, SnapDivisor::Beat);
    assert_eq!(draft.preroll.label(), "2s");
    assert!(!draft.count_in);
    assert_eq!(draft.trainer_mode(), PracticeTrainerMode::Wait);
    assert_eq!(
        app.world().resource::<PracticeFlow>().preview,
        gameplay_drums::practice::PreviewState::Stopped
    );
}

#[test]
fn conditional_rows_appear_and_disappear_in_the_action_frame() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    send_ui_action(
        &mut app,
        PracticeUiAction::SetTrainerMode(PracticeTrainerMode::Ramp),
    );
    app.update();
    assert!(texts(&mut app).iter().any(|text| text == "Start tempo"));

    send_ui_action(
        &mut app,
        PracticeUiAction::SetTrainerMode(PracticeTrainerMode::Off),
    );
    app.update();
    assert!(!texts(&mut app).iter().any(|text| text == "Start tempo"));
}

#[test]
fn hidden_selection_normalizes_and_fresh_performance_resets_it() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    send_ui_action(
        &mut app,
        PracticeUiAction::SetTrainerMode(PracticeTrainerMode::Ramp),
    );
    send_ui_action(&mut app, PracticeUiAction::SelectItem(SetupItem::RampStart));
    app.update();
    send_ui_action(
        &mut app,
        PracticeUiAction::SetTrainerMode(PracticeTrainerMode::Off),
    );
    app.update();
    assert_eq!(
        app.world().resource::<SetupSelection>().0,
        SetupItem::TrainerMode
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    assert_eq!(
        app.world().resource::<SetupSelection>().0,
        SetupItem::Source
    );
}

#[test]
fn recommended_source_survives_cycling_and_seeks_without_attempt_start() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    let mut recommended = PracticeDraft {
        source: PracticeDraftSource::Recommended,
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        user_tempo: 0.8,
        ..Default::default()
    };
    recommended.trainer.mode = PracticeTrainerMode::Wait;
    app.world_mut().insert_resource(PracticeSourceCatalog {
        recommended: Some(recommended.clone()),
    });
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::WholeSong),
    );
    app.update();
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Recommended),
    );
    app.update();

    assert_eq!(*app.world().resource::<PracticeDraft>(), recommended);
    let seeks: Vec<_> = app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert!(
        seeks.ends_with(&[
            gameplay_drums::practice::PreviewAction::Pause,
            gameplay_drums::practice::PreviewAction::Seek(2_000),
        ]),
        "{seeks:?}"
    );
}

#[test]
fn selecting_custom_preserves_the_loaded_values_and_changes_only_identity() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    let loaded = PracticeDraft {
        source: PracticeDraftSource::Recommended,
        loop_region: Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        }),
        user_tempo: 0.8,
        ..Default::default()
    };
    *app.world_mut().resource_mut::<PracticeDraft>() = loaded.clone();

    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Custom),
    );
    app.update();

    let draft = app.world().resource::<PracticeDraft>();
    assert_eq!(draft.source, PracticeDraftSource::Custom);
    assert_eq!(draft.loop_region, loaded.loop_region);
    assert_eq!(draft.user_tempo, loaded.user_tempo);
}

#[test]
fn saved_edits_keep_identity_and_confirm_update_keeps_name() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    let chart = dtx_config::PracticeChartKey::new("dtx1:test", 0);
    let mut registry = dtx_config::PracticePresetRegistry::default();
    let id = registry
        .create(
            chart.clone(),
            Some("Verse"),
            None,
            dtx_config::PracticePresetConfig {
                loop_start_ms: Some(1_000),
                loop_end_ms: Some(5_000),
                snap: dtx_config::PracticeSnapPreset::Bar,
                tempo: 1.0,
                preroll: dtx_config::PracticePrerollPreset::OneBar,
                count_in: true,
                trainer: dtx_config::PracticeTrainerPreset::Off,
            },
        )
        .expect("preset");
    app.world_mut().insert_resource(PracticePresetStore::ready(
        std::env::temp_dir().join("practice-hud-saved-policy.toml"),
        chart,
        None,
        registry,
    ));
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Saved(id)),
    );
    send_ui_action(&mut app, PracticeUiAction::SetTempo(0.75));
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectItem(SetupItem::UpdateSaved),
    );
    send_ui_action(&mut app, PracticeUiAction::Confirm);
    app.update();

    assert_eq!(
        app.world().resource::<PracticeDraft>().source,
        PracticeDraftSource::Saved(id)
    );
    let commands: Vec<_> = app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert!(commands.iter().any(|command| matches!(
        command,
        PresetCommand::UpdateSaved { name: Some(name), .. } if name == "Verse"
    )));
}

#[test]
fn cycling_saved_sources_refreshes_name_and_custom_clears_it() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    let chart = dtx_config::PracticeChartKey::new("dtx1:cycle", 0);
    let mut registry = dtx_config::PracticePresetRegistry::default();
    let config = dtx_config::PracticePresetConfig {
        loop_start_ms: Some(1_000),
        loop_end_ms: Some(5_000),
        snap: dtx_config::PracticeSnapPreset::Bar,
        tempo: 1.0,
        preroll: dtx_config::PracticePrerollPreset::OneBar,
        count_in: true,
        trainer: dtx_config::PracticeTrainerPreset::Off,
    };
    let first = registry
        .create(chart.clone(), Some("A"), None, config.clone())
        .expect("first preset");
    let second = registry
        .create(chart.clone(), Some("B"), None, config)
        .expect("second preset");
    app.world_mut().insert_resource(PracticePresetStore::ready(
        std::env::temp_dir().join("practice-hud-cycle-policy.toml"),
        chart,
        None,
        registry,
    ));
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Saved(first)),
    );
    send_ui_action(&mut app, PracticeUiAction::SelectItem(SetupItem::Source));
    send_ui_action(&mut app, PracticeUiAction::Adjust(1));
    app.update();

    assert_eq!(
        app.world().resource::<PracticeDraft>().source,
        PracticeDraftSource::Saved(second)
    );
    assert_eq!(app.world().resource::<PresetNameInput>().value, "B");

    send_ui_action(
        &mut app,
        PracticeUiAction::SelectItem(SetupItem::UpdateSaved),
    );
    send_ui_action(&mut app, PracticeUiAction::Confirm);
    app.update();
    assert!(app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .any(|command| matches!(command, PresetCommand::UpdateSaved { id, name: Some(name), .. } if *id == second && name == "B")));

    send_ui_action(&mut app, PracticeUiAction::SelectItem(SetupItem::Source));
    send_ui_action(&mut app, PracticeUiAction::Adjust(1));
    app.update();
    assert_eq!(
        app.world().resource::<PracticeDraft>().source,
        PracticeDraftSource::Custom
    );
    assert!(app.world().resource::<PresetNameInput>().value.is_empty());
}

#[test]
fn optional_preset_name_is_used_by_save_and_blank_keeps_auto_label() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;

    app.world_mut().resource_mut::<PresetNameInput>().value = "  Chorus  ".to_owned();
    send_ui_action(&mut app, PracticeUiAction::SaveAsNew);
    app.update();
    assert!(app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .any(|command| matches!(command, PresetCommand::SaveNew { name: Some(name), .. } if name == "Chorus")));

    app.world_mut().resource_mut::<PresetNameInput>().value = "   ".to_owned();
    send_ui_action(&mut app, PracticeUiAction::SaveAsNew);
    app.update();
    assert!(app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .any(|command| matches!(command, PresetCommand::SaveNew { name: None, .. })));
}

#[test]
fn every_typed_setter_normalizes_nonfinite_and_extreme_values() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    app.world_mut().resource_mut::<PracticeDraft>().source = PracticeDraftSource::Saved(7);
    for action in [
        PracticeUiAction::SetTempo(f32::NAN),
        PracticeUiAction::SetRampStart(f32::INFINITY),
        PracticeUiAction::SetRampTarget(f32::NEG_INFINITY),
        PracticeUiAction::SetRampStep(f32::INFINITY),
        PracticeUiAction::SetRampThreshold(f32::NAN),
        PracticeUiAction::SetRampPasses(0),
        PracticeUiAction::SetPreroll(gameplay_drums::practice::session::PrerollSetting::Seconds(
            f32::NAN,
        )),
        PracticeUiAction::SetLoopStart(-5_000),
        PracticeUiAction::SetLoopEnd(i64::MAX),
    ] {
        send_ui_action(&mut app, action);
    }
    app.update();

    let draft = app.world().resource::<PracticeDraft>();
    assert_eq!(draft.source, PracticeDraftSource::Saved(7));
    assert_eq!(draft.user_tempo, 1.0);
    assert_eq!(draft.trainer.ramp_config.start_tempo, 0.7);
    assert_eq!(draft.trainer.ramp_config.target_tempo, 1.0);
    assert_eq!(draft.trainer.ramp_config.step, 0.05);
    assert_eq!(draft.trainer.ramp_config.threshold_pct, 90.0);
    assert_eq!(draft.trainer.ramp_config.required_successes, 1);
    assert_eq!(draft.preroll.label(), "1 bar");
    assert_eq!(
        draft.loop_region,
        Some(LoopRegion {
            start_ms: 0,
            end_ms: 10_000,
        })
    );
}

#[test]
fn mouse_adjust_buttons_use_the_same_typed_reducer() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let directions: Vec<_> = app
        .world_mut()
        .query::<&SetupAdjustButton>()
        .iter(app.world())
        .filter(|adjuster| adjuster.item == SetupItem::Tempo)
        .map(|adjuster| adjuster.direction)
        .collect();
    assert!(directions.contains(&-1));
    assert!(directions.contains(&1));
    app.world_mut().spawn((
        Interaction::Pressed,
        SetupAdjustButton {
            item: SetupItem::Tempo,
            direction: 1,
        },
    ));
    app.update();

    assert_eq!(app.world().resource::<PracticeDraft>().user_tempo, 1.05);
    assert_eq!(
        app.world().resource::<PracticeDraft>().source,
        PracticeDraftSource::Custom
    );
}

#[test]
fn snap_and_preroll_keyboard_adjustments_are_directional_and_wrap() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::Snap;
    press_key(&mut app, KeyCode::ArrowLeft);
    assert_eq!(
        app.world().resource::<PracticeDraft>().snap,
        SnapDivisor::Quarter
    );
    press_key(&mut app, KeyCode::ArrowRight);
    assert_eq!(
        app.world().resource::<PracticeDraft>().snap,
        SnapDivisor::Bar
    );

    app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::Preroll;
    press_key(&mut app, KeyCode::ArrowLeft);
    assert_eq!(
        app.world().resource::<PracticeDraft>().preroll,
        gameplay_drums::practice::session::PrerollSetting::Off
    );
    press_key(&mut app, KeyCode::ArrowRight);
    assert_eq!(
        app.world().resource::<PracticeDraft>().preroll,
        gameplay_drums::practice::session::PrerollSetting::OneBar
    );
}

#[test]
fn snap_and_preroll_nav_adjustments_are_directional_and_wrap() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::Snap;
    send_nav(&mut app, game_shell::SystemVerb::Decrease);
    assert_eq!(
        app.world().resource::<PracticeDraft>().snap,
        SnapDivisor::Quarter
    );
    send_nav(&mut app, game_shell::SystemVerb::Increase);
    assert_eq!(
        app.world().resource::<PracticeDraft>().snap,
        SnapDivisor::Bar
    );

    app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::Preroll;
    send_nav(&mut app, game_shell::SystemVerb::Decrease);
    assert_eq!(
        app.world().resource::<PracticeDraft>().preroll,
        gameplay_drums::practice::session::PrerollSetting::Off
    );
    send_nav(&mut app, game_shell::SystemVerb::Increase);
    assert_eq!(
        app.world().resource::<PracticeDraft>().preroll,
        gameplay_drums::practice::session::PrerollSetting::OneBar
    );
}

#[test]
fn snap_and_preroll_mouse_adjustments_are_directional_and_wrap() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    for (item, direction) in [
        (SetupItem::Snap, -1),
        (SetupItem::Snap, 1),
        (SetupItem::Preroll, -1),
        (SetupItem::Preroll, 1),
    ] {
        app.world_mut()
            .spawn((Interaction::Pressed, SetupAdjustButton { item, direction }));
        app.update();
    }

    assert_eq!(
        app.world().resource::<PracticeDraft>().snap,
        SnapDivisor::Bar
    );
    assert_eq!(
        app.world().resource::<PracticeDraft>().preroll,
        gameplay_drums::practice::session::PrerollSetting::OneBar
    );
}

#[test]
fn delete_confirmation_and_retry_are_explicit_typed_states() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<PracticeDraft>().source = PracticeDraftSource::Saved(9);
    send_ui_action(&mut app, PracticeUiAction::RequestDeleteSaved);
    app.update();
    assert!(matches!(
        app.world().resource::<PracticePresetPrompt>(),
        PracticePresetPrompt::ConfirmDelete { id: 9 }
    ));
    assert!(texts(&mut app).iter().any(|text| {
        text.starts_with(dtx_ui::StateMarker::Destructive.label())
            && text.contains("Confirm Delete")
    }));

    send_ui_action(&mut app, PracticeUiAction::CancelPresetPrompt);
    app.update();
    assert_eq!(
        *app.world().resource::<PracticePresetPrompt>(),
        PracticePresetPrompt::None
    );
}

#[test]
fn saved_rows_reconcile_on_live_source_changes() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<PracticeDraft>().source = PracticeDraftSource::Saved(3);
    app.update();
    let copy = texts(&mut app);
    assert!(copy.iter().any(|text| text == "Update Saved Loop"));
    assert!(copy.iter().any(|text| {
        text.starts_with(dtx_ui::StateMarker::Destructive.label())
            && text.contains("Delete Saved Loop")
    }));

    send_ui_action(&mut app, PracticeUiAction::SetCountIn(false));
    app.update();
    assert!(texts(&mut app)
        .iter()
        .any(|text| text == "Update Saved Loop"));

    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::WholeSong),
    );
    app.update();
    assert!(!texts(&mut app)
        .iter()
        .any(|text| text == "Update Saved Loop"));
}

#[test]
fn retry_action_reemits_the_exact_failed_command_and_cancel_clears_it() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let command = PresetCommand::SaveNew {
        name: Some("Named".to_owned()),
        draft: PracticeDraft {
            user_tempo: 0.75,
            ..Default::default()
        },
    };
    app.world_mut().write_message(PresetResult::Failed {
        message: "disk full".to_owned(),
        retry: Box::new(command.clone()),
    });
    app.update();
    assert!(texts(&mut app).iter().any(|text| text == "Retry Save"));

    app.world_mut().write_message(PresetResult::Failed {
        message: "permission denied".to_owned(),
        retry: Box::new(command.clone()),
    });
    app.update();
    assert!(texts(&mut app)
        .iter()
        .any(|text| text.starts_with(dtx_ui::StateMarker::Error.label())
            && text.contains("permission denied")));

    send_ui_action(&mut app, PracticeUiAction::RetryPreset);
    app.update();
    let commands: Vec<_> = app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert!(commands.contains(&command));

    send_ui_action(&mut app, PracticeUiAction::CancelPresetPrompt);
    app.update();
    assert!(!texts(&mut app).iter().any(|text| text == "Retry Save"));
}

#[test]
fn failed_save_keeps_draft_and_reports_retry() {
    let chart = dtx_config::PracticeChartKey::new("dtx1:test", 0);
    let mut store = PracticePresetStore::read_only(
        std::env::temp_dir().join("practice-hud-read-only.toml"),
        chart,
        None,
        dtx_config::PracticePresetRegistry::default(),
        "unsupported version",
    );
    let draft = PracticeDraft {
        user_tempo: 0.8,
        ..Default::default()
    };
    let original = draft.clone();

    let result = apply_preset_command(
        &mut store,
        PresetCommand::SaveNew {
            name: None,
            draft: draft.clone(),
        },
    );

    assert!(matches!(result, PresetResult::Failed { .. }));
    assert_eq!(draft, original);
    assert!(store.registry.presets.is_empty());
}

#[test]
fn progress_omits_ineligible_partial_attempt() {
    let session = PracticeSession {
        current_attempt_eligible: false,
        current_attempt: gameplay_drums::practice::session::AttemptStats {
            counts: gameplay_drums::resources::JudgmentCounts {
                perfect: 10,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(progress_rows(&session, 20_000)
        .iter()
        .all(|row| !row.contains("10")));
}

#[test]
fn progress_filters_exact_whole_song_span_and_formats_each_record_mode() {
    let mut session = PracticeSession::default();
    session.attempt_history.extend([
        AttemptRecord {
            start_ms: 0,
            end_ms: 10_000,
            tempo: 1.0,
            counts: default(),
            max_combo: 1,
            overhits: 0,
            accuracy_pct: 91.0,
            mean_error_ms: 2.0,
            waited: 1,
            flow_pct: 80.0,
            trainer_mode: PracticeTrainerMode::Wait,
        },
        AttemptRecord {
            start_ms: 1_000,
            end_ms: 10_000,
            tempo: 0.8,
            counts: default(),
            max_combo: 1,
            overhits: 0,
            accuracy_pct: 95.0,
            mean_error_ms: -1.0,
            waited: 0,
            flow_pct: 100.0,
            trainer_mode: PracticeTrainerMode::Ramp,
        },
    ]);
    session.trainer.disable();

    let rows = progress_rows(&session, 10_000);

    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("flow: 80.0%"), "{:?}", rows);
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut().query::<&T>().iter(app.world()).count()
}

fn click_tab(app: &mut App, label: &str) {
    let tab = match label {
        "Setup" => PracticeTab::Setup,
        "Progress" => PracticeTab::Progress,
        "Preview" => PracticeTab::Preview,
        _ => panic!("unknown practice tab {label}"),
    };
    app.world_mut().resource_mut::<PendingTabClick>().0 = Some(tab);
    app.update();
}

fn write_mouse_button(app: &mut App, state: bevy::input::ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("primary window");
    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: MouseButton::Left,
            state,
            window,
        });
}

fn computed_width<T: Component>(app: &mut App) -> f32 {
    app.world_mut()
        .query_filtered::<&ComputedNode, With<T>>()
        .single(app.world())
        .expect("one computed node")
        .size()
        .x
}

fn resize_surface(app: &mut App, width: f32, height: f32) {
    app.world_mut()
        .insert_resource(gameplay_drums::layout::PlayfieldLayout::from_size(
            width,
            height,
            &gameplay_drums::lanes::Lanes::default(),
        ));
    let physical_size = UVec2::new(width as u32, height as u32);
    let mut cameras = app.world_mut().query::<&mut Camera>();
    let mut camera = cameras.single_mut(app.world_mut()).expect("one UI camera");
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size,
        scale_factor: 1.0,
    });
    camera.viewport = Some(Viewport {
        physical_size,
        ..default()
    });
    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .resolution =
        WindowResolution::new(width as u32, height as u32).with_scale_factor_override(1.0);
}

fn node_rect(node: &ComputedNode, transform: &bevy::ui::UiGlobalTransform) -> Rect {
    let inverse_scale = node.inverse_scale_factor();
    Rect::from_center_size(
        transform.translation * inverse_scale,
        node.size() * inverse_scale,
    )
}

fn computed_rect<T: Component>(app: &mut App) -> Rect {
    app.world_mut()
        .query_filtered::<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<T>>()
        .single(app.world())
        .map(|(node, transform)| node_rect(node, transform))
        .expect("one computed node")
}

fn assert_semantic_heading_scale(app: &mut App, scale: dtx_config::TextScale) {
    let heading_px = app
        .world_mut()
        .query::<(&dtx_ui::SemanticText, &TextFont)>()
        .iter(app.world())
        .find_map(|(semantic, font)| {
            (semantic.0 == dtx_ui::TypographyRole::Heading).then_some(font.font_size)
        })
        .expect("semantic heading");
    assert_eq!(
        heading_px,
        bevy::text::FontSize::Px(dtx_ui::Typography.px(dtx_ui::TypographyRole::Heading, scale))
    );
}

fn assert_all_semantic_fonts_match_policy(app: &mut App, scale: dtx_config::TextScale) {
    let fonts: Vec<_> = app
        .world_mut()
        .query::<(&dtx_ui::SemanticText, &TextFont)>()
        .iter(app.world())
        .map(|(semantic, font)| (semantic.0, font.font_size))
        .collect();
    assert!(!fonts.is_empty());
    for (role, font_size) in fonts {
        assert_eq!(
            font_size,
            bevy::text::FontSize::Px(dtx_ui::Typography.px(role, scale)),
            "semantic {role:?} did not receive the live policy"
        );
    }
}

fn assert_preview_handoff_matches_computed_region(app: &mut App) {
    let preview = computed_rect::<PracticePreviewRegion>(app);
    let handoff = app
        .world()
        .resource::<PracticePreviewGeometry>()
        .0
        .expect("visible preview handoff");
    assert_eq!(handoff.origin, preview.min);
    assert_eq!(handoff.size, preview.size());

    let root = app
        .world_mut()
        .query_filtered::<Entity, With<PracticeSetupRoot>>()
        .single(app.world())
        .expect("one setup root");
    let children = app
        .world()
        .get::<Children>(root)
        .expect("setup root children");
    for &entity in [children[0], children[2]].iter() {
        let chrome_rect = {
            let node = app
                .world()
                .get::<ComputedNode>(entity)
                .expect("computed chrome node");
            let transform = app
                .world()
                .get::<bevy::ui::UiGlobalTransform>(entity)
                .expect("computed chrome transform");
            node_rect(node, transform)
        };
        let computed = app
            .world()
            .get::<ComputedNode>(entity)
            .expect("computed chrome node");
        assert!(
            computed.content_size().x <= computed.size().x + 1.0
                && computed.content_size().y <= computed.size().y + 1.0,
            "chrome content {:?} overflows node {:?}",
            computed.content_size(),
            computed.size()
        );
        let mut descendants = app
            .world()
            .get::<Children>(entity)
            .into_iter()
            .flat_map(|children| children.iter())
            .collect::<Vec<_>>();
        while let Some(child) = descendants.pop() {
            if let (Some(node), Some(transform)) = (
                app.world().get::<ComputedNode>(child),
                app.world().get::<bevy::ui::UiGlobalTransform>(child),
            ) {
                let rect = node_rect(node, transform);
                if rect.size().cmpgt(Vec2::ZERO).all() {
                    assert!(
                        rect.min.x >= chrome_rect.min.x - 1.0
                            && rect.min.y >= chrome_rect.min.y - 1.0
                            && rect.max.x <= chrome_rect.max.x + 1.0
                            && rect.max.y <= chrome_rect.max.y + 1.0,
                        "descendant {child:?} rect {rect:?} escapes chrome {chrome_rect:?}"
                    );
                }
            }
            if let Some(children) = app.world().get::<Children>(child) {
                descendants.extend(children.iter());
            }
        }
    }
}

fn assert_timeline_wraps(app: &mut App) {
    let strip = computed_rect::<PracticeTimelineStrip>(app);
    let button = app
        .world_mut()
        .query_filtered::<
            (&ComputedNode, &bevy::ui::UiGlobalTransform),
            With<PreviewTransportButton>,
        >()
        .iter(app.world())
        .next()
        .map(|(node, transform)| node_rect(node, transform))
        .expect("preview transport button");
    assert!(
        (strip.center().y - button.center().y).abs() > 1.0,
        "narrow transport and timeline must occupy separate rows"
    );
}

#[test]
fn reference_layout_is_split_and_xlarge_narrow_layout_is_tabbed() {
    assert_eq!(
        practice_layout_mode(1280.0, 720.0, 1.0),
        PracticeLayoutMode::Split
    );
    assert_eq!(
        practice_layout_mode(1920.0, 1080.0, 1.0),
        PracticeLayoutMode::Split
    );
    assert_eq!(
        practice_layout_mode(900.0, 720.0, 1.5),
        PracticeLayoutMode::Tabbed
    );
    assert_eq!(
        practice_layout_mode(900.0, 720.0, 1.0),
        PracticeLayoutMode::Tabbed
    );
    assert_eq!(
        practice_layout_mode(1920.0, 1080.0, 1.5),
        PracticeLayoutMode::Split
    );
}

#[test]
fn semantic_typography_keeps_preview_handoff_equal_to_unclipped_chrome_layout() {
    for (width, height, scale) in [
        (1280.0, 720.0, dtx_config::TextScale::Standard),
        (1920.0, 1080.0, dtx_config::TextScale::Standard),
        (1920.0, 1080.0, dtx_config::TextScale::XLarge),
    ] {
        let mut app = setup_hud_app(width, height, scale);
        assert_semantic_heading_scale(&mut app, scale);
        assert_preview_handoff_matches_computed_region(&mut app);
    }

    for scale in [
        dtx_config::TextScale::XLarge,
        dtx_config::TextScale::Standard,
    ] {
        let mut app = setup_hud_app(480.0, 720.0, scale);
        click_tab(&mut app, "Preview");
        app.update();
        assert_semantic_heading_scale(&mut app, scale);
        assert_preview_handoff_matches_computed_region(&mut app);
        assert_timeline_wraps(&mut app);
    }
}

#[test]
fn live_resize_and_text_scale_update_preview_geometry_in_the_same_frame() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::Standard);
    click_tab(&mut app, "Preview");

    resize_surface(&mut app, 480.0, 720.0);
    app.world_mut()
        .insert_resource(dtx_ui::AccessibilityPolicy::from(
            &dtx_config::AccessibilityConfig {
                text_scale: dtx_config::TextScale::XLarge,
                ..default()
            },
        ));
    app.update();

    assert_all_semantic_fonts_match_policy(&mut app, dtx_config::TextScale::XLarge);
    assert_preview_handoff_matches_computed_region(&mut app);
    let preview = computed_rect::<PracticePreviewRegion>(&mut app);
    let layout = app
        .world()
        .resource::<gameplay_drums::layout::PlayfieldLayout>();
    assert_eq!(layout.origin, preview.min);
    assert_eq!(Vec2::new(layout.width, layout.height), preview.size());
    assert_eq!(
        app.world().resource::<ConsumerLayout>().0,
        Some(gameplay_drums::stage_rect::StageRect {
            origin: preview.min,
            size: preview.size(),
        })
    );
}

#[test]
fn transport_row_structure_and_reserved_geometry_share_exact_breakpoints() {
    let breakpoint = gameplay_drums::practice::hud::setup::transport_single_row_min_width();
    assert_eq!(
        practice_transport_row_mode(breakpoint - 1.0),
        PracticeTransportRowMode::Stacked
    );
    assert_eq!(
        practice_transport_row_mode(breakpoint),
        PracticeTransportRowMode::Single
    );
    for (scale, below, above) in [
        (
            dtx_config::TextScale::Standard,
            breakpoint - 1.0,
            breakpoint + 1.0,
        ),
        (
            dtx_config::TextScale::XLarge,
            breakpoint - 1.0,
            breakpoint + 1.0,
        ),
    ] {
        let mut app = setup_hud_app(above, 720.0, scale);
        click_tab(&mut app, "Preview");

        let one_row_node = app
            .world_mut()
            .query_filtered::<&Node, With<PracticeTimelineRoot>>()
            .single(app.world())
            .expect("timeline root");
        assert_eq!(one_row_node.flex_direction, FlexDirection::Row);
        assert_eq!(one_row_node.flex_wrap, FlexWrap::NoWrap);
        let one_row_height = computed_rect::<PracticeTimelineRoot>(&mut app).height();
        let one_row_preview = computed_rect::<PracticePreviewRegion>(&mut app).height();
        let one_row_strip = computed_rect::<PracticeTimelineStrip>(&mut app);
        let one_row_button = app
            .world_mut()
            .query_filtered::<
                (&ComputedNode, &bevy::ui::UiGlobalTransform),
                With<PreviewTransportButton>,
            >()
            .iter(app.world())
            .next()
            .map(|(node, transform)| node_rect(node, transform))
            .expect("preview transport button");
        assert!((one_row_strip.center().y - one_row_button.center().y).abs() <= 1.0);

        resize_surface(&mut app, below, 720.0);
        app.update();

        let two_row_node = app
            .world_mut()
            .query_filtered::<&Node, With<PracticeTimelineRoot>>()
            .single(app.world())
            .expect("timeline root");
        assert_eq!(two_row_node.flex_direction, FlexDirection::Column);
        assert_eq!(two_row_node.flex_wrap, FlexWrap::NoWrap);
        let two_row_height = computed_rect::<PracticeTimelineRoot>(&mut app).height();
        let two_row_preview = computed_rect::<PracticePreviewRegion>(&mut app).height();
        assert_timeline_wraps(&mut app);
        assert_eq!(two_row_height - one_row_height, 24.0);
        assert_eq!(one_row_preview - two_row_preview, 24.0);
        assert_preview_handoff_matches_computed_region(&mut app);
        assert_all_semantic_fonts_match_policy(&mut app, scale);

        resize_surface(&mut app, above, 720.0);
        app.update();

        assert_eq!(
            computed_rect::<PracticeTimelineRoot>(&mut app).height(),
            one_row_height
        );
        assert_eq!(
            computed_rect::<PracticePreviewRegion>(&mut app).height(),
            one_row_preview
        );
        assert_preview_handoff_matches_computed_region(&mut app);
        assert_all_semantic_fonts_match_policy(&mut app, scale);
    }
}

#[test]
fn setup_shell_labels_preview_as_not_judged() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    assert!(texts(&mut app)
        .iter()
        .any(|text| text == "PREVIEW: INPUT IS NOT JUDGED"));
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 1);
}

#[test]
fn setup_shell_despawns_when_leaving_performance() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();

    assert_eq!(count::<PracticeSetupRoot>(&mut app), 0);
}

#[test]
fn split_shell_has_settings_preview_and_persistent_full_width_timeline() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    assert_eq!(count::<PracticeSettingsPane>(&mut app), 1);
    assert_eq!(count::<PracticePreviewRegion>(&mut app), 1);
    assert_eq!(count::<PracticeTimelineRoot>(&mut app), 1);
    let mode = app
        .world_mut()
        .query::<&PracticeSetupLayout>()
        .single(app.world())
        .expect("one setup layout");
    assert_eq!(mode.0, PracticeLayoutMode::Split);
}

#[test]
fn xlarge_narrow_shell_uses_tabs_without_hiding_timeline() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::XLarge);

    let mode = app
        .world_mut()
        .query::<&PracticeSetupLayout>()
        .single(app.world())
        .expect("one setup layout");
    assert_eq!(mode.0, PracticeLayoutMode::Tabbed);
    assert_eq!(count::<PracticeTimelineRoot>(&mut app), 1);
    assert!(texts(&mut app).iter().any(|text| text == "✓ Setup"));
    assert!(texts(&mut app).iter().any(|text| text == "Progress"));
    assert!(texts(&mut app).iter().any(|text| text == "Preview"));
}

#[test]
fn tabbed_layout_collapses_inactive_pane_and_visible_pane_fills_content() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::XLarge);

    assert!((computed_width::<PracticeSettingsPane>(&mut app) - 900.0).abs() <= 1.0);
    assert_eq!(computed_width::<PracticePreviewRegion>(&mut app), 0.0);

    click_tab(&mut app, "Preview");

    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 0.0);
    assert!((computed_width::<PracticePreviewRegion>(&mut app) - 900.0).abs() <= 1.0);
    assert!(texts(&mut app)
        .iter()
        .any(|text| text == "PREVIEW: INPUT IS NOT JUDGED"));
}

#[test]
fn tabbed_preview_keeps_navigation_visible_and_clickable_back_to_setup() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::XLarge);

    click_tab(&mut app, "Preview");
    assert_eq!(count::<PracticeTabButton>(&mut app), 3);
    let computed_tabs: Vec<_> = app
        .world_mut()
        .query::<(&PracticeTabButton, &ComputedNode)>()
        .iter(app.world())
        .map(|(tab, node)| (tab.0, node.size()))
        .collect();
    assert_eq!(computed_tabs.len(), 3);
    assert!(computed_tabs
        .iter()
        .all(|(_, size)| size.x > 0.0 && size.y > 0.0));
    assert!(texts(&mut app).iter().any(|text| text == "Setup"));
    assert!(texts(&mut app).iter().any(|text| text == "✓ Preview"));
    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 0.0);
    assert!((computed_width::<PracticePreviewRegion>(&mut app) - 900.0).abs() <= 1.0);

    click_tab(&mut app, "Setup");
    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
    assert!(texts(&mut app).iter().any(|text| text == "✓ Setup"));
    assert!((computed_width::<PracticeSettingsPane>(&mut app) - 900.0).abs() <= 1.0);
    assert_eq!(computed_width::<PracticePreviewRegion>(&mut app), 0.0);
}

#[test]
fn tab_click_reconciles_shell_and_chrome_in_one_update() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::XLarge);

    click_tab(&mut app, "Preview");

    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Preview);
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 1);
    assert_eq!(count::<PracticeTabButton>(&mut app), 3);
    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 0.0);
    assert!((computed_width::<PracticePreviewRegion>(&mut app) - 900.0).abs() <= 1.0);
    let preview = computed_rect::<PracticePreviewRegion>(&mut app);
    let layout = app
        .world()
        .resource::<gameplay_drums::layout::PlayfieldLayout>();
    assert_eq!(layout.origin, preview.min);
    assert_eq!(Vec2::new(layout.width, layout.height), preview.size());
}

#[test]
fn initial_shell_frame_owns_fitted_playfield_geometry() {
    let mut app = build_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    app.update();

    let preview = computed_rect::<PracticePreviewRegion>(&mut app);
    let layout = app
        .world()
        .resource::<gameplay_drums::layout::PlayfieldLayout>();
    assert_eq!(layout.origin, preview.min);
    assert_eq!(Vec2::new(layout.width, layout.height), preview.size());
    assert_eq!(
        app.world().resource::<ConsumerLayout>().0,
        Some(gameplay_drums::stage_rect::StageRect {
            origin: preview.min,
            size: preview.size(),
        })
    );
}

#[test]
fn first_resize_frame_owns_fitted_playfield_geometry() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    resize_surface(&mut app, 1920.0, 1080.0);
    app.update();

    let preview = computed_rect::<PracticePreviewRegion>(&mut app);
    let layout = app
        .world()
        .resource::<gameplay_drums::layout::PlayfieldLayout>();
    assert_eq!(layout.origin, preview.min);
    assert_eq!(Vec2::new(layout.width, layout.height), preview.size());
    assert_eq!(
        app.world().resource::<ConsumerLayout>().0,
        Some(gameplay_drums::stage_rect::StageRect {
            origin: preview.min,
            size: preview.size(),
        })
    );
}

#[test]
fn computed_playfield_and_drum_strip_fit_the_live_preview_region() {
    for (width, height, scale) in [
        (1280.0, 720.0, dtx_config::TextScale::Standard),
        (1920.0, 1080.0, dtx_config::TextScale::XLarge),
    ] {
        let mut app = setup_hud_app(width, height, scale);
        app.update();
        let preview = computed_rect::<PracticePreviewRegion>(&mut app);
        let layout = app
            .world()
            .resource::<gameplay_drums::layout::PlayfieldLayout>();
        let playfield = Rect::new(
            layout.backboard_left(),
            layout.backboard_top(),
            layout.backboard_left() + layout.backboard_width(),
            layout.backboard_top() + layout.backboard_height(),
        );
        let strip = Rect::new(
            layout.strip_left(),
            layout.lane_top(),
            layout.strip_left() + layout.strip_width(),
            layout.lane_top() + layout.lane_height(),
        );
        assert!(
            preview.contains(playfield.min),
            "{playfield:?} outside {preview:?}"
        );
        assert!(
            preview.contains(playfield.max),
            "{playfield:?} outside {preview:?}"
        );
        assert!(preview.contains(strip.min), "{strip:?} outside {preview:?}");
        assert!(preview.contains(strip.max), "{strip:?} outside {preview:?}");
        assert!(
            strip.width() >= 300.0,
            "drum strip is not usable: {strip:?}"
        );
        assert!(
            strip.height() >= 300.0,
            "playfield is not usable: {strip:?}"
        );
    }
}

#[test]
fn split_computed_widths_honor_layout_minima_without_horizontal_overflow() {
    for (width, height, scale, settings_min, preview_min) in [
        (1280.0, 720.0, dtx_config::TextScale::Standard, 400.0, 520.0),
        (1920.0, 1080.0, dtx_config::TextScale::XLarge, 900.0, 780.0),
    ] {
        let mut app = setup_hud_app(width, height, scale);
        let mut query = app.world_mut().query::<(
            Option<&PracticeSettingsPane>,
            Option<&PracticePreviewRegion>,
            &ComputedNode,
        )>();
        let (settings_width, settings_content, preview_width, preview_content) =
            query.iter(app.world()).fold(
                (0.0, 0.0, 0.0, 0.0),
                |mut sizes, (settings, preview, node)| {
                    if settings.is_some() {
                        sizes.0 = node.size().x;
                        sizes.1 = node.content_size().x;
                    }
                    if preview.is_some() {
                        sizes.2 = node.size().x;
                        sizes.3 = node.content_size().x;
                    }
                    sizes
                },
            );
        assert!(
            settings_width + 1.0 >= settings_min,
            "settings width {settings_width}"
        );
        assert!(
            preview_width + 1.0 >= preview_min,
            "preview width {preview_width}"
        );
        assert!(settings_content <= settings_width + 1.0);
        assert!(preview_content <= preview_width + 1.0);
        assert!((settings_width + preview_width - width).abs() <= 1.0);
    }
}

#[test]
fn resize_from_tabbed_preview_to_split_selects_setup_coherently() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::Standard);
    click_tab(&mut app, "Preview");
    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Preview);

    resize_surface(&mut app, 1280.0, 720.0);
    app.update();

    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 1);
    assert_eq!(count::<PracticeTabButton>(&mut app), 2);
    assert!(texts(&mut app).iter().any(|text| text == "✓ Setup"));
    assert!(!texts(&mut app).iter().any(|text| text == "✓ Preview"));
    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 400.0);

    resize_surface(&mut app, 900.0, 720.0);
    app.update();
    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 1);
    assert_eq!(count::<PracticeTabButton>(&mut app), 3);
    assert!(texts(&mut app).iter().any(|text| text == "✓ Setup"));
}

#[test]
fn split_resize_recomputes_pane_minima_without_a_mode_change() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 400.0);

    resize_surface(&mut app, 1920.0, 1080.0);
    app.world_mut()
        .insert_resource(dtx_ui::AccessibilityPolicy::from(
            &dtx_config::AccessibilityConfig {
                text_scale: dtx_config::TextScale::XLarge,
                ..default()
            },
        ));
    app.update();

    let mode = app
        .world_mut()
        .query::<&PracticeSetupLayout>()
        .single(app.world())
        .expect("one setup layout");
    assert_eq!(mode.0, PracticeLayoutMode::Split);
    assert_eq!(computed_width::<PracticeSettingsPane>(&mut app), 900.0);
    assert_eq!(computed_width::<PracticePreviewRegion>(&mut app), 1020.0);
}

#[test]
fn editing_shell_uses_continue_as_the_pinned_primary_action() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Editing;
    app.update();

    let action_text = app
        .world_mut()
        .query_filtered::<&Text, With<PracticePrimaryAction>>()
        .single(app.world())
        .expect("one primary action");
    assert_eq!(action_text.0, "Continue Practice");
}

#[test]
fn keyboard_traversal_selects_and_activates_the_pinned_primary_action() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);

    press_key(&mut app, KeyCode::ArrowUp);

    assert_eq!(
        app.world().resource::<SetupSelection>().0,
        SetupItem::StartOrContinue
    );
    let action_text = app
        .world_mut()
        .query_filtered::<&Text, With<PracticePrimaryAction>>()
        .single(app.world())
        .expect("one primary action");
    assert_eq!(
        action_text.0,
        format!("{} Start Practice", dtx_ui::StateMarker::Focus.label())
    );

    press_key(&mut app, KeyCode::Enter);
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::hud::setup_controls::StartOrContinueRequested>>()
        .iter_current_update_messages()
        .next()
        .is_some());
}

#[test]
fn keyboard_and_pad_tab_actions_share_the_tab_reducer() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::Standard);

    press_key(&mut app, KeyCode::Tab);
    assert_eq!(
        *app.world().resource::<PracticeTab>(),
        PracticeTab::Progress
    );

    send_nav(&mut app, game_shell::SystemVerb::NextTab);
    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Preview);

    press_key(&mut app, KeyCode::Space);
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .any(|action| *action == gameplay_drums::practice::PreviewAction::Play));

    let setup_selection = *app.world().resource::<SetupSelection>();
    send_nav(&mut app, game_shell::SystemVerb::Decrease);
    assert_eq!(*app.world().resource::<SetupSelection>(), setup_selection);
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .any(|action| *action == gameplay_drums::practice::PreviewAction::PrevBar));

    send_nav(&mut app, game_shell::SystemVerb::NavigateUp);
    assert_eq!(
        *app.world().resource::<PracticeTab>(),
        PracticeTab::Progress
    );
}

#[test]
fn every_practice_label_has_semantic_typography() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let missing = app
        .world_mut()
        .query::<(Entity, &Text, Option<&dtx_ui::SemanticText>)>()
        .iter(app.world())
        .filter_map(|(entity, text, semantic)| {
            semantic.is_none().then_some((entity, text.0.clone()))
        })
        .collect::<Vec<_>>();
    assert!(missing.is_empty(), "non-semantic labels: {missing:?}");
}

#[test]
fn standard_large_and_xlarge_keep_accessible_setup_structure() {
    for scale in [
        dtx_config::TextScale::Standard,
        dtx_config::TextScale::Large,
        dtx_config::TextScale::XLarge,
    ] {
        let mut app = setup_hud_app(1280.0, 720.0, scale);
        assert_eq!(count::<PracticeSetupRoot>(&mut app), 1, "{scale:?}");
        assert_eq!(count::<PracticeSettingsPane>(&mut app), 1, "{scale:?}");
        assert_eq!(count::<PracticeTimelineRoot>(&mut app), 1, "{scale:?}");
        assert_all_semantic_fonts_match_policy(&mut app, scale);
        assert_preview_handoff_matches_computed_region(&mut app);
    }
}

#[test]
fn running_toast_and_countdown_scale_semantically_in_the_spawn_update() {
    for scale in [
        dtx_config::TextScale::Standard,
        dtx_config::TextScale::Large,
        dtx_config::TextScale::XLarge,
    ] {
        let mut app = setup_hud_app(1280.0, 720.0, scale);
        app.world_mut().resource_mut::<PracticeFlow>().phase =
            gameplay_drums::practice::PracticePhase::Running;
        app.world_mut()
            .resource_mut::<gameplay_drums::practice::toast::ToastQueue>()
            .push("Practice saved");
        app.add_systems(
            Update,
            (
                gameplay_drums::practice::toast::toast_ui,
                gameplay_drums::practice::metronome::spawn_countdown,
            )
                .chain()
                .before(dtx_ui::SemanticTypographyUpdate),
        );

        app.update();

        let toast_font = app
            .world_mut()
            .query_filtered::<
                (&dtx_ui::SemanticText, &TextFont),
                With<gameplay_drums::practice::toast::ToastText>,
            >()
            .iter(app.world())
            .next()
            .map(|(semantic, font)| (semantic.0, font.font_size))
            .expect("semantic practice toast");
        assert_eq!(toast_font.0, dtx_ui::TypographyRole::Label);
        assert_eq!(
            toast_font.1,
            bevy::text::FontSize::Px(dtx_ui::Typography.px(dtx_ui::TypographyRole::Label, scale))
        );

        let countdown_font = app
            .world_mut()
            .query_filtered::<
                (&dtx_ui::SemanticText, &TextFont),
                With<gameplay_drums::practice::metronome::CountdownText>,
            >()
            .single(app.world())
            .map(|(semantic, font)| (semantic.0, font.font_size))
            .expect("semantic countdown");
        assert_eq!(countdown_font.0, dtx_ui::TypographyRole::Display);
        assert_eq!(
            countdown_font.1,
            bevy::text::FontSize::Px(dtx_ui::Typography.px(dtx_ui::TypographyRole::Display, scale))
        );
    }
}

#[test]
fn focused_selected_destructive_error_and_disabled_states_have_markers() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    press_key(&mut app, KeyCode::ArrowUp);
    let labels = texts(&mut app);
    assert!(labels.iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Focus.label()) && label.contains("Start Practice")
    }));
    assert!(labels.iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Selected.label()) && label.contains("Setup")
    }));
    assert!(labels.iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Disabled.label())
            && label.contains("Update Saved Loop")
    }));
    assert!(labels.iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Disabled.label())
            && label.contains("Delete Saved Loop")
    }));

    let chart = dtx_config::PracticeChartKey::new("dtx1:markers", 0);
    let mut registry = dtx_config::PracticePresetRegistry::default();
    let id = registry
        .create(
            chart.clone(),
            Some("Marker Loop"),
            None,
            dtx_config::PracticePresetConfig {
                loop_start_ms: None,
                loop_end_ms: None,
                snap: dtx_config::PracticeSnapPreset::Bar,
                tempo: 1.0,
                preroll: dtx_config::PracticePrerollPreset::OneBar,
                count_in: true,
                trainer: dtx_config::PracticeTrainerPreset::Off,
            },
        )
        .expect("valid preset");
    app.world_mut().insert_resource(PracticePresetStore::ready(
        std::env::temp_dir().join("practice-marker-state.toml"),
        chart,
        None,
        registry,
    ));
    send_ui_action(
        &mut app,
        PracticeUiAction::SelectSource(PracticeDraftSource::Saved(id)),
    );
    app.update();
    assert!(texts(&mut app).iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Destructive.label())
            && label.contains("Delete Saved Loop")
    }));

    let retry_draft = app.world().resource::<PracticeDraft>().clone();
    app.world_mut().write_message(PresetResult::Failed {
        message: "Could not save preset".to_owned(),
        retry: Box::new(PresetCommand::SaveNew {
            name: None,
            draft: retry_draft,
        }),
    });
    app.update();
    assert!(texts(&mut app).iter().any(|label| {
        label.starts_with(dtx_ui::StateMarker::Error.label())
            && label.contains("Could not save preset")
    }));
}

#[test]
fn setup_progress_preview_and_running_show_context_legends() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::Standard);
    let setup = texts(&mut app).join(" ");
    assert!(setup.contains("Adjust"), "{setup}");
    assert!(setup.contains("Start Practice"), "{setup}");

    click_tab(&mut app, "Progress");
    let progress = texts(&mut app).join(" ");
    assert!(progress.contains("Progress"), "{progress}");
    assert!(progress.contains("Setup"), "{progress}");

    click_tab(&mut app, "Preview");
    let preview = texts(&mut app).join(" ");
    assert!(preview.contains("Play Preview"), "{preview}");
    assert!(preview.contains("Previous bar"), "{preview}");

    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Running;
    app.update();
    let running = texts(&mut app).join(" ");
    assert!(
        running.contains("Esc") && running.contains("Pause"),
        "{running}"
    );
    assert!(
        running.contains("Tab") && running.contains("Settings"),
        "{running}"
    );
}

#[test]
fn setup_motion_uses_out_quint_and_reduced_motion_starts_at_final_position() {
    let mut standard =
        setup_hud_app_with_accessibility(1280.0, 720.0, dtx_config::AccessibilityConfig::default());
    let transition = standard
        .world_mut()
        .query::<&PracticePanelTransition>()
        .single(standard.world())
        .expect("animated setup panel");
    assert_eq!(transition.easing, dtx_ui::easing::EaseFunction::OutQuint);
    assert!(transition.translation_px > 0.0);

    let mut reduced = setup_hud_app_with_accessibility(
        1280.0,
        720.0,
        dtx_config::AccessibilityConfig {
            reduce_motion: true,
            reduce_flashes: true,
            ..Default::default()
        },
    );
    assert_eq!(count::<PracticePanelTransition>(&mut reduced), 0);
    let transform = reduced
        .world_mut()
        .query_filtered::<&UiTransform, With<PracticeSettingsPane>>()
        .single(reduced.world())
        .expect("settings transform");
    assert_eq!(transform.translation, bevy::ui::Val2::ZERO);
    assert_eq!(count::<PracticeSetupRoot>(&mut reduced), 1);
}

fn ancestor_with<T: Component>(app: &App, mut entity: Entity) -> bool {
    loop {
        if app.world().get::<T>(entity).is_some() {
            return true;
        }
        let Some(parent) = app.world().get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}

#[test]
fn split_keyboard_and_pad_reach_every_transport_without_mutating_settings() {
    for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        for pad in [false, true] {
            let mut app = setup_hud_app(width, height, dtx_config::TextScale::Standard);
            let original = app.world().resource::<PracticeDraft>().clone();

            if pad {
                send_nav(&mut app, game_shell::SystemVerb::NextTab);
            } else {
                press_key(&mut app, KeyCode::Tab);
            }
            assert_eq!(
                *app.world().resource::<PracticeSurfaceFocus>(),
                PracticeSurfaceFocus::Preview(PreviewTransportButton::Back)
            );
            assert!(texts(&mut app).iter().any(|label| {
                label.starts_with(dtx_ui::StateMarker::Focus.label()) && label.contains("Back")
            }));
            assert_eq!(
                texts(&mut app)
                    .iter()
                    .filter(|label| label.starts_with(dtx_ui::StateMarker::Focus.label()))
                    .count(),
                1,
                "only the active Split surface may carry the focus marker"
            );
            if pad {
                send_nav(&mut app, game_shell::SystemVerb::Confirm);
            } else {
                press_key(&mut app, KeyCode::Enter);
            }
            assert!(app
                .world()
                .resource::<Messages<gameplay_drums::practice::InitialSetupCancelRequested>>()
                .iter_current_update_messages()
                .next()
                .is_some());

            for (button, action) in [
                (
                    PreviewTransportButton::PrevBar,
                    gameplay_drums::practice::PreviewAction::PrevBar,
                ),
                (
                    PreviewTransportButton::PlayPause,
                    gameplay_drums::practice::PreviewAction::Play,
                ),
                (
                    PreviewTransportButton::NextBar,
                    gameplay_drums::practice::PreviewAction::NextBar,
                ),
            ] {
                if pad {
                    send_nav(&mut app, game_shell::SystemVerb::Increase);
                    send_nav(&mut app, game_shell::SystemVerb::Confirm);
                } else {
                    press_key(&mut app, KeyCode::ArrowRight);
                    press_key(&mut app, KeyCode::Enter);
                }
                assert_eq!(
                    *app.world().resource::<PracticeSurfaceFocus>(),
                    PracticeSurfaceFocus::Preview(button)
                );
                assert!(app
                    .world()
                    .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
                    .iter_current_update_messages()
                    .any(|seen| *seen == action));
            }

            app.world_mut().resource_mut::<PracticeFlow>().preview =
                gameplay_drums::practice::PreviewState::Playing;
            if pad {
                send_nav(&mut app, game_shell::SystemVerb::Decrease);
                send_nav(&mut app, game_shell::SystemVerb::Confirm);
            } else {
                press_key(&mut app, KeyCode::ArrowLeft);
                press_key(&mut app, KeyCode::Enter);
            }
            assert!(app
                .world()
                .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
                .iter_current_update_messages()
                .any(|seen| *seen == gameplay_drums::practice::PreviewAction::Pause));

            if pad {
                send_nav(&mut app, game_shell::SystemVerb::NextTab);
            } else {
                press_key(&mut app, KeyCode::Tab);
            }
            assert_eq!(
                *app.world().resource::<PracticeSurfaceFocus>(),
                PracticeSurfaceFocus::Settings
            );
            assert_eq!(
                texts(&mut app)
                    .iter()
                    .filter(|label| label.starts_with(dtx_ui::StateMarker::Focus.label()))
                    .count(),
                1,
                "focus must return exclusively to the settings cursor"
            );
            assert_eq!(*app.world().resource::<PracticeDraft>(), original);
            assert_eq!(count::<PracticeSettingsPane>(&mut app), 1);

            if pad {
                send_nav(&mut app, game_shell::SystemVerb::Back);
            } else {
                press_key(&mut app, KeyCode::Escape);
            }
            assert!(app
                .world()
                .resource::<Messages<gameplay_drums::practice::InitialSetupCancelRequested>>()
                .iter_current_update_messages()
                .next()
                .is_some());
        }
    }
}

fn assert_split_progress_transport(mut app: App, pad: bool) {
    let original = app.world().resource::<PracticeDraft>().clone();
    app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::Tempo;
    click_tab(&mut app, "Progress");
    let layout = app
        .world_mut()
        .query_filtered::<&PracticeSetupLayout, With<PracticeSetupRoot>>()
        .single(app.world())
        .expect("practice layout")
        .0;
    assert_eq!(layout, PracticeLayoutMode::Split);
    assert_eq!(
        *app.world().resource::<PracticeTab>(),
        PracticeTab::Progress
    );
    assert_eq!(
        *app.world().resource::<PracticeSurfaceFocus>(),
        PracticeSurfaceFocus::Settings
    );

    if pad {
        send_nav(&mut app, game_shell::SystemVerb::NextTab);
    } else {
        press_key(&mut app, KeyCode::Tab);
    }
    assert_eq!(
        *app.world().resource::<PracticeTab>(),
        PracticeTab::Progress
    );
    assert_eq!(
        *app.world().resource::<PracticeSurfaceFocus>(),
        PracticeSurfaceFocus::Preview(PreviewTransportButton::Back)
    );

    if pad {
        send_nav(&mut app, game_shell::SystemVerb::Increase);
        send_nav(&mut app, game_shell::SystemVerb::Increase);
        send_nav(&mut app, game_shell::SystemVerb::Confirm);
    } else {
        press_key(&mut app, KeyCode::ArrowRight);
        press_key(&mut app, KeyCode::ArrowRight);
        press_key(&mut app, KeyCode::Enter);
    }
    assert_eq!(
        *app.world().resource::<PracticeSurfaceFocus>(),
        PracticeSurfaceFocus::Preview(PreviewTransportButton::PlayPause)
    );
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .any(|seen| *seen == gameplay_drums::practice::PreviewAction::Play));
    assert_eq!(*app.world().resource::<PracticeDraft>(), original);

    if pad {
        send_nav(&mut app, game_shell::SystemVerb::NextTab);
        send_nav(&mut app, game_shell::SystemVerb::Confirm);
    } else {
        press_key(&mut app, KeyCode::Tab);
        press_key(&mut app, KeyCode::Enter);
    }
    assert_eq!(
        *app.world().resource::<PracticeSurfaceFocus>(),
        PracticeSurfaceFocus::Settings
    );
    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
    assert_eq!(*app.world().resource::<PracticeDraft>(), original);
}

#[test]
fn split_progress_keyboard_focuses_and_activates_persistent_preview_transport() {
    for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        assert_split_progress_transport(
            setup_hud_app(width, height, dtx_config::TextScale::Standard),
            false,
        );
    }
}

#[test]
fn split_progress_pad_focuses_and_activates_persistent_preview_transport() {
    for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        assert_split_progress_transport(
            setup_hud_app(width, height, dtx_config::TextScale::Standard),
            true,
        );
    }
}

#[test]
fn progress_hides_primary_action_and_cannot_focus_or_activate_it() {
    for width in [900.0, 1280.0, 1920.0] {
        let mut app = setup_hud_app(width, 720.0, dtx_config::TextScale::Standard);
        app.world_mut().resource_mut::<SetupSelection>().0 = SetupItem::StartOrContinue;
        click_tab(&mut app, "Progress");

        assert_eq!(count::<PracticePrimaryAction>(&mut app), 0);
        assert_ne!(
            app.world().resource::<SetupSelection>().0,
            SetupItem::StartOrContinue
        );
        assert!(!texts(&mut app)
            .iter()
            .any(|label| label.contains("Start Practice")));

        press_key(&mut app, KeyCode::Enter);
        assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
        assert!(app
            .world()
            .resource::<Messages<gameplay_drums::practice::hud::setup_controls::StartOrContinueRequested>>()
            .iter_current_update_messages()
            .next()
            .is_none());

        click_tab(&mut app, "Progress");
        send_nav(&mut app, game_shell::SystemVerb::Confirm);
        assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
        assert!(app
            .world()
            .resource::<Messages<gameplay_drums::practice::hud::setup_controls::StartOrContinueRequested>>()
            .iter_current_update_messages()
            .next()
            .is_none());
    }
}

#[test]
fn panel_entry_motion_only_marks_fresh_surface_entries() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    assert_eq!(count::<PracticePanelTransition>(&mut app), 1);
    app.world_mut()
        .query::<&mut PracticePanelTransition>()
        .single_mut(app.world_mut())
        .expect("panel transition")
        .elapsed_ms = 180.0;
    app.update();
    assert_eq!(count::<PracticePanelTransition>(&mut app), 0);

    app.world_mut().resource_mut::<PracticeDraft>().trainer.mode = PracticeTrainerMode::Ramp;
    app.update();
    assert_eq!(count::<PracticePanelTransition>(&mut app), 0);
    assert_eq!(count::<PracticeTabCrossfade>(&mut app), 0);
    app.world_mut().resource_mut::<PracticeDraft>().source = PracticeDraftSource::Saved(7);
    app.update();
    assert_eq!(count::<PracticePanelTransition>(&mut app), 0);
    app.world_mut()
        .insert_resource(PracticePresetPrompt::ConfirmDelete { id: 7 });
    app.update();
    assert_eq!(count::<PracticePanelTransition>(&mut app), 0);

    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Running;
    app.update();
    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Editing;
    app.update();
    assert_eq!(count::<PracticePanelTransition>(&mut app), 1);
}

#[test]
fn tab_crossfade_belongs_to_the_new_visible_surface_and_cleans_up() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::Standard);
    assert_eq!(count::<PracticeTabCrossfade>(&mut app), 0);

    click_tab(&mut app, "Preview");
    let fade = app
        .world_mut()
        .query_filtered::<Entity, With<PracticeTabCrossfade>>()
        .single(app.world())
        .expect("preview crossfade");
    assert!(ancestor_with::<PracticePreviewRegion>(&app, fade));

    app.world_mut()
        .query::<&mut PracticeTabCrossfade>()
        .single_mut(app.world_mut())
        .expect("tab crossfade")
        .elapsed_ms = 140.0;
    app.update();
    assert_eq!(count::<PracticeTabCrossfade>(&mut app), 0);

    click_tab(&mut app, "Setup");
    let fade = app
        .world_mut()
        .query_filtered::<Entity, With<PracticeTabCrossfade>>()
        .single(app.world())
        .expect("settings crossfade");
    assert!(ancestor_with::<PracticeSettingsPane>(&app, fade));
    app.world_mut()
        .query::<&mut PracticeTabCrossfade>()
        .single_mut(app.world_mut())
        .expect("setup crossfade")
        .elapsed_ms = 140.0;
    app.update();

    click_tab(&mut app, "Progress");
    app.world_mut()
        .query::<&mut PracticeTabCrossfade>()
        .single_mut(app.world_mut())
        .expect("progress crossfade")
        .elapsed_ms = 140.0;
    app.update();
    click_tab(&mut app, "Preview");
    let fade = app
        .world_mut()
        .query_filtered::<Entity, With<PracticeTabCrossfade>>()
        .single(app.world())
        .expect("progress to preview crossfade");
    assert!(ancestor_with::<PracticePreviewRegion>(&app, fade));
    app.world_mut()
        .query::<&mut PracticeTabCrossfade>()
        .single_mut(app.world_mut())
        .expect("preview crossfade")
        .elapsed_ms = 140.0;
    app.update();
    click_tab(&mut app, "Progress");
    let fade = app
        .world_mut()
        .query_filtered::<Entity, With<PracticeTabCrossfade>>()
        .single(app.world())
        .expect("preview to progress crossfade");
    assert!(ancestor_with::<PracticeSettingsPane>(&app, fade));

    let mut reduced = setup_hud_app_with_accessibility(
        900.0,
        720.0,
        dtx_config::AccessibilityConfig {
            reduce_motion: true,
            ..Default::default()
        },
    );
    click_tab(&mut reduced, "Preview");
    assert_eq!(count::<PracticeTabCrossfade>(&mut reduced), 0);
    click_tab(&mut reduced, "Progress");
    assert_eq!(count::<PracticeTabCrossfade>(&mut reduced), 0);
}

#[test]
fn legacy_practice_surface_symbols_are_gone() {
    let legacy_symbols = [
        ["full", "_hud"].concat(),
        ["FULL", "_HUD"].concat(),
        ["PracticePause", "Surface"].concat(),
        ["practice ", "ra", "il"].concat(),
    ];
    for source in [
        include_str!("../src/practice/hud/mod.rs"),
        include_str!("../src/practice/hud/setup.rs"),
        include_str!("../src/practice/hud/timeline_ui.rs"),
        include_str!("../src/practice/session.rs"),
        include_str!("../src/ui_z.rs"),
    ] {
        for symbol in &legacy_symbols {
            assert!(!source.contains(symbol));
        }
    }
}

#[test]
fn setup_copy_refreshes_from_draft_in_the_same_update() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    {
        let mut draft = app.world_mut().resource_mut::<PracticeDraft>();
        draft.source = PracticeDraftSource::Custom;
        draft.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        draft.user_tempo = 0.75;
        draft.snap = SnapDivisor::Beat;
        draft.preroll = gameplay_drums::practice::session::PrerollSetting::Off;
        draft.count_in = false;
        draft.trainer.mode = PracticeTrainerMode::Wait;
    }
    app.update();

    let copy = texts(&mut app);
    for expected in [
        "✓ Custom",
        "0:02.0",
        "0:06.0",
        "0.75×",
        "Beat",
        "off",
        "Off",
        "Wait",
    ] {
        assert!(
            copy.iter().any(|text| text == expected),
            "missing {expected:?} in {copy:?}"
        );
    }
}

#[test]
fn progress_copy_refreshes_from_session_metrics() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    click_tab(&mut app, "Progress");
    assert!(texts(&mut app)
        .iter()
        .any(|text| text == "No completed attempts yet"));

    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.transport.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        session.attempt_history.push(AttemptRecord {
            start_ms: 2_000,
            end_ms: 6_000,
            tempo: 0.8,
            counts: default(),
            max_combo: 20,
            overhits: 1,
            accuracy_pct: 93.5,
            mean_error_ms: -12.0,
            waited: 0,
            flow_pct: 100.0,
            trainer_mode: PracticeTrainerMode::Off,
        });
        session
            .lane_diag
            .apply_judgment(0, dtx_scoring::JudgmentKind::Perfect, -18);
    }
    app.update();

    let copy = texts(&mut app);
    assert!(copy
        .iter()
        .any(|text| text == "Latest: 93.5% at 0.80×, timing -12 ms"));
    assert!(copy
        .iter()
        .any(|text| text.contains("HH") && text.contains("rushing")));
}

#[test]
fn timeline_markers_follow_the_draft_not_the_committed_session() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 10_000;
    app.world_mut().resource_mut::<PracticeDraft>().loop_region =
        Some(gameplay_drums::practice::session::LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
    app.world_mut()
        .resource_mut::<PracticeSession>()
        .transport
        .loop_region = Some(gameplay_drums::practice::session::LoopRegion {
        start_ms: 7_000,
        end_ms: 9_000,
    });
    app.update();

    let (node, visibility) = app
        .world_mut()
        .query_filtered::<(&Node, &Visibility), With<PracticeLoopFill>>()
        .single(app.world())
        .expect("one loop marker");
    assert_eq!(node.left, Val::Percent(20.0));
    assert_eq!(node.width, Val::Percent(40.0));
    assert_eq!(*visibility, Visibility::Visible);

    let mut handles: Vec<_> = app
        .world_mut()
        .query::<(&PracticeLoopHandle, &Node, &Visibility)>()
        .iter(app.world())
        .map(|(handle, node, visibility)| (*handle, node.left, node.right, *visibility))
        .collect();
    handles.sort_by_key(|(handle, _, _, _)| match handle {
        PracticeLoopHandle::Start => 0,
        PracticeLoopHandle::End => 1,
    });
    assert_eq!(
        handles,
        vec![
            (
                PracticeLoopHandle::Start,
                Val::Percent(20.0),
                Val::Auto,
                Visibility::Visible
            ),
            (
                PracticeLoopHandle::End,
                Val::Auto,
                Val::Percent(40.0),
                Visibility::Visible
            ),
        ]
    );
}

#[test]
fn real_timeline_drag_updates_draft_and_setup_copy_in_the_same_frame() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    {
        let mut timeline = app.world_mut().resource_mut::<ChipTimeline>();
        timeline.end_ms = 16_000;
        timeline.bar_ms = (0..=8).map(|bar| bar * 2_000).collect();
        timeline.beat_ms = (0..=32).map(|beat| beat * 500).collect();
    }
    app.update();
    let strip_rect = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<PracticeTimelineStrip>>()
        .single(app.world())
        .map(|(node, transform)| node_rect(node, transform))
        .expect("computed timeline strip");
    let y = strip_rect.center().y;
    let start = Vec2::new(strip_rect.min.x + strip_rect.width() * 0.25, y);
    let end = Vec2::new(strip_rect.min.x + strip_rect.width() * 0.50, y);

    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(Some(start));
    write_mouse_button(&mut app, bevy::input::ButtonState::Pressed);
    app.update();

    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(Some(end));
    app.update();

    assert_eq!(
        app.world().resource::<PracticeDraft>().loop_region,
        Some(LoopRegion {
            start_ms: 4_000,
            end_ms: 8_000,
        })
    );
    let copy = texts(&mut app);
    assert!(copy.iter().any(|text| text == "✓ Custom"));
    assert!(copy.iter().any(|text| text == "0:04.0"));
    assert!(copy.iter().any(|text| text == "0:08.0"));
}

#[test]
fn real_timeline_click_emits_snapped_preview_seek() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    {
        let mut timeline = app.world_mut().resource_mut::<ChipTimeline>();
        timeline.end_ms = 16_000;
        timeline.bar_ms = (0..=8).map(|bar| bar * 2_000).collect();
    }
    app.update();
    let strip_rect = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<PracticeTimelineStrip>>()
        .single(app.world())
        .map(|(node, transform)| node_rect(node, transform))
        .expect("computed timeline strip");
    let cursor = Vec2::new(
        strip_rect.min.x + strip_rect.width() * 0.30,
        strip_rect.center().y,
    );
    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(Some(cursor));
    write_mouse_button(&mut app, bevy::input::ButtonState::Pressed);
    app.update();
    write_mouse_button(&mut app, bevy::input::ButtonState::Released);
    app.update();

    let actions: Vec<_> = app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(
        actions,
        vec![gameplay_drums::practice::PreviewAction::Seek(4_000)]
    );
}

#[test]
fn end_handle_computed_geometry_stays_inside_timeline_strip() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 16_000;
    app.world_mut().resource_mut::<PracticeDraft>().loop_region = Some(LoopRegion {
        start_ms: 4_000,
        end_ms: 16_000,
    });
    app.update();

    let strip = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<PracticeTimelineStrip>>()
        .single(app.world())
        .map(|(node, transform)| node_rect(node, transform))
        .expect("timeline strip");
    let end = app
        .world_mut()
        .query::<(
            &PracticeLoopHandle,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(app.world())
        .find_map(|(handle, node, transform)| {
            (*handle == PracticeLoopHandle::End).then(|| node_rect(node, transform))
        })
        .expect("end handle");
    assert!(end.min.x >= strip.min.x - 1.0);
    assert!(
        end.max.x <= strip.max.x + 1.0,
        "end {end:?}, strip {strip:?}"
    );
    assert!(end.width() >= 20.0);
}

#[test]
fn preview_transport_buttons_keep_task_five_preview_actions() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut()
        .spawn((Interaction::Pressed, PreviewTransportButton::NextBar));
    app.update();

    let actions: Vec<_> = app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(
        actions,
        vec![gameplay_drums::practice::PreviewAction::NextBar]
    );
}

#[test]
fn visible_transport_back_button_routes_through_the_shared_back_action() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let buttons = app
        .world_mut()
        .query::<&PreviewTransportButton>()
        .iter(app.world())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 4);
    for expected in [
        PreviewTransportButton::Back,
        PreviewTransportButton::PrevBar,
        PreviewTransportButton::PlayPause,
        PreviewTransportButton::NextBar,
    ] {
        assert!(buttons.contains(&expected));
    }

    app.world_mut()
        .spawn((Interaction::Pressed, PreviewTransportButton::Back));
    app.update();

    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::InitialSetupCancelRequested>>()
        .iter_current_update_messages()
        .next()
        .is_some());

    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Editing;
    app.world_mut()
        .spawn((Interaction::Pressed, PreviewTransportButton::Back));
    app.update();
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::CancelPracticeSettings>>()
        .iter_current_update_messages()
        .next()
        .is_some());
}

#[test]
fn compact_running_hud_is_hidden_during_setup_and_restored_for_running() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    let compact_visibility = |app: &mut App| {
        let mini = app
            .world_mut()
            .query_filtered::<Option<&Visibility>, With<gameplay_drums::practice::hud::mini_strip::MiniStripRoot>>()
            .single(app.world())
            .expect("mini strip")
            .copied();
        let chip = app
            .world_mut()
            .query_filtered::<Option<&Visibility>, With<gameplay_drums::practice::hud::chip::StatusChip>>()
            .single(app.world())
            .expect("status chip");
        (mini, chip.copied())
    };
    assert_eq!(
        compact_visibility(&mut app),
        (Some(Visibility::Hidden), Some(Visibility::Hidden))
    );

    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Running;
    app.update();
    assert_eq!(
        compact_visibility(&mut app),
        (Some(Visibility::Inherited), Some(Visibility::Inherited))
    );
    assert_eq!(count::<PracticeSetupRoot>(&mut app), 0);
    assert_eq!(count::<PracticeTimelineRoot>(&mut app), 0);
    let layout = app
        .world()
        .resource::<gameplay_drums::layout::PlayfieldLayout>();
    assert_eq!(layout.origin, Vec2::ZERO);
    assert_eq!(layout.width, 1280.0);
    assert_eq!(layout.height, 720.0);
}

#[test]
fn fresh_performance_setup_resets_tab_while_editing_retains_it() {
    let mut app = setup_hud_app(900.0, 720.0, dtx_config::TextScale::XLarge);
    click_tab(&mut app, "Progress");
    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Editing;
    app.update();
    assert_eq!(
        *app.world().resource::<PracticeTab>(),
        PracticeTab::Progress
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    app.update();
    app.world_mut().resource_mut::<PracticeFlow>().phase =
        gameplay_drums::practice::PracticePhase::Setup;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.update();

    assert_eq!(*app.world().resource::<PracticeTab>(), PracticeTab::Setup);
}

#[test]
fn performance_exit_clears_pending_timeline_gesture_before_reentry() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    *app.world_mut().resource_mut::<TimelineGesture>() = TimelineGesture::Pending {
        press_x: 420.0,
        press_ms: 4_000,
    };

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );
}

#[test]
fn performance_exit_clears_drag_timeline_gesture_before_reentry() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    *app.world_mut().resource_mut::<TimelineGesture>() =
        TimelineGesture::DragLoop { anchor_ms: 4_000 };

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );
}

#[test]
fn releasing_timeline_outside_does_not_leave_a_stale_seek_on_reentry() {
    let mut app = setup_hud_app(1280.0, 720.0, dtx_config::TextScale::Standard);
    app.world_mut().resource_mut::<ChipTimeline>().end_ms = 16_000;
    app.update();
    let strip = computed_rect::<PracticeTimelineStrip>(&mut app);
    let inside = strip.center();
    let outside = Vec2::new(strip.max.x + 20.0, strip.center().y);
    let mut window = app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window");
    window.set_cursor_position(Some(inside));
    write_mouse_button(&mut app, bevy::input::ButtonState::Pressed);
    app.update();
    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(Some(outside));
    write_mouse_button(&mut app, bevy::input::ButtonState::Released);
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );
    assert_eq!(
        app.world()
            .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
            .iter_current_update_messages()
            .count(),
        0
    );

    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(None);
    app.world_mut()
        .resource_mut::<TimelineGesture>()
        .clone_from(&TimelineGesture::Pending {
            press_x: inside.x,
            press_ms: 4_000,
        });
    app.update();
    assert_eq!(
        *app.world().resource::<TimelineGesture>(),
        TimelineGesture::Idle
    );

    app.world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window")
        .set_cursor_position(Some(inside));
    write_mouse_button(&mut app, bevy::input::ButtonState::Pressed);
    app.update();
    write_mouse_button(&mut app, bevy::input::ButtonState::Released);
    app.update();
    let actions: Vec<_> = app
        .world()
        .resource::<Messages<gameplay_drums::practice::PreviewAction>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(actions.len(), 1, "only the fresh re-entry click may seek");
}
