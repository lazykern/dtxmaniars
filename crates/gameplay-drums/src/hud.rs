//! Drums performance HUD — centered playfield (osu-style) + corner overlays.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_ui::{
    widget::{
        combo_display::ComboDisplay, frame_chrome, hud_ref::HudRefRect, judgment_popup,
        now_playing, perf_combo, phrase_meter, playfield_speed, score_detailed, song_progress,
    },
    ThemeResource,
};
use game_shell::{AppState, EGameMode};

use crate::components::LastJudgment;
use crate::derived::ChartDerived;
use crate::hud_cache::{set_text_if_changed, HudDisplayCache};
use crate::keyboard_viz;
use crate::lane_geometry;

pub use crate::lane_map::LANE_COUNT;
use crate::layout::{ref_hud_right_x, ref_phrase_x, PlayfieldLayout};
use crate::playfield_viz;
use crate::resources::{
    ActiveChart, Combo, FastSlowCount, GameplayClock, JudgmentCounts, Score, ScrollSettings,
    SkillValue,
};

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
struct LaneColumn {
    col: usize,
}

#[derive(Component)]
struct PlayfieldBackboard;

#[derive(Component)]
struct HitLine;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        (
            spawn_hud,
            (
                apply_backboard_layout,
                apply_lane_column_layout,
                apply_hit_line_layout,
                apply_progress_layout,
                apply_speed_layout,
                apply_hud_ref_layout,
            )
                .chain(),
        )
            .chain(),
    )
    .add_systems(OnExit(AppState::Performance), despawn_hud)
    .add_systems(
        Update,
        (
            (
                apply_backboard_layout,
                apply_lane_column_layout,
                apply_hit_line_layout,
                apply_progress_layout,
                apply_speed_layout,
                apply_hud_ref_layout,
            )
                .chain()
                .run_if(resource_changed::<PlayfieldLayout>),
            sync_score_number,
            sync_judgment_rows,
            sync_accuracy,
            sync_difficulty_badge,
            sync_fast_slow,
            sync_skill_by_song,
            sync_playfield_speed,
            sync_perf_combo,
            sync_now_playing,
            sync_song_progress,
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
) {
    if *mode != EGameMode::Drums {
        return;
    }
    let t = theme.0;
    let s = layout.scale;
    let root = commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.0, 0.0, 0.0)),
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
            BackgroundColor(Color::srgba(0.06, 0.07, 0.11, 0.88)),
        ));

        for col in 0..lane_geometry::COLUMN_COUNT {
            let tint = lane_geometry::lane_fill_color(col);
            root.spawn((
                LaneColumn { col },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.col_left(col)),
                    top: Val::Px(layout.lane_top()),
                    width: Val::Px(layout.col_width(col) - 2.0),
                    height: Val::Px(layout.lane_height()),
                    ..default()
                },
                BackgroundColor(tint),
            ));
        }

        root.spawn((
            HitLine,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.strip_left()),
                top: Val::Px(layout.judge_y()),
                width: Val::Px(layout.strip_width()),
                height: Val::Px(4.0 * layout.scale),
                ..default()
            },
            BackgroundColor(Color::srgb(0.95, 0.85, 0.1)),
        ));
    });

    frame_chrome::spawn_frame_chrome(&mut commands, root, &t, s);
    score_detailed::spawn_score_detailed_panel(&mut commands, root, &t, s);
    phrase_meter::spawn_phrase_meter(&mut commands, root, &t, s, ref_phrase_x());
    song_progress::spawn_song_progress(
        &mut commands,
        root,
        &t,
        s,
        lane_geometry::STRIP_REF_LEFT,
        lane_geometry::STRIP_REF_WIDTH,
    );
    playfield_speed::spawn_playfield_speed(
        &mut commands,
        root,
        &t,
        s,
        lane_geometry::STRIP_REF_LEFT + lane_geometry::STRIP_REF_WIDTH - 96.0,
    );
    let hud_right = ref_hud_right_x();
    now_playing::spawn_now_playing(&mut commands, root, &t, s, hud_right);
    // Combo below the song-info card (was clipping at y=72). Card ≈ y 20..118.
    perf_combo::spawn_perf_combo(&mut commands, root, &t, s, hud_right, 150.0);

    playfield_viz::spawn_lane_receptors(&mut commands, root, &layout, &t);
    keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &t);
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
        node.left = Val::Px(layout.col_left(col.col));
        node.top = Val::Px(layout.lane_top());
        node.width = Val::Px(layout.col_width(col.col) - 2.0);
        node.height = Val::Px(layout.lane_height());
    }
}

fn apply_hit_line_layout(
    layout: Res<PlayfieldLayout>,
    mut hit_line: Query<&mut Node, With<HitLine>>,
) {
    for mut node in &mut hit_line {
        node.left = Val::Px(layout.strip_left());
        node.top = Val::Px(layout.judge_y());
        node.width = Val::Px(layout.strip_width());
        node.height = Val::Px(3.0 * layout.scale);
    }
}

fn apply_progress_layout(
    layout: Res<PlayfieldLayout>,
    mut track: Query<&mut Node, (With<song_progress::SongProgressTrack>, Without<song_progress::SongProgressFill>)>,
    mut fill: Query<&mut Node, (With<song_progress::SongProgressFill>, Without<song_progress::SongProgressTrack>)>,
) {
    for mut node in &mut track {
        node.left = Val::Px(layout.progress_bar_left());
        node.top = Val::Px(layout.progress_bar_top());
        node.width = Val::Px(layout.progress_bar_width());
    }
    for mut node in &mut fill {
        node.left = Val::Px(layout.progress_bar_left());
        node.top = Val::Px(layout.progress_bar_top());
    }
}

fn apply_speed_layout(
    layout: Res<PlayfieldLayout>,
    mut speed: Query<(&HudRefRect, &mut Node), With<playfield_speed::PlayfieldSpeedText>>,
) {
    for (rect, mut node) in &mut speed {
        rect.apply(layout.scale, &mut node);
    }
}

fn apply_hud_ref_layout(
    layout: Res<PlayfieldLayout>,
    mut q: Query<
        (&HudRefRect, &mut Node),
        (
            Without<PlayfieldBackboard>,
            Without<LaneColumn>,
            Without<HitLine>,
            Without<song_progress::SongProgressFill>,
            Without<playfield_speed::PlayfieldSpeedText>,
            Without<phrase_meter::PhrasePlayhead>,
            Without<phrase_meter::PhraseSection>,
        ),
    >,
) {
    for (rect, mut node) in &mut q {
        rect.apply(layout.scale, &mut node);
    }
}

fn despawn_hud(mut commands: Commands, query: Query<Entity, With<HudRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
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

fn kind_label(kind: JudgmentKind) -> &'static str {
    match kind {
        JudgmentKind::Perfect => "PERFECT",
        JudgmentKind::Great => "GREAT",
        JudgmentKind::Good => "GOOD",
        JudgmentKind::Poor => "POOR",
        JudgmentKind::Miss => "MISS",
    }
}

fn sync_score_number(
    score: Res<Score>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<score_detailed::ScoreNumberText>>,
) {
    if !score.is_changed() {
        return;
    }
    let s = format!("{}", score.0);
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
        let text = format!("{label:<8} {value:>3}  {pct:>3.0}%");
        set_text_if_changed(&mut t, &mut cache.counters_text, text);
    }
}

fn sync_accuracy(
    counts: Res<JudgmentCounts>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<score_detailed::AccuracyText>>,
) {
    if !counts.is_changed() {
        return;
    }
    let text = format!("{:.2}%", accuracy_pct(&counts));
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.counters_text, text.clone());
    }
}

fn sync_difficulty_badge(
    chart: Res<ActiveChart>,
    derived: Res<ChartDerived>,
    mut q: Query<&mut Text, With<score_detailed::DifficultyBadgeText>>,
) {
    if !chart.is_changed() && !derived.is_changed() {
        return;
    }
    let label = dlevel_label(chart.chart.metadata.dlevel);
    let text = format!("{label} {:.2}", derived.chart_level);
    for mut t in &mut q {
        *t = Text::new(text.clone());
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
    let text = format!("Fast {:>3}   Slow {:>3}", fs.fast, fs.slow);
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
    let text = format!("{:.2}", skill.current);
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.counters_text, text.clone());
    }
}

fn sync_playfield_speed(
    scroll: Res<ScrollSettings>,
    mut cache: ResMut<HudDisplayCache>,
    mut q: Query<&mut Text, With<playfield_speed::PlayfieldSpeedText>>,
) {
    if !scroll.is_changed() {
        return;
    }
    let mult = scroll.pixels_per_ms / ScrollSettings::NX_BASE_PIXELS_PER_MS;
    let text = format!("SPEED {mult:.1}");
    for mut t in &mut q {
        set_text_if_changed(&mut t, &mut cache.perf_info_text, text.clone());
    }
}

fn sync_perf_combo(
    combo: Res<Combo>,
    time: Res<Time>,
    mut q: Query<(&mut ComboDisplay, &mut Text), With<perf_combo::PerfComboNumber>>,
) {
    if !combo.is_changed() && !time.is_changed() {
        return;
    }
    let delta = time.delta_secs() * 1000.0;
    for (mut display, mut text) in &mut q {
        display.set_combo(combo.current);
        display.tick(delta);
        *text = Text::new(format!("{}", display.last_combo));
    }
}

fn sync_now_playing(
    chart: Res<ActiveChart>,
    mut q_title: Query<&mut Text, With<now_playing::NowPlayingTitle>>,
    mut q_artist: Query<
        &mut Text,
        (
            With<now_playing::NowPlayingArtist>,
            Without<now_playing::NowPlayingTitle>,
        ),
    >,
    mut q_maker: Query<
        &mut Text,
        (
            With<now_playing::NowPlayingMaker>,
            Without<now_playing::NowPlayingTitle>,
            Without<now_playing::NowPlayingArtist>,
        ),
    >,
) {
    if !chart.is_changed() {
        return;
    }
    let title = chart.chart.metadata.title.as_deref().unwrap_or("— no chart —");
    let artist = chart.chart.metadata.artist.as_deref().unwrap_or("");
    let maker = chart.chart.metadata.maker.as_deref().unwrap_or("");
    for mut t in &mut q_title {
        *t = Text::new(title);
    }
    for mut t in &mut q_artist {
        *t = Text::new(artist);
    }
    for mut t in &mut q_maker {
        *t = Text::new(maker);
    }
}

fn sync_song_progress(
    derived: Res<ChartDerived>,
    clock: Res<GameplayClock>,
    layout: Res<PlayfieldLayout>,
    mut fill: Query<
        &mut Node,
        (
            With<song_progress::SongProgressFill>,
            Without<song_progress::SongProgressTrack>,
        ),
    >,
) {
    if !derived.is_changed() && !clock.is_changed() {
        return;
    }
    let last = derived.phrase.last_chip_ms.max(1) as f32;
    let now = clock.current_ms as f32;
    let frac = (now / last).clamp(0.0, 1.0);
    let w = layout.progress_bar_width() * frac;
    for mut node in &mut fill {
        node.width = Val::Px(w);
    }
}

fn dlevel_label(dlevel: Option<u32>) -> &'static str {
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
    gameplay_clock: Res<GameplayClock>,
    layout: Res<PlayfieldLayout>,
    mut q: Query<(&HudRefRect, &mut Node), With<phrase_meter::PhrasePlayhead>>,
) {
    if !derived.is_changed() && !gameplay_clock.is_changed() && !layout.is_changed() {
        return;
    }
    let last = derived.phrase.last_chip_ms.max(1) as f32;
    let now = gameplay_clock.current_ms as f32;
    let frac = (now / last).clamp(0.0, 1.0);
    let bar_h = 540.0;
    let y = (1.0 - frac) * bar_h;
    for (rect, mut n) in &mut q {
        n.top = Val::Px(y * layout.scale);
        n.left = Val::Px(rect.left * layout.scale);
        n.width = Val::Px(rect.width * layout.scale);
        n.height = Val::Px(rect.height * layout.scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_map::LANE_ORDER;

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
