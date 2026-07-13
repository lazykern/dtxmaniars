//! Drums performance HUD — centered playfield (osu-style) + corner overlays.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_ui::{
    theme::REF_WIDTH,
    widget::{
        combo_display::ComboDisplay, frame_chrome, gauge_bar, hud_ref::HudRefRect, judgment_popup,
        live_graph, now_playing, perf_combo, phrase_meter, playfield_speed, score_detailed,
        song_progress,
    },
    ThemeResource,
};
use game_shell::{AppState, EGameMode};

use dtx_layout::WidgetKind;

use crate::components::LastJudgment;
use crate::derived::ChartDerived;
use crate::hud_cache::{set_text_if_changed, HudDisplayCache};
use crate::keyboard_viz;
use crate::widget_layout::WidgetContainer;

pub use crate::lane_map::LANE_COUNT;
use crate::layout::PlayfieldLayout;
use crate::resources::{
    AccuracyHistory, ActiveChart, Combo, FastSlowCount, GameplayClock, JudgmentCounts, Score,
    ScrollSettings, SkillValue,
};

/// Live accuracy graph geometry (ref px). Shared by spawn + sync so bar
/// heights and the panel stay aligned.
const GRAPH_REF_Y: f32 = 96.0;
const GRAPH_REF_H: f32 = 500.0;

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
struct PlayfieldBackboard;

/// Alpha of the dark strip behind the lanes that keeps notes readable over
/// bright BGA frames.
const LANE_BACKDROP_ALPHA: f32 = 0.55;

#[derive(Component)]
struct LaneBackdrop;

#[derive(Component)]
struct HitLine;

#[derive(Component)]
struct NoFailBadge;

const fn no_fail_badge_text(enabled: bool) -> Option<&'static str> {
    if enabled {
        Some("NO FAIL ◈")
    } else {
        None
    }
}

/// Spawn a per-widget container under `root` (absolute, ref-origin 0,0,
/// full-size) that `apply_widget_layout` positions. Returns the container
/// entity to parent the widget's children to.
fn spawn_widget_container(commands: &mut Commands, root: Entity, kind: WidgetKind) -> Entity {
    let container = commands
        .spawn((
            WidgetContainer(kind),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ZIndex(0),
            bevy::ui::UiTransform::default(),
            Visibility::Inherited,
            // Full-screen transparent overlay: never absorb pointer events, so
            // the plan-3 editor can pick widgets/playfield beneath it.
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(container);
    container
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        (
            spawn_hud,
            (
                apply_backboard_layout,
                apply_hit_line_layout,
                apply_gauge_layout,
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
                apply_hit_line_layout,
                apply_gauge_layout,
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
            sync_stage_gauge,
            sync_phrase_meter,
            sync_phrase_playhead,
            sync_hud_judgment,
            keyboard_viz::decay_key_cap_flashes,
            sample_accuracy_history,
            sync_live_graph,
        )
            .run_if(in_state(AppState::Performance)),
    );
}

fn spawn_hud(
    mut commands: Commands,
    mode: Res<EGameMode>,
    theme: Res<ThemeResource>,
    layout: Res<PlayfieldLayout>,
    lanes: Res<crate::lanes::Lanes>,
    mut history: ResMut<AccuracyHistory>,
    no_fail: Res<crate::resources::NoFailEnabled>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    *history = AccuracyHistory::default();
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
            // Single stage transform (identity in normal play). The Customize
            // surface shrinks the whole scene — playfield + every HUD widget,
            // all children of this root — into a miniature via this one
            // transform (`stage_rect::apply_stage_transform`).
            bevy::ui::UiTransform::default(),
            BackgroundColor(Color::srgb(0.0, 0.0, 0.0)),
        ))
        .id();

    if let Some(label) = no_fail_badge_text(no_fail.0) {
        commands.entity(root).with_children(|root| {
            root.spawn((
                NoFailBadge,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    top: Val::Px(20.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.92)),
                BorderColor::all(t.accent),
                Text::new(label),
                dtx_ui::Theme::font(14.0),
                TextColor(t.text_primary),
            ));
        });
    }

    // Playfield (backboard, hit line, key caps, notes) stays parented to `root`:
    // moving/scaling it as a widget needs PlayfieldLayout to take an origin
    // offset, deferred to a later plan. WidgetKind::Playfield exists in the
    // registry only so the editor can list it (drag disabled there in v1).
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
            BackgroundColor(Color::BLACK),
            // Backmost HudRoot child: BGA movie (-3) and images (-2) render on
            // top of this black board, the lane backdrop (-1) dims the strip,
            // notes/HUD (>=0) on top of everything.
            ZIndex(-4),
        ));

        // Semi-opaque dark strip over the BGA so notes stay readable against
        // bright video frames.
        root.spawn((
            LaneBackdrop,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.strip_left()),
                top: Val::Px(layout.backboard_top()),
                width: Val::Px(layout.strip_width()),
                height: Val::Px(layout.backboard_height()),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(LANE_BACKDROP_ALPHA)),
            ZIndex(-1),
        ));

        root.spawn((
            HitLine,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.strip_left()),
                top: Val::Px(layout.judge_y()),
                width: Val::Px(layout.strip_width()),
                height: Val::Px(3.0 * layout.scale),
                ..default()
            },
            BackgroundColor(Color::srgb(0.95, 0.85, 0.1)),
        ));
    });

    let c_frame = spawn_widget_container(&mut commands, root, WidgetKind::FrameChrome);
    frame_chrome::spawn_frame_chrome(
        &mut commands,
        c_frame,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_left() + layout.ref_strip_width(),
    );
    let c_gauge = spawn_widget_container(&mut commands, root, WidgetKind::Gauge);
    gauge_bar::spawn_stage_gauge(
        &mut commands,
        c_gauge,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_width(),
    );
    let c_score = spawn_widget_container(&mut commands, root, WidgetKind::ScorePanel);
    score_detailed::spawn_score_detailed_panel(&mut commands, c_score, &t, s);
    let c_phrase = spawn_widget_container(&mut commands, root, WidgetKind::PhraseMeter);
    phrase_meter::spawn_phrase_meter(&mut commands, c_phrase, &t, s, layout.ref_phrase_x());
    let c_progress = spawn_widget_container(&mut commands, root, WidgetKind::SongProgress);
    song_progress::spawn_song_progress(
        &mut commands,
        c_progress,
        &t,
        s,
        layout.ref_strip_left(),
        layout.ref_strip_width(),
    );
    // SPEED lives in the left OPTIONS area (was clipping the CY/RD pads).
    let c_speed = spawn_widget_container(&mut commands, root, WidgetKind::SpeedReadout);
    playfield_speed::spawn_playfield_speed(&mut commands, c_speed, &t, s, 24.0, 470.0);
    let hud_right = layout.ref_hud_right_x();
    let c_now_playing = spawn_widget_container(&mut commands, root, WidgetKind::NowPlaying);
    now_playing::spawn_now_playing(&mut commands, c_now_playing, &t, s, hud_right);
    // Combo centered on the recentered lane strip (was pinned to the right column).
    let combo_ref_x = layout.ref_strip_left() + layout.ref_strip_width() / 2.0 - 180.0;
    let c_combo = spawn_widget_container(&mut commands, root, WidgetKind::Combo);
    perf_combo::spawn_perf_combo(&mut commands, c_combo, &t, s, combo_ref_x, 150.0);
    // Live accuracy graph fills the right column below the song card.
    let graph_x = hud_right + 40.0;
    let graph_w = REF_WIDTH - graph_x - 12.0;
    let c_live_graph = spawn_widget_container(&mut commands, root, WidgetKind::LiveGraph);
    live_graph::spawn_live_graph(
        &mut commands,
        c_live_graph,
        &t,
        s,
        graph_x,
        GRAPH_REF_Y,
        graph_w,
        GRAPH_REF_H,
    );

    keyboard_viz::spawn_key_caps(&mut commands, root, &layout, &lanes, &t);
    let c_judgment = spawn_widget_container(&mut commands, root, WidgetKind::JudgmentPopup);
    judgment_popup::spawn_judgment_popup(&mut commands, c_judgment, &t);

    // Route BGA image/movie overlays under HudRoot so they ride the stage
    // transform (align/shrink with the scene in the Customize editor) and sit in
    // its stacking context behind lanes/HUD via negative ZIndex.
    commands.insert_resource(dtx_bga::BgaParent(Some(root)));
}

fn apply_backboard_layout(
    layout: Res<PlayfieldLayout>,
    mut backboards: Query<&mut Node, With<PlayfieldBackboard>>,
    mut backdrops: Query<&mut Node, (With<LaneBackdrop>, Without<PlayfieldBackboard>)>,
) {
    for mut node in &mut backboards {
        node.left = Val::Px(layout.backboard_left());
        node.top = Val::Px(layout.backboard_top());
        node.width = Val::Px(layout.backboard_width());
        node.height = Val::Px(layout.backboard_height());
    }
    for mut node in &mut backdrops {
        node.left = Val::Px(layout.strip_left());
        node.top = Val::Px(layout.backboard_top());
        node.width = Val::Px(layout.strip_width());
        node.height = Val::Px(layout.backboard_height());
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

fn apply_gauge_layout(
    layout: Res<PlayfieldLayout>,
    mut tracks: Query<(&mut Node, &mut gauge_bar::GaugeBarWidget)>,
    mut ticks: Query<
        &mut Node,
        (
            With<gauge_bar::GaugeThresholdTick>,
            Without<gauge_bar::GaugeBarWidget>,
        ),
    >,
) {
    let s = layout.scale;
    for (mut node, mut bar) in &mut tracks {
        node.left = Val::Px(layout.strip_left());
        node.top = Val::Px(64.0 * s + layout.origin.y);
        node.width = Val::Px(layout.strip_width());
        node.height = Val::Px(10.0 * s);
        bar.track_width = layout.strip_width();
    }
    for mut node in &mut ticks {
        node.top = Val::Px(-2.0 * s);
        node.width = Val::Px(2.0 * s);
        node.height = Val::Px(14.0 * s);
    }
}

fn sync_stage_gauge(
    gauge: Res<crate::gauge::StageGauge>,
    time: Res<Time>,
    mut bars: Query<&mut gauge_bar::GaugeBarWidget>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<gauge_bar::GaugeFill>>,
) {
    // One stage gauge per HUD.
    let Some(mut bar) = bars.iter_mut().next() else {
        return;
    };
    bar.set_pct(gauge.pct());
    bar.tick(time.delta_secs() * 1000.0);
    for (mut node, mut color) in &mut fills {
        node.width = Val::Px(bar.fill_width());
        color.0 = crate::gauge::gauge_fill_color(gauge.value, gauge.failed);
    }
}

fn apply_progress_layout(
    layout: Res<PlayfieldLayout>,
    mut track: Query<
        &mut Node,
        (
            With<song_progress::SongProgressTrack>,
            Without<song_progress::SongProgressFill>,
        ),
    >,
    mut fill: Query<
        &mut Node,
        (
            With<song_progress::SongProgressFill>,
            Without<song_progress::SongProgressTrack>,
        ),
    >,
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
        rect.apply(layout.scale, layout.origin, &mut node);
    }
}

fn apply_hud_ref_layout(
    layout: Res<PlayfieldLayout>,
    mut q: Query<
        (&HudRefRect, &mut Node),
        (
            Without<PlayfieldBackboard>,
            Without<HitLine>,
            Without<song_progress::SongProgressFill>,
            Without<playfield_speed::PlayfieldSpeedText>,
            Without<phrase_meter::PhrasePlayhead>,
            Without<phrase_meter::PhraseSection>,
            Without<live_graph::LiveGraphBar>,
        ),
    >,
) {
    for (rect, mut node) in &mut q {
        rect.apply(layout.scale, layout.origin, &mut node);
    }
}

fn despawn_hud(mut commands: Commands, query: Query<Entity, With<HudRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    // HudRoot is gone; drop the stale parent so BGA overlays don't try to attach
    // to a despawned entity outside Performance.
    commands.insert_resource(dtx_bga::BgaParent(None));
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
        &mut UiTransform,
    )>,
) {
    if let Some(ev) = last.0 {
        let key = (ev.kind, ev.delta_ms);
        if prev.as_ref() != Some(&key) {
            *prev = Some(key);
            let label = kind_label(ev.kind);
            for (mut popup, mut text, mut color, mut vis, _) in &mut q {
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
    for (mut popup, _, mut color, mut vis, mut transform) in &mut q {
        let (alpha, scale) = popup.tick(delta);
        color.0 = color.0.with_alpha(alpha);
        transform.scale = Vec2::splat(scale);
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
    let text = format!("{:.2}%", counts.achievement_pct());
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
    mut q: Query<
        (&mut ComboDisplay, &mut Text, &mut UiTransform),
        With<perf_combo::PerfComboNumber>,
    >,
) {
    let delta = time.delta_secs() * 1000.0;
    for (mut display, mut text, mut transform) in &mut q {
        display.set_combo(combo.current);
        display.tick(delta);
        transform.scale = Vec2::splat(display.scale());
        *text = Text::new(format!("{}", display.last_combo));
    }
}

/// Resolve the performance cover image (`#PREIMAGE`) for a chart against its
/// source directory, matching case-insensitively. Returns `None` when metadata
/// or the file is absent (fallback tile stays visible).
///
/// Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/InfoBox.cs:20-34`
pub fn performance_preimage_path(chart: &ActiveChart) -> Option<std::path::PathBuf> {
    let filename = chart.chart.metadata.preimage_filename.as_deref()?;
    let dir = chart.source_path.as_ref()?.parent()?;
    dtx_core::resolve_chart_asset_path(dir, filename)
}

#[allow(clippy::type_complexity)]
fn sync_now_playing(
    chart: Res<ActiveChart>,
    asset_server: Res<AssetServer>,
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
    mut q_art: Query<(&mut ImageNode, &mut BackgroundColor), With<now_playing::NowPlayingArt>>,
) {
    if !chart.is_changed() {
        return;
    }
    let title = chart
        .chart
        .metadata
        .title
        .as_deref()
        .unwrap_or("— no chart —");
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

    let cover = performance_preimage_path(&chart);
    for (mut image, mut bg) in &mut q_art {
        match &cover {
            Some(path) => {
                image.image = asset_server.load(path.to_string_lossy().to_string());
                image.color = Color::WHITE;
                bg.0 = bg.0.with_alpha(0.0);
            }
            None => {
                image.image = Handle::default();
                image.color = image.color.with_alpha(0.0);
                bg.0 = Color::srgb(0.15, 0.15, 0.2);
            }
        }
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

fn sync_phrase_meter(
    derived: Res<ChartDerived>,
    clock: Res<GameplayClock>,
    layout: Res<PlayfieldLayout>,
    mut q: Query<(
        &phrase_meter::PhraseSection,
        &HudRefRect,
        &mut Node,
        &mut BackgroundColor,
    )>,
) {
    if !derived.is_changed() && !clock.is_changed() && !layout.is_changed() {
        return;
    }
    let s = layout.scale;
    let unit_w = phrase_meter::PHRASE_BAR_W / 10.0;
    let last = derived.phrase.last_chip_ms.max(1) as f32;
    let now = clock.current_ms as f32;
    let frac = (now / last).clamp(0.0, 1.0);
    // 0 = top (chart end) … 1 = bottom (chart start); song fills bottom→up.
    let head_from_top = 1.0 - frac;
    let blocks = phrase_meter::PHRASE_BLOCKS as f32;
    let cur = ((head_from_top * blocks) as usize).min(phrase_meter::PHRASE_BLOCKS - 1);
    for (sec, rect, mut node, mut color) in &mut q {
        let units = derived.phrase.block_units(sec.index);
        node.left = Val::Px(rect.left * s + layout.origin.x);
        node.top = Val::Px(rect.top * s + layout.origin.y);
        node.height = Val::Px(rect.height * s);
        node.width = Val::Px(unit_w * units as f32 * s);
        let center_from_top = (sec.index as f32 + 0.5) / blocks;
        *color = if sec.index == cur {
            BackgroundColor(Color::srgb(0.30, 0.72, 1.0))
        } else if center_from_top >= head_from_top {
            BackgroundColor(Color::srgb(0.95, 0.85, 0.1))
        } else {
            BackgroundColor(Color::srgb(0.32, 0.34, 0.42))
        };
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
    let bar_h = phrase_meter::PHRASE_BAR_H;
    let y = (1.0 - frac) * bar_h;
    for (rect, mut n) in &mut q {
        n.top = Val::Px(y * layout.scale + layout.origin.y);
        n.left = Val::Px(rect.left * layout.scale + layout.origin.x);
        n.width = Val::Px(rect.width * layout.scale);
        n.height = Val::Px(rect.height * layout.scale);
    }
}

fn sample_accuracy_history(
    counts: Res<JudgmentCounts>,
    clock: Res<GameplayClock>,
    derived: Res<ChartDerived>,
    mut history: ResMut<AccuracyHistory>,
) {
    if !clock.is_changed() {
        return;
    }
    let total = derived.phrase.last_chip_ms.max(1);
    let slot = live_graph::slot_for_pos(clock.current_ms, total);
    history.record(slot, counts.achievement_pct());
}

fn sync_live_graph(
    history: Res<AccuracyHistory>,
    layout: Res<PlayfieldLayout>,
    mut q: Query<(&live_graph::LiveGraphBar, &HudRefRect, &mut Node)>,
) {
    if !history.is_changed() && !layout.is_changed() {
        return;
    }
    let s = layout.scale;
    let bar_area_h = live_graph::bar_area_h(GRAPH_REF_H);
    for (bar, rect, mut node) in &mut q {
        // Bars are excluded from apply_hud_ref_layout, so re-apply x/width here
        // too or they detach from the panel on a window-scale change.
        node.left = Val::Px(rect.left * s + layout.origin.x);
        node.width = Val::Px(rect.width.max(1.0) * s);
        let Some(acc) = history.samples.get(bar.slot).copied().flatten() else {
            node.top = Val::Px(rect.top * s + layout.origin.y);
            node.height = Val::Px(0.0);
            continue;
        };
        let h = live_graph::bar_height(acc, bar_area_h);
        node.top = Val::Px((rect.top - h) * s + layout.origin.y);
        node.height = Val::Px(h * s);
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
        assert!((JudgmentCounts::default().achievement_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn kind_label_perfect() {
        assert_eq!(kind_label(JudgmentKind::Perfect), "PERFECT");
    }

    #[test]
    fn no_fail_badge_combines_text_and_non_color_marker() {
        assert_eq!(no_fail_badge_text(false), None);
        assert_eq!(no_fail_badge_text(true), Some("NO FAIL ◈"));
    }

    fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "gameplay-drums-{}-{}-{}",
            tag,
            std::process::id(),
            std::thread::current().name().unwrap_or("t")
        ))
    }

    fn chart_with_preimage(
        preimage: Option<&str>,
        source: Option<std::path::PathBuf>,
    ) -> ActiveChart {
        ActiveChart::new(
            dtx_core::chart::Chart {
                metadata: dtx_core::chart::Metadata {
                    preimage_filename: preimage.map(str::to_string),
                    ..Default::default()
                },
                ..Default::default()
            },
            source,
        )
    }

    #[test]
    fn performance_preimage_resolves_case_insensitively() {
        let dir = unique_temp_dir("cover-ok");
        std::fs::create_dir_all(&dir).expect("create cover dir");
        std::fs::write(dir.join("Cover.PNG"), b"image").expect("write cover");
        let chart = chart_with_preimage(Some("cover.png"), Some(dir.join("song.dtx")));
        assert_eq!(
            performance_preimage_path(&chart),
            Some(dir.join("Cover.PNG"))
        );
        std::fs::remove_dir_all(dir).expect("remove cover dir");
    }

    #[test]
    fn performance_preimage_none_without_metadata() {
        let chart = chart_with_preimage(None, Some(std::path::PathBuf::from("/x/song.dtx")));
        assert_eq!(performance_preimage_path(&chart), None);
    }

    #[test]
    fn performance_preimage_none_when_file_missing() {
        let dir = unique_temp_dir("cover-missing");
        std::fs::create_dir_all(&dir).expect("create dir");
        let chart = chart_with_preimage(Some("nope.png"), Some(dir.join("song.dtx")));
        assert_eq!(performance_preimage_path(&chart), None);
        std::fs::remove_dir_all(dir).expect("remove dir");
    }
}
