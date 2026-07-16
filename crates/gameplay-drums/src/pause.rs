//! Pause overlay (resume / retry / quit).
//!
//! `PauseState` (in `game-shell`) is orthogonal to `AppState`. Pressing Escape
//! during `AppState::Performance` toggles it. While paused the gameplay clock
//! is frozen (see `lib.rs`), input is dropped (see `input.rs`), the BGM
//! instance is paused, chart drum/layer voices are paused, and an overlay menu
//! handles resume, restart, settings, and exit actions.
//!
//! UX is redesigned (ADR-0014); mechanics-neutral. Loosely mirrors
//! `dtxpt/src/overlays/pause.rs`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bevy::ecs::system::SystemParam;
use bevy::{ecs::message::MessageId, prelude::*};
use bevy_kira_audio::prelude::AudioInstance;
use dtx_audio::{BgmHandle, DrumPolyphony};
use dtx_input::SystemVerb;
use dtx_ui::theme::Theme;
use game_shell::{
    request_transition, AppState, PauseState, PracticeIntent, PracticeOrigin,
    PracticeRecommendation, PracticeRequest, TransitionRequest,
};

use crate::resources::ActiveDrumSounds;

/// Root marker for the pause overlay. Practice Settings leaves this overlay
/// and enters the dedicated Editing phase.
#[derive(Component)]
pub struct PauseOverlay;

/// One selectable pause-menu row. The set differs between normal play and
/// practice — see [`pause_items`].
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseItemKind {
    Resume,
    RestartSong,
    PracticeThisSection,
    QuickSettings,
    ReturnToSongSelect,
    RestartLoop,
    PracticeSettings,
    ExitToSongSelect,
}

impl PauseItemKind {
    fn label(self) -> &'static str {
        match self {
            PauseItemKind::Resume => "Resume",
            PauseItemKind::RestartSong => "Restart Song",
            PauseItemKind::PracticeThisSection => "Practice This Section",
            PauseItemKind::QuickSettings => "Quick Settings",
            PauseItemKind::ReturnToSongSelect => "Return to Song Select",
            PauseItemKind::RestartLoop => "Restart Loop",
            PauseItemKind::PracticeSettings => "Practice Settings",
            PauseItemKind::ExitToSongSelect => "Exit to Song Select",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseContext {
    Normal,
    Practice,
}

const NORMAL_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::RestartSong,
    PauseItemKind::PracticeThisSection,
    PauseItemKind::QuickSettings,
    PauseItemKind::ReturnToSongSelect,
];
const PRACTICE_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::RestartLoop,
    PauseItemKind::PracticeSettings,
    PauseItemKind::ExitToSongSelect,
];

pub fn pause_items(context: PauseContext) -> &'static [PauseItemKind] {
    match context {
        PauseContext::Normal => NORMAL_ITEMS,
        PauseContext::Practice => PRACTICE_ITEMS,
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSettingKind {
    ScrollSpeed,
    LaneVisibility,
    BgmVolume,
    InputOffset,
    Back,
}

const QUICK_SETTINGS: &[QuickSettingKind] = &[
    QuickSettingKind::ScrollSpeed,
    QuickSettingKind::LaneVisibility,
    QuickSettingKind::BgmVolume,
    QuickSettingKind::InputOffset,
    QuickSettingKind::Back,
];

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PauseView {
    #[default]
    Menu,
    QuickSettings,
}

#[derive(Component)]
struct PauseMenuContent;

#[derive(Component)]
struct QuickSettingsContent;

#[derive(Component, Debug, Clone, Copy)]
struct QuickAdjustButton {
    setting: QuickSettingKind,
    direction: i32,
}

/// Currently highlighted pause-menu row.
#[derive(Resource, Default)]
pub struct PauseSelection(pub usize);

#[derive(Resource, Default)]
struct QuickSettingsSelection(usize);

#[derive(Resource, Default)]
pub struct PausedRestart {
    target_ms: Option<i64>,
    attempt_start_ms: i64,
    seek_revision: u64,
    seek_id: Option<MessageId<crate::seek::SeekToChartTime>>,
}

impl PausedRestart {
    pub(crate) fn owns_acknowledged_seek(
        &self,
        seek: &crate::seek::SeekToChartTime,
        acknowledgement: &crate::seek::SeekAcknowledgement,
    ) -> bool {
        self.target_ms == Some(seek.target_ms)
            && seek.snap.is_none()
            && seek.attempt_start_ms.is_none()
            && acknowledgement.revision == self.seek_revision
            && acknowledgement.request == Some(*seek)
    }
}

#[derive(Resource, Default)]
pub struct CancelledPausedRestart {
    seek_id: Option<MessageId<crate::seek::SeekToChartTime>>,
}

impl CancelledPausedRestart {
    pub(crate) fn owns(&self, seek_id: MessageId<crate::seek::SeekToChartTime>) -> bool {
        self.seek_id == Some(seek_id)
    }
}

#[derive(SystemParam)]
pub(crate) struct PauseMenuState<'w> {
    selection: ResMut<'w, PauseSelection>,
    quick_selection: ResMut<'w, QuickSettingsSelection>,
    view: ResMut<'w, PauseView>,
    next_pause: ResMut<'w, NextState<PauseState>>,
    practice: Option<ResMut<'w, crate::practice::PracticeSession>>,
    timeline: Res<'w, crate::timeline::ChipTimeline>,
    clock: Res<'w, crate::resources::GameplayClock>,
    intent: ResMut<'w, PracticeIntent>,
    completed_run: ResMut<'w, game_shell::CompletedRunContext>,
    normal_events: ResMut<'w, crate::results_analysis::NormalPlayEventStream>,
    pending_inputs: ResMut<'w, crate::input::PendingLaneInputs>,
    lane_hits: ResMut<'w, Messages<crate::events::LaneHit>>,
    input_hits: ResMut<'w, Messages<crate::events::InputHit>>,
}

#[derive(SystemParam)]
pub(crate) struct PauseMenuUi<'w, 's> {
    menu_rows: Query<
        'w,
        's,
        (
            &'static PauseItemKind,
            &'static mut Text,
            &'static mut TextColor,
        ),
        Without<QuickSettingKind>,
    >,
    quick_rows: Query<
        'w,
        's,
        (
            &'static QuickSettingKind,
            &'static mut Text,
            &'static mut TextColor,
        ),
        Without<PauseItemKind>,
    >,
    menu_visibility: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<PauseMenuContent>, Without<QuickSettingsContent>),
    >,
    quick_visibility: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<QuickSettingsContent>, Without<PauseMenuContent>),
    >,
}

pub fn practice_request_at(
    timeline: &crate::timeline::ChipTimeline,
    playhead_ms: i64,
) -> PracticeRequest {
    let current = timeline
        .bar_ms
        .partition_point(|bar_ms| *bar_ms <= playhead_ms)
        .saturating_sub(1);
    let start = timeline
        .bar_ms
        .get(current.saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let end = timeline
        .bar_ms
        .get(current.saturating_add(2))
        .copied()
        .unwrap_or(timeline.end_ms)
        .max(start.saturating_add(1));
    PracticeRequest {
        origin: PracticeOrigin::NormalPause,
        seed: game_shell::PracticeSeed::Recommended(PracticeRecommendation::weak_section(
            start, end, None,
        )),
    }
}

/// Minimum gap between two accepted hits of the SAME system verb. Pads
/// double-fire (flam/retrigger 20-40 ms apart), and an un-guarded verb would
/// toggle pause straight back off. Same reason — and same window — as
/// game_shell::navigation's pad-nav debounce, which guards pad *navigation*.
const VERB_DEBOUNCE: Duration = Duration::from_millis(80);

/// Per-verb min-interval guard for the system-verb path.
#[derive(Resource, Debug)]
pub struct VerbGuard {
    min_gap: Duration,
    last: HashMap<SystemVerb, Instant>,
}

impl Default for VerbGuard {
    fn default() -> Self {
        Self {
            min_gap: VERB_DEBOUNCE,
            last: HashMap::new(),
        }
    }
}

impl VerbGuard {
    /// True if `verb` fired at `now` may act. Keyed per verb, so a Pause never
    /// swallows a Restart. Pure in `now` — the caller supplies the clock.
    pub fn accept(&mut self, verb: SystemVerb, now: Instant) -> bool {
        if let Some(last) = self.last.get(&verb) {
            if now.saturating_duration_since(*last) < self.min_gap {
                return false;
            }
        }
        self.last.insert(verb, now);
        true
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PauseSelection>()
        .init_resource::<QuickSettingsSelection>()
        .init_resource::<PauseView>()
        .init_resource::<PausedRestart>()
        .init_resource::<CancelledPausedRestart>()
        .init_resource::<VerbGuard>()
        // Always start a performance un-paused.
        .add_systems(
            OnEnter(AppState::Performance),
            (force_running, reset_paused_restart),
        )
        .add_systems(
            OnExit(AppState::Performance),
            (force_running, reset_paused_restart),
        )
        .add_systems(
            Update,
            // Both write NextState<PauseState>, but they compute the same
            // transition from the same current state, so a same-frame Escape +
            // pad hit is idempotent — no ordering constraint needed.
            (toggle_pause, system_verb_pause, system_verb_restart)
                .run_if(in_state(AppState::Performance)),
        )
        .add_systems(
            OnEnter(PauseState::Paused),
            (
                pause_chart_audio,
                clear_gameplay_input_queues,
                spawn_overlay,
            )
                .chain(),
        )
        .add_systems(
            OnExit(PauseState::Paused),
            (
                clear_gameplay_input_queues,
                cancel_paused_restart,
                resume_chart_audio,
                despawn_overlay,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                pause_kb_emit,
                pause_pointer_emit,
                pause_menu_input,
                refresh_pause_legend,
            )
                .chain()
                .run_if(in_state(PauseState::Paused)),
        )
        .add_systems(
            FixedUpdate,
            finish_paused_restart
                .after(crate::seek::apply_seek_system)
                .after(crate::practice::stats::track_attempt_stats)
                .run_if(in_state(PauseState::Paused)),
        )
        .add_systems(
            FixedUpdate,
            clear_cancelled_paused_restart
                .after(crate::seek::apply_seek_system)
                .after(crate::practice::stats::track_attempt_stats),
        );
}

fn force_running(mut next: ResMut<NextState<PauseState>>) {
    next.set(PauseState::Running);
}

fn reset_paused_restart(mut restart: ResMut<PausedRestart>) {
    *restart = PausedRestart::default();
}

fn cancel_paused_restart(
    mut restart: ResMut<PausedRestart>,
    mut cancelled: ResMut<CancelledPausedRestart>,
) {
    cancelled.seek_id = restart.seek_id.take();
    *restart = PausedRestart::default();
}

fn clear_cancelled_paused_restart(mut cancelled: ResMut<CancelledPausedRestart>) {
    cancelled.seek_id = None;
}

fn clear_gameplay_input_queues(
    mut pending: ResMut<crate::input::PendingLaneInputs>,
    mut lane_hits: ResMut<Messages<crate::events::LaneHit>>,
    mut input_hits: ResMut<Messages<crate::events::InputHit>>,
    mut judgments: ResMut<Messages<crate::events::JudgmentEvent>>,
    mut misses: ResMut<Messages<crate::events::NoteMissed>>,
    mut empty_hits: ResMut<Messages<crate::events::EmptyHit>>,
) {
    crate::input::clear_pending_lane_inputs_now(&mut pending);
    lane_hits.clear();
    input_hits.clear();
    judgments.clear();
    misses.clear();
    empty_hits.clear();
}

fn finish_paused_restart(
    mut restart: ResMut<PausedRestart>,
    acknowledgement: Res<crate::seek::SeekAcknowledgement>,
    mut practice: Option<ResMut<crate::practice::PracticeSession>>,
    mut last_seek_from: ResMut<crate::seek::LastSeekFrom>,
    mut combo: ResMut<crate::resources::Combo>,
    mut finalized: ResMut<crate::practice::stats::LastFinalizedAttempt>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    let Some(target_ms) = restart.target_ms else {
        return;
    };
    if acknowledgement.revision < restart.seek_revision {
        return;
    }
    if acknowledgement.revision != restart.seek_revision
        || acknowledgement.request.is_none_or(|request| {
            request.target_ms != target_ms
                || request.snap.is_some()
                || request.attempt_start_ms.is_some()
        })
    {
        warn!("paused restart cancelled by a competing seek");
        *restart = PausedRestart::default();
        return;
    }
    match acknowledgement.result {
        Some(crate::seek::SeekResult::Applied { resolved_ms }) if resolved_ms == target_ms => {}
        Some(crate::seek::SeekResult::Applied { resolved_ms }) => {
            warn!("paused restart cancelled: target {target_ms} resolved to {resolved_ms}");
            last_seek_from.0 = None;
            *restart = PausedRestart::default();
            return;
        }
        Some(crate::seek::SeekResult::Rejected(reason)) => {
            warn!("paused restart cancelled: seek rejected ({reason:?})");
            *restart = PausedRestart::default();
            return;
        }
        None => return,
    }
    let Some(session) = practice.as_deref_mut() else {
        *restart = PausedRestart::default();
        return;
    };
    let end_ms = last_seek_from.0.take().unwrap_or(target_ms);
    finalized.0 = session.roll_attempt(end_ms, restart.attempt_start_ms);
    combo.current = 0;
    *restart = PausedRestart::default();
    next_pause.set(PauseState::Running);
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut view: Option<ResMut<PauseView>>,
    flow: Option<Res<crate::practice::PracticeFlow>>,
    editor_open: Option<Res<crate::editor::EditorOpen>>,
) {
    if keys.just_pressed(KeyCode::Escape)
        && !editor_open.is_some_and(|editor| editor.0)
        && system_verbs_active(flow.as_deref())
    {
        if *state.get() == PauseState::Paused
            && view
                .as_deref()
                .is_some_and(|view| *view == PauseView::QuickSettings)
        {
            if let Some(view) = view.as_deref_mut() {
                *view = PauseView::Menu;
            }
            return;
        }
        toggle(state.get(), &mut next);
    }
}

/// Did this frame's batch carry `verb`? Counts — never `any()`, which
/// short-circuits and leaves the rest of the batch unread to replay (and
/// re-toggle) on the next frame. A pad retrigger really does put two hits in
/// one frame, so the reader must be drained to the end.
fn drain_verb(hits: &mut MessageReader<crate::events::SystemVerbHit>, verb: SystemVerb) -> bool {
    hits.read().filter(|hit| hit.verb == verb).count() > 0
}

/// `SystemVerb::Pause` from a pad or a bound key — the distant-kit equivalent of
/// Escape. Toggles both ways, so firing it while paused resumes. Always drains
/// during Performance, but only acts while the gameplay surface owns the verb.
fn system_verb_pause(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut guard: ResMut<VerbGuard>,
    flow: Option<Res<crate::practice::PracticeFlow>>,
    editor_open: Option<Res<crate::editor::EditorOpen>>,
) {
    // Drain first: the guard decides whether to ACT, never whether to READ.
    if !drain_verb(&mut hits, SystemVerb::Pause) {
        return;
    }
    if editor_open.is_some_and(|editor| editor.0) || !system_verbs_active(flow.as_deref()) {
        return;
    }
    if !guard.accept(SystemVerb::Pause, Instant::now()) {
        return; // pad retrigger a few frames later — not a second press
    }
    toggle(state.get(), &mut next);
}

/// `SystemVerb::Restart` — re-request `SongLoading`, exactly as the pause menu's
/// Retry row does, preserving `SelectedSong` and `PracticeIntent`. Fires during
/// Performance whether running or paused.
fn system_verb_restart(
    mut hits: MessageReader<crate::events::SystemVerbHit>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut guard: ResMut<VerbGuard>,
    flow: Option<Res<crate::practice::PracticeFlow>>,
    editor_open: Option<Res<crate::editor::EditorOpen>>,
) {
    if !drain_verb(&mut hits, SystemVerb::Restart) {
        return;
    }
    if editor_open.is_some_and(|editor| editor.0) || !system_verbs_active(flow.as_deref()) {
        return;
    }
    if !guard.accept(SystemVerb::Restart, Instant::now()) {
        return;
    }
    next_pause.set(PauseState::Running);
    request_transition(&mut requests, AppState::SongLoading);
}

fn system_verbs_active(flow: Option<&crate::practice::PracticeFlow>) -> bool {
    flow.is_none_or(|flow| flow.phase == crate::practice::PracticePhase::Running)
}

/// Shared by Escape and `SystemVerb::Pause`.
fn toggle(state: &PauseState, next: &mut NextState<PauseState>) {
    match state {
        PauseState::Running => next.set(PauseState::Paused),
        PauseState::Paused => next.set(PauseState::Running),
    }
}

pub(crate) fn pause_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::pause_audio_instance(instances, handle);
    }
    dtx_audio::pause_polyphony(instances, polyphony);
    active.pause_all(instances);
}

pub(crate) fn resume_all_chart_audio(
    bgm: &BgmHandle,
    polyphony: &DrumPolyphony,
    active: &ActiveDrumSounds,
    instances: &mut Assets<AudioInstance>,
) {
    if let Some(handle) = &bgm.instance {
        dtx_audio::resume_audio_instance(instances, handle);
    }
    dtx_audio::resume_polyphony(instances, polyphony);
    active.resume_all(instances);
}

fn pause_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    pause_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn resume_chart_audio(
    bgm: Res<BgmHandle>,
    polyphony: Res<DrumPolyphony>,
    active: Res<ActiveDrumSounds>,
    mut instances: ResMut<Assets<AudioInstance>>,
    wait_state: Option<Res<crate::practice::wait::WaitState>>,
    flow: Option<Res<crate::practice::PracticeFlow>>,
) {
    if !should_resume_chart_audio(wait_state.as_deref(), flow.as_deref()) {
        return;
    }
    resume_all_chart_audio(&bgm, &polyphony, &active, &mut instances);
}

fn should_resume_chart_audio(
    wait_state: Option<&crate::practice::wait::WaitState>,
    flow: Option<&crate::practice::PracticeFlow>,
) -> bool {
    wait_state.is_none_or(|state| !state.halted())
        && flow.is_none_or(|flow| {
            flow.phase == crate::practice::PracticePhase::Running
                || flow.preview == crate::practice::PreviewState::Playing
        })
}

fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    mut quick_selection: ResMut<QuickSettingsSelection>,
    mut view: ResMut<PauseView>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    selection.0 = 0;
    quick_selection.0 = 0;
    *view = PauseView::Menu;
    let context = if practice.is_some() {
        PauseContext::Practice
    } else {
        PauseContext::Normal
    };
    let items = pause_items(context);
    let theme = Theme::default();
    commands
        .spawn((
            PauseOverlay,
            dtx_ui::ModalDialog::new(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        if matches!(
                            item,
                            PauseItemKind::ReturnToSongSelect | PauseItemKind::ExitToSongSelect
                        ) {
                            dtx_ui::DialogAction::Destructive
                        } else {
                            dtx_ui::DialogAction::Custom(index as u16)
                        }
                    })
                    .collect(),
            ),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(crate::ui_z::PAUSE),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("PAUSED"),
                Theme::title_font(),
                dtx_ui::SemanticText(dtx_ui::TypographyRole::Display),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            root.spawn((
                PauseMenuContent,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
            ))
            .with_children(|menu| {
                for (index, item) in items.iter().enumerate() {
                    let tone = if matches!(
                        item,
                        PauseItemKind::ReturnToSongSelect | PauseItemKind::ExitToSongSelect
                    ) {
                        dtx_ui::InteractionTone::Destructive
                    } else {
                        dtx_ui::InteractionTone::Focus
                    };
                    let action = if matches!(
                        item,
                        PauseItemKind::ReturnToSongSelect | PauseItemKind::ExitToSongSelect
                    ) {
                        dtx_ui::DialogAction::Destructive
                    } else {
                        dtx_ui::DialogAction::Custom(index as u16)
                    };
                    menu.spawn((
                        *item,
                        Button,
                        dtx_ui::ActionButton::new(action, tone),
                        Text::new(item.label()),
                        Theme::hud_font(),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Hud),
                        TextColor(theme.text_secondary),
                    ));
                }
            });
            root.spawn((
                QuickSettingsContent,
                Visibility::Hidden,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
            ))
            .with_children(|quick| {
                for (index, setting) in QUICK_SETTINGS.iter().enumerate() {
                    let mut row = quick.spawn((
                        *setting,
                        Button,
                        dtx_ui::ActionButton::new(
                            dtx_ui::DialogAction::Custom(index as u16),
                            dtx_ui::InteractionTone::Focus,
                        ),
                        Text::new(""),
                        Theme::hud_font(),
                        dtx_ui::SemanticText(dtx_ui::TypographyRole::Hud),
                        TextColor(theme.text_secondary),
                    ));
                    if *setting != QuickSettingKind::Back {
                        row.with_children(|row| {
                            for (label, direction) in [("−", -1), ("+", 1)] {
                                row.spawn((
                                    QuickAdjustButton {
                                        setting: *setting,
                                        direction,
                                    },
                                    Button,
                                    Text::new(label),
                                    Theme::hud_font(),
                                    dtx_ui::SemanticText(dtx_ui::TypographyRole::Hud),
                                    TextColor(theme.text_primary),
                                ));
                            }
                        });
                    }
                }
            });
            let legend = pause_legend(PauseView::Menu, midi.is_some_and(|m| m.0));
            dtx_ui::widget::nav_legend::spawn_nav_legend(root, &theme, legend);
        });
}

fn pause_legend(
    view: PauseView,
    midi: bool,
) -> &'static [dtx_ui::widget::nav_legend::LegendItem<'static>] {
    match (view, midi) {
        (PauseView::Menu, true) => &[
            ("HH", "up"),
            ("CY", "down"),
            ("BD", "select"),
            ("SD", "resume"),
        ],
        (PauseView::Menu, false) => &[("↑/↓", "move"), ("Enter", "select"), ("Esc", "resume")],
        (PauseView::QuickSettings, true) => &[
            ("HH/CY", "move"),
            ("HT/LT", "adjust"),
            ("BD", "confirm"),
            ("SD", "back"),
        ],
        (PauseView::QuickSettings, false) => &[
            ("↑/↓", "move"),
            ("←/→", "adjust"),
            ("Enter", "confirm"),
            ("Esc", "back"),
        ],
    }
}

fn refresh_pause_legend(
    mut commands: Commands,
    view: Res<PauseView>,
    midi: Option<Res<game_shell::MidiConnected>>,
    overlays: Query<Entity, With<PauseOverlay>>,
    legends: Query<Entity, With<dtx_ui::widget::nav_legend::NavLegend>>,
    parents: Query<&ChildOf>,
) {
    if !view.is_changed()
        && !midi
            .as_ref()
            .is_some_and(|connected| connected.is_changed())
    {
        return;
    }
    let Ok(root) = overlays.single() else {
        return;
    };
    for legend in &legends {
        let mut ancestor = legend;
        while let Ok(parent) = parents.get(ancestor) {
            ancestor = parent.parent();
            if ancestor == root {
                commands.entity(legend).despawn();
                break;
            }
        }
    }
    let theme = Theme::default();
    commands.entity(root).with_children(|overlay| {
        dtx_ui::widget::nav_legend::spawn_nav_legend(
            overlay,
            &theme,
            pause_legend(*view, midi.is_some_and(|connected| connected.0)),
        );
    });
}

fn despawn_overlay(mut commands: Commands, overlays: Query<Entity, With<PauseOverlay>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

/// Keyboard → `NavAction` for the pause overlay. Esc keeps its own toggle path.
fn pause_kb_emit(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<game_shell::NavAction>) {
    use game_shell::{NavAction, NavSource, SystemVerb};
    let verb = if keys.just_pressed(KeyCode::ArrowDown) {
        SystemVerb::NavigateDown
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        SystemVerb::NavigateUp
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        SystemVerb::Decrease
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        SystemVerb::Increase
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        SystemVerb::Confirm
    } else {
        return;
    };
    out.write(NavAction {
        verb,
        source: NavSource::Keyboard,
        coarse: false,
    });
}

fn pause_pointer_emit(
    practice: Option<Res<crate::practice::PracticeSession>>,
    mut selection: ResMut<PauseSelection>,
    mut quick_selection: ResMut<QuickSettingsSelection>,
    mut view: ResMut<PauseView>,
    menu_rows: Query<
        (&PauseItemKind, &Interaction),
        (Changed<Interaction>, Without<QuickSettingKind>),
    >,
    quick_rows: Query<
        (&QuickSettingKind, &Interaction),
        (Changed<Interaction>, Without<PauseItemKind>),
    >,
    adjust_buttons: Query<
        (&QuickAdjustButton, &Interaction),
        (
            Changed<Interaction>,
            Without<PauseItemKind>,
            Without<QuickSettingKind>,
        ),
    >,
    mut out: MessageWriter<game_shell::NavAction>,
) {
    use game_shell::{NavAction, NavSource, SystemVerb};
    let context = if practice.is_some() {
        PauseContext::Practice
    } else {
        PauseContext::Normal
    };
    for (item, interaction) in &menu_rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(index) = pause_items(context).iter().position(|row| row == item) {
            selection.0 = index;
            *view = PauseView::Menu;
            out.write(NavAction {
                verb: SystemVerb::Confirm,
                source: NavSource::Keyboard,
                coarse: false,
            });
        }
    }
    for (setting, interaction) in &quick_rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(index) = QUICK_SETTINGS.iter().position(|row| row == setting) {
            quick_selection.0 = index;
            *view = PauseView::QuickSettings;
            out.write(NavAction {
                verb: if *setting == QuickSettingKind::Back {
                    SystemVerb::Confirm
                } else {
                    SystemVerb::Increase
                },
                source: NavSource::Keyboard,
                coarse: false,
            });
        }
    }
    for (button, interaction) in &adjust_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(index) = QUICK_SETTINGS
            .iter()
            .position(|setting| *setting == button.setting)
        {
            quick_selection.0 = index;
            *view = PauseView::QuickSettings;
            out.write(NavAction {
                verb: if button.direction < 0 {
                    SystemVerb::Decrease
                } else {
                    SystemVerb::Increase
                },
                source: NavSource::Keyboard,
                coarse: false,
            });
        }
    }
}

pub(crate) fn pause_menu_input(
    mut commands: Commands,
    mut actions: MessageReader<game_shell::NavAction>,
    mut state: PauseMenuState,
    mut requests: MessageWriter<TransitionRequest>,
    mut open_settings: MessageWriter<crate::practice::OpenPracticeSettings>,
    mut seeks: MessageWriter<crate::seek::SeekToChartTime>,
    mut paused_restart: ResMut<PausedRestart>,
    seek_acknowledgement: Res<crate::seek::SeekAcknowledgement>,
    mut ui: PauseMenuUi,
    mut quick_runtime: crate::perf_hotkeys::PauseQuickSettings,
) {
    use game_shell::SystemVerb;
    let context = if state.practice.is_some() {
        PauseContext::Practice
    } else {
        PauseContext::Normal
    };
    let items = pause_items(context);
    let mut confirm = false;
    let mut resume = false;
    let mut adjustment = 0;
    for action in actions.read() {
        match *state.view {
            PauseView::Menu => match action.verb {
                SystemVerb::NavigateDown => {
                    state.selection.0 = (state.selection.0 + 1) % items.len()
                }
                SystemVerb::NavigateUp => {
                    state.selection.0 = (state.selection.0 + items.len() - 1) % items.len();
                }
                SystemVerb::Confirm => confirm = true,
                SystemVerb::Back => resume = true,
                _ => {}
            },
            PauseView::QuickSettings => match action.verb {
                SystemVerb::NavigateDown => {
                    state.quick_selection.0 = (state.quick_selection.0 + 1) % QUICK_SETTINGS.len();
                }
                SystemVerb::NavigateUp => {
                    state.quick_selection.0 =
                        (state.quick_selection.0 + QUICK_SETTINGS.len() - 1) % QUICK_SETTINGS.len();
                }
                SystemVerb::Decrease => adjustment = -1,
                SystemVerb::Increase => adjustment = 1,
                SystemVerb::Confirm => confirm = true,
                SystemVerb::Back => *state.view = PauseView::Menu,
                _ => {}
            },
        }
    }

    if resume {
        clear_menu_gameplay_queues(&mut state);
        state.next_pause.set(PauseState::Running);
    }
    match *state.view {
        PauseView::Menu if resume => {}
        PauseView::Menu if confirm => {
            clear_menu_gameplay_queues(&mut state);
            let selected = items[state.selection.0 % items.len()];
            match selected {
                PauseItemKind::Resume => {
                    clear_menu_gameplay_queues(&mut state);
                    state.next_pause.set(PauseState::Running);
                }
                PauseItemKind::RestartSong => {
                    state.next_pause.set(PauseState::Running);
                    request_transition(&mut requests, AppState::SongLoading);
                }
                PauseItemKind::PracticeThisSection => {
                    let request = practice_request_at(&state.timeline, state.clock.current_ms);
                    let mut session = crate::practice::PracticeSession::default();
                    let mut draft = crate::practice::PracticeDraft::default();
                    let mut flow = crate::practice::PracticeFlow::default();
                    crate::practice::begin_practice_setup(
                        &request,
                        &mut session,
                        &mut draft,
                        &mut flow,
                    );
                    *state.intent = PracticeIntent::Request(request);
                    *state.completed_run = game_shell::CompletedRunContext::default();
                    state.normal_events.clear();
                    commands.insert_resource(crate::practice::PracticeSourceCatalog {
                        recommended: Some(draft.clone()),
                    });
                    commands.insert_resource(session);
                    commands.insert_resource(draft);
                    commands.insert_resource(flow);
                    state.next_pause.set(PauseState::Running);
                }
                PauseItemKind::QuickSettings => {
                    state.quick_selection.0 = 0;
                    *state.view = PauseView::QuickSettings;
                }
                PauseItemKind::ReturnToSongSelect | PauseItemKind::ExitToSongSelect => {
                    state.next_pause.set(PauseState::Running);
                    request_transition(&mut requests, AppState::SongSelect);
                }
                PauseItemKind::RestartLoop => {
                    if let Some(session) = state.practice.as_deref_mut() {
                        session.invalidate_current_attempt();
                        let attempt_start_ms = session
                            .transport
                            .loop_region
                            .map(|region| region.start_ms)
                            .unwrap_or(session.current_attempt.start_ms);
                        let target_ms = crate::practice::session::preroll_target(
                            &state.timeline,
                            session.transport.preroll,
                            attempt_start_ms,
                        );
                        let seek_id = seeks.write(crate::seek::SeekToChartTime {
                            target_ms,
                            snap: None,
                            attempt_start_ms: None,
                        });
                        paused_restart.target_ms = Some(target_ms);
                        paused_restart.attempt_start_ms = attempt_start_ms;
                        paused_restart.seek_revision =
                            seek_acknowledgement.revision.wrapping_add(1);
                        paused_restart.seek_id = Some(seek_id);
                    }
                }
                PauseItemKind::PracticeSettings => {
                    open_settings.write(crate::practice::OpenPracticeSettings);
                }
            }
        }
        PauseView::QuickSettings => {
            let selected = QUICK_SETTINGS[state.quick_selection.0 % QUICK_SETTINGS.len()];
            if confirm && selected == QuickSettingKind::Back {
                *state.view = PauseView::Menu;
            } else if selected != QuickSettingKind::Back && confirm {
                adjustment = 1;
            }
            if adjustment != 0 && selected != QuickSettingKind::Back {
                quick_runtime.adjust(selected, adjustment);
            }
        }
        PauseView::Menu => {}
    }

    let quick_open = *state.view == PauseView::QuickSettings;
    if let Ok(mut visibility) = ui.menu_visibility.single_mut() {
        *visibility = if quick_open {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if let Ok(mut visibility) = ui.quick_visibility.single_mut() {
        *visibility = if quick_open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let theme = Theme::default();
    let selected_menu = items[state.selection.0 % items.len()];
    for (item, mut text, mut color) in &mut ui.menu_rows {
        let selected = *item == selected_menu;
        text.0 = if selected {
            format!("{} {}", dtx_ui::StateMarker::Focus.label(), item.label())
        } else {
            format!("  {}", item.label())
        };
        color.0 = if selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }
    let selected_quick = QUICK_SETTINGS[state.quick_selection.0 % QUICK_SETTINGS.len()];
    for (setting, mut text, mut color) in &mut ui.quick_rows {
        let selected = *setting == selected_quick;
        let value = quick_runtime.value(*setting);
        text.0 = if selected {
            format!("{} {value}", dtx_ui::StateMarker::Focus.label())
        } else {
            format!("  {value}")
        };
        color.0 = if selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }
}

fn clear_menu_gameplay_queues(state: &mut PauseMenuState) {
    crate::input::clear_pending_lane_inputs_now(&mut state.pending_inputs);
    state.lane_hits.clear();
    state.input_hits.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::wait::{WaitPhase, WaitSet, WaitState};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn esc_opener_pauses() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Running));
        world.init_resource::<NextState<PauseState>>();
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn esc_while_paused_resumes() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Paused));
        world.init_resource::<NextState<PauseState>>();
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn esc_in_quick_settings_returns_to_pause_menu() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Paused));
        world.init_resource::<NextState<PauseState>>();
        world.insert_resource(PauseView::QuickSettings);

        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");

        assert_eq!(*world.resource::<PauseView>(), PauseView::Menu);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn esc_is_owned_by_practice_setup_and_editing() {
        for phase in [
            crate::practice::PracticePhase::Setup,
            crate::practice::PracticePhase::Editing,
        ] {
            let mut world = World::new();
            let mut keys = ButtonInput::<KeyCode>::default();
            keys.press(KeyCode::Escape);
            world.insert_resource(keys);
            world.insert_resource(State::new(PauseState::Running));
            world.init_resource::<NextState<PauseState>>();
            let mut flow = crate::practice::PracticeFlow::default();
            flow.phase = phase;
            world.insert_resource(flow);

            world
                .run_system_once(toggle_pause)
                .expect("toggle_pause runs");

            assert!(matches!(
                world.resource::<NextState<PauseState>>(),
                NextState::Unchanged
            ));
        }
    }

    #[test]
    fn pause_items_match_context_contracts() {
        assert_eq!(
            pause_items(PauseContext::Normal),
            &[
                PauseItemKind::Resume,
                PauseItemKind::RestartSong,
                PauseItemKind::PracticeThisSection,
                PauseItemKind::QuickSettings,
                PauseItemKind::ReturnToSongSelect,
            ]
        );
        assert_eq!(
            pause_items(PauseContext::Practice),
            &[
                PauseItemKind::Resume,
                PauseItemKind::RestartLoop,
                PauseItemKind::PracticeSettings,
                PauseItemKind::ExitToSongSelect,
            ]
        );
    }

    #[test]
    fn pause_legend_is_generated_from_the_active_view() {
        assert_eq!(
            pause_legend(PauseView::Menu, true),
            &[
                ("HH", "up"),
                ("CY", "down"),
                ("BD", "select"),
                ("SD", "resume"),
            ]
        );
        assert_eq!(
            pause_legend(PauseView::QuickSettings, true),
            &[
                ("HH/CY", "move"),
                ("HT/LT", "adjust"),
                ("BD", "confirm"),
                ("SD", "back"),
            ]
        );
        assert_eq!(
            pause_legend(PauseView::QuickSettings, false),
            &[
                ("↑/↓", "move"),
                ("←/→", "adjust"),
                ("Enter", "confirm"),
                ("Esc", "back"),
            ]
        );
    }

    fn descendant_count<T: Component>(world: &mut World, root: Entity) -> usize {
        let mut query = world.query_filtered::<Entity, With<T>>();
        query
            .iter(world)
            .filter(|entity| {
                let mut current = *entity;
                loop {
                    if current == root {
                        return true;
                    }
                    let Some(parent) = world.get::<ChildOf>(current) else {
                        return false;
                    };
                    current = parent.parent();
                }
            })
            .count()
    }

    fn descendant_text(world: &mut World, root: Entity) -> Vec<String> {
        let mut query = world.query::<(Entity, &Text)>();
        query
            .iter(world)
            .filter_map(|(entity, text)| {
                let mut current = entity;
                loop {
                    if current == root {
                        return Some(text.0.clone());
                    }
                    let parent = world.get::<ChildOf>(current)?;
                    current = parent.parent();
                }
            })
            .collect()
    }

    fn spawn_test_pause_overlay(mut commands: Commands) {
        commands.spawn(PauseOverlay).with_children(|root| {
            dtx_ui::widget::nav_legend::spawn_nav_legend(
                root,
                &Theme::default(),
                pause_legend(PauseView::Menu, false),
            );
        });
    }

    #[test]
    fn pause_legend_refresh_preserves_running_mini_strip_through_resume() {
        let mut world = World::new();
        world.insert_resource(PauseView::QuickSettings);
        world.insert_resource(game_shell::MidiConnected(false));
        world
            .run_system_once(crate::practice::hud::mini_strip::spawn_mini_strip)
            .expect("mini strip spawns");
        let mini_strip = world
            .query_filtered::<Entity, With<crate::practice::hud::mini_strip::MiniStripRoot>>()
            .single(&world)
            .expect("mini strip");
        world
            .run_system_once(spawn_test_pause_overlay)
            .expect("pause overlay spawns");
        let overlay = world
            .query_filtered::<Entity, With<PauseOverlay>>()
            .single(&world)
            .expect("pause overlay");

        world
            .run_system_once(refresh_pause_legend)
            .expect("quick settings legend refreshes");
        assert_eq!(
            descendant_count::<dtx_ui::widget::nav_legend::NavLegend>(&mut world, mini_strip),
            1
        );
        assert_eq!(
            descendant_count::<dtx_ui::widget::nav_legend::NavLegend>(&mut world, overlay),
            1
        );

        *world.resource_mut::<PauseView>() = PauseView::Menu;
        world
            .run_system_once(refresh_pause_legend)
            .expect("pause menu legend refreshes");
        world
            .run_system_once(refresh_pause_legend)
            .expect("unchanged pause menu refresh is a no-op");
        assert_eq!(
            descendant_count::<dtx_ui::widget::nav_legend::NavLegend>(&mut world, overlay),
            1
        );

        world
            .run_system_once(despawn_overlay)
            .expect("pause overlay despawns on resume");
        assert_eq!(
            descendant_count::<dtx_ui::widget::nav_legend::NavLegend>(&mut world, mini_strip),
            1
        );
        let text = descendant_text(&mut world, mini_strip);
        for expected in ["Esc", "Pause", "Tab", "Settings"] {
            assert_eq!(
                text.iter()
                    .filter(|label| label.as_str() == expected)
                    .count(),
                1,
                "running mini-strip legend must retain exactly one {expected:?}: {text:?}"
            );
        }
    }

    #[test]
    fn practice_this_section_builds_bar_aligned_normal_pause_request() {
        let timeline = crate::timeline::ChipTimeline {
            bar_ms: vec![0, 2_000, 4_000, 6_000, 8_000, 10_000],
            end_ms: 10_000,
            ..Default::default()
        };

        let request = practice_request_at(&timeline, 5_100);

        assert_eq!(request.origin, game_shell::PracticeOrigin::NormalPause);
        let game_shell::PracticeSeed::Recommended(section) = request.seed else {
            panic!("recommended")
        };
        assert_eq!(section.loop_start_ms, 2_000);
        assert_eq!(section.loop_end_ms, 8_000);
        assert!(section.loop_start_ms <= 5_100 && section.loop_end_ms > 5_100);
    }

    fn dispatch_world(selection: usize) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<game_shell::NavAction>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.init_resource::<Messages<crate::practice::actions::PracticeAction>>();
        world.init_resource::<Messages<crate::practice::OpenPracticeSettings>>();
        world.init_resource::<Messages<crate::seek::SeekToChartTime>>();
        world.init_resource::<Messages<crate::events::LaneHit>>();
        world.init_resource::<Messages<crate::events::InputHit>>();
        world.insert_resource(PauseSelection(selection));
        world.init_resource::<QuickSettingsSelection>();
        world.init_resource::<PauseView>();
        world.init_resource::<PausedRestart>();
        world.init_resource::<crate::seek::SeekAcknowledgement>();
        world.init_resource::<crate::input::PendingLaneInputs>();
        world.init_resource::<NextState<PauseState>>();
        world.insert_resource(crate::practice::PracticeSession::default());
        world.init_resource::<crate::timeline::ChipTimeline>();
        world.init_resource::<crate::resources::GameplayClock>();
        world.init_resource::<PracticeIntent>();
        world.init_resource::<game_shell::CompletedRunContext>();
        world.init_resource::<crate::results_analysis::NormalPlayEventStream>();
        world.init_resource::<crate::perf_hotkeys::PerfHotkeyDraft>();
        world.init_resource::<crate::resources::ScrollSettings>();
        world.init_resource::<crate::resources::InputOffsetMs>();
        world.init_resource::<crate::resources::BgmAdjustState>();
        world.init_resource::<crate::resources::ShowTimingLines>();
        world.init_resource::<crate::resources::LaneDisplayState>();
        world.init_resource::<crate::resources::DrumAudioSettings>();
        world.init_resource::<dtx_audio::BgmHandle>();
        world.init_resource::<Assets<AudioInstance>>();
        world.write_message(game_shell::NavAction {
            verb: game_shell::SystemVerb::Confirm,
            source: game_shell::NavSource::Keyboard,
            coarse: false,
        });
        world
    }

    #[test]
    fn practice_confirm_exit_goes_to_song_select() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(3); // Exit Practice row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongSelect]);
    }

    #[test]
    fn practice_confirm_restart_loop_queues_seek_before_resume() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(1); // Restart loop row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
        let seeks: Vec<crate::seek::SeekToChartTime> = world
            .resource::<Messages<crate::seek::SeekToChartTime>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(seeks.len(), 1);
        assert_eq!(seeks[0].attempt_start_ms, None);
        assert!(
            !world
                .resource::<crate::practice::PracticeSession>()
                .current_attempt_eligible
        );
    }

    #[test]
    fn practice_confirm_settings_only_requests_editing() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(2);
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
        assert_eq!(
            world
                .resource::<Messages<crate::practice::OpenPracticeSettings>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }

    fn verb_world(state: PauseState, verb: dtx_input::SystemVerb) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<crate::events::SystemVerbHit>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.insert_resource(State::new(state));
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<VerbGuard>();
        world.write_message(crate::events::SystemVerbHit {
            verb,
            source: dtx_input::VerbSource::Keyboard,
        });
        world
    }

    /// A guard that never debounces — isolates the drain fix from the min-interval one.
    fn open_guard() -> VerbGuard {
        VerbGuard {
            min_gap: Duration::ZERO,
            last: HashMap::new(),
        }
    }

    #[test]
    fn pause_verb_opens_pause() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn pause_verb_while_paused_resumes() {
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_does_not_toggle_pause() {
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_pause)
            .expect("system_verb_pause runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn restart_verb_requests_song_loading_while_running() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn restart_verb_also_fires_while_paused() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Paused, dtx_input::SystemVerb::Restart);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongLoading]);
    }

    #[test]
    fn pause_verb_does_not_restart() {
        use bevy::ecs::message::Messages;
        let mut world = verb_world(PauseState::Running, dtx_input::SystemVerb::Pause);
        world
            .run_system_once(system_verb_restart)
            .expect("system_verb_restart runs");
        assert_eq!(
            world
                .resource::<Messages<TransitionRequest>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    /// An e-kit pad retrigger puts TWO `SystemVerbHit`s in one frame.
    /// `Iterator::any` short-circuits, so the second message stayed unread and
    /// replayed the next frame — pause on, pause off, overlay flashes for one
    /// frame. This app runs a real message lifecycle (`TimePlugin` +
    /// `StatesPlugin`), so a leftover survives into the next update.
    #[test]
    fn a_double_pause_hit_in_one_frame_pauses_once() {
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<PauseState>()
            // Debounce OFF: this test must fail on `any()` alone, so the drain
            // is what it proves — the min-interval guard is tested separately.
            .insert_resource(open_guard())
            .add_message::<crate::events::SystemVerbHit>()
            .add_systems(Update, system_verb_pause);

        for _ in 0..2 {
            app.world_mut().write_message(crate::events::SystemVerbHit {
                verb: dtx_input::SystemVerb::Pause,
                source: dtx_input::VerbSource::Midi,
            });
        }
        // Frame 1 reads the hits and sets NextState; frame 2's StateTransition
        // applies it (transitions run before Update).
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<PauseState>>().get(),
            PauseState::Paused,
            "the first hit pauses"
        );
        // A message lives ~2 frames: an unread leftover replays here and, with
        // `any()`, toggled Paused → Running.
        app.update();
        assert_eq!(
            *app.world().resource::<State<PauseState>>().get(),
            PauseState::Paused,
            "a same-frame retrigger must not toggle pause twice"
        );
    }

    /// A pad retrigger 20-40 ms later lands in a DIFFERENT frame, so draining
    /// the reader can't collapse it — only a min-interval guard can.
    #[test]
    fn verb_guard_debounces_a_pad_retrigger() {
        let mut g = VerbGuard::default();
        let t0 = Instant::now();
        assert!(g.accept(SystemVerb::Pause, t0), "first hit acts");
        assert!(
            !g.accept(SystemVerb::Pause, t0 + Duration::from_millis(20)),
            "a 20 ms retrigger must not toggle pause back off"
        );
        assert!(
            !g.accept(SystemVerb::Pause, t0 + Duration::from_millis(79)),
            "still inside the window"
        );
        // A deliberate second press, well past the window, must act.
        assert!(g.accept(SystemVerb::Pause, t0 + Duration::from_millis(400)));
    }

    /// Keyed per verb: pausing must not swallow a Restart that lands right after.
    #[test]
    fn verb_guard_is_per_verb() {
        let mut g = VerbGuard::default();
        let t0 = Instant::now();
        assert!(g.accept(SystemVerb::Pause, t0));
        assert!(g.accept(SystemVerb::Restart, t0 + Duration::from_millis(10)));
        assert!(!g.accept(SystemVerb::Pause, t0 + Duration::from_millis(20)));
    }

    #[test]
    fn leaving_pause_keeps_wait_halted_audio_paused() {
        let halted = WaitState {
            phase: WaitPhase::Halted(WaitSet {
                target_ms: 1_000,
                chips: vec![7],
            }),
            ..default()
        };

        assert!(!should_resume_chart_audio(Some(&halted), None));
        assert!(should_resume_chart_audio(None, None));
    }

    #[test]
    fn stopped_practice_surface_owns_paused_chart_audio() {
        let mut flow = crate::practice::PracticeFlow::default();
        assert!(!should_resume_chart_audio(None, Some(&flow)));

        flow.phase = crate::practice::PracticePhase::Editing;
        assert!(
            !should_resume_chart_audio(None, Some(&flow)),
            "Editing owns frozen chart audio after the pause overlay exits"
        );

        flow.preview = crate::practice::PreviewState::Playing;
        assert!(should_resume_chart_audio(None, Some(&flow)));

        flow.phase = crate::practice::PracticePhase::Running;
        flow.preview = crate::practice::PreviewState::Stopped;
        assert!(should_resume_chart_audio(None, Some(&flow)));
    }
}
