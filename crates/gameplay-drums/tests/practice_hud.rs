use bevy::prelude::*;
use game_shell::{AppState, PauseState};
use gameplay_drums::practice::hud::setup::{
    practice_layout_mode, PracticeLayoutMode, PracticePreviewRegion, PracticePrimaryAction,
    PracticeSettingsPane, PracticeSetupLayout, PracticeSetupRoot,
};
use gameplay_drums::practice::hud::timeline_ui::{
    PracticeLoopFill, PracticeLoopHandle, PracticeTimelineRoot, PreviewTransportButton,
};
use gameplay_drums::practice::{PracticeDraft, PracticeFlow, PracticeSession};
use gameplay_drums::resources::GameplayClock;
use gameplay_drums::timeline::ChipTimeline;

fn setup_hud_app(width: f32, height: f32, text_scale: dtx_config::TextScale) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
    ))
    .init_state::<AppState>()
    .init_state::<PauseState>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_message::<gameplay_drums::practice::actions::PracticeAction>()
    .add_message::<gameplay_drums::practice::PreviewAction>()
    .init_resource::<GameplayClock>()
    .init_resource::<ChipTimeline>()
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

    gameplay_drums::practice::hud::plugin(&mut app);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.update();
    app
}

fn texts(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect()
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut().query::<&T>().iter(app.world()).count()
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

    let handles: Vec<_> = app
        .world_mut()
        .query::<(&PracticeLoopHandle, &Node, &Visibility)>()
        .iter(app.world())
        .map(|(handle, node, visibility)| (*handle, node.left, *visibility))
        .collect();
    assert_eq!(
        handles,
        vec![
            (
                PracticeLoopHandle::Start,
                Val::Percent(20.0),
                Visibility::Visible
            ),
            (
                PracticeLoopHandle::End,
                Val::Percent(60.0),
                Visibility::Visible
            ),
        ]
    );
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
}
