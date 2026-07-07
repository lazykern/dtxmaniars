//! Practice UI: persistent transport strip. The paused-tier panel moved
//! to `practice/hud/full_hud.rs` (Task 4, practice UX v2).
//!
//! v1 layout contract (spec §UI): every element here is a discrete,
//! self-contained UI entity — no tendrils into hud.rs internals — so the
//! future layout-editor widget registry can absorb them as widgets.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use game_shell::AppState;

use super::session::PracticeSession;
use crate::resources::GameplayClock;
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
    // WidgetKind::PracticeTransport is registry-only in v1: this bar is
    // bottom-anchored, so anchor-aware movement is deferred to the editor plan.
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
