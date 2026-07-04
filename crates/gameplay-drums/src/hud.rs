//! Drums performance HUD — DTXMania classic layout (ADR-0014 amended).
//!
//! Layers: frame chrome (top + side rails) → lane field (middle) → status
//! panels (left: SCORE + SCORE DETAILED + OPTIONS + SKILLS + pad chips; right:
//! NOW PLAYING + PHRASE METER). Mechanics are BocuD-ported; visuals here are
//! the classic DTXManiaNX layout, not the osu redesign.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_ui::{
    theme::Theme,
    widget::{
        frame_chrome, gauge_bar, judgment_popup, now_playing, options_panel, pad_chips,
        phrase_meter, score_detailed,
    },
    ThemeResource,
};
use game_shell::{AppState, EGameMode};

use crate::components::LastJudgment;
use crate::derived::ChartDerived;
use crate::gauge::{gauge_fill_color, StageGauge};
use crate::hud_cache::{set_text_if_changed, HudDisplayCache};
use crate::keyboard_viz;
use crate::lane_map::{LaneMap, LANE_ORDER};
use crate::phrase::PHRASE_SECTION_COUNT;

pub use crate::lane_map::LANE_COUNT;
use crate::layout::PlayfieldLayout;
use crate::playfield_viz;
use crate::resources::{
    ActiveChart, BgmAdjustState, Combo, FastSlowCount, InputOffsetMs, JudgmentCounts, Score,
    ScrollSettings, ShowPerfInfo, SkillValue,
};

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
                sync_score_number,
                sync_judgment_rows,
                sync_fast_slow,
                sync_skill_by_song,
                sync_options,
                sync_now_playing,
                sync_phrase_playhead,
                sync_hud_judgment,
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

    // Lane field (middle, between left status panel x<260 and right x>1020).
    let lane_field_x = 260.0 * layout.scale;
    let lane_field_w = 760.0 * layout.scale;
    let lane_strip_w = lane_field_w / LANE_COUNT as f32;

    commands.entity(root).with_children(|root| {
        // Lane field background.
        root.spawn((
            PlayfieldBackboard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(lane_field_x),
                top: Val::Px(60.0 * layout.scale),
                width: Val::Px(lane_field_w),
                height: Val::Px(660.0 * layout.scale),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.10, 0.92)),
        ));

        for lane in 0..LANE_COUNT {
            root.spawn((
                LaneColumn { lane },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(lane_field_x + lane as f32 * lane_strip_w),
                    top: Val::Px(80.0 * layout.scale),
                    width: Val::Px(lane_strip_w - 2.0),
                    height: Val::Px(540.0 * layout.scale),
                    ..default()
                },
                BackgroundColor(t.panel_bg),
            ));
        }

        // Yellow judge line (BocuD: yellow stripe at judge y).
        root.spawn((
            HitLine,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(lane_field_x),
                top: Val::Px(620.0 * layout.scale),
                width: Val::Px(lane_field_w),
                height: Val::Px(4.0 * layout.scale),
                ..default()
            },
            BackgroundColor(Color::srgb(0.95, 0.85, 0.1)),
        ));

        for (lane, ch) in LANE_ORDER.iter().enumerate() {
            root.spawn((
                LaneLabel { lane },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(lane_field_x + lane as f32 * lane_strip_w + 4.0),
                    top: Val::Px(72.0 * layout.scale),
                    ..default()
                },
                Text::new(lane_label(*ch)),
                Theme::label_font(),
                TextColor(t.text_secondary),
            ));
        }
    });

    // DTXMania-classic widget layers (over the lane field, inside the rails).
    frame_chrome::spawn_frame_chrome(&mut commands, root, &t);
    score_detailed::spawn_score_detailed_panel(&mut commands, root, &t);
    options_panel::spawn_options_panel(&mut commands, root, &t);
    now_playing::spawn_now_playing(&mut commands, root, &t);
    phrase_meter::spawn_phrase_meter(&mut commands, root, &t);
    pad_chips::spawn_pad_chips(&mut commands, root, &t);

    playfield_viz::spawn_lane_receptors(&mut commands, root, &layout, &t);
    keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &lane_map, &t);
    judgment_popup::spawn_judgment_popup(&mut commands, root, &t);
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

fn sync_hud_score_target(_score: Res<Score>) {
    // Replaced by sync_score_number in classic-hud layout.
}

fn sync_hud_combo(
    _combo: Res<Combo>,
) {
    // Replaced by sync_judgment_rows.
}

fn sync_hud_judgment(
    last: Res<LastJudgment>,
    theme: Res<ThemeResource>,
    time: Res<Time>,
    mut prev: Local<Option<(JudgmentKind, i64)>>,
    mut q: Query<(
        &mut judgment_popup::JudgmentPopup,
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

fn refresh_perf_info(_show: Res<ShowPerfInfo>) {
    // Replaced by sync_options.
}

// === Classic DTXMania HUD sync systems ===

fn sync_score_number(
    score: Res<Score>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<score_detailed::ScoreNumberText>>,
) {
    if !score.is_changed() {
        return;
    }
    let s = format!("{:07}", score.0);
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.score_text, s.clone());
    }
}

fn sync_judgment_rows(
    counts: Res<JudgmentCounts>,
    combo: Res<Combo>,
    derived: Res<ChartDerived>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<(&score_detailed::JudgmentRowText, &mut Text)>,
) {
    if !counts.is_changed() && !combo.is_changed() {
        return;
    }
    let total = derived.total_drum_chips.max(1);
    for (row, mut t) in &mut q {
        let (label, value) = match row.kind {
            0 => ("Perfect", counts.perfect),
            1 => ("Great", counts.great),
            2 => ("Good", counts.good),
            3 => ("Ok", counts.ok),
            4 => ("Miss", counts.miss),
            5 => ("MaxCombo", combo.max),
            _ => ("?", 0),
        };
        let pct = (value as f32 / total as f32) * 100.0;
        let text = format!("{label:<10} {value:>4}  {pct:>3.0}%");
        set_text_if_changed(&mut t, &mut cache.counters_text, text);
    }
}

fn sync_fast_slow(
    fs: Res<FastSlowCount>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<score_detailed::FastSlowText>>,
) {
    if !fs.is_changed() {
        return;
    }
    let text = format!("Fast {:>4}   Slow {:>4}", fs.fast, fs.slow);
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.counters_text, text.clone());
    }
}

fn sync_skill_by_song(
    skill: Res<SkillValue>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<score_detailed::SkillBySongText>>,
) {
    if !skill.is_changed() {
        return;
    }
    let text = format!("{:>5.2}", skill.current);
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.counters_text, text.clone());
    }
}

fn sync_options(
    scroll: Res<ScrollSettings>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<(&options_panel::OptionRowText, &mut Text)>,
) {
    if !scroll.is_changed() {
        return;
    }
    let mult = scroll.pixels_per_ms / ScrollSettings::NX_BASE_PIXELS_PER_MS;
    for (row, mut t) in &mut q {
        let text = match row.kind {
            0 => format!("Speed   x{mult:.1}"),
            1 => "Risky   OFF".to_string(),
            2 => "Auto    OFF".to_string(),
            3 => "Mirror  OFF".to_string(),
            _ => String::new(),
        };
        set_text_if_changed(&mut t, &mut cache.perf_info_text, text);
    }
}

fn sync_now_playing(
    chart: Res<ActiveChart>,
    derived: Res<ChartDerived>,
    mut q_title: Query<&mut Text, With<now_playing::NowPlayingTitle>>,
    mut q_artist: Query<&mut Text, (With<now_playing::NowPlayingArtist>, Without<now_playing::NowPlayingTitle>)>,
    mut q_maker: Query<&mut Text, (With<now_playing::NowPlayingMaker>, Without<now_playing::NowPlayingTitle>, Without<now_playing::NowPlayingArtist>)>,
    mut q_diff: Query<&mut Text, (With<now_playing::NowPlayingDifficulty>, Without<now_playing::NowPlayingTitle>, Without<now_playing::NowPlayingArtist>, Without<now_playing::NowPlayingMaker>)>,
) {
    let title = chart.chart.metadata.title.as_deref().unwrap_or("— no chart —");
    let artist = chart.chart.metadata.artist.as_deref().unwrap_or("");
    let maker = chart.chart.metadata.maker.as_deref().unwrap_or("");
    let diff_label = dlevel_label(chart.chart.metadata.dlevel);
    let diff_num = derived.chart_level;
    for mut t in &mut q_title {
        *t = Text::new(title);
    }
    for mut t in &mut q_artist {
        *t = Text::new(artist);
    }
    for mut t in &mut q_maker {
        *t = Text::new(maker);
    }
    for mut t in &mut q_diff {
        *t = Text::new(format!("{diff_label}  {diff_num:.2}"));
    }
}

fn dlevel_label(dlevel: Option<u32>) -> &'static str {
    // BocuD uses 0=DTXMANIA, 1=BASIC..5=MASTER, 6+=EXTREME.
    match dlevel {
        Some(0) => "DTXMANIA",
        Some(1) => "BASIC",
        Some(2) => "NOVICE",
        Some(3) => "REGULAR",
        Some(4) => "EXPERT",
        Some(5) => "MASTER",
        Some(6) | Some(7) => "EXTREME",
        _ => "BASIC",
    }
}

fn sync_phrase_playhead(
    derived: Res<ChartDerived>,
    gameplay_clock: Res<crate::resources::GameplayClock>,
    mut q: Query<&mut Node, With<phrase_meter::PhrasePlayhead>>,
) {
    if !derived.is_changed() && !gameplay_clock.is_changed() {
        return;
    }
    let last = derived.phrase.last_chip_ms.max(1) as f32;
    let now = gameplay_clock.current_ms as f32;
    let frac = (now / last).clamp(0.0, 1.0);
    let bar_h = 540.0_f32;
    let bar_y = 15.0_f32;
    let y = bar_y + (1.0 - frac) * bar_h;
    for mut n in &mut q {
        n.top = Val::Px(y);
    }
    let _ = PHRASE_SECTION_COUNT; // keep import
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
