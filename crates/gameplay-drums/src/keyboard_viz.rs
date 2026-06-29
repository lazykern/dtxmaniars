//! Keyboard visualization — key-cap row below lane labels (dtxpt-inspired).

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use dtx_ui::ThemeResource;
use game_shell::AppState;

use crate::events::JudgmentEvent;
use crate::lane_map::LaneMap;
use crate::layout::PlayfieldLayout;

#[derive(Component)]
pub struct KeyCapRow;

#[derive(Component)]
pub struct KeyCap {
    pub lane: u8,
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

pub fn spawn_key_caps(
    commands: &mut Commands,
    parent: Entity,
    layout: &PlayfieldLayout,
    lane_map: &LaneMap,
    theme: &dtx_ui::theme::Theme,
) {
    let cap_h = layout.key_cap_height();
    for lane in 0..lane_map.labels.len() {
        let key_label = lane_map
            .keys
            .iter()
            .find_map(|(k, &l)| (l == lane as u8).then(|| key_display(*k)))
            .unwrap_or_else(|| "?".into());

        commands.entity(parent).with_children(|row| {
            row.spawn((
                KeyCap { lane: lane as u8 },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.lane_left(lane) + 2.0),
                    top: Val::Px(layout.key_viz_top()),
                    width: Val::Px(layout.lane_width() - 4.0),
                    height: Val::Px(cap_h),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.07, 0.09, 0.85)),
                children![(
                    Text::new(format!("{}\n{}", lane_map.labels[lane], key_label)),
                    Theme::label_font(),
                    TextColor(theme.text_secondary),
                )],
            ));
        });
    }
}

fn key_display(key: KeyCode) -> String {
    match key {
        KeyCode::Digit1 => "1".into(),
        KeyCode::Digit2 => "2".into(),
        KeyCode::Digit3 => "3".into(),
        KeyCode::Digit4 => "4".into(),
        KeyCode::Digit5 => "5".into(),
        KeyCode::Digit6 => "6".into(),
        KeyCode::Digit7 => "7".into(),
        KeyCode::Digit8 => "8".into(),
        KeyCode::Digit9 => "9".into(),
        other => format!("{other:?}"),
    }
}

fn apply_key_cap_layout(layout: Res<PlayfieldLayout>, mut caps: Query<(&KeyCap, &mut Node)>) {
    for (cap, mut node) in &mut caps {
        let lane = cap.lane as usize;
        node.left = Val::Px(layout.lane_left(lane) + 2.0);
        node.top = Val::Px(layout.key_viz_top());
        node.width = Val::Px(layout.lane_width() - 4.0);
        node.height = Val::Px(layout.key_cap_height());
    }
}

fn flash_key_caps_on_hit(
    mut events: MessageReader<JudgmentEvent>,
    theme: Res<ThemeResource>,
    mut caps: Query<(&KeyCap, &mut BackgroundColor)>,
) {
    let accent = theme.0.accent;
    for ev in events.read() {
        if ev.kind == dtx_scoring::JudgmentKind::Miss {
            continue;
        }
        for (cap, mut bg) in &mut caps {
            if cap.lane == ev.lane {
                bg.0 = accent.with_alpha(0.55);
            }
        }
    }
}

pub fn decay_key_cap_flashes(
    theme: Res<ThemeResource>,
    time: Res<Time>,
    mut caps: Query<&mut BackgroundColor, With<KeyCap>>,
) {
    let base = Color::srgba(0.07, 0.07, 0.09, 0.85);
    let dt = time.delta_secs();
    for mut bg in &mut caps {
        if bg.0 != base {
            let a = (bg.0.alpha() - dt * 4.0).max(base.alpha());
            bg.0 = theme.0.accent.with_alpha(a * 0.55);
            if a <= base.alpha() + 0.01 {
                bg.0 = base;
            }
        }
    }
}
