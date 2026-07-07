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
    ExitPractice,
}

impl RailItem {
    pub const ORDER: [RailItem; 10] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
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
        RailItem::Scrub => match session.scrub_cursor_ms {
            Some(ms) => format!("Scrub  ◀ {} ▶   (Enter: play here)", format_chart_time(ms)),
            None => "Scrub  ◀ ▶".into(),
        },
        RailItem::RestartSection => "Restart section".into(),
        RailItem::SetA => "Set A here".into(),
        RailItem::SetB => "Set B here".into(),
        RailItem::ClearLoop => "Clear loop".into(),
        RailItem::Rate => format!("Rate  ◀ x{:.2} ▶", session.rate),
        RailItem::Snap => format!("Snap  ◀ {} ▶", session.snap.label()),
        RailItem::Preroll => format!("Pre-roll  ◀ {} ▶", session.preroll.label()),
        RailItem::ExitPractice => {
            if exit_armed {
                "Exit practice — Enter again to confirm".into()
            } else {
                "Exit practice".into()
            }
        }
    }
}

pub fn attempt_history_text(session: &PracticeSession) -> String {
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in session.attempt_history.iter().enumerate().rev().take(8) {
        lines.push(format!(
            "#{}  {:.1}%  {:+.0}ms  x{:.2}",
            i + 1,
            a.accuracy_pct,
            a.mean_error_ms,
            a.rate
        ));
    }
    lines.join("\n")
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
    session.scrub_cursor_ms = Some(clock.current_ms);
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
                for item in RailItem::ORDER {
                    rail.spawn((
                        item,
                        Text::new(rail_label(item, &session, false)),
                        Theme::hud_font(),
                        TextColor(theme.text_secondary),
                    ));
                }
                rail.spawn((
                    AttemptHistoryText,
                    Text::new(attempt_history_text(&session)),
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
        session.scrub_cursor_ms = None;
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
    mut rows: Query<(&RailItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailItem>)>,
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
                let cur = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                session.scrub_cursor_ms = Some(timeline.snap_neighbor(cur, session.snap, dir));
            }
            RailItem::Rate => session.step_rate(dir),
            RailItem::Snap => session.snap = session.snap.next(),
            RailItem::Preroll => session.preroll = session.preroll.next(),
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match selected {
            RailItem::Resume => next_pause.set(PauseState::Running),
            RailItem::Scrub => {
                let intent = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            RailItem::RestartSection => {
                let intent = session
                    .loop_region
                    .map(|r| r.start_ms)
                    .unwrap_or(session.current_attempt.start_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            RailItem::SetA => {
                let ms =
                    timeline.bar_start_before(session.scrub_cursor_ms.unwrap_or(clock.current_ms));
                session.set_loop_start(ms);
            }
            RailItem::SetB => {
                let cursor = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                let mut ms = timeline.bar_start_before(cursor);
                if let Some(r) = session.loop_region {
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
            RailItem::ClearLoop => session.loop_region = None,
            RailItem::Rate | RailItem::Snap | RailItem::Preroll => {}
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
        t.0 = attempt_history_text(&session);
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
        t.0 = format_chart_time(session.scrub_cursor_ms.unwrap_or(clock.current_ms));
    }
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
        match session.scrub_cursor_ms {
            Some(ms) => {
                node.left = Val::Percent(time_to_pct(ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match session.loop_region.filter(|r| r.end_ms != i64::MAX) {
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
