//! Full practice HUD (paused tier), layout B "L-shape": bottom density
//! timeline (mouse scrub + drag loop; keyboard scrub kept) + right rail
//! (rate, snap, pre-roll, ramp config, attempt history, restart, exit).
//! Fixed overlay — not a dtx-layout widget.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use super::format_chart_time;
use crate::practice::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// Root marker for the full practice HUD.
#[derive(Component)]
pub struct FullHudRoot;

/// The bottom timeline strip (mouse hit-target; markers are children).
#[derive(Component)]
pub struct FullHudTimelineStrip;

#[derive(Component)]
pub struct HudPlayhead;
#[derive(Component)]
pub struct HudScrubCursor;
#[derive(Component)]
pub struct HudLoopFill;
#[derive(Component)]
pub struct HudTimeText;
#[derive(Component)]
pub struct AttemptHistoryText;
#[derive(Component)]
pub struct LaneDiagnosisText;

/// One selectable right-rail row.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum RailItem {
    Resume,
    Scrub,
    RestartSection,
    SetA,
    SetB,
    ClearLoop,
    Rate,
    Snap,
    Preroll,
    Metronome,
    RampArm,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
    RampStreak,
    WaitMode,
    ExitPractice,
}

impl RailItem {
    pub const ORDER: [RailItem; 18] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::Metronome,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::RampArm,
        RailItem::RampStart,
        RailItem::RampTarget,
        RailItem::RampStep,
        RailItem::RampThreshold,
        RailItem::RampStreak,
        RailItem::WaitMode,
        RailItem::ExitPractice,
    ];
}

/// Currently highlighted rail row.
#[derive(Resource, Default)]
pub struct RailSelection(pub usize);

/// Exit needs a second Enter press (confirm); reset on selection move.
#[derive(Resource, Default)]
pub struct ExitArmed(pub bool);

pub fn rail_label(item: RailItem, session: &PracticeSession, exit_armed: bool) -> String {
    match item {
        RailItem::Resume => "Resume".into(),
        RailItem::Scrub => match session.transport.scrub_cursor_ms {
            Some(ms) => format!("Scrub  ◀ {} ▶   (Enter: play here)", format_chart_time(ms)),
            None => "Scrub  ◀ ▶".into(),
        },
        RailItem::RestartSection => "Restart section".into(),
        RailItem::SetA => "Set A here".into(),
        RailItem::SetB => "Set B here".into(),
        RailItem::ClearLoop => "Clear loop".into(),
        RailItem::Rate => {
            if session.trainer.ramp.armed {
                format!(
                    "Tempo  ◀ x{:.2} ▶   (ramp x{:.2})",
                    session.transport.user_tempo, session.trainer.ramp.step_tempo
                )
            } else {
                format!("Tempo  ◀ x{:.2} ▶", session.transport.user_tempo)
            }
        }
        RailItem::Snap => format!("Snap  ◀ {} ▶", session.transport.snap.label()),
        RailItem::Preroll => format!("Pre-roll  ◀ {} ▶", session.transport.preroll.label()),
        RailItem::Metronome => format!(
            "Count-in  {}",
            if session.transport.metronome {
                "on"
            } else {
                "off"
            }
        ),
        RailItem::RampArm => {
            if session.trainer.ramp.armed {
                let (cur, total) = crate::practice::ramp::ramp_step_index(
                    &session.trainer.ramp_config,
                    session.transport.user_tempo,
                );
                format!("Ramp  ON  ({cur}/{total})")
            } else {
                "Ramp  off  (Enter: arm)".into()
            }
        }
        RailItem::RampStart => format!(
            "Ramp start  ◀ x{:.2} ▶",
            session.trainer.ramp_config.start_tempo
        ),
        RailItem::RampTarget => format!(
            "Ramp target  ◀ x{:.2} ▶",
            session.trainer.ramp_config.target_tempo
        ),
        RailItem::RampStep => format!("Ramp step  ◀ +{:.2} ▶", session.trainer.ramp_config.step),
        RailItem::RampThreshold => {
            format!(
                "Ramp pass  ◀ ≥{:.0}% ▶",
                session.trainer.ramp_config.threshold_pct
            )
        }
        RailItem::RampStreak => format!(
            "Ramp streak  ◀ ×{} ▶",
            session.trainer.ramp_config.required_successes
        ),
        RailItem::WaitMode => {
            if session.trainer.wait_enabled {
                "Wait  ON".into()
            } else {
                "Wait  off  (Enter: on)".into()
            }
        }
        RailItem::ExitPractice => {
            if exit_armed {
                "Exit practice — Enter again to confirm".into()
            } else {
                "Exit practice".into()
            }
        }
    }
}

/// Attempts for the current span only (armed A/B region, or the
/// implicit whole-song span when none). `end_ms` = chart end (reserved
/// for future span-end display; span identity is start-keyed).
pub fn attempt_history_text(session: &PracticeSession, end_ms: i64) -> String {
    let span_start = session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .map(|r| r.start_ms)
        .unwrap_or(0);
    let _ = end_ms;
    let span_attempts: Vec<_> = session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == span_start)
        .collect();
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in span_attempts.iter().enumerate().rev().take(8) {
        lines.push(format!(
            "#{}  {:.1}%  {:+.0}ms  x{:.2}",
            i + 1,
            a.accuracy_pct,
            a.mean_error_ms,
            a.tempo
        ));
    }
    lines.join("\n")
}

/// Clickable transport-row button.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum TransportButton {
    PrevBar,
    Resume,
    NextBar,
}

impl TransportButton {
    fn label(self) -> &'static str {
        match self {
            TransportButton::PrevBar => "|◀ bar",
            TransportButton::Resume => "▶ resume",
            TransportButton::NextBar => "bar ▶|",
        }
    }
}

pub fn transport_buttons(
    interactions: Query<(&Interaction, &TransportButton), Changed<Interaction>>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            TransportButton::PrevBar | TransportButton::NextBar => {
                let dir: i8 = if *button == TransportButton::NextBar {
                    1
                } else {
                    -1
                };
                let cur = session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(clock.current_ms);
                session.transport.scrub_cursor_ms =
                    Some(timeline.snap_neighbor(cur, crate::timeline::SnapDivisor::Bar, dir));
            }
            TransportButton::Resume => next_pause.set(PauseState::Running),
        }
    }
}

pub fn spawn_full_hud(
    mut commands: Commands,
    mut selection: ResMut<RailSelection>,
    mut exit_armed: ResMut<ExitArmed>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
    timeline: Res<ChipTimeline>,
) {
    selection.0 = 0;
    exit_armed.0 = false;
    session.transport.scrub_cursor_ms = Some(clock.current_ms);
    let theme = Theme::default();
    commands
        .spawn((
            FullHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            // Right rail.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(72.0),
                    width: Val::Px(340.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|rail| {
                rail.spawn((
                    Text::new("PRACTICE"),
                    Theme::title_font(),
                    TextColor(theme.text_primary),
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                ));
                for (idx, item) in RailItem::ORDER.iter().enumerate() {
                    let header = match idx {
                        0 => Some("TRANSPORT"),
                        7 => Some("LOOP"),
                        10 => Some("TRAINER"),
                        _ => None,
                    };
                    if let Some(h) = header {
                        rail.spawn((
                            Text::new(h),
                            Theme::label_font(),
                            TextColor(theme.text_secondary.with_alpha(0.6)),
                            Node {
                                margin: UiRect::top(Val::Px(8.0)),
                                ..default()
                            },
                        ));
                    }
                    rail.spawn((
                        *item,
                        Text::new(rail_label(*item, &session, false)),
                        Theme::hud_font(),
                        TextColor(theme.text_secondary),
                    ));
                }
                rail.spawn((
                    AttemptHistoryText,
                    Text::new(attempt_history_text(&session, timeline.end_ms)),
                    Theme::label_font(),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
                rail.spawn((
                    LaneDiagnosisText,
                    Text::new(crate::practice::diagnosis::diagnosis_text(
                        &session.lane_diag,
                    )),
                    Theme::label_font(),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
            });

            // Bottom timeline row: time text + density strip.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    height: Val::Px(72.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    padding: UiRect::horizontal(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|row| {
                for button in [
                    TransportButton::PrevBar,
                    TransportButton::Resume,
                    TransportButton::NextBar,
                ] {
                    row.spawn((
                        button,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(button.label()),
                            Theme::label_font(),
                            TextColor(theme.text_primary),
                        ));
                    });
                }
                row.spawn((
                    HudTimeText,
                    Text::new(format_chart_time(clock.current_ms)),
                    Theme::hud_font(),
                    TextColor(theme.text_primary),
                ));
                let strip = spawn_density_strip(row, &timeline.density, &theme);
                row.commands().entity(strip).insert(FullHudTimelineStrip);
                row.commands().entity(strip).with_children(|markers| {
                    // Bar ticks along the top edge.
                    for &bar in &timeline.bar_ms {
                        markers.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(time_to_pct(bar, timeline.end_ms)),
                                top: Val::Px(0.0),
                                width: Val::Px(1.0),
                                height: Val::Px(8.0),
                                ..default()
                            },
                            BackgroundColor(theme.text_secondary.with_alpha(0.6)),
                        ));
                    }
                    markers.spawn((
                        HudLoopFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Percent(0.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.25)),
                        Visibility::Hidden,
                    ));
                    markers.spawn((
                        HudPlayhead,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(theme.accent),
                    ));
                    markers.spawn((
                        HudScrubCursor,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            width: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        Visibility::Hidden,
                    ));
                });
            });
        });
}

pub fn despawn_full_hud(
    mut commands: Commands,
    roots: Query<Entity, With<FullHudRoot>>,
    mut session: Option<ResMut<PracticeSession>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if let Some(session) = session.as_mut() {
        session.transport.scrub_cursor_ms = None;
    }
}

/// Keyboard nav for the rail (port of the v1 pause-panel input; the
/// v1 semantics for each row are unchanged).
#[allow(clippy::too_many_arguments)]
pub fn full_hud_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RailSelection>,
    mut exit_armed: ResMut<ExitArmed>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
    mut rows: Query<(&RailItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailItem>)>,
    mut diag_text: Query<
        &mut Text,
        (
            With<LaneDiagnosisText>,
            Without<RailItem>,
            Without<AttemptHistoryText>,
        ),
    >,
) {
    let count = RailItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
        exit_armed.0 = false;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
        exit_armed.0 = false;
    }
    let selected = RailItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        match selected {
            RailItem::Scrub => {
                let cur = session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(clock.current_ms);
                session.transport.scrub_cursor_ms =
                    Some(timeline.snap_neighbor(cur, session.transport.snap, dir));
            }
            RailItem::Rate => session.step_user_tempo(dir),
            RailItem::Snap => session.transport.snap = session.transport.snap.next(),
            RailItem::Preroll => session.transport.preroll = session.transport.preroll.next(),
            RailItem::RampStart => {
                let c = &mut session.trainer.ramp_config;
                c.start_tempo =
                    (c.start_tempo + dir as f32 * 0.05).clamp(0.5, c.target_tempo - 0.05);
                let cfg = session.trainer.ramp_config;
                crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
            }
            RailItem::RampTarget => {
                let c = &mut session.trainer.ramp_config;
                c.target_tempo =
                    (c.target_tempo + dir as f32 * 0.05).clamp(c.start_tempo + 0.05, 1.5);
                let cfg = session.trainer.ramp_config;
                crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
            }
            RailItem::RampStep => {
                let c = &mut session.trainer.ramp_config;
                c.step = (c.step + dir as f32 * 0.05).clamp(0.05, 0.25);
            }
            RailItem::RampThreshold => {
                let c = &mut session.trainer.ramp_config;
                c.threshold_pct = (c.threshold_pct + dir as f32 * 5.0).clamp(50.0, 100.0);
            }
            RailItem::RampStreak => {
                let c = &mut session.trainer.ramp_config;
                c.required_successes = (c.required_successes as i8 + dir).clamp(1, 3) as u8;
            }
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match selected {
            RailItem::Resume => next_pause.set(PauseState::Running),
            RailItem::Scrub => {
                let intent = session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(clock.current_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.transport.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            RailItem::RestartSection => {
                let intent = session
                    .transport
                    .loop_region
                    .map(|r| r.start_ms)
                    .unwrap_or(session.current_attempt.start_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.transport.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            RailItem::SetA => {
                let ms = timeline.bar_start_before(
                    session
                        .transport
                        .scrub_cursor_ms
                        .unwrap_or(clock.current_ms),
                );
                session.set_loop_start(ms);
            }
            RailItem::SetB => {
                let cursor = session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(clock.current_ms);
                let mut ms = timeline.bar_start_before(cursor);
                if let Some(r) = session.transport.loop_region {
                    if ms <= r.start_ms {
                        ms = timeline.snap_neighbor(
                            r.start_ms,
                            crate::timeline::SnapDivisor::Bar,
                            1,
                        );
                    }
                }
                session.set_loop_end(ms);
            }
            RailItem::ClearLoop => session.clear_loop(),
            RailItem::Metronome => {
                session.transport.metronome = !session.transport.metronome;
            }
            RailItem::Rate | RailItem::Snap | RailItem::Preroll => {}
            RailItem::RampArm => {
                practice_actions.write(crate::practice::actions::PracticeAction::ToggleRamp);
            }
            RailItem::WaitMode => {
                session.trainer.wait_enabled = !session.trainer.wait_enabled;
                if session.trainer.wait_enabled && session.trainer.ramp.armed {
                    session.trainer.ramp.armed = false;
                }
            }
            RailItem::RampStart
            | RailItem::RampTarget
            | RailItem::RampStep
            | RailItem::RampThreshold
            | RailItem::RampStreak => {}
            RailItem::ExitPractice => {
                if exit_armed.0 {
                    next_pause.set(PauseState::Running);
                    request_transition(&mut requests, AppState::SongSelect);
                } else {
                    exit_armed.0 = true;
                }
            }
        }
    }

    let theme = Theme::default();
    for (item, mut text, mut color) in &mut rows {
        text.0 = rail_label(*item, &session, exit_armed.0);
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }
    if let Ok(mut t) = history.single_mut() {
        t.0 = attempt_history_text(&session, timeline.end_ms);
    }
    if let Ok(mut t) = diag_text.single_mut() {
        t.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
}

/// Reposition playhead / scrub cursor / loop fill each frame while open.
#[allow(clippy::type_complexity)]
pub fn update_full_hud_markers(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut time_text: Query<&mut Text, With<HudTimeText>>,
    mut markers: ParamSet<(
        Query<&mut Node, With<HudPlayhead>>,
        Query<(&mut Node, &mut Visibility), With<HudScrubCursor>>,
        Query<(&mut Node, &mut Visibility), With<HudLoopFill>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut t) = time_text.single_mut() {
        t.0 = format_chart_time(
            session
                .transport
                .scrub_cursor_ms
                .unwrap_or(clock.current_ms),
        );
    }
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
        match session.transport.scrub_cursor_ms {
            Some(ms) => {
                node.left = Val::Percent(time_to_pct(ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match session
            .transport
            .loop_region
            .filter(|r| r.end_ms != i64::MAX)
        {
            Some(r) => {
                let a = time_to_pct(r.start_ms, end);
                let b = time_to_pct(r.end_ms, end);
                node.left = Val::Percent(a);
                node.width = Val::Percent((b - a).max(0.0));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{AttemptRecord, LoopRegion};

    fn record(start_ms: i64, acc: f32) -> AttemptRecord {
        AttemptRecord {
            start_ms,
            end_ms: start_ms + 4_000,
            tempo: 1.0,
            counts: Default::default(),
            overhits: 0,
            max_combo: 0,
            accuracy_pct: acc,
            mean_error_ms: 0.0,
            waited: 0,
            flow_pct: 0.0,
        }
    }

    #[test]
    fn wait_rail_label_reflects_toggle() {
        let mut s = PracticeSession::default();
        assert_eq!(
            rail_label(RailItem::WaitMode, &s, false),
            "Wait  off  (Enter: on)"
        );
        s.trainer.wait_enabled = true;
        assert_eq!(rail_label(RailItem::WaitMode, &s, false), "Wait  ON");
    }

    #[test]
    fn metronome_rail_label_reflects_toggle() {
        let mut s = PracticeSession::default();
        assert_eq!(rail_label(RailItem::Metronome, &s, false), "Count-in  on");
        s.transport.metronome = false;
        assert_eq!(rail_label(RailItem::Metronome, &s, false), "Count-in  off");
    }

    #[test]
    fn attempt_history_filters_to_current_span() {
        let mut s = PracticeSession::default();
        s.transport.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        s.attempt_history.push(record(0, 50.0)); // old free-play span
        s.attempt_history.push(record(2_000, 91.0)); // this loop
        s.attempt_history.push(record(8_000, 60.0)); // scrub junk
        s.attempt_history.push(record(2_000, 95.0)); // this loop
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("91.0%") && text.contains("95.0%"));
        assert!(!text.contains("50.0%") && !text.contains("60.0%"));
    }

    #[test]
    fn attempt_history_no_loop_uses_implicit_whole_song_span() {
        let mut s = PracticeSession::default();
        s.attempt_history.push(record(0, 88.0)); // implicit span
        s.attempt_history.push(record(4_000, 70.0)); // partial
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("88.0%"));
        assert!(!text.contains("70.0%"));
    }
}
