//! Quick-tier mini loop-strip: thin full-width bar at the bottom edge
//! showing playhead + armed A/B region over the full song extent.
//! Density is deliberately omitted at this size (spec §Quick tier).

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::widget::density_strip::time_to_pct;
use game_shell::AppState;

use crate::practice::session::PracticeSession;
use crate::resources::GameplayClock;
use crate::timeline::ChipTimeline;

#[derive(Component)]
pub struct MiniStripRoot;
#[derive(Component)]
pub struct MiniPlayhead;
#[derive(Component)]
pub struct MiniLoopFill;

pub fn spawn_mini_strip(mut commands: Commands) {
    let theme = Theme::default();
    commands
        .spawn((
            MiniStripRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(crate::ui_z::PRACTICE),
        ))
        .with_children(|strip| {
            strip.spawn((
                MiniLoopFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.35)),
                Visibility::Hidden,
            ));
            strip.spawn((
                MiniPlayhead,
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
        });
}

pub fn despawn_mini_strip(mut commands: Commands, roots: Query<Entity, With<MiniStripRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
pub fn update_mini_strip(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut markers: ParamSet<(
        Query<&mut Node, With<MiniPlayhead>>,
        Query<(&mut Node, &mut Visibility), With<MiniLoopFill>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
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

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_mini_strip
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_mini_strip)
    .add_systems(
        Update,
        update_mini_strip
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}
