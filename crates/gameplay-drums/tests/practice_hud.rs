use bevy::camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use game_shell::{AppState, PauseState};
use gameplay_drums::practice::hud::setup::{
    practice_layout_mode, update_tab_selection, PracticeLayoutMode, PracticePreviewRegion,
    PracticePrimaryAction, PracticeSettingsPane, PracticeSetupLayout, PracticeSetupRoot,
    PracticeTab, PracticeTabButton,
};
use gameplay_drums::practice::hud::timeline_ui::{
    PracticeLoopFill, PracticeLoopHandle, PracticeTimelineRoot, PracticeTimelineStrip,
    PreviewTransportButton, TimelineGesture,
};
use gameplay_drums::practice::session::{AttemptRecord, LoopRegion};
use gameplay_drums::practice::{
    PracticeDraft, PracticeDraftSource, PracticeFlow, PracticeSession, PracticeTrainerMode,
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
