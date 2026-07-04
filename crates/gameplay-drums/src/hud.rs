//! Drums performance HUD — osu-style widgets over BocuD mechanics (ADR-0014).

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_ui::{
    theme::Theme,
    widget::{
        combo_display::ComboDisplay,
        gauge_bar::{sync_gauge_bar, GaugeBarWidget, GaugeFill},
        judgment_popup::{spawn_judgment_popup, JudgmentPopup},
        rolling_counter::RollingCounter,
    },
    ThemeResource,
};
use game_shell::{AppState, EGameMode};

use crate::components::LastJudgment;
use crate::gauge::{gauge_fill_color, StageGauge};
use crate::hud_cache::{set_text_if_changed, HudDisplayCache};
use crate::keyboard_viz;
use crate::lane_map::{LaneMap, LANE_ORDER};

pub use crate::lane_map::LANE_COUNT;
use crate::layout::PlayfieldLayout;
use crate::playfield_viz;
use crate::resources::{BgmAdjustState, Combo, InputOffsetMs, JudgmentCounts, Score, ScrollSettings, ShowPerfInfo};

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
struct JudgmentCountsText;

#[derive(Component)]
struct PerfInfoText;

#[derive(Component)]
struct ScoreLabel;

#[derive(Component)]
struct LaneColumn {
    lane: usize,
}

#[derive(Component)]
struct PlayfieldBackboard;

#[derive(Component)]
struct HitLine;

#[derive(Component)]
struct LaneLabel {
    lane: usize,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), spawn_hud)
        .add_systems(OnExit(AppState::Performance), despawn_hud)
        .add_systems(
            Update,
            (
                (
                    apply_backboard_layout,
                    apply_lane_column_layout,
                    apply_hit_line_layout,
                    apply_lane_label_layout,
                )
                    .chain()
                    .run_if(resource_changed::<PlayfieldLayout>),
                sync_hud_score_target,
                tick_hud_score,
                sync_hud_combo,
                sync_hud_gauge,
                sync_hud_judgment,
                refresh_judgment_counts,
                refresh_perf_info,
                keyboard_viz::decay_key_cap_flashes,
            )
                .run_if(in_state(AppState::Performance)),
        );
}

fn spawn_hud(
    mut commands: Commands,
    mode: Res<EGameMode>,
    theme: Res<ThemeResource>,
    layout: Res<PlayfieldLayout>,
    lane_map: Res<LaneMap>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        root.spawn((
            PlayfieldBackboard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.backboard_left()),
                top: Val::Px(layout.backboard_top()),
                width: Val::Px(layout.backboard_width()),
                height: Val::Px(layout.backboard_height()),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.92)),
        ));

        for lane in 0..LANE_COUNT {
            root.spawn((
                LaneColumn { lane },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.lane_left(lane)),
                    top: Val::Px(layout.lane_top()),
                    width: Val::Px(layout.lane_width() - 4.0),
                    height: Val::Px(layout.lane_height()),
                    ..default()
                },
                BackgroundColor(t.panel_bg),
            ));
        }

        root.spawn((
            HitLine,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.lane_strip_left()),
                top: Val::Px(layout.judge_y()),
                width: Val::Px(layout.lane_strip_width()),
                height: Val::Px(3.0 * layout.scale),
                ..default()
            },
            BackgroundColor(t.accent),
        ));

        for (lane, ch) in LANE_ORDER.iter().enumerate() {
            root.spawn((
                LaneLabel { lane },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.lane_left(lane) + 4.0),
                    top: Val::Px(layout.label_top()),
                    ..default()
                },
                Text::new(lane_label(*ch)),
                Theme::label_font(),
                TextColor(t.text_secondary),
            ));
        }

        root.spawn((
            RollingCounter::default(),
            ScoreLabel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(24.0),
                ..default()
            },
            Text::new("0"),
            Theme::hud_font(),
            TextColor(t.text_primary),
        ));

        root.spawn((
            ComboDisplay::default(),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(24.0),
                ..default()
            },
            Text::new("0x"),
            Theme::hud_font(),
            TextColor(t.accent),
        ));

        root.spawn((
            GaugeBarWidget::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                width: Val::Px(280.0),
                height: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(t.gauge_track),
            children![(
                GaugeFill,
                Node {
                    width: Val::Px(224.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(t.gauge_fill),
            )],
        ));

        root.spawn((
            JudgmentCountsText,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                bottom: Val::Px(56.0),
                ..default()
            },
            Text::new(""),
            Theme::label_font(),
            TextColor(t.text_secondary),
        ));

        root.spawn((
            PerfInfoText,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(56.0),
                ..default()
            },
            Text::new(""),
            Theme::label_font(),
            TextColor(t.text_secondary),
            Visibility::Hidden,
        ));
    });

    playfield_viz::spawn_lane_receptors(&mut commands, root, &layout, &t);
    keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &lane_map, &t);
    spawn_judgment_popup(&mut commands, root, &t);
}

fn apply_backboard_layout(
    layout: Res<PlayfieldLayout>,
    mut backboards: Query<&mut Node, With<PlayfieldBackboard>>,
) {
    for mut node in &mut backboards {
        node.left = Val::Px(layout.backboard_left());
        node.top = Val::Px(layout.backboard_top());
        node.width = Val::Px(layout.backboard_width());
        node.height = Val::Px(layout.backboard_height());
    }
}

fn apply_lane_column_layout(
    layout: Res<PlayfieldLayout>,
    mut lanes: Query<(&LaneColumn, &mut Node)>,
) {
    for (col, mut node) in &mut lanes {
        node.left = Val::Px(layout.lane_left(col.lane));
        node.top = Val::Px(layout.lane_top());
        node.width = Val::Px(layout.lane_width() - 4.0);
        node.height = Val::Px(layout.lane_height());
    }
}

fn apply_hit_line_layout(
    layout: Res<PlayfieldLayout>,
    mut hit_line: Query<&mut Node, With<HitLine>>,
) {
    for mut node in &mut hit_line {
        node.left = Val::Px(layout.lane_strip_left());
        node.top = Val::Px(layout.judge_y());
        node.width = Val::Px(layout.lane_strip_width());
        node.height = Val::Px(3.0 * layout.scale);
    }
}

fn apply_lane_label_layout(
    layout: Res<PlayfieldLayout>,
    mut labels: Query<(&LaneLabel, &mut Node)>,
) {
    for (label, mut node) in &mut labels {
        node.left = Val::Px(layout.lane_left(label.lane) + 4.0);
        node.top = Val::Px(layout.label_top());
    }
}

fn lane_label(channel: dtx_core::EChannel) -> &'static str {
    use dtx_core::EChannel;
    match channel {
        EChannel::HiHatClose => "HH",
        EChannel::Snare => "SD",
        EChannel::BassDrum => "BD",
        EChannel::HighTom => "HT",
        EChannel::LowTom => "LT",
        EChannel::FloorTom => "FT",
        EChannel::Cymbal => "CY",
        EChannel::HiHatOpen => "HHO",
        EChannel::RideCymbal => "RD",
        EChannel::LeftCymbal => "LC",
        EChannel::LeftPedal => "LP",
        EChannel::LeftBassDrum => "LBD",
        _ => "?",
    }
}

fn despawn_hud(mut commands: Commands, query: Query<Entity, With<HudRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn sync_hud_score_target(score: Res<Score>, mut q: Query<&mut RollingCounter>) {
    if !score.is_changed() {
        return;
    }
    for mut counter in &mut q {
        counter.set_target(score.0);
    }
}

fn tick_hud_score(
    time: Res<Time>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<(&mut RollingCounter, &mut Text), With<ScoreLabel>>,
) {
    let delta = time.delta_secs() * 1000.0;
    for (mut counter, mut text) in &mut q {
        counter.tick(delta);
        set_text_if_changed(
            &mut text,
            &mut cache.score_text,
            format!("Score: {}", counter.displayed),
        );
    }
}

fn sync_hud_combo(
    combo: Res<Combo>,
    time: Res<Time>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<(&mut ComboDisplay, &mut Text)>,
) {
    let delta = time.delta_secs() * 1000.0;
    for (mut display, mut text) in &mut q {
        display.set_combo(combo.current);
        display.tick(delta);
        set_text_if_changed(
            &mut text,
            &mut cache.combo_text,
            format!("Combo: {}x / Max {}", display.last_combo, combo.max),
        );
    }
}

fn sync_hud_gauge(
    gauge: Res<StageGauge>,
    time: Res<Time>,
    bars: Query<&mut GaugeBarWidget>,
    fills: Query<&mut Node, With<GaugeFill>>,
    mut fill_colors: Query<&mut BackgroundColor, With<GaugeFill>>,
) {
    if gauge.is_changed() {
        sync_gauge_bar(gauge.pct(), time, bars, fills);
        for mut color in &mut fill_colors {
            color.0 = gauge_fill_color(gauge.value, gauge.failed);
        }
    }
}

fn sync_hud_judgment(
    last: Res<LastJudgment>,
    theme: Res<ThemeResource>,
    time: Res<Time>,
    mut prev: Local<Option<(JudgmentKind, i64)>>,
    mut q: Query<(
        &mut JudgmentPopup,
        &mut Text,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    if let Some(ev) = last.0 {
        let key = (ev.kind, ev.delta_ms);
        if prev.as_ref() != Some(&key) {
            *prev = Some(key);
            let label = kind_label(ev.kind);
            for (mut popup, mut text, mut color, mut vis) in &mut q {
                let c = popup.trigger(label, &theme.0);
                *text = Text::new(if ev.delta_ms != 0 {
                    format!("{label} {:+}ms", ev.delta_ms)
                } else {
                    label.into()
                });
                color.0 = c;
                *vis = Visibility::Visible;
            }
        }
    }
    let delta = time.delta_secs() * 1000.0;
    for (mut popup, _, mut color, mut vis) in &mut q {
        let (alpha, _scale) = popup.tick(delta);
        color.0 = color.0.with_alpha(alpha);
        if !popup.is_active() && alpha <= 0.01 {
            *vis = Visibility::Hidden;
        }
    }
}

fn kind_label(kind: JudgmentKind) -> &'static str {
    match kind {
        JudgmentKind::Perfect => "PERFECT",
        JudgmentKind::Great => "GREAT",
        JudgmentKind::Good => "GOOD",
        JudgmentKind::Poor => "POOR",
        JudgmentKind::Miss => "MISS",
    }
}

fn refresh_judgment_counts(
    counts: Res<JudgmentCounts>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<JudgmentCountsText>>,
) {
    if !counts.is_changed() {
        return;
    }
    let acc = accuracy_pct(&counts);
    let text = format!(
        "Acc: {acc:.1}%  P:{} G:{} GO:{} OK:{} M:{}",
        counts.perfect, counts.great, counts.good, counts.ok, counts.miss
    );
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.counters_text, text.clone());
    }
}

fn accuracy_pct(counts: &JudgmentCounts) -> f32 {
    let total = counts.total();
    if total == 0 {
        return 100.0;
    }
    let weighted = counts.perfect as f32 * 100.0
        + counts.great as f32 * 80.0
        + counts.good as f32 * 60.0
        + counts.ok as f32 * 40.0;
    weighted / total as f32
}

fn refresh_perf_info(
    show: Res<ShowPerfInfo>,
    scroll: Res<ScrollSettings>,
    input_offset: Res<InputOffsetMs>,
    bgm_adjust: Res<BgmAdjustState>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<(&mut Text, &mut Visibility), With<PerfInfoText>>,
) {
    let mult = scroll.pixels_per_ms / ScrollSettings::NX_BASE_PIXELS_PER_MS;
    let text = format!(
        "Scroll {:.1}x  In {:+}ms  BGM {:+}/{:+}ms",
        mult,
        input_offset.0,
        bgm_adjust.common_ms,
        bgm_adjust.song_ms,
    );
    for (mut t, mut vis) in &mut q {
        if show.0 {
            *vis = Visibility::Inherited;
            set_text_if_changed(&mut t, &mut cache.perf_info_text, text.clone());
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_count_matches_order() {
        assert_eq!(LANE_COUNT, LANE_ORDER.len());
    }

    #[test]
    fn accuracy_default_full() {
        assert!((accuracy_pct(&JudgmentCounts::default()) - 100.0).abs() < 0.01);
    }

    #[test]
    fn kind_label_perfect() {
        assert_eq!(kind_label(JudgmentKind::Perfect), "PERFECT");
    }
}
