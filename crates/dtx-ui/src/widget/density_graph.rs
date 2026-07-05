//! GITADORA note-density graph: one vertical bar per display lane,
//! height ∝ note count, staggered re-grow on selection change.

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::theme::Theme;
use crate::tween::ScalarTween;

pub const LANE_COUNT: usize = 9;
pub const BAR_MAX_H: f32 = 200.0;
pub const BAR_STAGGER_MS: f32 = 20.0;
pub const BAR_GROW_MS: f32 = 220.0;

/// Per-lane note counts in display order LC HH LP SD HT BD LT FT CY.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct DensityData {
    pub lanes: [u32; LANE_COUNT],
    pub total: u32,
}

/// Normalized bar heights: tallest lane = 1.0, empty chart = all 0.
pub fn bar_fractions(lanes: &[u32; LANE_COUNT]) -> [f32; LANE_COUNT] {
    let max = *lanes.iter().max().unwrap_or(&0);
    let mut out = [0.0; LANE_COUNT];
    if max == 0 {
        return out;
    }
    for (i, n) in lanes.iter().enumerate() {
        out[i] = *n as f32 / max as f32;
    }
    out
}

#[derive(Component, Debug)]
pub struct DensityBar {
    pub lane: usize,
    pub tween: ScalarTween,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DensityTotalText;

/// Spawn graph panel content: END label, bar rail, START label,
/// TOTAL NOTES footer.
pub fn spawn_density_graph(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent.spawn((
        Text::new("END"),
        Theme::font(10.0),
        TextColor(theme.text_secondary),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(BAR_MAX_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::End,
            column_gap: Val::Px(3.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        })
        .with_children(|rail| {
            let colors = theme.lane_colors();
            for (lane, &color) in colors.iter().enumerate() {
                rail.spawn((
                    DensityBar {
                        lane,
                        tween: ScalarTween::new(0.0, 0.0, BAR_GROW_MS, EaseFunction::OutQuint),
                    },
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
        });
    parent.spawn((
        Text::new("START"),
        Theme::font(10.0),
        TextColor(theme.text_secondary),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            ..default()
        })
        .with_children(|footer| {
            footer.spawn((
                Text::new("TOTAL NOTES"),
                Theme::font(10.0),
                TextColor(theme.text_secondary),
            ));
            footer.spawn((
                DensityTotalText,
                Text::new("0"),
                Theme::font(22.0),
                TextColor(theme.text_primary),
            ));
        });
}

/// On `DensityData` change: restart each bar's tween toward the new
/// fraction, staggered by lane; update total text. Every frame: tick
/// tweens and write heights.
pub fn density_graph_system(
    time: Res<Time>,
    data: Res<DensityData>,
    added: Query<(), Added<DensityTotalText>>,
    mut bars: Query<(&mut DensityBar, &mut Node)>,
    mut totals: Query<&mut Text, With<DensityTotalText>>,
) {
    if data.is_changed() || !added.is_empty() {
        let fractions = bar_fractions(&data.lanes);
        for (mut bar, _) in &mut bars {
            let lane = bar.lane;
            let from = bar.tween.value();
            bar.tween.reset(
                from,
                fractions[lane],
                BAR_GROW_MS + lane as f32 * BAR_STAGGER_MS,
                EaseFunction::OutQuint,
            );
        }
        for mut text in &mut totals {
            *text = Text::new(data.total.to_string());
        }
    }
    let dt_ms = time.delta_secs() * 1000.0;
    for (mut bar, mut node) in &mut bars {
        if bar.tween.finished {
            continue;
        }
        bar.tween.tick(dt_ms);
        node.height = Val::Px(BAR_MAX_H * bar.tween.value());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_scale_to_tallest() {
        let mut lanes = [0u32; LANE_COUNT];
        lanes[3] = 200; // SD
        lanes[5] = 100; // BD
        let f = bar_fractions(&lanes);
        assert_eq!(f[3], 1.0);
        assert!((f[5] - 0.5).abs() < 0.001);
        assert_eq!(f[0], 0.0);
    }

    #[test]
    fn fractions_empty_chart_all_zero() {
        let f = bar_fractions(&[0; LANE_COUNT]);
        assert!(f.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn lane_count_matches_theme_lane_colors() {
        assert_eq!(Theme::default().lane_colors().len(), LANE_COUNT);
    }

    #[test]
    fn bars_grow_after_spawn_without_data_change() {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(DensityData {
            lanes: [10, 0, 0, 0, 0, 0, 0, 0, 0],
            total: 10,
        });
        app.add_systems(bevy::app::Update, density_graph_system);
        app.update(); // consume initial change with no bars present
        let theme = Theme::default();
        {
            let world = app.world_mut();
            let mut commands = world.commands();
            commands.spawn(Node::default()).with_children(|p| {
                spawn_density_graph(p, &theme);
            });
        }
        app.world_mut().flush();
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            app.update();
        }
        let world = app.world_mut();
        let mut q = world.query::<(&DensityBar, &Node)>();
        let lane0 = q.iter(world).find(|(b, _)| b.lane == 0).unwrap();
        match lane0.1.height {
            Val::Px(h) => assert!(h > 1.0, "bar height {h} should have grown"),
            other => panic!("unexpected height {other:?}"),
        }
    }
}
