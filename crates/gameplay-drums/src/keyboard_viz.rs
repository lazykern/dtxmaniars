//! Keyboard visualization — key-cap row below lane labels (dtxpt-inspired).

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use game_shell::AppState;

use crate::events::{JudgmentEvent, LaneHit};
use crate::lane_geometry::{column_color, column_of, COLUMNS, COLUMN_COUNT};
use crate::lane_map::lane_channel;
use crate::layout::PlayfieldLayout;

#[derive(Component)]
pub struct KeyCapRow;

#[derive(Component)]
pub struct KeyCap {
    pub col: u8,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            flash_key_caps_on_hit,
            apply_key_cap_layout.run_if(resource_changed::<PlayfieldLayout>),
        )
            .run_if(in_state(AppState::Performance)),
    );
}

fn key_cap_border_radius(cap_h: f32, scale: f32) -> BorderRadius {
    BorderRadius {
        top_left: Val::Px(cap_h * 0.4),
        top_right: Val::Px(cap_h * 0.4),
        bottom_left: Val::Px(4.0 * scale),
        bottom_right: Val::Px(4.0 * scale),
    }
}

pub fn spawn_key_caps(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    theme: &dtx_ui::theme::Theme,
) {
    let cap_h = layout.key_cap_height();
    for col in 0..COLUMN_COUNT {
        let rim = column_color(col);
        commands.entity(parent).with_children(|row| {
            row.spawn((
                KeyCap { col: col as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col) + 2.0),
                    top: Val::Px(layout.key_viz_top()),
                    width: Val::Px(layout.col_width(col) - 4.0),
                    height: Val::Px(cap_h),
                    border: UiRect::all(Val::Px(2.0 * layout.scale)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: key_cap_border_radius(cap_h, layout.scale),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.11, 0.13)),
                BorderColor::all(rim),
                children![(
                    Text::new(COLUMNS[col].label),
                    Theme::font(13.0 * layout.scale),
                    TextColor(theme.text_primary),
                )],
            ));
        });
    }
}

fn apply_key_cap_layout(layout: Res<PlayfieldLayout>, mut caps: Query<(&KeyCap, &mut Node)>) {
    for (cap, mut node) in &mut caps {
        let col = cap.col as usize;
        node.left = Val::Px(layout.col_left(col) + 2.0);
        node.top = Val::Px(layout.key_viz_top());
        node.width = Val::Px(layout.col_width(col) - 4.0);
        node.height = Val::Px(layout.key_cap_height());
        node.border = UiRect::all(Val::Px(2.0 * layout.scale));
        node.border_radius = key_cap_border_radius(layout.key_cap_height(), layout.scale);
    }
}

fn flash_key_caps_on_hit(
    mut lane_hits: MessageReader<LaneHit>,
    mut events: MessageReader<JudgmentEvent>,
    mut caps: Query<(&KeyCap, &mut BackgroundColor)>,
) {
    let to_col = |lane: u8| lane_channel(lane).and_then(column_of);
    // Immediate feedback on key press (input lane), mapped to its visual column.
    for hit in lane_hits.read() {
        let Some(col) = to_col(hit.lane) else {
            continue;
        };
        for (cap, mut bg) in &mut caps {
            if cap.col as usize == col {
                bg.0 = Color::srgb(0.30, 0.30, 0.34);
            }
        }
    }
    for ev in events.read() {
        if ev.kind == dtx_scoring::JudgmentKind::Miss {
            continue;
        }
        let Some(col) = to_col(ev.lane) else {
            continue;
        };
        for (cap, mut bg) in &mut caps {
            if cap.col as usize == col {
                bg.0 = column_color(col).with_alpha(0.85);
            }
        }
    }
}

pub fn decay_key_cap_flashes(mut caps: Query<(&KeyCap, &mut BackgroundColor)>, time: Res<Time>) {
    let dt = time.delta_secs();
    let rest = Color::srgb(0.11, 0.11, 0.13);
    let target = rest.to_srgba();
    let f = (dt * 6.0).min(1.0);
    for (_cap, mut bg) in &mut caps {
        if bg.0 == rest {
            continue;
        }
        let cur = bg.0.to_srgba();
        let lerp = |a: f32, b: f32| a + (b - a) * f;
        let next = Color::srgba(
            lerp(cur.red, target.red),
            lerp(cur.green, target.green),
            lerp(cur.blue, target.blue),
            lerp(cur.alpha, target.alpha),
        );
        let n = next.to_srgba();
        bg.0 = if (n.red - target.red).abs() < 0.01
            && (n.green - target.green).abs() < 0.01
            && (n.blue - target.blue).abs() < 0.01
            && (n.alpha - target.alpha).abs() < 0.01
        {
            rest
        } else {
            next
        };
    }
}
