//! Playfield visuals — lane receptor flash + hit bursts (dtxpt-inspired, UI-space).

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_ui::ThemeResource;
use game_shell::AppState;

use crate::events::JudgmentEvent;
use crate::hud::HudRoot;
use crate::layout::PlayfieldLayout;
use crate::scroll::lane_color;

const RECEPTOR_FLASH_SECS: f32 = 0.12;
const HIT_BURST_SECS: f32 = 0.18;

#[derive(Component)]
pub struct LaneReceptor {
    pub lane: u8,
}

#[derive(Component)]
pub struct ReceptorFlash {
    pub timer: Timer,
    pub strength: f32,
}

#[derive(Component)]
pub struct HitBurst {
    pub timer: Timer,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            flash_receptors_on_hit,
            tick_receptor_flashes,
            tick_hit_bursts,
            apply_receptor_layout,
        )
            .run_if(in_state(AppState::Performance)),
    );
}

pub fn spawn_lane_receptors(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    theme: &dtx_ui::theme::Theme,
) {
    for lane in 0..crate::lane_map::LANE_COUNT {
        commands.entity(parent).with_children(|root| {
            root.spawn((
                LaneReceptor { lane: lane as u8 },
                ReceptorFlash {
                    timer: Timer::from_seconds(0.0, TimerMode::Once),
                    strength: 0.0,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.lane_left(lane) + 2.0),
                    top: Val::Px(layout.judge_y() - 12.0 * layout.scale),
                    width: Val::Px(layout.lane_width() - 4.0),
                    height: Val::Px(24.0 * layout.scale),
                    ..default()
                },
                BackgroundColor(theme.panel_bg),
            ));
        });
    }
}

fn flash_receptors_on_hit(
    mut events: MessageReader<JudgmentEvent>,
    layout: Res<PlayfieldLayout>,
    hud_root: Query<Entity, With<HudRoot>>,
    mut commands: Commands,
    mut receptors: Query<(&LaneReceptor, &mut ReceptorFlash)>,
) {
    let Ok(hud) = hud_root.single() else {
        return;
    };
    for ev in events.read() {
        let strength = match ev.kind {
            JudgmentKind::Perfect => 1.0,
            JudgmentKind::Great => 0.9,
            JudgmentKind::Good => 0.75,
            JudgmentKind::Poor => 0.55,
            JudgmentKind::Miss => 0.0,
        };
        if strength <= 0.0 {
            continue;
        }
        for (receptor, mut flash) in &mut receptors {
            if receptor.lane == ev.lane {
                flash.timer = Timer::from_seconds(RECEPTOR_FLASH_SECS, TimerMode::Once);
                flash.strength = strength;
            }
        }
        spawn_hit_burst(&mut commands, hud, &layout, ev.lane, strength);
    }
}

fn spawn_hit_burst(
    commands: &mut Commands,
    hud: Entity,
    layout: &PlayfieldLayout,
    lane: u8,
    strength: f32,
) {
    let lane = lane as usize;
    let burst = commands
        .spawn((
            HitBurst {
                timer: Timer::from_seconds(HIT_BURST_SECS, TimerMode::Once),
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.lane_left(lane) + 4.0),
                top: Val::Px(layout.judge_y() - layout.note_height()),
                width: Val::Px(layout.note_width()),
                height: Val::Px(layout.note_height() * 1.6),
                ..default()
            },
            BackgroundColor(lane_color(lane as u8).with_alpha(0.85 * strength)),
        ))
        .id();
    commands.entity(hud).add_child(burst);
}

fn tick_receptor_flashes(
    time: Res<Time>,
    theme: Res<ThemeResource>,
    mut receptors: Query<(&LaneReceptor, &mut ReceptorFlash, &mut BackgroundColor)>,
) {
    for (receptor, mut flash, mut bg) in &mut receptors {
        if flash.timer.duration().as_secs_f32() == 0.0 {
            continue;
        }
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            flash.strength = 0.0;
            bg.0 = theme.0.panel_bg;
        } else {
            let t = 1.0 - flash.timer.fraction();
            let base = lane_color(receptor.lane);
            bg.0 = base.with_alpha(0.15 + 0.35 * flash.strength * t);
        }
    }
}

fn tick_hit_bursts(
    time: Res<Time>,
    mut commands: Commands,
    mut bursts: Query<(Entity, &mut HitBurst, &mut BackgroundColor)>,
) {
    for (entity, mut burst, mut bg) in &mut bursts {
        burst.timer.tick(time.delta());
        let t = 1.0 - burst.timer.fraction();
        bg.0 = bg.0.with_alpha(bg.0.alpha() * t);
        if burst.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn apply_receptor_layout(
    layout: Res<PlayfieldLayout>,
    mut receptors: Query<(&LaneReceptor, &mut Node)>,
) {
    for (receptor, mut node) in &mut receptors {
        let lane = receptor.lane as usize;
        node.left = Val::Px(layout.lane_left(lane) + 2.0);
        node.top = Val::Px(layout.judge_y() - 12.0 * layout.scale);
        node.width = Val::Px(layout.lane_width() - 4.0);
        node.height = Val::Px(24.0 * layout.scale);
    }
}
