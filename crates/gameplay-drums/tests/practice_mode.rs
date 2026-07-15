//! Integration tests for practice mode: seek, gates, loop.

use bevy::prelude::*;
use dtx_audio::BgmHandle;
use dtx_core::assets::DtxAssets;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip, Metadata};
use game_shell::{AppState, PracticeIntent, PracticeOrigin};
use gameplay_drums::components::{LastJudgment, Note, NoteVisual};
use gameplay_drums::events::{InputHit, JudgmentEvent, LaneHit, NoteMissed};
use gameplay_drums::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use gameplay_drums::orchestrator::{
    detect_end_of_stage, enter_derive_from_chart, enter_reset_run_state, enter_seed_bgm_state,
    DrumsStageCompletion,
};
use gameplay_drums::practice::session::{LoopRegion, PracticeSession, PracticeTransport};
use gameplay_drums::practice::{
    cancel_initial_setup, start_or_continue_practice, CancelPracticeSettings, OpenPracticeSettings,
    PracticeDraft, PracticeEditSnapshot, PracticeFlow, PracticePhase, PresetCommand, PreviewAction,
    PreviewState,
};
use gameplay_drums::resources::{
    AccuracyHistory, ActiveChart, BgmAdjustState, Combo, EffectivePlaybackRate, GameStartMs,
    GameplayClock, JudgmentCounts, MetronomeEnabled, PlaybackRateSource, Score, ShowTimingLines,
    TimingLineCrossed,
};
use gameplay_drums::se_scheduler::PlayedSeChips;
use gameplay_drums::seek::SeekToChartTime;
use gameplay_drums::timeline::build_chip_timeline;

fn chart_with_measures(n: u32) -> Chart {
    let chips: Vec<Chip> = (0..n)
        .map(|i| Chip::new(i, EChannel::BassDrum, 1.0))
        .collect();
    Chart {
        metadata: Metadata::default(),
        chips,
        ..Default::default()
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        bevy_kira_audio::AudioPlugin,
    ))
    .init_state::<AppState>()
    .init_resource::<DrumsStageCompletion>()
    .init_resource::<GameplayClock>()
    .init_resource::<EffectivePlaybackRate>()
    .init_resource::<game_shell::CompletedRunContext>()
    .init_resource::<ActiveChart>()
    .init_resource::<Score>()
    .init_resource::<gameplay_drums::resources::DrumScoring>()
    .init_resource::<Combo>()
    .init_resource::<JudgmentCounts>()
    .init_resource::<gameplay_drums::resources::DrumGameplaySettings>()
    .init_resource::<gameplay_drums::resources::DrumAudioSettings>()
    .init_resource::<JudgedChips>()
    .init_resource::<LastJudgment>()
    .init_resource::<GameStartMs>()
    .init_resource::<BgmAdjustState>()
    .init_resource::<BpmChangeList>()
    .init_resource::<BarLengthChangeList>()
    .init_resource::<BgmHandle>()
    .init_resource::<dtx_audio::ChartSoundBank>()
    .init_resource::<dtx_audio::DrumPolyphony>()
    .init_resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
    .init_resource::<gameplay_drums::bgm_scheduler::PrimaryBgmChip>()
    .init_resource::<gameplay_drums::bgm_scheduler::BgmRecoveryState>()
    .init_resource::<gameplay_drums::resources::CurrentEmptyHitTemplates>()
    .init_resource::<gameplay_drums::resources::ActiveDrumSounds>()
    .init_resource::<gameplay_drums::se_scheduler::PlayedSeChips>()
    .init_resource::<gameplay_drums::resources::FastSlowCount>()
    .init_resource::<gameplay_drums::resources::SkillValue>()
    .init_resource::<gameplay_drums::derived::ChartDerived>()
    .init_resource::<gameplay_drums::resources::TimingLineCrossed>()
    .init_resource::<gameplay_drums::timeline::ChipTimeline>()
    .init_resource::<gameplay_drums::seek::PendingBgmStart>()
    .init_resource::<gameplay_drums::seek::PendingAudioStarts>()
    .init_resource::<gameplay_drums::seek::LastSeekFrom>()
    .init_resource::<gameplay_drums::seek::PreviewSkippedChips>()
    .init_resource::<gameplay_drums::seek::StoppedSeekRebuild>()
    .init_resource::<dtx_bga::BgaClock>()
    .init_resource::<gameplay_drums::pause::PracticePauseSurface>()
    .init_resource::<game_shell::EditorSession>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_systems(
        OnEnter(AppState::Performance),
        (
            enter_reset_run_state,
            enter_derive_from_chart,
            enter_seed_bgm_state,
            build_chip_timeline,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (gameplay_drums::seek::apply_seek_system, detect_end_of_stage)
            .chain()
            .run_if(in_state(AppState::Performance)),
    );
    app
}

fn enter_performance(app: &mut App, chart: Chart) {
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
}

fn build_lifecycle_app(intent: PracticeIntent) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::state::app::StatesPlugin,
    ))
    .add_plugins(dtx_audio::plugin)
    .add_plugins(dtx_timing::plugin)
    .add_plugins(game_shell::GameShellPlugin)
    .insert_resource(intent)
    .init_resource::<game_shell::EGameMode>()
    .add_plugins(gameplay_drums::DrumsPlugin);
    app
}

fn chart_with_scheduled_audio() -> Chart {
    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "chart-bgm.wav".into());
    assets.wav.insert(2, "chart-se.wav".into());
    Chart {
        metadata: Metadata::default(),
        chips: vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::SE01, 0.0, 2),
            Chip::new(1, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn chart_for_preview_seek() -> Chart {
    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "preview-bgm.wav".into());
    assets.wav.insert(2, "preview-se.wav".into());
    Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::SE01, 0.0, 2),
            Chip::new(0, EChannel::BassDrum, 0.5),
            Chip::new(1, EChannel::Snare, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn chart_with_mixer_limited_bgm() -> Chart {
    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "mixer-bgm.wav".into());
    Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::MixerAdd, 0.0, 1),
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(1, EChannel::MixerRemove, 0.0, 1),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn chart_with_overlapping_audio_slices() -> Chart {
    let mut assets = DtxAssets::default();
    for (slot, path) in [
        (1, "primary.wav"),
        (2, "layer.wav"),
        (3, "old-se.wav"),
        (4, "new-se.wav"),
    ] {
        assets.wav.insert(slot, path.into());
    }
    assets
        .wav
        .volumes
        .extend([(1, 37), (2, 48), (3, 59), (4, 63)]);
    assets
        .wav
        .pans
        .extend([(1, -41), (2, 22), (3, -13), (4, 34)]);
    Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::BGM, 0.5, 2),
            Chip::with_wav(0, EChannel::SE01, 0.25, 3),
            Chip::with_wav(0, EChannel::SE01, 0.5, 4),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn chart_with_repeated_primary_path() -> Chart {
    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "shared-bgm.wav".into());
    assets.wav.insert(2, "shared-bgm.wav".into());
    assets.wav.insert(3, "distinct-layer.wav".into());
    assets.wav.volumes.insert(1, 37);
    assets.wav.pans.insert(1, -41);
    Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::BGM, 0.25, 2),
            Chip::with_wav(0, EChannel::BGM, 0.5, 3),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn chart_with_ineligible_primary_and_same_path_layer() -> Chart {
    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "shared-bgm.wav".into());
    assets.wav.insert(2, "shared-bgm.wav".into());
    assets.wav.volumes.insert(2, 48);
    assets.wav.pans.insert(2, 22);
    Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::MixerAdd, 0.0, 1),
            Chip::with_wav(0, EChannel::MixerAdd, 0.0, 2),
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::BGM, 0.25, 2),
            Chip::with_wav(1, EChannel::MixerRemove, 0.0, 1),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    }
}

fn ready_clock(app: &mut App, current_ms: i64) {
    let mut clock = app.world_mut().resource_mut::<GameplayClock>();
    clock.start();
    clock.sync(Some(current_ms));
}

fn queue_bass_drum_midi(app: &mut App) {
    app.world_mut()
        .insert_resource(gameplay_drums::bindings::BindResolver::default());
    app.world_mut()
        .resource_mut::<dtx_input::midi::VirtualSource>()
        .note_on(36, 100, 0);
}

fn queue_key_cap_flash_messages(app: &mut App) {
    app.world_mut().write_message(LaneHit {
        lane: 2,
        audio_ms: 2_000,
    });
    app.world_mut().write_message(InputHit {
        lanes: vec![2],
        audio_ms: 2_000,
        captured_at: std::time::Instant::now(),
    });
    app.world_mut().write_message(JudgmentEvent {
        lane: 2,
        kind: dtx_scoring::JudgmentKind::Perfect,
        delta_ms: 0,
        chip_idx: 0,
    });
}

fn bass_drum_key_cap_color(app: &mut App) -> Color {
    let col = app
        .world()
        .resource::<gameplay_drums::lanes::Lanes>()
        .col_of(EChannel::BassDrum)
        .expect("Bass Drum has a visual column");
    let world = app.world_mut();
    let mut caps = world.query::<(&gameplay_drums::keyboard_viz::KeyCap, &BackgroundColor)>();
    caps.iter(world)
        .find_map(|(cap, background)| (cap.col as usize == col).then_some(background.0))
        .expect("Bass Drum key cap exists")
}

fn phrase_playhead_top(app: &mut App) -> f32 {
    let world = app.world_mut();
    let mut playhead =
        world.query_filtered::<&Node, With<dtx_ui::widget::phrase_meter::PhrasePlayhead>>();
    let node = playhead.single(world).expect("phrase playhead exists");
    match node.top {
        Val::Px(top) => top,
        other => panic!("phrase playhead top must use pixels, got {other:?}"),
    }
}

#[test]
fn setup_every_practice_intent_enters_stopped_without_seeking_or_attempting() {
    for origin in [
        PracticeOrigin::SongSelect,
        PracticeOrigin::Results,
        PracticeOrigin::NormalPause,
    ] {
        let mut app = build_lifecycle_app(PracticeIntent::manual(origin));
        enter_performance(&mut app, chart_with_measures(4));

        let flow = app.world().resource::<PracticeFlow>();
        assert_eq!(flow.phase, PracticePhase::Setup, "{origin:?}");
        assert_eq!(flow.preview, PreviewState::Stopped, "{origin:?}");
        assert_eq!(flow.origin, origin, "{origin:?}");
        assert!(flow.edit_snapshot.is_none(), "{origin:?}");
        assert!(app.world().contains_resource::<PracticeDraft>());
        let session = app.world().resource::<PracticeSession>();
        assert!(session.attempt_history.is_empty(), "{origin:?}");
        assert_eq!(session.current_attempt.start_ms, 0, "{origin:?}");
        assert!(!session.current_attempt_eligible, "{origin:?}");
        assert!(app
            .world()
            .resource::<Messages<SeekToChartTime>>()
            .is_empty());
    }
}

#[test]
fn setup_stopped_suppresses_fallback_bgm_but_normal_play_starts_it() {
    let dir = std::env::temp_dir().join(format!(
        "dtxmaniars-setup-fallback-bgm-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create fixture directory");
    let chart_path = dir.join("chart.dtx");
    std::fs::write(&chart_path, b"#TITLE: Setup fallback\n").expect("write fixture chart");
    std::fs::write(dir.join("bgm.wav"), b"").expect("write fallback BGM marker");

    let mut practice = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    practice
        .world_mut()
        .resource_mut::<ActiveChart>()
        .source_path = Some(chart_path.clone());
    enter_performance(&mut practice, chart_with_measures(4));
    assert!(practice.world().resource::<BgmHandle>().path.is_none());

    let mut normal = build_lifecycle_app(PracticeIntent::None);
    normal.world_mut().resource_mut::<ActiveChart>().source_path = Some(chart_path);
    enter_performance(&mut normal, chart_with_measures(4));
    assert_eq!(
        normal.world().resource::<BgmHandle>().path.as_deref(),
        Some(dir.join("bgm.wav").to_string_lossy().as_ref())
    );

    std::fs::remove_dir_all(dir).expect("remove fixture directory");
}

#[test]
fn setup_stopped_gates_chart_audio_schedulers_until_preview() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_scheduled_audio());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(100));
    }

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app
        .world()
        .resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
        .0
        .is_empty());
    assert!(app.world().resource::<PlayedSeChips>().0.is_empty());
    assert!(app.world().resource::<BgmHandle>().path.is_none());

    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world()
            .resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
            .0
            .len(),
        1
    );
    assert_eq!(app.world().resource::<PlayedSeChips>().0.len(), 1);
    assert_eq!(
        app.world().resource::<BgmHandle>().path.as_deref(),
        Some("chart-bgm.wav")
    );
}

#[test]
fn setup_stopped_gates_global_beat_metronome_but_normal_play_still_clicks() {
    let mut setup = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut setup, chart_with_measures(1));
    setup.world_mut().resource_mut::<ShowTimingLines>().0 = true;
    setup.world_mut().resource_mut::<MetronomeEnabled>().0 = true;
    ready_clock(&mut setup, 100);

    setup.world_mut().run_schedule(FixedUpdate);

    assert!(setup.world().resource::<TimingLineCrossed>().0.is_empty());

    let mut normal = build_lifecycle_app(PracticeIntent::None);
    enter_performance(&mut normal, chart_with_measures(1));
    normal.world_mut().resource_mut::<ShowTimingLines>().0 = true;
    normal.world_mut().resource_mut::<MetronomeEnabled>().0 = true;
    ready_clock(&mut normal, 100);

    normal.world_mut().run_schedule(FixedUpdate);

    assert!(!normal.world().resource::<TimingLineCrossed>().0.is_empty());
}

#[test]
fn setup_ready_midi_does_not_emit_gameplay_input() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(1));
    ready_clock(&mut app, 2_000);
    queue_bass_drum_midi(&mut app);

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app.world().resource::<Messages<InputHit>>().is_empty());
}

#[test]
fn editing_ready_midi_does_not_emit_gameplay_input() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(1));
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    ready_clock(&mut app, 2_000);
    queue_bass_drum_midi(&mut app);

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app.world().resource::<Messages<InputHit>>().is_empty());
}

#[test]
fn setup_ready_midi_keeps_raw_hit_and_pad_navigation() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(1));
    ready_clock(&mut app, 2_000);
    queue_bass_drum_midi(&mut app);

    app.world_mut().run_schedule(FixedUpdate);

    let last = app.world().resource::<gameplay_drums::LastMidiHit>();
    assert_eq!((last.note, last.velocity), (36, 100));
    let nav_hits = app
        .world()
        .resource::<Messages<gameplay_drums::PadNavHit>>()
        .iter_current_update_messages()
        .collect::<Vec<_>>();
    assert_eq!(nav_hits.len(), 1);
    assert_eq!(nav_hits[0].lane, 2);
}

fn assert_stale_gameplay_messages_do_not_flash_key_caps(phase: PracticePhase) {
    let rest = Color::srgb(0.11, 0.11, 0.13);
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(1));
    app.world_mut().resource_mut::<PracticeFlow>().phase = phase;
    assert_eq!(bass_drum_key_cap_color(&mut app), rest, "{phase:?}");

    queue_key_cap_flash_messages(&mut app);
    app.world_mut().run_schedule(Update);

    assert_eq!(bass_drum_key_cap_color(&mut app), rest, "{phase:?}");
}

#[test]
fn setup_stale_gameplay_messages_do_not_flash_key_caps() {
    assert_stale_gameplay_messages_do_not_flash_key_caps(PracticePhase::Setup);
}

#[test]
fn editing_stale_gameplay_messages_do_not_flash_key_caps() {
    assert_stale_gameplay_messages_do_not_flash_key_caps(PracticePhase::Editing);
}

#[test]
fn running_and_normal_play_gameplay_messages_flash_key_caps() {
    let rest = Color::srgb(0.11, 0.11, 0.13);
    for intent in [
        PracticeIntent::manual(PracticeOrigin::SongSelect),
        PracticeIntent::None,
    ] {
        let mut app = build_lifecycle_app(intent);
        enter_performance(&mut app, chart_with_measures(1));
        if let Some(mut flow) = app.world_mut().get_resource_mut::<PracticeFlow>() {
            flow.phase = PracticePhase::Running;
        }

        queue_key_cap_flash_messages(&mut app);
        app.world_mut().run_schedule(Update);

        assert_ne!(bass_drum_key_cap_color(&mut app), rest);
    }
}

#[test]
fn preview_clock_movement_does_not_sample_accuracy_but_moves_playhead() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    *app.world_mut().resource_mut::<AccuracyHistory>() = AccuracyHistory::default();
    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    let playhead_before = phrase_playhead_top(&mut app);
    ready_clock(&mut app, 2_000);

    app.world_mut().run_schedule(Update);

    assert!(app
        .world()
        .resource::<AccuracyHistory>()
        .samples
        .iter()
        .all(Option::is_none));
    assert_ne!(phrase_playhead_top(&mut app), playhead_before);
}

#[test]
fn running_and_normal_play_clock_movement_samples_accuracy_history() {
    for intent in [
        PracticeIntent::manual(PracticeOrigin::SongSelect),
        PracticeIntent::None,
    ] {
        let mut app = build_lifecycle_app(intent);
        enter_performance(&mut app, chart_with_measures(4));
        *app.world_mut().resource_mut::<AccuracyHistory>() = AccuracyHistory::default();
        if let Some(mut flow) = app.world_mut().get_resource_mut::<PracticeFlow>() {
            flow.phase = PracticePhase::Running;
        }
        ready_clock(&mut app, 2_000);

        app.world_mut().run_schedule(Update);

        assert!(app
            .world()
            .resource::<AccuracyHistory>()
            .samples
            .iter()
            .any(Option::is_some));
    }
}

#[test]
fn setup_unready_clock_drops_bound_confirmation_key_before_start() {
    use bevy::ecs::system::RunSystemOnce;

    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "later-bgm.wav".into());
    let chart = Chart {
        metadata: Metadata::default(),
        chips: vec![
            Chip::new(0, EChannel::BassDrum, 0.0),
            Chip::with_wav(2, EChannel::BGM, 0.0, 1),
        ],
        assets,
        ..Default::default()
    };
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart);
    assert!(app
        .world()
        .resource::<GameplayClock>()
        .is_waiting_for_audio());
    app.world_mut()
        .insert_resource(gameplay_drums::bindings::BindResolver::default());

    let window = app.world_mut().spawn_empty().id();
    app.world_mut()
        .write_message(bevy::input::keyboard::KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: bevy::input::keyboard::Key::Space,
            state: bevy::input::ButtonState::Pressed,
            text: Some(" ".into()),
            repeat: false,
            window,
        });
    app.world_mut().run_schedule(PreUpdate);

    app.world_mut()
        .run_system_once(start_or_continue_practice)
        .expect("start system runs");
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    app.world_mut().run_schedule(FixedUpdate);

    assert!(
        app.world()
            .resource::<Messages<gameplay_drums::events::InputHit>>()
            .is_empty(),
        "the stopped-Setup keyboard capture must not survive into Running"
    );
    assert!(
        app.world().resource::<JudgedChips>().0.is_empty(),
        "the bound Space confirmation captured while the clock was unready must not judge after Start"
    );
}

#[test]
fn setup_start_clears_a_preexisting_pending_keyboard_capture() {
    use bevy::ecs::system::RunSystemOnce;

    let mut assets = DtxAssets::default();
    assets.wav.insert(1, "later-bgm.wav".into());
    let chart = Chart {
        metadata: Metadata::default(),
        chips: vec![
            Chip::new(0, EChannel::BassDrum, 0.0),
            Chip::with_wav(2, EChannel::BGM, 0.0, 1),
        ],
        assets,
        ..Default::default()
    };
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart);
    app.world_mut()
        .insert_resource(gameplay_drums::bindings::BindResolver::default());
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Running;

    let window = app.world_mut().spawn_empty().id();
    app.world_mut()
        .write_message(bevy::input::keyboard::KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: bevy::input::keyboard::Key::Space,
            state: bevy::input::ButtonState::Pressed,
            text: Some(" ".into()),
            repeat: false,
            window,
        });
    app.world_mut().run_schedule(PreUpdate);
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Setup;

    app.world_mut()
        .run_system_once(start_or_continue_practice)
        .expect("start system runs");
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    app.world_mut().run_schedule(FixedUpdate);

    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::events::InputHit>>()
        .is_empty());
    assert!(app.world().resource::<JudgedChips>().0.is_empty());
}

#[test]
fn setup_preview_gates_autoplay_without_mutating_judged_chips() {
    let mut chart = chart_with_measures(1);
    chart.chips.push(Chip::new(0, EChannel::BGALayer1, 0.0));
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart);
    app.world_mut()
        .resource_mut::<gameplay_drums::autoplay::AutoplayEnabled>()
        .0 = true;
    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app.world().resource::<Messages<LaneHit>>().is_empty());
    assert!(app.world().resource::<JudgedChips>().0.is_empty());
}

#[test]
fn setup_entry_resets_count_in_state() {
    use gameplay_drums::practice::metronome::{
        ActiveClickSchedule, Click, ClickSchedule, CountdownDisplay,
    };

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    app.world_mut()
        .resource_mut::<ActiveClickSchedule>()
        .schedule = ClickSchedule {
        clicks: vec![Click {
            at_ms: 0,
            accent: true,
            beats_remaining: 1,
        }],
    };
    app.world_mut().resource_mut::<CountdownDisplay>().current = Some((1, true, 0.0));
    enter_performance(&mut app, chart_with_measures(4));
    assert!(app
        .world()
        .resource::<ActiveClickSchedule>()
        .schedule
        .clicks
        .is_empty());
    assert!(app.world().resource::<CountdownDisplay>().current.is_none());
}

#[test]
fn setup_preview_seek_does_not_schedule_or_fire_count_in() {
    use bevy::ecs::system::RunSystemOnce;
    use gameplay_drums::practice::metronome::{ActiveClickSchedule, CountdownDisplay};

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));

    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.transport.metronome = true;
        session.transport.preroll = gameplay_drums::practice::session::PrerollSetting::OneBar;
    }
    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    app.world_mut().write_message(SeekToChartTime {
        target_ms: 4_000,
        snap: None,
        attempt_start_ms: None,
    });

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app
        .world()
        .resource::<ActiveClickSchedule>()
        .schedule
        .clicks
        .is_empty());
    assert!(app.world().resource::<CountdownDisplay>().current.is_none());

    {
        let mut draft = app.world_mut().resource_mut::<PracticeDraft>();
        draft.loop_region = Some(LoopRegion {
            start_ms: 4_000,
            end_ms: 6_000,
        });
        draft.preroll = gameplay_drums::practice::session::PrerollSetting::OneBar;
        draft.count_in = true;
    }
    app.world_mut()
        .run_system_once(start_or_continue_practice)
        .expect("start system runs");
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world()
            .resource::<ActiveClickSchedule>()
            .schedule
            .clicks
            .len(),
        4,
        "Start creates the committed attempt's one-bar count-in"
    );
}

fn assert_preview_seek_preserves_gameplay_reconstruction(phase: PracticePhase) {
    use gameplay_drums::practice::metronome::{ActiveClickSchedule, CountdownDisplay};

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_for_preview_seek());
    {
        let mut flow = app.world_mut().resource_mut::<PracticeFlow>();
        flow.phase = phase;
        flow.preview = PreviewState::Playing;
    }
    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.current_attempt.start_ms = 321;
        session.current_attempt.counts.perfect = 7;
        session.current_attempt_eligible = false;
    }
    app.world_mut().resource_mut::<JudgedChips>().0 = [3, 99].into();
    ready_clock(&mut app, 250);
    app.world_mut().spawn((
        Note {
            chip_id: 77,
            lane: 2,
            target_ms: 250,
        },
        NoteVisual,
        Node::default(),
    ));

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world().resource::<JudgedChips>().0,
        std::collections::HashSet::from([3, 99]),
        "{phase:?} preview seek must preserve gameplay judgment state"
    );
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::bgm_scheduler::PlayedBgmChips>()
            .0,
        std::collections::HashSet::from([0])
    );
    assert_eq!(
        app.world().resource::<PlayedSeChips>().0,
        std::collections::HashSet::from([1])
    );
    assert!(!app.world().resource::<TimingLineCrossed>().0.is_empty());
    assert!(app.world().resource::<GameplayClock>().current_ms >= 1_500);
    assert_eq!(
        app.world()
            .resource::<PracticeSession>()
            .current_attempt
            .counts
            .perfect,
        7
    );
    assert_eq!(
        app.world()
            .resource::<PracticeSession>()
            .current_attempt
            .start_ms,
        321
    );
    assert!(
        !app.world()
            .resource::<PracticeSession>()
            .current_attempt_eligible
    );
    assert!(app
        .world()
        .resource::<PracticeSession>()
        .attempt_history
        .is_empty());
    assert!(app
        .world()
        .resource::<ActiveClickSchedule>()
        .schedule
        .clicks
        .is_empty());
    assert!(app.world().resource::<CountdownDisplay>().current.is_none());

    app.world_mut().run_schedule(Update);

    assert_eq!(
        app.world().resource::<dtx_bga::BgaClock>().current_ms,
        app.world().resource::<GameplayClock>().current_ms
    );
    let visible_ids = {
        let world = app.world_mut();
        let mut notes = world.query_filtered::<&Note, With<NoteVisual>>();
        notes
            .iter(world)
            .map(|note| note.chip_id)
            .collect::<Vec<_>>()
    };
    assert!(
        visible_ids.contains(&3),
        "{phase:?} preview visuals must not inherit the gameplay sentinel"
    );
    assert!(!visible_ids.contains(&77), "seek must clear stale visuals");

    send_seek(&mut app, 500);
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(Update);

    assert_eq!(
        app.world().resource::<JudgedChips>().0,
        std::collections::HashSet::from([3, 99])
    );
    let visible_ids = {
        let world = app.world_mut();
        let mut notes = world.query_filtered::<&Note, With<NoteVisual>>();
        notes
            .iter(world)
            .map(|note| note.chip_id)
            .collect::<Vec<_>>()
    };
    assert!(
        visible_ids.contains(&2),
        "backward preview seek must rebuild visual eligibility"
    );
}

#[test]
fn setup_preview_seek_preserves_gameplay_reconstruction() {
    assert_preview_seek_preserves_gameplay_reconstruction(PracticePhase::Setup);
}

#[test]
fn editing_preview_seek_preserves_gameplay_reconstruction() {
    assert_preview_seek_preserves_gameplay_reconstruction(PracticePhase::Editing);
}

#[test]
fn stopped_setup_and_editing_seek_reconstruct_notes_and_bga_once() {
    for phase in [PracticePhase::Setup, PracticePhase::Editing] {
        let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
        app.init_asset::<Image>();
        app.add_plugins(dtx_bga::plugin);
        enter_performance(&mut app, chart_for_preview_seek());
        app.world_mut().resource_mut::<PracticeFlow>().phase = phase;
        ready_clock(&mut app, 250);
        app.world_mut().insert_resource(dtx_bga::ActiveChartRes {
            source_dir: None,
            events: vec![dtx_bga::TimedVisualEvent::replace(
                1_000,
                dtx_core::bga::BgaLayer::Layer1,
                1,
            )],
            bmp_paths: std::collections::HashMap::new(),
            avi_paths: std::collections::HashMap::new(),
        });

        send_seek(&mut app, 1_500);
        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(app.world().resource::<GameplayClock>().current_ms, 1_500);
        assert_eq!(
            app.world().resource::<dtx_bga::BgaClock>().current_ms,
            1_500
        );
        let visible_ids = {
            let world = app.world_mut();
            let mut notes = world.query_filtered::<&Note, With<NoteVisual>>();
            notes
                .iter(world)
                .map(|note| note.chip_id)
                .collect::<Vec<_>>()
        };
        assert!(
            visible_ids.contains(&3),
            "{phase:?} stopped seek rebuilds notes"
        );

        app.update();
        assert_eq!(
            app.world().resource::<dtx_bga::BgaPlayer>().next_event_idx,
            1
        );
        let held = app.world().resource::<GameplayClock>().current_ms;
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().resource::<GameplayClock>().current_ms, held);
    }
}

fn assert_seek_past_mixer_remove_stops_bgm(intent: PracticeIntent, phase: Option<PracticePhase>) {
    let mut app = build_lifecycle_app(intent);
    enter_performance(&mut app, chart_with_mixer_limited_bgm());
    if let Some(phase) = phase {
        let mut flow = app.world_mut().resource_mut::<PracticeFlow>();
        flow.phase = phase;
        if phase != PracticePhase::Running {
            flow.preview = PreviewState::Playing;
        }
    }
    ready_clock(&mut app, 500);
    {
        let mut bgm = app.world_mut().resource_mut::<BgmHandle>();
        bgm.instance = Some(Handle::default());
        bgm.path = Some("mixer-bgm.wav".into());
    }

    send_seek(&mut app, 2_500);
    app.world_mut().run_schedule(FixedUpdate);

    let bgm = app.world().resource::<BgmHandle>();
    assert!(
        bgm.instance.is_none(),
        "the pre-seek BGM handle must be stopped"
    );
    assert!(bgm.path.is_none(), "ineligible BGM must not be restarted");
    assert!(app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .is_none());
    assert!(!app
        .world()
        .resource::<gameplay_drums::mixer_events::MixerEligibility>()
        .is_slot_eligible(1));
    assert!(app.world().resource::<GameplayClock>().current_ms >= 2_500);
}

#[test]
fn setup_preview_seek_past_mixer_remove_stops_bgm() {
    assert_seek_past_mixer_remove_stops_bgm(
        PracticeIntent::manual(PracticeOrigin::SongSelect),
        Some(PracticePhase::Setup),
    );
}

#[test]
fn editing_preview_seek_past_mixer_remove_stops_bgm() {
    assert_seek_past_mixer_remove_stops_bgm(
        PracticeIntent::manual(PracticeOrigin::SongSelect),
        Some(PracticePhase::Editing),
    );
}

#[test]
fn running_practice_seek_past_mixer_remove_stops_bgm() {
    assert_seek_past_mixer_remove_stops_bgm(
        PracticeIntent::manual(PracticeOrigin::SongSelect),
        Some(PracticePhase::Running),
    );
}

#[test]
fn normal_play_seek_past_mixer_remove_stops_bgm() {
    assert_seek_past_mixer_remove_stops_bgm(PracticeIntent::None, None);
}

#[test]
fn eligible_seek_restarts_bgm_at_resolved_position() {
    let mut app = build_lifecycle_app(PracticeIntent::None);
    enter_performance(&mut app, chart_with_mixer_limited_bgm());
    ready_clock(&mut app, 500);
    {
        let mut bgm = app.world_mut().resource_mut::<BgmHandle>();
        bgm.instance = Some(Handle::default());
        bgm.path = Some("stale-bgm.wav".into());
    }

    send_seek(&mut app, 1_000);
    app.world_mut().run_schedule(FixedUpdate);

    let bgm = app.world().resource::<BgmHandle>();
    assert!(bgm.instance.is_some());
    assert_eq!(bgm.path.as_deref(), Some("mixer-bgm.wav"));
    assert_eq!(app.world().resource::<GameStartMs>().0, 0);
    assert!(app
        .world()
        .resource::<gameplay_drums::mixer_events::MixerEligibility>()
        .is_slot_eligible(1));
    assert!(app.world().resource::<GameplayClock>().current_ms >= 1_000);
}

#[test]
fn stopped_cancel_seek_queues_all_spanning_audio_with_choke() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_overlapping_audio_slices());
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Running;
    ready_clock(&mut app, 1_500);

    app.world_mut().write_message(OpenPracticeSettings);
    app.update();
    app.world_mut().write_message(PreviewAction::Seek(250));
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().write_message(CancelPracticeSettings);
    app.update();

    app.world_mut().run_schedule(FixedUpdate);

    let bgm = app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .as_ref()
        .expect("the active primary BGM is queued as clock authority");
    assert_eq!(bgm.wav_slot, 1);
    assert!((bgm.start_seconds - 1.5).abs() < f64::EPSILON);
    assert_eq!((bgm.volume, bgm.pan), (37, -41));
    assert_eq!(
        bgm.playback_mix(app.world().resource::<dtx_audio::ChartSoundBank>()),
        (37, -41),
        "uncached primary BGM playback uses authored mix"
    );
    assert_eq!(app.world().resource::<GameStartMs>().0, 0);
    let slices = &app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0;
    assert_eq!(
        slices
            .iter()
            .map(|slice| slice.wav_slot)
            .collect::<Vec<_>>(),
        vec![2, 4],
        "the later BGM stays a layer while SE01 choke keeps only its newest slice"
    );
    assert_eq!(
        slices
            .iter()
            .map(|slice| (slice.wav_slot, slice.start_seconds, slice.volume, slice.pan))
            .collect::<Vec<_>>(),
        vec![(2, 0.5, 48, 22), (4, 0.5, 63, 34)],
        "uncached layer BGM and SE reconstruction retain authored mix and offset"
    );
    let sound_bank = app.world().resource::<dtx_audio::ChartSoundBank>();
    assert_eq!(slices[0].playback_mix(sound_bank), (48, 22));
    assert_eq!(slices[1].playback_mix(sound_bank), (63, 34));
}

#[test]
fn cached_seek_slice_keeps_cached_mix_authority() {
    use bevy_kira_audio::prelude::{Frame, StaticSoundData, StaticSoundSettings};
    use bevy_kira_audio::AudioSource as KiraAudioSource;

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_overlapping_audio_slices());
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    let source = app
        .world_mut()
        .resource_mut::<Assets<KiraAudioSource>>()
        .add(KiraAudioSource {
            sound: StaticSoundData {
                sample_rate: 1_000,
                frames: vec![Frame::from_mono(0.0); 10_000].into(),
                settings: StaticSoundSettings::default(),
                slice: None,
            },
        });
    app.world_mut()
        .resource_mut::<dtx_audio::ChartSoundBank>()
        .insert(
            2,
            dtx_audio::LoadedChartSound {
                handle: source,
                path: "cached-layer.wav".into(),
                volume: 71,
                pan: -28,
            },
        );
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    let layer = app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0
        .iter()
        .find(|slice| slice.wav_slot == 2)
        .expect("layer BGM is queued");
    assert_eq!((layer.volume, layer.pan), (48, 22));
    assert_eq!(
        layer.playback_mix(app.world().resource::<dtx_audio::ChartSoundBank>()),
        (71, -28),
        "the cached playback path keeps the preloaded sound's mix authority"
    );
}

#[test]
fn seek_does_not_reconstruct_a_decoded_slice_past_its_duration() {
    use bevy_kira_audio::prelude::{Frame, StaticSoundData, StaticSoundSettings};
    use bevy_kira_audio::AudioSource as KiraAudioSource;

    let mut assets = DtxAssets::default();
    for (slot, path) in [
        (1, "primary.wav"),
        (2, "layer.wav"),
        (3, "se.wav"),
        (4, "system.wav"),
    ] {
        assets.wav.insert(slot, path.into());
    }
    let chart = Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::BGM, 0.0, 1),
            Chip::with_wav(0, EChannel::BGM, 0.25, 2),
            Chip::with_wav(0, EChannel::SE01, 0.25, 3),
            Chip::with_wav(0, EChannel::Click, 0.25, 4),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    };
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart);
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    for (slot, path) in [(2, "layer.wav"), (3, "se.wav"), (4, "system.wav")] {
        let source = app
            .world_mut()
            .resource_mut::<Assets<KiraAudioSource>>()
            .add(KiraAudioSource {
                sound: StaticSoundData {
                    sample_rate: 1_000,
                    frames: vec![Frame::from_mono(0.0); 500].into(),
                    settings: StaticSoundSettings::default(),
                    slice: None,
                },
            });
        app.world_mut()
            .resource_mut::<dtx_audio::ChartSoundBank>()
            .insert(
                slot,
                dtx_audio::LoadedChartSound {
                    handle: source,
                    path: path.into(),
                    volume: 100,
                    pan: 0,
                },
            );
    }
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    let slices = &app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0;
    for slot in [2, 3, 4] {
        assert!(
            !slices.iter().any(|slice| slice.wav_slot == slot),
            "decoded one-shot slot {slot} must expire"
        );
    }
}

fn install_short_primary(app: &mut App) {
    use bevy_kira_audio::prelude::{Frame, StaticSoundData, StaticSoundSettings};
    use bevy_kira_audio::AudioSource as KiraAudioSource;

    let source = app
        .world_mut()
        .resource_mut::<Assets<KiraAudioSource>>()
        .add(KiraAudioSource {
            sound: StaticSoundData {
                sample_rate: 1_000,
                frames: vec![Frame::from_mono(0.0); 500].into(),
                settings: StaticSoundSettings::default(),
                slice: None,
            },
        });
    app.world_mut()
        .resource_mut::<dtx_audio::ChartSoundBank>()
        .insert(
            1,
            dtx_audio::LoadedChartSound {
                handle: source,
                path: "shared-bgm.wav".into(),
                volume: 37,
                pan: -41,
            },
        );
}

fn assert_short_primary_pending(app: &App, expected_offset: f64) {
    let pending = app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .as_ref()
        .expect("the looped primary remains authoritative");
    assert_eq!(pending.wav_slot, 1);
    assert_eq!(pending.path, "shared-bgm.wav");
    assert!((pending.start_seconds - expected_offset).abs() < f64::EPSILON);
    assert_eq!((pending.volume, pending.pan), (37, -41));
    assert_eq!(
        pending.playback_mix(app.world().resource::<dtx_audio::ChartSoundBank>()),
        (37, -41)
    );
    assert!(!app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0
        .iter()
        .any(|slice| slice.path == "shared-bgm.wav"));
}

#[test]
fn preview_seek_wraps_short_decoded_primary_offset() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_repeated_primary_path());
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::practice(0.75);
    install_short_primary(&mut app);
    ready_clock(&mut app, 250);

    send_seek(&mut app, 750);
    app.world_mut().run_schedule(FixedUpdate);

    assert_short_primary_pending(&app, 0.25);
    assert!((app.world().resource::<EffectivePlaybackRate>().value - 0.75).abs() < f64::EPSILON);
}

#[test]
fn paused_normal_seek_wraps_short_decoded_primary_multiple_times() {
    let mut app = build_lifecycle_app(PracticeIntent::None);
    enter_performance(&mut app, chart_with_repeated_primary_path());
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::practice(0.75);
    install_short_primary(&mut app);
    ready_clock(&mut app, 250);
    app.world_mut()
        .resource_mut::<NextState<game_shell::PauseState>>()
        .set(game_shell::PauseState::Paused);
    app.update();

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert_short_primary_pending(&app, 0.0);
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::seek::PendingAudioStarts>()
            .0
            .iter()
            .map(|slice| slice.path.as_str())
            .collect::<Vec<_>>(),
        vec!["distinct-layer.wav"]
    );
    assert!((app.world().resource::<EffectivePlaybackRate>().value - 0.75).abs() < f64::EPSILON);
}

#[test]
fn seek_reconstruction_respects_configured_polyphony() {
    let mut assets = DtxAssets::default();
    assets.wav.insert(3, "system.wav".into());
    assets.wav.volumes.insert(3, 52);
    assets.wav.pans.insert(3, -19);
    let chart = Chart {
        metadata: Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![
            Chip::with_wav(0, EChannel::Click, 0.25, 3),
            Chip::with_wav(0, EChannel::FirstSoundChip, 0.5, 3),
            Chip::new(2, EChannel::BassDrum, 0.0),
        ],
        assets,
        ..Default::default()
    };
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart);
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    app.world_mut()
        .resource_mut::<dtx_audio::DrumPolyphony>()
        .set_voices(1);
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    let slices = &app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0;
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0].chip_idx, 1);
    assert!((slices[0].start_seconds - 0.5).abs() < f64::EPSILON);
    assert_eq!((slices[0].volume, slices[0].pan), (52, -19));
}

fn assert_running_seek_starts_all_spanning_audio(intent: PracticeIntent) {
    let mut app = build_lifecycle_app(intent);
    enter_performance(&mut app, chart_with_overlapping_audio_slices());
    if let Some(mut flow) = app.world_mut().get_resource_mut::<PracticeFlow>() {
        flow.phase = PracticePhase::Setup;
        flow.preview = PreviewState::Playing;
    }
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world().resource::<BgmHandle>().path.as_deref(),
        Some("primary.wav")
    );
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::resources::ActiveDrumSounds>()
            .layer_bgm_instances
            .len(),
        1
    );
    assert!(app
        .world()
        .resource::<gameplay_drums::resources::ActiveDrumSounds>()
        .stick_se_instances
        .contains_key(&EChannel::SE01));
}

#[test]
fn preview_seek_starts_all_spanning_audio_slices() {
    assert_running_seek_starts_all_spanning_audio(PracticeIntent::manual(
        PracticeOrigin::SongSelect,
    ));
}

#[test]
fn normal_seek_starts_all_spanning_audio_slices() {
    assert_running_seek_starts_all_spanning_audio(PracticeIntent::None);
}

fn assert_running_seek_deduplicates_repeated_primary_path() {
    let mut app = build_lifecycle_app(PracticeIntent::None);
    enter_performance(&mut app, chart_with_repeated_primary_path());
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world().resource::<BgmHandle>().path.as_deref(),
        Some("shared-bgm.wav")
    );
    let active = app
        .world()
        .resource::<gameplay_drums::resources::ActiveDrumSounds>();
    assert_eq!(
        active.layer_bgm_instances.len(),
        1,
        "the repeated primary path must not become a duplicate layer"
    );
}

#[test]
fn preview_seek_deduplicates_repeated_primary_path() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_repeated_primary_path());
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    let primary = app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .as_ref()
        .expect("one authoritative BGM is queued");
    assert_eq!(primary.path, "shared-bgm.wav");
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::seek::PendingAudioStarts>()
            .0
            .iter()
            .map(|slice| slice.path.as_str())
            .collect::<Vec<_>>(),
        vec!["distinct-layer.wav"],
        "only the distinct BGM path remains a layer"
    );
}

#[test]
fn normal_running_seek_deduplicates_repeated_primary_path() {
    assert_running_seek_deduplicates_repeated_primary_path();
}

#[test]
fn stopped_preview_seek_keeps_same_path_layer_when_primary_is_ineligible() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(
        &mut app,
        chart_with_ineligible_primary_and_same_path_layer(),
    );
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Editing;
    ready_clock(&mut app, 250);

    send_seek(&mut app, 2_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert!(app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .is_none());
    let slices = &app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0;
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0].wav_slot, 2);
    assert_eq!(slices[0].path, "shared-bgm.wav");
    assert!((slices[0].start_seconds - 2.0).abs() < f64::EPSILON);
    assert_eq!((slices[0].volume, slices[0].pan), (48, 22));
}

#[test]
fn normal_running_seek_starts_same_path_layer_when_primary_is_ineligible() {
    let mut app = build_lifecycle_app(PracticeIntent::None);
    enter_performance(
        &mut app,
        chart_with_ineligible_primary_and_same_path_layer(),
    );
    ready_clock(&mut app, 250);

    send_seek(&mut app, 2_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert!(app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .is_none());
    assert!(app.world().resource::<BgmHandle>().path.is_none());
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::resources::ActiveDrumSounds>()
            .layer_bgm_instances
            .len(),
        1
    );
}

#[test]
fn pause_menu_settings_hands_off_to_editing_before_unpausing() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Running;
    ready_clock(&mut app, 1_500);

    app.world_mut()
        .resource_mut::<NextState<game_shell::PauseState>>()
        .set(game_shell::PauseState::Paused);
    app.update();
    app.world_mut()
        .resource_mut::<gameplay_drums::pause::PauseSelection>()
        .0 = 2;
    let held_ms = app.world().resource::<GameplayClock>().current_ms;

    app.world_mut().write_message(game_shell::NavAction {
        verb: game_shell::NavVerb::Confirm,
        source: game_shell::NavSource::Keyboard,
        coarse: false,
    });
    app.update();

    assert_eq!(
        app.world().resource::<PracticeFlow>().phase,
        PracticePhase::Editing
    );
    assert_eq!(
        *app.world()
            .resource::<State<game_shell::PauseState>>()
            .get(),
        game_shell::PauseState::Paused,
        "the pause state cannot exit before Editing owns the chart"
    );
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, held_ms);

    app.update();
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        *app.world()
            .resource::<State<game_shell::PauseState>>()
            .get(),
        game_shell::PauseState::Running
    );
    assert_eq!(
        app.world().resource::<PracticeFlow>().phase,
        PracticePhase::Editing
    );
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, held_ms);
}

fn assert_gameplay_seek_rebuilds_judged(intent: PracticeIntent) {
    let mut app = build_lifecycle_app(intent);
    enter_performance(&mut app, chart_for_preview_seek());
    if let Some(mut flow) = app.world_mut().get_resource_mut::<PracticeFlow>() {
        flow.phase = PracticePhase::Running;
    }
    app.world_mut().resource_mut::<JudgedChips>().0 = [99].into();
    app.world_mut()
        .resource_mut::<gameplay_drums::seek::PreviewSkippedChips>()
        .0 = [3, 99].into();
    ready_clock(&mut app, 250);

    send_seek(&mut app, 1_500);
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world().resource::<JudgedChips>().0,
        std::collections::HashSet::from([0, 1, 2])
    );
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::seek::PreviewSkippedChips>()
            .0,
        std::collections::HashSet::from([0, 1, 2]),
        "a gameplay seek must leave future preview reconstruction at the same position"
    );
}

#[test]
fn running_practice_seek_rebuilds_gameplay_judged() {
    assert_gameplay_seek_rebuilds_judged(PracticeIntent::manual(PracticeOrigin::SongSelect));
}

#[test]
fn normal_play_seek_rebuilds_gameplay_judged() {
    assert_gameplay_seek_rebuilds_judged(PracticeIntent::None);
}

#[test]
fn leaving_performance_drops_practice_surface_but_preserves_session_for_results() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Result);
    app.update();

    assert!(app.world().contains_resource::<PracticeSession>());
    assert!(!app.world().contains_resource::<PracticeDraft>());
    assert!(!app.world().contains_resource::<PracticeFlow>());
}

#[test]
fn setup_drops_judgment_and_miss_output() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    app.world_mut().spawn((
        Note {
            chip_id: 0,
            lane: 2,
            target_ms: 2_000,
        },
        NoteVisual,
        Node::default(),
    ));
    app.world_mut().write_message(LaneHit {
        lane: 2,
        audio_ms: 2_000,
    });

    app.world_mut().run_schedule(FixedUpdate);

    assert!(app.world().resource::<Messages<JudgmentEvent>>().is_empty());
    assert!(app.world().resource::<Messages<NoteMissed>>().is_empty());
    assert!(app.world().resource::<JudgedChips>().0.is_empty());
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert!(app
        .world()
        .resource::<PracticeSession>()
        .attempt_history
        .is_empty());
}

#[test]
fn setup_preview_cleans_passed_visuals_without_gameplay_output() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    app.world_mut().spawn((
        Note {
            chip_id: 0,
            lane: 2,
            target_ms: 2_000,
        },
        NoteVisual,
        Node::default(),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<NoteVisual>>()
            .iter(app.world())
            .count(),
        0
    );
    assert!(app.world().resource::<JudgedChips>().0.is_empty());
    assert!(app.world().resource::<Messages<NoteMissed>>().is_empty());
}

#[test]
fn setup_normal_play_has_no_practice_resources_and_still_judges() {
    let mut app = build_lifecycle_app(PracticeIntent::None);
    enter_performance(&mut app, chart_with_measures(4));
    assert!(!app.world().contains_resource::<PracticeSession>());
    assert!(!app.world().contains_resource::<PracticeDraft>());
    assert!(!app.world().contains_resource::<PracticeFlow>());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(2_000));
    }
    app.world_mut().write_message(LaneHit {
        lane: 2,
        audio_ms: 2_000,
    });

    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(app.world().resource::<JudgedChips>().0.len(), 1);
    assert_eq!(
        app.world()
            .resource::<Messages<JudgmentEvent>>()
            .iter_current_update_messages()
            .count(),
        1
    );
}

#[test]
fn setup_start_commits_draft_records_last_used_and_seeks_to_preroll() {
    use bevy::ecs::system::RunSystemOnce;
    use gameplay_drums::practice::session::{AttemptStats, PrerollSetting};

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut draft = app.world_mut().resource_mut::<PracticeDraft>();
        draft.loop_region = Some(LoopRegion {
            start_ms: 4_000,
            end_ms: 8_000,
        });
        draft.user_tempo = 0.8;
        draft.preroll = PrerollSetting::OneBar;
    }
    let snapshot_session = app.world().resource::<PracticeSession>().clone();
    {
        let mut flow = app.world_mut().resource_mut::<PracticeFlow>();
        flow.preview = PreviewState::Playing;
        flow.edit_snapshot = Some(PracticeEditSnapshot {
            chart_ms: 6_000,
            session: snapshot_session,
        });
    }
    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.current_attempt = AttemptStats {
            start_ms: 1_000,
            counts: JudgmentCounts {
                perfect: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        session.current_attempt_eligible = false;
    }
    app.world_mut().write_message(LaneHit {
        lane: 2,
        audio_ms: 4_000,
    });

    app.world_mut()
        .run_system_once(start_or_continue_practice)
        .expect("start system runs");

    let session = app.world().resource::<PracticeSession>();
    assert_eq!(
        session.transport.loop_region,
        app.world().resource::<PracticeDraft>().loop_region
    );
    assert_eq!(session.transport.user_tempo, 0.8);
    assert_eq!(session.current_attempt.start_ms, 4_000);
    assert_eq!(session.current_attempt.counts.total(), 0);
    assert!(session.current_attempt_eligible);
    let flow = app.world().resource::<PracticeFlow>();
    assert_eq!(flow.phase, PracticePhase::Running);
    assert_eq!(flow.preview, PreviewState::Stopped);
    assert!(flow.edit_snapshot.is_none());
    let seeks = app
        .world()
        .resource::<Messages<SeekToChartTime>>()
        .iter_current_update_messages()
        .collect::<Vec<_>>();
    assert_eq!(seeks.len(), 1);
    assert_eq!(seeks[0].target_ms, 2_000);
    assert_eq!(seeks[0].attempt_start_ms, Some(4_000));
    let commands = app
        .world()
        .resource::<Messages<PresetCommand>>()
        .iter_current_update_messages()
        .collect::<Vec<_>>();
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        commands[0],
        PresetCommand::RecordLastUsed { draft } if draft.user_tempo == 0.8
    ));
    app.world_mut().run_schedule(FixedUpdate);
    assert!(
        !app.world().resource::<JudgedChips>().0.contains(&1),
        "the input that confirmed Start cannot become the first judged hit"
    );
}

#[test]
fn setup_cancel_routes_origins_and_defends_results_snapshot() {
    use bevy::ecs::system::RunSystemOnce;

    for (origin, available, expected, skips, warns) in [
        (
            PracticeOrigin::SongSelect,
            false,
            AppState::SongSelect,
            false,
            false,
        ),
        (
            PracticeOrigin::NormalPause,
            false,
            AppState::SongSelect,
            false,
            false,
        ),
        (PracticeOrigin::Results, true, AppState::Result, true, false),
        (
            PracticeOrigin::Results,
            false,
            AppState::SongSelect,
            false,
            true,
        ),
    ] {
        let mut app = build_lifecycle_app(PracticeIntent::manual(origin));
        enter_performance(&mut app, chart_with_measures(4));
        app.world_mut()
            .resource_mut::<game_shell::ResultReturnState>()
            .available = available;

        app.world_mut()
            .run_system_once(cancel_initial_setup)
            .expect("cancel system runs");

        let requests = app
            .world()
            .resource::<Messages<game_shell::TransitionRequest>>()
            .iter_current_update_messages()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1, "{origin:?} available={available}");
        assert_eq!(requests[0].0, expected, "{origin:?} available={available}");
        assert_eq!(
            app.world()
                .resource::<game_shell::ResultReturnState>()
                .skip_processing_once,
            skips,
            "{origin:?} available={available}"
        );
        assert_eq!(
            !app.world()
                .resource::<gameplay_drums::practice::toast::ToastQueue>()
                .is_empty(),
            warns,
            "{origin:?} available={available}"
        );
    }
}

#[test]
fn setup_stale_messages_cannot_mutate_outputs_or_attempts() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    app.world_mut()
        .resource_mut::<ActiveChart>()
        .chart
        .empty_hit_events
        .push(dtx_core::EmptyHitEvent {
            lane: 2,
            measure: 0,
            value: 0.0,
            wav_slot: 0,
        });
    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.trainer.arm_ramp();
        session.trainer.ramp.step_tempo = 0.70;
    }
    app.world_mut()
        .resource_mut::<gameplay_drums::practice::stats::LastFinalizedAttempt>()
        .0 = Some(gameplay_drums::practice::session::AttemptRecord {
        start_ms: 0,
        end_ms: 8_000,
        tempo: 0.70,
        counts: JudgmentCounts {
            perfect: 4,
            ..Default::default()
        },
        max_combo: 4,
        overhits: 0,
        accuracy_pct: 100.0,
        mean_error_ms: 0.0,
        waited: 0,
        flow_pct: 100.0,
    });
    let gauge_before = app
        .world()
        .resource::<gameplay_drums::gauge::StageGauge>()
        .value;
    app.world_mut().write_message(JudgmentEvent {
        lane: 2,
        kind: dtx_scoring::JudgmentKind::Perfect,
        delta_ms: 0,
        chip_idx: 0,
    });
    app.world_mut().write_message(NoteMissed {
        lane: 2,
        audio_ms: 8_500,
        chip_idx: 1,
    });
    app.world_mut().write_message(EmptyHit {
        lane: 2,
        audio_ms: 8_500,
    });
    app.world_mut()
        .write_message(gameplay_drums::practice::ab_loop::PracticeLoopCompleted {
            region_start_ms: 0,
            region_end_ms: 8_000,
        });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(9_000));
    }

    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Combo>().current, 0);
    assert_eq!(app.world().resource::<JudgmentCounts>().total(), 0);
    assert_eq!(
        app.world()
            .resource::<gameplay_drums::gauge::StageGauge>()
            .value,
        gauge_before
    );
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(session.current_attempt.counts.total(), 0);
    assert_eq!(session.current_attempt.overhits, 0);
    assert!(session.attempt_history.is_empty());
    assert_eq!(session.trainer.ramp.step_tempo, 0.70);
    assert!(app
        .world()
        .resource::<Messages<SeekToChartTime>>()
        .is_empty());
    assert!(app
        .world()
        .resource::<gameplay_drums::resources::CurrentEmptyHitTemplates>()
        .get(2)
        .is_none());
}

#[test]
fn setup_wait_watcher_cannot_halt() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    app.world_mut()
        .resource_mut::<PracticeSession>()
        .trainer
        .enable_wait(true);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(2_100));
    }

    app.world_mut().run_schedule(FixedUpdate);

    assert!(!app
        .world()
        .resource::<gameplay_drums::practice::wait::WaitState>()
        .halted());
    assert!(app.world().resource::<JudgedChips>().0.is_empty());
}

#[test]
fn setup_chart_clock_stays_frozen_until_preview_plays() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(4));
    app.world_mut().resource_mut::<GameplayClock>().start();

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(std::time::Duration::from_millis(16));
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 0);

    app.world_mut().resource_mut::<PracticeFlow>().preview = PreviewState::Playing;
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(std::time::Duration::from_millis(16));
    app.world_mut().run_schedule(FixedUpdate);
    assert!(app.world().resource::<GameplayClock>().current_ms > 0);
}

#[test]
fn preview_wraps_draft_loop_without_attempt_completion() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().resource_mut::<PracticeDraft>().loop_region = Some(LoopRegion {
        start_ms: 2_000,
        end_ms: 6_000,
    });

    app.world_mut().write_message(PreviewAction::Play);
    app.update();
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        app.world().resource::<PracticeFlow>().preview,
        PreviewState::Playing
    );
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 2_000);
    assert!(app
        .world()
        .resource::<PracticeSession>()
        .attempt_history
        .is_empty());
    assert!(app
        .world()
        .resource::<Messages<gameplay_drums::practice::ab_loop::PracticeLoopCompleted>>()
        .is_empty());
}

#[test]
fn preview_play_normalizes_invalid_draft_once_before_wrapping() {
    for region in [
        LoopRegion {
            start_ms: 6_000,
            end_ms: 2_000,
        },
        LoopRegion {
            start_ms: 4_000,
            end_ms: 4_000,
        },
        LoopRegion {
            start_ms: 90_000,
            end_ms: 100_000,
        },
    ] {
        let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
        enter_performance(&mut app, chart_with_measures(8));
        {
            let mut draft = app.world_mut().resource_mut::<PracticeDraft>();
            draft.loop_region = Some(region);
            draft.user_tempo = f32::NAN;
        }

        app.world_mut().write_message(PreviewAction::Play);
        app.update();
        app.world_mut().run_schedule(FixedUpdate);

        let (normalized_tempo, normalized_region) = {
            let draft = app.world().resource::<PracticeDraft>();
            (draft.user_tempo, draft.loop_region)
        };
        assert!(normalized_tempo.is_finite());
        assert_ne!(normalized_region, Some(region));
        let first_ms = app.world().resource::<GameplayClock>().current_ms;
        app.world_mut()
            .resource_mut::<gameplay_drums::seek::LastSeekFrom>()
            .0 = None;
        for _ in 0..3 {
            app.world_mut().run_schedule(FixedUpdate);
            assert!(
                app.world().resource::<GameplayClock>().current_ms >= first_ms,
                "a normalized preview must not fixed-tick seek-loop"
            );
        }
        assert!(app
            .world()
            .resource::<gameplay_drums::seek::LastSeekFrom>()
            .0
            .is_none());
        let warning_count = app
            .world()
            .resource::<gameplay_drums::practice::toast::ToastQueue>()
            .iter()
            .filter(|toast| toast.message.contains("using whole song"))
            .count();
        assert_eq!(warning_count, usize::from(normalized_region.is_none()));
    }
}

#[test]
fn preview_transport_plays_pauses_seeks_and_steps_bars() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(16));

    app.world_mut().write_message(PreviewAction::Play);
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(
        app.world().resource::<PracticeFlow>().preview,
        PreviewState::Playing
    );

    app.world_mut().write_message(PreviewAction::Pause);
    app.update();
    assert_eq!(
        app.world().resource::<PracticeFlow>().preview,
        PreviewState::Stopped
    );

    app.world_mut().write_message(PreviewAction::Seek(5_500));
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 5_500);

    app.world_mut().write_message(PreviewAction::PrevBar);
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 2_000);

    app.world_mut().write_message(PreviewAction::NextBar);
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 4_000);
    assert!(app
        .world()
        .resource::<PracticeSession>()
        .attempt_history
        .is_empty());
}

#[test]
fn preview_play_keeps_old_audio_paused_until_seek_reconstruction() {
    use bevy_kira_audio::prelude::{Audio, Frame, StaticSoundData, StaticSoundSettings};
    use bevy_kira_audio::{AudioControl, AudioSource as KiraAudioSource};

    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_for_preview_seek());
    ready_clock(&mut app, 250);
    let source = app
        .world_mut()
        .resource_mut::<Assets<KiraAudioSource>>()
        .add(KiraAudioSource {
            sound: StaticSoundData {
                sample_rate: 1_000,
                frames: vec![Frame::from_mono(0.0); 10_000].into(),
                settings: StaticSoundSettings::default(),
                slice: None,
            },
        });
    let old = app.world().resource::<Audio>().play(source).handle();
    app.update();
    app.world_mut().resource_mut::<BgmHandle>().instance = Some(old.clone());
    dtx_audio::pause_audio_instance(
        &mut app
            .world_mut()
            .resource_mut::<Assets<bevy_kira_audio::AudioInstance>>(),
        &old,
    );
    app.update();
    app.world_mut().write_message(PreviewAction::Play);
    app.update();

    assert_eq!(
        app.world().resource::<BgmHandle>().instance.as_ref(),
        Some(&old),
        "Update must not replace or resume the old audio before the seek applies"
    );
    assert!(
        app.world()
            .resource::<gameplay_drums::practice::PreviewController>()
            .start_pending
    );
}

#[test]
fn preview_cancel_editing_restores_frozen_session_and_cursor() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(16));
    {
        let mut flow = app.world_mut().resource_mut::<PracticeFlow>();
        flow.phase = PracticePhase::Running;
    }
    {
        let mut session = app.world_mut().resource_mut::<PracticeSession>();
        session.transport.user_tempo = 0.75;
    }
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(5_000));
    }

    app.world_mut().write_message(OpenPracticeSettings);
    app.update();
    app.world_mut().resource_mut::<PracticeDraft>().user_tempo = 1.25;
    app.world_mut().write_message(PreviewAction::Seek(20_000));
    app.update();
    app.world_mut().write_message(CancelPracticeSettings);
    app.update();
    app.world_mut().run_schedule(FixedUpdate);

    let session = app.world().resource::<PracticeSession>();
    assert_eq!(session.transport.user_tempo, 0.75);
    assert!(!session.current_attempt_eligible);
    let flow = app.world().resource::<PracticeFlow>();
    assert_eq!(flow.phase, PracticePhase::Running);
    assert_eq!(flow.preview, PreviewState::Stopped);
    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 5_000);
    assert_eq!(
        *app.world()
            .resource::<gameplay_drums::pause::PracticePauseSurface>(),
        gameplay_drums::pause::PracticePauseSurface::Overlay
    );
    assert!(matches!(
        app.world().resource::<NextState<game_shell::PauseState>>(),
        NextState::Pending(game_shell::PauseState::Paused)
    ));
}

#[test]
fn preview_transients_do_not_leak_across_performance_reentry() {
    let mut app = build_lifecycle_app(PracticeIntent::manual(PracticeOrigin::SongSelect));
    enter_performance(&mut app, chart_with_measures(16));
    app.world_mut().resource_mut::<PracticeFlow>().phase = PracticePhase::Running;
    ready_clock(&mut app, 5_000);

    app.world_mut().write_message(OpenPracticeSettings);
    app.update();
    app.world_mut().write_message(CancelPracticeSettings);
    app.update();
    assert!(app
        .world()
        .resource::<gameplay_drums::practice::PreviewController>()
        .restore_ms
        .is_none());
    app.world_mut()
        .resource_mut::<gameplay_drums::seek::PendingBgmStart>()
        .0 = Some(gameplay_drums::seek::PendingBgm {
        wav_slot: 99,
        path: "stale.wav".into(),
        start_seconds: 4.0,
        volume: 100,
        pan: 0,
    });
    app.world_mut()
        .resource_mut::<gameplay_drums::seek::PendingAudioStarts>()
        .0
        .push(gameplay_drums::seek::PendingAudioSlice {
            chip_idx: 99,
            wav_slot: 99,
            path: "stale-layer.wav".into(),
            start_seconds: 4.0,
            volume: 100,
            pan: 0,
            kind: gameplay_drums::seek::PendingAudioKind::LayerBgm,
        });
    app.world_mut()
        .resource_mut::<gameplay_drums::seek::StoppedSeekRebuild>()
        .0 = true;
    app.world_mut()
        .resource_mut::<gameplay_drums::seek::LastSeekFrom>()
        .0 = Some(4_000);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::SongSelect);
    app.update();
    *app.world_mut().resource_mut::<PracticeIntent>() =
        PracticeIntent::manual(PracticeOrigin::SongSelect);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();

    assert_eq!(
        app.world().resource::<PracticeFlow>().phase,
        PracticePhase::Setup
    );
    let controller = app
        .world()
        .resource::<gameplay_drums::practice::PreviewController>();
    assert!(controller.restore_ms.is_none());
    assert!(!controller.start_pending);
    assert!(app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .is_none());
    assert!(app
        .world()
        .resource::<gameplay_drums::seek::PendingAudioStarts>()
        .0
        .is_empty());
    assert!(
        !app.world()
            .resource::<gameplay_drums::seek::StoppedSeekRebuild>()
            .0
    );
    assert!(app
        .world()
        .resource::<gameplay_drums::seek::LastSeekFrom>()
        .0
        .is_none());
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(
        app.world().resource::<PracticeFlow>().phase,
        PracticePhase::Setup
    );
}

#[test]
fn practice_never_requests_end_of_stage() {
    // v3: practice is a room — the implicit whole-song loop wraps
    // instead; detect_end_of_stage must never fire while a
    // PracticeSession exists, loop or no loop.
    for region in [
        None,
        Some(LoopRegion {
            start_ms: 0,
            end_ms: i64::MAX,
        }), // A-only
        Some(LoopRegion {
            start_ms: 0,
            end_ms: 2_000,
        }), // armed
    ] {
        let mut app = build_app();
        enter_performance(&mut app, chart_with_measures(2));
        let mut s = PracticeSession::default();
        s.transport.loop_region = region;
        app.world_mut().insert_resource(s);
        {
            let mut clock = app.world_mut().resource_mut::<GameplayClock>();
            clock.start();
            clock.sync(Some(50_000));
        }
        app.update();
        assert!(
            !app.world().resource::<DrumsStageCompletion>().end_requested,
            "practice must never end the stage (region: {region:?})"
        );
    }
}

#[test]
fn loop_watcher_seeks_back_to_region_start() {
    let mut app = build_app();
    // Register the watcher in front of the seek system.
    app.add_message::<gameplay_drums::practice::ab_loop::PracticeLoopCompleted>()
        .add_systems(
            Update,
            gameplay_drums::practice::ab_loop::loop_watcher
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        );
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            preroll: gameplay_drums::practice::session::PrerollSetting::Off,
            ..Default::default()
        },
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(6_100));
    }
    app.update();
    let clock = app.world().resource::<GameplayClock>();
    assert_eq!(
        clock.current_ms, 2_000,
        "past region end the clock must land back on A"
    );
    // Chip timing here lands exactly on measure boundaries (`chip_target_ms`
    // is unclamped): chip 0 sits exactly at A (2000ms), chip 2 exactly at B
    // (6000ms) — neither is strictly before A, so both stay live post-seek.
    let judged = &app.world().resource::<JudgedChips>().0;
    assert!(
        !judged.contains(&0),
        "chip at A (2000ms) is live, not seeded"
    );
    assert!(
        !judged.contains(&2),
        "chip at B (6000ms) is live, not seeded"
    );
}

fn send_seek(app: &mut App, target_ms: i64) {
    app.world_mut()
        .resource_mut::<Messages<SeekToChartTime>>()
        .write(SeekToChartTime {
            target_ms,
            snap: None,
            attempt_start_ms: None,
        });
}

#[test]
fn forward_seek_seeds_skip_sets_and_jumps_clock() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 9_000);
    let judged = &app.world().resource::<JudgedChips>().0;
    // Chips 0..=3 land before 9000ms at default 120 BPM (measure=2000ms).
    assert!(judged.contains(&0) && judged.contains(&3));
    assert!(!judged.contains(&4), "chips past target stay live");
}

#[test]
fn backward_seek_prunes_skip_sets() {
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(8));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(0));
    }
    send_seek(&mut app, 9_000);
    app.update();
    send_seek(&mut app, 0);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 0);
    assert!(
        app.world().resource::<JudgedChips>().0.is_empty(),
        "backward seek must un-mark judged chips"
    );
    assert!(app.world().resource::<PlayedSeChips>().0.is_empty());
}

#[test]
fn seek_is_inert_without_practice_in_normal_play() {
    // Regression guard: with no PracticeSession and no seek messages,
    // a normal stage run is untouched by the new systems.
    let mut app = build_app();
    enter_performance(&mut app, chart_with_measures(2));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(10_000));
    }
    app.update();
    assert!(
        app.world().resource::<DrumsStageCompletion>().end_requested,
        "normal end-of-stage unchanged"
    );
}

use gameplay_drums::practice::actions::{
    apply_practice_actions, emit_practice_actions, PracticeAction, PracticeBindings,
};

fn add_action_wiring(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<PracticeBindings>()
        .init_resource::<gameplay_drums::practice::toast::ToastQueue>()
        .init_state::<game_shell::PauseState>()
        .add_message::<PracticeAction>()
        .add_systems(
            Update,
            (emit_practice_actions, apply_practice_actions)
                .chain()
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(resource_exists::<PracticeSession>),
        );
}

#[test]
fn practice_rate_survives_pause_and_resume() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::practice(0.75);

    app.world_mut()
        .resource_mut::<NextState<game_shell::PauseState>>()
        .set(game_shell::PauseState::Paused);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<game_shell::PauseState>>()
        .set(game_shell::PauseState::Running);
    app.update();

    let rate = app.world().resource::<EffectivePlaybackRate>();
    assert_eq!(rate.source, PlaybackRateSource::PracticeTempo);
    assert!((rate.value - 0.75).abs() < f64::EPSILON);
}

fn prepare_fallback_bgm(app: &mut App, test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dtxmaniars-{test_name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create fixture directory");
    let chart_path = dir.join("chart.dtx");
    std::fs::write(&chart_path, b"#TITLE: Rate Seek\n").expect("write fixture chart");
    std::fs::write(dir.join("bgm.wav"), b"").expect("write fallback BGM marker");
    app.world_mut().resource_mut::<ActiveChart>().source_path = Some(chart_path);
    dir
}

#[test]
fn seek_uses_chart_seconds_without_rate_scaling() {
    let mut app = build_app();
    let dir = prepare_fallback_bgm(&mut app, "rate-seek");
    enter_performance(&mut app, chart_with_measures(8));
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::practice(0.75);
    app.world_mut().resource_mut::<GameplayClock>().start();

    send_seek(&mut app, 9_000);
    app.update();

    assert_eq!(app.world().resource::<GameplayClock>().current_ms, 9_000);
    let pending = app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .as_ref()
        .expect("fallback BGM queued");
    assert!((pending.start_seconds - 9.0).abs() < f64::EPSILON);
    std::fs::remove_dir_all(dir).expect("remove fixture directory");
}

#[test]
fn restart_keeps_practice_rate_and_queues_zero_offset() {
    let mut app = build_app();
    let dir = prepare_fallback_bgm(&mut app, "rate-restart");
    enter_performance(&mut app, chart_with_measures(8));
    *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
        EffectivePlaybackRate::practice(0.75);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(9_000));
    }

    send_seek(&mut app, 0);
    app.update();

    let rate = app.world().resource::<EffectivePlaybackRate>();
    assert_eq!(rate.source, PlaybackRateSource::PracticeTempo);
    assert!((rate.value - 0.75).abs() < f64::EPSILON);
    let pending = app
        .world()
        .resource::<gameplay_drums::seek::PendingBgmStart>()
        .0
        .as_ref()
        .expect("fallback BGM queued");
    assert!((pending.start_seconds - 0.0).abs() < f64::EPSILON);
    std::fs::remove_dir_all(dir).expect("remove fixture directory");
}

#[test]
fn bracket_key_sets_loop_start_snapped_to_bar() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(4_700));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::BracketLeft);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    let region = session.transport.loop_region.expect("A marker set");
    assert_eq!(region.start_ms, 4_000, "A snaps down to the bar start");
}

#[test]
fn restart_key_seeks_to_loop_start() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            preroll: gameplay_drums::practice::session::PrerollSetting::Off,
            ..Default::default()
        },
        ..Default::default()
    });
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(5_000));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(
        app.world().resource::<GameplayClock>().current_ms,
        2_000,
        "R restarts the loop at A"
    );
}

use gameplay_drums::events::EmptyHit;

fn add_ramp_wiring(app: &mut App) {
    if !app.world().contains_resource::<Messages<PracticeAction>>() {
        app.add_message::<PracticeAction>();
    }
    app.add_message::<JudgmentEvent>()
        .add_message::<NoteMissed>()
        .add_message::<EmptyHit>()
        .add_message::<gameplay_drums::practice::ab_loop::PracticeLoopCompleted>()
        .init_resource::<gameplay_drums::practice::stats::LastFinalizedAttempt>()
        .init_resource::<gameplay_drums::practice::toast::ToastQueue>()
        .add_systems(
            Update,
            gameplay_drums::practice::ramp::handle_toggle_ramp
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            gameplay_drums::practice::ab_loop::loop_watcher
                .before(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            (
                gameplay_drums::practice::stats::track_attempt_stats,
                gameplay_drums::practice::stats::wrap_micro_report,
                gameplay_drums::practice::ramp::apply_ramp,
            )
                .chain()
                .after(gameplay_drums::seek::apply_seek_system)
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<PracticeSession>),
        );
}

fn send_practice_action(app: &mut App, action: PracticeAction) {
    app.world_mut()
        .resource_mut::<Messages<PracticeAction>>()
        .write(action);
}

fn looped_session(rate: f32) -> PracticeSession {
    let mut s = PracticeSession {
        transport: PracticeTransport {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            preroll: gameplay_drums::practice::session::PrerollSetting::Off,
            user_tempo: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    s.trainer.arm_ramp();
    s.trainer.ramp.step_tempo = rate;
    s.current_attempt.start_ms = 2_000;
    s
}

/// Run the clock past B so the loop watcher rolls one attempt.
fn finish_loop_pass(app: &mut App, perfect_hits: u32) {
    for _ in 0..perfect_hits {
        app.world_mut()
            .resource_mut::<Messages<JudgmentEvent>>()
            .write(JudgmentEvent {
                lane: 3,
                kind: dtx_scoring::JudgmentKind::Perfect,
                delta_ms: 0,
                chip_idx: 0, // chip 0 sits at 2000ms — inside the loop
            });
    }
    if perfect_hits == 0 {
        app.world_mut()
            .resource_mut::<Messages<NoteMissed>>()
            .write(NoteMissed {
                lane: 3,
                audio_ms: 5_000,
                chip_idx: 1, // value=1.0 -> end of measure 1 == 4000ms, inside the 2000-6000 loop
            });
    }
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.update();
}

#[test]
fn ramp_steps_rate_up_after_clean_pass() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.70));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.75).abs() < 1e-6,
        "clean pass steps 0.70 → 0.75, got {}",
        session.effective_tempo()
    );
    assert!(session.trainer.ramp_armed());
}

#[test]
fn two_failed_passes_step_rate_down() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.80));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 0); // fail #1 → hold
    assert!((app.world().resource::<PracticeSession>().effective_tempo() - 0.80).abs() < 1e-6);
    finish_loop_pass(&mut app, 0); // fail #2 → step down
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.75).abs() < 1e-6,
        "second fail steps 0.80 → 0.75, got {}",
        session.effective_tempo()
    );
}

#[test]
fn toggle_ramp_without_loop_arms_over_whole_song() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    send_practice_action(&mut app, PracticeAction::ToggleRamp);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        session.trainer.ramp_armed(),
        "arming with no loop uses the whole song"
    );
    assert!((session.effective_tempo() - 0.70).abs() < 1e-6);
}

#[test]
fn toggle_ramp_with_loop_arms() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            loop_region: Some(LoopRegion {
                start_ms: 2_000,
                end_ms: 6_000,
            }),
            preroll: gameplay_drums::practice::session::PrerollSetting::Off,
            ..Default::default()
        },
        ..Default::default()
    });
    send_practice_action(&mut app, PracticeAction::ToggleRamp);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        session.trainer.ramp_armed(),
        "arming with an A/B loop must succeed"
    );
    assert!(
        (session.effective_tempo() - 0.70).abs() < 1e-6,
        "armed ramp starts at the configured start tempo"
    );
    assert!(
        (session.transport.user_tempo - 1.0).abs() < 1e-6,
        "arming must not touch the user's chosen tempo"
    );
}

#[test]
fn tempo_nudge_while_armed_disarms_and_nudges_user_tempo() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.transport.user_tempo = 1.0;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Minus);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        !session.trainer.ramp_armed(),
        "manual nudge disarms the ramp"
    );
    assert!(
        (session.transport.user_tempo - 0.95).abs() < 1e-6,
        "nudge applies to the user tempo (1.00 → 0.95)"
    );
}

#[test]
fn pre_roll_miss_is_excluded_from_attempt() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    // Attempt starts at 4000ms; chip 0 (at 2000ms) is pre-roll.
    let mut s = PracticeSession::default();
    s.current_attempt.start_ms = 4_000;
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(4_500));
    }
    app.world_mut()
        .resource_mut::<Messages<NoteMissed>>()
        .write(NoteMissed {
            lane: 3,
            audio_ms: 2_300,
            chip_idx: 0, // value=1.0 -> end of measure 0 == 2000ms < attempt start 4000 → pre-roll
        });
    app.world_mut()
        .resource_mut::<Messages<NoteMissed>>()
        .write(NoteMissed {
            lane: 3,
            audio_ms: 4_300,
            chip_idx: 1, // value=1.0 -> end of measure 1 == 4000ms >= 4000 → counts
        });
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(
        session.current_attempt.counts.miss, 1,
        "pre-roll miss must not count against the attempt"
    );
}

#[test]
fn empty_hits_accumulate_as_overhits() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(PracticeSession::default());
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(1_000));
    }
    app.world_mut()
        .resource_mut::<Messages<gameplay_drums::events::EmptyHit>>()
        .write(gameplay_drums::events::EmptyHit {
            lane: 3,
            audio_ms: 1_000,
        });
    app.world_mut()
        .resource_mut::<Messages<gameplay_drums::events::EmptyHit>>()
        .write(gameplay_drums::events::EmptyHit {
            lane: 4,
            audio_ms: 1_100,
        });
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(session.current_attempt.overhits, 2);
}

#[test]
fn manual_restart_does_not_step_the_ramp() {
    let mut app = build_app();
    add_action_wiring(&mut app);
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    app.world_mut().insert_resource(looped_session(0.70));
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    // A clean partial attempt, then a manual restart (R) — NOT a wrap.
    app.world_mut()
        .resource_mut::<Messages<JudgmentEvent>>()
        .write(JudgmentEvent {
            lane: 3,
            kind: dtx_scoring::JudgmentKind::Perfect,
            delta_ms: 0,
            chip_idx: 0,
        });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.70).abs() < 1e-6,
        "manual restart must never count as a ramp pass"
    );
}

#[test]
fn empty_loop_pass_makes_no_ramp_decision() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    // A previous clean pass already sits in history at this loop's start,
    // so the OLD stale-attempt bug would re-read it and step up.
    let mut s = looped_session(0.70);
    s.attempt_history
        .push(gameplay_drums::practice::session::AttemptRecord {
            start_ms: 2_000,
            end_ms: 6_000,
            tempo: 0.70,
            counts: Default::default(),
            overhits: 0,
            max_combo: 4,
            accuracy_pct: 100.0,
            mean_error_ms: 0.0,
            waited: 0,
            flow_pct: 0.0,
        });
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    // Wrap with ZERO judgments (current_attempt has no data → not recorded).
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.sync(Some(6_100));
    }
    app.update();
    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.effective_tempo() - 0.70).abs() < 1e-6,
        "an empty pass must not re-apply the previous attempt's accuracy"
    );
}

#[test]
fn ineligible_loop_does_not_record_or_advance_ramp_then_next_loop_recovers() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut session = looped_session(0.70);
    session.current_attempt_eligible = false;
    app.world_mut().insert_resource(session);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }

    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert!(session.attempt_history.is_empty());
    assert!((session.effective_tempo() - 0.70).abs() < 1e-6);
    assert!(session.current_attempt_eligible);

    finish_loop_pass(&mut app, 4);
    let session = app.world().resource::<PracticeSession>();
    assert_eq!(session.attempt_history.len(), 1);
    assert!((session.effective_tempo() - 0.75).abs() < 1e-6);
}

#[test]
fn no_loop_set_wraps_at_chart_end_as_implicit_loop() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(4)); // chart end ≈ 8000-10000ms
    app.world_mut().insert_resource(PracticeSession::default());
    let end = app
        .world()
        .resource::<gameplay_drums::timeline::ChipTimeline>()
        .end_ms;
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(end + 1_000)); // past chart end
    }
    app.update();
    let now = app.world().resource::<GameplayClock>().current_ms;
    assert!(
        now < end,
        "reaching chart end in practice wraps to the start (implicit loop): now={now} end={end}"
    );
}

#[test]
fn loop_wrap_pushes_a_micro_report_toast() {
    let mut app = build_app();
    add_ramp_wiring(&mut app);
    enter_performance(&mut app, chart_with_measures(8));
    let mut s = looped_session(0.70);
    s.trainer.disarm_ramp(); // report fires with or without ramp
    app.world_mut().insert_resource(s);
    {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(3_000));
    }
    finish_loop_pass(&mut app, 4);
    let toasts = app
        .world()
        .resource::<gameplay_drums::practice::toast::ToastQueue>();
    let report = toasts
        .iter()
        .find(|notification| notification.message.starts_with("pass "))
        .expect("wrap must push a micro-report toast");
    assert!(
        report.message.contains('%'),
        "report shows accuracy: {}",
        report.message
    );
    assert!(
        report.message.contains("ms"),
        "report shows mean error: {}",
        report.message
    );
}
