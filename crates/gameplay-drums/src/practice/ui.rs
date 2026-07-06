//! Practice UI: persistent transport strip + practice pause panel.
//!
//! v1 layout contract (spec §UI): every element here is a discrete,
//! self-contained UI entity — no tendrils into hud.rs internals — so the
//! future layout-editor widget registry can absorb them as widgets.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use game_shell::{request_transition, AppState, PauseState, TransitionRequest};

use super::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

#[derive(Component)]
struct TransportRoot;
#[derive(Component)]
struct TransportTimeText;
#[derive(Component)]
struct TransportRateText;
#[derive(Component)]
struct TransportAttemptText;
#[derive(Component)]
struct PlayheadMarker;
#[derive(Component)]
struct ScrubCursorMarker;
#[derive(Component)]
struct LoopMarkerA;
#[derive(Component)]
struct LoopMarkerB;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_transport
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_transport)
    .add_systems(
        Update,
        (update_transport_texts, update_transport_markers)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_exists::<PracticeSession>),
    );

    app.init_resource::<PracticeSelection>()
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_practice_panel.run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(PauseState::Paused), despawn_practice_panel)
        .add_systems(
            Update,
            practice_panel_input
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<PracticeSession>),
        );
}

fn marker_node(left_pct: f32, width_px: f32, color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(left_pct),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(width_px),
            ..default()
        },
        BackgroundColor(color),
    )
}

fn spawn_transport(
    mut commands: Commands,
    timeline: Res<ChipTimeline>,
    chart: Res<crate::resources::ActiveChart>,
) {
    let theme = Theme::default();
    let bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    commands
        .spawn((
            TransportRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(900),
        ))
        .with_children(|root| {
            root.spawn((
                TransportTimeText,
                Text::new("0:00.0"),
                Theme::label_font(),
                TextColor(theme.text_primary),
            ));
            root.spawn((
                Text::new(format!("{bpm:.0} BPM")),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
            let strip = spawn_density_strip(root, &timeline.density, &theme);
            root.commands().entity(strip).with_children(|markers| {
                markers.spawn((PlayheadMarker, marker_node(0.0, 2.0, theme.accent)));
                markers.spawn((
                    ScrubCursorMarker,
                    marker_node(0.0, 2.0, Color::WHITE),
                    Visibility::Hidden,
                ));
                markers.spawn((
                    LoopMarkerA,
                    marker_node(0.0, 2.0, Color::srgb(0.3, 0.9, 0.5)),
                    Visibility::Hidden,
                ));
                markers.spawn((
                    LoopMarkerB,
                    marker_node(0.0, 2.0, Color::srgb(0.95, 0.5, 0.3)),
                    Visibility::Hidden,
                ));
            });
            root.spawn((
                TransportRateText,
                Text::new("x1.00"),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
            root.spawn((
                TransportAttemptText,
                Text::new(""),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
        });
}

fn despawn_transport(mut commands: Commands, roots: Query<Entity, With<TransportRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn update_transport_texts(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TransportTimeText>>,
        Query<&mut Text, With<TransportRateText>>,
        Query<&mut Text, With<TransportAttemptText>>,
    )>,
) {
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format_chart_time(clock.current_ms);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("x{:.2}", session.rate);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        let n = session.attempt_history.len() + 1;
        let a = &session.current_attempt;
        t.0 = if a.has_data() {
            format!("attempt #{n}  {:.1}%", a.accuracy_pct())
        } else {
            format!("attempt #{n}")
        };
    }
}

#[allow(clippy::type_complexity)]
fn update_transport_markers(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut markers: ParamSet<(
        Query<&mut Node, With<PlayheadMarker>>,
        Query<(&mut Node, &mut Visibility), With<ScrubCursorMarker>>,
        Query<(&mut Node, &mut Visibility), With<LoopMarkerA>>,
        Query<(&mut Node, &mut Visibility), With<LoopMarkerB>>,
    )>,
) {
    let end = timeline.end_ms;
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
    let region = session.loop_region;
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match region {
            Some(r) => {
                node.left = Val::Percent(time_to_pct(r.start_ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = markers.p3().single_mut() {
        match region.filter(|r| r.end_ms != i64::MAX) {
            Some(r) => {
                node.left = Val::Percent(time_to_pct(r.end_ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

/// Root marker for the practice pause panel.
#[derive(Component)]
struct PracticePanel;

/// One selectable practice-panel row.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum PracticeItem {
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

impl PracticeItem {
    const ORDER: [PracticeItem; 10] = [
        PracticeItem::Resume,
        PracticeItem::Scrub,
        PracticeItem::RestartSection,
        PracticeItem::SetA,
        PracticeItem::SetB,
        PracticeItem::ClearLoop,
        PracticeItem::Rate,
        PracticeItem::Snap,
        PracticeItem::Preroll,
        PracticeItem::ExitPractice,
    ];
}

/// Currently highlighted practice-panel row.
#[derive(Resource, Default)]
struct PracticeSelection(usize);

#[derive(Component)]
struct AttemptHistoryText;

fn practice_item_label(item: PracticeItem, session: &PracticeSession) -> String {
    match item {
        PracticeItem::Resume => "Resume".into(),
        PracticeItem::Scrub => match session.scrub_cursor_ms {
            Some(ms) => format!("Scrub  ◀ {} ▶   (Enter: play here)", format_chart_time(ms)),
            None => "Scrub  ◀ ▶".into(),
        },
        PracticeItem::RestartSection => "Restart section".into(),
        PracticeItem::SetA => "Set A here".into(),
        PracticeItem::SetB => "Set B here".into(),
        PracticeItem::ClearLoop => "Clear loop".into(),
        PracticeItem::Rate => format!("Rate  ◀ x{:.2} ▶", session.rate),
        PracticeItem::Snap => format!("Snap  ◀ {} ▶", session.snap.label()),
        PracticeItem::Preroll => format!("Pre-roll  ◀ {} ▶", session.preroll.label()),
        PracticeItem::ExitPractice => "Exit practice".into(),
    }
}

fn attempt_history_text(session: &PracticeSession) -> String {
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in session.attempt_history.iter().enumerate().rev().take(5) {
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

fn spawn_practice_panel(
    mut commands: Commands,
    mut selection: ResMut<PracticeSelection>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
) {
    selection.0 = 0;
    session.scrub_cursor_ms = Some(clock.current_ms);
    let theme = Theme::default();
    commands
        .spawn((
            PracticePanel,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("PRACTICE"),
                Theme::title_font(),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(18.0)),
                    ..default()
                },
            ));
            for item in PracticeItem::ORDER {
                root.spawn((
                    item,
                    Text::new(practice_item_label(item, &session)),
                    Theme::hud_font(),
                    TextColor(theme.text_secondary),
                ));
            }
            root.spawn((
                AttemptHistoryText,
                Text::new(attempt_history_text(&session)),
                Theme::label_font(),
                TextColor(theme.text_secondary),
                Node {
                    margin: UiRect::top(Val::Px(18.0)),
                    ..default()
                },
            ));
        });
}

fn despawn_practice_panel(
    mut commands: Commands,
    panels: Query<Entity, With<PracticePanel>>,
    mut session: Option<ResMut<PracticeSession>>,
) {
    for e in &panels {
        commands.entity(e).despawn();
    }
    if let Some(session) = session.as_mut() {
        session.scrub_cursor_ms = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn practice_panel_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<PracticeSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut requests: MessageWriter<TransitionRequest>,
    mut rows: Query<(&PracticeItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<PracticeItem>)>,
) {
    let count = PracticeItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
    }
    let selected = PracticeItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        match selected {
            PracticeItem::Scrub => {
                let cur = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                session.scrub_cursor_ms = Some(timeline.snap_neighbor(cur, session.snap, dir));
            }
            PracticeItem::Rate => session.step_rate(dir),
            PracticeItem::Snap => session.snap = session.snap.next(),
            PracticeItem::Preroll => session.preroll = session.preroll.next(),
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match selected {
            PracticeItem::Resume => next_pause.set(PauseState::Running),
            PracticeItem::Scrub => {
                let intent = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                seeks.write(SeekToChartTime {
                    target_ms: preroll_target(&timeline, session.preroll, intent),
                    snap: None,
                    attempt_start_ms: Some(intent),
                });
                next_pause.set(PauseState::Running);
            }
            PracticeItem::RestartSection => {
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
            PracticeItem::SetA => {
                let ms =
                    timeline.bar_start_before(session.scrub_cursor_ms.unwrap_or(clock.current_ms));
                session.set_loop_start(ms);
            }
            PracticeItem::SetB => {
                let cursor = session.scrub_cursor_ms.unwrap_or(clock.current_ms);
                let mut ms = timeline.bar_start_before(cursor);
                // Min region: one bar. If B lands on/before A, push B one
                // bar past A.
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
            PracticeItem::ClearLoop => session.loop_region = None,
            PracticeItem::Rate | PracticeItem::Snap | PracticeItem::Preroll => {}
            PracticeItem::ExitPractice => {
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongSelect);
            }
        }
    }

    // Repaint rows every frame (labels are cheap, list is 10 rows).
    let theme = Theme::default();
    for (item, mut text, mut color) in &mut rows {
        text.0 = practice_item_label(*item, &session);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_time_formats_minutes_seconds_tenths() {
        assert_eq!(format_chart_time(0), "0:00.0");
        assert_eq!(format_chart_time(83_450), "1:23.4");
        assert_eq!(format_chart_time(-50), "0:00.0");
    }
}
